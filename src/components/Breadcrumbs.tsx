import { Fragment } from "react";

interface Props {
  path: string;
  /** Path the scan started at — we don't navigate above this. */
  rootPath: string;
  onNavigate: (path: string) => void;
}

/** macOS-style breadcrumb trail. Each segment is clickable. */
export function Breadcrumbs({ path, rootPath, onNavigate }: Props) {
  const segments = path.split("/").filter(Boolean);
  const rootSegments = rootPath.split("/").filter(Boolean);

  // Build cumulative paths matching each visible segment.
  const cumulative: string[] = [];
  for (let i = 0; i < segments.length; i++) {
    cumulative.push("/" + segments.slice(0, i + 1).join("/"));
  }

  // We always show segments from the scan root onward — the path components
  // above the root aren't meaningful in this view.
  const startIdx = rootSegments.length - 1; // include the root segment itself

  return (
    <div className="flex items-center gap-1 text-xs font-mono overflow-x-auto whitespace-nowrap">
      {segments.map((seg, i) => {
        if (i < startIdx) return null;
        const target = cumulative[i] ?? "/";
        const isCurrent = i === segments.length - 1;
        return (
          <Fragment key={i}>
            {i > startIdx && <span className="text-muted/50 select-none">›</span>}
            <button
              type="button"
              onClick={() => !isCurrent && onNavigate(target)}
              disabled={isCurrent}
              className={
                isCurrent
                  ? "text-fg font-medium"
                  : "text-muted hover:text-fg transition-colors"
              }
            >
              {seg}
            </button>
          </Fragment>
        );
      })}
    </div>
  );
}
