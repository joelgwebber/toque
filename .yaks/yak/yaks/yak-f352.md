---
id: yak-f352
title: Mouse hit map recorded during render
status: shaving
type: task
priority: 4
created: '2026-11-21T04:22:16Z'
updated: '2026-11-21T19:59:04Z'
parent: yak-2107
---

yaks' `src/tui/mouse.rs` pattern: render records the rects it laid out, handlers read them back, reset each frame. Maybe just a small generic `HitMap<K>`; maybe only a documented pattern. Decide when canon's mouse phase starts.

#### 2026-11-21T19:55:20Z

*delta:Canon UI v2 · kind: move · from: hairy · to: shaving · thread: ksQQ8ThW0PX0SAyxtEBqIr2N1ZLOAHSsupIBxBCA0aLJyrhJPreUvVwLftHG*

moved: hairy -> shaving

#### 2026-11-21T19:59:04Z

*delta:Canon UI v2*

Landed in toque 3575652: HeadlessApp::handle_mouse plus click/dclick/scroll protocol actions (test: click_and_dclick_reach_the_app_at_their_cell), and toque::hits::HitMap<K> (clear/record/record_rows/at; 2 tests), the yaks mouse.rs pattern made generic. yaks keeps its own HitMap for now (its fields are app-specific); canon-2feb is the first user.
