# PartyLine design brief

You are designing the brand identity and app art for PartyLine, a personal macOS app. Read the whole brief before you draw anything. Work in two rounds: concepts first, then the full asset set from the concept the owner picks.

## What PartyLine is

PartyLine records a call on the owner's Mac and transcribes it on the device. Nothing goes to a server. Its one distinctive feature is the transcript format: every line is labeled `ME:` or `THEM:`, like a screenplay, so the owner can paste the transcript into an AI model and ask questions about the conversation.

It is a private fork of an open-source meeting transcriber. One person uses it. There is no marketing site, no store listing, and no team. The art only has to look right in the Dock, the menu bar, the app window, and the installer.

## The name

A party line was a telephone line shared by several households. Anyone on the line could pick up and hear the conversation. The name is a joke about an app that quietly listens to your calls, and it is a telephone reference, not a celebration reference.

## Who it is for

The owner is a creative strategist with a film degree who plays tabletop games and builds his own tools. He describes his taste as playful. He wants something he enjoys seeing in his Dock, with the character of a well-made hobby object. He does not want it to look like a venture-funded productivity product.

## Creative direction

This section is the brief writer's proposal. The owner may overrule any of it.

Build the identity on the telephone exchange: switchboard jacks, patch cords, handsets, braided cable, the two ends of one line. The strongest idea available is two voices on one line, because it matches the ME and THEM labels. Give each voice its own color and let the mark show the two meeting.

Three starting points for round 1:

1. **Switchboard.** One jack with two patch cords, one in each voice color.
2. **Handset.** A classic telephone handset split into two tones, earpiece and mouthpiece.
3. **Script.** Two alternating dialogue lines, set like a screenplay, joined by a single cord.

You may replace any of these with a better idea, but show three distinct concepts.

Tone: warm, a little mischievous, mid-century telephone-company graphics rather than modern app gloss. Flat shapes or simple two-step shading. It should still read as a current macOS app when it sits next to system icons.

Avoid:

- Microphones, waveforms, and speech-to-text clichés.
- Sparkles, robots, brains, or any other AI shorthand.
- Purple-to-blue gradients and glassy 3D blobs.
- Party hats, confetti, balloons, or disco balls.
- Any real telephone company's logo, bell symbol, or trade dress.
- Text inside the app icon.

## Color

The app's interface today is neutral grayscale with a light window. It uses shadcn-style CSS variables written as HSL triplets, and the typeface is Source Sans 3. Propose a palette that drops into those variables:

- One brand color for primary buttons and the icon.
- A ME color and a THEM color for the two speakers. A reader with red-green color blindness must be able to tell them apart, and both must work on white and on near-black.
- Text in either speaker color must reach a 4.5:1 contrast ratio on its background.
- Neutrals for background, foreground, muted, and border, in light and dark sets.

Deliver the palette as a table of names, hex values, and HSL triplets (for example `24 95% 53%`), with separate light and dark columns.

## Assets to deliver

Required:

| # | Asset | File name | Spec | Where it appears |
|---|-------|-----------|------|------------------|
| 1 | App icon master | `app-icon-1024.png` and `app-icon.svg` | 1024 x 1024 px canvas, transparent background. Put the icon body in a centered 824 x 824 px rounded square with about a 185 px corner radius, leaving a 100 px transparent margin. | Dock, Finder, installer. The developer generates every smaller size from this file. |
| 2 | Small-size icon check | `app-icon-32.png`, `app-icon-16.png` | Hand-tuned versions if the master turns to mud when scaled down. Otherwise say the master scales cleanly. | Finder lists, window title bar |
| 3 | Sidebar mark | `logo-collapsed.png` and `.svg` | 500 x 500 px, transparent, no rounded-square body. Displayed at 40 x 40 px on a white sidebar. | Top of the app sidebar |
| 4 | Wordmark | `logo.png` and `.svg` | 845 x 295 px, transparent. The mark plus the word "PartyLine" as one word with a capital P and a capital L. Supply a light-background and a dark-background version. | About dialog, README |
| 5 | Color palette | `palette.md` | The table described in the color section. | Interface theme |

Optional, in priority order:

| # | Asset | File name | Spec | Where it appears |
|---|-------|-----------|------|------------------|
| 6 | Menu bar glyph | `tray-template.png`, `tray-template@2x.png`, `.svg` | 22 x 22 px and 44 x 44 px. Pure black on transparent, alpha only, no color and no gray fills. macOS tints it. One or two simple shapes. | Menu bar |
| 7 | Recording-state glyph | `tray-recording-template.png` and `@2x` | Same spec as 6, visibly different at a glance. | Menu bar while recording |
| 8 | Installer background | `dmg-background.png`, `dmg-background@2x.png` | 660 x 400 px and 1320 x 800 px. Leave clear space at left-center for the app icon and at right-center for the Applications folder. | Disk image window |

## Rules for every file

- Supply an SVG source for every raster file. Outline all type in the SVGs.
- Export PNGs in sRGB with a real alpha channel. No baked-in checkerboard and no white box behind transparent art.
- Use only original artwork. Do not trace or reuse existing logos, stock icons, or photographs.
- If the wordmark uses a typeface, name it and confirm its license allows this use. Prefer an open license such as the SIL Open Font License.
- Use the file names in the tables exactly. Put everything in one folder named `brand/`.

## Round 1: concepts

Show three concepts. For each one, show the app icon at 1024 px, at 128 px, and at 32 px side by side on a light and a dark background, plus a three-swatch color idea (brand, ME, THEM). Add one sentence on the idea behind it. Do not produce the full set yet.

## Round 2: full set

After the owner picks a concept, deliver assets 1 through 5 and whichever optional assets he asks for.

## How the work will be judged

- The icon is recognizable at 16 px and does not look like any icon already in a typical Dock.
- A stranger would guess "telephone" or "conversation" before "party."
- The ME and THEM colors are distinguishable in grayscale.
- The sidebar mark is legible at 40 px on white.
- The menu bar glyph is legible at 22 px in both light and dark menu bars.
- Every file matches its name, size, and transparency spec, so the developer can drop the folder in without edits.
