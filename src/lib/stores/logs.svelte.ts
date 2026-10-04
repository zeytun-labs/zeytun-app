import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { settingsStore } from "./settings.svelte";

export interface LogEvent {
  level: number;
  message: string;
}

export interface LogLine {
  id: number;
  level: number;
  message: string;
  time: string;
}

// ponytail: cap at 2000, trim to 1000. Upgrade to ring buffer if cap grows past ~10k.
const MAX_LOGS = 2000;
const TRIM_TO = 1000;

// mihomo gRPC log levels: 0=PANIC 1=FATAL 2=ERROR 3=WARN 4=INFO 5=DEBUG 6=TRACE
// Lower number = more severe. Filter: show only level <= threshold.
const LOG_LEVEL_MAP: Record<string, number> = {
  panic: 0,
  fatal: 1,
  error: 2,
  warn: 3,
  warning: 3,
  info: 4,
  debug: 5,
  trace: 6,
};

class LogsStore {
  logs = $state<LogLine[]>([]);
  #unlisten: UnlistenFn | null = null;
  #idCounter = 0;

  async init() {
    if (this.#unlisten) return;

    this.#unlisten = await listen<LogEvent>("core-log-event", (event) => {
      // Filter by configured log level
      const threshold = LOG_LEVEL_MAP[settingsStore.settings.logLevel] ?? 4;
      if (event.payload.level > threshold) return;

      const now = new Date();
      const time = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}:${now.getSeconds().toString().padStart(2, '0')}.${now.getMilliseconds().toString().padStart(3, '0')}`;

      this.logs.push({
        id: this.#idCounter++,
        level: event.payload.level,
        message: event.payload.message,
        time,
      });

      // Batch trim: amortize the cost of dropping old entries.
      // One splice per ~1000 logs instead of shift() on every insert.
      if (this.logs.length > MAX_LOGS) {
        this.logs.splice(0, this.logs.length - TRIM_TO);
      }
    });
  }

  clear() {
    this.logs = [];
  }

  stopListening() {
    this.#unlisten?.();
    this.#unlisten = null;
  }
}

export const logsStore = new LogsStore();
