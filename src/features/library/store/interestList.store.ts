import {
    compareInterestEntries,
    createInterestEntry,
    InterestEntry,
    loadInterestList,
    persistInterestEntry,
    removePersistedInterestEntry,
    supportsInterestList
} from "@/features/library/services/interestList";
import { HomeMediaItemDto } from "@/types/contracts/library";
import { create } from "zustand";

type InterestListStore = {
  entries: InterestEntry[];
  hydrated: boolean;
  hydrate: () => Promise<void>;
  upsertItem: (item: HomeMediaItemDto) => void;
  removeEntry: (mediaId: string) => void;
  toggleItem: (item: HomeMediaItemDto) => void;
};

function persistInBackground(task: Promise<void>) {
  task.catch((error) => {
    console.error("Failed to persist portable new items state", error);
  });
}

function mergeInterestEntries(currentEntries: InterestEntry[], loadedEntries: InterestEntry[]) {
  const entriesById = new Map(loadedEntries.map((entry) => [entry.id, entry]));

  for (const entry of currentEntries) {
    entriesById.set(entry.id, entry);
  }

  return [...entriesById.values()].sort(compareInterestEntries);
}

export const useInterestListStore = create<InterestListStore>((set, get) => ({
  entries: [],
  hydrated: false,
  hydrate: async () => {
    if (get().hydrated) {
      return;
    }

    const entries = await loadInterestList();
    set((state) => ({
      entries: mergeInterestEntries(state.entries, entries),
      hydrated: true
    }));
  },
  upsertItem: (item) => {
    if (!supportsInterestList(item)) {
      return;
    }

    set((state) => {
      const currentEntry = state.entries.find((entry) => entry.id === item.id);
      const nextEntry = createInterestEntry(item, currentEntry?.addedAt ?? Date.now());
      const nextEntries = [
        nextEntry,
        ...state.entries.filter((entry) => entry.id !== item.id)
      ].sort(compareInterestEntries);

      persistInBackground(persistInterestEntry(nextEntry));

      return { entries: nextEntries };
    });
  },
  removeEntry: (mediaId) => {
    set((state) => {
      const currentEntry = state.entries.find((entry) => entry.id === mediaId);
      const nextEntries = state.entries.filter((entry) => entry.id !== mediaId);

      if (currentEntry) {
        persistInBackground(removePersistedInterestEntry(currentEntry));
      }

      return { entries: nextEntries };
    });
  },
  toggleItem: (item) => {
    if (!supportsInterestList(item)) {
      return;
    }

    set((state) => {
      const hasEntry = state.entries.some((entry) => entry.id === item.id);
      const nextEntry = createInterestEntry(item);
      const nextEntries = hasEntry
        ? state.entries.filter((entry) => entry.id !== item.id)
        : [nextEntry, ...state.entries].sort(compareInterestEntries);

      if (hasEntry) {
        const currentEntry = state.entries.find((entry) => entry.id === item.id);
        if (currentEntry) {
          persistInBackground(removePersistedInterestEntry(currentEntry));
        }
      } else {
        persistInBackground(persistInterestEntry(nextEntry));
      }

      return { entries: nextEntries };
    });
  }
}));