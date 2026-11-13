# PartyLine · Plum

The finished identity uses the angled receiver and dialogue blocks selected by Chad. The receiver spans the full height of the blocks, with a 64 px gap on the master canvas.

The final pass centered the artwork, made the icon rim concentric, and aligned the 16 px and 32 px dialogue bars to their pixel grids. The selected Plum, lavender, and pale-gold icon colors are unchanged.

## Files

| Asset | Files | Size |
|---|---|---|
| App master | `app-icon.svg`, `app-icon-1024.png` | 1024 × 1024 |
| Small icons | `app-icon-16.*`, `app-icon-32.*`, `app-icon-128.*` | 16, 32, and 128 px |
| Sidebar mark | `logo-collapsed.*` | 500 × 500; light backgrounds |
| Dark sidebar mark | `logo-collapsed-dark.*` | 500 × 500; dark backgrounds |
| Wordmark | `logo.*` | 845 × 295; light backgrounds |
| Dark wordmark | `logo-dark.*` | 845 × 295; dark backgrounds |
| Menu bar | `tray-template.*`, `tray-template@2x.*` | 22 and 44 px |
| Recording menu bar | `tray-recording-template.*`, `tray-recording-template@2x.*` | 22 and 44 px |
| Installer | `dmg-background.*`, `dmg-background@2x.*` | 660 × 400 and 1320 × 800 |
| Theme | `palette.md`, `palette.css`, `palette.json` | Light and dark tokens |
| Review | `preview.*`, `color-checks.*` | Overview and simulated color checks |

`.*` means PNG and SVG. All PNGs are RGBA with embedded sRGB profiles. Every raster has an SVG source, and all SVG lettering is outlined. `asset-manifest.json` records dimensions, checksums, and validation results.

The app master has a centered 824 × 824 px rounded body, a 185 px outer corner radius, and 100 px transparent margins. Use the tuned 16 px and 32 px exports for those exact sizes.

## Typography

The wordmark uses **Source Sans 3 Semibold 3.052**, matching the app's font family. Adobe distributes it under the [SIL Open Font License 1.1](https://github.com/adobe-fonts/source-sans/blob/release/LICENSE.md), which permits this use. The original font, copyright notice, license, and source checksums are in `fonts/`. No font installation is needed to display the outlined assets.

## Color checks

Speaker text exceeds 4.5:1 contrast in both interface themes. The colors remain separated by brightness in grayscale and the supplied protanopia and deuteranopia simulations. `color-checks.json` records the values and [simulation source](https://github.com/njsmith/colorspacious/blob/master/colorspacious/cvd.py). Keep the `ME:` and `THEM:` labels in the transcript.

## Handoff and rebuilding

The kit is integrated into PartyLine's icons, sidebar, welcome screen, About dialog, theme, transcript labels, menu bar, and installer. See `HANDOFF.md` for destinations and behavior.

To rebuild the kit, install `tools/requirements.txt` in a Python environment and run `python tools/build.py` from this folder. Font files are bundled, so rebuilding requires no font download.

Run `python tools/install.py` on macOS to regenerate platform icons and copy the selected assets into the frontend. Then run `pnpm tauri build --bundles app,dmg` from `frontend/` to rebuild PartyLine and its installer.

Earlier concepts remain in their existing subfolders in the working repository. The delivery ZIP contains only the finished kit.
