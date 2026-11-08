import { useEffect, useMemo, useState } from "react";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { api } from "./api";
import type { Graph as GraphData } from "./types";

interface SimNode extends SimulationNodeDatum {
  id: string;
  kind: string;
  label: string;
  path: string | null;
}

type SimLink = SimulationLinkDatum<SimNode> & { kind: string };

const COLORS: Record<string, string> = {
  project: "#8b5cf6",
  note: "#3b82f6",
  task: "#ec4899",
  tag: "#f59e0b",
};

export function Graph({
  project,
  onOpen,
}: {
  project: string | null;
  onOpen: (path: string) => void;
}) {
  const [data, setData] = useState<GraphData | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    api
      .getGraph(project)
      .then((graph) => {
        if (!cancelled) setData(graph);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [project]);

  const layout = useMemo(() => {
    if (!data) return { nodes: [] as SimNode[], links: [] as SimLink[] };

    const nodes: SimNode[] = data.nodes.map((node) => ({
      id: node.id,
      kind: node.kind,
      label: node.label,
      path: node.path,
    }));
    const links: SimLink[] = data.edges.map((edge) => ({
      source: edge.source,
      target: edge.target,
      kind: edge.kind,
    }));

    const simulation = forceSimulation(nodes)
      .force(
        "link",
        forceLink<SimNode, SimLink>(links)
          .id((node) => node.id)
          .distance(55)
          .strength(0.6),
      )
      .force("charge", forceManyBody().strength(-140))
      .force("center", forceCenter(300, 300))
      .force("collide", forceCollide(14))
      .stop();

    for (let index = 0; index < 300; index += 1) {
      simulation.tick();
    }

    return { nodes, links };
  }, [data]);

  if (error) {
    return <div className="graph-empty">{error}</div>;
  }
  if (!data) {
    return <div className="graph-empty">Carregando grafo…</div>;
  }
  if (data.nodes.length === 0) {
    return (
      <div className="graph-empty">
        Sem dados para exibir.
        <br />
        Crie notas, tasks ou tags.
      </div>
    );
  }

  return (
    <div className="graph-panel">
      <svg
        className="graph-svg"
        viewBox="0 0 600 600"
        preserveAspectRatio="xMidYMid meet"
      >
        {layout.links.map((link, index) => {
          const source = link.source as SimNode;
          const target = link.target as SimNode;
          if (!source || !target) return null;
          return (
            <line
              key={`link-${index}`}
              x1={source.x ?? 0}
              y1={source.y ?? 0}
              x2={target.x ?? 0}
              y2={target.y ?? 0}
              stroke={
                link.kind === "link" ? "#22d3ee" : "rgba(255,255,255,0.14)"
              }
              strokeWidth={link.kind === "link" ? 1.6 : 1}
              strokeDasharray={link.kind === "parent" ? "3 3" : undefined}
            />
          );
        })}
        {layout.nodes.map((node) => (
          <g
            key={node.id}
            transform={`translate(${node.x ?? 0}, ${node.y ?? 0})`}
            className={`graph-node ${node.path ? "clickable" : ""}`}
            onClick={() => {
              if (node.path) onOpen(node.path);
            }}
          >
            <circle
              r={node.kind === "project" ? 9 : node.kind === "tag" ? 5 : 7}
              fill={COLORS[node.kind] ?? "#94a3b8"}
            />
            <text x={10} y={3} fontSize={9} fill="#c9c9d4">
              {node.label}
            </text>
          </g>
        ))}
      </svg>
      <div className="graph-legend">
        <span>
          <i style={{ background: COLORS.project }} /> projeto
        </span>
        <span>
          <i style={{ background: COLORS.note }} /> nota
        </span>
        <span>
          <i style={{ background: COLORS.task }} /> task
        </span>
        <span>
          <i style={{ background: COLORS.tag }} /> tag
        </span>
        <span className="field-hint">
          {data.nodes.length} nós · {data.edges.length} arestas
        </span>
      </div>
    </div>
  );
}
