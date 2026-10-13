import { create } from "zustand";

export type CatalogSortOrder = "title" | "yearDesc" | "yearAsc" | "ratingDesc" | "ratingAsc";

type CatalogFiltersStore = {
  searchQuery: string;
  selectedDecade: number | null;
  selectedGenres: string[];
  selectedCountries: string[];
  sortOrder: CatalogSortOrder;
  setSearchQuery: (value: string) => void;
  setSelectedDecade: (value: number | null) => void;
  setSortOrder: (value: CatalogSortOrder) => void;
  toggleGenre: (value: string) => void;
  toggleCountry: (value: string) => void;
  clearFilters: () => void;
};

export const useCatalogFiltersStore = create<CatalogFiltersStore>((set) => ({
  searchQuery: "",
  selectedDecade: null,
  selectedGenres: [],
  selectedCountries: [],
  sortOrder: "yearDesc",
  setSearchQuery: (value) => set({ searchQuery: value }),
  setSelectedDecade: (value) => set({ selectedDecade: value }),
  setSortOrder: (value) => set({ sortOrder: value }),
  toggleGenre: (value) => set((state) => ({
    selectedGenres: state.selectedGenres.includes(value)
      ? state.selectedGenres.filter((entry) => entry !== value)
      : [...state.selectedGenres, value]
  })),
  toggleCountry: (value) => set((state) => ({
    selectedCountries: state.selectedCountries.includes(value)
      ? state.selectedCountries.filter((entry) => entry !== value)
      : [...state.selectedCountries, value]
  })),
  clearFilters: () => set({
    searchQuery: "",
    selectedDecade: null,
    selectedGenres: [],
    selectedCountries: [],
    sortOrder: "yearDesc"
  })
}));