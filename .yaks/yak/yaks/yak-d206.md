---
id: yak-d206
title: 'Image pane: graphics protocol or half-blocks, zoom and pan'
status: hairy
type: task
priority: 3
created: '2026-11-21T04:22:16Z'
updated: '2026-11-21T04:22:16Z'
parent: yak-2107
---

Extract yaks' `src/tui/imageview.rs` and `imagesize.rs` (ratatui-image; kitty/sixel/iTerm2 when the terminal speaks one, half-blocks headless so frames and snapshots show it; zoom and pan by cropping). Untangle it from yaks' `App`, `Cmd` and `detail` first.

canon wants it for album art (phase 5 of its player design).
