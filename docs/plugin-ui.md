# Plugin UI

Plugin views are built from [Archipel Kit](https://github.com/archipel-dev/kit), the UI
library of all Archipel tools, so every plugin looks and behaves like the host. There is no
ad-hoc styling: colors come from the theme and sizes from the kit's tokens.

## How it works

- The host calls `archipel_kit::install(ctx, Theme::dark())` once. Fonts, spacing and the
  theme live in the `egui::Context`; switching the theme restyles every plugin view. A plugin
  never installs or changes the theme.
- `archipel_sdk::ui::ui(frame)` gives the plugin the `&mut egui::Ui` of its view (`None` when
  the host is not egui).
- `archipel_sdk::kit` is `archipel_kit`: `kit::components` (buttons, inputs, lists, trees,
  tables, panels, banners, dialogs, ...), `kit::Theme`, `kit::icons`, `kit::tokens` and
  `kit::egui`.
- `archipel_sdk::ui` adds small helpers on top of the kit: `header`, `section`, `property`,
  `empty_state`, `secondary_text`, and `render_node` for declarative `ViewNode` trees.

```rust,ignore
use archipel_sdk::kit::{Theme, components::{Button, Checkbox, TextField}};

ui.add(TextField::new(&mut self.name).label("Name"));
ui.add(Checkbox::new(&mut self.loud, "Loud"));
if ui.add(Button::primary("Say Hello")).clicked() { /* ... */ }
let accent = Theme::colors(ui.ctx()).text_secondary; // never a hard-coded Color32
```

## Kit documentation

The kit documents each component with usage, API, behavior and accessibility:

- [Kit docs index](https://github.com/archipel-dev/kit/blob/main/docs/README.md) and the
  [getting-started guide](https://github.com/archipel-dev/kit/blob/main/docs/getting-started.md)
- [Building components](https://github.com/archipel-dev/kit/blob/main/docs/building-components.md):
  when the kit lacks a widget, compose one from its parts so it still follows the theme.
- [Behaviours](https://github.com/archipel-dev/kit/blob/main/docs/behaviours.md): tooltips,
  context menus, cursors and drag and drop on any widget with `.with(...)`.
- [Theming](https://github.com/archipel-dev/kit/blob/main/docs/theming.md) and
  [Testing](https://github.com/archipel-dev/kit/blob/main/docs/testing.md)

## Rules of thumb

1. Use kit components for everything interactive. If you reach for `ui.button` or
   `egui::TextEdit`, use `Button` or `TextField` instead.
2. Colors from `Theme::colors(ctx)`, sizes from `kit::tokens`; no literal colors or fonts.
3. Every control needs an accessible name (its label). The `audit` test fails on unnamed
   controls, tiny targets and overlaps.
4. Test views in both themes with `archipel_sdk::testing` ([api.md](api.md#testing)).

Declarative views (`ViewNode`) are rendered by the host with the same kit components, so
Lua and WASM plugins look the same as Rust plugins. Limits of the current renderer: a
`multiline` text input renders as a single-line field, and `Code` is shown as monospace
text.
