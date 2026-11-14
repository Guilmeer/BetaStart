# Beta Start

A free, open-source (MIT) Raycast-style launcher and productivity hub.
A central floating window to launch apps, search files, run extension commands,
manage windows, and access a memory + tasks "second brain", with AI integration
through MCP.

- Full architecture: [`ARCH.md`](./ARCH.md)
- Priorities and production order: [`ROADMAP.md`](./ROADMAP.md)

## Stack

- **Tauri 2** (Rust backend + React/TypeScript frontend)
- **Deno as a sidecar process** (isolated) for TS/JS extensions
- **SQLite** for indexes/clipboard/tasks/tokens + **JSON** for configuration
- **MIT**

## Prerequisites (Windows)

- Node.js 20+ and npm
- Rust (stable) — install from <https://rustup.rs>
- Visual Studio Build Tools with "Desktop development with C++"
- For extensions: Deno (the app downloads it automatically in **Extensions** if needed)

## Development

```bash
npm install
npm run tauri dev
```

The floating window opens centered. Default shortcut to open/close: **Alt + Space**.
Closing the window (X/click outside) only hides it — the app keeps running in the
system tray. To quit completely, use **Quit** in the tray menu.

## Build

```bash
npm run tauri build
```

With `createUpdaterArtifacts` enabled, release builds require the signing keys
(see [`docs/AUTO_UPDATE.md`](./docs/AUTO_UPDATE.md)). For convenience:

```powershell
# local signed build; -Repo owner/repo also points the updater at GitHub
powershell -ExecutionPolicy Bypass -File scripts\build-signed.ps1 -Repo your-user/BetaStart
```

## Signed auto-update

The app uses `tauri-plugin-updater` with **minisign** signatures: every release is
signed with the private key, and the app only installs packages whose signature
matches the embedded public key. The **⚙ Settings → Updates** screen checks for,
downloads, and installs new versions.

The complete guide (keys, GitHub Secrets, version/tag, local signed builds, and
testing with a local server) is in [`docs/AUTO_UPDATE.md`](./docs/AUTO_UPDATE.md).

## Usage

- **Alt + Space** opens/closes the launcher.
- Type to search **apps**, **files**, **clipboard**, extension commands, and the
  **calculator** (e.g. `12*8+3`).
- **🔌 MCP**: server for AI clients (stdio + HTTP/SSE) with tokens and scopes,
  resources, prompts, and audit logs.
- **🧠 Memory & Tasks**: notes, projects, search, tasks with subtasks, list/kanban,
  and reminders.
- **🪟 Window manager**: save/apply layouts, per-app rules, and multi-monitor support.
- **📋 Clipboard**: filterable history, pin/unpin, delete, and clear unpinned items.
- **⚙ Settings**: shortcut, Windows key, theme, start with Windows, search, clipboard,
  MCP, and extensions.
- **🧩 Extensions**: install/manage extensions and the Deno runtime.
- **Quick web search**: `g`, `yt`, `gh`, `w`, `ddg`, or `maps` + term.
- **Esc** closes; closing the window sends it to the tray.

## Extensions

An extension is a folder with `extension.json` (manifest) + TS/JS code:

```
my-extension/
├── extension.json   # id, entry, commands, permissions
└── main.ts          # imports @betastart/sdk
```

A ready-to-use example is in [`examples/hello-extension`](./examples/hello-extension).

### Installing the example

1. Open **🧩 Extensions**.
2. Under **Local folder**, use `<repo>\examples\hello-extension`.
3. Click **Install**, then **Approve permissions**.
4. Type `Hello` or `date` in the launcher.

You can also install from a **Git URL** or the **Store** (an `index.json`, remote or
local — see [`examples/store/index.json`](./examples/store/index.json)).

### SDK

Besides `registerCommand` and `ui`, the SDK exposes host APIs (subject to the
permissions approved in the manifest):

```ts
import { memory, projects, tasks, windows, search, clipboard } from "@betastart/sdk";

const list = await projects.list();
const brief = await projects.brief("my-project");
const notes = await memory.search("decision", { project: "my-project" });
const task = await tasks.create({ title: "Review layout", project: "my-project" });
const windowsList = await windows.list();
const results = await search.query("report");
const text = await clipboard.read();
```

- `memory`: `list`, `read`, `write`, `search`
- `projects`: `list`, `create`, `link`, `unlink`, `brief`
- `graph`: `get`
- `tasks`: `list`, `create`, `createMany`, `update`, `delete`
- `windows`: `list`, `moveResize`, `applyLayout`, `saveLayout`
- `search`: `query`
- `clipboard`: `write`, `read`

A complete example is in [`examples/brain-extension`](./examples/brain-extension).
The host validates every call using the execution's ephemeral token and the
manifest permissions.

## Memory & Tasks

Open **🧠** in the top bar (the window grows automatically).

- The **main memory** lives in `global/`; each project lives in `projects/<slug>/`.
- Everything is **real Markdown** with YAML frontmatter — you can open/edit it
  outside the app and version it with Git.
- In the app you can:
  - create **projects**, **notes**, and **tasks**;
  - edit title, tags, and body;
  - search by **keyword** within a project (or across everything);
  - switch tasks between **List** and **Kanban**;
  - create **subtasks** (parent task);
  - set **due dates** and **reminders** (Windows notifications).
- The index is refreshed when the screen opens, when saving, and at boot. The
  **Refresh** button reindexes manually.

Task format:

```md
---
type: task
id: "uuid"
title: "My task"
status: todo            # todo | in_progress | done | blocked | cancelled
priority: high          # high | medium | low
due: "2026-12-01T10:00"
remind: "2026-12-01T09:30"
tags: [work, urgent]
parent: "parent-task-id"  # optional (subtask)
---
Body/description…
```

### Graph

In the right panel of the memory screen, switch between **Tasks** and **Graph**.
The force-directed graph (`d3-force`) shows projects, notes, tasks, and tags, with
edges for:

- **project** — project → item;
- **tag** — tag → item;
- **parent** — parent task → subtask;
- **link** — `[[Another note]]` found in the body.

In the editor, links appear as **chips**: cyan when the target exists and **red**
when it doesn't. Missing targets also appear as **red nodes** in the graph. The
graph refreshes after save/create/delete.

Click a node that has a file to open it. The project filter also filters the graph.

## Window Manager

Open **🪟** in the top bar.

### Layouts with hotkeys

1. Arrange windows manually (e.g. browser 25% on the right, Discord 75% on the left).
2. Under **Layouts**, enter a name. For the shortcut, **click the field and press
   the combination** (e.g. `Win+Shift+F7`) — the app listens and records it.
3. Click **Save current**.
4. Press the shortcut anytime to reapply the layout.

> If the combination is already in use (by the launcher, another layout, or the
> system), the field gets a **red border** and shows the reason. Just record another.

Layouts store the process and position in **exact pixels** (in addition to fractions),
so they reapply precisely on the same monitor setup and scale proportionally if the
resolution changes. Layouts saved before this version should be **re-saved once**.

### Per-app rules

Configure app (process/title/class), monitor, and region (`x/y/w/h` as fractions from
0 to 1). Rules are applied **automatically** when a new window for the app appears
(watcher ~1.2s) or manually with **Apply rules now**.

Example: `chrome` on monitor 1, `x=0.75, y=0, w=0.25, h=1` (right, 25%).
`discord` on monitor 1, `x=0, y=0, w=0.75, h=1` (left, 75%).

### Multi-monitor

Monitors are enumerated with resolution and work area; layouts and rules store the
monitor index. Maximized windows are restored before being repositioned.

### Runtime and security

- Each command runs in a **separate Deno process** (real isolation).
- Permissions are declared in the manifest and approved by the user:
  `fs:read`, `fs:write`, `network`, `shell`, `clipboard:read`, `clipboard:write`,
  `memory`, `tasks`, `windows`, `search`.
- Deno receives only the flags matching the approved permissions.
- Commands return safe actions: `{ type: "message" }`, `{ type: "copy" }`, or
  `{ type: "open" }` (with a host-side permission check).

## MCP

Open **🔌** in the top bar.

1. Create a client, choose the **scopes**, and click **Generate token**.
2. Copy the token (shown **only once**) and the configuration snippet.
3. Revoke clients at any time.

### stdio (Claude Desktop, Cursor, etc.)

```json
{
  "mcpServers": {
    "betastart": {
      "command": "C:\\path\\to\\betastart.exe",
      "args": ["mcp", "--token", "bst_..."]
    }
  }
}
```

### Local HTTP/SSE

- SSE: `http://127.0.0.1:19000/sse?token=<token>`
- JSON-RPC POST: `http://127.0.0.1:19000/mcp`
- Health: `GET /health`
- The server starts with the app when `mcp.http.enabled` is on.

### Tools

`memory.list`, `memory.read`, `memory.write`, `memory.search`, `project.list`,
`project.create`, `project.link`, `project.unlink`, `project.brief`, `graph.get`,
`tasks.list`, `tasks.create`, `tasks.create_many`, `tasks.update`, `tasks.delete`,
`windows.list`, `windows.move_resize`, `windows.apply_layout`,
`windows.save_layout`, `extensions.list`, `extensions.run`, `search.query`,
`clipboard.list`, `settings.get`, `settings.set`.

There are also **resources** (`resources/list` and `resources/read` expose notes/tasks
as Markdown) and ready-made **prompts** (`project-brief`, `plan-project`,
`review-tasks`, and `capture-note`). The 🔌 screen shows the **latest calls**
(client, tool, success/error, and detail) for diagnostics.

Responses are designed to use fewer tokens:

- `tasks.list` is **compact** (`id`, `title`, `status`, `priority`, `due`, tags):
  ~166 bytes/task versus ~417 for the full record — use `compact: false` if you
  need the file path.
- `tasks.create_many` creates an entire plan in one call, reindexing only once.
- `project.brief` returns a project digest (metadata, notes with **relative** paths,
  task count by status, and the first open tasks) in ~1.3 KB, without chaining
  `project.list` + `memory.list` + `tasks.list`.
- `memory.read` accepts the relative path returned by the brief, along with `project`.

Typical token-efficient flow:

```json
{ "project": "my-project" }                          // project.brief
{ "project": "my-project", "path": "MEMORY.md" }     // memory.read
{ "project": "my-project", "tasks": [ { "title": "..." } ] }  // tasks.create_many
{ "project": "my-project", "status": "todo" }         // tasks.list
```

To work on a project that already exists on disk, use `project.link` (the folder is
indexed in place, nothing is copied):

```json
{ "root": "E:\\projects\\my-project", "ignore": ["examples/", "vendor/"] }
```

The UI reflects the link in ~2s, without restarting the app.

### stdio diagnostics

If an MCP client cannot connect, the server writes a log to
`%APPDATA%\com.betastart.app\mcp-stdio.log` (start, load time, token, EOF, panics) —
the same file can be opened from **🔌 → Diagnostic log**. Set
`BETASTART_MCP_TRACE=1` to also log every received method.

How to read the log:

- ends with `stdin fechado (EOF)` → the client closed the connection;
- ends with `pronto em …` and has no EOF → the process was **killed externally**;
- ends with `erro (exit N)` → a real server failure (the reason is on that line).

Exit code `255` on Windows is usually **OpenCode killing the process** (initialize
timeout or transport error), not a Beta Start crash. In that case, `opencode reload`
(or any write to `opencode.json`) forces a respawn — OpenCode does not yet restart a
crashed server automatically
([issue #53226](https://github.com/anomalyco/opencode/issues/53226)).

### CLI

```bash
betastart mcp-token "Client name"   # creates a client with all scopes and prints the token
```

## Repository structure

```
src-tauri/src/
├── main.rs            # entrypoint
├── lib.rs             # Tauri builder, plugins, setup
├── commands.rs        # IPC commands exposed to the frontend
├── error.rs           # unified AppError
├── state.rs           # AppState (config + SQLite connection)
├── models.rs          # shared DTOs
├── core/
│   ├── config.rs      # config.json (load/save/patch/normalize)
│   ├── paths.rs       # app, workspace, and runtime directories
│   ├── window.rs      # window show/hide/toggle
│   ├── tray.rs        # tray icon and menu
│   ├── hotkey.rs      # global launcher hotkey
│   └── win_hook.rs    # low-level Win key hook
├── db/
│   ├── mod.rs         # open + migrations
│   ├── schema.rs      # SQL schema
│   ├── apps.rs        # app index/search
│   ├── files.rs       # file index/search (FTS5)
│   ├── clipboard.rs   # clipboard history (text + images)
│   ├── extensions.rs  # extension registry/commands/permissions
│   ├── windows.rs     # window layouts/rules
│   ├── memory.rs      # memory index/search (FTS5)
│   ├── tasks.rs       # task index + reminders
│   └── usage.rs       # launcher usage ranking
├── indexer/           # background file indexing
├── clipboard_watcher.rs # background clipboard capture
├── memory/
│   └── mod.rs         # YAML frontmatter, wiki links, scan/indexing
├── graph/
│   └── mod.rs         # projects/notes/tasks/tags/links graph
├── mcp/
│   ├── mod.rs         # context, JSON-RPC, stdio, resources, prompts
│   ├── tools.rs       # tool definitions and implementation
│   ├── http.rs        # local HTTP/SSE (axum)
│   └── auth.rs        # Argon2 tokens
├── memory_api.rs      # memory + tasks + graph commands
├── mcp_api.rs         # MCP token/client commands
├── reminder_watcher.rs # reminder notifications
├── window_manager/    # layouts, rules, multi-monitor
├── window_watcher.rs  # applies rules to new windows
├── extensions/
│   ├── mod.rs         # manifest, installation (folder/Git), validation
│   ├── api.rs         # local host API for the extension SDK
│   └── runtime.rs     # Deno sidecar, runner, permissions, execution
└── platform/
    ├── mod.rs         # types + launch (Windows/Linux)
    ├── autostart.rs   # start with Windows
    ├── icons.rs       # real Windows icon extraction
    ├── apps.rs        # Start Menu (.lnk) scan
    └── windows.rs     # window + monitor enumeration (Win32)
src/                   # React frontend (launcher, settings, extensions, windows, memory, clipboard)
examples/              # example extension + store index
docs/                  # guides (signed auto-update)
scripts/               # signed build helper
```

## Current state — Phases 0 to 6

**Phase 0 — Foundation**
- [x] Persistent config + automatic migrations
- [x] SQLite database with the full schema
- [x] Start Menu app index + search
- [x] Floating window, tray, close→tray
- [x] Global hotkey (Alt+Space)
- [x] Window enumeration (Win32)

**Phase 1 — Launcher MVP**
- [x] Win key hook ("replace" mode)
- [x] File search with background indexing
- [x] Clipboard history + search
- [x] Inline calculator
- [x] Unified results
- [x] Settings (shortcut + Win key mode)

**Phase 2 — Extensions**
- [x] `extension.json` manifest with validation
- [x] Installation from local folder, Git, and store (`index.json`)
- [x] Deno sidecar runtime with automatic detection/download
- [x] Declared permissions and user approval
- [x] Extension commands in the launcher
- [x] `@betastart/sdk` (`registerCommand`, `ui`, `clipboard`, memory/tasks/windows/search)
- [x] 🧩 Extensions screen (install, enable, approve, uninstall)
- [x] Example extension + end-to-end tests + per-run logs

**Phase 3 — Window Manager**
- [x] Monitor enumeration with work area (multi-monitor)
- [x] Window enumeration with process name
- [x] Save/restore current layout
- [x] Per-layout hotkey (shortcut dispatcher)
- [x] Per-app rules (process/title/class, monitor, fractions)
- [x] Watcher that applies rules to new windows
- [x] **Hotkey capture by recording** (click → press → save)
- [x] **Conflict detection** with red border (launcher, layouts, system)
- [x] Layouts with **exact pixels** (reapply without drift)
- [x] 🪟 Window manager screen
- [x] Window picker for creating rules
- [x] Optional restore of the last layout at startup
- [x] Tests (Win+... hotkeys, layout/rule DAO, Win32 enumeration, pixel accuracy)

**Phase 4 — Memory + Tasks**
- [x] Markdown workspace with YAML frontmatter
- [x] Projects (`projects/<slug>/`) + main memory (`global/`)
- [x] SQLite indexing (`memory_index` + `tasks_index`) and scoped search
- [x] Note and task CRUD directly in files
- [x] Tasks with status, priority, due date, tags, **subtasks**, and reminders
- [x] **List** and **kanban** views
- [x] Reminder notifications (notification plugin)
- [x] 🧠 Memory & Tasks screen (resizes the window)
- [x] Autosave, backlinks, and daily note
- [x] Frontmatter + end-to-end indexing tests

**Phase 5 — Graph**
- [x] Wiki link (`[[...]]`) index per note
- [x] Graph of projects, notes, tasks, and tags
- [x] Project, tag, parent, and link edges
- [x] Force-directed view (`d3-force`) in the side panel
- [x] Clicking a node opens the corresponding file
- [x] Graph builder test

**Phase 6 — MCP**
- [x] MCP server (JSON-RPC) with 21 tools
- [x] **stdio** transport (`betastart mcp --token ...`)
- [x] Local **HTTP/SSE** transport (`/sse`, `/messages`, `/mcp`, `/health`)
- [x] Argon2 token hashes + per-client scopes
- [x] 🔌 MCP screen (create/revoke tokens, scopes, snippet)
- [x] `betastart mcp-token "Name"` CLI
- [x] Resources, prompts, and call audit
- [x] Initialize/tools/scopes/auth tests

## Recent improvements

- **Extensions**: local API with ephemeral token, full SDK (memory, projects, tasks,
  windows, search, and clipboard), per-run logs, atomic Deno download, and version cache.
- **Launcher**: real app icons, results ranked by frequency/recency, quick web search
  actions (`g`, `yt`, `gh`, `w`, `ddg`, `maps`), and file-extension badges.
- **Clipboard**: text and image history, filter, pin/unpin, delete, copy back
  (including images), and clear unpinned items.
- **Search**: real FTS5 for files and memory, with substring fallback.
- **Memory**: autosave, backlinks panel (`[[...]]`), and daily note.
- **Windows**: open-window picker for rules and optional restore of the last layout
  at startup.
- **Settings**: light/dark/automatic theme, start with Windows, index folders,
  clipboard, MCP, and extensions.
- **MCP**: resources, prompts, and audit of the latest calls.
- **Quality**: GitHub Actions CI and a release workflow with a signed NSIS installer
  and updater artifacts.

## Next steps

Follow [`ROADMAP.md`](./ROADMAP.md): Phase 7 (distribution — installer + auto-update).
