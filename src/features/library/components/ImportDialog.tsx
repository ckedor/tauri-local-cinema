import { ImagePicker } from "@/features/library/components/ImagePicker";
import { runLibraryScanAndMarkNewItems } from "@/features/library/services/newItemsSync";
import { importApply, importPreview, importRebuildPlan, pickImportFiles, pickLibraryRoot } from "@/services/tauri/commands/library";
import { LIBRARY_EVENTS, LibraryImportProgressEvent } from "@/services/tauri/events/library";
import { ImportCategory, ImportIdentityDto, ImportPlanDto } from "@/types/contracts/library";
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  LinearProgress,
  MenuItem,
  Stack,
  TextField,
  Typography
} from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";

const categoryOptions: { value: ImportCategory; label: string }[] = [
  { value: "movie", label: "Filme" },
  { value: "show", label: "Serie" },
  { value: "documentary", label: "Documentario" },
  { value: "standup", label: "Stand-up" },
  { value: "concert", label: "Show" }
];

type Stage = "drop" | "identifying" | "plan" | "copying" | "done";

type Step = { message: string; at: number };

// A step with no news for this long is probably stuck (Claude's own request
// times out at 150s).
const STALL_SECONDS = 60;

type ImportDialogProps = {
  open: boolean;
  onClose: () => void;
};

export function ImportDialog({ open, onClose }: ImportDialogProps) {
  const [stage, setStage] = useState<Stage>("drop");
  const [plan, setPlan] = useState<ImportPlanDto | null>(null);
  const [draft, setDraft] = useState<ImportIdentityDto | null>(null);
  // Picked apart from the identity: changing it doesn't need a new plan.
  const [artwork, setArtwork] = useState<{ posterUrl: string | null; coverUrl: string | null }>({ posterUrl: null, coverUrl: null });
  const [progress, setProgress] = useState<LibraryImportProgressEvent | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [isRebuilding, setIsRebuilding] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [doneMessage, setDoneMessage] = useState<string | null>(null);
  const [steps, setSteps] = useState<Step[]>([]);
  const [now, setNow] = useState(Date.now());
  // Bumped on every new run and on cancel, so a late answer from an abandoned
  // run is ignored.
  const runIdRef = useRef(0);
  // The drop listener outlives renders; it reads the current stage from here.
  const stageRef = useRef<Stage>(stage);
  stageRef.current = stage;

  const isBusy = stage === "identifying" || stage === "copying" || isRebuilding;
  // Each new plan starts from TMDb's best poster and cover.
  useEffect(() => {
    setArtwork({ posterUrl: plan?.posters[0]?.url ?? null, coverUrl: plan?.backdrops[0]?.url ?? null });
  }, [plan]);

  const isDirty = Boolean(plan && draft && JSON.stringify(plan.identity) !== JSON.stringify(draft));
  const hasConflict = Boolean(plan?.files.some((file) => file.action === "conflict"));

  // Native file drops only reach the app through the webview event (the HTML
  // drop event has no file paths in Tauri).
  useEffect(() => {
    if (!open) {
      return;
    }

    let active = true;
    let unlisten: (() => void) | undefined;

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setIsDragging(true);
          return;
        }

        setIsDragging(false);
        const acceptsDrop = stageRef.current === "drop" || stageRef.current === "plan";
        if (event.payload.type === "drop" && event.payload.paths.length > 0 && acceptsDrop) {
          void runPreview(event.payload.paths, null);
        }
      })
      .then((dispose) => {
        if (active) {
          unlisten = dispose;
        } else {
          dispose();
        }
      })
      .catch(() => {
        // Not running inside Tauri.
      });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    let active = true;
    let unlisten: (() => void) | undefined;

    void listen<LibraryImportProgressEvent>(LIBRARY_EVENTS.importProgress, (event) => {
      if (!active) {
        return;
      }

      if (event.payload.kind === "step") {
        setSteps((current) => [...current, { message: event.payload.message, at: Date.now() }]);
      } else {
        setProgress(event.payload);
      }
    }).then((dispose) => {
      if (active) {
        unlisten = dispose;
      } else {
        dispose();
      }
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [open]);

  // Ticks the step timers while something is running.
  useEffect(() => {
    if (stage !== "identifying" && stage !== "copying") {
      return;
    }

    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [stage]);

  async function runPreview(sourcePaths: string[], category: ImportCategory | null) {
    const runId = ++runIdRef.current;
    setErrorMessage(null);
    setProgress(null);
    setSteps([{ message: "Enviando para o backend...", at: Date.now() }]);
    setNow(Date.now());
    setStage("identifying");

    try {
      const nextPlan = await importPreview(sourcePaths, category);
      if (runId !== runIdRef.current) {
        return;
      }
      setPlan(nextPlan);
      setDraft(nextPlan.identity);
      setStage("plan");
    } catch (error) {
      if (runId !== runIdRef.current) {
        return;
      }
      setErrorMessage(asMessage(error, "Nao foi possivel analisar a pasta."));
      setStage(plan ? "plan" : "drop");
    }
  }

  function cancelIdentifying() {
    runIdRef.current += 1;
    setStage(plan ? "plan" : "drop");
  }

  async function handlePickFolder() {
    const path = await pickLibraryRoot().catch(() => null);
    if (path) {
      await runPreview([path], null);
    }
  }

  async function handlePickFiles() {
    const paths = await pickImportFiles().catch(() => [] as string[]);
    if (paths.length > 0) {
      await runPreview(paths, null);
    }
  }

  async function handleRebuild(identity: ImportIdentityDto) {
    setErrorMessage(null);
    setIsRebuilding(true);

    try {
      const nextPlan = await importRebuildPlan(identity);
      setPlan({ ...nextPlan, identifiedBy: plan?.identifiedBy ?? nextPlan.identifiedBy });
      setDraft(nextPlan.identity);
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel montar o plano."));
    } finally {
      setIsRebuilding(false);
    }
  }

  async function handleApply() {
    if (!plan) {
      return;
    }

    setErrorMessage(null);
    setProgress(null);
    setSteps([]);
    setNow(Date.now());
    setStage("copying");

    try {
      const folder = await runLibraryScanAndMarkNewItems(() => importApply({ ...plan.identity, ...artwork }));
      setDoneMessage(`Adicionado em ${folder}`);
      setStage("done");
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel adicionar."));
      setStage("plan");
    }
  }

  function handleClose() {
    if (stage === "copying") {
      return;
    }

    runIdRef.current += 1;
    setStage("drop");
    setSteps([]);
    setPlan(null);
    setDraft(null);
    setProgress(null);
    setErrorMessage(null);
    setDoneMessage(null);
    onClose();
  }

  function updateDraft(patch: Partial<ImportIdentityDto>) {
    setDraft((current) => (current ? { ...current, ...patch } : current));
  }

  const copyPercent = progress && progress.totalBytes > 0
    ? Math.min(100, Math.round((progress.copiedBytes / progress.totalBytes) * 100))
    : 0;

  return (
    <Dialog fullWidth maxWidth="md" onClose={handleClose} open={open}>
      <DialogTitle>Adicionar a biblioteca</DialogTitle>

      <DialogContent dividers>
        <Stack spacing={2}>
          {errorMessage ? <Alert severity="error">{errorMessage}</Alert> : null}

          {stage === "drop" ? (
            <Box
              sx={{
                border: "2px dashed",
                borderColor: isDragging ? "secondary.main" : "rgba(255,255,255,0.2)",
                borderRadius: 3,
                backgroundColor: isDragging ? "rgba(237, 174, 73, 0.08)" : "transparent",
                py: 8,
                textAlign: "center"
              }}
            >
              <Stack alignItems="center" spacing={2}>
                <Typography variant="h6">Arraste uma pasta ou arquivos para ca</Typography>
                <Typography color="text.secondary">
                  Filme, serie, temporada nova, episodios, documentario, stand-up ou show.
                </Typography>
                <Stack direction="row" spacing={1.5}>
                  <Button onClick={() => void handlePickFolder()} variant="outlined">
                    Escolher pasta
                  </Button>
                  <Button onClick={() => void handlePickFiles()} variant="outlined">
                    Escolher arquivos
                  </Button>
                </Stack>
              </Stack>
            </Box>
          ) : null}

          {stage === "identifying" ? <StepLog now={now} steps={steps} /> : null}

          {stage === "copying" ? (
            <Stack spacing={1.25} sx={{ py: 4 }}>
              <Stack direction="row" justifyContent="space-between" spacing={2}>
                <Typography color="text.secondary" noWrap>{progress?.message ?? "Copiando..."}</Typography>
                <Typography color="text.secondary">{copyPercent}%</Typography>
              </Stack>
              <LinearProgress value={copyPercent} variant="determinate" />
              {progress ? (
                <Typography color="text.secondary" variant="body2">
                  {formatBytes(progress.copiedBytes)} de {formatBytes(progress.totalBytes)}
                </Typography>
              ) : null}
              {steps.length > 0 ? <StepLog now={now} steps={steps} /> : null}
            </Stack>
          ) : null}

          {stage === "done" ? <Alert severity="success">{doneMessage}</Alert> : null}

          {stage === "plan" && plan && draft ? (
            <Stack spacing={2.5}>
              <Stack direction="row" spacing={2}>
                {plan.posterUrl ? (
                  <Box component="img" src={plan.posterUrl} alt="" sx={{ width: 110, borderRadius: 1.5, alignSelf: "flex-start" }} />
                ) : null}
                <Stack spacing={0.75} sx={{ minWidth: 0 }}>
                  <Typography variant="h6">{plan.tmdbTitle ?? draft.title}</Typography>
                  <Typography color="text.secondary" variant="body2" sx={{ wordBreak: "break-all" }}>
                    {draft.sourcePaths.length === 1 ? draft.sourcePaths[0] : `${draft.sourcePaths.length} arquivos`}
                  </Typography>
                  {plan.overview ? (
                    <Typography color="text.secondary" variant="body2" sx={{ display: "-webkit-box", WebkitLineClamp: 3, WebkitBoxOrient: "vertical", overflow: "hidden" }}>
                      {plan.overview}
                    </Typography>
                  ) : null}
                  <Stack direction="row" spacing={1}>
                    <Chip
                      label={plan.identifiedBy === "claude" ? "Identificado por Claude" : plan.identifiedBy === "heuristic" ? "Identificado pelo nome" : "Editado"}
                      size="small"
                    />
                    {plan.existingShow ? (
                      <Chip
                        color="secondary"
                        label={`Serie existente: ${plan.existingShow.title} (temporadas ${plan.existingShow.seasons.join(", ") || "-"})`}
                        onDelete={isBusy ? undefined : () => void handleRebuild({ ...draft, existingShowId: null })}
                        size="small"
                      />
                    ) : null}
                  </Stack>
                </Stack>
              </Stack>

              <Box sx={{ display: "grid", gap: 2, gridTemplateColumns: { xs: "1fr", sm: "1fr 2fr 1fr 1fr" } }}>
                <TextField
                  disabled={isBusy}
                  label="Categoria"
                  onChange={(event) => void runPreview(draft.sourcePaths, event.target.value as ImportCategory)}
                  select
                  size="small"
                  value={draft.category}
                >
                  {categoryOptions.map((option) => (
                    <MenuItem key={option.value} value={option.value}>{option.label}</MenuItem>
                  ))}
                </TextField>
                <TextField
                  disabled={isBusy}
                  label="Titulo"
                  onChange={(event) => updateDraft({ title: event.target.value })}
                  size="small"
                  value={draft.title}
                />
                <TextField
                  disabled={isBusy}
                  label="Ano"
                  onChange={(event) => updateDraft({ year: parseOptionalNumber(event.target.value) })}
                  size="small"
                  value={draft.year ?? ""}
                />
                <TextField
                  disabled={isBusy}
                  label="TMDb ID"
                  onChange={(event) => updateDraft({ tmdbId: event.target.value.trim() || null })}
                  size="small"
                  value={draft.tmdbId ?? ""}
                />
                {draft.category === "show" ? (
                  <TextField
                    disabled={isBusy}
                    helperText="Para arquivos sem temporada no nome"
                    label="Temporada padrao"
                    onChange={(event) => updateDraft({ defaultSeason: parseOptionalNumber(event.target.value) })}
                    size="small"
                    value={draft.defaultSeason ?? ""}
                  />
                ) : null}
                {!plan.existingShow && plan.libraryRoots.length > 1 ? (
                  <TextField
                    disabled={isBusy}
                    label="Pasta da biblioteca"
                    onChange={(event) => updateDraft({ libraryRoot: event.target.value })}
                    select
                    size="small"
                    sx={{ gridColumn: { sm: "span 2" } }}
                    value={draft.libraryRoot ?? ""}
                  >
                    {plan.libraryRoots.map((root) => (
                      <MenuItem key={root} value={root}>{root}</MenuItem>
                    ))}
                  </TextField>
                ) : null}
              </Box>

              {plan.backdrops.length ? (
                <Stack spacing={1}>
                  <Typography sx={{ fontWeight: 700 }}>Capa</Typography>
                  <ImagePicker
                    currentPath={null}
                    options={plan.backdrops}
                    selectedUrl={artwork.coverUrl}
                    aspectRatio="16 / 9"
                    height={88}
                    minCurrentWidth={0}
                    onSelect={(coverUrl) => setArtwork((current) => ({ ...current, coverUrl }))}
                  />
                </Stack>
              ) : null}

              {plan.posters.length ? (
                <Stack spacing={1}>
                  <Typography sx={{ fontWeight: 700 }}>Poster</Typography>
                  <ImagePicker
                    currentPath={null}
                    options={plan.posters}
                    selectedUrl={artwork.posterUrl}
                    aspectRatio="2 / 3"
                    height={120}
                    minCurrentWidth={0}
                    onSelect={(posterUrl) => setArtwork((current) => ({ ...current, posterUrl }))}
                  />
                </Stack>
              ) : null}

              {isDirty ? (
                <Button disabled={isBusy} onClick={() => void handleRebuild(draft)} sx={{ alignSelf: "flex-start" }} variant="outlined">
                  {isRebuilding ? "Atualizando..." : "Atualizar plano"}
                </Button>
              ) : null}

              {plan.notes.map((note) => (
                <Alert key={note} severity="info">{note}</Alert>
              ))}

              <Stack spacing={1}>
                <Typography variant="subtitle2">
                  Destino: <Box component="span" sx={{ color: "text.secondary", wordBreak: "break-all" }}>{plan.targetFolder}</Box>
                </Typography>
                <Box
                  sx={{
                    maxHeight: 260,
                    overflowY: "auto",
                    border: "1px solid rgba(255,255,255,0.08)",
                    borderRadius: 2,
                    p: 1.5,
                    fontFamily: "monospace",
                    fontSize: 13
                  }}
                >
                  {plan.files.map((file) => (
                    <Stack direction="row" key={file.source} spacing={1.5} sx={{ py: 0.4 }}>
                      <Box component="span" sx={{ color: actionColor(file.action), minWidth: 64 }}>{actionLabel(file.action)}</Box>
                      <Box component="span" sx={{ flexGrow: 1, wordBreak: "break-all" }}>
                        <Box component="span" sx={{ color: "text.secondary" }}>{fileName(file.source)}</Box>
                        {" -> "}
                        {relativeTo(file.target, plan.targetFolder)}
                      </Box>
                      <Box component="span" sx={{ color: "text.secondary", whiteSpace: "nowrap" }}>{formatBytes(file.sizeBytes)}</Box>
                    </Stack>
                  ))}
                </Box>
                <Typography color="text.secondary" variant="body2">
                  {plan.files.filter((file) => file.action === "copy").length} arquivo(s) para copiar, {formatBytes(plan.totalBytes)}. Os originais nao sao alterados.
                </Typography>
              </Stack>
            </Stack>
          ) : null}
        </Stack>
      </DialogContent>

      <DialogActions>
        {stage === "plan" ? (
          <Button color="inherit" disabled={isBusy} onClick={() => setStage("drop")}>
            Outra pasta
          </Button>
        ) : null}
        {stage === "identifying" ? (
          <Button color="inherit" onClick={cancelIdentifying}>
            Cancelar analise
          </Button>
        ) : null}
        <Button color="inherit" disabled={stage === "copying"} onClick={handleClose}>
          {stage === "done" ? "Fechar" : "Cancelar"}
        </Button>
        {stage === "plan" ? (
          <Button disabled={isBusy || isDirty || hasConflict} onClick={() => void handleApply()} variant="contained">
            {isDirty ? "Atualize o plano" : "Adicionar"}
          </Button>
        ) : null}
      </DialogActions>
    </Dialog>
  );
}

function StepLog({ steps, now }: { steps: Step[]; now: number }) {
  const current = steps[steps.length - 1];
  const currentSeconds = current ? Math.floor((now - current.at) / 1000) : 0;
  const totalSeconds = steps.length > 0 ? Math.floor((now - steps[0].at) / 1000) : 0;

  return (
    <Stack spacing={1.5} sx={{ py: 2 }}>
      <Stack spacing={0.75}>
        {steps.map((step, index) => {
          const isCurrent = index === steps.length - 1;
          const seconds = Math.floor(((isCurrent ? now : steps[index + 1].at) - step.at) / 1000);

          return (
            <Stack alignItems="center" direction="row" key={`${index}-${step.message}`} spacing={1.25}>
              <Box sx={{ width: 18, display: "flex", justifyContent: "center", flexShrink: 0 }}>
                {isCurrent ? <CircularProgress color="secondary" size={14} /> : <Box component="span" sx={{ color: "success.main" }}>✓</Box>}
              </Box>
              <Typography color={isCurrent ? "text.primary" : "text.secondary"} sx={{ flexGrow: 1 }} variant="body2">
                {step.message}
              </Typography>
              <Typography color="text.secondary" sx={{ fontVariantNumeric: "tabular-nums" }} variant="caption">
                {seconds}s
              </Typography>
            </Stack>
          );
        })}
      </Stack>
      <Typography color="text.secondary" variant="caption">Tempo total: {totalSeconds}s</Typography>
      {currentSeconds >= STALL_SECONDS ? (
        <Alert severity="warning">
          Esta etapa esta sem novidades ha {currentSeconds}s. Pode estar travada; cancele e tente de novo, ou veja o terminal (linhas [import]).
        </Alert>
      ) : null}
    </Stack>
  );
}

function actionLabel(action: string) {
  return action === "copy" ? "copiar" : action === "skip" ? "ja existe" : "conflito";
}

function actionColor(action: string) {
  return action === "copy" ? "success.main" : action === "skip" ? "text.secondary" : "error.main";
}

function parseOptionalNumber(raw: string) {
  const value = Number.parseInt(raw, 10);
  return Number.isFinite(value) ? value : null;
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

function relativeTo(path: string, folder: string) {
  return path.startsWith(folder) ? path.slice(folder.length).replace(/^[\\/]/, "") : path;
}

function formatBytes(bytes: number) {
  if (bytes >= 1e9) {
    return `${(bytes / 1e9).toFixed(1)} GB`;
  }

  if (bytes >= 1e6) {
    return `${Math.round(bytes / 1e6)} MB`;
  }

  return `${Math.round(bytes / 1e3)} KB`;
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
