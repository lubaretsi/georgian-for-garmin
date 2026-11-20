from struct import pack_into, unpack_from

import pytest
from registry_data import PART_IDS

from garmin_fonts.codec import HEADER, decode, encode, identity


@pytest.mark.parametrize("part", PART_IDS)
def test_stock_round_trips_byte_identically(stock, part):
    assert encode(*decode(stock[part])) == stock[part]


def test_footer_names_part_and_version(stock):
    _, footer, _ = decode(stock["006-D9035-03"])
    assert identity(footer) == ("006-D9035-03", "2.10")


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
