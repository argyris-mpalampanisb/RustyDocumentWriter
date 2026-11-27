#!/bin/sh
# Regenerate the PALETTES entries in src/app/theme.rs from the themes installed here.
V=${VSCODE_EXT:-/usr/share/code/resources/app/extensions}
E=${VSCODE_USER_EXT:-$HOME/.vscode/extensions}
cd "$(dirname "$0")"
python3 extract.py --source "VS Code" \
  "$V/theme-defaults/themes/dark_modern.json" "$V/theme-defaults/themes/light_modern.json" "$V/theme-defaults/themes/hc_black.json" \
  --name "Monokai" "$V/theme-monokai/themes/monokai-color-theme.json" \
  --name "Monokai Dimmed" "$V/theme-monokai-dimmed/themes/dimmed-monokai-color-theme.json" \
  --name "Solarized Dark" "$V/theme-solarized-dark/themes/solarized-dark-color-theme.json" \
  --name "Solarized Light" "$V/theme-solarized-light/themes/solarized-light-color-theme.json" \
  "$V/theme-abyss/themes/abyss-color-theme.json" "$V/theme-kimbie-dark/themes/kimbie-dark-color-theme.json" \
  "$V/theme-quietlight/themes/quietlight-color-theme.json" "$V/theme-red/themes/Red-color-theme.json" \
  --name "Tomorrow Night Blue" "$V/theme-tomorrow-night-blue/themes/tomorrow-night-blue-color-theme.json" \
  "$E"/sdras.night-owl-*/themes/"Night Owl-color-theme.json" "$E"/sdras.night-owl-*/themes/"Night Owl-Light-color-theme.json" \
  --source "Zed" zed_builtin.json
