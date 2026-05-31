export const LIBRARY_EVENTS = {
  scanProgress: "library://scan-progress"
} as const;

export type LibraryScanProgressEvent = {
  phase: string;
  message: string;
  processed: number;
  total: number;
  percent: number;
};