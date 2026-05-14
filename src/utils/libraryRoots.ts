export function normalizeLibraryRootPaths(paths: string[]) {
  const normalizedPaths: string[] = [];
  const seenPaths = new Set<string>();

  for (const path of paths) {
    const trimmedPath = path.trim();
    if (!trimmedPath) {
      continue;
    }

    const normalizedKey = trimmedPath.toLocaleLowerCase();
    if (seenPaths.has(normalizedKey)) {
      continue;
    }

    seenPaths.add(normalizedKey);
    normalizedPaths.push(trimmedPath);
  }

  return normalizedPaths;
}

export function appendLibraryRootPath(paths: string[], nextPath: string) {
  return normalizeLibraryRootPaths([...paths, nextPath]);
}