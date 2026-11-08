use rusqlite::Connection;

use crate::error::AppResult;

/// Aplica as migrações usando `PRAGMA user_version`.
pub fn init(conn: &mut Connection) -> AppResult<()> {
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let _ = conn.pragma_update(None, "foreign_keys", "ON");

    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    if version < 2 {
        conn.execute_batch(MIGRATION_V2)?;
        conn.pragma_update(None, "user_version", 2)?;
    }
    if version < 3 {
        conn.execute_batch(MIGRATION_V3)?;
        conn.pragma_update(None, "user_version", 3)?;
    }
    Ok(())
}

const MIGRATION_V3: &str = r#"
ALTER TABLE memory_index ADD COLUMN content TEXT;
ALTER TABLE tasks_index ADD COLUMN remind INTEGER;
CREATE UNIQUE INDEX IF NOT EXISTS idx_reminders_task ON reminders(task_id);
"#;

const MIGRATION_V2: &str = r#"
CREATE INDEX IF NOT EXISTS idx_files_name ON files_index(name);
CREATE INDEX IF NOT EXISTS idx_memory_project ON memory_index(project);
"#;

const SCHEMA_V1: &str = r#"
-- ============================================================
-- Launcher: índice de aplicativos
-- ============================================================
CREATE TABLE IF NOT EXISTS apps_index (
  id            INTEGER PRIMARY KEY,
  name          TEXT    NOT NULL,
  exe_path      TEXT    NOT NULL UNIQUE,
  icon_path     TEXT,
  start_menu    TEXT,
  keywords      TEXT    NOT NULL DEFAULT '',
  launch_count  INTEGER NOT NULL DEFAULT 0,
  last_launched INTEGER,
  indexed_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_apps_name ON apps_index(name);

-- ============================================================
-- Launcher: índice de arquivos + FTS5
-- ============================================================
CREATE TABLE IF NOT EXISTS files_index (
  id     INTEGER PRIMARY KEY,
  path   TEXT    NOT NULL UNIQUE,
  name   TEXT    NOT NULL,
  ext    TEXT,
  size   INTEGER,
  mtime  INTEGER,
  is_dir INTEGER NOT NULL DEFAULT 0
);
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts
  USING fts5(name, path, content='files_index', content_rowid='id');

-- ============================================================
-- Histórico de clipboard
-- ============================================================
CREATE TABLE IF NOT EXISTS clipboard_history (
  id         INTEGER PRIMARY KEY,
  kind       TEXT    NOT NULL,
  text       TEXT,
  html       TEXT,
  image_path TEXT,
  source_app TEXT,
  copied_at  INTEGER NOT NULL,
  pinned     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_clipboard_copied ON clipboard_history(copied_at DESC);

-- ============================================================
-- Extensões
-- ============================================================
CREATE TABLE IF NOT EXISTS extension_registry (
  id               INTEGER PRIMARY KEY,
  ext_id           TEXT    NOT NULL UNIQUE,
  name             TEXT    NOT NULL,
  version          TEXT,
  install_source   TEXT    NOT NULL,
  source_url       TEXT,
  install_path     TEXT    NOT NULL,
  manifest_json    TEXT    NOT NULL,
  enabled          INTEGER NOT NULL DEFAULT 1,
  permissions_json TEXT,
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

-- ============================================================
-- Clientes / tokens MCP
-- ============================================================
CREATE TABLE IF NOT EXISTS mcp_clients (
  id          INTEGER PRIMARY KEY,
  client_name TEXT    NOT NULL,
  token_hash  TEXT    NOT NULL UNIQUE,
  scopes_json TEXT    NOT NULL,
  created_at  INTEGER NOT NULL,
  last_seen   INTEGER,
  revoked     INTEGER NOT NULL DEFAULT 0
);

-- ============================================================
-- Ambiente de memória / tasks (índices denormalizados dos arquivos)
-- ============================================================
CREATE TABLE IF NOT EXISTS memory_index (
  id         INTEGER PRIMARY KEY,
  file_path  TEXT NOT NULL UNIQUE,
  kind       TEXT NOT NULL,
  title      TEXT,
  project    TEXT,
  tags       TEXT,
  updated_at INTEGER
);
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts
  USING fts5(title, content, tags, content='memory_index', content_rowid='id');

CREATE TABLE IF NOT EXISTS tasks_index (
  id         TEXT PRIMARY KEY,
  file_path  TEXT NOT NULL,
  title      TEXT NOT NULL,
  status     TEXT NOT NULL DEFAULT 'todo',
  priority   TEXT NOT NULL DEFAULT 'medium',
  due        INTEGER,
  tags       TEXT,
  project    TEXT,
  parent_id  TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks_index(status, project);

-- ============================================================
-- Gerenciador de janelas
-- ============================================================
CREATE TABLE IF NOT EXISTS window_layouts (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  hotkey        TEXT,
  monitors_json TEXT NOT NULL,
  windows_json  TEXT NOT NULL,
  created_at    INTEGER,
  updated_at    INTEGER
);

CREATE TABLE IF NOT EXISTS window_rules (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  app_match   TEXT NOT NULL,
  match_field TEXT NOT NULL DEFAULT 'process',
  monitor     INTEGER NOT NULL DEFAULT 0,
  x  REAL, y  REAL, w  REAL, h  REAL,
  x_px INTEGER, y_px INTEGER, w_px INTEGER, h_px INTEGER,
  enabled     INTEGER NOT NULL DEFAULT 1
);

-- ============================================================
-- Lembretes e cache do grafo
-- ============================================================
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
"#;
