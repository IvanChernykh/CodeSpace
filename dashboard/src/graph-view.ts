import type { EdgeKind, GraphSnapshot, SymbolKind, SymbolRecord } from "./types.js";
import { clamp, escapeHtml } from "./dom.js";

const KIND_COLORS: Record<SymbolKind, string> = {
  function: "#60a5fa",
  method: "#60a5fa",
  class: "#4ade80",
  struct: "#4ade80",
  interface: "#34d399",
  enum: "#f0b429",
  trait: "#f0b429",
  module: "#94a3b8",
  constant: "#f87171",
  variable: "#f87171",
  type_alias: "#f0b429",
  test: "#a78bfa",
  unknown: "#64748b",
};

const EDGE_COLORS: Partial<Record<EdgeKind, string>> = {
  calls: "#60a5fa",
  imports: "#f0b429",
  contains: "#3a4356",
  "test-covers": "#a78bfa",
  extends: "#4ade80",
  implements: "#4ade80",
  inherits: "#4ade80",
  "depends-on": "#f87171",
};

interface SimNode {
  id: number;
  x: number;
  y: number;
  vx: number;
  vy: number;
  fixed: boolean;
  degree: number;
  record: SymbolRecord;
  group: SVGGElement;
  circle: SVGCircleElement;
  label: SVGTextElement;
}

interface SimEdge {
  source: SimNode;
  target: SimNode;
  kind: EdgeKind;
  line: SVGLineElement;
}

export interface GraphViewOptions {
  onSelectNode: (id: number | null) => void;
  onHoverNode?: (id: number | null) => void;
  maxNodes?: number;
}

const SVG_NS = "http://www.w3.org/2000/svg";

export class GraphView {
  private readonly svg: SVGSVGElement;
  private readonly viewport: SVGGElement;
  private readonly edgeLayer: SVGGElement;
  private readonly nodeLayer: SVGGElement;
  private readonly options: GraphViewOptions;
  private readonly maxNodes: number;

  private nodes: SimNode[] = [];
  private edges: SimEdge[] = [];
  private nodeById = new Map<number, SimNode>();
  private hiddenKinds = new Set<EdgeKind>();

  private scale = 1;
  private tx = 0;
  private ty = 0;
  private selectedId: number | null = null;

  private dragNode: SimNode | null = null;
  private panActive = false;
  private panStart = { x: 0, y: 0, tx: 0, ty: 0 };
  private rafHandle: number | null = null;
  private settleTicks = 0;

  constructor(svg: SVGSVGElement, options: GraphViewOptions) {
    this.svg = svg;
    this.options = options;
    this.maxNodes = options.maxNodes ?? 160;

    this.viewport = document.createElementNS(SVG_NS, "g");
    this.viewport.setAttribute("class", "graph-viewport");
    this.edgeLayer = document.createElementNS(SVG_NS, "g");
    this.edgeLayer.setAttribute("class", "graph-edges");
    this.nodeLayer = document.createElementNS(SVG_NS, "g");
    this.nodeLayer.setAttribute("class", "graph-nodes");
    this.viewport.append(this.edgeLayer, this.nodeLayer);
    this.svg.append(this.viewport);

    this.svg.addEventListener("wheel", this.handleWheel, { passive: false });
    this.svg.addEventListener("pointerdown", this.handlePointerDown);
    window.addEventListener("pointermove", this.handlePointerMove);
    window.addEventListener("pointerup", this.handlePointerUp);
  }

  destroy(): void {
    this.stopSimulation();
    this.svg.removeEventListener("wheel", this.handleWheel);
    this.svg.removeEventListener("pointerdown", this.handlePointerDown);
    window.removeEventListener("pointermove", this.handlePointerMove);
    window.removeEventListener("pointerup", this.handlePointerUp);
    this.viewport.remove();
  }

  setEdgeFilter(kind: EdgeKind, visible: boolean): void {
    if (visible) this.hiddenKinds.delete(kind);
    else this.hiddenKinds.add(kind);
    for (const edge of this.edges) {
      edge.line.style.display = this.hiddenKinds.has(edge.kind) ? "none" : "";
    }
  }

  selectNode(id: number | null): void {
    this.selectedId = id;
    for (const node of this.nodes) {
      node.group.classList.toggle("is-selected", node.id === id);
    }
  }

  focus(id: number): void {
    const node = this.nodeById.get(id);
    if (!node) return;
    const rect = this.svg.getBoundingClientRect();
    this.scale = clamp(this.scale, 0.4, 2.2);
    this.tx = rect.width / 2 - node.x * this.scale;
    this.ty = rect.height / 2 - node.y * this.scale;
    this.applyTransform();
  }

  setGraph(graph: GraphSnapshot): void {
    this.stopSimulation();
    this.edgeLayer.replaceChildren();
    this.nodeLayer.replaceChildren();
    this.nodeById.clear();

    const rect = this.svg.getBoundingClientRect();
    const width = rect.width || 800;
    const height = rect.height || 600;

    const degree = new Map<number, number>();
    for (const edge of graph.edges) {
      degree.set(edge.from, (degree.get(edge.from) ?? 0) + 1);
      degree.set(edge.to, (degree.get(edge.to) ?? 0) + 1);
    }

    const ranked = [...graph.symbols].sort(
      (a, b) => (degree.get(b.id) ?? 0) - (degree.get(a.id) ?? 0),
    );
    const chosen = ranked.slice(0, this.maxNodes);
    const chosenIds = new Set(chosen.map((s) => s.id));

    this.nodes = chosen.map((record, index) => {
      const angle = (index / Math.max(chosen.length, 1)) * Math.PI * 2;
      const radius = Math.min(width, height) * 0.32;
      const x = width / 2 + Math.cos(angle) * radius + (Math.random() - 0.5) * 20;
      const y = height / 2 + Math.sin(angle) * radius + (Math.random() - 0.5) * 20;
      return this.buildNode(record, x, y, degree.get(record.id) ?? 0);
    });
    for (const node of this.nodes) this.nodeById.set(node.id, node);

    this.edges = [];
    for (const edge of graph.edges) {
      if (!chosenIds.has(edge.from) || !chosenIds.has(edge.to)) continue;
      const source = this.nodeById.get(edge.from);
      const target = this.nodeById.get(edge.to);
      if (!source || !target) continue;
      this.edges.push(this.buildEdge(source, target, edge.kind));
    }

    this.settleTicks = 0;
    this.startSimulation();
  }

  private buildNode(record: SymbolRecord, x: number, y: number, degree: number): SimNode {
    const group = document.createElementNS(SVG_NS, "g");
    group.setAttribute("class", "graph-node");
    group.dataset.id = String(record.id);

    const radius = clamp(4 + Math.sqrt(degree) * 2, 5, 16);
    const circle = document.createElementNS(SVG_NS, "circle");
    circle.setAttribute("r", String(radius));
    circle.setAttribute("fill", KIND_COLORS[record.kind] ?? KIND_COLORS.unknown);

    const label = document.createElementNS(SVG_NS, "text");
    label.setAttribute("x", String(radius + 6));
    label.setAttribute("y", "4");
    label.textContent = record.name;

    group.append(circle, label);
    group.addEventListener("pointerdown", (ev) => {
      ev.stopPropagation();
      this.dragNode = node;
      node.fixed = true;
    });
    group.addEventListener("click", (ev) => {
      ev.stopPropagation();
      this.options.onSelectNode(record.id);
    });
    group.addEventListener("pointerenter", () => this.options.onHoverNode?.(record.id));
    group.addEventListener("pointerleave", () => this.options.onHoverNode?.(null));
    group.setAttribute("aria-label", record.qualified_name);
    const titleEl = document.createElementNS(SVG_NS, "title");
    titleEl.textContent = escapeHtml(record.qualified_name);
    group.append(titleEl);

    this.nodeLayer.append(group);
    const node: SimNode = {
      id: record.id,
      x,
      y,
      vx: 0,
      vy: 0,
      fixed: false,
      degree,
      record,
      group,
      circle,
      label,
    };
    return node;
  }

  private buildEdge(source: SimNode, target: SimNode, kind: EdgeKind): SimEdge {
    const line = document.createElementNS(SVG_NS, "line");
    line.setAttribute("class", `graph-edge edge-${kind}`);
    line.setAttribute("stroke", EDGE_COLORS[kind] ?? "#2a3242");
    if (this.hiddenKinds.has(kind)) line.style.display = "none";
    this.edgeLayer.append(line);
    return { source, target, kind, line };
  }

  private startSimulation(): void {
    this.stopSimulation();
    const step = () => {
      const moving = this.tick();
      this.render();
      if (moving) {
        this.rafHandle = requestAnimationFrame(step);
      } else {
        this.rafHandle = null;
      }
    };
    this.rafHandle = requestAnimationFrame(step);
  }

  private stopSimulation(): void {
    if (this.rafHandle !== null) {
      cancelAnimationFrame(this.rafHandle);
      this.rafHandle = null;
    }
  }

  /** One physics step: repulsion + spring edges + weak centering + damping. Returns true while still settling. */
  private tick(): boolean {
    const rect = this.svg.getBoundingClientRect();
    const width = rect.width || 800;
    const height = rect.height || 600;
    const centerX = width / 2;
    const centerY = height / 2;
    const n = this.nodes.length;
    if (n === 0) return false;

    for (let i = 0; i < n; i += 1) {
      const a = this.nodes[i];
      if (!a) continue;
      let fx = (centerX - a.x) * 0.002;
      let fy = (centerY - a.y) * 0.002;
      for (let j = 0; j < n; j += 1) {
        if (i === j) continue;
        const b = this.nodes[j];
        if (!b) continue;
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        const distSq = Math.max(dx * dx + dy * dy, 25);
        const force = 900 / distSq;
        fx += (dx / Math.sqrt(distSq)) * force;
        fy += (dy / Math.sqrt(distSq)) * force;
      }
      a.vx = (a.vx + fx) * 0.82;
      a.vy = (a.vy + fy) * 0.82;
    }

    for (const edge of this.edges) {
      const dx = edge.target.x - edge.source.x;
      const dy = edge.target.y - edge.source.y;
      const dist = Math.max(Math.sqrt(dx * dx + dy * dy), 1);
      const target = edge.kind === "contains" ? 60 : 110;
      const stretch = (dist - target) * 0.02;
      const ux = dx / dist;
      const uy = dy / dist;
      if (!edge.source.fixed) {
        edge.source.vx += ux * stretch;
        edge.source.vy += uy * stretch;
      }
      if (!edge.target.fixed) {
        edge.target.vx -= ux * stretch;
        edge.target.vy -= uy * stretch;
      }
    }

    let maxSpeed = 0;
    for (const node of this.nodes) {
      if (node.fixed) {
        node.vx = 0;
        node.vy = 0;
        continue;
      }
      node.x += clamp(node.vx, -12, 12);
      node.y += clamp(node.vy, -12, 12);
      maxSpeed = Math.max(maxSpeed, Math.abs(node.vx) + Math.abs(node.vy));
    }

    if (maxSpeed < 0.6) {
      this.settleTicks += 1;
    } else {
      this.settleTicks = 0;
    }
    return this.settleTicks < 20;
  }

  private render(): void {
    for (const node of this.nodes) {
      node.group.setAttribute("transform", `translate(${node.x.toFixed(1)},${node.y.toFixed(1)})`);
    }
    for (const edge of this.edges) {
      edge.line.setAttribute("x1", edge.source.x.toFixed(1));
      edge.line.setAttribute("y1", edge.source.y.toFixed(1));
      edge.line.setAttribute("x2", edge.target.x.toFixed(1));
      edge.line.setAttribute("y2", edge.target.y.toFixed(1));
    }
  }

  private applyTransform(): void {
    this.viewport.setAttribute("transform", `translate(${this.tx},${this.ty}) scale(${this.scale})`);
  }

  private toGraphCoords(clientX: number, clientY: number): { x: number; y: number } {
    const rect = this.svg.getBoundingClientRect();
    return {
      x: (clientX - rect.left - this.tx) / this.scale,
      y: (clientY - rect.top - this.ty) / this.scale,
    };
  }

  private handleWheel = (ev: WheelEvent): void => {
    ev.preventDefault();
    const rect = this.svg.getBoundingClientRect();
    const cursorX = ev.clientX - rect.left;
    const cursorY = ev.clientY - rect.top;
    const before = this.scale;
    const next = clamp(this.scale * (ev.deltaY > 0 ? 0.9 : 1.1), 0.2, 3);
    const worldX = (cursorX - this.tx) / before;
    const worldY = (cursorY - this.ty) / before;
    this.scale = next;
    this.tx = cursorX - worldX * next;
    this.ty = cursorY - worldY * next;
    this.applyTransform();
  };

  private handlePointerDown = (ev: PointerEvent): void => {
    if (this.dragNode) return;
    this.panActive = true;
    this.panStart = { x: ev.clientX, y: ev.clientY, tx: this.tx, ty: this.ty };
    this.options.onSelectNode(null);
  };

  private handlePointerMove = (ev: PointerEvent): void => {
    if (this.dragNode) {
      const { x, y } = this.toGraphCoords(ev.clientX, ev.clientY);
      this.dragNode.x = x;
      this.dragNode.y = y;
      this.render();
      this.wakeSimulation();
      return;
    }
    if (this.panActive) {
      this.tx = this.panStart.tx + (ev.clientX - this.panStart.x);
      this.ty = this.panStart.ty + (ev.clientY - this.panStart.y);
      this.applyTransform();
    }
  };

  private handlePointerUp = (): void => {
    if (this.dragNode) {
      this.dragNode.fixed = false;
      this.dragNode = null;
      this.wakeSimulation();
    }
    this.panActive = false;
  };

  private wakeSimulation(): void {
    this.settleTicks = 0;
    if (this.rafHandle === null) {
      this.startSimulation();
    }
  }
}
