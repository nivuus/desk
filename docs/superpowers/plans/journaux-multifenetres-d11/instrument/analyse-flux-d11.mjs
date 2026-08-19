#!/usr/bin/env node
// Sous-bloc D11, recette ④ — LECTURE des relevés de `pilote-flux-d11.mjs`.
//
// Le critère du plan est la DISTINCTION deux à deux des marqueurs. Ce lecteur
// la vérifie, et va **une étape plus loin** : il apparie chaque marqueur mesuré
// à la mire attendue par PLUS PROCHE VOISIN dans l'espace RGB, et rapporte la
// distance au premier ET au second candidat.
//
// ⚠️ POURQUOI L'APPARIEMENT EXACT EST IMPOSSIBLE, et pourquoi ce n'est pas un
// défaut : le marqueur traverse un encodeur H.264 en 4:2:0. Le
// sous-échantillonnage chromatique et la quantification déplacent la couleur
// de quelques dizaines d'unités RGB — `ffc201` mesuré pour `ffc300` attendu.
// La quantification par pas de 16 du prédicat absorbe le bruit pour la
// DISTINCTION, jamais pour l'IDENTIFICATION. C'est la distance au second
// candidat qui dit si l'appariement est ambigu ou non ; elle est rapportée
// sans seuil, pour que le lecteur juge lui-même.
//
// ⚠️ CE QUE CET APPARIEMENT N'ÉTABLIT TOUJOURS PAS : que chaque page décode le
// flux de LA FENÊTRE WINDOWS qu'elle prétend montrer. Il établit que les dix
// pages portent les dix mires assignées, une chacune — pas la correspondance
// page ↔ HWND, qui n'est pas au programme (tâche 12, step 3).
//
// Usage : node analyse-flux-d11.mjs ../flux-1.json ../flux-2.json ../flux-rouge.json

import { readFile } from 'node:fs/promises';

function hslVersRgb(h, s, l) {
    h /= 360;
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    const f = (t) => {
        t = (t + 1) % 1;
        if (t < 1 / 6) return p + (q - p) * 6 * t;
        if (t < 1 / 2) return q;
        if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
        return p;
    };
    return [f(h + 1 / 3), f(h), f(h - 1 / 3)].map((x) => Math.round(x * 255));
}

for (const chemin of process.argv.slice(2)) {
    const d = JSON.parse(await readFile(chemin, 'utf8'));
    console.log(`\n=== ${chemin}  (étiquette ${d.etiquette}, mires ${d.mires.join(',')})`);
    console.log(`loadavg ${d.loadavg_debut} -> ${d.loadavg_fin} | virsh ${d.virsh_debut} -> ${d.virsh_fin}`);
    console.log('marqueurs de journal :', JSON.stringify(d.marqueurs_journal));
    // Les mires RÉELLEMENT demandées, dédupliquées : le rouge en demande deux
    // identiques, et il n'y a alors qu'UNE référence pour deux pages.
    const refs = [...new Set(d.mires)].map((n) => ({ n, rgb: hslVersRgb((Number(n) * 47) % 360, 1, 0.5) }));
    for (const t of d.tours) {
        const ids = [];
        for (const p of t.releve) {
            if (!p.rgb) { ids.push(`${p.sessionId}:SANS-MARQUEUR`); continue; }
            const m = [0, 2, 4].map((i) => parseInt(p.rgb.slice(i, i + 2), 16));
            const ds = refs
                .map((r) => ({ n: r.n, d: Math.round(Math.hypot(...r.rgb.map((v, i) => v - m[i]))) }))
                .sort((a, b) => a.d - b.d);
            ids.push(`${p.sessionId}:n=${ds[0].n}(d=${ds[0].d}${ds[1] ? `,2e=${ds[1].n}@${ds[1].d}` : ''})`);
        }
        const distincts = new Set(t.releve.map((p) => p.marqueur)).size;
        console.log(`  tour ${t.tour} : ${t.pages_mesurees} pages, étalement ${t.etalement_ms} ms, `
            + `${distincts} marqueur(s) distinct(s), collisions=${t.collisions.length}`);
        console.log(`    ${ids.join('  ')}`);
        for (const [marqueur, sessions] of t.collisions) {
            console.log(`    COLLISION marqueur=${marqueur} sessions=${sessions.join(',')}`);
        }
        console.log(`    tailles reçues : ${JSON.stringify(t.releve.map((p) => p.source))}`);
    }
}
