import { useEffect, useMemo } from "react";
import {
  Background,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  useReactFlow,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import {
  nodeDepth,
  previewText,
  type ScenarioChainNode,
} from "./scenarioChain";

const COL_GAP = 130;
const ROW_GAP = 60;
const NODE_W = 100;

type ChainNodeData = {
  seed: string;
  reply: string | null;
  isRoot: boolean;
  active: boolean;
};

function ChainSeedNode({ data }: NodeProps) {
  const d = data as ChainNodeData;
  return (
    <div
      className={
        d.active
          ? "scenario-flow-node scenario-flow-node--active"
          : "scenario-flow-node"
      }
    >
      <Handle type="target" position={Position.Left} className="scenario-flow-handle" />
      <span className="scenario-flow-node__badge">{d.isRoot ? "root" : "fork"}</span>
      <span className="scenario-flow-node__seed">
        {previewText(d.seed, 36) || "(empty)"}
      </span>
      <span
        className={
          d.reply
            ? "scenario-flow-node__reply"
            : "scenario-flow-node__reply scenario-flow-node__reply--empty"
        }
      >
        {d.reply ? previewText(d.reply, 24) : "no reply"}
      </span>
      <Handle type="source" position={Position.Right} className="scenario-flow-handle" />
    </div>
  );
}

const nodeTypes = { chainSeed: ChainSeedNode };

function toFlowElements(
  chain: ScenarioChainNode[],
  activeNodeId: string | null,
): { nodes: Node[]; edges: Edge[] } {
  const depthBuckets = new Map<number, ScenarioChainNode[]>();
  for (const n of chain) {
    const d = nodeDepth(chain, n.id);
    const bucket = depthBuckets.get(d) ?? [];
    bucket.push(n);
    depthBuckets.set(d, bucket);
  }

  const nodes: Node[] = chain.map((n) => {
    const depth = nodeDepth(chain, n.id);
    const siblings = depthBuckets.get(depth) ?? [n];
    const row = siblings.findIndex((s) => s.id === n.id);
    const rows = siblings.length;
    const y = (row - (rows - 1) / 2) * ROW_GAP;
    return {
      id: n.id,
      type: "chainSeed",
      position: { x: depth * COL_GAP, y },
      data: {
        seed: n.seed,
        reply: n.reply,
        isRoot: n.parentId == null,
        active: n.id === activeNodeId,
      } satisfies ChainNodeData,
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      style: { width: NODE_W },
    };
  });

  const edges: Edge[] = chain
    .filter((n) => n.parentId)
    .map((n) => ({
      id: `e-${n.parentId}-${n.id}`,
      source: n.parentId as string,
      target: n.id,
      type: "smoothstep",
      animated: n.id === activeNodeId,
      markerEnd: { type: MarkerType.ArrowClosed, width: 8, height: 8 },
      style: { strokeWidth: 1 },
    }));

  return { nodes, edges };
}

function FitViewOnChange({ revision }: { revision: string }) {
  const { fitView } = useReactFlow();
  useEffect(() => {
    const t = window.setTimeout(() => {
      void fitView({ padding: 0.2, duration: 200 });
    }, 30);
    return () => window.clearTimeout(t);
  }, [revision, fitView]);
  return null;
}

function AttackChainGraphInner({
  chain,
  activeNodeId,
  onSelect,
}: {
  chain: ScenarioChainNode[];
  activeNodeId: string | null;
  onSelect: (node: ScenarioChainNode) => void;
}) {
  const byId = useMemo(() => new Map(chain.map((n) => [n.id, n])), [chain]);
  const revision = useMemo(
    () => chain.map((n) => `${n.id}:${n.parentId}:${n.reply ? 1 : 0}`).join("|"),
    [chain],
  );
  const initial = useMemo(
    () => toFlowElements(chain, activeNodeId),
    // rebuild when structure/active changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [revision, activeNodeId],
  );

  const [nodes, setNodes, onNodesChange] = useNodesState(initial.nodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initial.edges);

  useEffect(() => {
    setNodes(initial.nodes);
    setEdges(initial.edges);
  }, [initial, setNodes, setEdges]);

  return (
    <div className="scenario-flow">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        nodeTypes={nodeTypes}
        fitView
        nodesDraggable
        nodesConnectable={false}
        elementsSelectable
        panOnScroll
        zoomOnScroll
        minZoom={0.35}
        maxZoom={1.5}
        proOptions={{ hideAttribution: true }}
        onNodeClick={(_, node) => {
          const src = byId.get(node.id);
          if (src) onSelect(src);
        }}
      >
        <Background gap={18} size={1} />
        <Controls showInteractive={false} />
        <FitViewOnChange revision={`${revision}|${activeNodeId ?? ""}`} />
      </ReactFlow>
    </div>
  );
}

export function AttackChainGraph({
  chain,
  activeNodeId,
  onSelect,
}: {
  chain: ScenarioChainNode[];
  activeNodeId: string | null;
  onSelect: (node: ScenarioChainNode) => void;
}) {
  if (chain.length === 0) return null;
  return (
    <ReactFlowProvider>
      <AttackChainGraphInner
        chain={chain}
        activeNodeId={activeNodeId}
        onSelect={onSelect}
      />
    </ReactFlowProvider>
  );
}
