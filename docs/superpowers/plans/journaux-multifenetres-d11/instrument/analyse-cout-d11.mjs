#!/usr/bin/env node
// Sous-bloc D11, recette ⑤ — LECTURE des relevés de `pilote-cout-d11.mjs`.
//
// Deux choses, dans cet ordre :
//
//   1. LA RÈGLE D'ADMISSION du bras, écrite avant la mesure : un bras n'est
//      réputé obtenu que si TOUTES les lignes `duplication de sortie établie`
//      du rang portent la MÊME valeur de `desktop_width`. À défaut, le bras
//      est DÉCLARÉ NON PRIS — pas approché, pas interprété. C'est un contrôle,
//      et il PEUT échouer ; c'est même le seul rouge de cette recette.
//
//   2. LA MESURE APPARIÉE INTRA-EXÉCUTION, qui est ce que la VM rend
//      réellement possible. La pollution du registre s'étant révélée PAR GUID
//      (voir le document de résultats), une exécution porte à la fois des
//      sessions dont la sortie est née 1280×720 et, reproductiblement, UNE
//      dont la sortie est née 3840×2160. Les deux groupes vivent dans la MÊME
//      exécution, au MÊME instant, sous la MÊME charge d'hôte et le MÊME
//      barreau d'échelle : c'est un appariement plus serré que les deux bras
//      séparés du plan, et non pas un repli au rabais.
//
// ⚠️ CE QUE CE LECTEUR N'ÉTABLIT PAS : le groupe « surdimensionné » ne compte
// qu'UNE session par exécution, et c'est TOUJOURS la même (la 4ᵉ ouverte, sur
// le 4ᵉ GUID du pilote). « Sortie surdimensionnée » et « 4ᵉ session ouverte »
// sont donc CONFONDUS, et rien ici ne les départage.
//
// La grandeur est la CADENCE DU CAPTEUR (`cadence du capteur … cadence="…"`),
// relevée entre les deux bornes que le pilote a posées : le prix se paie côté
// AGENT (surface dupliquée, recadrage, copie GPU), pas côté navigateur.
//
// Usage : node analyse-cout-d11.mjs ../cout-sale-1.json ../cout-propre-1.json

import { readFile } from 'node:fs/promises';

const moy = (t) => (t.length ? t.reduce((a, b) => a + b, 0) / t.length : NaN);

for (const chemin of process.argv.slice(2)) {
    const d = JSON.parse(await readFile(chemin, 'utf8'));
    const plat = chemin.replace(/\/([^/]+)\.json$/, '/agent-$1-plat.log');
    const brut = await readFile(plat, 'utf8').catch(() => null);
    console.log(`\n=== ${chemin}  (${d.n_fenetres} fenêtres, budget ${d.budget_bps}, `
        + `mode posé ${d.mode_sortie_pose ?? 'aucun'})`);
    if (!brut) { console.log('  !! journal absent :', plat); continue; }
    console.log(`  loadavg ${d.loadavg_debut?.split(' ').slice(0, 3).join(' ')} -> `
        + `${d.loadavg_fin?.split(' ').slice(0, 3).join(' ')} | établissement `
        + `${JSON.stringify(d.etablissement)}`);
    console.log('  marqueurs :', JSON.stringify(d.marqueurs_journal));

    // ---- 1. La règle d'admission. ----
    const naissances = new Map();
    for (const l of brut.split('\n')) {
        const m = l.match(/session=([^}]+)\}.*duplication de sortie établie desktop_width=(\d+) desktop_height=(\d+)/);
        if (m) naissances.set(m[1], `${m[2]}x${m[3]}`);
    }
    const valeurs = [...new Set(naissances.values())];
    console.log(`  surfaces dupliquées : ${JSON.stringify([...naissances])}`);
    console.log(`  ADMISSION du bras : ${valeurs.length === 1
        ? `OBTENU (une seule valeur, ${valeurs[0]})`
        : `NON OBTENU — ${valeurs.length} valeurs distinctes ${JSON.stringify(valeurs)}. `
          + 'MESURE DEUX BRAS DÉCLARÉE NON PRISE.'}`);

    // ---- 2. La mesure appariée intra-exécution. ----
    const t0 = Date.parse(d.utile_debut), t1 = Date.parse(d.utile_fin);
    const cadences = new Map();
    const tailles = new Map();
    for (const l of brut.split('\n')) {
        const h = l.match(/^(\S+Z)/);
        if (!h) continue;
        const t = Date.parse(h[1]);
        const c = l.match(/cadence du capteur session=(\S+) images=(\d+) endormie=(\S+) cadence="([\d.]+)"/);
        if (c && t >= t0 && t <= t1) {
            if (!cadences.has(c[1])) cadences.set(c[1], { hz: [], endormie: c[3] === 'true' });
            cadences.get(c[1]).hz.push(Number(c[4]));
        }
        const e = l.match(/session=([^}]+)\}: agent::windows_source::encodage: taille d'encodage changée sans toucher à la fenêtre width=(\d+) height=(\d+)/);
        if (e) tailles.set(e[1], `${e[2]}x${e[3]}`);
    }
    const lignes = [];
    for (const [s, v] of cadences) {
        lignes.push({
            session: s, surface: naissances.get(s) ?? '?', encodage: tailles.get(s) ?? '?',
            n: v.hz.length, cadence: Number(moy(v.hz).toFixed(1)), endormie: v.endormie,
        });
    }
    lignes.sort((a, b) => Number(a.session.slice(2)) - Number(b.session.slice(2)));
    console.log(`  fenêtre utile ${d.utile_debut} -> ${d.utile_fin} (${Math.round((t1 - t0) / 1000)} s)`);
    for (const l of lignes) {
        console.log(`    ${l.session.padEnd(5)} surface=${l.surface.padEnd(9)} encodage=${l.encodage.padEnd(9)} `
            + `relevés=${l.n} cadence_moy=${l.cadence} Hz${l.endormie ? ' ENDORMIE' : ''}`);
    }
    const grand = lignes.filter((l) => l.surface !== '?' && Number(l.surface.split('x')[0]) > 1280);
    const petit = lignes.filter((l) => l.surface === '1280x720');
    if (grand.length && petit.length) {
        const mg = moy(grand.map((l) => l.cadence)), mp = moy(petit.map((l) => l.cadence));
        console.log(`  APPARIÉ : surdimensionnée(s) ${grand.map((l) => l.session).join(',')} `
            + `= ${mg.toFixed(1)} Hz (n=${grand.length}) | à la taille demandée = ${mp.toFixed(1)} Hz `
            + `(n=${petit.length}, min ${Math.min(...petit.map((l) => l.cadence))}, `
            + `max ${Math.max(...petit.map((l) => l.cadence))}) | écart ${(mg - mp).toFixed(1)} Hz `
            + `soit ${((mg / mp - 1) * 100).toFixed(1)} %`);
    } else {
        console.log('  APPARIÉ : impossible — un des deux groupes est vide.');
    }
    // Le navigateur, relevé EN PLUS et jamais à la place : la cadence décodée
    // et les images jetées disent ce que le client a reçu, pas ce que la
    // duplication a coûté.
    const parPage = new Map();
    for (const e of d.echantillons ?? []) {
        if (!parPage.has(e.sessionId)) parPage.set(e.sessionId, {});
        parPage.get(e.sessionId)[e.borne] = e;
    }
    const nav = [...parPage.values()]
        .filter((p) => p.debut?.framesDecoded != null && p.fin?.framesDecoded != null)
        .map((p) => ({
            dec: p.fin.framesDecoded - p.debut.framesDecoded,
            jet: (p.fin.framesDropped ?? 0) - (p.debut.framesDropped ?? 0),
            perdus: (p.fin.packetsLost ?? 0) - (p.debut.packetsLost ?? 0),
            taille: `${p.fin.frameWidth}x${p.fin.frameHeight}`,
        }));
    const dt = (t1 - t0) / 1000;
    console.log(`  navigateur (${nav.length} pages) : décodées/s `
        + `${nav.map((x) => (x.dec / dt).toFixed(1)).join(' ')} | jetées `
        + `${nav.map((x) => x.jet).join(' ')} | paquets perdus ${nav.map((x) => x.perdus).join(' ')} | `
        + `tailles ${JSON.stringify([...new Set(nav.map((x) => x.taille))])}`);
}
