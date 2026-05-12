import { SectionCard } from "@/components/ui/SectionCard";
import {
    getInitialSetupStatus,
    pickLibraryRoot,
    resetLibraryDatabaseAndRescan,
    saveLibraryRoot,
    startInitialScan
} from "@/services/tauri/commands/library";
import { Alert, Button, CircularProgress, Stack, TextField, Typography } from "@mui/material";
import { useEffect, useState } from "react";

export function SettingsPage() {
  const [libraryRootPath, setLibraryRootPath] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    void loadSettings();
  }, []);

  async function loadSettings() {
    setIsLoading(true);

    try {
      const status = await getInitialSetupStatus();
      setLibraryRootPath(status.libraryRootPath ?? "");
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
        setLibraryRootPath(nextPath);
        setMessage(null);
        setErrorMessage(null);
      }
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel abrir o seletor de pasta."));
    }
  }

  async function handleSaveAndScan() {
    if (!libraryRootPath.trim()) {
      setErrorMessage("Escolha uma pasta antes de salvar.");
      return;
    }

    setIsSaving(true);

    try {
      await saveLibraryRoot(libraryRootPath.trim());
      const total = await startInitialScan();
      setMessage(`${total} ${total === 1 ? "item indexado" : "itens indexados"}.`);
      setErrorMessage(null);
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel atualizar a biblioteca."));
      setMessage(null);
    } finally {
      setIsSaving(false);
    }
  }

  async function handleResetAndRescan() {
    if (!libraryRootPath.trim()) {
      setErrorMessage("Escolha uma pasta antes de reescanear tudo.");
      return;
    }

    setIsResetting(true);

    try {
      const total = await resetLibraryDatabaseAndRescan();
      setMessage(`Banco apagado e biblioteca reindexada com ${total} ${total === 1 ? "item" : "itens"}.`);
      setErrorMessage(null);
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel apagar o banco e reescanear a biblioteca."));
      setMessage(null);
    } finally {
      setIsResetting(false);
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

              <TextField
                fullWidth
                label="Pasta da biblioteca"
                value={libraryRootPath}
                InputProps={{ readOnly: true }}
                placeholder="Selecione uma pasta"
              />

              <Alert severity="warning">
                "Apagar banco e reescanear tudo" remove todos os registros atuais e reconstrui a biblioteca do zero.
              </Alert>

              <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
                <Button color="secondary" onClick={handlePickFolder} variant="outlined">
                  Escolher pasta
                </Button>
                <Button disabled={!libraryRootPath.trim() || isSaving} onClick={handleSaveAndScan} variant="contained">
                  {isSaving ? "Salvando e escaneando..." : "Salvar e reescanear"}
                </Button>
                <Button
                  color="error"
                  disabled={!libraryRootPath.trim() || isResetting}
                  onClick={handleResetAndRescan}
                  variant="contained"
                >
                  {isResetting ? "Apagando banco e reescaneando..." : "Apagar banco e reescanear tudo"}
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
