# Beta Start

A free, open-source (MIT) launcher and productivity hub for Windows.
One floating window to open apps, find files, use your clipboard, manage windows,
run extensions, and keep a personal memory + task system — with AI integration
through MCP.

## Why Beta Start

- **One shortcut for everything** — apps, files, clipboard, calculations, and
  extension commands in a single search.
- **A real second brain** — notes and tasks stored as plain Markdown files you can
  keep, edit, and version with Git.
- **Windows that stay organized** — save layouts, create per-app rules, and restore
  them with a shortcut.
- **Extensible** — install TypeScript/JavaScript extensions from a folder, Git, or
  the store.
- **AI-ready** — expose memory, tasks, windows, search, and clipboard to AI clients
  through MCP.
- **Always up to date** — signed automatic updates from the app itself.

## Install (Windows)

1. Download the latest installer from
   [Releases](https://github.com/Guilmeer/BetaStart/releases/latest).
2. Run the installer. On Windows 10/11, the Microsoft WebView2 Runtime is required
   (it is already present on most systems).
3. If Windows shows **"Windows protected your PC"**, click **More info → Run anyway**.
   The app updates are cryptographically signed; the SmartScreen warning appears
   because the installer does not yet have an Authenticode publisher certificate.
   See [`docs/CODE_SIGNING.md`](./docs/CODE_SIGNING.md) for details.
4. Beta Start opens with **Alt + Space** and stays available in the system tray.

Closing the window or clicking outside only hides it. To quit completely, use
**Quit** in the tray menu.

## Getting Started

- **Alt + Space** opens and closes the launcher.
- Type to search **apps**, **files**, **clipboard**, extension commands, or use the
  **calculator** (e.g. `12*8+3`).
- Use **↑ / ↓** to move, **Enter** to open, and **Esc** to close.
- Quick web search: type `g`, `yt`, `gh`, `w`, `ddg`, or `maps` followed by a term.
- Use the top bar icons to open **Clipboard**, **MCP**, **Memory & Tasks**,
  **Window Manager**, **Extensions**, and **Settings**.

## Extensions

An extension is a folder with an `extension.json` manifest and TS/JS code.

- Install from a **local folder**, a **Git repository**, or the **store**.
- Approve the permissions declared by the extension before running it.
- Extension commands appear directly in the launcher.
- Each run is isolated in its own Deno process.
- Logs for each run are available from **Extensions → Logs**.

A ready-to-use example is in
[`examples/hello-extension`](./examples/hello-extension). The store format is
documented by [`examples/store/index.json`](./examples/store/index.json).

## Memory & Tasks

Open **🧠** in the top bar. The window grows to fit the workspace.

- The main memory lives in `global/`; each project lives in `projects/<slug>/`.
- Everything is **real Markdown** with YAML frontmatter, so you can open, edit, and
  version the files outside the app.
- Create **projects**, **notes**, and **tasks**; edit titles, tags, and content.
- Search across a project or everything.
- Tasks support status, priority, due dates, reminders, tags, and **subtasks**.
- Switch between **List** and **Kanban** views.
- Edits are autosaved; a **daily note** is one click away.
- Notes that reference each other with `[[Note name]]` show a **Linked from**
  panel with backlinks.

## Graph

In the memory screen, switch the right panel to **Graph**. The force-directed graph
shows projects, notes, tasks, and tags, with edges for:

- **project** — project → item;
- **tag** — tag → item;
- **parent** — parent task → subtask;
- **link** — `[[Another note]]` references.

Click a node to open its file. The project filter also filters the graph.

## Window Manager

Open **🪟** in the top bar.

### Layouts

1. Arrange your windows manually.
2. Name the layout and click **Save current**.
3. Record a shortcut by clicking the field and pressing the combination.
4. Press the shortcut anytime to reapply the layout.

Layouts remember exact pixel positions and scale proportionally if the monitor
resolution changes.

### Per-app rules

Choose an open window to fill the rule automatically, or configure the process,
title, or class manually along with the target monitor and region. Rules are
applied automatically when a new window appears, or on demand with
**Apply rules now**.

You can also restore the last layout automatically when Beta Start starts.

## Clipboard

Open **📋** in the top bar.

- Searchable history of copied **text and images**.
- **Pin** important items so they stay at the top.
- **Delete** individual items or clear everything that is not pinned.
- Copy a text or image item back to the clipboard.
- Images can be previewed inside the app or opened in the default viewer.

## MCP (AI Integration)

Open **🔌** in the top bar.

1. Create a client and choose its scopes.
2. Generate a token. It is shown only once.
3. Copy the generated configuration snippet or use the HTTP/SSE endpoint.

Supported clients include Claude Desktop, Cursor, and any MCP-compatible tool.

### stdio

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

### HTTP/SSE

- SSE: `http://127.0.0.1:19000/sse?token=<token>`
- JSON-RPC POST: `http://127.0.0.1:19000/mcp`
- Health: `GET /health`

### Tools

Memory: `memory.list`, `memory.read`, `memory.write`, `memory.search`,
`memory.context`, `project.list`, `project.create`, `project.link`,
`project.unlink`, `project.brief`.

Graph: `graph.query`, `graph.neighbors`, `graph.path`, `graph.explain`, plus
`graph.get` for the full graph (expensive by design).

Tasks: `tasks.list`, `tasks.create`, `tasks.create_many`, `tasks.update`,
`tasks.delete`.

Windows: `windows.list`, `windows.move_resize`, `windows.apply_layout`,
`windows.save_layout`.

Other: `extensions.list`, `extensions.run`, `search.query`, `clipboard.list`,
`settings.get`, `settings.set`.

MCP also exposes **resources** (notes and tasks as Markdown), ready-made
**prompts**, and an audit of recent calls in the 🔌 screen.

### Token-efficient by default

- Every response is compact JSON; tool descriptions steer the assistant to the
  cheapest path.
- `project.brief` summarizes a project in a few hundred tokens.
- `memory.read` supports `mode: "outline"` and `mode: "range"`, and caps `full`
  reads.
- `graph.query` / `graph.neighbors` return a scoped subgraph with a token
  budget, instead of the whole graph.
- Paths come back relative to the project.
- The server sends read/write **instructions** on `initialize`.

### AI guidance

**MCP → AI guidance** writes (or updates) a marked block in the `AGENTS.md` of
the selected project, teaching the assistant to query Beta Start instead of
re-reading files. Existing content is preserved.

## Settings

Open **⚙** in the top bar.

- Launcher shortcut and Windows key mode.
- **Language**: English (default) or **Português (Brasil)**.
- Theme: dark, light, or automatic.
- Start with Windows and close-on-blur behavior.
- Search settings: include apps/files, indexed folders, and result count.
- Clipboard history limit.
- Window rule behavior and layout restoration.
- MCP server and port.
- Extension store URL and Deno path.
- **Updates**: check, download, and install new versions.

## Automatic Updates

Beta Start can update itself from signed releases:

1. Open **⚙ Settings → Updates**.
2. Click **Check for updates**.
3. If a new version is available, click **Download and install**.

Updates are signed with a minisign key and verified before installation. Windows
SmartScreen may still warn about an unsigned installer; see
[`docs/CODE_SIGNING.md`](./docs/CODE_SIGNING.md) for the Authenticode options.

## License

MIT — see [`LICENSE`](./LICENSE).
