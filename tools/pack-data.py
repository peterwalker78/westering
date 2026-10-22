#!/usr/bin/env python3
"""Packs the star catalogue and constellation figures into the compact files
the app embeds. Run once when the sources change; the outputs are committed.

Sources (downloaded to tools/raw/ by scripts/pack-data):
  bsc5.dat.gz                Yale Bright Star Catalogue, 5th revised edition
                             (Hoffleit & Warren 1991), CDS catalogue V/50
  stellarium-modern.json     Stellarium's "modern" sky culture, CC BY-SA 4.0:
                             the stick figures, by Hipparcos number
  constellations.lines.json  d3-celestial by Olaf Frohn, BSD-3-Clause, for
                             the order of the figures and Serpens' two halves
  constellations.json        d3-celestial, for English names and ranks
And tools/hip-to-hr.json, each Hipparcos star in the figures matched to its
HR number through SIMBAD (a double without an HR identifier in SIMBAD takes
its brightest BSC component within 20 arcseconds).

Outputs:
  core/data/stars.bin            14-byte little-endian records, brightest first:
                                 u16 HR, f32 RA (deg, J2000), f32 Dec (deg),
                                 i16 V magnitude x100, i16 B-V x100
                                 (B-V of 99.99 means unknown)
  core/data/constellations.txt   one figure per line:
                                 abbrev|name|rank|a-b a-b ...
                                 where a and b are HR numbers
"""

import gzip
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RAW = ROOT / "tools" / "raw"
OUT = ROOT / "core" / "data"


def field(line, start, end):
    """Bytes start..end, 1-based and inclusive, as in the catalogue's ReadMe."""
    return line[start - 1 : end].strip()


def stars():
    rows = []
    with gzip.open(RAW / "bsc5.dat.gz", "rt", encoding="latin-1") as f:
        for line in f:
            line = line.rstrip("\n").ljust(197)
            ra_h = field(line, 76, 77)
            vmag = field(line, 103, 107)
            if not ra_h or not vmag:
                continue  # novae and non-stellar objects kept for numbering
            ra = 15 * (int(ra_h) + int(field(line, 78, 79)) / 60 + float(field(line, 80, 83)) / 3600)
            dec = int(field(line, 85, 86)) + int(field(line, 87, 88)) / 60 + int(field(line, 89, 90)) / 3600
            if field(line, 84, 84) == "-":
                dec = -dec
            bv = field(line, 110, 114)
            rows.append(
                (
                    int(field(line, 1, 4)),
                    ra,
                    dec,
                    round(float(vmag) * 100),
                    round(float(bv) * 100) if bv else 9999,
                )
            )
    rows.sort(key=lambda r: r[3])
    return rows


# Where d3-celestial's English name is an asterism or reads oddly after "the".
NAMES = {
    "UMa": "Ursa Major, the Great Bear",
    "UMi": "Ursa Minor, the Little Bear",
    "Com": "Coma Berenices, Berenice's Hair",
    "Cap": "Capricornus, the Sea Goat",
    "Ser": "Serpens, the Serpent",
    "CrA": "Corona Australis, the Southern Crown",
    "Cae": "Caelum, the Chisel",
    "Dor": "Dorado, the Dolphinfish",
    "Hya": "Hydra, the Water Snake",
    "Hyi": "Hydrus, the Lesser Water Snake",
    "Nor": "Norma, the Set Square",
    "Ret": "Reticulum, the Reticle",
    "Pic": "Pictor, the Painter's Easel",
    "Men": "Mensa, Table Mountain",
    "Cir": "Circinus, the Compasses",
    "Pyx": "Pyxis, the Mariner's Compass",
    "Cet": "Cetus, the Sea Monster",
    "Aqr": "Aquarius, the Water Bearer",
    "Eri": "Eridanus, the River",
    "Oph": "Ophiuchus, the Serpent Bearer",
    "Cha": "Chamaeleon, the Chameleon",
}


# Where Stellarium's figure passes through the fainter star of a close pair,
# or a faint neighbour, rather than the star people see there.
SWAP = {
    2583: 2580,  # HD 50896 (V 6.9) for omicron-1 CMa
    5991: 5987,  # HR 5991 (V 5.7) for theta Lup
    3043: 3045,  # HR 3043 (V 5.3) for xi Pup
    7431: 7440,  # 51 Sgr (V 5.6) for 52 Sgr
    8449: 8454,  # pi-1 Peg (V 5.6) for pi-2
    6262: 6271,  # zeta-1 Sco (V 4.7) for zeta-2
    221: 224,  # HR 221 (V 5.9) for delta Psc
}


def figures():
    names = {f["id"]: f["properties"] for f in json.load(open(RAW / "constellations.json"))["features"]}
    order = [f["id"] for f in json.load(open(RAW / "constellations.lines.json"))["features"]]
    hip_to_hr = {int(k): v for k, v in json.load(open(ROOT / "tools" / "hip-to-hr.json")).items()}
    edges, dropped = {}, []
    for con in json.load(open(RAW / "stellarium-modern.json"))["constellations"]:
        abbrev = con["id"].split()[-1]
        found = []
        for strand in con["lines"]:
            for a, b in zip(strand, strand[1:]):
                ha, hb = hip_to_hr.get(a), hip_to_hr.get(b)
                if ha is None or hb is None:
                    dropped.append((abbrev, a, b))
                    continue
                ha, hb = SWAP.get(ha, ha), SWAP.get(hb, hb)
                if ha != hb and (ha, hb) not in found and (hb, ha) not in found:
                    found.append((ha, hb))
        edges[abbrev] = found
    # Serpens is drawn as two figures, the head (with beta Ser) and the tail.
    serpens = edges.pop("Ser")
    joined = {}
    for a, b in serpens:
        joined.setdefault(a, set()).add(b)
        joined.setdefault(b, set()).add(a)
    head, stack = set(), [5867]
    while stack:
        x = stack.pop()
        if x not in head:
            head.add(x)
            stack.extend(joined.get(x, ()))
    halves = [[e for e in serpens if e[0] in head], [e for e in serpens if e[0] not in head]]
    out = []
    for abbrev in order:
        found = halves.pop(0) if abbrev == "Ser" else edges[abbrev]
        props = names[abbrev]
        pairs = " ".join(f"{a}-{b}" for a, b in found)
        # The Latin name people know, and the English one where it says more.
        latin, english = props["name"], props["en"]
        name = latin if english in (latin, "") else f"{latin}, the {english}"
        name = NAMES.get(abbrev, name)
        out.append(f"{abbrev}|{name}|{props['rank']}|{pairs}")
    return out, dropped


def main():
    catalogue = stars()
    with open(OUT / "stars.bin", "wb") as f:
        for hr, ra, dec, vmag, bv in catalogue:
            f.write(struct.pack("<Hffhh", hr, ra, dec, vmag, bv))
    figs, dropped = figures()
    (OUT / "constellations.txt").write_text(
        "# Constellation figures from Stellarium's modern sky culture, CC BY-SA 4.0,\n"
        "# with each star matched to its Yale Bright Star Catalogue HR number; names\n"
        "# and ranks from d3-celestial (c) 2015 Olaf Frohn, BSD-3-Clause.\n"
        "# abbrev|name|rank|HR-HR pairs\n" + "\n".join(figs) + "\n"
    )
    for abbrev, a, b in dropped:
        print(f"{abbrev}: HIP {a}-{b} left out, a star not in the BSC", file=sys.stderr)
    print(f"{len(catalogue)} stars, {len(figs)} figures", file=sys.stderr)


main()
