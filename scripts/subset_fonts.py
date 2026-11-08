#!/usr/bin/env python3
"""Regenerate the embedded fonts in `assets/fonts/` (see `src/raster.rs`).

    pip install fonttools          # provides `pyftsubset` (`fontTools.subset`)
    python3 scripts/subset_fonts.py [--emoji common|base] [--out assets/fonts]

The sources are downloaded once into `target/font-sources/` (gitignored), from
the pinned URLs below, so the script does not depend on any system font. To use
local copies instead, point the variables at them:

    TOQUE_DEJAVU_DIR=/dir/with/DejaVuSansMono{,-Bold,-Oblique,-BoldOblique}.ttf
    TOQUE_DEJAVU_SANS_DIR=/dir/with/DejaVuSans{,-Bold,-Oblique,-BoldOblique}.ttf
    TOQUE_NOTO_EMOJI=/path/to/Noto-COLRv1.ttf
    TOQUE_EMOJI_DATA=/path/to/emoji-data.txt

(`TOQUE_DEJAVU_SANS_DIR` defaults to `TOQUE_DEJAVU_DIR`.)

Outputs, one subset per face, each a plain TrueType file `resvg` can load:

    DejaVuSansMono-subset.ttf             regular
    DejaVuSansMono-Bold-subset.ttf        bold
    DejaVuSansMono-Oblique-subset.ttf     `font-style:italic` (toque's `Modifier::ITALIC`)
    DejaVuSansMono-BoldOblique-subset.ttf bold + italic
    NotoColorEmoji-COLRv1-subset.ttf      color emoji

What goes where:

* **Text** (`TEXT_RANGES` + `DOCSHOT_CODEPOINTS` + `EXTRA_CODEPOINTS`) comes from
  DejaVu Sans Mono, minus any codepoint the emoji face draws, so an emoji with a
  text-style glyph in DejaVu (U+26A1 and friends) is drawn as the color emoji a
  terminal would show, in its two cells.
* **Every text face covers the same codepoints.** The rasterizer falls back to
  another face for a character the run's face lacks and then stays on it, so a
  bold run with one box-drawing character would lose its bold. DejaVu's bold and
  oblique mono lack Box Drawing, Misc Symbols and more, so `borrow_missing` copies
  those outlines from the regular face (upright, light: what a terminal draws).
* **Braille Patterns** are not in any DejaVu Sans Mono face, so they are borrowed
  from DejaVu Sans (same family and license, the face's own style), shifted
  sideways into the mono advance, so all 256 patterns are monospace.
* **Powerline / Nerd Font** private-use glyphs (U+E0A0-U+E0D4 and the like) are
  in neither DejaVu face, and no other font is embedded: they draw as the
  missing-glyph box (`--notdef-outline` keeps it; see `src/raster.rs`).
* **Emoji** come from Noto Color Emoji (COLRv1). `--emoji base` keeps every
  Unicode 15 `Emoji_Presentation` base emoji (no skin-tone modifiers, ZWJ
  sequences or flags: layout tables are dropped, so a sequence draws as its
  parts); `--emoji common` (the default) keeps only `COMMON_EMOJI`. Sizes of
  both are in the README (png section).
"""

import argparse
import os
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

try:
    from fontTools import subset
    from fontTools.pens.recordingPen import DecomposingRecordingPen
    from fontTools.pens.transformPen import TransformPen
    from fontTools.pens.ttGlyphPen import TTGlyphPen
    from fontTools.ttLib import TTFont
except ImportError:
    sys.exit("needs fontTools: pip install fonttools")

ROOT = Path(__file__).resolve().parent.parent

# -- sources (pinned) ---------------------------------------------------------

# DejaVu 2.34, not the newer 2.35-2.37 on GitHub: from 2.35 the vertical box
# drawing strokes (U+2502 and friends) stop 17% short of the 18px row toque's SVG
# uses at 14px, so a frame's borders turn into dashes. 2.34 overshoots the cell
# (-618..1992 of 2048 units) and joins up. (2.34 is also what the first,
# narrow subset used.)
DEJAVU_VERSION = "2.34"
DEJAVU_URL = (
    "https://downloads.sourceforge.net/project/dejavu/dejavu/"
    f"{DEJAVU_VERSION}/dejavu-fonts-ttf-{DEJAVU_VERSION}.tar.bz2"
)
NOTO_COMMIT = "99701e07d3d1fe1bdbcdb14964547f0fd592966f"
NOTO_URL = (
    "https://github.com/googlefonts/noto-emoji/raw/"
    f"{NOTO_COMMIT}/2D/fonts/Noto-COLRv1.ttf"
)
EMOJI_DATA_URL = "https://www.unicode.org/Public/15.0.0/ucd/emoji/emoji-data.txt"

# -- what to keep -------------------------------------------------------------

# Inclusive codepoint ranges, intersected with what DejaVu Sans Mono has.
TEXT_RANGES = [
    (0x0020, 0x007E, "Basic Latin"),
    (0x00A0, 0x00FF, "Latin-1 Supplement"),
    (0x0100, 0x017F, "Latin Extended-A"),
    (0x2000, 0x206F, "General Punctuation"),
    (0x2070, 0x209F, "Superscripts and Subscripts"),
    (0x20A0, 0x20CF, "Currency Symbols"),
    (0x2100, 0x214F, "Letterlike Symbols"),
    (0x2190, 0x21FF, "Arrows"),
    (0x2200, 0x22FF, "Mathematical Operators"),
    (0x2300, 0x23FF, "Miscellaneous Technical (⌘ ⏎ ⌫ ...)"),
    (0x2500, 0x257F, "Box Drawing"),
    (0x2580, 0x259F, "Block Elements"),
    (0x25A0, 0x25FF, "Geometric Shapes"),
    (0x2600, 0x26FF, "Miscellaneous Symbols"),
    (0x2700, 0x27BF, "Dingbats"),
    (0x2800, 0x28FF, "Braille Patterns (borrowed from DejaVu Sans)"),
    (0xFFFD, 0xFFFD, "Replacement Character"),
]

# Every non-ASCII codepoint in the yaks repo's `docs/assets/*.svg`, which toque
# renders (yaks-4f4d). Regenerate the list from a yaks checkout with:
#   python3 -c "import glob,html;print(' '.join('%04X'%c for c in sorted({ord(c) for f in glob.glob('docs/assets/*.svg') for c in html.unescape(open(f,encoding='utf8').read()) if ord(c)>127})))"
# The ones in TEXT_RANGES are redundant; the emoji and the odd symbols are not.
DOCSHOT_CODEPOINTS = [
    0x00B7, 0x00D7, 0x2014, 0x2022, 0x2026, 0x2039, 0x203A, 0x2190, 0x2191,
    0x2192, 0x2193, 0x23F3, 0x2500, 0x2502, 0x2503, 0x253C, 0x2580, 0x2584,
    0x258E, 0x25AA, 0x25B2, 0x25B6, 0x25B8, 0x25C2, 0x25C6, 0x25CF, 0x25E6,
    0x2610, 0x2611, 0x2717, 0x2726, 0x2B50, 0x1F411, 0x1F4D6, 0x1F552,
    0x1F5BC, 0x1F6D6, 0x1F9AC, 0x1FA92,
]  # fmt: skip

# Zero-width joiner and variation selectors: present so emoji sequences, which
# ratatui stores as one cell's text, do not draw a missing-glyph box for them.
EXTRA_CODEPOINTS = [0x200D, 0xFE0E, 0xFE0F]

# `--emoji common`: Emoticons, every Emoji_Presentation emoji below U+2C00 (the
# status marks: ✅ ❌ ⭐ ⚡ ⏳ ...), the colored circles/squares, and these hand-picked
# pictographs a TUI or a chat is likely to print. Skin tones, flags and ZWJ
# sequences are out in both sets.
COMMON_EMOJI_RANGES = [
    (0x1F600, 0x1F64F, "Emoticons"),
    (0x1F7E0, 0x1F7EB, "colored circles and squares"),
]
COMMON_EMOJI = [
    # hands, people
    0x1F440, 0x1F447, 0x1F448, 0x1F449, 0x1F44B, 0x1F44C, 0x1F44D, 0x1F44E, 0x1F44F,
    0x1F9E0, 0x1F916, 0x1F914, 0x1F9D1,
    # animals and nature
    0x1F40D, 0x1F411, 0x1F41B, 0x1F433, 0x1F980, 0x1F98A, 0x1F331, 0x1F332, 0x1F30D,
    0x1F31F, 0x1F308, 0x1F525, 0x1F4A5, 0x1F4AB,
    # food, things
    0x1F34E, 0x2615, 0x1F355, 0x1F37A, 0x1F381, 0x1F389, 0x1F3AF, 0x1F3A8, 0x1F3C1,
    0x1F3C6, 0x1F3B5, 0x1F3AE,
    # office, files, dev
    0x1F4A1, 0x1F4AC, 0x1F4AD, 0x1F4AF, 0x1F4BB, 0x1F4BE, 0x1F4C1, 0x1F4C2, 0x1F4C4,
    0x1F4C5, 0x1F4C8, 0x1F4C9, 0x1F4CA, 0x1F4CB, 0x1F4CC, 0x1F4CD, 0x1F4CE, 0x1F4D6,
    0x1F4DA, 0x1F4DD, 0x1F4E6, 0x1F4E7, 0x1F4E2, 0x1F4F7, 0x1F50D, 0x1F50E, 0x1F510,
    0x1F511, 0x1F512, 0x1F513, 0x1F514, 0x1F517, 0x1F527, 0x1F528, 0x1F5D1, 0x1F504,
    0x1F4F1, 0x1F4BC, 0x1F9EA, 0x1F9F9, 0x1F9F0, 0x1F9EC,
    # transport, signs, time
    0x1F680, 0x1F6A7, 0x1F6A8, 0x1F6AB, 0x1F6D1, 0x1F6E0, 0x1F552, 0x1F551, 0x1F55B,
    0x1F6D6, 0x1F9AC, 0x1FA92, 0x1F5BC,
    # dots and shapes
    0x1F534, 0x1F535, 0x1F536, 0x1F537, 0x1F538, 0x1F539, 0x1F4A0, 0x1F500, 0x1F501,
]  # fmt: skip

# Skin-tone modifiers: a bare one would draw as a color swatch.
SKIN_TONES = range(0x1F3FB, 0x1F400)


# -- sources ------------------------------------------------------------------


def cache_dir() -> Path:
    d = ROOT / "target" / "font-sources"
    d.mkdir(parents=True, exist_ok=True)
    return d


def download(url: str, name: str) -> Path:
    dest = cache_dir() / name
    if not dest.exists():
        print(f"downloading {url}", file=sys.stderr)
        with urllib.request.urlopen(url) as r, open(dest, "wb") as f:
            f.write(r.read())
    return dest


def dejavu_dir() -> Path:
    if os.environ.get("TOQUE_DEJAVU_DIR"):
        return Path(os.environ["TOQUE_DEJAVU_DIR"])
    out = cache_dir() / f"dejavu-fonts-ttf-{DEJAVU_VERSION}" / "ttf"
    if not out.exists():
        with tarfile.open(download(DEJAVU_URL, f"dejavu-{DEJAVU_VERSION}.tar.bz2")) as tar:
            tar.extractall(cache_dir(), filter="data")
    return out


def noto_emoji() -> Path:
    if os.environ.get("TOQUE_NOTO_EMOJI"):
        return Path(os.environ["TOQUE_NOTO_EMOJI"])
    return download(NOTO_URL, f"Noto-COLRv1-{NOTO_COMMIT[:7]}.ttf")


def emoji_data() -> Path:
    if os.environ.get("TOQUE_EMOJI_DATA"):
        return Path(os.environ["TOQUE_EMOJI_DATA"])
    return download(EMOJI_DATA_URL, "emoji-data-15.0.0.txt")


def emoji_presentation() -> set[int]:
    """Codepoints with the Unicode `Emoji_Presentation` property."""
    cps = set()
    for line in emoji_data().read_text(encoding="utf8").splitlines():
        fields = line.split("#")[0].split(";")
        if len(fields) == 2 and fields[1].strip() == "Emoji_Presentation":
            lo, _, hi = fields[0].strip().partition("..")
            cps.update(range(int(lo, 16), int(hi or lo, 16) + 1))
    return cps


# -- building -----------------------------------------------------------------


def ranges(rs) -> set[int]:
    return {c for lo, hi, *_ in rs for c in range(lo, hi + 1)}


def run_subset(src: Path, cps: set[int], dest: Path) -> None:
    """`pyftsubset`, in-process: the glyph subset only. No hinting (resvg does
    not use it) and no layout tables (monospace text and emoji without
    sequences need none); the `.notdef` outline is kept so a missing glyph
    draws as a box."""
    unicodes = ",".join(f"{c:X}" for c in sorted(cps))
    subset.main([
        str(src),
        f"--unicodes={unicodes}",
        "--layout-features=",
        "--no-hinting",
        "--notdef-outline",
        f"--output-file={dest}",
    ])  # fmt: skip


def borrow_missing(face_path: Path, donor_specs: list, wanted: set[int], dest: Path) -> None:
    """Write a copy of the face with every `wanted` codepoint it lacks, taken
    from the first donor that has it (outlines only, centered in the face's
    advance). A donor spec is `(path, allowed codepoints)`: DejaVu Sans is
    proportional, so it may only lend the Braille patterns.

    Why: the rasterizer falls back to another face when a run has a character
    its face lacks, and it then keeps the fallback face for the rest of the run:
    a bold run with one box-drawing character would draw upright and light.
    So every text face must cover the same codepoints. DejaVu's bold and oblique
    mono lack Box Drawing, Misc Symbols and more, which are borrowed from the
    regular face (upright and light: what a terminal draws for them too), and
    Braille from DejaVu Sans (same style as the face)."""
    face = TTFont(face_path, recalcTimestamp=False)  # byte-reproducible output
    donors = [(TTFont(path), allowed) for path, allowed in donor_specs]
    cmap = face.getBestCmap()
    adv = face["hmtx"][cmap[0x41]][0]
    order = list(face.getGlyphOrder())
    subtables = [t for t in face["cmap"].tables if t.isUnicode()]
    for cp in sorted(wanted - set(cmap)):
        donor = next(
            (d for d, allowed in donors if cp in d.getBestCmap() and cp in allowed), None
        )
        if donor is None:
            continue
        assert donor["head"].unitsPerEm == face["head"].unitsPerEm
        src = donor.getBestCmap()[cp]
        dx = (adv - donor["hmtx"][src][0]) / 2  # <0: the donor is wider
        flat = DecomposingRecordingPen(donor.getGlyphSet())  # may be composite
        donor.getGlyphSet()[src].draw(flat)
        pen = TTGlyphPen(None)
        flat.replay(TransformPen(pen, (1, 0, 0, 1, dx, 0)))
        glyph = pen.glyph()
        name = f"borrowed{cp:04X}"
        face["glyf"][name] = glyph
        glyph.recalcBounds(face["glyf"])
        face["hmtx"][name] = (adv, getattr(glyph, "xMin", 0))
        order.append(name)
        for t in subtables:
            if cp <= 0xFFFF or t.format != 4:
                t.cmap[cp] = name
    face.setGlyphOrder(order)
    face["glyf"].glyphOrder = order
    face.save(dest)


FACES = [
    # output stem, DejaVu Sans Mono face, DejaVu Sans face (Braille donor)
    ("DejaVuSansMono-subset", "DejaVuSansMono", "DejaVuSans"),
    ("DejaVuSansMono-Bold-subset", "DejaVuSansMono-Bold", "DejaVuSans-Bold"),
    ("DejaVuSansMono-Oblique-subset", "DejaVuSansMono-Oblique", "DejaVuSans-Oblique"),
    ("DejaVuSansMono-BoldOblique-subset", "DejaVuSansMono-BoldOblique", "DejaVuSans-BoldOblique"),
]  # fmt: skip


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--emoji", choices=["common", "base"], default="common")
    ap.add_argument("--out", type=Path, default=ROOT / "assets" / "fonts")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    noto = noto_emoji()
    noto_cmap = set(TTFont(noto).getBestCmap())
    mono_cmap = set(TTFont(dejavu_dir() / "DejaVuSansMono.ttf").getBestCmap())
    docshot = set(DOCSHOT_CODEPOINTS)

    # -- emoji face
    if args.emoji == "base":
        emoji = emoji_presentation() - set(SKIN_TONES)
    else:
        emoji_pres = emoji_presentation()
        emoji = ranges(COMMON_EMOJI_RANGES) | set(COMMON_EMOJI)
        emoji |= {c for c in emoji_pres if c < 0x2C00}
    emoji = (emoji - set(SKIN_TONES)) & noto_cmap
    # A docshot codepoint DejaVu lacks but Noto has is drawn by the emoji face.
    emoji |= (docshot - mono_cmap) & noto_cmap
    # Text faces draw what the emoji face does not (see the docstring).
    text = ranges(TEXT_RANGES) | docshot | set(EXTRA_CODEPOINTS)
    text -= emoji
    # The joiner and selectors go in both faces (see EXTRA_CODEPOINTS).
    emoji |= set(EXTRA_CODEPOINTS) & noto_cmap
    text |= set(EXTRA_CODEPOINTS)
    run_subset(noto, emoji, args.out / "NotoColorEmoji-COLRv1-subset.ttf")
    for stem, mono, sans in FACES:
        with tempfile.TemporaryDirectory() as tmp:
            merged = Path(tmp) / f"{mono}.ttf"
            donors = [
                (dejavu_dir() / "DejaVuSansMono.ttf", range(0x110000)),
                (sans_dir() / f"{sans}.ttf", range(0x2800, 0x2900)),  # Braille only
            ]
            borrow_missing(dejavu_dir() / f"{mono}.ttf", donors, text, merged)
            run_subset(merged, text, args.out / f"{stem}.ttf")

    for p in sorted(args.out.glob("*-subset.ttf")):
        print(f"{p.stat().st_size:>9}  {p.name}")


def sans_dir() -> Path:
    return Path(os.environ.get("TOQUE_DEJAVU_SANS_DIR") or dejavu_dir())


if __name__ == "__main__":
    main()
