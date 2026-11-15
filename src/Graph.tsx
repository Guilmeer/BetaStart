import {
  useCallback,
  useEffect,
  useReducer,
  useRef,
  useState,
} from "react";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { api } from "./api";
import { useI18n } from "./i18n";
import type { Graph as GraphData } from "./types";

interface SimNode extends SimulationNodeDatum {
  id: string;
  kind: string;
  label: string;
  path: string | null;
  degree: number;
}

type SimLink = SimulationLinkDatum<SimNode> & { kind: string };

interface View {
  x: number;
  y: number;
  k: number;
}

const MIN_ZOOM = 0.25;
const MAX_ZOOM = 4;
const TICK_INTERVAL = 33; // ~30 fps

const COLORS: Record<string, string> = {
  project: "var(--accent)",
  note: "var(--info)",
  task: "var(--ok)",
  tag: "var(--graph-tag)",
  missing: "var(--danger)",
};

function radiusOf(node: SimNode): number {
  if (node.kind === "project") return 10 + Math.min(8, node.degree * 0.7);
  if (node.kind === "tag" || node.kind === "missing") {
    return 5 + Math.min(4, node.degree * 0.5);
  }
  return 7 + Math.min(6, node.degree * 0.6);
}

function clampZoom(value: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value));
}

export function Graph({
  project,
  version,
  onOpen,
}: {
  project: string | null;
  version: number;
  onOpen: (path: string) => void;
}) {
  const { t } = useI18n();
  const [data, setData] = useState<GraphData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [size, setSize] = useState({ width: 640, height: 520 });
  const [view, setView] = useState<View>({ x: 0, y: 0, k: 1 });
  const [hovered, setHovered] = useState<string | null>(null);
  const [, bump] = useReducer((value: number) => value + 1, 0);

  const canvasRef = useRef<HTMLDivElement | null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const nodesRef = useRef<SimNode[]>([]);
  const linksRef = useRef<SimLink[]>([]);
  const simRef = useRef<Simulation<SimNode, SimLink> | null>(null);
  const viewRef = useRef(view);
  const sizeRef = useRef(size);
  const lastTickRef = useRef(0);
  const lastProjectRef = useRef<string | null>(project);
  const interactedRef = useRef(false);
  const dragRef = useRef<{
    node: SimNode;
    startX: number;
    startY: number;
    moved: boolean;
  } | null>(null);
  const panRef = useRef<{
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);

  useEffect(() => {
    viewRef.current = view;
  }, [view]);

  useEffect(() => {
    sizeRef.current = size;
  }, [size]);

  // Dados do grafo (recarrega quando o projeto ou o índice mudam).
  useEffect(() => {
    let cancelled = false;
    setError(null);
    api
      .getGraph(project)
      .then((graph) => {
        if (cancelled) return;
        setData(graph);
        // Trocar de projeto reenquadra; um refresh do índice preserva o zoom.
        if (lastProjectRef.current !== project) {
          lastProjectRef.current = project;
          setView({ x: 0, y: 0, k: 1 });
        }
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [project, version]);

  // Tamanho real do painel -> viewBox em pixels (coordenadas 1:1).
  // Depende de `data`: o canvas só existe depois que o grafo carrega.
  useEffect(() => {
    if (!data) return;
    const element = canvasRef.current;
    if (!element) return;
    const measure = () => {
      const rect = element.getBoundingClientRect();
      setSize({
        width: Math.max(240, Math.round(rect.width)),
        height: Math.max(200, Math.round(rect.height)),
      });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [data]);

  // Enquadra todos os nós no espaço visível.
  const fitView = useCallback(() => {
    const nodes = nodesRef.current;
    if (!nodes.length) return;
    const { width, height } = sizeRef.current;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const node of nodes) {
      const radius = radiusOf(node);
      const x = node.x ?? 0;
      const y = node.y ?? 0;
      minX = Math.min(minX, x - radius);
      maxX = Math.max(maxX, x + radius);
      minY = Math.min(minY, y - radius);
      maxY = Math.max(maxY, y + radius);
    }
    const padding = 44;
    const spanX = Math.max(1, maxX - minX);
    const spanY = Math.max(1, maxY - minY);
    const k = clampZoom(
      Math.min(
        (width - padding * 2) / spanX,
        (height - padding * 2) / spanY,
        1.5,
      ),
    );
    const centerX = (minX + maxX) / 2;
    const centerY = (minY + maxY) / 2;
    setView({
      x: width / 2 - centerX * k,
      y: height / 2 - centerY * k,
      k,
    });
  }, []);

  // Simulação: roda animada, mas só re-renderiza ~30x por segundo.
  useEffect(() => {
    if (!data) return;

    const previous = new Map(nodesRef.current.map((node) => [node.id, node]));
    const known = new Set(data.nodes.map((node) => node.id));
    const degree = new Map<string, number>();
    for (const edge of data.edges) {
      if (!known.has(edge.source) || !known.has(edge.target)) continue;
      degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
      degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
    }

    const nodes: SimNode[] = data.nodes.map((node) => {
      const base = {
        id: node.id,
        kind: node.kind,
        label: node.label,
        path: node.path,
        degree: degree.get(node.id) ?? 0,
      };
      const old = previous.get(node.id);
      return old ? Object.assign(old, base) : { ...base };
    });

    // Defesa extra: nunca passar aresta para o d3-force sem os dois nós.
    const links: SimLink[] = data.edges
      .filter((edge) => known.has(edge.source) && known.has(edge.target))
      .map((edge) => ({
        source: edge.source,
        target: edge.target,
        kind: edge.kind,
      }));

    nodesRef.current = nodes;
    linksRef.current = links;
    bump();

    const tick = () => {
      const now = performance.now();
      if (now - lastTickRef.current < TICK_INTERVAL) return;
      lastTickRef.current = now;
      bump();
    };

    const { width, height } = sizeRef.current;
    const reused = nodes.some((node) => previous.has(node.id));
    const simulation = forceSimulation<SimNode>(nodes)
      .force(
        "link",
        forceLink<SimNode, SimLink>(links)
          .id((node) => node.id)
          .distance((link) => {
            const source = link.source as SimNode;
            const target = link.target as SimNode;
            return 44 + Math.min(46, (source.degree + target.degree) * 3);
          })
          .strength(0.45),
      )
      .force("charge", forceManyBody<SimNode>().strength(-220).distanceMax(520))
      .force(
        "center",
        forceCenter(width / 2, height / 2).strength(0.08),
      )
      .force(
        "collide",
        forceCollide<SimNode>((node) => radiusOf(node) + 4).strength(0.9),
      )
      .alpha(reused ? 0.35 : 1)
      .alphaDecay(0.025)
      .on("tick", tick)
      .on("end", bump);

    simRef.current = simulation;
    if (!reused) {
      // Aquece o layout e enquadra; depois a animação só assenta.
      simulation.tick(80);
      interactedRef.current = false;
      fitView();
    }

    return () => {
      simulation.on("tick", null);
      simulation.on("end", null);
      simulation.stop();
      if (simRef.current === simulation) simRef.current = null;
    };
  }, [data, bump, fitView]);

  // Redimensionou: recentraliza e, se o usuário ainda não mexeu no zoom,
  // reenquadra para o painel novo.
  useEffect(() => {
    const simulation = simRef.current;
    if (!simulation) return;
    simulation.force(
      "center",
      forceCenter(size.width / 2, size.height / 2).strength(0.08),
    );
    simulation.alpha(0.25).restart();
    if (!interactedRef.current) fitView();
  }, [size, fitView]);

  const clientToView = useCallback((clientX: number, clientY: number) => {
    const svg = svgRef.current;
    if (!svg) return { x: 0, y: 0 };
    const point = svg.createSVGPoint();
    point.x = clientX;
    point.y = clientY;
    const matrix = svg.getScreenCTM();
    if (!matrix) return { x: 0, y: 0 };
    const mapped = point.matrixTransform(matrix.inverse());
    return { x: mapped.x, y: mapped.y };
  }, []);

  const viewToGraph = useCallback((x: number, y: number) => {
    const current = viewRef.current;
    return { x: (x - current.x) / current.k, y: (y - current.y) / current.k };
  }, []);

  // Zoom com a roda, mantendo o ponto sob o cursor.
  // Também depende de `data`: o <svg> só é montado depois do carregamento.
  useEffect(() => {
    if (!data) return;
    const svg = svgRef.current;
    if (!svg) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      interactedRef.current = true;
      const current = viewRef.current;
      const point = clientToView(event.clientX, event.clientY);
      const factor = Math.exp(-event.deltaY * 0.0015);
      const k = clampZoom(current.k * factor);
      if (k === current.k) return;
      setView({
        x: point.x - (point.x - current.x) * (k / current.k),
        y: point.y - (point.y - current.y) * (k / current.k),
        k,
      });
    };
    svg.addEventListener("wheel", onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", onWheel);
  }, [clientToView, data]);

  const zoomBy = (factor: number) => {
    interactedRef.current = true;
    const current = viewRef.current;
    const k = clampZoom(current.k * factor);
    const centerX = sizeRef.current.width / 2;
    const centerY = sizeRef.current.height / 2;
    setView({
      x: centerX - (centerX - current.x) * (k / current.k),
      y: centerY - (centerY - current.y) * (k / current.k),
      k,
    });
  };

  const startNodeDrag = (
    event: React.PointerEvent<SVGGElement>,
    node: SimNode,
  ) => {
    event.stopPropagation();
    event.preventDefault();
    interactedRef.current = true;
    dragRef.current = {
      node,
      startX: event.clientX,
      startY: event.clientY,
      moved: false,
    };
    node.fx = node.x;
    node.fy = node.y;
    simRef.current?.alphaTarget(0.12).restart();
    svgRef.current?.setPointerCapture(event.pointerId);
  };

  const handlePointerDown = (event: React.PointerEvent<SVGSVGElement>) => {
    if (event.button !== 0) return;
    interactedRef.current = true;
    const point = clientToView(event.clientX, event.clientY);
    panRef.current = {
      startX: point.x,
      startY: point.y,
      originX: viewRef.current.x,
      originY: viewRef.current.y,
    };
    svgRef.current?.setPointerCapture(event.pointerId);
  };

  const handlePointerMove = (event: React.PointerEvent<SVGSVGElement>) => {
    const drag = dragRef.current;
    if (drag) {
      if (
        Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) > 3
      ) {
        drag.moved = true;
      }
      const point = clientToView(event.clientX, event.clientY);
      const target = viewToGraph(point.x, point.y);
      drag.node.fx = target.x;
      drag.node.fy = target.y;
      bump();
      return;
    }

    const pan = panRef.current;
    if (!pan) return;
    const point = clientToView(event.clientX, event.clientY);
    setView({
      x: pan.originX + point.x - pan.startX,
      y: pan.originY + point.y - pan.startY,
      k: viewRef.current.k,
    });
  };

  const handlePointerUp = (event: React.PointerEvent<SVGSVGElement>) => {
    const drag = dragRef.current;
    if (drag) {
      drag.node.fx = null;
      drag.node.fy = null;
      simRef.current?.alphaTarget(0);
      if (!drag.moved && drag.node.path) onOpen(drag.node.path);
      dragRef.current = null;
    }
    panRef.current = null;
    if (svgRef.current?.hasPointerCapture(event.pointerId)) {
      svgRef.current.releasePointerCapture(event.pointerId);
    }
  };

  const cancelPointer = (event: React.PointerEvent<SVGSVGElement>) => {
    const drag = dragRef.current;
    if (drag) {
      drag.node.fx = null;
      drag.node.fy = null;
      simRef.current?.alphaTarget(0);
      dragRef.current = null;
    }
    panRef.current = null;
    if (svgRef.current?.hasPointerCapture(event.pointerId)) {
      svgRef.current.releasePointerCapture(event.pointerId);
    }
  };

  if (error) {
    return <div className="graph-empty">{error}</div>;
  }
  if (!data) {
    return <div className="graph-empty">{t("Loading graph…")}</div>;
  }
  if (data.nodes.length === 0) {
    return (
      <div className="graph-empty">
        {t("No data to display.")}
        <br />
        {t("Create notes, tasks, or tags.")}
      </div>
    );
  }

  const nodes = nodesRef.current;
  const links = linksRef.current;
  const showLabels = view.k >= 0.8;

  return (
    <div className="graph-panel">
      <div className="graph-toolbar">
        <button className="graph-tool" onClick={() => zoomBy(1.25)} title="Zoom +">
          +
        </button>
        <button className="graph-tool" onClick={() => zoomBy(0.8)} title="Zoom −">
          −
        </button>
        <button
          className="graph-tool wide"
          onClick={() => {
            interactedRef.current = false;
            fitView();
          }}
          title={t("Show the whole graph")}
        >
          {t("fit")}
        </button>
        <span className="graph-zoom">{Math.round(view.k * 100)}%</span>
      </div>

      <div className="graph-canvas" ref={canvasRef}>
        <svg
          ref={svgRef}
          className="graph-svg"
          viewBox={`0 0 ${size.width} ${size.height}`}
          preserveAspectRatio="xMidYMid meet"
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onPointerCancel={cancelPointer}
        >
          <g transform={`translate(${view.x} ${view.y}) scale(${view.k})`}>
            {links.map((link, index) => {
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
                    link.kind === "link"
                      ? "var(--info)"
                      : "var(--overlay-14)"
                  }
                  strokeWidth={
                    (link.kind === "link" ? 1.5 : 1) / view.k
                  }
                  strokeDasharray={
                    link.kind === "parent"
                      ? `${4 / view.k} ${3 / view.k}`
                      : undefined
                  }
                />
              );
            })}

            {nodes.map((node) => {
              const radius = radiusOf(node);
              const active = hovered === node.id;
              const visible =
                showLabels || active || node.kind === "project";
              const label =
                node.label.length > 28
                  ? `${node.label.slice(0, 27)}…`
                  : node.label;
              return (
                <g
                  key={node.id}
                  transform={`translate(${node.x ?? 0} ${node.y ?? 0})`}
                  className={`graph-node ${node.path ? "clickable" : ""}`}
                  onPointerDown={(event) => startNodeDrag(event, node)}
                  onPointerEnter={() => setHovered(node.id)}
                  onPointerLeave={() =>
                    setHovered((value) => (value === node.id ? null : value))
                  }
                >
                  <circle
                    r={radius}
                    fill={COLORS[node.kind] ?? "var(--text-3)"}
                    stroke={
                      node.kind === "missing"
                        ? "var(--danger)"
                        : active
                          ? "var(--text-strong)"
                          : "var(--graph-node-stroke)"
                    }
                    strokeWidth={(active ? 1.6 : 0.8) / view.k}
                  />
                  {visible && (
                    <text
                      x={radius + 6}
                      y={4}
                      className="graph-node-label"
                      // Fonte em unidades do grafo: o zoom aumenta o texto.
                      fontSize={active ? 13 : 12}
                      fontWeight={active || node.kind === "project" ? 600 : 400}
                      strokeWidth={3}
                      paintOrder="stroke"
                      pointerEvents="none"
                    >
                      {label}
                    </text>
                  )}
                </g>
              );
            })}
          </g>
        </svg>
      </div>

      <div className="graph-legend">
        <span>
          <i style={{ background: COLORS.project }} /> {t("project")}
        </span>
        <span>
          <i style={{ background: COLORS.note }} /> {t("note")}
        </span>
        <span>
          <i style={{ background: COLORS.task }} /> {t("task")}
        </span>
        <span>
          <i style={{ background: COLORS.tag }} /> {t("tag")}
        </span>
        <span>
          <i style={{ background: COLORS.missing }} /> {t("unresolved")}
        </span>
        <span className="field-hint">
          {t("{nodes} nodes · {edges} edges", {
            nodes: data.nodes.length,
            edges: data.edges.length,
          })}
        </span>
      </div>
      <div className="graph-hint">
        {t("drag the background to move · scroll to zoom · drag a node to rearrange · click to open")}
      </div>
    </div>
  );
}
