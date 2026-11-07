# Beta Start — Architecture (ARCH.md)

> Documento vivo de arquitetura. Toda decisão estrutural do projeto vive aqui.
> Atualize este arquivo sempre que a arquitetura mudar.

- **Nome:** Beta Start (identificador interno: `betastart`)
- **Licença:** MIT
- **Plataforma inicial:** Windows 10/11 · roadmap: Linux → macOS
- **Modelo:** launcher/centro de produtividade estilo Raycast, gratuito e open-source

---

## 1. Visão geral

O Beta Start é um aplicativo de produtividade residente (fica na bandeja o dia todo) que
concentra, em uma janela flutuante central, o acesso a aplicativos, arquivos, comandos de
extensões, gerenciamento de janelas, e um "segundo cérebro" (memória + tasks) navegável e
exposto a IAs via MCP.

**Pilares funcionais:**

1. **Launcher** — substituto eficiente do Menu Iniciar (janela flutuante, arrastável,
   abre por hotkey ou pela tecla Win, fecha ao clicar fora e vai para a system tray).
2. **Gerenciador de janelas** — hotkeys para posicionar janelas (tamanhos/posições) e
   regras automáticas por aplicativo.
3. **Extensões** — ecossistema da comunidade em TypeScript/JavaScript com instalação simples.
4. **Porta MCP** — servidor MCP (stdio + HTTP/SSE) para IAs lerem/escreverem no app.
5. **Ambiente de memória + tasks** — arquivos reais em disco, organizados por projetos,
   com busca híbrida, navegação interativa e visualizador de grafo force-directed.

**Princípios de design:**

- **Memória = arquivos reais.** Nada fica preso em banco proprietário; o usuário pode abrir
  o workspace em qualquer editor e versionar com Git.
- **Extensões isoladas.** Uma extensão com bug ou maliciosa não derruba o app.
- **Baixo footprint.** O app fica residente; binário pequeno (Tauri) e uso de RAM controlado.
- **Segurança por escopo.** Tanto extensões quanto clientes MCP declaram/aprovam o que acessam.
- **Transparência e portabilidade.** Dados do usuário separados de cache/estado do app.

---

## 2. Stack tecnológica

| Camada | Tecnologia | Notas |
|---|---|---|
| Backend / núcleo | **Rust** (Tauri 2) | Acesso a Win32, hook de teclado, janelas, MCP |
| Frontend (UI) | **React + TypeScript** | Janela flutuante, settings, memória/grafo, loja |
| Extensões | **TypeScript/JavaScript** rodando em **Deno como processo sidecar** | Processo isolado, permissões por flag |
| Persistência auxiliar | **SQLite** (via `rusqlite`) | Índices, clipboard, tasks, tokens MCP |
| Configuração | **JSON** (`config.json`) | Preferências, hotkeys, layouts legíveis |
| Distribuição | **Instalador (NSIS/WiX/MSI) + Tauri updater** | Auto-update via GitHub Releases |

**Por que Tauri 2:** binário pequeno (~10–15 MB), baixo consumo de RAM (crítico para um app
residente), backend Rust com ótimo acesso a APIs de sistema, e frontend web reaproveitável.
**Por que Deno (sidecar):** TS nativo, ESM e flags de permissão por extensão. Cada comando roda
em um processo Deno separado, então um crash ou abuso de uma extensão não derruba o app. O
embedding via `deno_core` (V8 in-process) continua como opção futura caso se queira remover a
dependência de um runtime externo.

---

## 3. Estrutura de diretórios em disco

### 3.1 Diretórios de dados (estado do app)

| Caminho (Windows) | Conteúdo |
|---|---|
| `%APPDATA%\BetaStart\` | `config.json`, `data.db` (SQLite), `mcp.token`, logs |
| `%LOCALAPPDATA%\BetaStart\` | cache de extensões, cache de ícones, cache do grafo |
| `%APPDATA%\BetaStart\extensions\` | extensões instaladas (uma pasta por extensão) |
| `~/.betastart\` (opcional, home) | override de config para dev/portable |

### 3.2 Workspace de memória (dados do usuário)

Pasta padrão `~/Documents/BetaStart` (configurável). É a **fonte de verdade** da memória/tasks.

```
workspace/
├── .betastart/
│   └── project.json          # manifesto do workspace (versão, projetos habilitados)
├── global/                   # "memória principal" — notas/tasks sem projeto
│   ├── inbox/                # notas soltas
│   ├── tasks/                # tasks gerais
│   └── README.md
├── projects/
│   └── <slug-do-projeto>/
│       ├── project.md        # manifesto do projeto (frontmatter)
│       ├── notes/
│       ├── tasks/
│       └── assets/           # arquivos anexados
└── archive/                  # itens arquivados (opcional)
```

Um arquivo `.md` é interpretado pelo tipo declarado no frontmatter: `note`, `task` ou `project`.
Qualquer outro arquivo (`*.md` sem frontmatter, imagens, PDFs etc.) é tratado como **file** (anexo).

---

## 4. Módulos e ordem de construção

A ordem abaixo é a **ordem de dependência** (o que deve existir antes para construir o que vem
depois) e também a ordem sugerida de implementação.

| # | Módulo | Responsabilidade | Depende de |
|---|---|---|---|
| 1 | `core` | Ciclo de vida, bandeja (tray), single-instance, config, logging, auto-início | — |
| 2 | `platform` | Abstração de SO: hotkey global, hook de teclado, enumeração/movimentação de janelas, apps instalados, monitor | `core` |
| 3 | `db` | Abertura/migração do SQLite, FTS5, DAOs | `core` |
| 4 | `ui-shell` | Shell React: janela flutuante, roteador de telas, tema, IPC bridge | `core` |
| 5 | `indexer` | Índice de apps e arquivos (crawler + FTS) | `platform`, `db` |
| 6 | `launcher` | Command palette, pipeline de busca (apps/arquivos/comandos/calculadora/clipboard) | `indexer`, `ui-shell` |
| 7 | `clipboard` | Listener de clipboard + histórico | `platform`, `db` |
| 8 | `extensions` | Runtime Deno, manifesto, permissões, loja/instalação, ponte de comandos | `db`, `ui-shell` |
| 9 | `window-manager` | Layouts por hotkey, regras por app, salvar/restaurar | `platform`, `db` |
| 10 | `memory` | Workspace de arquivos, projetos, frontmatter, busca híbrida | `db` |
| 11 | `tasks` | Índice de tasks, lista/kanban, lembretes | `memory`, `db` |
| 12 | `graph` | Modelo de grafo + render force-directed | `memory`, `tasks` |
| 13 | `mcp` | Servidor MCP (stdio + SSE/HTTP), auth, tools | `memory`, `tasks`, `window-manager`, `extensions`, `launcher` |
| 14 | `updater` | Auto-update via GitHub Releases | `core` |
| 15 | `packaging` | Instalador, assets, signing | todos |

**Milestone 1 (MVP):** módulos 1–8 (launcher + extensões + clipboard). Depois 9, 10–12, 13.

---

## 5. Modelo de dados

### 5.1 Workspace / memória (arquivos)

Todo nó é um arquivo. A interpretação vem do **frontmatter** (YAML no topo do `.md`).

**Frontmatter comum (todas as notas/arquivos):**

```yaml
---
type: note | task | project        # se ausente e .md => "note"
id: "<uuid>"                       # opcional; gerado se ausente
title: "Título legível"
tags: [ "tag1", "tag2" ]
project: "<slug>"                  # derivado da localização se ausente
links: [ "[[path/para/outro]]" ]   # wiki-links para o grafo
created: "2026-11-07T12:00:00Z"
updated: "2026-11-07T12:00:00Z"
---
```

**Frontmatter de task (estende o comum):**

```yaml
---
type: task
status: todo | in_progress | done | blocked | cancelled   # default "todo"
priority: high | medium | low                              # default "medium"
due: "2026-11-07T18:00:00Z"       # opcional
parent: "<id-da-task-pai>"         # subtasks: id da task pai
remind: "2026-11-07T17:30:00Z"     # lembrete (opcional)
tags: [ ... ]
---
```

**Frontmatter de projeto (`project.md`):**

```yaml
---
type: project
slug: "meu-projeto"
title: "Meu Projeto"
description: "..."
color: "#8b5cf6"                  # para o grafo/UI
archived: false
---
```

### 5.2 SQLite (`data.db`)

#### Tabelas de índice de busca (launcher)

```sql
CREATE TABLE IF NOT EXISTS apps_index (
  id            INTEGER PRIMARY KEY,
  name          TEXT    NOT NULL,
  exe_path      TEXT    NOT NULL UNIQUE,
  icon_path     TEXT,
  start_menu    TEXT,                      -- caminho do atalho no Start Menu
  keywords      TEXT,                      -- palavras-chave extras (CSV)
  launch_count  INTEGER NOT NULL DEFAULT 0,
  last_launched INTEGER,
  indexed_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS files_index (
  id     INTEGER PRIMARY KEY,
  path   TEXT    NOT NULL UNIQUE,
  name   TEXT    NOT NULL,
  ext    TEXT,
  size   INTEGER,
  mtime  INTEGER,
  is_dir INTEGER NOT NULL DEFAULT 0
);
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(name, path, content='files_index', content_rowid='id');
```

#### Histórico de clipboard

```sql
CREATE TABLE IF NOT EXISTS clipboard_history (
  id         INTEGER PRIMARY KEY,
  kind       TEXT    NOT NULL,             -- 'text' | 'image' | 'files' | 'html'
  text       TEXT,
  html       TEXT,
  image_path TEXT,
  source_app TEXT,
  copied_at  INTEGER NOT NULL,
  pinned     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_clipboard_copied ON clipboard_history(copied_at DESC);
```

#### Extensões

```sql
CREATE TABLE IF NOT EXISTS extension_registry (
  id               INTEGER PRIMARY KEY,
  ext_id           TEXT    NOT NULL UNIQUE,  -- "namespace/name"
  name             TEXT    NOT NULL,
  version          TEXT,
  install_source   TEXT    NOT NULL,         -- 'store' | 'git' | 'local'
  source_url       TEXT,
  install_path     TEXT    NOT NULL,
  manifest_json    TEXT    NOT NULL,
  enabled          INTEGER NOT NULL DEFAULT 1,
  permissions_json TEXT,                     -- permissões concedidas pelo usuário
  updated_at       INTEGER
);

CREATE TABLE IF NOT EXISTS extension_commands (
  id         INTEGER PRIMARY KEY,
  ext_id     TEXT NOT NULL,
  command_id TEXT NOT NULL,
  title      TEXT,
  subtitle   TEXT,
  keywords   TEXT,
  handler    TEXT,
  icon       TEXT,
  UNIQUE(ext_id, command_id)
);
```

#### Clientes/tokens MCP

```sql
CREATE TABLE IF NOT EXISTS mcp_clients (
  id          INTEGER PRIMARY KEY,
  client_name TEXT    NOT NULL,
  token_hash  TEXT    NOT NULL UNIQUE,      -- hash do token (nunca em claro)
  scopes_json TEXT    NOT NULL,             -- ["memory","tasks","windows","extensions","search","settings"]
  created_at  INTEGER NOT NULL,
  last_seen   INTEGER,
  revoked     INTEGER NOT NULL DEFAULT 0
);
```

#### Índices do ambiente de memória/tasks (denormalizados dos arquivos)

```sql
CREATE TABLE IF NOT EXISTS memory_index (
  id         INTEGER PRIMARY KEY,
  file_path  TEXT NOT NULL UNIQUE,
  kind       TEXT NOT NULL,                 -- 'note' | 'task' | 'project' | 'file'
  title      TEXT,
  project    TEXT,                          -- slug ou NULL (memória principal)
  tags       TEXT,                          -- JSON array
  updated_at INTEGER
);
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(title, content, tags, content='memory_index', content_rowid='id');

CREATE TABLE IF NOT EXISTS tasks_index (
  id         TEXT PRIMARY KEY,              -- id do frontmatter (uuid)
  file_path  TEXT NOT NULL,
  title      TEXT NOT NULL,
  status     TEXT NOT NULL DEFAULT 'todo',
  priority   TEXT NOT NULL DEFAULT 'medium',
  due        INTEGER,
  tags       TEXT,                          -- JSON array
  project    TEXT,
  parent_id  TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks_index(status, project);
```

#### Gerenciador de janelas

```sql
CREATE TABLE IF NOT EXISTS window_layouts (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  hotkey        TEXT,                        -- ex.: "Win+Shift+F7"
  monitors_json TEXT NOT NULL,               -- geometria dos monitores no momento do save
  windows_json  TEXT NOT NULL,               -- [{process,title,class,monitor,x,y,w,h}, ...]
  created_at    INTEGER,
  updated_at    INTEGER
);

CREATE TABLE IF NOT EXISTS window_rules (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  app_match   TEXT NOT NULL,                 -- regex sobre process/title/class
  match_field TEXT NOT NULL DEFAULT 'process',  -- 'process' | 'title' | 'class'
  monitor     INTEGER NOT NULL DEFAULT 0,
  x  REAL, y  REAL, w  REAL, h  REAL,        -- percentual (0.0..1.0)
  x_px INTEGER, y_px INTEGER, w_px INTEGER, h_px INTEGER,  -- pixels (alternativa)
  enabled     INTEGER NOT NULL DEFAULT 1
);
```

#### Lembretes e cache de grafo

```sql
CREATE TABLE IF NOT EXISTS reminders (
  id          INTEGER PRIMARY KEY,
  task_id     TEXT NOT NULL,
  due_at      INTEGER NOT NULL,
  notified_at INTEGER,
  enabled     INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS graph_cache (
  workspace_hash TEXT PRIMARY KEY,
  nodes_json     TEXT,
  edges_json     TEXT,
  updated_at     INTEGER
);
```

### 5.3 `config.json`

```jsonc
{
  "version": 1,
  "onboardingDone": false,
  "startMenu": {
    "mode": "ask",             // 'ask' | 'replace' | 'win_x'
    // replace = tecla Win abre o Beta Start; win_x = Win+X abre o Beta Start
    "fallback": "Win+Shift"    // atalho que sempre abre o Menu Iniciar real
  },
  "hotkeys": {
    "launcher": "Win",
    "windowLayouts": { "exemplo": "Win+Shift+F7" }
  },
  "workspace": {
    "path": "~/Documents/BetaStart",
    "activeProject": null
  },
  "search": {
    "includeApps": true,
    "includeFiles": true,
    "indexDirs": ["~/Documents", "~/Desktop"],
    "excludePatterns": ["node_modules", ".git"],
    "maxResults": 8
  },
  "tray": { "showOnClose": true, "startOnBoot": false },
  "mcp": {
    "enabled": true,
    "stdio": { "enabled": true },
    "http": { "enabled": true, "host": "127.0.0.1", "port": 19000 }
  },
  "appearance": { "theme": "auto" }
}
```

---

## 6. Assinaturas de funções

### 6.1 Comandos Tauri (backend Rust — `#[tauri::command]`)

Todas as funções abaixo ficam expostas ao frontend via IPC Tauri. Convenção: `snake_case`,
parâmetros serializados em JSON, retorno `Result<T, AppError>`.

```rust
// ---- core ----
fn app_version() -> String;
fn get_config() -> Config;
fn set_config(patch: ConfigPatch) -> Result<(), AppError>;
fn set_onboarding_done() -> Result<(), AppError>;

// ---- platform / launcher ----
fn toggle_launcher(visible: bool) -> Result<(), AppError>;
fn set_start_menu_mode(mode: StartMenuMode) -> Result<(), AppError>;  // Replace | WinX
fn register_hotkey(action: HotkeyAction, combo: String) -> Result<(), AppError>;
fn unregister_hotkey(action: HotkeyAction) -> Result<(), AppError>;

fn list_installed_apps() -> Vec<AppEntry>;
fn list_open_windows() -> Vec<WindowInfo>;   // {handle, process, title, class, monitor, rect}

// ---- search ----
fn search(query: String, limit: usize) -> SearchResults;
   // SearchResults { apps: Vec<AppEntry>, files: Vec<FileEntry>,
   //                 commands: Vec<CommandEntry>, clipboard: Vec<ClipEntry> }
fn rebuild_index() -> Result<(), AppError>;

// ---- clipboard ----
fn list_clipboard(limit: usize, kind: Option<ClipKind>) -> Vec<ClipEntry>;
fn pin_clipboard(id: i64, pinned: bool) -> Result<(), AppError>;
fn delete_clipboard(id: i64) -> Result<(), AppError>;

// ---- window-manager ----
fn apply_layout(layout_id: i64) -> Result<(), AppError>;
fn save_current_layout(name: String, hotkey: Option<String>) -> Result<i64, AppError>;
fn list_layouts() -> Vec<WindowLayout>;
fn list_window_rules() -> Vec<WindowRule>;
fn upsert_window_rule(rule: WindowRule) -> Result<i64, AppError>;
fn delete_window_rule(id: i64) -> Result<(), AppError>;
fn move_resize_window(handle: u64, monitor: i32, x: f64, y: f64, w: f64, h: f64) -> Result<(), AppError>;

// ---- extensions ----
fn list_extensions() -> Vec<ExtensionMeta>;
fn list_extension_commands() -> Vec<CommandEntry>;
fn install_extension(source: InstallSource) -> Result<String, AppError>;  // InstallSource { kind, url | path }
fn uninstall_extension(ext_id: String) -> Result<(), AppError>;
fn enable_extension(ext_id: String, enabled: bool) -> Result<(), AppError>;
fn grant_extension_permission(ext_id: String, permissions: Vec<String>) -> Result<(), AppError>;
fn run_extension_command(ext_id: String, command_id: String, args: serde_json::Value) -> Result<serde_json::Value, AppError>;

// ---- memory ----
fn list_workspace_tree(project: Option<String>) -> Vec<FsNode>;
fn read_memory_file(path: String) -> Result<String, AppError>;
fn write_memory_file(path: String, content: String) -> Result<(), AppError>;
fn create_project(slug: String, title: String) -> Result<(), AppError>;
fn search_memory(project: Option<String>, query: String, limit: usize) -> Vec<MemoryHit>;

// ---- tasks ----
fn list_tasks(project: Option<String>, filter: TaskFilter) -> Vec<Task>;
fn create_task(input: NewTask) -> Result<Task, AppError>;
fn update_task(id: String, patch: TaskPatch) -> Result<Task, AppError>;
fn delete_task(id: String) -> Result<(), AppError>;
fn move_task(id: String, new_parent: Option<String>, new_project: Option<String>) -> Result<(), AppError>;

// ---- graph ----
fn get_graph(project: Option<String>) -> Graph;  // Graph { nodes: Vec<GraphNode>, edges: Vec<GraphEdge> }

// ---- mcp ----
fn mcp_list_clients() -> Vec<McpClient>;
fn mcp_create_client(name: String, scopes: Vec<String>) -> Result<McpClientCreated, AppError>; // retorna o token uma única vez
fn mcp_revoke_client(id: i64) -> Result<(), AppError>;
fn mcp_get_status() -> McpStatus;  // { http_enabled, port, running }
```

### 6.2 Tipos principais (Rust / frontend compartilhado)

```rust
struct AppEntry      { name: String, exe_path: String, icon_path: Option<String>, keywords: Vec<String> }
struct FileEntry     { path: String, name: String, ext: Option<String>, is_dir: bool }
struct CommandEntry  { ext_id: String, command_id: String, title: String, subtitle: Option<String>, icon: Option<String> }
struct ClipEntry     { id: i64, kind: String, text: Option<String>, image_path: Option<String>, copied_at: i64, pinned: bool }
struct WindowInfo    { handle: u64, process: String, title: String, class: Option<String>, monitor: i32, rect: Rect }
struct Rect          { x: i64, y: i64, w: i64, h: i64 }
struct WindowLayout  { id: i64, name: String, hotkey: Option<String>, monitors: Vec<Rect>, windows: Vec<WindowInfo> }
struct WindowRule    { id: i64, name: String, app_match: String, match_field: String, monitor: i32, x: Option<f64>, y: Option<f64>, w: Option<f64>, h: Option<f64> }
struct Task          { id: String, file_path: String, title: String, status: String, priority: String, due: Option<i64>, tags: Vec<String>, project: Option<String>, parent_id: Option<String> }
struct GraphNode     { id: String, kind: String, label: String, project: Option<String>, file_path: Option<String> }
struct GraphEdge     { source: String, target: String, kind: String }  // 'link' | 'parent' | 'tag' | 'project' | 'dependency'
```

### 6.3 SDK de extensões (TypeScript — `@betastart/sdk`)

API exposta às extensões dentro do runtime Deno:

```typescript
// Manifesto (extension.json) -> ver seção 7
// import { registerCommand, ui, clipboard, memory, fs, ... } from "@betastart/sdk";

interface CommandContext {
  args: unknown;
  ui: {
    list(items: ListItem[]): Promise<string | null>;      // renderiza lista, retorna item escolhido
    form(fields: FormField[]): Promise<Record<string, string> | null>;
    detail(markdown: string): Promise<void>;
    toast(message: string): void;
    close(): void;
  };
  clipboard: { write(text: string): Promise<void>; read(): Promise<string> };
  memory: {
    read(path: string): Promise<string>;
    write(path: string, content: string): Promise<void>;
    search(query: string, project?: string): Promise<MemoryHit[]>;
    activeProject(): Promise<string | null>;
  };
  tasks: {
    list(filter?: TaskFilter): Promise<Task[]>;
    create(input: NewTask): Promise<Task>;
    update(id: string, patch: TaskPatch): Promise<Task>;
  };
  windows: {
    list(): Promise<WindowInfo[]>;
    applyLayout(layoutId: number): Promise<void>;
  };
  search: { query(q: string): Promise<SearchResults> };
}

// Registrar um comando no launcher
declare function registerCommand(opts: {
  id: string;
  title: string;
  subtitle?: string;
  keywords?: string[];
  icon?: string;
  run(ctx: CommandContext): Promise<void> | void;
}): void;

// Registrar uma fonte de resultados no launcher (contribui itens dinâmicos)
declare function registerSearchProvider(opts: {
  id: string;
  search(q: string, ctx: CommandContext): Promise<ListItem[]>;
}): void;
```

---

## 7. Manifesto de extensão (`extension.json`)

Cada extensão é uma pasta com um `extension.json` na raiz.

```jsonc
{
  "schema": 1,
  "id": "autor/nome-da-extensao",        // namespace/name, único
  "name": "Nome da Extensão",
  "version": "1.0.0",
  "description": "...",
  "author": "Fulano",
  "icon": "icon.png",
  "entry": "mod.ts",                     // módulo de entrada (TS/JS)
  "commands": [
    {
      "id": "meu-comando",
      "title": "Título no launcher",
      "subtitle": "Descrição curta",
      "keywords": ["alias1"],
      "mode": "view",                    // 'view' | 'no-view' (headless)
      "icon": "icon.png"
    }
  ],
  "searchProviders": ["mod.ts#search"],  // opcional
  "permissions": [
    "fs:read",        // leitura no workspace
    "fs:write",       // escrita no workspace
    "network",        // acesso à rede
    "shell",          // executar comandos do SO
    "clipboard:read",
    "clipboard:write",
    "memory",         // acesso à memória via SDK
    "tasks",          // acesso a tasks via SDK
    "windows",        // gerenciar janelas
    "search"          // usar o índice do launcher
  ],
  "dependencies": { "npm": { "pkg": "1.0.0" } }  // resolvidas via Deno/npm
}
```

**Fluxo de instalação:** o usuário vê a lista de permissões e aprova/rejeita antes de habilitar.
Permissões negadas viram erro de runtime para a extensão (nunca bloqueiam silenciosamente).

---

## 8. Porta MCP

### 8.1 Transportes

- **stdio**: o cliente MCP (Claude Desktop, Cursor, etc.) lança `betastart mcp` como subprocesso.
  O protocolo MCP JSON-RPC roda sobre stdin/stdout.
- **HTTP/SSE**: servidor HTTP local em `127.0.0.1:<porta>` (default 19000), expondo
  `/sse` (stream) e `/messages` (POST). Útil para múltiplos clientes e futuro acesso remoto.

### 8.2 Autenticação

- Token único gerado no primeiro uso, armazenado com hash (`argon2`) em `mcp_clients.token_hash`.
- O token é mostrado **uma única vez** ao usuário e deve ser enviado como
  header `Authorization: Bearer <token>` (HTTP) ou nas mensagens de inicialização (stdio).
- Cada cliente tem **escopos** aprovados: `memory`, `tasks`, `windows`, `extensions`,
  `search`, `settings`. O usuário revoga/edita escopos pela UI.

### 8.3 Tools expostas (JSON-RPC `tools/*`)

Cada tool define `name`, `description` e `inputSchema` (JSON Schema). Assinatura conceitual:

| Tool | Escopo | Parâmetros | Retorno |
|---|---|---|---|
| `memory.read` | memory | `{ path: string }` | conteúdo do arquivo |
| `memory.write` | memory | `{ path, content }` | ok |
| `memory.list` | memory | `{ project?: string }` | árvore/lista de arquivos |
| `memory.search` | memory | `{ query, project? }` | hits (path, título, trecho) |
| `project.list` | memory | — | projetos |
| `project.create` | memory | `{ slug, title }` | projeto |
| `tasks.list` | tasks | `{ project?, status?, priority? }` | tasks |
| `tasks.create` | tasks | `{ title, project?, parent?, due?, priority?, tags? }` | task |
| `tasks.update` | tasks | `{ id, patch }` | task |
| `tasks.delete` | tasks | `{ id }` | ok |
| `windows.list` | windows | — | janelas abertas |
| `windows.move_resize` | windows | `{ handle, monitor, x, y, w, h }` | ok |
| `windows.apply_layout` | windows | `{ layout_id }` | ok |
| `windows.save_layout` | windows | `{ name }` | layout_id |
| `extensions.list` | extensions | — | extensões |
| `extensions.run` | extensions | `{ ext_id, command_id, args }` | resultado |
| `search.query` | search | `{ query }` | resultados do launcher |
| `clipboard.list` | search | `{ limit? }` | histórico |
| `settings.get` / `settings.set` | settings | `{ key?, value }` | config |

**Regra de escopo:** uma tool que exige escopo não concedido retorna erro MCP
`{"code": -32001, "message": "permission denied"}`.

---

## 9. Segurança

1. **Extensões**: processo/sandbox isolado, permissões declaradas, sem acesso ao SQLite nem a
   `config.json`; interagem apenas via SDK. Downloads de extensões são verificados por checksum.
2. **MCP**: token com hash, escopos por cliente, servidor restrito a `127.0.0.1` por padrão,
   revogação imediata pela UI.
3. **Memória**: escrita limitada ao workspace do usuário (sandbox de path), sem acesso fora
   da pasta de memória, salvo permissão explícita.
4. **Tecla Win**: fallback sempre disponível (Win+Shift) para abrir o Menu Iniciar real, evitando
   "travar" o usuário.

---

## 10. Modelo do grafo (force-directed)

- **Nós**: `project`, `note`, `task`, `tag`, `file` (anexos).
- **Arestas** (tipos):
  - `parent` — task → subtask; projeto → filho; pasta → arquivo.
  - `link` — wiki-link `[[...]]` entre notas/arquivos.
  - `tag` — item → tag.
  - `project` — item → projeto.
  - `dependency` — (futuro) task → task.
- Render com `d3-force` (ou `force-graph`) no painel lateral; nós coloridos por `kind`,
  tamanho por número de conexões, filtro por projeto/tipo. Cache pré-computado em `graph_cache`.

---

## 11. Roadmap de plataforma

| Fase | Plataforma | Notas |
|---|---|---|
| 1 (agora) | **Windows** | Win32, hook de teclado, Start Menu, window mgmt |
| 2 | Linux | X11 + Wayland (via `xdotool`/compositor nativo), sem hook Win |
| 3 | macOS | API de acessibilidade para janelas; sem tecla Win (substituir por ⌘/Option) |

A camada `platform` (módulo 2) é a única que muda entre SOs; todo o resto é portável.

---

## 12. Fora de escopo (decisões adiadas)

- Busca semântica por **embeddings/RAG** (futuro; hoje escopo por projeto + busca híbrida).
- **Virtual desktops** no gerenciador de janelas (futuro).
- Multi-linguagem de extensões além de TS/JS (futuro, se houver demanda).
- Acesso MCP remoto (hoje só localhost).
- Sincronização multi-dispositivo (futuro).

---

## 13. Decisões registradas (ADR resumido)

| # | Decisão | Motivo |
|---|---|---|
| ADR-001 | Tauri 2 (Rust + React) | footprint baixo, app residente, acesso a sistema |
| ADR-002 | Extensões TS/JS em Deno **sidecar** (processo isolado) | TS nativo, permissões por flag, isolamento forte; `deno_core` fica como opção futura |
| ADR-003 | Memória = arquivos Markdown em disco | transparência, Git, portabilidade |
| ADR-004 | Tasks = `.md` com frontmatter | unifica memória e tasks no mesmo ambiente |
| ADR-005 | SQLite p/ índices + JSON p/ config | busca rápida sem prender dados do usuário |
| ADR-006 | MCP stdio + HTTP/SSE, token + escopos | compatibilidade máxima + segurança granular |
| ADR-007 | Escopo por projeto + busca híbrida (sem embeddings) | economia de tokens sem custo de infraestrutura |
| ADR-008 | MIT | adoção máxima |
