import type { ColumnDef } from "@tanstack/table-core";
import { renderComponent } from "$lib/components/ui/data-table";
import { formatBytes } from "$lib/utils";
import { profileStore } from "$lib/stores/profile.svelte";
import type { ConnDto } from "$lib/core/types";
import CellStatus from "./cell-status.svelte";
import CellClient from "./cell-client.svelte";
import { fmtTime, formatDuration, methodOf } from "./lib";
import CellBadge from "./cell-badge.svelte";

const policyName = (tag: string) => profileStore.getMemberName(tag);

/** Human labels for the column-visibility dropdown. */
export const COLUMN_LABELS: Record<string, string> = {
  status: "Status",
  network: "Network",
  start: "Start",
  client: "Client",
  policy: "Policy",
  up: "Up",
  down: "Down",
  duration: "Duration",
  method: "Method",
  url: "URL",
};

export function buildColumns(): ColumnDef<ConnDto>[] {
  return [
    {
      id: "status",
      header: "",
      accessorKey: "status",
      cell: ({ row }) =>
        renderComponent(CellStatus, { status: row.original.status }),
    },
    {
      id: "network",
      header: "Network",
      accessorKey: "network",
      cell: ({ row }) =>
        renderComponent(CellBadge, { value: row.original.network || "—" }),
    },
    {
      id: "start",
      header: "Start",
      accessorFn: (r) => r.createdAt,
      cell: ({ row }) => fmtTime(row.original.createdAt),
    },
    {
      id: "client",
      header: "Client",
      accessorKey: "processName",
      cell: ({ row }) =>
        renderComponent(CellClient, {
          name: row.original.processName,
          path: row.original.processPath,
        }),
    },
    {
      id: "policy",
      header: "Policy",
      accessorFn: (r) => policyName(r.policy),
      cell: ({ row }) => policyName(row.original.policy),
    },
    {
      id: "up",
      header: "Up",
      accessorKey: "upBytes",
      cell: ({ row }) => formatBytes(row.original.upBytes),
    },
    {
      id: "down",
      header: "Down",
      accessorKey: "downBytes",
      cell: ({ row }) => formatBytes(row.original.downBytes),
    },
    {
      id: "duration",
      header: "Duration",
      accessorFn: (r) => r.durationMs,
      cell: ({ row }) => formatDuration(row.original),
    },
    {
      id: "url",
      header: "URL",
      accessorFn: (r) => r.host || r.address,
      cell: ({ row }) => row.original.host || row.original.address,
    },
  ];
}
