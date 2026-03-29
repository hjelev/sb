# Repository Agent Instructions

These instructions are always in effect for this repository.

## Mandatory Design Policy

Before making any code change, read and follow [sb-design-principles.md](sb-design-principles.md).

If there is any conflict between speed and visual behavior, preserve both by using constant-time updates in hot paths.

Do not introduce per-item runtime manipulations in scrolling, rendering, or selection paths.

## Change Guardrails

- Keep keyboard-first interaction behavior intact.
- Keep footer/status rendering stable during incremental updates.
- Prefer simple, maintainable implementations.
- Avoid regressions in responsiveness and readability.
