use rusqlite::{Connection, OptionalExtension, params};

use crate::error::AppResult;
use crate::extensions::{ExtensionManifest, ManifestCommand};
use crate::models::{CommandEntry, InstalledExtension};

pub struct InstalledRow {
    pub manifest: ExtensionManifest,
    pub install_path: String,
    pub granted: Vec<String>,
    pub pending: bool,
    pub enabled: bool,
}

/// Registra/atualiza uma extensão instalada e seus comandos.
pub fn upsert(
    conn: &Connection,
    manifest: &ExtensionManifest,
    source: &str,
    source_url: Option<&str>,
    install_path: &str,
) -> AppResult<()> {
    let manifest_json = serde_json::to_string(manifest)?;
    // Extensões sem permissões já nascem aprovadas; com permissões ficam pendentes.
    let granted_json: Option<String> = if manifest.permissions.is_empty() {
        Some("[]".into())
    } else {
        None
    };

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO extension_registry
           (ext_id, name, version, install_source, source_url, install_path,
            manifest_json, enabled, permissions_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9)
         ON CONFLICT(ext_id) DO UPDATE SET
           name             = excluded.name,
           version          = excluded.version,
           install_source   = excluded.install_source,
           source_url       = excluded.source_url,
           install_path     = excluded.install_path,
           manifest_json    = excluded.manifest_json,
           enabled          = 1,
           permissions_json = excluded.permissions_json,
           updated_at       = excluded.updated_at",
        params![
            manifest.id,
            manifest.name,
            manifest.version,
            source,
            source_url,
            install_path,
            manifest_json,
            granted_json,
            crate::db::now_secs()
        ],
    )?;

    tx.execute(
        "DELETE FROM extension_commands WHERE ext_id = ?1",
        params![manifest.id],
    )?;
    for command in &manifest.commands {
        tx.execute(
            "INSERT OR REPLACE INTO extension_commands
               (ext_id, command_id, title, subtitle, keywords, handler, icon)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                manifest.id,
                command.id,
                command.title,
                command.subtitle,
                command.keywords.join(","),
                command.id,
                command.icon
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn set_permissions(conn: &Connection, ext_id: &str, permissions: &[String]) -> AppResult<()> {
    let json = serde_json::to_string(permissions)?;
    conn.execute(
        "UPDATE extension_registry SET permissions_json = ?2, updated_at = ?3 WHERE ext_id = ?1",
        params![ext_id, json, crate::db::now_secs()],
    )?;
    Ok(())
}

pub fn set_enabled(conn: &Connection, ext_id: &str, enabled: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE extension_registry SET enabled = ?2, updated_at = ?3 WHERE ext_id = ?1",
        params![ext_id, enabled as i64, crate::db::now_secs()],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, ext_id: &str) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM extension_commands WHERE ext_id = ?1",
        params![ext_id],
    )?;
    tx.execute(
        "DELETE FROM extension_registry WHERE ext_id = ?1",
        params![ext_id],
    )?;
    tx.commit()?;
    Ok(())
}

fn build(
    manifest: ExtensionManifest,
    install_source: String,
    install_path: String,
    enabled: bool,
    granted_json: Option<String>,
) -> InstalledExtension {
    let granted: Vec<String> = granted_json
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    let pending = !manifest.permissions.is_empty() && granted_json.is_none();

    InstalledExtension {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        author: manifest.author.clone(),
        install_source,
        install_path,
        enabled,
        permissions: manifest.permissions.clone(),
        granted,
        pending,
        commands: manifest.commands.clone(),
    }
}

pub fn list(conn: &Connection) -> AppResult<Vec<InstalledExtension>> {
    let mut stmt = conn.prepare(
        "SELECT manifest_json, install_source, install_path, enabled, permissions_json
           FROM extension_registry
          ORDER BY name COLLATE NOCASE ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)? != 0,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (manifest_json, source, install_path, enabled, granted_json) = row?;
        if let Ok(manifest) = serde_json::from_str::<ExtensionManifest>(&manifest_json) {
            out.push(build(manifest, source, install_path, enabled, granted_json));
        }
    }
    Ok(out)
}

pub fn get_installed(conn: &Connection, ext_id: &str) -> AppResult<Option<InstalledExtension>> {
    let row = conn
        .query_row(
            "SELECT manifest_json, install_source, install_path, enabled, permissions_json
               FROM extension_registry WHERE ext_id = ?1",
            params![ext_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;

    Ok(row.and_then(
        |(manifest_json, source, install_path, enabled, granted_json)| {
            serde_json::from_str::<ExtensionManifest>(&manifest_json)
                .ok()
                .map(|manifest| build(manifest, source, install_path, enabled, granted_json))
        },
    ))
}

pub fn get_row(conn: &Connection, ext_id: &str) -> AppResult<Option<InstalledRow>> {
    let row = conn
        .query_row(
            "SELECT manifest_json, install_path, permissions_json, enabled
               FROM extension_registry WHERE ext_id = ?1",
            params![ext_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)? != 0,
                ))
            },
        )
        .optional()?;

    Ok(
        row.and_then(|(manifest_json, install_path, granted_json, enabled)| {
            serde_json::from_str::<ExtensionManifest>(&manifest_json)
                .ok()
                .map(|manifest| {
                    let granted: Vec<String> = granted_json
                        .as_deref()
                        .and_then(|raw| serde_json::from_str(raw).ok())
                        .unwrap_or_default();
                    let pending = !manifest.permissions.is_empty() && granted_json.is_none();
                    InstalledRow {
                        manifest,
                        install_path,
                        granted,
                        pending,
                        enabled,
                    }
                })
        }),
    )
}

pub fn commands(conn: &Connection, query: &str, limit: usize) -> AppResult<Vec<CommandEntry>> {
    let escaped = query.replace('%', "").replace('_', "");
    let like = format!("%{escaped}%");

    let mut stmt = conn.prepare(
        "SELECT c.ext_id, c.command_id, c.title, c.subtitle, c.icon
           FROM extension_commands c
           JOIN extension_registry e ON e.ext_id = c.ext_id
          WHERE e.enabled = 1
            AND e.permissions_json IS NOT NULL
            AND (c.title LIKE ?1 OR c.command_id LIKE ?1 OR c.keywords LIKE ?1)
          ORDER BY c.title COLLATE NOCASE ASC
          LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![like, limit as i64], |row| {
        Ok(CommandEntry {
            ext_id: row.get(0)?,
            command_id: row.get(1)?,
            title: row.get(2)?,
            subtitle: row.get(3)?,
            icon: row.get(4)?,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(
        conn.query_row("SELECT COUNT(*) FROM extension_registry", [], |row| {
            row.get(0)
        })?,
    )
}

#[allow(dead_code)]
pub fn manifest_commands(manifest: &ExtensionManifest) -> &[ManifestCommand] {
    &manifest.commands
}
