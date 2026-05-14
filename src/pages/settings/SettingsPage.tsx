import { LibraryRootsField } from "@/components/ui/LibraryRootsField";
import { SectionCard } from "@/components/ui/SectionCard";
import {
    getInitialSetupStatus,
    pickLibraryRoot,
    rescanLibrary,
    saveLibraryRoots
} from "@/services/tauri/commands/library";
import { normalizeLibraryRootPaths, appendLibraryRootPath } from "@/utils/libraryRoots";
import { Alert, Button, CircularProgress, Stack, Typography } from "@mui/material";
import { useEffect, useState } from "react";

export function SettingsPage() {
  const [libraryRootPaths, setLibraryRootPaths] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isRescanning, setIsRescanning] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    void loadSettings();
  }, []);

  async function loadSettings() {
    setIsLoading(true);

    try {
      const status = await getInitialSetupStatus();
      setLibraryRootPaths(status.libraryRootPaths);
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

  async function handleSaveAndRescan() {
    const nextPaths = normalizeLibraryRootPaths(libraryRootPaths);
    if (nextPaths.length === 0) {
      setErrorMessage("Escolha ao menos uma pasta antes de reescanear tudo.");
      return;
    }

    setIsRescanning(true);

    try {
      await saveLibraryRoots(nextPaths);
      const total = await rescanLibrary();
      setLibraryRootPaths(nextPaths);
      setMessage(`Biblioteca reindexada do zero com ${total} ${total === 1 ? "item" : "itens"}.`);
      setErrorMessage(null);
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel reescanear a biblioteca."));
      setMessage(null);
    } finally {
      setIsRescanning(false);
    }
  }

  return (
    <Stack spacing={3}>
      <SectionCard>
        <Stack spacing={3}>
          {isLoading ? (
            <Stack alignItems="center" spacing={2} sx={{ py: 4 }}>
              <CircularProgress color="secondary" size={28} />
              <Typography color="text.secondary">Carregando configuracoes...</Typography>
            </Stack>
          ) : (
            <>
              {errorMessage ? <Alert severity="error">{errorMessage}</Alert> : null}
              {message ? <Alert severity="success">{message}</Alert> : null}

              <LibraryRootsField
                disabled={isRescanning}
                onAddPath={handlePickFolder}
                onRemovePath={handleRemoveFolder}
                paths={libraryRootPaths}
              />

              <Alert severity="info">
                O reescaneamento sempre apaga o banco atual e reconstrui a biblioteca inteira a partir das pastas salvas.
              </Alert>

              <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
                <Button disabled={libraryRootPaths.length === 0 || isRescanning} onClick={handleSaveAndRescan} variant="contained">
                  {isRescanning ? "Reescaneando biblioteca..." : "Salvar pastas e reescanear tudo"}
                </Button>
              </Stack>
            </>
          )}
        </Stack>
      </SectionCard>
    </Stack>
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
