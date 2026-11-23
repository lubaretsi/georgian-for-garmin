"""The stock font catalog: pinned parts from the nix directories, the cache or their URL, others
from Garmin's catalog; offline never downloads."""

import hashlib
import io
import json

import pytest
from registry_data import ENTRIES, PINS

from garmin_fonts import catalog
from garmin_fonts.catalog import CatalogError, fetch_stock


class Network:
    """A stand-in for the network: URL (or the catalog endpoint) to bytes, and every request
    made."""

    def __init__(self, responses):
        self.responses, self.requests = responses, []

    def __call__(self, request, timeout=None):
        url = getattr(request, "full_url", request)
        self.requests.append(url)
        if url not in self.responses:
            raise OSError(f"no route to {url}")
        return io.BytesIO(self.responses[url])


@pytest.fixture
def isolated(monkeypatch, tmp_path):
    """No nix directories, an empty cache, online, and no real network."""
    for var in (*catalog.PINNED_DIRS, "GFG_OFFLINE"):
        monkeypatch.delenv(var, raising=False)
    monkeypatch.setenv("GFG_CATALOG_CACHE", str(tmp_path / "cache"))
    network = Network({})
    monkeypatch.setattr(catalog, "_open", network)
    return network


def test_every_stock_part_is_pinned_with_its_host_and_size():
    for part in (s["part"] for s in ENTRIES["stock"]):
        pin = PINS[part]
        assert pin.kind == "bitmap" and pin.pack is None
        assert f"{pin.size_px}px" in pin.name and pin.url.endswith(f"/{part}.fnt")
        assert f"/{part}/{pin.version}/" in pin.url


def test_pinned_parts_come_from_the_nix_directories(catalog_stock):
    stock = fetch_stock("006-D8448-70", offline=True)
    assert stock.source == "nix" and stock.pinned and stock.version == "2.00"
    assert stock.data == catalog_stock["006-D8448-70"]
    assert stock.sha256 == PINS["006-D8448-70"].sha256
    assert fetch_stock("006-D9035-03", offline=True).name == "Armenian Font, NotoSans Regular 27 px"


def test_offline_without_the_file_fails_clearly(isolated):
    with pytest.raises(CatalogError, match="not at hand offline") as error:
        fetch_stock("006-D8947-22", offline=True)
    assert error.value.code == "font_unavailable"
    assert isolated.requests == []


def test_a_pinned_download_is_checked_and_cached(isolated, catalog_stock):
    pin = PINS["006-D8947-22"]
    isolated.responses[pin.url] = catalog_stock[pin.part]
    first = fetch_stock(pin.part)
    assert first.source == "download" and first.data == catalog_stock[pin.part]
    assert first.path == catalog.cache_dir() / pin.part / pin.version / f"{pin.part}.fnt"
    again = fetch_stock(pin.part, offline=True)
    assert again.source == "cache" and again.data == first.data
    assert isolated.requests == [pin.url]


def test_a_download_that_differs_from_the_pin_is_refused(isolated):
    pin = PINS["006-D8947-22"]
    isolated.responses[pin.url] = b"not the font"
    with pytest.raises(CatalogError, match="but the registry pins"):
        fetch_stock(pin.part)
    assert not (catalog.cache_dir() / pin.part).exists()


def test_a_corrupt_cache_entry_is_fetched_again(isolated, catalog_stock):
    pin = PINS["006-D8947-22"]
    cached = catalog.cache_dir() / pin.part / pin.version / f"{pin.part}.fnt"
    cached.parent.mkdir(parents=True)
    cached.write_bytes(b"half a font")
    isolated.responses[pin.url] = catalog_stock[pin.part]
    assert fetch_stock(pin.part).source == "download"
    assert cached.read_bytes() == catalog_stock[pin.part]


def catalog_answer(part, data, version="2.00"):
    url = f"https://download.garmin.com/gsup/{part}/{version}/full/rel/x/{part}.fnt"
    return url, json.dumps({"updates": [{
        "partNumber": part, "productName": "Test Font 21px", "version": version,
        "deliverable": {"secureDownloadUrl": url, "sizeInBytes": len(data),
                        "md5": hashlib.md5(data).hexdigest()},
    }]}).encode()


def test_unpinned_parts_come_from_garmins_catalog(isolated, catalog_stock):
    data = catalog_stock["006-D8947-20"]
    url, answer = catalog_answer("006-D8947-99", data)
    isolated.responses |= {catalog.ENDPOINT: answer, url: data}
    stock = fetch_stock("006-D8947-99")
    assert (stock.pinned, stock.source, stock.version, stock.name) == \
        (False, "catalog", "2.00", "Test Font 21px")
    offline = fetch_stock("006-D8947-99", offline=True)
    assert (offline.source, offline.data) == ("cache", data)


def test_an_unpinned_download_must_match_the_catalog(isolated, catalog_stock):
    url, answer = catalog_answer("006-D8947-99", catalog_stock["006-D8947-20"])
    isolated.responses |= {catalog.ENDPOINT: answer, url: b"something else"}
    with pytest.raises(CatalogError, match="size and MD5"):
        fetch_stock("006-D8947-99")


def test_unknown_and_malformed_parts(isolated):
    isolated.responses[catalog.ENDPOINT] = json.dumps({"updates": []}).encode()
    with pytest.raises(CatalogError, match="returned 0 entries"):
        fetch_stock("006-D8947-99")
    with pytest.raises(CatalogError, match="not a Garmin part number") as error:
        fetch_stock("D8947-99")
    assert error.value.code == "invalid_input"
    with pytest.raises(CatalogError, match="never fetched"):
        fetch_stock("006-D8947-98", offline=True)


def test_kind_comes_from_the_bytes_and_must_agree(isolated, catalog_stock, truetype_stock):
    # Garmin's catalog answers a request for a .fnt with a TrueType .bin too.
    data = truetype_stock["006-D1053-98"]
    url, answer = catalog_answer("006-D0952-99", data)
    isolated.responses |= {catalog.ENDPOINT: answer, url: data}
    stock = fetch_stock("006-D0952-99")
    assert stock.kind == "truetype" and stock.path.name == "006-D0952-99.bin"
    assert fetch_stock("006-D0952-99", offline=True).kind == "truetype"
    with pytest.raises(CatalogError, match="is a truetype font, not bitmap") as error:
        fetch_stock("006-D0952-99", kind="bitmap")
    assert error.value.code == "invalid_input"
    with pytest.raises(CatalogError, match="006-D8947-22 is a bitmap part, not truetype"):
        fetch_stock("006-D8947-22", kind="truetype")
    with pytest.raises(CatalogError, match="kind is bitmap or truetype"):
        fetch_stock("006-D8947-22", kind="otf")


def test_a_truetype_file_cached_as_fnt_is_moved(isolated, truetype_stock):
    data = truetype_stock["006-D1053-98"]
    url, answer = catalog_answer("006-D0952-99", data)
    isolated.responses |= {catalog.ENDPOINT: answer, url: data}
    old = catalog.cache_dir() / "006-D0952-99" / "2.00" / "006-D0952-99.fnt"
    old.parent.mkdir(parents=True)
    old.write_bytes(data)
    stock = fetch_stock("006-D0952-99")
    assert stock.source == "cache" and stock.path.suffix == ".bin" and not old.exists()
    assert url not in isolated.requests


@pytest.mark.parametrize("name, weight", [
    ("APAC Font, Nanum Gothic Bold 27px, KOR", 700),
    ("APAC Font, Kosugi Medium 21px, JPN", 500),
    ("Armenian Font, NotoSans Regular 27px, ARM", 400),
    ("DB, APAC Font, Nanum Gothic ExtraBold Size 3", 800),
    ("Extended Emoji Font, Twemoji Extended 30px ARGB4444", None),
    ("Font, Medium Bold mix", None),
])
def test_weight_from_a_catalog_name(name, weight):
    assert catalog.weight_of(name) == weight


def test_a_corrupt_unpinned_cache_entry_is_refused_offline(isolated, catalog_stock):
    data = catalog_stock["006-D8947-20"]
    url, answer = catalog_answer("006-D8947-99", data)
    isolated.responses |= {catalog.ENDPOINT: answer, url: data}
    path = fetch_stock("006-D8947-99").path
    path.write_bytes(data[:-1] + b"\x01")  # same size, different MD5
    with pytest.raises(CatalogError, match="doesn't match the size and MD5"):
        fetch_stock("006-D8947-99", offline=True)
    path.write_bytes(data + b"\x00")
    with pytest.raises(CatalogError, match="doesn't match the size and MD5"):
        fetch_stock("006-D8947-99", offline=True)
    (path.parent / "catalog.json").unlink()
    with pytest.raises(CatalogError, match="no readable catalog entry"):
        fetch_stock("006-D8947-99", offline=True)
    # Online, the corrupt file is fetched again.
    assert fetch_stock("006-D8947-99").source == "catalog"
    assert fetch_stock("006-D8947-99", offline=True).data == data


def test_gfg_offline_means_offline(isolated, monkeypatch):
    monkeypatch.setenv("GFG_OFFLINE", "1")
    with pytest.raises(CatalogError, match="offline"):
        fetch_stock("006-D8947-22")
    assert isolated.requests == []


def test_the_georgian_font_is_pinned(inputs):
    assert catalog.georgian_font(offline=True) == inputs["noto"]
    assert hashlib.sha256(inputs["noto"].read_bytes()).hexdigest() == \
        ENTRIES["georgian_font"]["sha256"]
