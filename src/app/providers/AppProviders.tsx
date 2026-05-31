import { theme } from "@/app/theme/theme";
import { useInterestListStore } from "@/features/library/store/interestList.store";
import { ThemeProvider } from "@mui/material/styles";
import { PropsWithChildren, useEffect } from "react";

export function AppProviders({ children }: PropsWithChildren) {
  useEffect(() => {
    void useInterestListStore.getState().hydrate();
  }, []);

  return <ThemeProvider theme={theme}>{children}</ThemeProvider>;
}
