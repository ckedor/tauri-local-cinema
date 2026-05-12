export type OpenPlayerWindowRequest = {
  mediaId?: string;
  mediaPath?: string;
  mediaTitle?: string;
  subtitlePath?: string | null;
};

export type PlayerSessionDto = {
  mediaId: string;
  mediaPath: string;
  mediaTitle: string;
  subtitlePath: string | null;
};

export type PlayerStatusDto = {
  session: PlayerSessionDto | null;
  isPlaying: boolean;
  processId: number | null;
  mpvPath: string | null;
  lastError: string | null;
};
