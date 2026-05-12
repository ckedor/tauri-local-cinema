import {
  ContinueWatchingEntry,
  persistContinueWatching,
  readStoredContinueWatching
} from "@/features/player/services/continueWatching";
import { create } from "zustand";

type ContinueWatchingStore = {
  entries: ContinueWatchingEntry[];
  upsertEntry: (entry: ContinueWatchingEntry) => void;
  removeEntry: (entryId: string) => void;
};

export const useContinueWatchingStore = create<ContinueWatchingStore>((set) => ({
  entries: readStoredContinueWatching(),
  upsertEntry: (entry) => {
    set((state) => {
      const nextEntries = [entry, ...state.entries.filter((currentEntry) => currentEntry.entryId !== entry.entryId)]
        .sort((left, right) => right.updatedAt - left.updatedAt);

      persistContinueWatching(nextEntries);

      return { entries: nextEntries };
    });
  },
  removeEntry: (entryId) => {
    set((state) => {
      const nextEntries = state.entries.filter((entry) => entry.entryId !== entryId);

      persistContinueWatching(nextEntries);

      return { entries: nextEntries };
    });
  }
}));