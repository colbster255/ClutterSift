import { invoke, isTauri } from "@tauri-apps/api/core";
export const native = isTauri();
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!native)
    throw new Error(
      "Open the Windows desktop app to scan your computer. The browser preview has no filesystem access.",
    );
  return invoke<T>(command, args);
}
export interface Settings {
  protected: string[];
  patterns: string[];
  exclusions: string[];
  theme: string;
  provider: string;
  model: string;
  cloud_consent: boolean;
}
export const defaults: Settings = {
  protected: [],
  patterns: [],
  exclusions: [],
  theme: "light",
  provider: "offline",
  model: "",
  cloud_consent: false,
};
export interface Progress {
  scan_id: string;
  files: number;
  bytes: number;
  candidates: number;
  candidate_bytes: number;
  errors: number;
  skipped: number;
  status: string;
}
export interface Item {
  id: string;
  path: string;
  bytes: number;
  modified: number;
  category: string;
  eligible: boolean;
  reason: string;
  duplicate_group: string | null;
}
export interface ScanView {
  progress: Progress;
  categories: { category: string; bytes: number; files: number }[];
  items: Item[];
  total: number;
}
export interface Drive {
  path: string;
  name: string;
  total: number;
  available: number;
  removable: boolean;
}
export interface Preview {
  id: string;
  items: Item[];
  bytes: number;
  executable: boolean;
  notice: string;
}
export interface Advice {
  summary: string;
  candidates: { id: string; explanation: string }[];
}
export interface RecoveryRecord {
  id: string;
  preview: string;
  created: number;
  item: Item;
  stored: string;
  status: string;
  error: string | null;
}
export interface CleanupResult {
  quarantined: number;
  failed: number;
  cancelled: boolean;
}
export interface LocalStatus {
  running: boolean;
  models: string[];
}
export function size(bytes: number): string {
  if (!bytes) return "0 B";
  const n = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), 4);
  return `${(bytes / 1024 ** n).toFixed(n === 0 ? 0 : 1)} ${["B", "KB", "MB", "GB", "TB"][n]}`;
}
