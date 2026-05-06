import { useEffect, useRef, useState } from "react";
import { useScanStore } from "../lib/scan";

export function LogOverlay() {
  const logs = useScanStore((s) => s.logs);
  const deleteStatus = useScanStore((s) => s.deleteStatus);
  const [open, setOpen] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new logs arrive while the panel is open.
  useEffect(() => {
    if (open && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs, deleteStatus, open]);

  // Most recent error/warn is shown as a summary in the chip when collapsed.
  const lastEvent = logs[logs.length - 1];

  return (
    <div className="fixed bottom-0 left-0 right-0 z-30 pointer-events-none">
      <div className="max-w-6xl mx-auto px-6 pb-3 flex justify-end">
        {open ? (
          <div className="pointer-events-auto w-full max-w-3xl rounded-lg border border-border bg-bg shadow-xl overflow-hidden">
            <div className="px-3 py-2 bg-surface border-b border-border flex items-center justify-between text-xs">
              <span className="font-medium">
                Activity log <span className="text-muted">· {logs.length} entries</span>
              </span>
              <button
                type="button"
                onClick={() => setOpen(false)}
                className="text-muted hover:text-fg"
              >
                Close
              </button>
            </div>
            <div ref={scrollRef} className="max-h-72 overflow-y-auto font-mono text-[11px]">
              {logs.length === 0 ? (
                <div className="px-3 py-4 text-muted">No activity yet.</div>
              ) : (
                logs.map((l, i) => (
                  <div
                    key={i}
                    className={`px-3 py-1 border-b border-border/30 last:border-b-0 ${colorFor(l.level)}`}
                  >
                    <span className="text-muted">{formatTime(l.ts)}</span>{" "}
                    <span>{l.message}</span>
                  </div>
                ))
              )}
              {deleteStatus?.current_path && deleteStatus.status === "running" && (
                <div className="px-3 py-1 text-muted bg-surface/50">
                  <span className="text-accent">▸</span> {deleteStatus.current_path}
                </div>
              )}
            </div>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setOpen(true)}
            className="pointer-events-auto px-3 py-1.5 rounded-full border border-border bg-bg shadow-md text-xs hover:bg-surface flex items-center gap-2"
          >
            <span>Log</span>
            <span className="text-muted">{logs.length}</span>
            {deleteStatus?.status === "running" && (
              <span className="inline-block w-1.5 h-1.5 rounded-full bg-accent animate-pulse" />
            )}
            {lastEvent && lastEvent.level !== "info" && (
              <span className={colorFor(lastEvent.level)}>
                {truncate(lastEvent.message, 40)}
              </span>
            )}
          </button>
        )}
      </div>
    </div>
  );
}

function colorFor(level: "info" | "warn" | "error"): string {
  switch (level) {
    case "warn":
      return "text-yellow-600";
    case "error":
      return "text-danger";
    default:
      return "text-fg";
  }
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function pad(n: number): string {
  return n.toString().padStart(2, "0");
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + "…";
}
