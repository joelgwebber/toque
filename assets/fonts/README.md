# Embedded fonts

Subsets of two font families, used by `src/raster.rs` (the `png` feature). Do not edit the `.ttf`
files by hand: `python3 scripts/subset_fonts.py` regenerates all of them byte for byte (the range
lists, the sources and the choices are in that script).

| File | From | Used for |
| --- | --- | --- |
| `DejaVuSansMono-subset.ttf` | DejaVu Sans Mono 2.34 | regular text |
| `DejaVuSansMono-Bold-subset.ttf` | DejaVu Sans Mono Bold 2.34 | `Modifier::BOLD` |
| `DejaVuSansMono-Oblique-subset.ttf` | DejaVu Sans Mono Oblique 2.34 | `Modifier::ITALIC` |
| `DejaVuSansMono-BoldOblique-subset.ttf` | DejaVu Sans Mono Bold Oblique 2.34 | both |
| `NotoColorEmoji-COLRv1-subset.ttf` | Noto Color Emoji COLRv1, noto-emoji commit `99701e0` | color emoji |

- `DejaVu-LICENSE.txt` covers all four DejaVu faces (Bitstream Vera license, public-domain
  changes). The Braille Patterns, which DejaVu Sans Mono lacks, are outlines of DejaVu Sans (same
  license), and the glyphs missing from the bold and oblique faces are copied from the regular
  face. The Vera license lets modified fonts be distributed under names that contain neither
  *Bitstream* nor *Vera*; these keep the family name *DejaVu Sans Mono*.
- `NotoColorEmoji-OFL.txt` is the SIL Open Font License 1.1 for the emoji face.
