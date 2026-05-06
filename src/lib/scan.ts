import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "./tauri";

/** Deepest path that is a prefix of every input. Returns "/" if nothing in common. */
function commonAncestor(paths: string[]): string {
  if (paths.length === 0) return "/";
  const splits = paths.map((p) => p.split("/").filter(Boolean));
  const out: string[] = [];
  const first = splits[0]!;
  for (let i = 0; i < first.length; i++) {
    const seg = first[i];
    if (splits.every((s) => s[i] === seg)) {
      out.push(seg!);
    } else {
      break;
    }
  }
  return out.length === 0 ? "/" : "/" + out.join("/");
}

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

export type Risk = "safe" | "needs-redownload" | "user-decides";

export interface CategorySummary {
  id: string;
  name: string;
  description: string;
  risk: Risk;
  total_size: number;
  item_count: number;
}

export interface CategoryItem {
  path: string;
  size: number;
  mtime: number | null;
  is_dir: boolean;
  group: string | null;
}

export type DeleteMode = "trash" | "quarantine" | "hard";

export interface DeleteResult {
  freed: number;
  deleted: string[];
  errors: { path: string; message: string }[];
}

export interface QuarantineEntry {
  id: number;
  original_path: string;
  quarantine_path: string;
  deleted_at: number;
  expires_at: number;
  size: number;
}

export interface DeleteStatus {
  delete_id: number;
  mode: DeleteMode;
  status: "running" | "done" | "cancelled" | "failed";
  files_seen: number;
  bytes_freed: number;
  total_files: number;
  current_path: string | null;
  errors: { path: string; message: string }[];
  started_at: number;
  finished_at: number | null;
  elapsed_ms: number;
}

export interface LogEntry {
  ts: number;
  level: "info" | "warn" | "error";
  message: string;
}

interface ScanState {
  status: ScanStatus | null;
  defaultRoots: string[];
  treemapRoot: string | null;
  treemap: TreemapNode[];
  largest: LargestFile[];
  loadingTreemap: boolean;
  loadingLargest: boolean;
  categories: CategorySummary[];
  loadingCategories: boolean;
  categoryItems: Record<string, CategoryItem[]>;
  loadingCategoryItems: Record<string, boolean>;
  quarantine: QuarantineEntry[];
  loadingQuarantine: boolean;
  deleteStatus: DeleteStatus | null;
  logs: LogEntry[];
  error: string | null;

  loadDefaultRoots: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  startScan: (roots?: string[]) => Promise<void>;
  cancelScan: () => Promise<void>;
  loadTreemap: (parent: string) => Promise<void>;
  loadLargest: (limit?: number) => Promise<void>;
  loadCategories: () => Promise<void>;
  loadCategoryItems: (id: string, limit?: number) => Promise<void>;
  startDelete: (paths: string[], mode: DeleteMode) => Promise<{ delete_id: number } | null>;
  cancelDelete: () => Promise<void>;
  retryDeleteAdmin: (paths: string[]) => Promise<DeleteResult>;
  loadQuarantine: () => Promise<void>;
  restoreFromQuarantine: (ids: number[]) => Promise<DeleteResult>;
  emptyQuarantine: (olderThanDays?: number) => Promise<DeleteResult>;
  refreshAll: () => Promise<void>;
  initEvents: () => Promise<UnlistenFn>;
  /** True iff a delete is currently running. UI uses this to disable other delete actions. */
  isDeleting: () => boolean;
  log: (level: LogEntry["level"], message: string) => void;
}

export const useScanStore = create<ScanState>((set, get) => ({
  status: null,
  defaultRoots: [],
  treemapRoot: null,
  treemap: [],
  largest: [],
  loadingTreemap: false,
  loadingLargest: false,
  categories: [],
  loadingCategories: false,
  categoryItems: {},
  loadingCategoryItems: {},
  quarantine: [],
  loadingQuarantine: false,
  deleteStatus: null,
  logs: [],
  error: null,

  async loadDefaultRoots() {
    try {
      const roots = await invoke<string[]>("default_scan_roots");
      set({ defaultRoots: roots });
      // Treemap initial view: if we have multiple scan roots (HOME + claimed
      // orphan homes), default to their common ancestor so they appear as
      // siblings. Otherwise just use the single root.
      const target =
        roots.length > 1 ? commonAncestor(roots) : roots[0] ?? null;
      if (target && get().treemap.length === 0) {
        set({ treemapRoot: target });
        get().loadTreemap(target);
      }
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

  async loadCategories() {
    set({ loadingCategories: true });
    try {
      const cats = await invoke<CategorySummary[]>("list_categories");
      set({ categories: cats, loadingCategories: false });
    } catch (e) {
      set({ error: String(e), loadingCategories: false });
    }
  },

  async loadCategoryItems(id, limit = 500) {
    set((s) => ({
      loadingCategoryItems: { ...s.loadingCategoryItems, [id]: true },
    }));
    try {
      const items = await invoke<CategoryItem[]>("get_category_items", {
        categoryId: id,
        limit,
      });
      set((s) => ({
        categoryItems: { ...s.categoryItems, [id]: items },
        loadingCategoryItems: { ...s.loadingCategoryItems, [id]: false },
      }));
    } catch (e) {
      set((s) => ({
        error: String(e),
        loadingCategoryItems: { ...s.loadingCategoryItems, [id]: false },
      }));
    }
  },

  async startDelete(paths, mode) {
    try {
      get().log("info", `Starting ${mode} on ${paths.length} item${paths.length === 1 ? "" : "s"}`);
      const result = await invoke<{ delete_id: number; total_files: number }>(
        "start_delete",
        { paths, mode },
      );
      // Seed an immediate "running" status so the UI doesn't have to wait
      // for the first event tick.
      set({
        deleteStatus: {
          delete_id: result.delete_id,
          mode,
          status: "running",
          files_seen: 0,
          bytes_freed: 0,
          total_files: result.total_files,
          current_path: null,
          errors: [],
          started_at: Math.floor(Date.now() / 1000),
          finished_at: null,
          elapsed_ms: 0,
        },
      });
      return { delete_id: result.delete_id };
    } catch (e) {
      set({ error: String(e) });
      get().log("error", `Delete failed to start: ${e}`);
      return null;
    }
  },

  async cancelDelete() {
    try {
      await invoke<void>("cancel_delete");
      get().log("info", "Cancel requested");
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async retryDeleteAdmin(paths) {
    try {
      get().log("warn", `Retrying ${paths.length} item${paths.length === 1 ? "" : "s"} with admin`);
      const result = await invoke<DeleteResult>("retry_delete_admin", { paths });
      const summary =
        result.errors.length === 0
          ? `Admin retry: freed ${result.freed} bytes (${result.deleted.length} items)`
          : `Admin retry: ${result.deleted.length} ok, ${result.errors.length} still failed`;
      get().log(result.errors.length === 0 ? "info" : "warn", summary);
      // Update deleteStatus so the banner reflects the new state.
      set((s) => {
        if (!s.deleteStatus) return {};
        const remainingErrors = s.deleteStatus.errors.filter(
          (e) => !result.deleted.includes(e.path),
        );
        return {
          deleteStatus: {
            ...s.deleteStatus,
            bytes_freed: s.deleteStatus.bytes_freed + result.freed,
            files_seen: s.deleteStatus.files_seen + result.deleted.length,
            errors: remainingErrors,
          },
        };
      });
      // Refresh the views.
      const root = get().treemapRoot ?? get().defaultRoots[0];
      if (root) get().loadTreemap(root);
      get().loadCategories();
      return result;
    } catch (e) {
      set({ error: String(e) });
      get().log("error", `Admin retry failed: ${e}`);
      return { freed: 0, deleted: [], errors: [{ path: "", message: String(e) }] };
    }
  },

  async loadQuarantine() {
    set({ loadingQuarantine: true });
    try {
      const entries = await invoke<QuarantineEntry[]>("list_quarantine");
      set({ quarantine: entries, loadingQuarantine: false });
    } catch (e) {
      set({ error: String(e), loadingQuarantine: false });
    }
  },

  async restoreFromQuarantine(ids) {
    try {
      const result = await invoke<DeleteResult>("restore_from_quarantine", { ids });
      get().loadQuarantine();
      get().loadCategories();
      return result;
    } catch (e) {
      set({ error: String(e) });
      return { freed: 0, deleted: [], errors: [{ path: "", message: String(e) }] };
    }
  },

  async emptyQuarantine(olderThanDays) {
    try {
      const result = await invoke<DeleteResult>("empty_quarantine", {
        olderThanDays: olderThanDays ?? null,
      });
      get().loadQuarantine();
      return result;
    } catch (e) {
      set({ error: String(e) });
      return { freed: 0, deleted: [], errors: [{ path: "", message: String(e) }] };
    }
  },

  async refreshAll() {
    const { treemapRoot, defaultRoots, loadTreemap, loadLargest, loadCategories, refreshStatus } =
      get();
    const target = treemapRoot ?? defaultRoots[0];
    await Promise.all([
      target ? loadTreemap(target) : Promise.resolve(),
      loadLargest(),
      loadCategories(),
      refreshStatus(),
    ]);
  },

  async initEvents() {
    const unsubProgress = await listen<ScanStatus>("scan:progress", (e) => {
      set({ status: e.payload });
    });
    const unsubFinished = await listen<ScanStatus>("scan:finished", (e) => {
      set({ status: e.payload });
      // Auto-refresh treemap, largest, and categories once a scan completes.
      const root = get().treemapRoot ?? get().defaultRoots[0];
      if (root) get().loadTreemap(root);
      get().loadLargest();
      get().loadCategories();
    });
    const unsubDeleteProgress = await listen<DeleteStatus>("delete:progress", (e) => {
      set({ deleteStatus: e.payload });
    });
    const unsubDeleteFinished = await listen<DeleteStatus>("delete:finished", (e) => {
      const s = e.payload;
      set({ deleteStatus: s });
      const summary =
        s.errors.length === 0
          ? `${s.status}: freed ${s.bytes_freed} bytes (${s.files_seen}/${s.total_files} items)`
          : `${s.status}: ${s.files_seen}/${s.total_files} items, ${s.errors.length} error${s.errors.length === 1 ? "" : "s"}`;
      get().log(s.errors.length === 0 ? "info" : "warn", summary);
      // Refresh affected views.
      const root = get().treemapRoot ?? get().defaultRoots[0];
      if (root) get().loadTreemap(root);
      get().loadCategories();
      if (s.mode === "quarantine") get().loadQuarantine();
    });
    return () => {
      unsubProgress();
      unsubFinished();
      unsubDeleteProgress();
      unsubDeleteFinished();
    };
  },

  isDeleting() {
    const s = get().deleteStatus;
    return s?.status === "running";
  },

  log(level, message) {
    set((s) => {
      const next = [...s.logs, { ts: Date.now(), level, message }];
      // Keep last 200 entries — that's enough to scroll back through a session.
      return { logs: next.slice(-200) };
    });
  },
}));
