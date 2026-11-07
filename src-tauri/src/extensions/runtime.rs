//! Runtime de extensões baseado no Deno como processo sidecar.
//!
//! Cada execução sobe um processo Deno isolado, com permissões derivadas do
//! que o usuário aprovou. O runner escreve o resultado num arquivo JSON, então
//! não dependemos de stdout (evita poluição de logs da extensão).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::extensions::ExtensionManifest;

pub struct RunOutcome {
    pub ok: bool,
    pub result: serde_json::Value,
    pub error: Option<String>,
    pub toast: Option<String>,
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

pub fn version(deno: &Path) -> Option<String> {
    let output = Command::new(deno).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next().map(|line| line.trim().to_string())
}

/// Garante que o Deno exista; se não, baixa a release oficial para a pasta gerenciada.
pub fn ensure_deno(configured: Option<&str>) -> Result<PathBuf, String> {
    if let Some(found) = find_deno(configured) {
        return Ok(found);
    }
    let target = managed_deno_path();
    download_deno(&target)?;
    Ok(target)
}

fn download_deno(target: &Path) -> Result<(), String> {
    let url =
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";
    let response = ureq::get(url)
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
    let mut out = std::fs::File::create(target).map_err(|e| e.to_string())?;
    std::io::copy(&mut exe, &mut out).map_err(|e| e.to_string())?;
    Ok(())
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
    let payload_path = temp.join(format!("payload-{stamp}.json"));
    let result_path = temp.join(format!("result-{stamp}.json"));

    let payload = serde_json::json!({
        "entryPath": entry.to_string_lossy(),
        "commandId": command_id,
        "args": args,
        "resultPath": result_path.to_string_lossy(),
    });
    std::fs::write(
        &payload_path,
        serde_json::to_vec(&payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

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

    let mut command = Command::new(deno);
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
    }
    if has("shell") {
        command.arg("--allow-run");
    }

    command
        .arg(&assets.runner)
        .arg(&payload_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command
        .spawn()
        .map_err(|e| format!("falha ao executar Deno: {e}"))?;

    let status = child
        .wait_timeout(Duration::from_secs(20))
        .map_err(|e| e.to_string())?;

    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&payload_path);
        let _ = std::fs::remove_file(&result_path);
        return Err("a extensão excedeu o tempo limite (20s)".into());
    }

    let _ = std::fs::remove_file(&payload_path);
    let raw = std::fs::read_to_string(&result_path)
        .map_err(|e| format!("a extensão não retornou resultado: {e}"))?;
    let _ = std::fs::remove_file(&result_path);

    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("resultado inválido: {e}"))?;

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
    })
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
