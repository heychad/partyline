# PartyLine palette

The selected icon uses Plum, lavender for ME, and pale gold for THEM. The light interface uses darker speaker colors for readable text.

| Token | Light hex | Light HSL | Dark hex | Dark HSL |
|---|---|---|---|---|
| background | `#FFFFFF` | `0 0% 100%` | `#1B1721` | `264 17.86% 10.98%` |
| foreground | `#271E32` | `267 25% 15.69%` | `#F4EDF7` | `282 38.46% 94.9%` |
| Brand / primary | `#684889` | `269.54 31.1% 40.98%` | `#BEA6D9` | `268.24 40.16% 75.1%` |
| primary-foreground | `#FFFFFF` | `0 0% 100%` | `#271E32` | `267 25% 15.69%` |
| ME | `#4F386E` | `265.56 32.53% 32.55%` | `#A393B9` | `265.26 21.35% 65.1%` |
| THEM | `#8B6418` | `39.65 70.55% 31.96%` | `#F3D789` | `44.15 81.54% 74.51%` |
| muted | `#F2EEF6` | `270 30.77% 94.9%` | `#30273B` | `267 20.41% 19.22%` |
| muted-foreground | `#62556F` | `270 13.27% 38.43%` | `#B8AABD` | `284.21 12.58% 70.39%` |
| border | `#D8D0E1` | `268.24 22.08% 84.9%` | `#51445E` | `270 16.05% 31.76%` |

## Text contrast

Ratios below use the exact hex values above. Light text backgrounds are white; dark text backgrounds are `#1B1721`.

| Pair | Light | Dark |
|---|---:|---:|
| speaker-me | 9.89:1 | 6.24:1 |
| speaker-them | 5.34:1 | 12.50:1 |
| foreground | 15.93:1 | 15.37:1 |
| muted-text | 6.01:1 | 6.44:1 |
| primary-button | 7.30:1 | 7.32:1 |

ME and THEM also differ in brightness. Keep the `ME:` and `THEM:` labels so speaker identity does not rely on color alone.

## Icon colors

| Role | Hex | HSL |
|---|---|---|
| edge | `#2B203A` | `265.38 28.89% 17.65%` |
| body | `#392B49` | `268 25.86% 22.75%` |
| receiver | `#D7C7C0` | `18.26 22.33% 79.8%` |
| me | `#A393B9` | `265.26 21.35% 65.1%` |
| them | `#F3D789` | `44.15 81.54% 74.51%` |

`palette.css` contains matching shadcn-style HSL variables. `logo.*` and `logo-collapsed.*` use the light-background colors; their `-dark` versions use the selected icon colors.
