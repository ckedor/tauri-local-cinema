import { Paper, PaperProps } from "@mui/material";
import { PropsWithChildren } from "react";

type SectionCardProps = PropsWithChildren<PaperProps>;

export function SectionCard({ children, sx, ...paperProps }: SectionCardProps) {
  return (
    <Paper
      {...paperProps}
      sx={{
        p: 3,
        backgroundColor: "rgba(27, 31, 36, 0.88)",
        border: "1px solid rgba(255, 255, 255, 0.06)",
        ...sx
      }}
    >
      {children}
    </Paper>
  );
}
