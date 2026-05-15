---
name: canmore
description: Use when the answer should become a temporary visual medium layer with structured feedback, not just prose or a permanent artifact.
---

# Canmore

Use Canmore when the right answer is a generated surface: diagram, decision board, comparison grid, slider panel, image direction board, or plugin-specific control layer.

Default to ephemeral. Promote only if the surface becomes project material.

Core flow:

1. `canmore_medium_recipe` to choose a surface shape.
2. `canmore_medium_create` to create the surface.
3. `canmore_medium_serve` to open it as a live browser surface when the user needs to see or touch it.
4. `canmore_medium_event` to record user interactions or plugin signals.
5. `canmore_medium_read` to read compact state and events.
6. `canmore_medium_promote` only when the surface is worth keeping.

For visual asset work, call built-in `image_gen` outside the plugin, then register the local output with `canmore_image_asset_register`. Do not use image APIs, API keys, SDK image paths, or hidden network generation.
