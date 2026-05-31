import {
    listPortableNewItems,
    removePortableNewItem,
    upsertPortableNewItem
} from "@/services/tauri/commands/library";
import { HomeMediaItemDto, NewItemEntryDto } from "@/types/contracts/library";

export type InterestMediaType = "movie" | "show" | "concert" | "standup" | "documentary";

export type InterestEntry = NewItemEntryDto;

export const INTEREST_LIST_STORAGE_KEY = "netcrico.library.interests";

const INTEREST_MEDIA_TYPES: InterestMediaType[] = ["movie", "show", "concert", "standup", "documentary"];

export function supportsInterestList(itemOrMediaType: Pick<HomeMediaItemDto, "mediaType"> | string | null | undefined) {
  const mediaType = typeof itemOrMediaType === "string"
    ? itemOrMediaType
    : itemOrMediaType?.mediaType;

  return Boolean(mediaType && INTEREST_MEDIA_TYPES.includes(mediaType as InterestMediaType));
}

export function createInterestEntry(item: HomeMediaItemDto, addedAt = Date.now()): InterestEntry {
  return {
    ...item,
    addedAt
  };
}

export async function loadInterestList(): Promise<InterestEntry[]> {
  const portableEntries = (await listPortableNewItems()).sort(compareInterestEntries);

  if (portableEntries.length > 0) {
    clearLegacyInterestList();
    return portableEntries;
  }

  const legacyEntries = readLegacyStoredInterestList();
  if (!legacyEntries.length) {
    return [];
  }

  await Promise.all(
    legacyEntries.map((entry) => upsertPortableNewItem(entry.mediaPath, entry.mediaType, entry.addedAt))
  );
  clearLegacyInterestList();

  return legacyEntries;
}

export async function persistInterestEntry(entry: InterestEntry) {
  await upsertPortableNewItem(entry.mediaPath, entry.mediaType, entry.addedAt);
}

export async function removePersistedInterestEntry(entry: Pick<InterestEntry, "mediaPath">) {
  await removePortableNewItem(entry.mediaPath);
}

function readLegacyStoredInterestList(): InterestEntry[] {
  if (typeof window === "undefined") {
    return [];
  }

  const rawValue = window.localStorage.getItem(INTEREST_LIST_STORAGE_KEY);

  if (!rawValue) {
    return [];
  }

  try {
    const parsedValue = JSON.parse(rawValue);

    if (!Array.isArray(parsedValue)) {
      window.localStorage.removeItem(INTEREST_LIST_STORAGE_KEY);
      return [];
    }

    return parsedValue
      .map(normalizeInterestEntry)
      .filter((entry): entry is InterestEntry => entry !== null)
      .sort(compareInterestEntries);
  } catch {
    window.localStorage.removeItem(INTEREST_LIST_STORAGE_KEY);
    return [];
  }
}

export function compareInterestEntries(left: InterestEntry, right: InterestEntry) {
  return right.addedAt - left.addedAt;
}

function clearLegacyInterestList() {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.removeItem(INTEREST_LIST_STORAGE_KEY);
}

function normalizeInterestEntry(value: unknown): InterestEntry | null {
  if (!value || typeof value !== "object") {
    return null;
  }

  const item = value as Partial<InterestEntry>;

  if (
    typeof item.id !== "string"
    || typeof item.title !== "string"
    || typeof item.mediaType !== "string"
    || !supportsInterestList(item.mediaType)
    || typeof item.mediaPath !== "string"
  ) {
    return null;
  }

  return {
    id: item.id,
    title: item.title,
    mediaType: item.mediaType,
    year: typeof item.year === "number" ? item.year : null,
    posterPath: typeof item.posterPath === "string" ? item.posterPath : null,
    coverPath: typeof item.coverPath === "string" ? item.coverPath : null,
    overview: typeof item.overview === "string" ? item.overview : null,
    englishTitle: typeof item.englishTitle === "string" ? item.englishTitle : null,
    originalTitle: typeof item.originalTitle === "string" ? item.originalTitle : null,
    genre: typeof item.genre === "string" ? item.genre : null,
    director: typeof item.director === "string" ? item.director : null,
    actors: typeof item.actors === "string" ? item.actors : null,
    country: typeof item.country === "string" ? item.country : null,
    imdbRating: typeof item.imdbRating === "string" ? item.imdbRating : null,
    mediaPath: item.mediaPath,
    subtitlePath: typeof item.subtitlePath === "string" ? item.subtitlePath : null,
    addedAt: typeof item.addedAt === "number" ? item.addedAt : Date.now()
  };
}