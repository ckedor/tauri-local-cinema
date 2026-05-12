import { HomeMediaItemDto } from "@/types/contracts/library";

export function formatMediaType(mediaType: string) {
  switch (mediaType) {
    case "movie":
      return "Filme";
    case "show":
      return "Serie";
    case "concert":
      return "Concerto";
    case "documentary":
      return "Documentario";
    case "standup":
      return "Stand-up";
    case "music_artist":
      return "Artista";
    case "music_album":
      return "Album";
    case "music_track":
      return "Faixa";
    default:
      return mediaType;
  }
}

export function getMediaFormat(mediaPath: string) {
  const extension = mediaPath.split(".").pop()?.trim().toLowerCase();
  return extension ? extension.toUpperCase() : "Desconhecido";
}

export function getPlaceholderDescription(item: HomeMediaItemDto) {
  if (item.mediaType === "music_artist") {
    return "";
  }

  if (item.mediaType === "music_album") {
    return "";
  }

  const mediaTypeLabel = formatMediaType(item.mediaType).toLocaleLowerCase();

  return `${item.title} aparece como ${mediaTypeLabel} em destaque na sua biblioteca local. A sinopse detalhada entra quando a etapa de metadados estiver ligada.`;
}