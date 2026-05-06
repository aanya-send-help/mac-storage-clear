import { useEffect, useMemo, useState } from "react";
import { useScanStore, type DeleteMode } from "../lib/scan";
import { formatBytes, formatRelativeTime } from "../lib/format";
import { DeleteMenu, deletePromptCopy } from "./DeleteMenu";

export function CategoryDetail({ categoryId }: { categoryId: string }) {
  const items = useScanStore((s) => s.categoryItems[categoryId] ?? []);
  const loading = useScanStore((s) => s.loadingCategoryItems[categoryId] ?? false);
  const loadItems = useScanStore((s) => s.loadCategoryItems);
  const startDelete = useScanStore((s) => s.startDelete);
  const isDeleting = useScanStore((s) => s.deleteStatus?.status === "running");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirm, setConfirm] = useState<DeleteMode | null>(null);

  useEffect(() => {
    if (items.length === 0 && !loading) loadItems(categoryId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [categoryId]);

  const allSelected = items.length > 0 && selected.size === items.length;
  const selectedSize = useMemo(
    () => items.filter((i) => selected.has(i.path)).reduce((acc, i) => acc + i.size, 0),
    [items, selected],
  );

  const toggle = (path: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const toggleAll = () => {
    if (allSelected) setSelected(new Set());
    else setSelected(new Set(items.map((i) => i.path)));
  };

  const requestDelete = (mode: DeleteMode) => {
    if (selected.size === 0) return;
    setConfirm(mode);
  };

  const performDelete = async () => {
    if (!confirm) return;
    const paths = Array.from(selected);
    await startDelete(paths, confirm);
    setConfirm(null);
    setSelected(new Set());
  };

  if (loading && items.length === 0) {
    return <div className="px-4 py-6 text-sm text-muted">Loading items…</div>;
  }

  if (items.length === 0) {
    return <div className="px-4 py-6 text-sm text-muted">No items.</div>;
  }

  return (
    <div className="divide-y divide-border">
      <div className="px-4 py-2 flex items-center justify-between gap-3 text-xs">
        <label className="flex items-center gap-2 cursor-pointer select-none">
          <input
            type="checkbox"
            checked={allSelected}
            onChange={toggleAll}
            className="accent-accent"
          />
          <span className="text-muted">
            {selected.size === 0
              ? `Select to delete · ${items.length} items`
              : `${selected.size} selected · ${formatBytes(selectedSize)}`}
          </span>
        </label>
        <DeleteMenu
          disabled={selected.size === 0 || isDeleting}
          trigger={<>Delete…</>}
          onPick={(mode) => requestDelete(mode)}
        />
      </div>

      {confirm &&
        (() => {
          const { question, buttonLabel, buttonClass } = deletePromptCopy(
            confirm,
            selected.size,
            formatBytes(selectedSize),
          );
          return (
            <div className="px-4 py-3 bg-surface/60 border-y border-border flex items-center justify-between gap-3">
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
                  disabled={isDeleting}
                  className={`px-3 py-1 text-xs rounded hover:opacity-90 disabled:opacity-50 ${buttonClass}`}
                >
                  {isDeleting ? "Working…" : buttonLabel}
                </button>
              </div>
            </div>
          );
        })()}

      <div className="max-h-[480px] overflow-y-auto">
        <table className="w-full text-sm">
          <tbody>
            {items.map((item) => {
              const checked = selected.has(item.path);
              return (
                <tr
                  key={item.path}
                  onClick={() => toggle(item.path)}
                  className="border-t border-border cursor-pointer hover:bg-surface/50"
                >
                  <td className="px-4 py-2 w-8">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggle(item.path)}
                      onClick={(e) => e.stopPropagation()}
                      className="accent-accent"
                    />
                  </td>
                  <td className="px-2 py-2 truncate max-w-2xl font-mono text-xs" title={item.path}>
                    {pathTail(item.path)}
                  </td>
                  <td className="px-4 py-2 text-right font-mono text-xs w-24">
                    {formatBytes(item.size)}
                  </td>
                  <td className="px-4 py-2 text-right text-muted text-xs w-28">
                    {item.mtime ? formatRelativeTime(item.mtime * 1000) : "—"}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function pathTail(p: string): string {
  const segs = p.split("/");
  if (segs.length <= 4) return p;
  return ".../" + segs.slice(-3).join("/");
}
