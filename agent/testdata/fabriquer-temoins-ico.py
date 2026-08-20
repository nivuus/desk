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
    """Une image DIB 32 bpp, damier opaque, telle qu'un `.ico` la porte.

    ⚠️ UN `.ico` PORTE UN `BITMAPINFOHEADER` DONT LA HAUTEUR EST **DOUBLE** :
    la moitié basse est le masque AND. C'est le format, et l'écrire faux
    donnerait un fichier qu'aucun lecteur n'accepterait — donc un témoin qui
    ne témoignerait de rien.
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
    for y in range(cote):           # bas vers haut, comme un DIB
        for x in range(cote):
            clair = ((x // 8) + (y // 8)) % 2 == 0
            v = 0xE0 if clair else 0x20
            pixels += bytes((v, v, v, 0xFF))   # BGRA, opaque
    masque = bytes(((cote + 31) // 32) * 4 * cote)   # tout à zéro : opaque
    return entete + bytes(pixels) + masque

def ico(cote: int) -> bytes:
    """Un `.ico` à UNE SEULE entrée.

    🔴 `bWidth == 0` VAUT 256 : le champ fait UN OCTET, et 256 n'y tient pas.
    C'est le seul piège de l'analyse, et c'est précisément ce que le témoin
    256 existe pour éprouver — un lecteur qui rendrait `0` ferait dire à une
    icône 256 qu'elle est de taille nulle, et un `max()` la classerait SOUS
    n'importe quelle autre entrée.
    """
    image = bmp_32bpp(cote)
    octet = 0 if cote == 256 else cote
    entete = struct.pack("<HHH", 0, 1, 1)         # reserved, type=1 (icône), count=1
    entree = struct.pack(
        "<BBBBHHII",
        octet, octet,   # bWidth, bHeight
        0,              # bColorCount — 0 quand >= 8 bpp
        0,              # bReserved
        1,              # wPlanes
        32,             # wBitCount
        len(image),     # dwBytesInRes
        6 + 16,         # dwImageOffset — l'en-tête plus UNE entrée de 16 octets
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
