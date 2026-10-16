import { RefreshImageOptionDto } from "@/types/contracts/library";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import { Box, ButtonBase, Typography } from "@mui/material";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";

/** A row of artwork to pick from: the item's current image (when it has one)
 * and TMDb's options. `selectedUrl` null means the current image. */
export type ImagePickerProps = {
  currentPath: string | null;
  options: RefreshImageOptionDto[];
  selectedUrl: string | null;
  aspectRatio: string;
  height: number;
  minCurrentWidth: number;
  onSelect: (url: string | null) => void;
};

export function ImagePicker({ currentPath, options, selectedUrl, aspectRatio, height, minCurrentWidth, onSelect }: ImagePickerProps) {
  const [currentWidth, setCurrentWidth] = useState<number | null>(null);
  const currentUrl = useMemo(() => (currentPath ? convertFileSrc(currentPath) : null), [currentPath]);
  const proposedLowRes = useRef(false);

  // A soft current image: propose TMDb's best once, the user can still go back.
  useEffect(() => {
    if (currentWidth !== null && currentWidth < minCurrentWidth && !proposedLowRes.current && selectedUrl === null && options[0]) {
      proposedLowRes.current = true;
      onSelect(options[0].url);
    }
  }, [currentWidth]);

  if (!currentUrl && !options.length) {
    return <Typography sx={{ fontSize: "0.85rem", color: "text.secondary" }}>Nenhuma imagem no TMDb.</Typography>;
  }

  return (
    <Box sx={{ display: "flex", gap: 1, overflowX: "auto", pb: 1, scrollbarWidth: "thin" }}>
      {currentUrl ? (
        <ImageTile
          src={currentUrl}
          label={currentWidth ? `Atual · ${currentWidth}px` : "Atual"}
          isWarning={currentWidth !== null && currentWidth < minCurrentWidth}
          isSelected={selectedUrl === null}
          aspectRatio={aspectRatio}
          height={height}
          onLoadWidth={setCurrentWidth}
          onClick={() => onSelect(null)}
        />
      ) : null}
      {options.map((option) => (
        <ImageTile
          key={option.url}
          src={option.previewUrl}
          label={`${option.width}px${option.language ? ` · ${option.language}` : ""}`}
          isSelected={selectedUrl === option.url}
          aspectRatio={aspectRatio}
          height={height}
          onClick={() => onSelect(option.url)}
        />
      ))}
    </Box>
  );
}

type ImageTileProps = {
  src: string;
  label: string;
  isSelected: boolean;
  isWarning?: boolean;
  aspectRatio: string;
  height: number;
  onLoadWidth?: (width: number) => void;
  onClick: () => void;
};

function ImageTile({ src, label, isSelected, isWarning, aspectRatio, height, onLoadWidth, onClick }: ImageTileProps) {
  return (
    <ButtonBase
      onClick={onClick}
      sx={{
        position: "relative",
        flexShrink: 0,
        height,
        aspectRatio,
        borderRadius: 2,
        overflow: "hidden",
        outline: "3px solid",
        outlineColor: isSelected ? "secondary.main" : "transparent",
        outlineOffset: -3,
        transition: "transform 160ms ease, outline-color 120ms ease",
        ":hover": { transform: "translateY(-2px)" }
      }}
    >
      <Box
        component="img"
        src={src}
        alt=""
        loading="lazy"
        onLoad={(event) => onLoadWidth?.(event.currentTarget.naturalWidth)}
        sx={{ width: "100%", height: "100%", objectFit: "cover", display: "block", backgroundColor: "rgba(255,255,255,0.05)" }}
      />
      <Box
        sx={{
          position: "absolute",
          left: 0,
          right: 0,
          bottom: 0,
          px: 0.75,
          py: 0.25,
          fontSize: "0.66rem",
          fontWeight: 700,
          textAlign: "left",
          color: isWarning ? "warning.light" : "rgba(255,255,255,0.85)",
          background: "linear-gradient(to top, rgba(0,0,0,0.8), transparent)"
        }}
      >
        {label}
      </Box>
      {isSelected ? (
        <Box sx={{ position: "absolute", top: 6, right: 6, width: 20, height: 20, borderRadius: "50%", display: "grid", placeItems: "center", color: "#16120a", backgroundColor: "secondary.main" }}>
          <CheckRoundedIcon sx={{ fontSize: 14 }} />
        </Box>
      ) : null}
    </ButtonBase>
  );
}
