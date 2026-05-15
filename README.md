# codex-canmore

`codex-canmore` gives Codex a medium layer.

Not a second canvas. Not a file-preview clone. The plugin lets Codex turn a thought into the right temporary surface: a decision board, a system diagram, a small control panel, an image direction board, a comparison grid, or any other visual layer that makes the next move clearer.

The surface can stay disposable. If it becomes useful project material, promote it.

## Shape

```text
user intent
  -> Codex chooses a medium
  -> Rust MCP stores a compact surface
  -> Codex or plugin code records structured feedback events
  -> Codex reads the events, not a pile of prose
  -> useful surfaces can be promoted
```

## What V1 Does

| Part | Behavior |
| --- | --- |
| Rust core | Stdio MCP server with local JSON storage under `.canmore-medium/`. |
| Medium surfaces | Stores title, purpose, medium type, cards, promotion state, and events. |
| Feedback events | Stores clicks, selections, slider changes, notes, and plugin-defined signals when Codex or plugin code reports them as structured data. |
| Image lane | Registers local files produced by the host built-in `image_gen`; no image API keys or direct generation calls. |
| HTML view | Writes a local static preview for each surface so the medium can be inspected without starting a web stack. |
| Context budget | Tool responses are compact unless the caller asks for full spec or events. |

## Boundary

Generated images come from Codex's built-in `image_gen` capability. This plugin does not call image-generation APIs, ask for API keys, or hide network generation behind the surface layer.

## Install Surface

The plugin ships one MCP server:

```text
bin/canmored.exe mcp
```

The binary is built from `server/`. The MCP config launches the native runtime directly.

## Commands

```powershell
npm run build
npm test
```

## Repository Map

```text
.codex-plugin/        plugin manifest
.mcp.json             MCP wiring
skills/canmore/       compact Codex skill
server/               Rust core
scripts/              build
tests/                repo-surface tests
bin/                  packaged Windows runtime
```
