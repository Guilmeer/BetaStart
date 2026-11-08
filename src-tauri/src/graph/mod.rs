//! Grafo de conhecimento: projetos, notas, tasks, tags e links wiki.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, params};

use crate::error::AppResult;
use crate::models::{Graph, GraphEdge, GraphNode};

struct GraphItem {
    file_path: String,
    kind: String,
    title: String,
    project: String,
    tags: Vec<String>,
    links: Vec<String>,
}

pub fn build_graph(conn: &Connection, project: Option<&str>) -> AppResult<Graph> {
    let mut items: Vec<GraphItem> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT file_path, kind, title, project,
                    COALESCE(tags, '[]'), COALESCE(links, '[]')
               FROM memory_index
              WHERE (?1 IS NULL OR project = ?1)",
        )?;
        let rows = stmt.query_map(params![project], |row| {
            let tags: String = row.get(4)?;
            let links: String = row.get(5)?;
            Ok(GraphItem {
                file_path: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                project: row.get(3)?,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                links: serde_json::from_str(&links).unwrap_or_default(),
            })
        })?;
        for row in rows {
            items.push(row?);
        }
    }

    // id da task -> file_path; e child id -> parent id
    let mut task_paths: HashMap<String, String> = HashMap::new();
    let mut task_parents: HashMap<String, String> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT id, file_path, parent_id FROM tasks_index")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows {
            let (id, file_path, parent) = row?;
            task_paths.insert(id.clone(), file_path);
            if let Some(parent) = parent {
                task_parents.insert(id, parent);
            }
        }
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seen_nodes: HashSet<String> = HashSet::new();
    let mut seen_edges: HashSet<String> = HashSet::new();
    let mut project_nodes: HashSet<String> = HashSet::new();
    let mut tag_nodes: HashSet<String> = HashSet::new();
    let mut title_index: HashMap<String, String> = HashMap::new();

    for item in &items {
        let id = item.file_path.clone();
        if seen_nodes.insert(id.clone()) {
            nodes.push(GraphNode {
                id: id.clone(),
                kind: item.kind.clone(),
                label: item.title.clone(),
                project: Some(item.project.clone()),
                path: Some(item.file_path.clone()),
            });
        }

        let title_key = item.title.trim().to_lowercase();
        if !title_key.is_empty() {
            title_index.entry(title_key).or_insert(id.clone());
        }
        if let Some(stem) = std::path::Path::new(&item.file_path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase())
        {
            title_index.entry(stem).or_insert(id.clone());
        }

        let project_id = format!("project:{}", item.project);
        if project_nodes.insert(item.project.clone()) {
            nodes.push(GraphNode {
                id: project_id.clone(),
                kind: "project".into(),
                label: if item.project == "global" {
                    "Global".into()
                } else {
                    item.project.clone()
                },
                project: Some(item.project.clone()),
                path: None,
            });
        }
        let edge_key = format!("{project_id}->{id}:project");
        if seen_edges.insert(edge_key) {
            edges.push(GraphEdge {
                source: project_id,
                target: id.clone(),
                kind: "project".into(),
            });
        }

        for tag in &item.tags {
            let tag_lower = tag.to_lowercase();
            let tag_id = format!("tag:{tag_lower}");
            if tag_nodes.insert(tag_lower.clone()) {
                nodes.push(GraphNode {
                    id: tag_id.clone(),
                    kind: "tag".into(),
                    label: format!("#{tag}"),
                    project: None,
                    path: None,
                });
            }
            let edge_key = format!("{tag_id}->{id}:tag");
            if seen_edges.insert(edge_key) {
                edges.push(GraphEdge {
                    source: tag_id,
                    target: id.clone(),
                    kind: "tag".into(),
                });
            }
        }
    }

    // task pai -> subtask
    for (child_id, parent_id) in &task_parents {
        let Some(child_path) = task_paths.get(child_id) else {
            continue;
        };
        let Some(parent_path) = task_paths.get(parent_id) else {
            continue;
        };
        let edge_key = format!("{parent_path}->{child_path}:parent");
        if seen_edges.insert(edge_key) {
            edges.push(GraphEdge {
                source: parent_path.clone(),
                target: child_path.clone(),
                kind: "parent".into(),
            });
        }
    }

    // links wiki [[destino]]
    for item in &items {
        for link in &item.links {
            let key = link.trim().to_lowercase();
            if let Some(target) = title_index.get(&key) {
                if target != &item.file_path {
                    let edge_key = format!("{}->{target}:link", item.file_path);
                    if seen_edges.insert(edge_key) {
                        edges.push(GraphEdge {
                            source: item.file_path.clone(),
                            target: target.clone(),
                            kind: "link".into(),
                        });
                    }
                }
            }
        }
    }

    Ok(Graph { nodes, edges })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MemoryRow, TaskRow};

    #[test]
    fn builds_project_tag_parent_and_link_edges() {
        let db_path = std::env::temp_dir().join(format!(
            "betastart-graph-test-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let conn = crate::db::open(&db_path).expect("db");

        crate::db::memory::replace_all(
            &conn,
            &[
                MemoryRow {
                    file_path: "p/a.md".into(),
                    kind: "note".into(),
                    title: "Alpha".into(),
                    project: "p".into(),
                    tags: vec!["x".into()],
                    links: vec!["Beta".into()],
                    content: String::new(),
                    updated_at: 0,
                },
                MemoryRow {
                    file_path: "p/b.md".into(),
                    kind: "note".into(),
                    title: "Beta".into(),
                    project: "p".into(),
                    tags: vec![],
                    links: vec![],
                    content: String::new(),
                    updated_at: 0,
                },
                MemoryRow {
                    file_path: "p/t.md".into(),
                    kind: "task".into(),
                    title: "Pai".into(),
                    project: "p".into(),
                    tags: vec![],
                    links: vec![],
                    content: String::new(),
                    updated_at: 0,
                },
                MemoryRow {
                    file_path: "p/c.md".into(),
                    kind: "task".into(),
                    title: "Filho".into(),
                    project: "p".into(),
                    tags: vec![],
                    links: vec![],
                    content: String::new(),
                    updated_at: 0,
                },
            ],
        )
        .expect("memory");

        crate::db::tasks::replace_all(
            &conn,
            &[
                TaskRow {
                    id: "t1".into(),
                    file_path: "p/t.md".into(),
                    title: "Pai".into(),
                    status: "todo".into(),
                    priority: "medium".into(),
                    due: None,
                    remind: None,
                    tags: vec![],
                    project: "p".into(),
                    parent_id: None,
                    updated_at: 0,
                },
                TaskRow {
                    id: "t2".into(),
                    file_path: "p/c.md".into(),
                    title: "Filho".into(),
                    status: "todo".into(),
                    priority: "medium".into(),
                    due: None,
                    remind: None,
                    tags: vec![],
                    project: "p".into(),
                    parent_id: Some("t1".into()),
                    updated_at: 0,
                },
            ],
        )
        .expect("tasks");

        let graph = build_graph(&conn, Some("p")).expect("graph");
        assert!(graph.nodes.iter().any(|node| node.kind == "project"));
        assert!(graph.nodes.iter().any(|node| node.kind == "tag"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "tag"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "parent"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "link"));

        let _ = std::fs::remove_file(&db_path);
    }
}
