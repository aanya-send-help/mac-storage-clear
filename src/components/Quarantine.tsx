import { useEffect, useMemo, useState } from "react";
import { useScanStore } from "../lib/scan";
import { formatBytes, formatRelativeTime } from "../lib/format";

export function Quarantine() {
  const entries = useScanStore((s) => s.quarantine);
  const loading = useScanStore((s) => s.loadingQuarantine);
  const loadQuarantine = useScanStore((s) => s.loadQuarantine);
  const restore = useScanStore((s) => s.restoreFromQuarantine);
  const emptyQ = useScanStore((s) => s.emptyQuarantine);

  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [pending, setPending] = useState(false);
  const [lastResult, setLastResult] = useState<string | null>(null);

  useEffect(() => {
    if (entries.length === 0 && !loading) loadQuarantine();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const total = useMemo(() => entries.reduce((acc, e) => acc + e.size, 0), [entries]);
  const allSelected = entries.length > 0 && selected.size === entries.length;
  const selectedSize = useMemo(
    () => entries.filter((e) => selected.has(e.id)).reduce((acc, e) => acc + e.size, 0),
    [entries, selected],
  );

  const toggle = (id: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleRestore = async () => {
    if (selected.size === 0) return;
    setPending(true);
    const result = await restore(Array.from(selected));
    setPending(false);
    setSelected(new Set());
    setLastResult(
      result.errors.length === 0
        ? `Restored ${result.deleted.length} item${result.deleted.length === 1 ? "" : "s"}`
        : `${result.deleted.length} ok, ${result.errors.length} failed`,
    );
  };

  const handleEmpty = async (olderThanDays?: number) => {
    if (!confirm(olderThanDays ? `Permanently delete quarantined items older than ${olderThanDays} days?` : `Permanently delete all ${entries.length} quarantined items? This can't be undone.`)) {
      return;
    }
    setPending(true);
    const result = await emptyQ(olderThanDays);
    setPending(false);
    setLastResult(
      result.errors.length === 0
        ? `Freed ${formatBytes(result.freed)} (${result.deleted.length} item${
            result.deleted.length === 1 ? "" : "s"
          })`
        : `${result.deleted.length} ok, ${result.errors.length} failed`,
    );
  };

  if (loading && entries.length === 0) {
    return <div className="text-sm text-muted p-4">Loading quarantine…</div>;
  }

  if (entries.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-border p-8 text-center text-sm text-muted">
        Quarantine is empty. Items moved here are restorable for 7 days.
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <div className="rounded-lg border border-border bg-surface p-4 flex flex-wrap items-center justify-between gap-3">
        <div className="text-sm">
          <span className="font-medium">{entries.length}</span> item
          {entries.length === 1 ? "" : "s"} ·{" "}
          <span className="font-mono">{formatBytes(total)}</span>
          {selected.size > 0 && (
            <span className="text-muted ml-2">
              ({selected.size} selected · {formatBytes(selectedSize)})
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleRestore}
            disabled={selected.size === 0 || pending}
            className="px-3 py-1.5 text-xs font-medium border border-border rounded-md hover:bg-bg disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Restore selected
          </button>
          <button
            type="button"
            onClick={() => handleEmpty(7)}
            disabled={pending || entries.length === 0}
            className="px-3 py-1.5 text-xs font-medium border border-border rounded-md hover:bg-bg disabled:opacity-50"
          >
            Purge expired (≥7d)
          </button>
          <button
            type="button"
            onClick={() => handleEmpty()}
            disabled={pending || entries.length === 0}
            className="px-3 py-1.5 text-xs font-medium bg-danger text-white rounded-md hover:opacity-90 disabled:opacity-50"
          >
            Empty all
          </button>
        </div>
      </div>

      {lastResult && (
        <div className="px-4 py-2 text-xs rounded-md bg-success/10 border border-success/20 text-success">
          {lastResult}
        </div>
      )}

      <div className="rounded-lg border border-border overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-surface text-xs uppercase tracking-wider text-muted">
            <tr>
              <th className="w-8 px-4 py-2">
                <input
                  type="checkbox"
                  checked={allSelected}
                  onChange={() => {
                    if (allSelected) setSelected(new Set());
                    else setSelected(new Set(entries.map((e) => e.id)));
                  }}
                  className="accent-accent"
                />
              </th>
              <th className="text-left px-2 py-2 font-medium">Original path</th>
              <th className="text-right px-4 py-2 font-medium w-24">Size</th>
              <th className="text-right px-4 py-2 font-medium w-32">Quarantined</th>
              <th className="text-right px-4 py-2 font-medium w-24">Expires</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e) => {
              const checked = selected.has(e.id);
              const expiresInDays = Math.ceil((e.expires_at * 1000 - Date.now()) / 86_400_000);
              return (
                <tr
                  key={e.id}
                  onClick={() => toggle(e.id)}
                  className="border-t border-border cursor-pointer hover:bg-surface/50"
                >
                  <td className="px-4 py-2">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggle(e.id)}
                      onClick={(ev) => ev.stopPropagation()}
                      className="accent-accent"
                    />
                  </td>
                  <td
                    className="px-2 py-2 truncate max-w-2xl font-mono text-xs"
                    title={e.original_path}
                  >
                    {e.original_path}
                  </td>
                  <td className="px-4 py-2 text-right font-mono text-xs">
                    {formatBytes(e.size)}
                  </td>
                  <td className="px-4 py-2 text-right text-muted text-xs">
                    {formatRelativeTime(e.deleted_at * 1000)}
                  </td>
                  <td className="px-4 py-2 text-right text-muted text-xs">
                    {expiresInDays > 0 ? `${expiresInDays}d` : "expired"}
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
