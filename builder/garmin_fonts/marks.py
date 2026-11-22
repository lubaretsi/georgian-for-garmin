"""Beta test marks: in a pre-release build each part's Georgian marks one capital of TEST_CAPS
with a bar, so a photo of the test notification tells which part the watch rendered.

Variants are assigned per font kind (bitmap, TrueType) in registry order, packs in order and
parts in pack order: the i-th part is variant `chr(ord("A") + i)`, marks TEST_CAPS[i % 8], and
draws its bar below, above or on both sides of the letter (STYLES[i // 8]). Adding or moving a
part in the registry changes the variants after it.
"""

from dataclasses import dataclass

# The capitals of the test text `Test მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ Test`: eight distinct Mtavruli letters.
TEST_CAPS = "ᲛᲗᲐᲕᲠᲣᲚᲘ"
# Where the bar goes: under the letter below the baseline, over its top, or both.
STYLES = ("below", "above", "both")


@dataclass(frozen=True)
class Mark:
    """One part's test mark: its variant letter, the marked capital's index in TEST_CAPS, and
    the bar's style."""

    variant: str
    position: int
    style: str

    @property
    def code_point(self):
        return ord(TEST_CAPS[self.position])

    @property
    def sides(self):
        """The bar's sides: ("below",), ("above",) or both."""
        return ("below", "above") if self.style == "both" else (self.style,)


def assign(packs):
    """{part number: Mark} for every part of the registry's `packs`.

    Raises ValueError when a kind has more parts than the variants can tell apart.
    """
    marks = {}
    for kind in sorted({pack["kind"] for pack in packs}):
        parts = [part["part"] for pack in packs if pack["kind"] == kind for part in pack["parts"]]
        if len(parts) > len(TEST_CAPS) * len(STYLES):
            raise ValueError(f"{len(parts)} {kind} parts are more than the test marks can tell apart")
        for i, number in enumerate(parts):
            position, style = i % len(TEST_CAPS), STYLES[i // len(TEST_CAPS)]
            marks[number] = Mark(chr(ord("A") + i), position, style)
    return marks


def manifest_entry(mark, picture, picture_sha256):
    """The manifest's `mark` field for a part."""
    return {
        "variant": mark.variant,
        "code_point": f"U+{mark.code_point:04X}",
        "position": mark.position,
        "style": mark.style,
        "picture": picture,
        "picture_sha256": picture_sha256,
    }
