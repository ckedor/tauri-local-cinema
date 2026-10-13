import { HomeMediaItemDto } from "@/types/contracts/library";

export function getDisplayTitle(item: Pick<HomeMediaItemDto, "title" | "englishTitle">) {
  return item.englishTitle?.trim() || item.title;
}

export function getOriginalTitle(item: Pick<HomeMediaItemDto, "originalTitle">) {
  return item.originalTitle?.trim() || null;
}