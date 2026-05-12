import { PlayerSession, toPlayerSession } from "@/features/player/services/playerSession";
import { usePlayerStore } from "@/features/player/store/player.store";
import { HomeMediaItemDto } from "@/types/contracts/library";

export async function launchPlayer(item: HomeMediaItemDto) {
  return launchPlayerSession(toPlayerSession(item));
}

export async function launchPlayerSession(session: PlayerSession) {
  usePlayerStore.getState().setSession(session);
  // Inline player route inside the main window.
  window.location.hash = "#/player";
}
