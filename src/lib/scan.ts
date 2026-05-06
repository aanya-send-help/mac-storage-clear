import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "./tauri";

export interface ScanStatus {
  scan_id: number;
  status: "running" | "done" | "cancelled" | "failed";
  files_seen: number;
  bytes_seen: number;
  current_path: string | null;
  started_at: number;
  finished_at: number | null;
  elapsed_ms: number;
}

export interface TreemapNode {
  name: string;
  full_path: string;
  size: number;
  is_dir: boolean;
  child_count: number;
}

export interface LargestFile {
  full_path: string;
  name: string;
  size: number;
  mtime: number | null;
}

interface ScanState {
  status: ScanStatus | null;
  defaultRoots: string[];
  treemapRoot: string | null;
  treemap: TreemapNode[];
  largest: LargestFile[];
  loadingTreemap: boolean;
  loadingLargest: boolean;
  error: string | null;

  loadDefaultRoots: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  startScan: (roots?: string[]) => Promise<void>;
  cancelScan: () => Promise<void>;
  loadTreemap: (parent: string) => Promise<void>;
  loadLargest: (limit?: number) => Promise<void>;
  initEvents: () => Promise<UnlistenFn>;
}

export const useScanStore = create<ScanState>((set, get) => ({
  status: null,
  defaultRoots: [],
  treemapRoot: null,
  treemap: [],
  largest: [],
  loadingTreemap: false,
  loadingLargest: false,
  error: null,

  async loadDefaultRoots() {
    try {
      const roots = await invoke<string[]>("default_scan_roots");
      set({ defaultRoots: roots });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async refreshStatus() {
    try {
      const status = await invoke<ScanStatus | null>("get_scan_status");
      set({ status });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async startScan(roots) {
    const targets = roots ?? get().defaultRoots;
    if (targets.length === 0) {
      set({ error: "no scan roots available" });
      return;
    }
    set({
      error: null,
      treemapRoot: targets[0] ?? null,
      treemap: [],
      largest: [],
    });
    try {
      await invoke<{ scan_id: number }>("start_scan", { roots: targets });
      await get().refreshStatus();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async cancelScan() {
    try {
      await invoke<void>("cancel_scan");
      await get().refreshStatus();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async loadTreemap(parent) {
    set({ loadingTreemap: true, treemapRoot: parent });
    try {
      const nodes = await invoke<TreemapNode[]>("get_treemap", {
        parent,
        limit: 200,
      });
      set({ treemap: nodes, loadingTreemap: false });
    } catch (e) {
      set({ error: String(e), loadingTreemap: false });
    }
  },

  async loadLargest(limit = 200) {
    set({ loadingLargest: true });
    try {
      const files = await invoke<LargestFile[]>("list_largest", { limit });
      set({ largest: files, loadingLargest: false });
    } catch (e) {
      set({ error: String(e), loadingLargest: false });
    }
  },

  async initEvents() {
    const unsubProgress = await listen<ScanStatus>("scan:progress", (e) => {
      set({ status: e.payload });
    });
    const unsubFinished = await listen<ScanStatus>("scan:finished", (e) => {
      set({ status: e.payload });
      // Auto-refresh treemap and largest once a scan completes.
      const root = get().treemapRoot ?? get().defaultRoots[0];
      if (root) get().loadTreemap(root);
      get().loadLargest();
    });
    return () => {
      unsubProgress();
      unsubFinished();
    };
  },
}));
