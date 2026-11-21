---
id: yak-9d07
title: 'keys: the popup keeps its columns when one help line is long'
status: shorn
type: task
priority: 3
created: '2026-11-21T16:38:51Z'
updated: '2026-11-21T19:59:06Z'
parent: yak-2107
---

popup_lines picks the fewest rows whose columns fit the width, so one help line longer than half the room collapses the popup to a single column and hides most entries behind '… N more'. Seen twice in canon's ? popup (canon-5cb7, canon-e536), worked around by shortening the lines. Better: cap each column's name width at an even share and cut (or wrap) longer names, so the column count is decided by the room, not by the longest line.

#### 2026-11-21T19:59:05Z

*delta:Canon UI v2 · kind: move · from: hairy · to: shaving · thread: ksQQ8ThW0PX0SAyxtEBqIr2N1ZLOAHSsupIBxBCA0aLJyrhJPreUvVwLftHG*

moved: hairy -> shaving

#### 2026-11-21T19:59:05Z

*delta:Canon UI v2*

Landed in toque 3575652: when no row count fits at full width, columns are held to half the width and long help lines are cut with an ellipsis. Test one_long_help_line_cuts_itself_rather_than_collapsing_the_popup fails without the fix, passes with it. yaks' popup snapshots unchanged.

#### 2026-11-21T19:59:06Z

*delta:Canon UI v2 · kind: move · from: shaving · to: shorn · thread: ksQQ8ThW0PX0SAyxtEBqIr2N1ZLOAHSsupIBxBCA0aLJyrhJPreUvVwLftHG*

moved: shaving -> shorn
