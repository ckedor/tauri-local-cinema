import { create } from "zustand";

type CatalogFiltersStore = {
  searchQuery: string;
  selectedDecade: number | null;
  setSearchQuery: (value: string) => void;
  setSelectedDecade: (value: number | null) => void;
  clearFilters: () => void;
};

export const useCatalogFiltersStore = create<CatalogFiltersStore>((set) => ({
  searchQuery: "",
  selectedDecade: null,
  setSearchQuery: (value) => set({ searchQuery: value }),
  setSelectedDecade: (value) => set({ selectedDecade: value }),
  clearFilters: () => set({ searchQuery: "", selectedDecade: null })
}));