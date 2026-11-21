---
id: yak-f352
title: Mouse hit map recorded during render
status: hairy
type: task
priority: 4
created: '2026-11-21T04:22:16Z'
updated: '2026-11-21T04:22:16Z'
parent: yak-2107
---

yaks' `src/tui/mouse.rs` pattern: render records the rects it laid out, handlers read them back, reset each frame. Maybe just a small generic `HitMap<K>`; maybe only a documented pattern. Decide when canon's mouse phase starts.
