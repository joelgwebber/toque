---
id: yak-9d07
title: 'keys: the popup keeps its columns when one help line is long'
status: hairy
type: task
priority: 3
created: '2026-11-21T16:38:51Z'
updated: '2026-11-21T16:38:51Z'
parent: yak-2107
---

popup_lines picks the fewest rows whose columns fit the width, so one help line longer than half the room collapses the popup to a single column and hides most entries behind '… N more'. Seen twice in canon's ? popup (canon-5cb7, canon-e536), worked around by shortening the lines. Better: cap each column's name width at an even share and cut (or wrap) longer names, so the column count is decided by the room, not by the longest line.
