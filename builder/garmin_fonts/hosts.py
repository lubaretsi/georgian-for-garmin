"""Host families: the kinds of Garmin font Georgian is added to, as the registry's `hosts` lists
them.

A host says what its fonts' Georgian is sized by (`x_height` letters, and for bitmap hosts
`capitals` to break ties; see `georgian.choose_sizing` and `truetype.choose_sizing`) and,
optionally, an activation marker in its own script for test builds: `glyph` drawn with the tile
of `drawn_as`, so a photo shows a build is active even where the watch takes no Georgian from it.
"""

from dataclasses import dataclass

from . import registry as font_registry


@dataclass(frozen=True)
class Host:
    """One host family from the registry."""

    id: str
    kind: str
    script: str
    x_height: str
    capitals: str | None = None
    marker: tuple[str, str] | None = None

    @property
    def markers(self):
        """The marker as `add_georgian` takes it: {glyph: drawn_as}, or {} without one."""
        return dict([self.marker]) if self.marker else {}


def hosts(registry=None):
    """{id: Host} for the registry's `hosts` (the repository's registry by default)."""
    registry = font_registry.default() if registry is None else registry
    return {
        entry["id"]: Host(
            entry["id"],
            entry["kind"],
            entry["script"],
            entry["size_like"]["x_height"],
            entry["size_like"].get("capitals"),
            (entry["marker"]["glyph"], entry["marker"]["drawn_as"]) if "marker" in entry else None,
        )
        for entry in registry["hosts"]
    }


def get(host_id, registry=None):
    """The Host with `host_id`; raises ValueError naming the known ones when there is none."""
    known = hosts(registry)
    if host_id not in known:
        raise ValueError(f"unknown host {host_id!r}; the registry has {', '.join(known)}")
    return known[host_id]
