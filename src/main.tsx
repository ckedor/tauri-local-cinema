import { AppProviders } from "@/app/providers/AppProviders";
import { router } from "@/app/router";
import "@/app/theme/global.css";
import { CssBaseline } from "@mui/material";
import { getCurrentWindow } from "@tauri-apps/api/window";
import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router-dom";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppProviders>
      <CssBaseline />
      <RouterProvider router={router} />
    </AppProviders>
  </React.StrictMode>
);

void revealWindowWhenReady();

async function revealWindowWhenReady() {
  const splashElement = document.getElementById("startup-splash");

  await waitForNextPaint();
  await waitForNextPaint();

  try {
    const currentWindow = getCurrentWindow();
    await currentWindow.setFullscreen(true).catch(() => undefined);
    await currentWindow.show().catch(() => undefined);
    await currentWindow.setFocus().catch(() => undefined);
  } catch {
    // Ignore when running outside the Tauri desktop shell.
  } finally {
    splashElement?.classList.add("is-hidden");
    window.setTimeout(() => splashElement?.remove(), 260);
  }
}

function waitForNextPaint() {
  return new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => resolve());
  });
}
