#!/usr/bin/env python3
"""Fabrique les deux témoins `.ico` du sous-bloc G2, SUR L'HÔTE, sans Windows.

🔴 POURQUOI CE SCRIPT EST VERSÉ AVEC LES DEUX FICHIERS QU'IL PRODUIT.

Le fait qui gouverne tout le sous-bloc G2 est celui-ci : un `.ico` ne contenant
QU'UNE entrée 48×48, interrogé à 256 par le Shell de Windows, rend **256×256
32bpp** — par `IShellItemImageFactory::GetImage` comme par
`PrivateExtractIconsW`, sans `SIIGBF_SCALEUP` et MÊME avec
`SIIGBF_BIGGERSIZEOK`. Les quatre lignes de rendu des deux témoins sont
identiques ; **seule la ligne `ICONDIR` diffère**. Un critère de réception qui
comparerait la taille rendue à 256 NE PEUT DONC PAS ÉCHOUER.

Les témoins d'origine de la spécification vivaient dans `C:\\dev\\` sur la VM,
« c'est-à-dire nulle part de durable ». Ceux-ci sont dans git, et ce script
est là pour que **personne n'ait à croire à leur contenu** : il se relit, et
il se rejoue.

    python3 agent/testdata/fabriquer-temoins-ico.py

⚠️ LA SEULE CHOSE QUI COMPTE EST L'`ICONDIR`, pas l'image. Les pixels sont un
damier arbitraire — aucun test n'en dépend, et aucun ne doit en dépendre : la
preuve de G2 vient de la RESSOURCE, jamais de l'image rendue.
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
        print(f"{chemin.name} : {chemin.stat().st_size} octets, une entrée de {cote} px")
    return 0

if __name__ == "__main__":
    sys.exit(main())
