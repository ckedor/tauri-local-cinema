import { Button, Stack, TextField } from "@mui/material";

type LibraryRootsFieldProps = {
  paths: string[];
  onAddPath: () => void;
  onRemovePath: (pathIndex: number) => void;
  disabled?: boolean;
};

export function LibraryRootsField({
  paths,
  onAddPath,
  onRemovePath,
  disabled = false
}: LibraryRootsFieldProps) {
  return (
    <Stack spacing={2}>
      {paths.length > 0 ? (
        paths.map((path, index) => (
          <Stack key={path} direction={{ xs: "column", sm: "row" }} spacing={2}>
            <TextField
              fullWidth
              label={paths.length === 1 ? "Pasta da biblioteca" : `Pasta ${index + 1}`}
              value={path}
              InputProps={{ readOnly: true }}
            />

            <Button color="error" disabled={disabled} onClick={() => onRemovePath(index)} variant="outlined">
              Remover
            </Button>
          </Stack>
        ))
      ) : (
        <TextField
          fullWidth
          label="Pastas da biblioteca"
          value=""
          InputProps={{ readOnly: true }}
          placeholder="Nenhuma pasta selecionada"
        />
      )}

      <Button color="secondary" disabled={disabled} onClick={onAddPath} variant="outlined">
        + Adicionar pasta
      </Button>
    </Stack>
  );
}