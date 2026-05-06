import { useEffect, useRef } from "react";
import { hierarchy, treemap } from "d3-hierarchy";
import { useScanStore } from "../lib/scan";
import { formatBytes } from "../lib/format";

interface Props {
  width?: number;
  height?: number;
}

export function Treemap({ width = 1000, height = 520 }: Props) {
  const treemapData = useScanStore((s) => s.treemap);
  const treemapRoot = useScanStore((s) => s.treemapRoot);
  const loadTreemap = useScanStore((s) => s.loadTreemap);
  const defaultRoots = useScanStore((s) => s.defaultRoots);
  const status = useScanStore((s) => s.status);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (treemapData.length === 0) {
      const target = treemapRoot ?? defaultRoots[0];
      if (target) loadTreemap(target);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // While a scan is running we don't aggregate, so the treemap is empty by
  // design until completion.
  if (status?.status === "running" && treemapData.length === 0) {
    return (
      <div
        className="rounded-lg border border-dashed border-border flex items-center justify-center text-muted text-sm"
        style={{ height }}
      >
        Scanning… treemap renders when scan finishes.
      </div>
    );
  }

  if (treemapData.length === 0) {
    return (
      <div
        className="rounded-lg border border-dashed border-border flex items-center justify-center text-muted text-sm"
        style={{ height }}
      >
        {treemapRoot
          ? `No data for ${treemapRoot}. Run a scan first.`
          : "Run a scan to populate the treemap."}
      </div>
    );
  }

  // Build d3 hierarchy from the flat children list.
  const root = hierarchy<{ name: string; size: number; full_path: string; is_dir: boolean }>({
    name: treemapRoot ?? "/",
    size: 0,
    full_path: treemapRoot ?? "/",
    is_dir: true,
    // @ts-expect-error d3 hierarchy children typing
    children: treemapData.map((n) => ({
      name: n.name,
      size: n.size,
      full_path: n.full_path,
      is_dir: n.is_dir,
    })),
  })
    .sum((d) => d.size)
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

  const layout = treemap<typeof root extends { data: infer D } ? D : never>()
    .size([width, height])
    .padding(1)
    .round(true);
  layout(root as never);

  const leaves = root.leaves();

  const totalSize = treemapData.reduce((acc, n) => acc + n.size, 0);

  return (
    <div ref={containerRef} className="rounded-lg border border-border overflow-hidden">
      <div className="px-4 py-3 bg-surface border-b border-border flex items-center justify-between text-sm">
        <div className="font-mono text-xs text-muted truncate" title={treemapRoot ?? ""}>
          {treemapRoot ?? "/"}
        </div>
        <div className="text-xs text-muted">
          {treemapData.length} entries · {formatBytes(totalSize)}
        </div>
      </div>
      <svg width={width} height={height} className="block">
        {leaves.map((leaf, i) => {
          const x = (leaf as unknown as { x0: number }).x0;
          const y = (leaf as unknown as { y0: number }).y0;
          const x1 = (leaf as unknown as { x1: number }).x1;
          const y1 = (leaf as unknown as { y1: number }).y1;
          const w = x1 - x;
          const h = y1 - y;
          const data = leaf.data as {
            name: string;
            size: number;
            full_path: string;
            is_dir: boolean;
          };
          const hue = (i * 47) % 360;
          const fill = `hsl(${hue}, 60%, 70%)`;
          const showLabel = w > 80 && h > 28;
          return (
            <g
              key={data.full_path}
              transform={`translate(${x},${y})`}
              className="cursor-pointer"
              onClick={() => {
                if (data.is_dir) loadTreemap(data.full_path);
              }}
            >
              <title>
                {data.name} · {formatBytes(data.size)}
              </title>
              <rect
                width={w}
                height={h}
                fill={fill}
                stroke="rgba(0,0,0,0.15)"
                strokeWidth={0.5}
              />
              {showLabel && (
                <>
                  <text
                    x={6}
                    y={16}
                    fontSize={11}
                    fontWeight={500}
                    className="select-none pointer-events-none"
                    fill="#111"
                  >
                    {truncate(data.name, Math.floor((w - 10) / 6))}
                  </text>
                  <text
                    x={6}
                    y={30}
                    fontSize={10}
                    className="select-none pointer-events-none"
                    fill="#222"
                  >
                    {formatBytes(data.size)}
                  </text>
                </>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function truncate(s: string, maxChars: number): string {
  if (s.length <= maxChars) return s;
  if (maxChars <= 1) return "…";
  return s.slice(0, maxChars - 1) + "…";
}
