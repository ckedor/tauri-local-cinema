import { supportsInterestList } from "@/features/library/services/interestList";
import { useInterestListStore } from "@/features/library/store/interestList.store";
import { listHomeMedia } from "@/services/tauri/commands/library";
import { HomeMediaItemDto } from "@/types/contracts/library";

export async function runLibraryScanAndMarkNewItems<T>(scanAction: () => Promise<T>) {
  const previousItems = await readCurrentCatalogItems();
  const result = await scanAction();

  if (!previousItems.length) {
    return result;
  }

  const nextItems = await readCurrentCatalogItems();
  const previousIds = new Set(previousItems.map((item) => item.id));
  const upsertItem = useInterestListStore.getState().upsertItem;

  nextItems
    .filter((item) => supportsInterestList(item) && !previousIds.has(item.id))
    .forEach((item) => upsertItem(item));

  return result;
}

async function readCurrentCatalogItems() {
  return listHomeMedia().catch(() => [] as HomeMediaItemDto[]);
}