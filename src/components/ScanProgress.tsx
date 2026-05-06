import { useScanStore } from "../lib/scan";
import { formatBytes, formatRelativeTime } from "../lib/format";

export function ScanProgress() {
  const status = useScanStore((s) => s.status);
  const cancelScan = useScanStore((s) => s.cancelScan);
  const startScan = useScanStore((s) => s.startScan);
  const defaultRoots = useScanStore((s) => s.defaultRoots);

  if (!status) {
    return (
      <div className="flex items-center justify-between gap-4 p-4 bg-surface border border-border rounded-lg">
        <div className="text-sm text-muted">
          {defaultRoots.length === 0
            ? "Detecting scan root…"
            : `Ready to scan ${defaultRoots[0]}`}
        </div>
        <button
          onClick={() => startScan()}
          disabled={defaultRoots.length === 0}
          className="px-4 py-2 text-sm font-medium bg-accent text-white rounded-md hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Start scan
        </button>
      </div>
    );
  }

  const isRunning = status.status === "running";
  const filesPerSec =
    status.elapsed_ms > 0
      ? Math.round((status.files_seen * 1000) / status.elapsed_ms)
      : 0;
  const bytesPerSec =
    status.elapsed_ms > 0
      ? (status.bytes_seen * 1000) / status.elapsed_ms
      : 0;

  return (
    <div className="p-4 bg-surface border border-border rounded-lg space-y-3">
      <div className="flex items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            {isRunning && (
              <span className="inline-block w-2 h-2 rounded-full bg-accent animate-pulse" />
            )}
            <span className="text-sm font-medium capitalize">{status.status}</span>
            <span className="text-xs text-muted font-mono">scan #{status.scan_id}</span>
            {!isRunning && status.finished_at && (
              <span className="text-xs text-muted">
                · {formatRelativeTime(status.finished_at * 1000)}
              </span>
            )}
          </div>
          <div className="mt-1 text-xs text-muted truncate max-w-2xl" title={status.current_path ?? ""}>
            {status.current_path ?? "—"}
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {isRunning ? (
            <button
              onClick={cancelScan}
              className="px-3 py-1.5 text-sm bg-danger text-white rounded-md hover:opacity-90"
            >
              Cancel
            </button>
          ) : (
            <button
              onClick={() => startScan()}
              className="px-3 py-1.5 text-sm bg-accent text-white rounded-md hover:opacity-90"
            >
              Re-scan
            </button>
          )}
        </div>
      </div>

      <div className="grid grid-cols-4 gap-4 text-sm">
        <Stat label="Files" value={status.files_seen.toLocaleString()} />
        <Stat label="Size" value={formatBytes(status.bytes_seen)} />
        <Stat label="Elapsed" value={formatDuration(status.elapsed_ms)} />
        <Stat
          label="Throughput"
          value={`${filesPerSec.toLocaleString()}/s · ${formatBytes(bytesPerSec)}/s`}
        />
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs text-muted">{label}</div>
      <div className="font-mono">{value}</div>
    </div>
  );
}

function formatDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const sec = s % 60;
  if (m > 0) return `${m}m ${sec}s`;
  return `${sec}s`;
}
