import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface SubProcessStat {
  name: string;
  path: string;
  connectionIds: string[];
  uploadSpeed: number;
  downloadSpeed: number;
  activeConnections: number;
  closedConnections: number;
  totalUpload: number;
  totalDownload: number;
}

export interface ProcessStat {
  name: string;
  path: string;
  connectionIds: string[];
  uploadSpeed: number;
  downloadSpeed: number;
  activeConnections: number;
  closedConnections: number;
  totalUploadSinceLaunch: number;
  totalDownloadSinceLaunch: number;
  todayUpload: number;
  todayDownload: number;
  topHost: string;
  subProcesses: SubProcessStat[];
}



export interface SpeedDataPoint {
  date: Date;
  upload: number;
  download: number;
}

export function formatSpeed(bytesPerSec: number) {
  if (bytesPerSec === 0) return { value: "0", unit: "KB/s" };
  const k = 1024;
  const sizes = ["B/s", "KB/s", "MB/s", "GB/s"];
  const i = Math.max(0, Math.floor(Math.log(bytesPerSec) / Math.log(k)));
  return {
    value: parseFloat((bytesPerSec / Math.pow(k, i)).toFixed(1)).toString(),
    unit: sizes[i]
  };
}

class DashboardStore {
  processes = $state<ProcessStat[]>([]);
  initialized = $state(false);
  
  speedHistory = $state<SpeedDataPoint[]>(Array.from({ length: 30 }, (_, i) => {
    const date = new Date();
    date.setSeconds(date.getSeconds() - (30 - i));
    return { date, upload: 0, download: 0 };
  }));

  globalUploadSpeed = $state(0);
  globalDownloadSpeed = $state(0);
  
  selectedProcessName = $state<string | null>(null);
  sortBy = $state<"speed" | "traffic" | "name">("traffic");
  searchQuery = $state("");

  #unlisten: UnlistenFn | null = null;
  #listening = false;

  constructor() {
    void this.setupListeners();
  }

  get selectedProcess() {
    return this.processes.find(p => p.name === this.selectedProcessName) || null;
  }

  async setupListeners() {
    // Guard synchronously so a double construction (e.g. Vite HMR) can't register
    // a second listener that would leak and double-count traffic.
    if (this.#listening) return;
    this.#listening = true;

    this.#unlisten = await listen<ProcessStat[]>('traffic-update', (event) => {
      this.processes = event.payload;

      let up = 0;
      let down = 0;
      for (const p of this.processes) {
        up += p.uploadSpeed;
        down += p.downloadSpeed;
      }
      this.globalUploadSpeed = up;
      this.globalDownloadSpeed = down;

      // Keep last 30 seconds
      const now = new Date();
      this.speedHistory = [...this.speedHistory, { date: now, upload: up, download: down }].slice(-30);

      this.initialized = true;
    });
  }

  /** Tear down the Tauri event listener. Call when the store is no longer needed. */
  destroy() {
    this.#unlisten?.();
    this.#unlisten = null;
    this.#listening = false;
  }

  filteredProcesses = $derived.by(() => {
    const q = this.searchQuery.trim().toLowerCase();
    const list = q
      ? this.processes.filter((p) => p.name.toLowerCase().includes(q))
      : this.processes;
    return [...list].sort((a, b) => {
      if (this.sortBy === "name") {
        return a.name.localeCompare(b.name);
      }
      if (this.sortBy === "speed") {
        const speedA = a.uploadSpeed + a.downloadSpeed;
        const speedB = b.uploadSpeed + b.downloadSpeed;
        if (speedB !== speedA) return speedB - speedA;
      } else {
        const totalA = a.totalUploadSinceLaunch + a.totalDownloadSinceLaunch;
        const totalB = b.totalUploadSinceLaunch + b.totalDownloadSinceLaunch;
        if (totalB !== totalA) return totalB - totalA;
      }
      return a.name.localeCompare(b.name);
    });
  });
}

export const dashboardStore = new DashboardStore();

// During dev, tear down the listener on hot-reload so we don't stack duplicates.
if (import.meta.hot) {
  import.meta.hot.dispose(() => dashboardStore.destroy());
}
