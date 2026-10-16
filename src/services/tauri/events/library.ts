export const LIBRARY_EVENTS = {
  scanProgress: "library://scan-progress",
  importProgress: "library://import-progress",
  refreshProgress: "library://refresh-progress"
} as const;

export type LibraryRefreshProgressEvent = {
  message: string;
};

export type LibraryImportProgressEvent = {
  kind: "step" | "copy";
  message: string;
  copiedBytes: number;
  totalBytes: number;
};

export type LibraryScanProgressEvent = {
  phase: string;
  message: string;
  processed: number;
  total: number;
  percent: number;
};