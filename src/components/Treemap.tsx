import { useEffect, useMemo, useRef, useState } from "react";
import { hierarchy, treemap } from "d3-hierarchy";
import { useScanStore, type DeleteMode } from "../lib/scan";
import { formatBytes } from "../lib/format";
import { Breadcrumbs } from "./Breadcrumbs";
import { DeleteMenu, deletePromptCopy } from "./DeleteMenu";

interface Props {
  width?: number;
  height?: number;
}

// Curated 10-color palette. Same name → same hue on each render thanks to
// the deterministic hash; eliminates the "rainbow vomit" of random hues.
const PALETTE = [
  "#fda4af", // rose
  "#fcd34d", // amber
  "#86efac", // emerald
  "#67e8f9", // cyan
  "#a5b4fc", // indigo
  "#c4b5fd", // violet
  "#f0abfc", // fuchsia
  "#fdba74", // orange
  "#a3e635", // lime
  "#5eead4", // teal
];

function colorForName(name: string): string {
  let h = 0;
  for (let i = 0; i < name.length; i++) {
    h = (h * 31 + name.charCodeAt(i)) >>> 0;
  }
  return PALETTE[h % PALETTE.length] ?? PALETTE[0]!;
}

export function Treemap({ width = 1000, height = 520 }: Props) {
  const treemapData = useScanStore((s) => s.treemap);
  const treemapRoot = useScanStore((s) => s.treemapRoot);
  const loadTreemap = useScanStore((s) => s.loadTreemap);
  const deleteItems = useScanStore((s) => s.deleteItems);
  const defaultRoots = useScanStore((s) => s.defaultRoots);
  const status = useScanStore((s) => s.status);
  const loading = useScanStore((s) => s.loadingTreemap);
  const containerRef = useRef<HTMLDivElement>(null);

  // Treemap selection: paths the user has Cmd/Shift-clicked. Plain click
  // still drills in; the modifier turns the click into a multi-select toggle.
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirm, setConfirm] = useState<DeleteMode | null>(null);
  const [pending, setPending] = useState(false);
  const [lastResult, setLastResult] = useState<string | null>(null);

  // Reset selection when the user navigates to a different path.
  useEffect(() => {
    setSelected(new Set());
    setLastResult(null);
  }, [treemapRoot]);

  useEffect(() => {
    if (treemapData.length === 0) {
      const target = treemapRoot ?? defaultRoots[0];
      if (target) loadTreemap(target);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const selectedSize = useMemo(
    () =>
      treemapData
        .filter((n) => selected.has(n.full_path))
        .reduce((acc, n) => acc + n.size, 0),
    [treemapData, selected],
  );

  const performDelete = async () => {
    if (!confirm) return;
    setPending(true);
    const paths = Array.from(selected);
    const result = await deleteItems(paths, confirm);
    setPending(false);
    setConfirm(null);
    setSelected(new Set());
    setLastResult(
      result.errors.length === 0
        ? `Freed ${formatBytes(result.freed)} (${result.deleted.length} item${
            result.deleted.length === 1 ? "" : "s"
          })`
        : `${result.deleted.length} ok, ${result.errors.length} failed: ${result.errors[0]?.message ?? ""}`,
    );
    // Re-fetch so the just-deleted tiles disappear from the view.
    if (treemapRoot) loadTreemap(treemapRoot);
  };

  const totalSize = treemapData.reduce((acc, n) => acc + n.size, 0);
  // The breadcrumb root is the deepest path that's a prefix of every scan
  // target. With one root this equals the root; with multiple it's their
  // common ancestor (typically "/Users").
  const rootForCrumbs =
    defaultRoots.length === 0
      ? treemapRoot ?? "/"
      : defaultRoots.length === 1
        ? defaultRoots[0]!
        : commonAncestorOf(defaultRoots);

  // While a scan is running we don't aggregate, so the treemap is empty by
  // design until completion.
  if (status?.status === "running" && treemapData.length === 0) {
    return (
      <EmptyShell height={height}>
        Scanning… treemap renders when scan finishes.
      </EmptyShell>
    );
  }

  if (treemapData.length === 0 && !loading) {
    return (
      <EmptyShell height={height}>
        {treemapRoot
          ? `No data for ${treemapRoot}. Run a scan first.`
          : "Run a scan to populate the treemap."}
      </EmptyShell>
    );
  }

  // Build d3 hierarchy from the flat children list.
  const root = hierarchy<{
    name: string;
    size: number;
    full_path: string;
    is_dir: boolean;
  }>({
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

  return (
    <div ref={containerRef} className="rounded-lg border border-border overflow-hidden relative">
      <div className="px-4 py-3 bg-surface border-b border-border flex items-center justify-between gap-4">
        <Breadcrumbs
          path={treemapRoot ?? rootForCrumbs}
          rootPath={rootForCrumbs}
          onNavigate={(p) => loadTreemap(p)}
        />
        {selected.size === 0 ? (
          <div className="text-xs text-muted whitespace-nowrap">
            {treemapData.length} entries · {formatBytes(totalSize)}
            <span className="ml-3 text-muted/60">⌘/⇧-click to select</span>
          </div>
        ) : (
          <div className="flex items-center gap-3 whitespace-nowrap">
            <span className="text-xs">
              <strong>{selected.size}</strong> selected · {formatBytes(selectedSize)}
            </span>
            <button
              type="button"
              onClick={() => setSelected(new Set())}
              className="text-xs text-muted hover:text-fg"
            >
              Clear
            </button>
            <DeleteMenu
              disabled={pending}
              trigger={<>Delete{pending ? "ing…" : "…"}</>}
              onPick={(mode) => setConfirm(mode)}
            />
          </div>
        )}
      </div>
      {confirm &&
        (() => {
          const { question, buttonLabel, buttonClass } = deletePromptCopy(
            confirm,
            selected.size,
            formatBytes(selectedSize),
          );
          return (
            <div className="px-4 py-3 bg-surface/60 border-b border-border flex items-center justify-between gap-3">
              <div className="text-xs">{question}</div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => setConfirm(null)}
                  className="px-3 py-1 text-xs border border-border rounded hover:bg-bg"
                >
                  Cancel
                </button>
                <button
                  type="button"
                  onClick={performDelete}
                  disabled={pending}
                  className={`px-3 py-1 text-xs rounded hover:opacity-90 disabled:opacity-50 ${buttonClass}`}
                >
                  {pending ? "Working…" : buttonLabel}
                </button>
              </div>
            </div>
          );
        })()}
      {lastResult && (
        <div className="px-4 py-2 text-xs text-success bg-success/10 border-b border-success/20">
          {lastResult}
        </div>
      )}
      <div className="relative">
        <svg width={width} height={height} className="block">
          {leaves.map((leaf) => {
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
            const fill = colorForName(data.name);
            const showLabel = w > 80 && h > 28;
            const clickable = data.is_dir;
            const isSelected = selected.has(data.full_path);
            return (
              <g
                key={data.full_path}
                transform={`translate(${x},${y})`}
                className="cursor-pointer hover:opacity-90 transition-opacity"
                onClick={(e) => {
                  if (e.metaKey || e.ctrlKey || e.shiftKey) {
                    // Multi-select toggle.
                    setSelected((prev) => {
                      const next = new Set(prev);
                      if (next.has(data.full_path)) next.delete(data.full_path);
                      else next.add(data.full_path);
                      return next;
                    });
                  } else if (clickable) {
                    loadTreemap(data.full_path);
                  } else {
                    // Plain click on a file: select it (so user can act).
                    setSelected(new Set([data.full_path]));
                  }
                }}
              >
                <title>
                  {data.full_path} · {formatBytes(data.size)}
                  {clickable ? " · click to drill, ⌘-click to select" : " · click to select"}
                </title>
                <rect
                  width={w}
                  height={h}
                  fill={fill}
                  stroke={isSelected ? "#0f0f10" : "rgba(0,0,0,0.15)"}
                  strokeWidth={isSelected ? 3 : 0.5}
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
        {loading && (
          <div className="absolute inset-0 flex items-center justify-center bg-bg/40 backdrop-blur-[1px] text-sm text-muted pointer-events-none">
            Loading…
          </div>
        )}
      </div>
    </div>
  );
}

function EmptyShell({
  height,
  children,
}: {
  height: number;
  children: React.ReactNode;
}) {
  return (
    <div
      className="rounded-lg border border-dashed border-border flex items-center justify-center text-muted text-sm"
      style={{ height }}
    >
      {children}
    </div>
  );
}

function truncate(s: string, maxChars: number): string {
  if (s.length <= maxChars) return s;
  if (maxChars <= 1) return "…";
  return s.slice(0, maxChars - 1) + "…";
}

function commonAncestorOf(paths: string[]): string {
  if (paths.length === 0) return "/";
  const splits = paths.map((p) => p.split("/").filter(Boolean));
  const out: string[] = [];
  const first = splits[0]!;
  for (let i = 0; i < first.length; i++) {
    const seg = first[i];
    if (splits.every((s) => s[i] === seg)) {
      out.push(seg!);
    } else {
      break;
    }
  }
  return out.length === 0 ? "/" : "/" + out.join("/");
}
