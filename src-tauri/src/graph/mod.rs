//! Grafo de conhecimento: projetos, notas, tasks, tags e links wiki.
//!
//! Dois usos distintos:
//! - **UI** (`build_graph`): o desenho force-directed quer todos os nós e as
//!   arestas. Os `id` são curtos e estáveis (hash do caminho) para que o
//!   layout não pule a cada refresh.
//! - **IA** (`query`, `neighbors`, `path`, `explain`): consultas escopadas com
//!   orçamento de tokens, no espírito do Graphify.

use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{Connection, params};

use crate::error::AppResult;
use crate::models::{
    Graph, GraphConnection, GraphEdge, GraphExplain, GraphNode, GraphPath, GraphSeed, GraphView,
};

struct GraphItem {
    file_path: String,
    kind: String,
    title: String,
    project: String,
    tags: Vec<String>,
    links: Vec<String>,
}

/// Id curto e estável para um arquivo (17 chars em vez do caminho absoluto).
///
/// A UI usa como chave de nó; a IA usa para `graph.explain`/`graph.path`.
pub fn node_id(path: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("n{hash:016x}")
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
    let mut path_to_id: HashMap<String, String> = HashMap::new();

    for item in &items {
        let id = node_id(&item.file_path);
        path_to_id.insert(item.file_path.clone(), id.clone());
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
    //
    // `tasks_index` é global, mas o grafo pode estar filtrado por projeto.
    // Sem checar os nós do grafo, uma subtask cujo pai está em outro projeto
    // gera uma aresta para um nó inexistente — e o forceLink do frontend lança
    // "node not found" no meio do render (a tela travava).
    for (child_id, parent_id) in &task_parents {
        let Some(child_path) = task_paths.get(child_id) else {
            continue;
        };
        let Some(parent_path) = task_paths.get(parent_id) else {
            continue;
        };
        let (Some(child), Some(parent)) =
            (path_to_id.get(child_path), path_to_id.get(parent_path))
        else {
            continue;
        };
        if !seen_nodes.contains(child) || !seen_nodes.contains(parent) {
            continue;
        }
        let edge_key = format!("{parent}->{child}:parent");
        if seen_edges.insert(edge_key) {
            edges.push(GraphEdge {
                source: parent.clone(),
                target: child.clone(),
                kind: "parent".into(),
            });
        }
    }

    // links wiki [[destino]]
    for item in &items {
        let Some(source) = path_to_id.get(&item.file_path) else {
            continue;
        };
        for link in &item.links {
            let key = link.trim().to_lowercase();
            if key.is_empty() {
                continue;
            }

            if let Some(target) = title_index.get(&key) {
                if target != source {
                    let edge_key = format!("{source}->{target}:link");
                    if seen_edges.insert(edge_key) {
                        edges.push(GraphEdge {
                            source: source.clone(),
                            target: target.clone(),
                            kind: "link".into(),
                        });
                    }
                }
            } else {
                // Destino inexistente: mostra um nó fantasma para o usuário perceber.
                let missing_id = format!("missing:{key}");
                if seen_nodes.insert(missing_id.clone()) {
                    nodes.push(GraphNode {
                        id: missing_id.clone(),
                        kind: "missing".into(),
                        label: link.clone(),
                        project: None,
                        path: None,
                    });
                }
                let edge_key = format!("{source}->{missing_id}:link");
                if seen_edges.insert(edge_key) {
                    edges.push(GraphEdge {
                        source: source.clone(),
                        target: missing_id,
                        kind: "link".into(),
                    });
                }
            }
        }
    }

    Ok(Graph { nodes, edges })
}

/// `build_graph` com cache invalidado pela versão do índice.
pub fn build_graph_cached(conn: &Connection, project: Option<&str>) -> AppResult<Graph> {
    let key = format!(
        "g4:{}:{}",
        crate::db::memory_version(conn),
        project.unwrap_or("*")
    );
    if let Some(json) = crate::db::graph_cache_get(conn, &key, 7 * 24 * 3600)? {
        if let Ok(graph) = serde_json::from_str::<Graph>(&json) {
            return Ok(graph);
        }
    }

    let graph = build_graph(conn, project)?;
    if let Ok(json) = serde_json::to_string(&graph) {
        let _ = crate::db::graph_cache_put(conn, &key, &json);
    }
    Ok(graph)
}

// ---------------------------------------------------------------------------
// Consultas escopadas (para IA)
// ---------------------------------------------------------------------------

fn degree_map(edges: &[GraphEdge]) -> HashMap<&str, usize> {
    let mut degree: HashMap<&str, usize> = HashMap::new();
    for edge in edges {
        *degree.entry(edge.source.as_str()).or_default() += 1;
        *degree.entry(edge.target.as_str()).or_default() += 1;
    }
    degree
}

fn adjacency(edges: &[GraphEdge]) -> HashMap<&str, Vec<&GraphEdge>> {
    let mut map: HashMap<&str, Vec<&GraphEdge>> = HashMap::new();
    for edge in edges {
        map.entry(edge.source.as_str()).or_default().push(edge);
        map.entry(edge.target.as_str()).or_default().push(edge);
    }
    map
}

fn node_cost(node: &GraphNode) -> usize {
    node.label.chars().count()
        + node.path.as_deref().map(|path| path.chars().count()).unwrap_or(0)
        + 40
}

/// Quantas arestas do candidato já têm a outra ponta dentro do recorte.
///
/// Essas são exatamente as arestas que passam a aparecer no JSON quando o nó
/// entra. Contar o grau inteiro superestimava e desperdiçava o orçamento.
fn edges_into(
    node: &GraphNode,
    included: &HashSet<&str>,
    adjacency: &HashMap<&str, Vec<&GraphEdge>>,
) -> usize {
    adjacency
        .get(node.id.as_str())
        .map(|edges| {
            edges
                .iter()
                .filter(|edge| {
                    let other = if edge.source == node.id {
                        edge.target.as_str()
                    } else {
                        edge.source.as_str()
                    };
                    included.contains(other)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Resolve um nó por id, rótulo, caminho ou nome do arquivo.
fn resolve_node<'a>(graph: &'a Graph, needle: &str) -> Result<&'a GraphNode, String> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Err("node is required".into());
    }
    if let Some(node) = graph.nodes.iter().find(|node| node.id == needle) {
        return Ok(node);
    }

    let lower = needle.to_lowercase();
    let normalized = lower.replace('\\', "/");
    let mut matches: Vec<&GraphNode> = graph
        .nodes
        .iter()
        .filter(|node| {
            if node.label.to_lowercase() == lower {
                return true;
            }
            let Some(path) = node.path.as_deref() else {
                return false;
            };
            let path = path.replace('\\', "/").to_lowercase();
            if path == normalized || path.ends_with(&format!("/{normalized}")) {
                return true;
            }
            path.rsplit('/')
                .next()
                .map(|name| name.trim_end_matches(".md") == normalized)
                .unwrap_or(false)
        })
        .collect();

    match matches.len() {
        0 => Err(format!(
            "node '{needle}' not found; use graph.query to locate it"
        )),
        1 => Ok(matches.remove(0)),
        _ => {
            let list: Vec<String> = matches
                .iter()
                .take(8)
                .map(|node| format!("{} ({})", node.label, node.id))
                .collect();
            Err(format!(
                "ambiguous node '{needle}': {}; try the id",
                list.join(", ")
            ))
        }
    }
}

/// Expande um conjunto de sementes por `depth` saltos, respeitando o orçamento.
fn expand(
    graph: &Graph,
    seeds: &[&GraphNode],
    depth: usize,
    budget_tokens: usize,
) -> GraphView {
    let limit_chars = budget_tokens.clamp(200, 16_000) * 4;
    let adjacency = adjacency(&graph.edges);
    let degree = degree_map(&graph.edges);
    let by_id: HashMap<&str, &GraphNode> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    let mut included: HashSet<&str> = HashSet::new();
    let mut cost = 0usize;
    let mut frontier: Vec<&GraphNode> = Vec::new();
    let mut truncated = false;

    for node in seeds {
        if included.insert(node.id.as_str()) {
            cost += node_cost(node);
            frontier.push(node);
        }
    }
    // Arestas entre as próprias sementes também entram na resposta.
    cost += graph
        .edges
        .iter()
        .filter(|edge| {
            included.contains(edge.source.as_str()) && included.contains(edge.target.as_str())
        })
        .count()
        * 60;

    for _ in 0..depth.max(1) {
        if frontier.is_empty() {
            break;
        }
        let mut next: Vec<&GraphNode> = Vec::new();
        for node in &frontier {
            if let Some(edges) = adjacency.get(node.id.as_str()) {
                for edge in edges {
                    let other = if edge.source == node.id {
                        edge.target.as_str()
                    } else {
                        edge.source.as_str()
                    };
                    if included.contains(other) {
                        continue;
                    }
                    if let Some(candidate) = by_id.get(other) {
                        next.push(candidate);
                    }
                }
            }
        }
        next.sort_by_key(|node| std::cmp::Reverse(*degree.get(node.id.as_str()).unwrap_or(&0)));
        next.dedup_by_key(|node| node.id.clone());

        frontier = Vec::new();
        for node in next {
            if included.contains(node.id.as_str()) {
                continue;
            }
            let node_chars = node_cost(node) + edges_into(node, &included, &adjacency) * 60;
            if cost + node_chars > limit_chars {
                truncated = true;
                continue;
            }
            cost += node_chars;
            included.insert(node.id.as_str());
            frontier.push(node);
        }
    }

    let mut nodes: Vec<GraphNode> = graph
        .nodes
        .iter()
        .filter(|node| included.contains(node.id.as_str()))
        .cloned()
        .collect();
    nodes.sort_by_key(|node| std::cmp::Reverse(*degree.get(node.id.as_str()).unwrap_or(&0)));

    let edges: Vec<GraphEdge> = graph
        .edges
        .iter()
        .filter(|edge| {
            included.contains(edge.source.as_str()) && included.contains(edge.target.as_str())
        })
        .cloned()
        .collect();

    GraphView {
        seeds: seeds
            .iter()
            .map(|node| GraphSeed {
                id: node.id.clone(),
                label: node.label.clone(),
            })
            .collect(),
        truncated,
        total_nodes: graph.nodes.len(),
        total_edges: graph.edges.len(),
        nodes,
        edges,
    }
}

/// Encontra nós por termo e devolve o subgrafo ao redor deles.
pub fn query(
    conn: &Connection,
    project: Option<&str>,
    terms: &str,
    depth: usize,
    budget_tokens: usize,
) -> AppResult<GraphView> {
    let graph = build_graph_cached(conn, project)?;
    let needles: Vec<String> = terms
        .split_whitespace()
        .map(|term| term.to_lowercase())
        .filter(|term| term.chars().count() > 1)
        .take(8)
        .collect();
    if needles.is_empty() {
        return Err("query is required".to_string().into());
    }

    let degree = degree_map(&graph.edges);
    let mut scored: Vec<(usize, usize, &GraphNode)> = graph
        .nodes
        .iter()
        .filter_map(|node| {
            let label = node.label.to_lowercase();
            let path = node
                .path
                .as_deref()
                .unwrap_or("")
                .replace('\\', "/")
                .to_lowercase();
            let mut score = 0usize;
            for needle in &needles {
                if label == *needle {
                    score += 6;
                } else if label.contains(needle.as_str()) {
                    score += 3;
                }
                if path.contains(needle.as_str()) {
                    score += 2;
                }
            }
            (score > 0).then(|| (score, *degree.get(node.id.as_str()).unwrap_or(&0), node))
        })
        .collect();

    scored.sort_by(|left, right| right.0.cmp(&left.0).then(right.1.cmp(&left.1)));
    let seeds: Vec<&GraphNode> = scored.iter().take(6).map(|(_, _, node)| *node).collect();

    if seeds.is_empty() {
        return Ok(GraphView {
            nodes: Vec::new(),
            edges: Vec::new(),
            seeds: Vec::new(),
            truncated: false,
            total_nodes: graph.nodes.len(),
            total_edges: graph.edges.len(),
        });
    }

    Ok(expand(&graph, &seeds, depth, budget_tokens))
}

/// Vizinhança de um nó específico.
pub fn neighbors(
    conn: &Connection,
    project: Option<&str>,
    needle: &str,
    hops: usize,
    budget_tokens: usize,
) -> AppResult<GraphView> {
    let graph = build_graph_cached(conn, project)?;
    let node = resolve_node(&graph, needle)?;
    Ok(expand(&graph, &[node], hops, budget_tokens))
}

/// Caminho mais curto entre dois nós.
pub fn find_path(
    conn: &Connection,
    project: Option<&str>,
    from: &str,
    to: &str,
) -> AppResult<GraphPath> {
    let graph = build_graph_cached(conn, project)?;
    let start = resolve_node(&graph, from)?;
    let end = resolve_node(&graph, to)?;

    let mut queue: VecDeque<&str> = VecDeque::new();
    let mut previous: HashMap<&str, (&str, &str)> = HashMap::new();
    let mut visited: HashSet<&str> = HashSet::new();
    queue.push_back(start.id.as_str());
    visited.insert(start.id.as_str());

    while let Some(current) = queue.pop_front() {
        if current == end.id.as_str() {
            break;
        }
        for edge in graph.edges.iter() {
            let next = if edge.source == current {
                edge.target.as_str()
            } else if edge.target == current {
                edge.source.as_str()
            } else {
                continue;
            };
            if visited.insert(next) {
                previous.insert(next, (current, edge.kind.as_str()));
                queue.push_back(next);
            }
        }
    }

    if !visited.contains(end.id.as_str()) {
        return Err(format!(
            "no path between '{}' and '{}' in this graph",
            start.label, end.label
        )
        .into());
    }

    // Reconstrói de trás para frente.
    let mut chain: Vec<(&str, &str)> = Vec::new(); // (nó, tipo da aresta que leva a ele)
    let mut cursor = end.id.as_str();
    while cursor != start.id.as_str() {
        let Some((previous_id, kind)) = previous.get(cursor).copied() else {
            break;
        };
        chain.push((cursor, kind));
        cursor = previous_id;
    }
    chain.reverse();

    let mut path_nodes: Vec<GraphNode> = Vec::new();
    if let Some(node) = graph.nodes.iter().find(|node| node.id == start.id) {
        path_nodes.push(node.clone());
    }
    let mut path_edges: Vec<GraphEdge> = Vec::new();
    let mut current = start.id.as_str();
    for (node_id, kind) in chain {
        if let Some(node) = graph.nodes.iter().find(|node| node.id == node_id) {
            path_nodes.push(node.clone());
        }
        path_edges.push(GraphEdge {
            source: current.to_string(),
            target: node_id.to_string(),
            kind: kind.to_string(),
        });
        current = node_id;
    }

    Ok(GraphPath {
        from: start.label.clone(),
        to: end.label.clone(),
        hops: path_edges.len(),
        nodes: path_nodes,
        edges: path_edges,
    })
}

/// Detalha um nó e suas conexões diretas.
pub fn explain(
    conn: &Connection,
    project: Option<&str>,
    needle: &str,
    limit: usize,
) -> AppResult<GraphExplain> {
    let graph = build_graph_cached(conn, project)?;
    let node = resolve_node(&graph, needle)?;
    let by_id: HashMap<&str, &GraphNode> = graph
        .nodes
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();

    let mut connections: Vec<GraphConnection> = Vec::new();
    for edge in graph.edges.iter() {
        let (direction, other) = if edge.source == node.id {
            ("out", edge.target.as_str())
        } else if edge.target == node.id {
            ("in", edge.source.as_str())
        } else {
            continue;
        };
        let Some(peer) = by_id.get(other) else {
            continue;
        };
        connections.push(GraphConnection {
            direction: direction.into(),
            kind: edge.kind.clone(),
            id: peer.id.clone(),
            label: peer.label.clone(),
            node_kind: peer.kind.clone(),
            path: peer.path.clone(),
        });
    }

    let degree = connections.len();
    connections.truncate(limit.clamp(1, 200));

    Ok(GraphExplain {
        node: node.clone(),
        degree,
        connections,
    })
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
                    links: vec!["Beta".into(), "Nope".into()],
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
                MemoryRow {
                    file_path: "q/a.md".into(),
                    kind: "task".into(),
                    title: "Task do projeto q".into(),
                    project: "q".into(),
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
                TaskRow {
                    id: "t3".into(),
                    file_path: "q/a.md".into(),
                    title: "Task do projeto q".into(),
                    status: "todo".into(),
                    priority: "medium".into(),
                    due: None,
                    remind: None,
                    tags: vec![],
                    project: "q".into(),
                    // Pai em outro projeto: não pode virar aresta no grafo de "q".
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
        assert!(graph.nodes.iter().any(|node| node.kind == "missing"));

        // Grafo filtrado por projeto não pode ter aresta para nó de fora.
        let filtered = build_graph(&conn, Some("q")).expect("graph q");
        let ids: HashSet<&str> = filtered.nodes.iter().map(|node| node.id.as_str()).collect();
        for edge in &filtered.edges {
            assert!(
                ids.contains(edge.source.as_str()),
                "aresta com origem ausente: {}",
                edge.source
            );
            assert!(
                ids.contains(edge.target.as_str()),
                "aresta com destino ausente: {}",
                edge.target
            );
        }

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn queries_are_scoped_and_budgeted() {
        let db_path = std::env::temp_dir().join(format!(
            "betastart-graph-query-test-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let conn = crate::db::open(&db_path).expect("db");

        crate::db::memory::replace_all(
            &conn,
            &[
                MemoryRow {
                    file_path: "p/alpha.md".into(),
                    kind: "note".into(),
                    title: "Alpha".into(),
                    project: "p".into(),
                    tags: vec!["core".into()],
                    links: vec!["Beta".into()],
                    content: String::new(),
                    updated_at: 0,
                },
                MemoryRow {
                    file_path: "p/beta.md".into(),
                    kind: "note".into(),
                    title: "Beta".into(),
                    project: "p".into(),
                    tags: vec![],
                    links: vec![],
                    content: String::new(),
                    updated_at: 0,
                },
            ],
        )
        .expect("memory");

        let view = query(&conn, Some("p"), "alpha", 1, 500).expect("query");
        assert!(view.nodes.iter().any(|node| node.label == "Alpha"));
        assert!(!view.nodes.is_empty());

        let path = find_path(&conn, Some("p"), "Alpha", "Beta").expect("path");
        assert_eq!(path.hops, 1);
        assert_eq!(path.nodes.len(), 2);

        let explain = explain(&conn, Some("p"), "Alpha", 10).expect("explain");
        assert!(explain.connections.iter().any(|item| item.label == "Beta"));

        // Nome ambíguo/inexistente devolve erro explicativo, não pânico.
        assert!(find_path(&conn, Some("p"), "Alpha", "Nada").is_err());

        let _ = std::fs::remove_file(&db_path);
    }
}
