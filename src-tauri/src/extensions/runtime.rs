//! Runtime de extensões baseado no Deno como processo sidecar.
//!
//! Cada execução sobe um processo Deno isolado, com permissões derivadas do
//! que o usuário aprovou. O runner escreve o resultado num arquivo JSON, então
//! não dependemos de stdout (evita poluição de logs da extensão).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use wait_timeout::ChildExt;

use crate::extensions::ExtensionManifest;
use crate::platform::hidden_command;

pub struct RunOutcome {
    pub ok: bool,
    pub result: serde_json::Value,
    pub error: Option<String>,
    pub toast: Option<String>,
    /// Log de `stderr` da execução, quando houve alguma saída.
    pub log_path: Option<String>,
}

/// Revoga o token da API local quando a execução termina (inclusive em erro).
struct BridgeGuard {
    token: Option<String>,
}

impl Drop for BridgeGuard {
    fn drop(&mut self) {
        if let Some(token) = &self.token {
            crate::extensions::api::revoke(token);
        }
    }
}

pub struct RunnerAssets {
    pub runner: PathBuf,
    pub import_map: PathBuf,
}

pub fn managed_deno_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("BetaStart")
        .join("runtime")
        .join("deno.exe")
}

/// Procura o Deno: caminho configurado → instalação gerenciada → PATH → ~/.deno.
pub fn find_deno(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = configured {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let managed = managed_deno_path();
    if managed.exists() {
        return Some(managed);
    }

    let exe = if cfg!(windows) { "deno.exe" } else { "deno" };
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(exe);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".deno").join("bin").join(exe);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

struct VersionCache {
    path: PathBuf,
    modified: Option<SystemTime>,
    version: String,
}

static VERSION_CACHE: Mutex<Option<VersionCache>> = Mutex::new(None);
static RUN_SEQ: AtomicU64 = AtomicU64::new(0);

/// Lê a versão do Deno, reaproveitando o resultado enquanto o executável não mudar.
///
/// A tela de Extensões chama isto a cada montagem; sem cache, cada visita
/// iniciaria um processo novo (e, antes do `CREATE_NO_WINDOW`, uma janela de
/// console que fechava o launcher).
pub fn version(deno: &Path) -> Option<String> {
    let modified = std::fs::metadata(deno)
        .and_then(|metadata| metadata.modified())
        .ok();

    if let Ok(cache) = VERSION_CACHE.lock() {
        if let Some(entry) = cache.as_ref() {
            if entry.path == deno && entry.modified == modified {
                return Some(entry.version.clone());
            }
        }
    }

    let output = hidden_command(deno).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_string())?;

    if let Ok(mut cache) = VERSION_CACHE.lock() {
        *cache = Some(VersionCache {
            path: deno.to_path_buf(),
            modified,
            version: version.clone(),
        });
    }
    Some(version)
}

/// Garante que o Deno exista e funcione; se não, baixa a release oficial para a pasta gerenciada.
pub fn ensure_deno(configured: Option<&str>) -> Result<PathBuf, String> {
    if let Some(found) = find_deno(configured) {
        // Um arquivo interrompido/parcial pode existir sem ser executável.
        if version(&found).is_some() {
            return Ok(found);
        }
    }
    // Caminho configurado quebrado: tenta as outras origens antes de baixar.
    if configured.is_some() {
        if let Some(fallback) = find_deno(None) {
            if version(&fallback).is_some() {
                return Ok(fallback);
            }
        }
    }
    let target = managed_deno_path();
    download_deno(&target)?;
    if version(&target).is_none() {
        return Err(format!(
            "o Deno foi baixado, mas não executou corretamente ({})",
            target.display()
        ));
    }
    Ok(target)
}

fn download_deno(target: &Path) -> Result<(), String> {
    let url =
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .timeout(Duration::from_secs(300))
        .build();
    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("download do Deno falhou: {e}"))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut exe = archive
        .by_name("deno.exe")
        .map_err(|e| format!("deno.exe não encontrado no pacote: {e}"))?;

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Escreve num arquivo temporário e só troca no final: um download
    // interrompido não deixa um deno.exe quebrado na pasta gerenciada.
    let partial = target.with_extension("download");
    let _ = std::fs::remove_file(&partial);
    let mut out = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    if let Err(error) = std::io::copy(&mut exe, &mut out) {
        let _ = std::fs::remove_file(&partial);
        return Err(error.to_string());
    }
    drop(out);

    if target.exists() {
        std::fs::remove_file(target).map_err(|e| {
            let _ = std::fs::remove_file(&partial);
            format!("não foi possível substituir {}: {e}", target.display())
        })?;
    }
    std::fs::rename(&partial, target).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        e.to_string()
    })?;
    Ok(())
}

/// Remove payloads/resultados antigos deixados por execuções interrompidas.
pub fn cleanup_temp_files(max_age: Duration) {
    let dir = crate::core::paths::ext_temp_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("payload-") && !name.starts_with("result-") {
            continue;
        }
        let path = entry.path();
        let expired = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .map(|age| age > max_age)
            .unwrap_or(false);
        if expired {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Mantém apenas os logs mais recentes.
pub fn cleanup_logs(max_files: usize) {
    let dir = crate::core::paths::extension_logs_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
                return None;
            }
            let modified = std::fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()?;
            Some((modified, path))
        })
        .collect();
    files.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in files.into_iter().skip(max_files) {
        let _ = std::fs::remove_file(path);
    }
}

/// Escreve o runner, o SDK e o import map na pasta de runtime.
pub fn write_assets(runtime_dir: &Path) -> Result<RunnerAssets, String> {
    std::fs::create_dir_all(runtime_dir).map_err(|e| e.to_string())?;

    let sdk = runtime_dir.join("sdk.js");
    let runner = runtime_dir.join("runner.js");
    let import_map = runtime_dir.join("import_map.json");

    std::fs::write(&sdk, SDK_JS).map_err(|e| e.to_string())?;
    std::fs::write(&runner, RUNNER_JS).map_err(|e| e.to_string())?;

    let map = format!(
        "{{\n  \"imports\": {{\n    \"@betastart/sdk\": \"{}\"\n  }}\n}}\n",
        file_url(&sdk)
    );
    std::fs::write(&import_map, map).map_err(|e| e.to_string())?;

    Ok(RunnerAssets { runner, import_map })
}

fn file_url(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        format!("file:///{normalized}")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_command(
    deno: &Path,
    runtime_dir: &Path,
    install_path: &Path,
    manifest: &ExtensionManifest,
    command_id: &str,
    args: serde_json::Value,
    granted: &[String],
    workspace: Option<&Path>,
) -> Result<RunOutcome, String> {
    let assets = write_assets(runtime_dir)?;
    let entry = crate::extensions::entry_path(install_path, manifest)?;

    let temp = crate::core::paths::ext_temp_dir();
    std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    // Evita colisão quando duas execuções começam no mesmo milissegundo.
    let sequence = RUN_SEQ.fetch_add(1, Ordering::Relaxed);
    let run_id = format!("{stamp}-{}-{sequence}", std::process::id());
    let payload_path = temp.join(format!("payload-{run_id}.json"));
    let result_path = temp.join(format!("result-{run_id}.json"));

    let log_dir = crate::core::paths::extension_logs_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    let safe_id: String = manifest
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let log_path = log_dir.join(format!("{safe_id}-{run_id}.log"));
    let log_file = std::fs::File::create(&log_path).ok();

    // O token é efêmero e cobre só esta execução; o `BridgeGuard` revoga no fim.
    let bridge = crate::extensions::api::bridge(&manifest.id, granted);
    let _bridge_guard = bridge.as_ref().map(|bridge| BridgeGuard {
        token: Some(bridge.token.clone()),
    });

    let payload = serde_json::json!({
        "entryPath": entry.to_string_lossy(),
        "commandId": command_id,
        "args": args,
        "resultPath": result_path.to_string_lossy(),
        "apiUrl": bridge.as_ref().map(|bridge| bridge.url.clone()),
        "apiToken": bridge.as_ref().map(|bridge| bridge.token.clone()),
    });
    let payload_bytes = serde_json::to_vec(&payload).map_err(|e| {
        let _ = std::fs::remove_file(&log_path);
        e.to_string()
    })?;
    if let Err(error) = std::fs::write(&payload_path, payload_bytes) {
        let _ = std::fs::remove_file(&log_path);
        return Err(error.to_string());
    }

    let mut read_paths = vec![
        runtime_dir.to_path_buf(),
        install_path.to_path_buf(),
        temp.clone(),
    ];
    let has = |permission: &str| granted.iter().any(|p| p == permission);

    if has("fs:read") {
        if let Some(workspace) = workspace {
            read_paths.push(workspace.to_path_buf());
        }
    }

    // O Deno separa múltiplos paths de permissão por vírgula.
    let read_arg = read_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut write_paths = vec![temp.clone()];
    if has("fs:write") {
        if let Some(workspace) = workspace {
            write_paths.push(workspace.to_path_buf());
        }
    }
    let write_arg = write_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut command = hidden_command(deno);
    command
        .arg("run")
        .arg("--quiet")
        .arg("--no-prompt")
        .arg("--no-config")
        .arg(format!(
            "--import-map={}",
            assets.import_map.to_string_lossy()
        ))
        .arg(format!("--allow-read={read_arg}"))
        .arg(format!("--allow-write={write_arg}"));

    if has("network") {
        command.arg("--allow-net");
    } else if let Some(bridge) = &bridge {
        // Sem `network`, a extensão pode falar apenas com a API local do app.
        command.arg(format!("--allow-net=127.0.0.1:{}", bridge.port));
    }
    if has("shell") {
        command.arg("--allow-run");
    }

    command
        .arg(&assets.runner)
        .arg(&payload_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(match log_file {
            Some(file) => Stdio::from(file),
            None => Stdio::null(),
        });

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            drop(command);
            let _ = std::fs::remove_file(&payload_path);
            let _ = std::fs::remove_file(&log_path);
            return Err(format!("falha ao executar Deno: {error}"));
        }
    };

    let status = child
        .wait_timeout(Duration::from_secs(20))
        .map_err(|e| e.to_string())?;

    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&payload_path);
        let _ = std::fs::remove_file(&result_path);
        let (_, log_hint) = collect_log(&log_path);
        return Err(format!("a extensão excedeu o tempo limite (20s){log_hint}"));
    };

    let _ = std::fs::remove_file(&payload_path);
    let (log_path_value, log_hint) = collect_log(&log_path);
    let raw = match std::fs::read_to_string(&result_path) {
        Ok(raw) => raw,
        Err(error) if !status.success() => {
            return Err(format!(
                "a extensão terminou com {status} sem retornar resultado: {error}{log_hint}"
            ));
        }
        Err(error) => {
            return Err(format!(
                "a extensão não retornou resultado: {error}{log_hint}"
            ));
        }
    };
    let _ = std::fs::remove_file(&result_path);

    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("resultado inválido: {e}{log_hint}"))?;

    Ok(RunOutcome {
        ok: value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        result: value
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        error: value
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        toast: value
            .get("toast")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        log_path: log_path_value,
    })
}

/// Retorna o caminho do log (se não estiver vazio) e um trecho para a mensagem de erro.
fn collect_log(path: &Path) -> (Option<String>, String) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (None, String::new());
    };
    if text.trim().is_empty() {
        let _ = std::fs::remove_file(path);
        return (None, String::new());
    }
    let tail: String = text.chars().rev().take(1600).collect::<String>();
    let tail: String = tail.chars().rev().collect();
    (
        Some(path.to_string_lossy().to_string()),
        format!("\nLog:\n{}", tail.trim()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::read_manifest;

    /// Roda a extensão de exemplo de verdade (pula se o Deno não estiver instalado).
    #[test]
    fn runs_example_extension_if_deno_available() {
        let deno = match find_deno(None) {
            Some(path) => path,
            None => {
                eprintln!("Deno não encontrado; teste ignorado.");
                return;
            }
        };

        let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("examples")
            .join("hello-extension");
        let manifest = read_manifest(&example).expect("manifest da extensão de exemplo");
        let runtime = std::env::temp_dir().join("betastart-ext-runtime-test");
        let granted = vec!["clipboard:write".to_string()];

        let outcome = run_command(
            &deno,
            &runtime,
            &example,
            &manifest,
            "hello",
            serde_json::json!({}),
            &granted,
            None,
        )
        .expect("execução da extensão");

        assert!(outcome.ok, "extensão falhou: {:?}", outcome.error);
        let object = outcome
            .result
            .as_object()
            .expect("resultado deve ser um objeto");
        assert_eq!(
            object.get("type").and_then(|value| value.as_str()),
            Some("message")
        );
    }

    /// Instala, registra no SQLite, aprova permissões e executa — caminho completo.
    #[test]
    fn installs_registers_and_runs_extension() {
        let deno = match find_deno(None) {
            Some(path) => path,
            None => {
                eprintln!("Deno não encontrado; teste ignorado.");
                return;
            }
        };

        let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("examples")
            .join("hello-extension");

        let root = std::env::temp_dir().join("betastart-ext-root-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("criar root");

        let manifest = crate::extensions::install_from_dir(&root, &example).expect("instalar");
        let installed = root.join(crate::extensions::folder_name(&manifest.id));

        let db_path = std::env::temp_dir().join(format!(
            "betastart-ext-db-test-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let conn = crate::db::open(&db_path).expect("abrir db");

        crate::db::extensions::upsert(
            &conn,
            &manifest,
            "local",
            None,
            &installed.to_string_lossy(),
        )
        .expect("registrar extensão");

        let pending = crate::db::extensions::get_row(&conn, &manifest.id)
            .expect("get")
            .expect("row");
        assert!(pending.pending, "deve começar pendente de permissões");

        crate::db::extensions::set_permissions(&conn, &manifest.id, &manifest.permissions)
            .expect("aprovar permissões");

        let row = crate::db::extensions::get_row(&conn, &manifest.id)
            .expect("get")
            .expect("row");
        assert!(!row.pending, "permissões aprovadas");

        let runtime = std::env::temp_dir().join("betastart-ext-runtime-test");
        let outcome = run_command(
            &deno,
            &runtime,
            std::path::Path::new(&row.install_path),
            &row.manifest,
            "hello",
            serde_json::json!({}),
            &row.granted,
            None,
        )
        .expect("executar");

        assert!(outcome.ok, "execução falhou: {:?}", outcome.error);
        assert_eq!(
            outcome.result.get("type").and_then(|value| value.as_str()),
            Some("message")
        );

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&db_path);
    }

    /// Executa a extensão de exemplo que conversa com a API local do host.
    #[test]
    fn runs_extension_with_host_sdk() {
        let deno = match find_deno(None) {
            Some(path) => path,
            None => {
                eprintln!("Deno não encontrado; teste ignorado.");
                return;
            }
        };

        let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("examples")
            .join("brain-extension");

        let root =
            std::env::temp_dir().join(format!("betastart-ext-sdk-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("root");

        let manifest = crate::extensions::install_from_dir(&root.join("extensions"), &example)
            .expect("instalar exemplo");
        let installed = root
            .join("extensions")
            .join(crate::extensions::folder_name(&manifest.id));

        let db_path = root.join("data.db");
        let conn = crate::db::open(&db_path).expect("db");
        crate::db::extensions::upsert(
            &conn,
            &manifest,
            "local",
            None,
            &installed.to_string_lossy(),
        )
        .expect("registrar");
        crate::db::extensions::set_permissions(&conn, &manifest.id, &manifest.permissions)
            .expect("aprovar");
        let row = crate::db::extensions::get_row(&conn, &manifest.id)
            .expect("get")
            .expect("row");
        drop(conn);

        let mut config = crate::core::config::Config::default();
        config.workspace.path = root.join("workspace").to_string_lossy().to_string();
        config.save(&root.join("config.json")).expect("config");

        let _guard = crate::extensions::api::test_lock();
        crate::extensions::api::start(root.clone()).expect("api server");

        let runtime = std::env::temp_dir().join("betastart-ext-runtime-sdk-test");
        let workspace = root.join("workspace");
        let outcome = run_command(
            &deno,
            &runtime,
            std::path::Path::new(&row.install_path),
            &row.manifest,
            "list-projects",
            serde_json::json!({}),
            &row.granted,
            Some(&workspace),
        )
        .expect("executar SDK");

        assert!(
            outcome.ok,
            "extensão falhou: {:?} (log {:?})",
            outcome.error, outcome.log_path
        );
        assert_eq!(
            outcome.result.get("type").and_then(|value| value.as_str()),
            Some("message")
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}

const SDK_JS: &str = r#"// @betastart/sdk — API exposta às extensões.
const registry = (globalThis.__betastartRegistry ??= []);

export function registerCommand(options) {
  if (!options || typeof options !== "object") {
    throw new Error("registerCommand: opções inválidas");
  }
  if (typeof options.id !== "string" || !options.id) {
    throw new Error("registerCommand: 'id' é obrigatório");
  }
  if (typeof options.run !== "function") {
    throw new Error("registerCommand: 'run' é obrigatório");
  }
  registry.push(options);
}

const providers = (globalThis.__betastartSearchProviders ??= []);
export function registerSearchProvider(options) {
  if (!options || typeof options.search !== "function") {
    throw new Error("registerSearchProvider: 'search' é obrigatório");
  }
  providers.push(options);
}

const api = {
  url: globalThis.__betastartApiUrl ?? null,
  token: globalThis.__betastartApiToken ?? null,
};

async function request(path, body) {
  if (!api.url || !api.token) {
    throw new Error("API do host indisponível nesta execução");
  }
  const response = await fetch(`${api.url}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token: api.token, ...body }),
  });
  let payload = null;
  try {
    payload = await response.json();
  } catch (_) {
    // resposta sem JSON
  }
  if (!response.ok || payload?.ok === false) {
    throw new Error(payload?.error ?? `falha na API do host (${response.status})`);
  }
  return payload;
}

async function callTool(name, args = {}) {
  const payload = await request("/tool", { name, args });
  return payload.result;
}

export const memory = {
  list: (project) =>
    callTool("memory.list", project ? { project } : {}),
  read: (path, project) =>
    callTool("memory.read", { path, ...(project ? { project } : {}) }),
  write: (path, content, project) =>
    callTool("memory.write", {
      path,
      content,
      ...(project ? { project } : {}),
    }),
  search: (query, options = {}) =>
    callTool("memory.search", { query, ...options }),
};

export const projects = {
  list: () => callTool("project.list", {}),
  create: (slug, title) => callTool("project.create", { slug, title }),
  link: (root, title) => callTool("project.link", { root, title }),
  unlink: (slug) => callTool("project.unlink", { slug }),
  brief: (project) => callTool("project.brief", { project }),
};

export const graph = {
  get: (project) =>
    callTool("graph.get", project ? { project } : {}),
};

export const tasks = {
  list: (options = {}) => callTool("tasks.list", options),
  create: (input) => callTool("tasks.create", input),
  createMany: (tasks) => callTool("tasks.create_many", { tasks }),
  update: (id, patch) => callTool("tasks.update", { id, ...patch }),
  delete: (id) => callTool("tasks.delete", { id }),
};

export const windows = {
  list: () => callTool("windows.list", {}),
  moveResize: (input) => callTool("windows.move_resize", input),
  applyLayout: (id) => callTool("windows.apply_layout", { id }),
  saveLayout: (name, hotkey) =>
    callTool("windows.save_layout", { name, hotkey }),
};

export const search = {
  query: (query, limit = 10) => callTool("search.query", { query, limit }),
};

export const host = { call: callTool };

export const ui = {
  close() {},
  toast(message) {
    globalThis.__betastartToast = String(message);
  },
};

export const clipboard = {
  write(text) {
    return { type: "copy", text: String(text) };
  },
  async read() {
    const payload = await request("/clipboard/read", {});
    return payload.text ?? "";
  },
};

export const args = (globalThis.__betastartArgs ??= {});
"#;

const RUNNER_JS: &str = r#"// Runner isolado: carrega a extensão e executa um comando.
const payloadPath = Deno.args[0];

function fail(error, resultPath) {
  try {
    if (resultPath) {
      Deno.writeTextFileSync(
        resultPath,
        JSON.stringify({ ok: false, error: String(error) }),
      );
    }
  } catch (_) {
    // sem resultado; o host reporta timeout/erro genérico
  }
  Deno.exit(0);
}

let payload;
try {
  payload = JSON.parse(Deno.readTextFileSync(payloadPath));
} catch (error) {
  fail(`payload inválido: ${error}`);
}

const resultPath = payload.resultPath;
globalThis.__betastartArgs = payload.args ?? {};
globalThis.__betastartApiUrl = payload.apiUrl ?? null;
globalThis.__betastartApiToken = payload.apiToken ?? null;

try {
  const normalized = String(payload.entryPath).replace(/\\/g, "/");
  const entryUrl = normalized.startsWith("/")
    ? `file://${normalized}`
    : `file:///${normalized}`;
  await import(entryUrl);
} catch (error) {
  fail(`falha ao carregar a extensão: ${error?.stack ?? error}`, resultPath);
}

const registry = globalThis.__betastartRegistry ?? [];
const command = registry.find((item) => item.id === payload.commandId);
if (!command) {
  fail(`comando '${payload.commandId}' não registrado`, resultPath);
}

try {
  const result = await command.run({
    args: payload.args ?? {},
    ui: {
      close() {},
      toast(message) {
        globalThis.__betastartToast = String(message);
      },
    },
  });
  Deno.writeTextFileSync(
    resultPath,
    JSON.stringify({
      ok: true,
      result: result ?? null,
      toast: globalThis.__betastartToast ?? null,
    }),
  );
} catch (error) {
  fail(`erro no comando '${payload.commandId}': ${error?.stack ?? error}`, resultPath);
}

Deno.exit(0);
"#;
