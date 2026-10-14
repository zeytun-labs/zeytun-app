import { Channel, Resource, invoke } from "@tauri-apps/api/core";

export type ReleaseMetadata = {
  rid: number;
  currentVersion: string;
  version: string;
  date?: string | null;
  body?: string | null;
};
export type DownloadEvent =
  | { event: "Started"; data: { contentLength: number | null } }
  | { event: "Progress"; data: { chunkLength: number } };

/** Opaque native resource: only Rust chooses and authenticates update bytes. */
export class ReleaseUpdate extends Resource {
  readonly version: string;
  readonly currentVersion: string;
  constructor(metadata: ReleaseMetadata) {
    super(metadata.rid);
    this.version = metadata.version;
    this.currentVersion = metadata.currentVersion;
  }
  async download(onEvent: (event: DownloadEvent) => void) {
    const channel = new Channel<DownloadEvent>();
    channel.onmessage = onEvent;
    await invoke("download_release_update", { rid: this.rid, onEvent: channel });
  }
  async install() {
    await invoke("install_release_update", { rid: this.rid });
  }
}
