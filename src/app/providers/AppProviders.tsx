import { theme } from "@/app/theme/theme";
import { ThemeProvider } from "@mui/material/styles";
import { PropsWithChildren } from "react";

export function AppProviders({ children }: PropsWithChildren) {
  return <ThemeProvider theme={theme}>{children}</ThemeProvider>;
}
