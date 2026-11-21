---
id: yak-d206
title: 'Image pane: graphics protocol or half-blocks, zoom and pan'
status: shaving
type: task
priority: 3
created: '2026-11-21T04:22:16Z'
updated: '2026-11-21T18:20:38Z'
parent: yak-2107
---

Extract yaks' `src/tui/imageview.rs` and `imagesize.rs` (ratatui-image; kitty/sixel/iTerm2 when the terminal speaks one, half-blocks headless so frames and snapshots show it; zoom and pan by cropping). Untangle it from yaks' `App`, `Cmd` and `detail` first.

canon wants it for album art (phase 5 of its player design).

#### 2026-11-21T18:20:38Z

*delta:Canon UI v2 · kind: move · from: hairy · to: shaving · thread: ksQQ8ThW0PX0SAyxtEBqIr2N1ZLOAHSsupIBxBCA0aLJyrhJPreUvVwLftHG*

moved: hairy -> shaving
