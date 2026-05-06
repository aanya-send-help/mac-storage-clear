import { useTheme, type Theme } from "../lib/theme";

const LABELS: Record<Theme, string> = {
  system: "System",
  light: "Light",
  dark: "Dark",
  pink: "Pink",
};

export function ThemePicker() {
  const { theme, setTheme, available } = useTheme();

  return (
    <label className="text-xs text-muted flex items-center gap-2">
      <span>Theme</span>
      <select
        value={theme}
        onChange={(e) => setTheme(e.target.value as Theme)}
        className="bg-surface border border-border rounded px-2 py-1 text-fg outline-none focus:border-accent"
      >
        {available.map((t) => (
          <option key={t} value={t}>
            {LABELS[t]}
          </option>
        ))}
      </select>
    </label>
  );
}
