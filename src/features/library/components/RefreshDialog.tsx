import { useContinueWatchingStore } from "@/features/player/store/continueWatching.store";
import { downloadSubtitles, refreshApply, refreshPreview, searchSeasonSubtitles, syncExistingSubtitles } from "@/services/tauri/commands/library";
import { LIBRARY_EVENTS, LibraryRefreshProgressEvent } from "@/services/tauri/events/library";
import {
  EXTRAS_SEASON,
  HomeMediaItemDto,
  RefreshPlanDto,
  SeasonSubtitlesDto,
  SubtitleSyncSummaryDto
} from "@/types/contracts/library";
import { ImagePicker } from "@/features/library/components/ImagePicker";
import AutoAwesomeRoundedIcon from "@mui/icons-material/AutoAwesomeRounded";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import {
  Alert,
  Box,
  Button,
  Checkbox,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  MenuItem,
  Stack,
  Tab,
  Tabs,
  TextField,
  Typography
} from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type ReactNode } from "react";

type RefreshDialogProps = {
  open: boolean;
  initialTab: "metadata" | "subtitles";
  item: HomeMediaItemDto;
  /** A show's seasons; empty for a movie, concert, stand-up or documentary. */
  seasons: number[];
  initialSeason: number | null;
  onClose: () => void;
  onApplied: () => void;
};

// Smaller than this, the current image looks soft at full screen, so the
// dry run proposes TMDb's best one instead of keeping it.
const MIN_COVER_WIDTH = 1280;
const MIN_POSTER_WIDTH = 500;

export function RefreshDialog({ open, initialTab, item, seasons, initialSeason, onClose, onApplied }: RefreshDialogProps) {
  const [tab, setTab] = useState<"metadata" | "subtitles">("metadata");
  const [steps, setSteps] = useState<string[]>([]);
  const [isBusy, setIsBusy] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [doneMessage, setDoneMessage] = useState<string | null>(null);

  const [plan, setPlan] = useState<RefreshPlanDto | null>(null);
  const [tmdbIdDraft, setTmdbIdDraft] = useState("");
  const [selectedFields, setSelectedFields] = useState<Set<string>>(new Set());
  const [selectedRenames, setSelectedRenames] = useState<Set<string>>(new Set());
  // null keeps the current image.
  const [coverUrl, setCoverUrl] = useState<string | null>(null);
  const [posterUrl, setPosterUrl] = useState<string | null>(null);

  // "all" covers every episode of the show.
  const [subtitleSeason, setSubtitleSeason] = useState<number | "all">("all");
  const [subtitles, setSubtitles] = useState<SeasonSubtitlesDto | null>(null);
  const [selectedOffers, setSelectedOffers] = useState<Set<string>>(new Set());

  const continueEntries = useContinueWatchingStore((state) => state.entries);
  const upsertContinueEntry = useContinueWatchingStore((state) => state.upsertEntry);
  // A late answer from a closed or restarted run is ignored.
  const runIdRef = useRef(0);

  useEffect(() => {
    if (!open) {
      return;
    }

    let active = true;
    let unlisten: (() => void) | undefined;
    void listen<LibraryRefreshProgressEvent>(LIBRARY_EVENTS.refreshProgress, (event) => {
      if (active) {
        setSteps((current) => [...current.slice(-40), event.payload.message]);
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

  useEffect(() => {
    if (!open) {
      runIdRef.current += 1;
      return;
    }

    setTab(initialTab);
    setPlan(null);
    setSubtitles(null);
    setDoneMessage(null);
    setErrorMessage(null);
    setSubtitleSeason(initialTab === "subtitles" || initialSeason === null ? "all" : initialSeason);
    // The dry run calls TMDb and maybe Claude: only when its tab is shown.
    if (initialTab === "metadata") {
      void runPreview(null);
    }
  }, [open, item.id]);

  async function runPreview(tmdbId: string | null) {
    const runId = ++runIdRef.current;
    setIsBusy(true);
    setErrorMessage(null);
    setSteps([]);

    try {
      const nextPlan = await refreshPreview(item.id, tmdbId);
      if (runId !== runIdRef.current) {
        return;
      }

      setPlan(nextPlan);
      setTmdbIdDraft(nextPlan.tmdbId);
      setSelectedFields(new Set(nextPlan.fields.map((field) => field.key)));
      setSelectedRenames(new Set(nextPlan.renames.filter((rename) => !rename.conflict).map((rename) => rename.from)));
      setCoverUrl(nextPlan.currentCover ? null : nextPlan.backdrops[0]?.url ?? null);
      setPosterUrl(nextPlan.currentPoster ? null : nextPlan.posters[0]?.url ?? null);
    } catch (error) {
      if (runId === runIdRef.current) {
        setErrorMessage(asMessage(error));
      }
    } finally {
      if (runId === runIdRef.current) {
        setIsBusy(false);
      }
    }
  }

  async function handleApply() {
    if (!plan) {
      return;
    }

    const runId = ++runIdRef.current;
    setIsBusy(true);
    setErrorMessage(null);
    setSteps([]);

    try {
      const result = await refreshApply({
        mediaId: plan.mediaId,
        fields: plan.fields.filter((field) => selectedFields.has(field.key)).map((field) => ({ key: field.key, value: field.proposed })),
        posterUrl,
        coverUrl,
        moves: plan.renames
          .filter((rename) => selectedRenames.has(rename.from))
          .flatMap((rename) => [{ from: rename.from, to: rename.to }, ...rename.companions])
      });

      // Episode ids are file paths: keep "Continuar" pointing at the renamed file.
      for (const entry of continueEntries) {
        const move = result.moved.find((candidate) => candidate.from === entry.mediaPath);
        if (move) {
          upsertContinueEntry({ ...entry, mediaId: move.to, mediaPath: move.to });
        }
      }

      if (runId === runIdRef.current) {
        setPlan(null);
        setDoneMessage(changeCount > 0 ? `${changeCount} ${changeCount === 1 ? "mudança aplicada" : "mudanças aplicadas"}.` : "Nada mudou.");
      }
      onApplied();
    } catch (error) {
      if (runId === runIdRef.current) {
        setErrorMessage(asMessage(error));
      }
    } finally {
      if (runId === runIdRef.current) {
        setIsBusy(false);
      }
    }
  }

  async function handleSearchSubtitles() {
    const runId = ++runIdRef.current;
    setIsBusy(true);
    setErrorMessage(null);
    setSteps([]);
    setSubtitles(null);

    try {
      const result = await searchSeasonSubtitles(item.id, subtitleSeason === "all" ? null : subtitleSeason);
      if (runId === runIdRef.current) {
        setSubtitles(result);
        setSelectedOffers(new Set(result.offers.map(offerKey)));
      }
    } catch (error) {
      if (runId === runIdRef.current) {
        setErrorMessage(asMessage(error));
      }
    } finally {
      if (runId === runIdRef.current) {
        setIsBusy(false);
      }
    }
  }

  // Fixes the files that are already there, not just new downloads.
  async function handleSyncExisting() {
    const runId = ++runIdRef.current;
    setIsBusy(true);
    setErrorMessage(null);
    setDoneMessage(null);
    setSteps([]);

    try {
      const summary = await syncExistingSubtitles(item.id, subtitleSeason === "all" ? null : subtitleSeason);
      if (runId === runIdRef.current) {
        const total = summary.synced + summary.alreadyInSync + summary.noReference;
        setDoneMessage(total === 0 ? "Nenhuma legenda baixada para sincronizar." : `${total} ${total === 1 ? "legenda verificada" : "legendas verificadas"}${describeSync(summary)}.`);
        if (summary.notes.length) {
          setErrorMessage(summary.notes.join("\n"));
        }
      }
    } catch (error) {
      if (runId === runIdRef.current) {
        setErrorMessage(asMessage(error));
      }
    } finally {
      if (runId === runIdRef.current) {
        setIsBusy(false);
      }
    }
  }

  async function handleDownloadSubtitles() {
    if (!subtitles) {
      return;
    }

    const runId = ++runIdRef.current;
    setIsBusy(true);
    setErrorMessage(null);
    setSteps([]);

    try {
      const offers = subtitles.offers.filter((offer) => selectedOffers.has(offerKey(offer)));
      const result = await downloadSubtitles(
        item.id,
        offers.map((offer) => ({ fileId: offer.fileId, targetPath: offer.targetPath, episodeId: offer.episodeId, embeddedTrack: offer.embeddedTrack }))
      );
      if (runId === runIdRef.current) {
        setSubtitles(null);
        setDoneMessage(
          [
            result.downloaded > 0 ? `${result.downloaded} ${result.downloaded === 1 ? "legenda baixada" : "legendas baixadas"}` : null,
            result.extracted > 0 ? `${result.extracted} ${result.extracted === 1 ? "extraída" : "extraídas"} do vídeo` : null
          ].filter(Boolean).join(" · ")
            + (result.remaining !== null && result.downloaded > 0 ? ` · restam ${result.remaining} hoje` : "")
            + describeSync(result.sync)
            + "."
        );
        const problems = [...result.errors, ...result.sync.notes];
        if (problems.length) {
          setErrorMessage(problems.join("\n"));
        }
      }
      if (result.downloaded > 0) {
        onApplied();
      }
    } catch (error) {
      if (runId === runIdRef.current) {
        setErrorMessage(asMessage(error));
      }
    } finally {
      if (runId === runIdRef.current) {
        setIsBusy(false);
      }
    }
  }

  const changeCount = plan
    ? selectedFields.size + selectedRenames.size + (coverUrl ? 1 : 0) + (posterUrl ? 1 : 0)
    : 0;
  const subtitleSeasons = seasons.filter((season) => season !== EXTRAS_SEASON);
  const isShow = item.mediaType === "show";

  return (
    <Dialog open={open} onClose={isBusy ? undefined : onClose} fullWidth maxWidth="md" PaperProps={{ sx: { borderRadius: 4, backgroundImage: "none", backgroundColor: "#15181d" } }}>
      <Box sx={{ px: 3, pt: 2.5 }}>
        <Typography sx={{ fontSize: "1.4rem", fontWeight: 800, color: "common.white", letterSpacing: "-0.01em" }}>Atualizar {item.title}</Typography>
        <Tabs
          value={tab}
          onChange={(_, value) => {
            setTab(value);
            setErrorMessage(null);
            setDoneMessage(null);
            if (value === "metadata" && !plan) {
              void runPreview(null);
            }
          }}
          textColor="inherit"
          sx={{ minHeight: 40, mt: 1, ".MuiTabs-indicator": { backgroundColor: "secondary.main", height: 3, borderRadius: 3 }, ".MuiTab-root": { minHeight: 40, textTransform: "none", fontWeight: 700 } }}
        >
          <Tab value="metadata" label="Metadata, imagens e episódios" disabled={isBusy} />
          <Tab value="subtitles" label="Legendas" disabled={isBusy} />
        </Tabs>
      </Box>

      <DialogContent sx={{ px: 3, pt: 2, minHeight: 360 }}>
        <Stack spacing={2.5}>
          {errorMessage ? <Alert severity="warning" sx={{ whiteSpace: "pre-line" }}>{errorMessage}</Alert> : null}
          {doneMessage ? (
            <Alert icon={<CheckRoundedIcon />} severity="success">
              {doneMessage}
            </Alert>
          ) : null}

          {isBusy ? <ProgressLog steps={steps} /> : null}

          {!isBusy && tab === "metadata" && plan ? (
            <>
              <Stack direction="row" spacing={1.5} alignItems="center">
                <Typography color="text.secondary" sx={{ flex: 1 }}>
                  TMDb: <Box component="span" sx={{ color: "common.white", fontWeight: 700 }}>{plan.tmdbTitle}{plan.tmdbYear ? ` (${plan.tmdbYear})` : ""}</Box>
                </Typography>
                <TextField size="small" label="TMDb ID" value={tmdbIdDraft} onChange={(event) => setTmdbIdDraft(event.target.value)} sx={{ width: 130 }} />
                <Button variant="outlined" size="small" disabled={!tmdbIdDraft.trim() || tmdbIdDraft.trim() === plan.tmdbId} onClick={() => void runPreview(tmdbIdDraft.trim())}>
                  Usar este
                </Button>
              </Stack>

              <Section title="Capa" hint="Fundo da página">
                <ImagePicker
                  currentPath={plan.currentCover}
                  options={plan.backdrops}
                  selectedUrl={coverUrl}
                  aspectRatio="16 / 9"
                  height={96}
                  minCurrentWidth={MIN_COVER_WIDTH}
                  onSelect={setCoverUrl}
                />
              </Section>

              <Section title="Poster" hint="Usado nos cards">
                <ImagePicker
                  currentPath={plan.currentPoster}
                  options={plan.posters}
                  selectedUrl={posterUrl}
                  aspectRatio="2 / 3"
                  height={132}
                  minCurrentWidth={MIN_POSTER_WIDTH}
                  onSelect={setPosterUrl}
                />
              </Section>

              <Section title="Metadata" hint={plan.fields.length ? undefined : "Tudo igual ao TMDb"}>
                <Stack spacing={0.5}>
                  {plan.fields.map((field) => (
                    <CheckRow key={field.key} checked={selectedFields.has(field.key)} onToggle={() => setSelectedFields((current) => toggle(current, field.key))}>
                      <Typography sx={{ width: 120, flexShrink: 0, color: "text.secondary", fontSize: "0.85rem", pt: 0.25 }}>{field.label}</Typography>
                      <Stack spacing={0.25} sx={{ minWidth: 0 }}>
                        {field.current ? (
                          <Typography sx={{ fontSize: "0.85rem", color: "rgba(255,255,255,0.38)", textDecoration: "line-through", ...clampLines(2) }}>{field.current}</Typography>
                        ) : null}
                        <Typography sx={{ fontSize: "0.9rem", color: "common.white", ...clampLines(3) }}>{field.proposed}</Typography>
                      </Stack>
                    </CheckRow>
                  ))}
                </Stack>
              </Section>

              {isShow ? (
                <Section
                  title="Nomes dos episódios"
                  hint={plan.renames.length ? `${plan.renames.length} arquivo${plan.renames.length === 1 ? "" : "s"} fora do padrão` : "Todos já seguem o TMDb"}
                  action={plan.renames.length > 1 ? (
                    <Button size="small" onClick={() => setSelectedRenames(selectedRenames.size ? new Set() : new Set(plan.renames.filter((rename) => !rename.conflict).map((rename) => rename.from)))}>
                      {selectedRenames.size ? "Desmarcar todos" : "Marcar todos"}
                    </Button>
                ) : null}
              >
                <Stack spacing={0.5} sx={{ maxHeight: 320, overflowY: "auto" }}>
                  {plan.renames.map((rename) => (
                    <CheckRow
                      key={rename.from}
                      checked={selectedRenames.has(rename.from)}
                      disabled={rename.conflict}
                      onToggle={() => setSelectedRenames((current) => toggle(current, rename.from))}
                    >
                      <Stack spacing={0.25} sx={{ minWidth: 0, flex: 1 }}>
                        <Typography sx={{ fontSize: "0.8rem", color: "rgba(255,255,255,0.38)", wordBreak: "break-all" }}>{fileName(rename.from)}</Typography>
                        <Stack direction="row" spacing={0.75} alignItems="center">
                          <Typography sx={{ fontSize: "0.9rem", color: rename.conflict ? "warning.main" : "common.white", fontWeight: 600, wordBreak: "break-all" }}>
                            {fileName(rename.to)}
                          </Typography>
                          {rename.matchedBy === "claude" ? (
                            <AutoAwesomeRoundedIcon titleAccess="Identificado pelo Claude" sx={{ fontSize: 15, color: "secondary.main" }} />
                          ) : null}
                        </Stack>
                        {rename.companions.length ? (
                          <Typography sx={{ fontSize: "0.75rem", color: "text.secondary" }}>
                            + {rename.companions.length} legenda{rename.companions.length === 1 ? "" : "s"}
                          </Typography>
                        ) : null}
                        {rename.conflict ? <Typography sx={{ fontSize: "0.75rem", color: "warning.main" }}>O destino já existe</Typography> : null}
                      </Stack>
                    </CheckRow>
                  ))}
                </Stack>
              </Section>
              ) : null}

              {plan.notes.length ? (
                <Stack spacing={0.5}>
                  {plan.notes.map((note) => (
                    <Typography key={note} sx={{ fontSize: "0.82rem", color: "text.secondary" }}>
                      · {note}
                    </Typography>
                  ))}
                </Stack>
              ) : null}
            </>
          ) : null}

          {!isBusy && tab === "subtitles" ? (
            <>
              <Stack direction="row" spacing={1.5} alignItems="center">
                {isShow ? (
                  <TextField
                    select
                    size="small"
                    label="Temporada"
                    value={subtitleSeason}
                    onChange={(event) => {
                      setSubtitleSeason(event.target.value === "all" ? "all" : Number(event.target.value));
                      setSubtitles(null);
                    }}
                    sx={{ minWidth: 200 }}
                    SelectProps={{ MenuProps: { PaperProps: { sx: { maxHeight: 320 } } } }}
                  >
                    <MenuItem value="all">Todos os episódios</MenuItem>
                    {subtitleSeasons.map((season) => (
                      <MenuItem key={season} value={season}>
                        {season === 0 ? "Especiais" : `Temporada ${season}`}
                      </MenuItem>
                    ))}
                  </TextField>
                ) : null}
                <Button variant="outlined" onClick={() => void handleSearchSubtitles()}>
                  Buscar legendas que faltam
                </Button>
                <Button color="inherit" onClick={() => void handleSyncExisting()} title="Alinha as legendas já baixadas com a legenda embutida no vídeo; o original vai para _quarantine">
                  Sincronizar as que já baixei
                </Button>
              </Stack>

              {subtitles ? (
                <Section
                  title={`${subtitles.offers.length} encontrada${subtitles.offers.length === 1 ? "" : "s"}`}
                  hint={`${subtitles.languages.join(", ")} · ${subtitles.covered} episódio${subtitles.covered === 1 ? "" : "s"} já completo${subtitles.covered === 1 ? "" : "s"}`}
                >
                  <Stack spacing={0.5} sx={{ maxHeight: 360, overflowY: "auto" }}>
                    {subtitles.offers.map((offer) => (
                      <CheckRow key={offerKey(offer)} checked={selectedOffers.has(offerKey(offer))} onToggle={() => setSelectedOffers((current) => toggle(current, offerKey(offer)))}>
                        <Box
                          sx={{
                            flexShrink: 0,
                            px: 0.75,
                            borderRadius: 1,
                            fontSize: "0.7rem",
                            fontWeight: 800,
                            lineHeight: "20px",
                            color: "#16120a",
                            backgroundColor: "secondary.main"
                          }}
                        >
                          {offer.language}
                        </Box>
                        <Stack spacing={0.25} sx={{ minWidth: 0 }}>
                          <Typography sx={{ fontSize: "0.9rem", color: "common.white", fontWeight: 600 }}>
                            {offer.seasonNumber === null ? "" : offer.seasonNumber === 0 ? "Especial " : `T${offer.seasonNumber} `}
                            {offer.seasonNumber === null ? "" : offer.episodeNumber !== null ? `E${String(offer.episodeNumber).padStart(2, "0")} · ` : "· "}
                            {offer.episodeTitle}
                          </Typography>
                          <Typography sx={{ fontSize: "0.78rem", color: "text.secondary", wordBreak: "break-all" }}>
                            {offer.source === "embedded"
                              ? `do próprio vídeo (${offer.release}) · sem download`
                              : `${offer.hashMatch ? "✓ sincronizada com o arquivo · " : ""}${offer.release || "sem nome"} · ${offer.downloadCount} downloads`}
                          </Typography>
                        </Stack>
                      </CheckRow>
                    ))}
                  </Stack>
                  {subtitles.notes.map((note) => (
                    <Typography key={note} sx={{ fontSize: "0.82rem", color: "text.secondary" }}>
                      · {note}
                    </Typography>
                  ))}
                </Section>
              ) : (
                <Typography color="text.secondary" sx={{ fontSize: "0.9rem" }}>
                  Procura os idiomas configurados em Configurações nos episódios que ainda não têm. Se o vídeo já traz a legenda, ela é extraída dele (sem download); o resto vem do OpenSubtitles. A conta gratuita baixa poucas por dia: o que faltar, rode de novo amanhã e ele continua de onde parou.
                </Typography>
              )}
            </>
          ) : null}
        </Stack>
      </DialogContent>

      <DialogActions sx={{ px: 3, pb: 2.5 }}>
        <Button color="inherit" disabled={isBusy} onClick={onClose}>
          {doneMessage ? "Fechar" : "Cancelar"}
        </Button>
        {tab === "metadata" && plan ? (
          <Button
            variant="contained"
            color="secondary"
            disabled={isBusy || changeCount === 0}
            onClick={() => void handleApply()}
            sx={{ color: "#16120a", fontWeight: 800, borderRadius: 999, px: 2.5 }}
          >
            Aplicar {changeCount} {changeCount === 1 ? "mudança" : "mudanças"}
          </Button>
        ) : null}
        {tab === "subtitles" && subtitles?.offers.length ? (
          <Button
            variant="contained"
            color="secondary"
            disabled={isBusy || selectedOffers.size === 0}
            onClick={() => void handleDownloadSubtitles()}
            sx={{ color: "#16120a", fontWeight: 800, borderRadius: 999, px: 2.5 }}
          >
            {subtitles.offers.every((offer) => offer.source === "embedded") ? "Extrair" : "Baixar"} {selectedOffers.size}
          </Button>
        ) : null}
      </DialogActions>
    </Dialog>
  );
}

function ProgressLog({ steps }: { steps: string[] }) {
  return (
    <Stack spacing={1.5} alignItems="center" sx={{ py: 5 }}>
      <CircularProgress color="secondary" size={28} />
      <Typography sx={{ color: "common.white", fontWeight: 600, textAlign: "center" }}>{steps[steps.length - 1] ?? "Começando..."}</Typography>
      <Stack spacing={0.25} sx={{ maxWidth: 520, opacity: 0.5 }}>
        {steps.slice(-5, -1).map((step, index) => (
          <Typography key={`${index}-${step}`} sx={{ fontSize: "0.78rem", color: "text.secondary", textAlign: "center" }}>
            {step}
          </Typography>
        ))}
      </Stack>
    </Stack>
  );
}

function Section({ title, hint, action, children }: { title: string; hint?: string; action?: ReactNode; children: ReactNode }) {
  return (
    <Stack spacing={1}>
      <Stack direction="row" alignItems="baseline" spacing={1.25}>
        <Typography sx={{ fontWeight: 800, color: "common.white" }}>{title}</Typography>
        {hint ? <Typography sx={{ fontSize: "0.82rem", color: "text.secondary", flex: 1 }}>{hint}</Typography> : <Box sx={{ flex: 1 }} />}
        {action}
      </Stack>
      {children}
    </Stack>
  );
}

function CheckRow({ checked, disabled, onToggle, children }: { checked: boolean; disabled?: boolean; onToggle: () => void; children: ReactNode }) {
  return (
    <Stack
      direction="row"
      spacing={1.25}
      alignItems="flex-start"
      onClick={disabled ? undefined : onToggle}
      sx={{
        px: 1,
        py: 0.75,
        borderRadius: 2,
        cursor: disabled ? "default" : "pointer",
        opacity: disabled ? 0.5 : checked ? 1 : 0.55,
        transition: "background-color 120ms ease, opacity 120ms ease",
        ":hover": { backgroundColor: disabled ? undefined : "rgba(255,255,255,0.04)" }
      }}
    >
      <Checkbox size="small" color="secondary" checked={checked} disabled={disabled} sx={{ p: 0.25, mt: -0.25 }} tabIndex={-1} />
      {children}
    </Stack>
  );
}

/** " · 3 sincronizadas · 2 já estavam certas · 1 sem referência" */
function describeSync(sync: SubtitleSyncSummaryDto) {
  const parts = [
    sync.synced > 0 ? `${sync.synced} ${sync.synced === 1 ? "sincronizada" : "sincronizadas"}` : null,
    sync.alreadyInSync > 0 ? `${sync.alreadyInSync} já ${sync.alreadyInSync === 1 ? "estava certa" : "estavam certas"}` : null,
    sync.noReference > 0 ? `${sync.noReference} sem legenda embutida para comparar` : null
  ].filter(Boolean);
  return parts.length ? ` · ${parts.join(" · ")}` : "";
}

function toggle(current: Set<string>, key: string) {
  const next = new Set(current);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  return next;
}

function offerKey(offer: { episodeId: string; language: string }) {
  return `${offer.episodeId}|${offer.language}`;
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

function clampLines(lines: number) {
  return { display: "-webkit-box", WebkitBoxOrient: "vertical" as const, WebkitLineClamp: lines, overflow: "hidden" };
}

function asMessage(error: unknown) {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  return typeof error === "string" && error ? error : "Algo deu errado.";
}
