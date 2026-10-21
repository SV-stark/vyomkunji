# Theming (VyomKunji palette file)

The VyomKunji app paints itself with a **palette** — a set of named colours. The palette
implements `fastframe_theme::Palette`, so it can be replaced by a JSON file without
rebuilding the app.

## Where the file goes

```
%APPDATA%\VyomKunji\theme.json
```

(`C:\Users\<you>\AppData\Roaming\VyomKunji\theme.json`, with fallback to `Signer\theme.json`)

The file is read at start-up. If it is missing, the built-in dark and light palettes are
used. A file that cannot be parsed is ignored, and the built-in palette is kept.

## Format

```json
{
  "base": "dark",
  "colors": {
    "accent": "#4C8DFF",
    "surface": "#161C28",
    "text": "#E9EDF5"
  }
}
```

- `base` is `"dark"` or `"light"` (default `"dark"`).
- `colors` holds any of the names below, as `#RRGGBB` or `#RRGGBBAA`.
- Unknown names make the file invalid, so a typo is reported rather than ignored.
- Colours you leave out keep their built-in value; `accent_hover` and `surface_hover`
  are derived from `accent` and `surface` when those are set.

## The colour names

| Name | Used for |
|---|---|
| `window` | App background |
| `panel` | Top bar, sidebar, status bar |
| `surface` | Cards |
| `surface_hover` | Cards and buttons on hover |
| `surface_active` | Selected and pressed widgets |
| `outline` | Hairline borders |
| `text` | Primary text |
| `secondary` | Secondary text |
| `dim` | Faint text, hints |
| `accent` | Buttons, selection, links |
| `accent_hover` | Accent on hover (derived) |
| `on_accent` | Text on the accent colour |
| `danger` | Errors, expired certificates |
| `warning` | Warnings, expiring certificates |
| `overlay` | Popups and code blocks |
| `shadow` | Shadows (alpha only) |

## Light and dark

The top bar has a **Light mode / Dark mode** button. A palette file sets one of the two
bases; the other keeps its built-in colours. To theme both, run the app once in each
mode and save a file per base, or keep one file and switch bases in the file.

## Example: a light palette

```json
{
  "base": "light",
  "colors": {
    "window": "#F7F8FA",
    "panel": "#FFFFFF",
    "surface": "#FFFFFF",
    "outline": "#DFE4EC",
    "text": "#121822",
    "secondary": "#5B6678",
    "accent": "#2563EB",
    "accent_hover": "#1D54C7"
  }
}
```
