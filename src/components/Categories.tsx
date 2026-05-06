import { useEffect, useState } from "react";
import { useScanStore, type CategorySummary, type Risk } from "../lib/scan";
import { formatBytes } from "../lib/format";
import { CategoryDetail } from "./CategoryDetail";

export function Categories() {
  const categories = useScanStore((s) => s.categories);
  const loading = useScanStore((s) => s.loadingCategories);
  const loadCategories = useScanStore((s) => s.loadCategories);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  useEffect(() => {
    if (categories.length === 0 && !loading) loadCategories();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (loading && categories.length === 0) {
    return <div className="text-sm text-muted p-4">Loading categories…</div>;
  }

  if (categories.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-border p-8 text-center text-sm text-muted">
        No categories yet. Run a scan first.
      </div>
    );
  }

  // Empty categories sink to the bottom.
  const sorted = [...categories].sort((a, b) => b.total_size - a.total_size);

  return (
    <div className="space-y-3">
      {sorted.map((cat) => (
        <CategoryCard
          key={cat.id}
          category={cat}
          expanded={expandedId === cat.id}
          onToggle={() => setExpandedId(expandedId === cat.id ? null : cat.id)}
        />
      ))}
    </div>
  );
}

function CategoryCard({
  category,
  expanded,
  onToggle,
}: {
  category: CategorySummary;
  expanded: boolean;
  onToggle: () => void;
}) {
  const isEmpty = category.item_count === 0;

  return (
    <div className="rounded-lg border border-border overflow-hidden bg-surface">
      <button
        type="button"
        onClick={onToggle}
        disabled={isEmpty}
        className={`w-full px-4 py-3 flex items-center justify-between gap-4 text-left ${
          isEmpty ? "opacity-50 cursor-not-allowed" : "hover:bg-bg/50"
        }`}
      >
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-medium">{category.name}</span>
            <RiskBadge risk={category.risk} />
            <span className="text-xs text-muted">
              {category.item_count.toLocaleString()} item
              {category.item_count === 1 ? "" : "s"}
            </span>
          </div>
          <div className="mt-1 text-xs text-muted">{category.description}</div>
        </div>
        <div className="text-right shrink-0">
          <div className="font-mono text-sm">{formatBytes(category.total_size)}</div>
          {!isEmpty && (
            <div className="text-xs text-muted mt-0.5">{expanded ? "Hide" : "Review →"}</div>
          )}
        </div>
      </button>
      {expanded && !isEmpty && (
        <div className="border-t border-border bg-bg/40">
          <CategoryDetail categoryId={category.id} />
        </div>
      )}
    </div>
  );
}

function RiskBadge({ risk }: { risk: Risk }) {
  const config: Record<Risk, { label: string; cls: string }> = {
    safe: {
      label: "🟢 Safe",
      cls: "bg-success/10 text-success border-success/30",
    },
    "needs-redownload": {
      label: "🟡 Re-downloads",
      cls: "bg-yellow-500/10 text-yellow-600 border-yellow-500/30",
    },
    "user-decides": {
      label: "🔴 You decide",
      cls: "bg-danger/10 text-danger border-danger/30",
    },
  };
  const c = config[risk];
  return (
    <span className={`text-[10px] px-1.5 py-0.5 rounded border font-medium ${c.cls}`}>
      {c.label}
    </span>
  );
}
