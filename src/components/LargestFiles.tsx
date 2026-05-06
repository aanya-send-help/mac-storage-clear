import { useEffect } from "react";
import { useScanStore } from "../lib/scan";
import { formatBytes, formatRelativeTime } from "../lib/format";

export function LargestFiles() {
  const largest = useScanStore((s) => s.largest);
  const loading = useScanStore((s) => s.loadingLargest);
  const loadLargest = useScanStore((s) => s.loadLargest);

  useEffect(() => {
    if (largest.length === 0 && !loading) {
      loadLargest();
    }
    // We only want this on mount; manual reloads come from elsewhere.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (loading) {
    return <div className="text-sm text-muted p-4">Loading largest files…</div>;
  }

  if (largest.length === 0) {
    return (
      <div className="text-sm text-muted p-4 border border-dashed border-border rounded-lg">
        No file data. Run a scan first.
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border overflow-hidden">
      <table className="w-full text-sm">
        <thead className="bg-surface text-xs uppercase tracking-wider text-muted">
          <tr>
            <th className="text-left px-4 py-2 font-medium">Path</th>
            <th className="text-right px-4 py-2 font-medium w-24">Size</th>
            <th className="text-right px-4 py-2 font-medium w-32">Modified</th>
          </tr>
        </thead>
        <tbody>
          {largest.map((f) => (
            <tr key={f.full_path} className="border-t border-border hover:bg-surface">
              <td className="px-4 py-2 truncate max-w-2xl font-mono text-xs" title={f.full_path}>
                {f.full_path}
              </td>
              <td className="px-4 py-2 text-right font-mono">{formatBytes(f.size)}</td>
              <td className="px-4 py-2 text-right text-muted text-xs">
                {f.mtime ? formatRelativeTime(f.mtime * 1000) : "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
