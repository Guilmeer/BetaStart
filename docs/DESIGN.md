# Beta Start — interface guidelines

This document is the visual and interaction reference for the frontend. Keep it in
sync with `src/App.css` and `src/Icons.tsx`.

## Direction

Beta Start is a tool people open dozens of times a day. The interface should feel
like a quiet instrument panel: fast, legible, and calm, with one signal colour
that always means "this is where you are" or "this is the action".

- Cool graphite instead of blue-black glass.
- The signal teal comes from the cyan half of the Beta Start app icon, so the
  chrome and the tray/taskbar icon finally speak the same language.
- Native Windows typography instead of a bundled display font.
- Monochrome iconography instead of emoji chrome.
- One accent used for selection, focus, and primary actions only.
- Flat surfaces with 1px borders; avoid stacked glass, glow, and gradient cards.

## Tokens

Dark theme values live in `:root`; light theme overrides live under
`[data-theme="light"]`.

| Token | Role |
| --- | --- |
| `--bg` | Window background |
| `--surface`, `--surface-2`, `--surface-3` | Panels, inputs, nested cards |
| `--border`, `--border-strong` | Hairlines and hover borders |
| `--text-strong` | Titles and primary labels |
| `--text`, `--text-2`, `--text-3` | Body copy hierarchy |
| `--text-muted`, `--text-dim` | Hints and metadata (never the only source of meaning) |
| `--accent` | Signal accent: selection, focus, primary actions |
| `--accent-soft`, `--accent-border` | Tinted states |
| `--ok`, `--warn`, `--danger`, `--info` | Status only, never decoration |

Palette rules:

- Purple is not part of the default palette. It is available only as an opt-in
  accent.
- Do not use saturated colour as a row background. Use `--accent-soft` plus a 2px
  accent keyline for the selected row.
- Status colours must be paired with text, not used alone.
- New feature colours must be added as tokens for both themes.

## Accent themes

`src/theme.ts` is the source of truth for the accent list; CSS defines a dark and
a light token block for each accent under `[data-accent="…"]`.

| Accent | Character |
| --- | --- |
| Teal | Default, cool and precise; matches the cyan in the app icon |
| Blue | Calm and conventional, closest to a Windows system accent |
| Violet | Expressive, best in short sessions |
| Rose | Warm without the terracotta/Claude association |
| Amber | The original warm variant, kept as an option |
| Slate | Monochrome, quiet, closest to the instrument-panel idea |

When adding an accent:

1. Add the id and label to `ACCENTS` in `src/theme.ts`.
2. Add dark and light token blocks in `App.css` (accent, strong, soft, border,
   ink, ring).
3. Add a `.accent-option[data-accent="…"]` swatch color.
4. Add the id to the Rust validation in `src-tauri/src/core/config.rs`.
5. Verify primary button contrast in both themes.

### Theme packs (not implemented yet)

Extensions cannot change the interface today: the Deno host only exposes the
memory, tasks, windows, search and clipboard tools, and the manifest has no
appearance contribution. A safe extension model would be declarative, not
arbitrary CSS:

- A theme pack declares `id`, `name`, and dark/light token sets in JSON.
- The host validates colour formats and contrast, then lists the pack in
  Settings.
- The config stores only the pack id; uninstalling the extension falls back to
  an accent.
- No JS or CSS injection is needed, so a theme pack carries no runtime risk.

Arbitrary CSS injection is intentionally avoided: it can break layout and leak
network requests from the WebView.

## Typography

- UI: `Segoe UI Variable Text` → `Segoe UI` → system sans.
- Titles: `Segoe UI Variable Display`.
- Code and paths: `Cascadia Code` → `Cascadia Mono` → `Consolas`.
- Base size is 13px. Avoid text below 10px; 10.5–11.5px is reserved for metadata.
- Titles use sentence case. Avoid all-caps labels and letter-spaced eyebrows.
- A note's Markdown body uses the UI font. Its first `h1` is hidden because the
  editor already renders the note title.

## Layout

- Search is the primary surface: 50px input, 16px side margins, results below,
  keyboard hints in the footer.
- Top bar: brand on the left, usage summary and six module icons on the right.
  The active module is always visible through an amber tinted icon button.
- Settings use a fixed header, a section index, a scrolling content column, and
  a fixed save bar. Primary action sits on the right.
- The window sizes per screen are defined in `SCREEN_SIZE` in `src/App.tsx`.
  Layout must remain readable at 760px wide (search) and comfortable at 1120px
  (memory).

## Interaction rules

- Keyboard first: `Alt+Space` opens, arrows move, `Enter` acts, `Esc` closes or
  returns to search. Do not add mouse-only primary actions.
- Every icon-only button needs `aria-label`, and a visible tooltip on hover.
- Use `role="listbox"`/`role="option"` for search results and keep
  `aria-activedescendant` in sync with the highlighted row.
- Keep the highlighted result inside the scroll viewport.
- Destructive actions use `.secondary.danger`; they never look like the primary
  action.
- Disabled primary buttons are neutral, not dimmed amber.
- Empty states explain what can be typed or done and offer examples.
- Show unsaved changes in the settings save bar; theme and language save
  immediately and say so in the section.
- Window rules use a monitor canvas with presets and drag-to-draw; keep the raw
  coordinates behind an "Advanced" disclosure.
- Long audit/history panels use native `<details>`, collapsed by default.
- Never communicate state with colour alone.
- Respect `prefers-reduced-motion`; animation is limited to panel entry and
  toast feedback.

## Icons

`src/Icons.tsx` is the single source of icon shapes. Icons are 24×24 stroke
paths rendered at 13–18px, `stroke-width: 1.7`, `currentColor`. Do not bring
emoji into the chrome. Add a new name to `IconName` and the `PATHS` map when a
new concept needs an icon.

## Accessibility floor

- Visible focus ring on every interactive control.
- Minimum 30px pointer target for toolbar buttons; 34px for form buttons.
- Text contrast: body and hints are tuned to meet AA at their sizes; verify new
  colours in both themes.
- `aria-live` for result counts and toasts.
- Inputs always have a visible label or an `aria-label` when space is tight.
- Scrollbars use the themed overlay tokens.
