import { useScanStore } from "../lib/scan";
import { formatBytes } from "../lib/format";

export function DeleteProgress() {
  const status = useScanStore((s) => s.deleteStatus);
  const cancel = useScanStore((s) => s.cancelDelete);

  if (!status) return null;
  if (status.status !== "running" && status.status !== "cancelled") {
    // Successful runs disappear; we already emit a log line + toast elsewhere.
    if (status.status === "done" && status.errors.length === 0) return null;
  }

  const filesPct =
    status.total_files > 0
      ? Math.round((status.files_seen * 100) / status.total_files)
      : 0;
  const bytesPerSec =
    status.elapsed_ms > 0 ? (status.bytes_freed * 1000) / status.elapsed_ms : 0;
  const filesPerSec =
    status.elapsed_ms > 0
      ? Math.round((status.files_seen * 1000) / status.elapsed_ms)
      : 0;
  const isRunning = status.status === "running";

  const banner =
    status.status === "running"
      ? "bg-accent/10 border-accent/30 text-fg"
      : status.status === "cancelled"
        ? "bg-yellow-500/10 border-yellow-500/30 text-fg"
        : status.status === "failed"
          ? "bg-danger/10 border-danger/30 text-fg"
          : "bg-success/10 border-success/30 text-fg";

  return (
    <div className={`rounded-lg border p-3 space-y-2 ${banner}`}>
      <div className="flex items-center justify-between gap-3">
        <div className="text-sm">
          <span className="font-medium capitalize">{labelForMode(status.mode)}</span>
          <span className="mx-2 text-muted">·</span>
          <span className="capitalize">{status.status}</span>
          {status.total_files > 0 && (
            <>
              <span className="mx-2 text-muted">·</span>
              <span className="font-mono">
                {status.files_seen}/{status.total_files} ({filesPct}%)
              </span>
            </>
          )}
          <span className="mx-2 text-muted">·</span>
          <span className="font-mono">{formatBytes(status.bytes_freed)} freed</span>
          {status.errors.length > 0 && (
            <span className="ml-2 text-danger text-xs">
              · {status.errors.length} error{status.errors.length === 1 ? "" : "s"}
            </span>
          )}
        </div>
        {isRunning && (
          <button
            type="button"
            onClick={cancel}
            className="px-3 py-1 text-xs bg-danger text-white rounded hover:opacity-90"
          >
            Cancel
          </button>
        )}
      </div>
      {status.current_path && (
        <div
          className="text-xs text-muted font-mono truncate"
          title={status.current_path}
        >
          {status.current_path}
        </div>
      )}
      {isRunning && (
        <div className="text-xs text-muted">
          {filesPerSec.toLocaleString()} items/s · {formatBytes(bytesPerSec)}/s ·{" "}
          {formatDuration(status.elapsed_ms)} elapsed
        </div>
      )}
      {status.status === "running" && status.total_files > 0 && (
        <div className="h-1.5 bg-bg/50 rounded-full overflow-hidden">
          <div
            className="h-full bg-accent transition-all"
            style={{ width: `${filesPct}%` }}
          />
        </div>
      )}
    </div>
  );
}

function labelForMode(mode: string): string {
  switch (mode) {
    case "trash":
      return "Move to Trash";
    case "quarantine":
      return "Move to Quarantine";
    case "hard":
      return "Delete";
    default:
      return mode;
  }
}

function formatDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const sec = s % 60;
  if (m > 0) return `${m}m ${sec}s`;
  return `${sec}s`;
}
