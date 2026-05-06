import { useEffect, useState } from "react";
import { invoke } from "./lib/tauri";
import { useScanStore } from "./lib/scan";
import { ThemeProvider } from "./lib/theme";
import { BuildBadge } from "./components/BuildBadge";
import { ThemePicker } from "./components/ThemePicker";
import { ScanProgress } from "./components/ScanProgress";
import { Treemap } from "./components/Treemap";
import { LargestFiles } from "./components/LargestFiles";
import { Categories } from "./components/Categories";
import { Quarantine } from "./components/Quarantine";
import { DeleteProgress } from "./components/DeleteProgress";
import { LogOverlay } from "./components/LogOverlay";

interface BuildInfo {
  version: string;
  build: "appstore" | "devid";
  privileged: boolean;
  sandboxed: boolean;
}

type TabId = "categories" | "treemap" | "largest" | "quarantine";

function Shell() {
  const [build, setBuild] = useState<BuildInfo | null>(null);
  const [tab, setTab] = useState<TabId>("categories");
  const error = useScanStore((s) => s.error);
  const loadDefaultRoots = useScanStore((s) => s.loadDefaultRoots);
  const refreshStatus = useScanStore((s) => s.refreshStatus);
  const initEvents = useScanStore((s) => s.initEvents);

  useEffect(() => {
    invoke<BuildInfo>("get_build_info")
      .then(setBuild)
      .catch((err: unknown) => console.error("get_build_info", err));
  }, []);

  useEffect(() => {
    loadDefaultRoots();
    refreshStatus();
    let cleanup: (() => void) | undefined;
    initEvents().then((fn) => {
      cleanup = fn;
    });
    return () => {
      cleanup?.();
    };
  }, [loadDefaultRoots, refreshStatus, initEvents]);

  return (
    <div className="min-h-screen bg-bg text-fg flex flex-col">
      <header className="px-6 py-4 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-lg font-semibold tracking-tight">Mac Storage Clear</h1>
          {build && <BuildBadge info={build} />}
        </div>
        <ThemePicker />
      </header>
      <main className="flex-1 p-6 max-w-6xl mx-auto w-full pb-20">
        <div className="space-y-6">
          <ScanProgress />
          <DeleteProgress />

          {error && (
            <div className="p-3 text-sm bg-danger/10 text-danger border border-danger/30 rounded-md font-mono">
              {error}
            </div>
          )}

          <div className="flex gap-1 border-b border-border">
            <TabButton active={tab === "categories"} onClick={() => setTab("categories")}>
              Categories
            </TabButton>
            <TabButton active={tab === "treemap"} onClick={() => setTab("treemap")}>
              Treemap
            </TabButton>
            <TabButton active={tab === "largest"} onClick={() => setTab("largest")}>
              Largest files
            </TabButton>
            <TabButton active={tab === "quarantine"} onClick={() => setTab("quarantine")}>
              Quarantine
            </TabButton>
          </div>

          {tab === "categories" && <Categories />}
          {tab === "treemap" && <Treemap />}
          {tab === "largest" && <LargestFiles />}
          {tab === "quarantine" && <Quarantine />}
        </div>
      </main>
      <LogOverlay />
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
        active
          ? "border-accent text-fg"
          : "border-transparent text-muted hover:text-fg"
      }`}
    >
      {children}
    </button>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <Shell />
    </ThemeProvider>
  );
}
