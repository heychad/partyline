# PartyLine asset handoff

Use the finished files at the root of `brand/`. The selected direction is **Plum · Script with angled receiver**.

## App and frontend

- Generate the macOS icon bundle and other platform sizes from `app-icon-1024.png`. Preserve its transparent margin. Use `app-icon-16.png` and `app-icon-32.png` for those exact native icon sizes after generation; these have been tuned separately.
- Replace `frontend/public/logo-collapsed.png` with `logo-collapsed.png`. Its canvas is 500 × 500; display it at 40 × 40 on the white sidebar. Use `logo-collapsed-dark.png` for a dark sidebar.
- Replace `frontend/public/logo.png` with `logo.png`. Use `logo-dark.png` on dark backgrounds. Both wordmarks have the same 845 × 295 canvas and placement.
- Apply the values in `palette.css` to the existing light and dark theme blocks. It includes `--speaker-me` and `--speaker-them`; keep the transcript's speaker labels. Existing destructive-action and radius tokens are outside this palette.

The current native bundle references `frontend/src-tauri/icons/icon.png`, `app_icon.icns`, and `app_icon.ico`. Regenerate the existing frontend favicon and public icon sizes in the same pass.

## Menu bar

`frontend/src-tauri/src/tray.rs` uses the template glyph on macOS and the app icon on other platforms.

The idle glyph appears while stopped or paused; active capture selects the recording glyph. The recording version adds a solid dot. The app embeds the 44 px PNG and marks it as a template image; the current Tauri tray backend displays it at 18 points. The PNGs contain pure black RGB with alpha antialiasing, so macOS can tint them for either menu bar appearance.

## Installer

The background is 660 × 400. The Finder window is 660 × 460, leaving room for its title and bottom bars without clipping the instruction. The background leaves clear rectangles around these Finder icon positions:

| Item | Center | Clear rectangle |
|---|---|---|
| PartyLine app | 180, 215 | x 105–255, y 125–300 |
| Applications folder | 480, 215 | x 405–555, y 125–300 |

The background contains the brand, an arrow, and the drag instruction. Finder supplies the app and folder icons. The 2× image has the same layout at twice the resolution.

## Verification

- Master dimensions, alpha bounds, and margins match the brief.
- All PNGs have RGBA data and an embedded sRGB profile.
- Every PNG has an editable SVG counterpart. All type is outlined.
- The sidebar was reviewed at 40 px, icons at 128, 32, and 16 px, and menu bar glyphs at 22 px on light and dark backgrounds.
- Light speaker text contrast is 9.89:1 for ME and 5.34:1 for THEM. Dark contrast is 6.24:1 and 12.50:1.
- Grayscale and simulated red-green color-vision checks are in `color-checks.png` and `color-checks.json`.

`tools/install.py` regenerates the platform icons from the SVG masters and copies the frontend, tray, and installer PNGs. The app and DMG build command is `pnpm tauri build --bundles app,dmg` from `frontend/`.
