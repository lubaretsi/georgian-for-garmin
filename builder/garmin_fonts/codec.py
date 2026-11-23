"""Codec for Garmin's 2-bit ExtData fonts (`.fnt`, staged on the watch as `.tmp`), plain and
"Compressed".

Layout: a 36-byte little-endian header, a table of sorted Unicode ranges, one 32-bit index
entry per glyph (8-bit width above a 24-bit glyph data offset), the glyph data, and a 21-byte
footer carrying the part number and release version. A glyph is fixed-height rows of four 2-bit
pixels per byte, least-significant pixel first. Unknown header and footer fields are preserved.

The header's 16-bit `flags` is 0x0403 for plain fonts and 0x0603 for "Compressed" ones (the
catalog's name for them): bit 0x0200 marks per-glyph RLE. Each compressed glyph is the RLE
stream of its plain bitmap (`ceil(width / 4) * height` bytes); index offsets point at the
variable-length streams, which are contiguous. This is the glyph format the Connect IQ SDK's
font compiler writes (`monkeybrains.jar`: `GarminFontObject.writeGlyph` with
`RLECompressor.compress`), worked out in the lab record 2026-11-23-cjk-bitmap-host.

`decode` returns plain tiles either way; `encode` writes the template's own format, compressing
each glyph exactly as Garmin's tool does, so every stock font re-encodes byte for byte.
"""

import re
from collections import Counter
from struct import Struct, iter_unpack, pack, unpack_from
from typing import NamedTuple

HEADER = Struct("<HH4I6HI")


class Header(NamedTuple):
    """The 36-byte header's fields; `unknown*` are preserved as they are."""

    size: int
    flags: int
    end: int
    ranges_start: int
    index_start: int
    pixels_start: int
    unknown6: int
    height: int
    baseline: int
    unknown9: int
    unknown10: int
    unknown11: int
    magic: int


class Decoded(NamedTuple):
    """A decoded font: its Header, its 21-byte footer, and `cmap` (code point, or None for the
    missing glyph, to a plain (width, bitmap) tile). Unpacks as (header, footer, cmap)."""

    header: Header
    footer: bytes
    cmap: dict

    @property
    def part(self):
        return identity(self.footer)[0]

    @property
    def version(self):
        return identity(self.footer)[1]

    def __repr__(self):
        kind = "compressed" if compressed(self.header) else "plain"
        return (f"Decoded(part={self.part!r}, version={self.version!r}, {kind}, "
                f"height={self.header.height}, baseline={self.header.baseline}, "
                f"glyphs={len(self.cmap)})")


FOOTER_SIZE = 21
OFFSET_MASK = 0xFFFFFF
PLAIN, COMPRESSED = 0x0403, 0x0603
# Modern Mkhedruli and Mtavruli.
GEORGIAN = (*range(0x10D0, 0x10F1), *range(0x1C90, 0x1CB1))


def identity(footer):
    """Return the (part number, release version) a decoded footer names: ("006-D9035-03", "2.10")."""
    version = unpack_from("<H", footer, 6)[0]
    return footer[8:-1].decode("ascii", "replace"), "%d.%02d" % divmod(version, 100)


def compressed(header):
    """Whether a decoded header is a "Compressed" (per-glyph RLE) font's."""
    return header[1] == COMPRESSED


# Per-glyph RLE (RLECompressor and RLEOutputStream in the Connect IQ SDK).
#
# Bit stream, least-significant bit first: (repeat_bits - 1) zero bits and a one bit; 5 bits of
# symbol_bits - 1; the escape symbol. Then symbols of `symbol_bits` bits: a symbol other than
# the escape is a literal; the escape is followed by a `repeat_bits` count c: c = 0 is a literal
# escape symbol, c > 0 repeats the previous symbol c + 1 more times. The decoder stops when the
# glyph's plain size is filled; the last byte is zero-padded.


class _Bits:
    def __init__(self, data):
        self.data, self.pos, self.buf, self.left = data, 0, 0, 0

    def read(self, n):
        while n > self.left:
            if self.pos >= len(self.data):
                raise ValueError("truncated RLE glyph")
            self.buf |= self.data[self.pos] << self.left
            self.pos += 1
            self.left += 8
        value = self.buf & ((1 << n) - 1)
        self.buf >>= n
        self.left -= n
        return value


def rle_decode(data, size):
    """The `size`-byte plain bitmap of an RLE glyph stream; raises ValueError if malformed."""
    bits = _Bits(data)
    repeat_bits = 1
    while not bits.read(1):
        repeat_bits += 1
        if repeat_bits > 32:
            raise ValueError("bad RLE header")
    symbol_bits = bits.read(5) + 1
    escape = bits.read(symbol_bits)
    total = size * 8
    out, filled, last = 0, 0, None
    while filled < total:
        symbol = bits.read(symbol_bits)
        count = 1
        if symbol == escape:
            repeat = bits.read(repeat_bits)
            if repeat:
                if last is None:
                    raise ValueError("RLE repeat before any symbol")
                symbol, count = last, repeat + 1
        for _ in range(count):
            out |= symbol << filled
            filled += symbol_bits
        last = symbol
    if len(data) > bits.pos:
        raise ValueError("trailing bytes after RLE glyph")
    return (out & ((1 << total) - 1)).to_bytes(size, "little")


def _runs(bits, symbol_bits):
    """Runs [symbol, length] as RLECompressor reads them, including its tail handling.

    `bits` is the glyph's bit string, bit i at character i; each symbol is its
    `symbol_bits`-character chunk of it, least-significant bit first.
    """
    runs = [
        [symbol, len(run) // symbol_bits]
        for run, symbol in _RUN_PATTERNS[symbol_bits].findall(bits)
    ]
    tail = _tail_run(bits, symbol_bits, runs[-1] if runs else None)
    if tail == "extend":
        runs[-1][1] += 1
    elif tail:
        runs.append(list(tail))
    return runs


def _tail_run(bits, symbol_bits, last):
    """How the bits after the last whole symbol end the runs, given the `last` run [symbol,
    length]: None (no tail), "extend" (the last run grows by one) or a new (symbol, length)."""
    tail = bits[len(bits) - len(bits) % symbol_bits:]
    if not tail:
        return None
    if last is None:
        return tail.ljust(symbol_bits, "0"), 1
    if last[0].startswith(tail):
        return "extend"
    # The SDK gives the tail symbol the previous run's length; decoders truncate it.
    return tail.ljust(symbol_bits, "0"), last[1]


# A run of one symbol: the matches tile the bit string from its start, one aligned chunk after
# another, and stop before a tail shorter than a symbol.
_RUN_PATTERNS = {n: re.compile(f"(([01]{{{n}}})\\2*)") for n in range(1, 33)}


def _value(symbol):
    return int(symbol[::-1], 2)


def _escape(symbols, symbol_bits):
    """The first symbol value that starts no run, else the largest symbol value.

    The SDK bounds this search with a 32-bit `1 << symbol_bits`, which is negative for 31 and
    1 for 32, so those sizes always use escape 0 (seen in stock 32-bit glyphs).
    """
    if symbol_bits >= 31:
        return 0
    used = {_value(symbol) for symbol in symbols}
    for i in range(1 << symbol_bits):
        if i not in used:
            return i
    return (1 << symbol_bits) - 1


def _run_cost(length, is_escape, symbol_bits, repeat_bits):
    one = symbol_bits + (repeat_bits if is_escape else 0)
    repeat = symbol_bits + repeat_bits
    limit = 1 << repeat_bits
    rest = length - 1
    bits = one
    if rest > limit:
        chunks = (rest - 1) // limit  # while rest > limit: rest -= limit
        bits += chunks * repeat
        rest -= chunks * limit
    if rest == 1:
        bits += one
    elif rest > 1:
        bits += repeat
    return bits


def _layout(bits, symbol_bits):
    """(bytes, repeat_bits, escape, runs) of RLECompressor.compress at `symbol_bits`."""
    runs = _runs(bits, symbol_bits)
    escape = _escape({s for s, _ in runs}, symbol_bits)
    escape_symbol = format(escape, f"0{symbol_bits}b")[::-1]
    # A run's cost depends only on its length and whether its symbol is the escape.
    kinds = Counter((n, s == escape_symbol) for s, n in runs)
    longest = max(n for n, _ in kinds)
    max_bits = 1
    while (1 << max_bits) < longest:
        max_bits += 1
    best = None
    for repeat_bits in range(max_bits, 0, -1):
        total = repeat_bits + 5 + symbol_bits + sum(
            count * _run_cost(n, is_escape, symbol_bits, repeat_bits)
            for (n, is_escape), count in kinds.items()
        )
        size = (total + 7) // 8
        if best is None or size < best[0]:
            best = (size, repeat_bits)
    return best[0], best[1], escape, runs


def rle_encode(data):
    """Compress a plain glyph bitmap exactly as the Connect IQ SDK's RLECompressor does."""
    if not data:
        raise ValueError("empty glyph bitmap")
    bits = format(int.from_bytes(data, "little"), f"0{len(data) * 8}b")[::-1]
    best = None
    for symbol_bits in range(1, 33):
        layout = _layout(bits, symbol_bits)
        if best is None or layout[0] <= best[1][0]:
            best = (symbol_bits, layout)
    symbol_bits, (size, repeat_bits, escape, runs) = best
    runs = [(_value(symbol), length) for symbol, length in runs]
    out, filled = 0, 0

    def put(value, n):
        nonlocal out, filled
        out |= value << filled
        filled += n

    put(1 << (repeat_bits - 1), repeat_bits)
    put(symbol_bits - 1, 5)
    put(escape, symbol_bits)
    limit = 1 << repeat_bits

    def one(symbol):
        put(symbol, symbol_bits)
        if symbol == escape:
            put(0, repeat_bits)

    for symbol, length in runs:
        one(symbol)
        rest = length - 1
        while rest > limit:
            put(escape, symbol_bits)
            put(limit - 1, repeat_bits)
            rest -= limit
        if rest == 1:
            one(symbol)
        elif rest > 1:
            put(escape, symbol_bits)
            put(rest - 1, repeat_bits)
    result = out.to_bytes((filled + 7) // 8, "little")
    if len(result) != size:
        raise AssertionError("RLE size estimate disagrees with the stream")
    return result


def decode(data, streams=None):
    """Return a Decoded (header, footer, cmap), with the missing-glyph tile at cmap[None].

    Each cmap value is a plain (width, bitmap) tile, whether the font is plain or compressed.
    For a compressed font, a `streams` dict gets each bitmap's RLE stream as stored, for
    `encode` to reuse. Raises ValueError on malformed input.
    """
    if len(data) < HEADER.size:
        raise ValueError("truncated font header")
    header = Header._make(HEADER.unpack_from(data))
    size, flags, end, ranges_start, index_start, pixels_start, _, height, *_ = header
    if (size, ranges_start, header[-1]) != (36, 36, 0x12345678) or flags not in (PLAIN, COMPRESSED):
        raise ValueError("not a supported Garmin 2-bit font")
    if not (
        ranges_start + 16 <= index_start < pixels_start < end <= len(data) - FOOTER_SIZE
        and height
    ):
        raise ValueError("invalid section bounds")
    if (pixels_start - index_start) % 4:
        raise ValueError("unaligned glyph index")
    stride, span, reserved, count = unpack_from("<4I", data, ranges_start)
    if (stride, span, reserved) != (12, 16 + 12 * count, 0) or ranges_start + span != index_start:
        raise ValueError("invalid cmap header")
    footer = data[end:]
    if len(footer) != FOOTER_SIZE or footer[:6] != bytes.fromhex("e2ad4ef70100") or footer[-1]:
        raise ValueError("unexpected font footer")
    entries = [entry for (entry,) in iter_unpack("<I", data[index_start:pixels_start])]
    offsets = [entry & OFFSET_MASK for entry in entries] + [end - pixels_start]
    tiles = []
    for i, entry in enumerate(entries):
        width = entry >> 24
        plain = ((width + 3) // 4) * height
        start, stop = pixels_start + offsets[i], pixels_start + offsets[i + 1]
        if offsets[0] or start > stop or stop > end:
            raise ValueError("noncontiguous or out-of-bounds glyph data")
        raw = data[start:stop]
        if flags == PLAIN:
            if len(raw) != plain:
                raise ValueError("noncontiguous or out-of-bounds glyph data")
            tiles.append((width, raw))
        elif plain:
            tiles.append((width, rle_decode(raw, plain)))
            if streams is not None:
                streams[tiles[-1][1]] = raw
        elif raw:
            raise ValueError("data for an empty glyph")
        else:
            tiles.append((width, b""))
    if not tiles:
        raise ValueError("no glyphs")
    cmap = {None: tiles[0]}
    next_index, previous = 1, -1
    for lo, hi, first in iter_unpack("<III", data[ranges_start + 16:index_start]):
        if not previous < lo <= hi <= 0x10FFFF or first != next_index or first + hi - lo >= len(tiles):
            raise ValueError("invalid Unicode range")
        for cp in range(lo, hi + 1):
            cmap[cp] = tiles[next_index]
            next_index += 1
        previous = hi
    if next_index != len(tiles):
        raise ValueError("unmapped glyph table entries")
    return Decoded(header, bytes(footer), cmap)


def encode(header, footer, cmap, streams=None):
    """Rebuild the range and index tables and the glyph data in the template's format (plain or
    compressed); preserve the template's other metadata.

    A compressed glyph whose bitmap is in `streams` (from `decode`) is written as that stream
    instead of being compressed again. Garmin's streams are the ones `rle_encode` writes (the
    round trip of every pinned stock font shows it), so the bytes are the same, only faster.
    """
    rle = compressed(header)
    cache = dict(streams or {})
    keys = sorted(cp for cp in cmap if cp is not None)
    ranges = []
    offsets, glyphs = bytearray(), bytearray()
    for i, cp in enumerate([None, *keys]):
        if cp is not None:
            if not 0 <= cp <= 0x10FFFF:
                raise ValueError(f"invalid code point: {cp}")
            if ranges and cp == ranges[-1][1] + 1:
                ranges[-1][1] = cp
            else:
                ranges.append([cp, cp, i])
        width, bitmap = cmap[cp]
        if not 0 <= width <= 255 or len(bitmap) != ((width + 3) // 4) * header[7]:
            raise ValueError(f"invalid glyph dimensions: {cp}")
        if len(glyphs) > OFFSET_MASK:
            raise ValueError("glyph offset exceeds 24 bits")
        offsets += pack("<I", (width << 24) | len(glyphs))
        if rle and bitmap:
            if bitmap not in cache:
                cache[bitmap] = rle_encode(bitmap)
            glyphs += cache[bitmap]
        else:
            glyphs += bitmap
    ranges_size = 16 + 12 * len(ranges)
    ranges_start = HEADER.size
    index_start = ranges_start + ranges_size
    pixels_start = index_start + len(offsets)
    end = pixels_start + len(glyphs)
    return b"".join((
        HEADER.pack(*header[:2], end, ranges_start, index_start, pixels_start, *header[6:]),
        pack("<4I", 12, ranges_size, 0, len(ranges)),
        *(pack("<III", *r) for r in ranges),
        offsets,
        glyphs,
        footer,
    ))


def round_trips(data):
    """Whether `data` decodes and re-encodes byte for byte (without reusing its RLE streams, so
    a compressed font exercises `rle_encode`). Plain fonts take milliseconds; large compressed
    ones take 10 to 30 seconds, as every glyph is compressed again in pure Python."""
    return encode(*decode(data)) == data
