import { useEffect, useRef, useState } from "react";
import type { DeleteMode } from "../lib/scan";

interface Props {
  /** What does the trigger button look like? */
  trigger: React.ReactNode;
  disabled?: boolean;
  onPick: (mode: DeleteMode) => void;
}

const OPTIONS: {
  mode: DeleteMode;
  label: string;
  hint: string;
  cls: string;
}[] = [
  {
    mode: "trash",
    label: "Move to Trash",
    hint: "macOS Finder Trash · restorable via 'Put Back'",
    cls: "hover:bg-accent/10 text-fg",
  },
  {
    mode: "quarantine",
    label: "Move to Quarantine",
    hint: "Our 7-day staging area · restore from this app",
    cls: "hover:bg-accent/10 text-fg",
  },
  {
    mode: "hard",
    label: "Delete now",
    hint: "Permanent · cannot be undone",
    cls: "hover:bg-danger/10 text-danger",
  },
];

export function DeleteMenu({ trigger, disabled, onPick }: Props) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", handler);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  return (
    <div ref={containerRef} className="relative inline-block">
      <button
        type="button"
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        className="px-3 py-1.5 text-xs font-medium border border-border rounded-md hover:bg-surface disabled:opacity-50 disabled:cursor-not-allowed inline-flex items-center gap-1"
      >
        {trigger}
        <span className="text-muted">▾</span>
      </button>
      {open && (
        <div className="absolute right-0 mt-1 w-72 rounded-lg border border-border bg-bg shadow-lg z-20 overflow-hidden">
          {OPTIONS.map((opt) => (
            <button
              key={opt.mode}
              type="button"
              onClick={() => {
                setOpen(false);
                onPick(opt.mode);
              }}
              className={`w-full text-left px-3 py-2 text-sm border-b border-border last:border-b-0 ${opt.cls}`}
            >
              <div className="font-medium">{opt.label}</div>
              <div className="text-xs text-muted mt-0.5">{opt.hint}</div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Confirmation copy + button color per delete mode. */
export function deletePromptCopy(
  mode: DeleteMode,
  count: number,
  bytes: string,
): { question: string; buttonLabel: string; buttonClass: string } {
  const items = `${count} item${count === 1 ? "" : "s"} (${bytes})`;
  switch (mode) {
    case "trash":
      return {
        question: `Move ${items} to the macOS Trash?`,
        buttonLabel: "Move to Trash",
        buttonClass: "bg-accent text-white",
      };
    case "quarantine":
      return {
        question: `Move ${items} to quarantine? Restorable for 7 days.`,
        buttonLabel: "Quarantine",
        buttonClass: "bg-accent text-white",
      };
    case "hard":
      return {
        question: `Permanently delete ${items}? This can't be undone.`,
        buttonLabel: "Delete now",
        buttonClass: "bg-danger text-white",
      };
  }
}
