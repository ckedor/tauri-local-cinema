#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ps_script="$repo_root/scripts/build-windows-installer.ps1"

if command -v cygpath >/dev/null 2>&1; then
  ps_script="$(cygpath -w "$ps_script")"
fi

if command -v pwsh >/dev/null 2>&1; then
  ps_command=(pwsh -NoProfile -ExecutionPolicy Bypass -File "$ps_script")
elif command -v powershell.exe >/dev/null 2>&1; then
  ps_command=(powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$ps_script")
elif command -v powershell >/dev/null 2>&1; then
  ps_command=(powershell -NoProfile -ExecutionPolicy Bypass -File "$ps_script")
else
  echo "PowerShell was not found in PATH. Install PowerShell or run npm run build:windows:installer." >&2
  exit 1
fi

"${ps_command[@]}" "$@"