#!/usr/bin/env python3
"""Builds the two `.ico` samples of sub-block G2, ON THE HOST, without Windows.

🔴 WHY THIS SCRIPT IS COMMITTED WITH THE TWO FILES IT PRODUCES.

The fact that governs the whole of sub-block G2 is this one: an `.ico` holding
ONLY ONE 48×48 entry, queried at 256 by the Windows Shell, returns **256×256
32bpp** — through `IShellItemImageFactory::GetImage` as through
`PrivateExtractIconsW`, without `SIIGBF_SCALEUP` and EVEN with
`SIIGBF_BIGGERSIZEOK`. The four render lines of the two samples are
identical; **only the `ICONDIR` line differs**. An acceptance criterion that
compared the returned size with 256 THEREFORE CANNOT FAIL.

The original samples of the specification lived in `C:\\dev\\` on the VM,
"that is to say nowhere lasting". These ones are in git, and this script
is there so that **nobody has to trust their content**: it can be re-read, and
it can be replayed.

    python3 agent/testdata/fabriquer-temoins-ico.py

⚠️ THE ONLY THING THAT MATTERS IS THE `ICONDIR`, not the image. The pixels are an
arbitrary checkerboard — no test depends on them, and none must: the
proof of G2 comes from the RESOURCE, never from the rendered image.
"""
import struct
import sys
from pathlib import Path

def bmp_32bpp(cote: int) -> bytes:
    """A 32 bpp DIB image, opaque checkerboard, as an `.ico` carries it.

    ⚠️ AN `.ico` CARRIES A `BITMAPINFOHEADER` WHOSE HEIGHT IS **DOUBLE**:
    the bottom half is the AND mask. It is the format, and writing it wrong
    would give a file no reader would accept — hence a witness that
    would witness nothing.
    """
    entete = struct.pack(
        "<IiiHHIIiiII",
        40,          # biSize
        cote,        # biWidth
        cote * 2,    # biHeight — DOUBLE, masque AND compris
        1,           # biPlanes
        32,          # biBitCount
        0, 0, 0, 0, 0, 0,
    )
    pixels = bytearray()
    for y in range(cote):           # bottom to top, like a DIB
        for x in range(cote):
            clair = ((x // 8) + (y // 8)) % 2 == 0
            v = 0xE0 if clair else 0x20
            pixels += bytes((v, v, v, 0xFF))   # BGRA, opaque
    masque = bytes(((cote + 31) // 32) * 4 * cote)   # all zero: opaque
    return entete + bytes(pixels) + masque

def ico(cote: int) -> bytes:
    """An `.ico` with a SINGLE entry.

    🔴 `bWidth == 0` MEANS 256: the field is ONE BYTE, and 256 does not fit.
    It is the only trap of the parsing, and it is precisely what the 256
    witness exists to exercise — a reader that returned `0` would make a
    256 icon claim a zero size, and a `max()` would rank it BELOW
    any other entry.
    """
    image = bmp_32bpp(cote)
    octet = 0 if cote == 256 else cote
    entete = struct.pack("<HHH", 0, 1, 1)         # reserved, type=1 (icon), count=1
    entree = struct.pack(
        "<BBBBHHII",
        octet, octet,   # bWidth, bHeight
        0,              # bColorCount — 0 quand >= 8 bpp
        0,              # bReserved
        1,              # wPlanes
        32,             # wBitCount
        len(image),     # dwBytesInRes
        6 + 16,         # dwImageOffset — the header plus ONE 16-byte entry
    )
    return entete + entree + image

def main() -> int:
    ici = Path(__file__).resolve().parent
    for cote, nom in ((48, "g2-temoin-48.ico"), (256, "g2-temoin-256.ico")):
        chemin = ici / nom
        chemin.write_bytes(ico(cote))
        print(f"{chemin.name}: {chemin.stat().st_size} bytes, one entry of {cote} px")
    return 0

if __name__ == "__main__":
    sys.exit(main())
