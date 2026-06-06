# Design: Centralize Translation and Projection Owners

## Ownership model

Each DTO family has one constructor/projection owner:

- core inputs/effects: controller conversion/effect modules;
- daemon/attach wire frames: transport conversion module;
- TUI display blocks/events: display projection adapters;
- provider/router requests: provider compatibility conversion module;
- session/transcript records: session or transcript compatibility adapter.

Reusable modules should emit neutral engine/core/runtime/message DTOs and let the owner construct edge-specific shapes.

## Rails

Use `syn` AST inventories for struct/enum constructor ownership where possible. Macro or generated-code exceptions must be documented by exact owner path and reason.
