import {
    VolumeDownRounded as VolumeDownRoundedIcon,
    VolumeOffRounded as VolumeOffRoundedIcon,
    VolumeUpRounded as VolumeUpRoundedIcon
} from "@mui/icons-material";
import { IconButton, Slider, Stack, Typography } from "@mui/material";
import { SxProps, Theme } from "@mui/material/styles";

type VolumeControlProps = {
  volume: number;
  isMuted: boolean;
  onChange: (nextVolume: number) => void;
  onToggleMuted: () => void;
  disabled?: boolean;
  sliderWidth?: number;
  sx?: SxProps<Theme>;
};

export function VolumeControl({
  volume,
  isMuted,
  onChange,
  onToggleMuted,
  disabled = false,
  sliderWidth = 120,
  sx
}: VolumeControlProps) {
  const normalizedVolume = Math.max(0, Math.min(100, Math.round(volume)));
  const shouldShowMutedIcon = isMuted || normalizedVolume === 0;

  return (
    <Stack direction="row" spacing={1} alignItems="center" sx={sx}>
      <IconButton
        aria-label={shouldShowMutedIcon ? "Ativar som" : "Silenciar"}
        disabled={disabled}
        onClick={onToggleMuted}
        sx={{ color: "rgba(255,255,255,0.88)" }}
      >
        {shouldShowMutedIcon ? (
          <VolumeOffRoundedIcon fontSize="small" />
        ) : normalizedVolume < 50 ? (
          <VolumeDownRoundedIcon fontSize="small" />
        ) : (
          <VolumeUpRoundedIcon fontSize="small" />
        )}
      </IconButton>

      <Slider
        aria-label={`Volume ${normalizedVolume}%`}
        color="secondary"
        disabled={disabled}
        min={0}
        max={100}
        step={1}
        value={normalizedVolume}
        onChange={(_, value) => onChange(Array.isArray(value) ? value[0] : value)}
        sx={{
          width: sliderWidth,
          color: "secondary.main",
          "& .MuiSlider-thumb": {
            width: 12,
            height: 12
          },
          "& .MuiSlider-rail": {
            opacity: 0.28
          }
        }}
      />

      <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.58)", minWidth: 36, textAlign: "right" }}>
        {normalizedVolume}%
      </Typography>
    </Stack>
  );
}