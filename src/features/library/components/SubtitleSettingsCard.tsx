import { SectionCard } from "@/components/ui/SectionCard";
import { getSubtitleSettings, saveSubtitleSettings } from "@/services/tauri/commands/library";
import { SubtitleSettingsDto } from "@/types/contracts/library";
import { Alert, Button, Stack, TextField, Typography } from "@mui/material";
import { useEffect, useState } from "react";

/** OpenSubtitles account used by "Atualizar" > Legendas. */
export function SubtitleSettingsCard() {
  const [settings, setSettings] = useState<SubtitleSettingsDto>({ apiKey: null, username: null, password: null, languages: "pt-BR, en" });
  const [message, setMessage] = useState<{ severity: "success" | "error"; text: string } | null>(null);

  useEffect(() => {
    void getSubtitleSettings()
      .then(setSettings)
      .catch(() => setMessage({ severity: "error", text: "Nao foi possivel carregar as configuracoes de legenda." }));
  }, []);

  function update(key: keyof SubtitleSettingsDto, value: string) {
    setSettings((current) => ({ ...current, [key]: key === "languages" ? value : value || null }));
    setMessage(null);
  }

  async function handleSave() {
    try {
      await saveSubtitleSettings(settings);
      setMessage({ severity: "success", text: "Legendas salvas." });
    } catch {
      setMessage({ severity: "error", text: "Nao foi possivel salvar." });
    }
  }

  return (
    <SectionCard>
      <Stack spacing={2.5}>
        <Typography variant="h6">Legendas (OpenSubtitles)</Typography>
        <TextField
          helperText="Crie em opensubtitles.com > API consumers."
          label="OpenSubtitles API Key"
          onChange={(event) => update("apiKey", event.target.value)}
          spellCheck={false}
          value={settings.apiKey ?? ""}
        />
        <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
          <TextField
            fullWidth
            helperText="Opcional; com conta a cota diaria e maior."
            label="Usuario"
            onChange={(event) => update("username", event.target.value)}
            value={settings.username ?? ""}
          />
          <TextField fullWidth label="Senha" onChange={(event) => update("password", event.target.value)} type="password" value={settings.password ?? ""} />
        </Stack>
        <TextField
          helperText="Em ordem de preferencia, separados por virgula."
          label="Idiomas"
          onChange={(event) => update("languages", event.target.value)}
          value={settings.languages}
        />
        <Button onClick={() => void handleSave()} sx={{ alignSelf: "flex-start" }} variant="contained">
          Salvar legendas
        </Button>
        {message ? <Alert severity={message.severity}>{message.text}</Alert> : null}
      </Stack>
    </SectionCard>
  );
}
