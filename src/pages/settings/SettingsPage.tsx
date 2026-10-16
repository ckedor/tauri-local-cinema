import { LibraryRootsField } from "@/components/ui/LibraryRootsField";
import { SectionCard } from "@/components/ui/SectionCard";
import { SubtitleSettingsCard } from "@/features/library/components/SubtitleSettingsCard";
import { runLibraryScanAndMarkNewItems } from "@/features/library/services/newItemsSync";
import {
  getInitialSetupStatus,
  getMetadataApiSettings,
  pickLibraryRoot,
  rescanLibrary,
  resetLibraryDatabaseAndRescan,
  saveLibraryRoots,
  saveMetadataApiSettings
} from "@/services/tauri/commands/library";
import { LIBRARY_EVENTS, LibraryScanProgressEvent } from "@/services/tauri/events/library";
import { appendLibraryRootPath, normalizeLibraryRootPaths } from "@/utils/libraryRoots";
import { Alert, Box, Button, CircularProgress, LinearProgress, Stack, TextField, Typography } from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export function SettingsPage() {
  const [libraryRootPaths, setLibraryRootPaths] = useState<string[]>([]);
  const [tmdbApiKey, setTmdbApiKey] = useState("");
  const [anthropicApiKey, setAnthropicApiKey] = useState("");
  const [scanProgress, setScanProgress] = useState<LibraryScanProgressEvent | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRescanning, setIsRescanning] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    void loadSettings();
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    void listen<LibraryScanProgressEvent>(LIBRARY_EVENTS.scanProgress, (event) => {
      if (!active) {
        return;
      }

      setScanProgress(event.payload);
    }).then((dispose) => {
      if (!active) {
        dispose();
        return;
      }

      unlisten = dispose;
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  async function loadSettings() {
    setIsLoading(true);

    try {
      const [status, apiSettings] = await Promise.all([getInitialSetupStatus(), getMetadataApiSettings()]);
      setLibraryRootPaths(status.libraryRootPaths);
      setTmdbApiKey(apiSettings.tmdbApiKey ?? "");
      setAnthropicApiKey(apiSettings.anthropicApiKey ?? "");
      setErrorMessage(null);
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel carregar as configuracoes."));
    } finally {
      setIsLoading(false);
    }
  }

  async function handlePickFolder() {
    try {
      const nextPath = await pickLibraryRoot();
      if (nextPath) {
        setLibraryRootPaths((currentPaths) => appendLibraryRootPath(currentPaths, nextPath));
        setMessage(null);
        setErrorMessage(null);
      }
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel abrir o seletor de pasta."));
    }
  }

  function handleRemoveFolder(pathIndex: number) {
    setLibraryRootPaths((currentPaths) => currentPaths.filter((_, index) => index !== pathIndex));
    setMessage(null);
  }

  async function handleSaveKeys() {
    setMessage(null);
    setErrorMessage(null);

    try {
      await saveMetadataApiSettings(tmdbApiKey, anthropicApiKey);
      setMessage("Chaves salvas.");
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel salvar as chaves."));
    }
  }

  async function handleSaveAndRescan() {
    await handleScan(async () => runLibraryScanAndMarkNewItems(() => rescanLibrary()));
  }

  async function handleHardReset() {
    await handleScan(() => resetLibraryDatabaseAndRescan(), true);
  }

  async function handleScan(runScan: () => Promise<number>, isHardReset = false) {
    const nextPaths = normalizeLibraryRootPaths(libraryRootPaths);
    if (nextPaths.length === 0) {
      setErrorMessage("Escolha ao menos uma pasta antes de reescanear a biblioteca.");
      return;
    }

    setMessage(null);
    setErrorMessage(null);
    setIsRescanning(true);
    setScanProgress({
      phase: "starting",
      message: "Preparando reescaneamento da biblioteca...",
      processed: 0,
      total: 1,
      percent: 0
    });

    try {
      await saveLibraryRoots(nextPaths);
      await saveMetadataApiSettings(tmdbApiKey, anthropicApiKey);
      const total = await runScan();
      setLibraryRootPaths(nextPaths);
      setMessage(
        isHardReset
          ? `Hard reset concluido com ${total} ${total === 1 ? "item" : "itens"}, substituindo metadata remota.`
          : `Biblioteca sincronizada com ${total} ${total === 1 ? "item" : "itens"}.`
      );
      setErrorMessage(null);
    } catch (error) {
      setErrorMessage(asMessage(error, isHardReset ? "Nao foi possivel executar o hard reset." : "Nao foi possivel reescanear a biblioteca."));
      setMessage(null);
      setScanProgress(null);
    } finally {
      setIsRescanning(false);
    }
  }

  const progressValue = Math.min(100, Math.max(0, scanProgress?.percent ?? 0));

  return (
    <Box sx={{ width: "100%", maxWidth: 920, mx: "auto", px: { xs: 2, md: 4 }, pb: 4 }}>
      <Stack spacing={3}>
        {isLoading ? (
          <SectionCard>
            <Stack alignItems="center" spacing={2} sx={{ py: 4 }}>
              <CircularProgress color="secondary" size={28} />
              <Typography color="text.secondary">Carregando configuracoes...</Typography>
            </Stack>
          </SectionCard>
        ) : (
          <>
            {errorMessage ? <Alert severity="error">{errorMessage}</Alert> : null}
            {message ? <Alert severity="success">{message}</Alert> : null}

            <SectionCard>
              <Stack spacing={3}>
                <Typography variant="h6">Pastas da biblioteca</Typography>
                <LibraryRootsField
                  disabled={isRescanning}
                  onAddPath={handlePickFolder}
                  onRemovePath={handleRemoveFolder}
                  paths={libraryRootPaths}
                />
              </Stack>
            </SectionCard>

            <SectionCard>
              <Stack spacing={2.5}>
                <Typography variant="h6">Chaves de metadata</Typography>
                <TextField
                  disabled={isRescanning}
                  helperText="Usada para posters, genero, atores, diretor, nota e nacionalidade."
                  label="TMDb API Key"
                  onChange={(event) => setTmdbApiKey(event.target.value)}
                  type="password"
                  value={tmdbApiKey}
                />
                <TextField
                  disabled={isRescanning}
                  helperText="Opcional. Usada pelo botao + para identificar pastas novas (titulo, TMDb, nova temporada)."
                  label="Anthropic API Key"
                  onChange={(event) => setAnthropicApiKey(event.target.value)}
                  type="password"
                  value={anthropicApiKey}
                />
                <Button disabled={isRescanning} onClick={() => void handleSaveKeys()} sx={{ alignSelf: "flex-start" }} variant="contained">
                  Salvar chaves
                </Button>
                <Alert severity="info">
                  O reescaneamento mantem o que continua na base, adiciona titulos novos, remove o que sumiu do disco e usa essas chaves salvas no banco para metadata e poster.
                </Alert>
              </Stack>
            </SectionCard>

            <SubtitleSettingsCard />

            <SectionCard>
              <Stack spacing={2.5}>
                <Typography variant="h6">Reescaneamento</Typography>
                {scanProgress ? (
                  <Stack spacing={1.25}>
                    <Stack direction="row" justifyContent="space-between" spacing={2}>
                      <Typography color="text.secondary">{scanProgress.message}</Typography>
                      <Typography color="text.secondary">{progressValue}%</Typography>
                    </Stack>
                    <LinearProgress value={progressValue} variant="determinate" />
                    <Typography color="text.secondary" variant="body2">
                      {scanProgress.processed} de {scanProgress.total} etapas concluídas.
                    </Typography>
                  </Stack>
                ) : <></>}

                <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
                  <Button disabled={libraryRootPaths.length === 0 || isRescanning} onClick={handleSaveAndRescan} variant="contained">
                    {isRescanning ? `Reescaneando biblioteca... ${progressValue}%` : "Salvar pastas e reescanear biblioteca"}
                  </Button>
                  <Button color="error" disabled={libraryRootPaths.length === 0 || isRescanning} onClick={handleHardReset} variant="outlined">
                    {isRescanning ? "Executando hard reset..." : "Hard reset: apagar base e substituir metadata"}
                  </Button>
                </Stack>
              </Stack>
            </SectionCard>
          </>
        )}
      </Stack>
    </Box>
  );
}

function asMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "string" && error) {
    return error;
  }

  return fallback;
}
