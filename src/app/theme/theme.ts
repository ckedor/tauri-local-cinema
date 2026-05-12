import { createTheme } from "@mui/material/styles";

export const theme = createTheme({
  palette: {
    mode: "dark",
    primary: {
      main: "#b85563"
    },
    secondary: {
      main: "#edae49"
    },
    error: {
      main: "#a04050"
    },
    background: {
      default: "#111418",
      paper: "#1b1f24"
    },
    text: {
      primary: "rgba(255, 255, 255, 0.78)",
      secondary: "rgba(255, 255, 255, 0.55)"
    }
  },
  shape: {
    borderRadius: 14
  },
  typography: {
    fontFamily: '"Segoe UI", "Helvetica Neue", sans-serif',
    h1: { color: "rgba(255, 255, 255, 0.82)" },
    h2: { color: "rgba(255, 255, 255, 0.82)" },
    h3: { color: "rgba(255, 255, 255, 0.82)" },
    h4: { color: "rgba(255, 255, 255, 0.82)" },
    h5: { color: "rgba(255, 255, 255, 0.82)" },
    h6: { color: "rgba(255, 255, 255, 0.82)" }
  }
});
