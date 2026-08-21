// Un encodeur PNG minimal, PUR, sans aucune dépendance neuve.
//
// POURQUOI IL EXISTE : la porte P0 doit poser une icône de manifeste à DEUX
// tailles — 512 (au-dessus du seuil d'installabilité) et 128 (en dessous) —
// et c'est la SECONDE qui décide si la sonde mesure quoi que ce soit (plan
// §3.3, P0-b). Un PNG fabriqué à la volée est le seul moyen de choisir la
// taille sans dépendance de décodage d'image, que le dépôt refuse
// (`plateforme/src/base/migrations/0005-icones.sql:43-47`).
//
// ⚠️ IL N'ENCODE QU'UN APLAT : c'est tout ce dont P0 a besoin. Le contenu de
// l'icône n'entre dans aucun verdict ; seules ses DIMENSIONS comptent, et
// c'est l'en-tête IHDR qui les porte.

import { deflateSync } from 'node:zlib';

const TABLE = (() => {
    const t = new Uint32Array(256);
    for (let n = 0; n < 256; n += 1) {
        let c = n;
        for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
        t[n] = c >>> 0;
    }
    return t;
})();

function crc32(buf) {
    let c = 0xffffffff;
    for (let i = 0; i < buf.length; i += 1) c = TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
}

function morceau(type, donnees) {
    const nom = Buffer.from(type, 'ascii');
    const taille = Buffer.alloc(4);
    taille.writeUInt32BE(donnees.length, 0);
    const somme = Buffer.alloc(4);
    somme.writeUInt32BE(crc32(Buffer.concat([nom, donnees])), 0);
    return Buffer.concat([taille, nom, donnees, somme]);
}

/// Rend un PNG RGBA d'un aplat, de `cote` × `cote` pixels.
export function aplatPng(cote, [r, v, b] = [0x7a, 0xa2, 0xf7]) {
    const ihdr = Buffer.alloc(13);
    ihdr.writeUInt32BE(cote, 0);
    ihdr.writeUInt32BE(cote, 4);
    ihdr[8] = 8; // profondeur
    ihdr[9] = 6; // RGBA
    // 10, 11, 12 : compression, filtre, entrelacement — tous à 0.

    const ligne = Buffer.alloc(1 + cote * 4);
    for (let x = 0; x < cote; x += 1) {
        ligne[1 + x * 4] = r;
        ligne[2 + x * 4] = v;
        ligne[3 + x * 4] = b;
        ligne[4 + x * 4] = 0xff;
    }
    const brut = Buffer.concat(Array.from({ length: cote }, () => ligne));

    return Buffer.concat([
        Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        morceau('IHDR', ihdr),
        morceau('IDAT', deflateSync(brut)),
        morceau('IEND', Buffer.alloc(0)),
    ]);
}
