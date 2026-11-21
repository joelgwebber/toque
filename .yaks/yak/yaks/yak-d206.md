---
id: yak-d206
title: 'Image pane: graphics protocol or half-blocks, zoom and pan'
status: shorn
type: task
priority: 3
created: '2026-11-21T04:22:16Z'
updated: '2026-11-21T18:27:53Z'
parent: yak-2107
---

Extract yaks' `src/tui/imageview.rs` and `imagesize.rs` (ratatui-image; kitty/sixel/iTerm2 when the terminal speaks one, half-blocks headless so frames and snapshots show it; zoom and pan by cropping). Untangle it from yaks' `App`, `Cmd` and `detail` first.

canon wants it for album art (phase 5 of its player design).

#### 2026-11-21T18:20:38Z

*delta:Canon UI v2 · kind: move · from: hairy · to: shaving · thread: ksQQ8ThW0PX0SAyxtEBqIr2N1ZLOAHSsupIBxBCA0aLJyrhJPreUvVwLftHG*

moved: hairy -> shaving

#### 2026-11-21T18:27:53Z

*delta:Canon UI v2*

Landed: toque 640993f (pushed): image feature (ImageView::new/render/zoom/pan, Zoom, Picker re-export; 4 unit tests incl. half-block colours and pan clamping) plus headless svg <path>/png <path> actions (tests: writes SVG, png says what it needs, unknown action lists actions). yaks switched in yaks-20a5 (b16f161f): all image-viewer tests pass, docshots byte-identical to main.

#### 2026-11-21T18:27:53Z

*delta:Canon UI v2 · kind: move · from: shaving · to: shorn · thread: ksQQ8ThW0PX0SAyxtEBqIr2N1ZLOAHSsupIBxBCA0aLJyrhJPreUvVwLftHG*

moved: shaving -> shorn
