import { create } from "zustand";

type AppStore = {
  initialized: boolean;
  setInitialized: (value: boolean) => void;
};

export const useAppStore = create<AppStore>((set) => ({
  initialized: false,
  setInitialized: (value) => set({ initialized: value })
}));
