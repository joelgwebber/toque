---
id: yak-4b3b
title: 'Theme surfaces: selection bands from the terminal''s own colours'
status: hairy
type: task
priority: 2
created: '2026-11-21T04:22:15Z'
updated: '2026-11-21T04:22:15Z'
parent: yak-2107
---

Extract yaks' `src/tui/surface.rs` (OSC 10/11 query, `COLORFGBG`, dark fallback; bands blended toward the foreground) and the general rule of `palette.rs` (16 ANSI foregrounds, named by role; no fixed backgrounds). The markdown roles stay in yaks.

Done when yaks uses it with unchanged snapshots and canon replaces its `Indexed(238)` band with it. Smallest extraction: also the test of how a toque change reaches two consumers.
