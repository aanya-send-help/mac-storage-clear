interface Props {
  info: { build: string; privileged: boolean; sandboxed: boolean; version: string };
}

export function BuildBadge({ info }: Props) {
  const label = info.build === "appstore" ? "App Store" : "Direct";
  return (
    <span
      className="text-[11px] px-2 py-0.5 rounded-full border border-border text-muted font-mono"
      title={`v${info.version} · ${label} build · privileged=${info.privileged} sandboxed=${info.sandboxed}`}
    >
      v{info.version} · {label}
    </span>
  );
}
