# Project Operating Notes

This repo is the public build surface for `codex-canmore`.

## Product

- Build a medium layer for Codex, not a duplicate artifact viewer.
- Surfaces are temporary by default.
- Promote only when the surface becomes project material.
- Feedback from the surface should be structured enough for an agent to act on without reading a giant transcript.
- Keep the Rust core boring, local, and small.

## Image Generation

- Do not use the OpenAI API, image endpoints, API keys, or direct network calls to generate images or assets.
- Image work is routed through the host-provided built-in `image_gen` capability.
- After built-in `image_gen` creates a local file, register it through `canmore_image_asset_register`.
- Store managed copies, hashes, sizes, notes, and surface links. Do not store original source paths.

## Verification

- `npm run build` builds the packaged Windows MCP runtime.
- `npm test` runs Rust tests plus repo-surface checks.
- Keep public docs and plugin metadata aligned with the medium-layer product.
