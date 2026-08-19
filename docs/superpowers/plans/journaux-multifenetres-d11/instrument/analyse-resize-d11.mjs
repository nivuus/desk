#!/usr/bin/env node
// Sous-bloc D11, recette ⑥ — LECTURE de la grille à trois issues de
// `client/src/main.ts` (posée par la tâche 18 de D10, jamais lue jusqu'ici).
//
// ORDRE DE LECTURE, celui du code, et aucune conclusion au-delà :
//   1. AUCUN `declenchement ResizeObserver` pour une session qui n'émet jamais
//      de `Resize` ⟹ le maillon est EN AMONT de la mise en page ;
//   2. log présent et `clientWidth`/`clientHeight` SUIT
//      `innerWidth`/`innerHeight` ⟹ ni l'observateur ni la mise en page ne
//      sont en cause ;
//   3. log présent et il NE SUIT PAS ⟹ la mise en page CSS du `<video>`.
//
// ⚠️ LE CANAL DE CONTRÔLE RESTE UNE HYPOTHÈSE À PART ENTIÈRE : la pièce de D8
// qui prétendait le disculper est réfutée (C3). Ce lecteur ne le disculpe pas
// non plus — il rapporte, côté agent, combien de `contrôle reçu … Resize`
// portent quelle session, et laisse la comparaison au lecteur.
//
// ⚠️ ET IL RAPPORTE LE CONTRÔLE D'ATTEIGNABILITÉ EN PREMIER : une absence de
// log ne devient une information qu'une fois la balise ressortie de CHAQUE
// page d'application.
//
// Usage : node analyse-resize-d11.mjs ../resize-1.json ../resize-2.json …

import { readFile } from 'node:fs/promises';

for (const chemin of process.argv.slice(2)) {
    const d = JSON.parse(await readFile(chemin, 'utf8'));
    const jsonl = chemin.replace(/\/([^/]+)\.json$/, '/console-$1.jsonl');
    const brut = await readFile(jsonl, 'utf8').catch(() => '');
    const entrees = brut.split('\n').filter(Boolean).map((l) => JSON.parse(l));
    console.log(`\n=== ${chemin}  (${d.n_fenetres} fenêtres, viewport imposé : ${d.viewport_impose})`);
    console.log(`  loadavg ${d.loadavg_debut?.split(' ').slice(0, 3).join(' ')} -> `
        + `${d.loadavg_fin?.split(' ').slice(0, 3).join(' ')} | marqueurs ${JSON.stringify(d.marqueurs_journal)}`);
    console.log(`  CONTRÔLE D'ATTEIGNABILITÉ : ${d.balises_ressorties}/${d.balises_posees} `
        + `page(s) d'application dont la console remonte`
        + (d.balises_ressorties === d.balises_posees && d.balises_posees > 0
            ? ' — une absence de log EST donc une information'
            : ' — RECETTE ANNULÉE, PAS INTERPRÉTÉE'));

    const parSession = new Map();
    for (const p of d.pages ?? []) {
        const m = (p.url ?? '').match(/session=([^&]+)/);
        if (m) parSession.set(p.sessionId, m[1]);
    }
    const etat = new Map([...parSession.values()].map((s) => [s, { decl: [], emis: [], differe: 0 }]));
    for (const e of entrees) {
        const s = parSession.get(e.sessionId);
        if (!s) continue;
        const tete = (e.args ?? []).find((a) => typeof a === 'string') ?? '';
        const obj = (e.args ?? []).find((a) => a && typeof a === 'object') ?? {};
        if (tete.includes('declenchement ResizeObserver')) etat.get(s).decl.push(obj);
        else if (tete.includes('[instrumentation resize] emission')) etat.get(s).emis.push(obj);
        else if (tete.includes('Resize différé')) etat.get(s).differe += 1;
    }
    for (const [s, v] of [...etat].sort((a, b) => Number(a[0].slice(2)) - Number(b[0].slice(2)))) {
        const o = v.decl[0] ?? {};
        const suit = v.decl.length
            ? v.decl.every((x) => x.clientWidth === x.innerWidth && x.clientHeight === x.innerHeight)
            : null;
        const issue = v.decl.length === 0
            ? '① maillon EN AMONT de la mise en page'
            : (suit ? '② ni l’observateur ni la mise en page' : '③ mise en page CSS du <video>');
        console.log(`    ${s.padEnd(5)} declenchement=${v.decl.length} emission=${v.emis.length} `
            + `« Resize différé »=${v.differe} | client ${o.clientWidth}x${o.clientHeight} `
            + `inner ${o.innerWidth}x${o.innerHeight} => issue ${issue}`);
    }
    console.log('  côté AGENT, `contrôle reçu` par session :');
    for (const l of (d.controles_recus ?? '').split('\n')) if (l.trim()) console.log('    ' + l.trim());
}
