---
id: yak-4b3b
title: 'Theme surfaces: selection bands from the terminal''s own colours'
status: shaving
type: task
priority: 2
created: '2026-11-21T04:22:15Z'
updated: '2026-11-21T05:11:22Z'
parent: yak-2107
---

Extract yaks' `src/tui/surface.rs` (OSC 10/11 query, `COLORFGBG`, dark fallback; bands blended toward the foreground) and the general rule of `palette.rs` (16 ANSI foregrounds, named by role; no fixed backgrounds). The markdown roles stay in yaks.

Done when yaks uses it with unchanged snapshots and canon replaces its `Indexed(238)` band with it. Smallest extraction: also the test of how a toque change reaches two consumers.

#### 2026-11-21T04:54:59Z

*tui-design · kind: move · from: hairy · to: shaving · thread: ksQQ8ThW0PX0SAyxtEBqIr2N1ZLOAHSsupIBxBCA0aLJyrhJPreUvVwLftHG*

moved: hairy -> shaving

#### 2026-11-21T05:02:27Z

*tui-design*

Committed as ed595ee (and a separate cargo fmt of src/raster.rs, b84b9af). Both consumers are switched over in their working trees and pass against it; they can land once toque main is pushed.

#### 2026-11-21T05:11:22Z

*tui-design*

Landed: toque ed595ee (pushed). Consumers switched: yaks 3a7789c3 (yaks-67ef, snapshots and the surfaces docshot unchanged), canon aec29b2 (canon-5781, Joel checked it live).
