import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ThemeProvider } from "./lib/theme";
import { BuildBadge } from "./components/BuildBadge";
import { ThemePicker } from "./components/ThemePicker";

interface BuildInfo {
  version: string;
  build: "appstore" | "devid";
  privileged: boolean;
  sandboxed: boolean;
}

function Shell() {
  const [build, setBuild] = useState<BuildInfo | null>(null);

  useEffect(() => {
    invoke<BuildInfo>("get_build_info")
      .then(setBuild)
      .catch((err: unknown) => {
        console.error("get_build_info failed", err);
      });
  }, []);

  return (
    <div className="min-h-screen bg-bg text-fg flex flex-col">
      <header className="px-6 py-4 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-lg font-semibold tracking-tight">Mac Storage Clear</h1>
          {build && <BuildBadge info={build} />}
        </div>
        <ThemePicker />
      </header>
      <main className="flex-1 p-6">
        <div className="max-w-3xl mx-auto">
          <p className="text-muted">
            Phase 0 scaffold. Scanner not wired yet — the disk visualization, categories,
            and delete pipeline land in subsequent phases.
          </p>
        </div>
      </main>
    </div>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <Shell />
    </ThemeProvider>
  );
}
