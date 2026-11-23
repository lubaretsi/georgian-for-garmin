import os
from concurrent.futures import ProcessPoolExecutor
from struct import pack_into, unpack_from

import pytest
from registry_data import PART_IDS, STOCK_PART_IDS

from garmin_fonts.codec import (
    COMPRESSED,
    HEADER,
    PLAIN,
    compressed,
    decode,
    encode,
    identity,
    rle_decode,
    rle_encode,
    round_trips,
)

# The registry's stock parts Garmin publishes "Compressed" (per-glyph RLE): the Lily 2 Active's
# Chinese, Japanese and Korean fonts. The Venu 2's are plain, like every pack part.
COMPRESSED_PARTS = {"006-D8448-70", "006-D8448-71", "006-D8944-66", "006-D8944-67",
                    "006-D8945-67", "006-D8945-68"}


@pytest.mark.parametrize("part", PART_IDS)
def test_stock_round_trips_byte_identically(stock, part):
    assert encode(*decode(stock[part])) == stock[part]


@pytest.fixture(scope="session")
def catalog_round_trips(catalog_stock):
    """{part: round trip equal} for every registry `stock` part, in parallel: re-compressing
    every glyph of the large compressed fonts takes a while."""
    with ProcessPoolExecutor(max_workers=os.cpu_count()) as pool:
        return dict(zip(STOCK_PART_IDS, pool.map(round_trips, catalog_stock.values())))


@pytest.mark.parametrize("part", STOCK_PART_IDS)
def test_catalog_stock_round_trips_byte_identically(catalog_stock, catalog_round_trips, part):
    header, footer, _ = decode(catalog_stock[part])
    assert identity(footer)[0] == part
    assert compressed(header) == (part in COMPRESSED_PARTS)
    assert header[1] == (COMPRESSED if part in COMPRESSED_PARTS else PLAIN)
    assert catalog_round_trips[part]


def test_reused_streams_give_the_same_bytes(catalog_stock):
    data = catalog_stock["006-D8945-67"]
    streams = {}
    header, footer, cmap = decode(data, streams)
    assert streams and encode(header, footer, cmap, streams) == data
    # A changed glyph is compressed afresh; the rest keep their streams.
    width, bitmap = cmap[0x41]
    cmap[0x41] = (width, bytes(len(bitmap)))
    assert encode(header, footer, cmap, streams) == encode(header, footer, cmap)


def test_rle_streams_round_trip(catalog_stock):
    _, _, cmap = decode(catalog_stock["006-D8944-66"])
    tiles = sorted({bitmap for _, bitmap in cmap.values() if bitmap})[:300]
    for bitmap in tiles + [bytes(12), b"\xff" * 40, bytes(range(256))]:
        assert rle_decode(rle_encode(bitmap), len(bitmap)) == bitmap


def test_malformed_rle_is_rejected(catalog_stock):
    _, _, cmap = decode(catalog_stock["006-D8944-66"])
    bitmap = next(b for _, b in cmap.values() if len(b) > 20 and len(set(b)) > 3)
    stream = rle_encode(bitmap)
    with pytest.raises(ValueError, match="truncated RLE glyph"):
        rle_decode(stream[:-2], len(bitmap))
    with pytest.raises(ValueError, match="trailing bytes"):
        rle_decode(stream + b"\x00\x00", len(bitmap))
    with pytest.raises(ValueError, match="bad RLE header"):
        rle_decode(bytes(8), 4)


def test_malformed_compressed_font_is_rejected(catalog_stock):
    data = bytearray(catalog_stock["006-D8944-66"])
    index_entry(2, 1)(data)  # one glyph's stream gains a byte, the next loses one
    with pytest.raises(ValueError, match="RLE"):
        decode(bytes(data))


def test_footer_names_part_and_version(stock):
    _, footer, _ = decode(stock["006-D9035-03"])
    assert identity(footer) == ("006-D9035-03", "2.10")


def test_decode_names_its_fields_and_prints_briefly(stock, catalog_stock):
    decoded = decode(stock["006-D9035-03"])
    header, footer, cmap = decoded
    assert (decoded.header, decoded.footer, decoded.cmap) == (header, footer, cmap)
    assert (header.height, header.baseline, header.flags, header.magic) == \
        (header[7], header[8], PLAIN, 0x12345678)
    assert (decoded.part, decoded.version) == ("006-D9035-03", "2.10")
    assert repr(decoded) == ("Decoded(part='006-D9035-03', version='2.10', plain, "
                             f"height={header.height}, baseline={header.baseline}, "
                             f"glyphs={len(cmap)})")
    assert "compressed" in repr(decode(catalog_stock["006-D8945-67"]))


@pytest.mark.parametrize("part", PART_IDS)
def test_truncated_input_is_rejected(stock, part):
    data = stock[part]
    lengths = {*range(0, len(data), 97), HEADER.size - 1, HEADER.size, len(data) - 21, len(data) - 1}
    for length in sorted(lengths):
        with pytest.raises(ValueError):
            decode(data[:length])


def u32(offset, value):
    return lambda data: pack_into("<I", data, offset, value)


def add_u32(offset, delta):
    return lambda data: pack_into("<I", data, offset, unpack_from("<I", data, offset)[0] + delta)


def swap_first_ranges(data):
    first, second = data[52:64], data[64:76]
    data[52:64], data[64:76] = second, first


def range_field(row, column, delta):
    return add_u32(52 + 12 * row + 4 * column, delta)


def last_range_hi(delta):
    def mutate(data):
        count = unpack_from("<I", data, 48)[0]
        add_u32(52 + 12 * (count - 1) + 4, delta)(data)

    return mutate


def footer_byte(index, value):
    def mutate(data):
        data[len(data) - 21 + index] = value

    return mutate


def index_entry(entry, delta):
    def mutate(data):
        add_u32(unpack_from("<I", data, 12)[0] + 4 * entry, delta)(data)

    return mutate


def invert_first_range(data):
    pack_into("<I", data, 56, unpack_from("<I", data, 52)[0] - 1)


# Header fields: end @4, ranges @8, index @12, pixels @16, magic @32; range rows from @52.
MALFORMED = {
    "magic": (u32(32, 0x12345679), "not a supported"),
    "flags": (lambda data: pack_into("<H", data, 2, 0x0402), "not a supported"),
    "end past footer": (add_u32(4, 1), "invalid section bounds"),
    "unaligned index": (add_u32(16, 2), "unaligned glyph index"),
    "range count": (add_u32(48, 1), "invalid cmap header"),
    "footer magic": (footer_byte(0, 0), "unexpected font footer"),
    "footer terminator": (footer_byte(20, 1), "unexpected font footer"),
    "bitmap offset": (index_entry(2, 1), "noncontiguous"),
    "glyph width": (index_entry(1, 4 << 24), "noncontiguous"),
    "range order": (swap_first_ranges, "invalid Unicode range"),
    "range first index": (range_field(1, 2, 1), "invalid Unicode range"),
    "range inverted": (invert_first_range, "invalid Unicode range"),
    "unmapped glyphs": (last_range_hi(-1), "unmapped glyph table entries"),
}


@pytest.mark.parametrize("part", PART_IDS)
@pytest.mark.parametrize("case", MALFORMED)
def test_malformed_input_is_rejected(stock, part, case):
    mutate, message = MALFORMED[case]
    data = bytearray(stock[part])
    mutate(data)
    with pytest.raises(ValueError, match=message):
        decode(bytes(data))


def test_encode_rejects_invalid_glyphs(stock):
    header, footer, cmap = decode(stock["006-D9035-03"])
    width, bitmap = cmap[None]
    with pytest.raises(ValueError, match="glyph dimensions"):
        encode(header, footer, {**cmap, 0x41: (width, bitmap[:-1])})
    with pytest.raises(ValueError, match="invalid code point"):
        encode(header, footer, {**cmap, 0x110000: cmap[None]})
