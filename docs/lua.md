# Lua plugins

Lua plugins use the same contract as Rust plugins (see [api.md](api.md)) through an
embedded Lua 5.4 runtime (the IDE's `jetthing-plugin-lua`, built on [mlua](https://github.com/mlua-rs/mlua)
with a vendored Lua). The runtime stays in the host (the Archipel IDE); this page describes the contract plugins
can rely on. A complete example lives in the IDE repository (`plugins/hello-lua`).

## Layout

```
my-plugin/
  archipel-plugin.toml   # kind = "lua", entry = "main.lua"
  main.lua               # returns the plugin table
  lib/util.lua           # optional modules: require("lib.util")
```

```toml
id = "com.example.todo"
name = "Todo"
version = "0.1.0"
api_version = 1
kind = "lua"
entry = "main.lua"

[app]                    # optional: standalone app
view = "main"
```

In the IDE, pack it with `cargo run -p xtask -- pack-plugin my-plugin` and install the `.jtplugin` from
**File ▸ Plugins… ▸ ⚙ ▸ Install Plugin from Disk…**. A project's own `plugins/` directory is also
scanned, which is handy while developing. Edit, then run **Refresh Plugins** or reinstall to reload.

## The plugin table

`main.lua` returns a table. Contribution fields have the same names and shapes as the Rust
`Contributions` (and as [api.md](api.md)). Use arrays, so the order is stable:

```lua
local plugin = {
  commands = {
    { id = "todo.add", label = "Add Todo", category = "Tools", params = '{"text":"string"}',
      run = function(ctx, params) ... return { ok = true } end },
  },
  menus        = { { menu = "Tools", command = "todo.add" } },
  keybindings  = { { command = "todo.add", key = "Mod+Alt+T" } },
  tool_windows = { { id = "todo", title = "Todo", view = "main", icon_text = "T" } },
  editors      = { { view = "list", title = "Todo List" } },
  status_widgets = { { id = "count", command = "todo.add", text = function() return "3 todos" end } },
  settings_pages = { { id = "todo", title = "Todo", fields = {
    { key = "sort", label = "Sort by", kind = { type = "choice", options = { "date", "name" }, default = "date" } },
  } } },
  views = {},
}
function plugin.activate(ctx) end          -- optional
function plugin.deactivate(ctx) end        -- optional
function plugin.poll(ctx) end              -- optional, called every frame
function plugin.on_view_event(ctx, view, event) end  -- declarative view interactions
return plugin
```

Setting kinds: `{type="text", default=""}`, `{type="bool", default=true}`,
`{type="number", default=1, min=0, max=10}`, `{type="choice", options={…}, default="…"}`.

## `ctx`: host services

| Function | Notes |
|---|---|
| `ctx.plugin_id`, `ctx.project_root` | strings (`project_root` is `nil` without a project) |
| `ctx.status(msg)` | status bar message |
| `ctx.notify(title, body?, level?)` | balloon; `level` is `"info"`, `"warning"` or `"error"` |
| `ctx.dispatch(command, params?)` | runs a command after the current call |
| `ctx.open_tool_window(id)`, `ctx.open_tab(view, title, key?)` | own views |
| `ctx.setting(key)`, `ctx.set_setting(key, value)` | settings-page defaults apply |
| `ctx.has_permission(name)` | `"network"`, `"process"`, `"fs.project"`, … |
| `ctx.request_repaint()`, `ctx.refresh_contributions()` | |

`ctx` is only valid during the call that received it. Do not store it.

## Views

A view function `views.<name>(ctx, ui, placement)` runs every frame. `placement.kind` is
`"tool_window"`, `"editor_tab"`, `"app"` or `"settings_page"`.

**Immediate mode.** Draw with `ui`, keep state in Lua variables:

```lua
function plugin.views.main(ctx, ui)
  ui.heading("Todo")
  state.draft = ui.text_input("draft", state.draft, "What needs doing?")
  ui.horizontal(function(row)
    if row.button("Add") then add(state.draft) end
    if row.button("Clear") then state.items = {} end
  end)
end
```

`ui` functions:

- text: `heading`, `label`, `muted`, `section`, `property(key, value)`, `code`, `empty_state(title, hint?)`
- layout: `separator`, `space(px?)`, `width()`
- `button(label) → clicked`, `selectable(label, selected) → clicked`
- `checkbox(label, value) → value`, `text_input(id, value, hint?) → value`, `text_area(id, value) → value`,
  `choice(id, selected, options) → value`
- `render(node) → events` draws a declarative tree inline
- containers: `horizontal(fn)`, `vertical(fn)`, `group(fn)`, `scroll(fn)`. Each passes a child `ui`;
  use that one inside the callback.

**Declarative.** Return a widget tree instead of drawing. Interactions arrive in
`plugin.on_view_event(ctx, view, event)` with `event.type` set to `"clicked"`, `"changed"`
(with `value`) or `"selected"` (with `item`):

```lua
function plugin.views.list(ctx, ui)
  return { type = "column", children = {
    { type = "row", children = {
      { type = "text_input", id = "draft", value = state.draft, hint = "New todo" },
      { type = "button", id = "add", label = "Add" } } },
    { type = "list", id = "items", items = items, selected = state.selected },
  } }
end
```

Node types: `column`, `row`, `heading`, `label{muted}`, `section`, `property{key,value}`, `separator`,
`button{id,label}`, `text_input{id,value,hint,multiline}`, `checkbox{id,label,checked}`,
`choice{id,options,selected}`, `list{id,items={id,label,detail},selected}`,
`table{id,columns,rows,selected}`, `tree{id,nodes={id,label,children},selected}`, `code{text}`,
`empty{title,hint}`. WASM plugins use the same model (`archipel_plugin_api::ViewNode`).

## Sandbox

- Only the `string`, `table`, `math`, `utf8` and `coroutine` libraries are available.
  There is no `io`, `os`, `package`, `debug`, `load`, `dofile` or `loadfile`. `print` writes to
  the IDE's stderr, prefixed with the plugin id.
- `require("a.b")` loads `<plugin dir>/a/b.lua` and nothing else.
- Memory is capped at 64 MiB per plugin.
- Each call has a time budget: 2 s for `activate`, commands and `poll`, and 250 ms per view frame.
  An endless loop fails that call with "script exceeded its time budget"; the plugin keeps running.
- Errors in commands come back as command failures with file and line. Errors in views appear in
  place of the view. Errors during load or `activate` mark the plugin *Failed* on the Plugins page.

Host APIs that need permissions (HTTP, processes, files) are not exposed to Lua yet. When they
are, they will check the manifest's `permissions`.
