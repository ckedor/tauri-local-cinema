import {
    persistPlayerSession,
    PlayerSession,
    readStoredPlayerSession
} from "@/features/player/services/playerSession";
import { create } from "zustand";

type PlayerStore = {
  session: PlayerSession | null;
  setSession: (session: PlayerSession | null) => void;
};

export const usePlayerStore = create<PlayerStore>((set) => ({
  session: readStoredPlayerSession(),
  setSession: (session) => {
    persistPlayerSession(session);
    set({ session });
  }
}));
