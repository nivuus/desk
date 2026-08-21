// Fréquence DOMINANTE d'un WAV PCM 16 bits mono/stéréo — le seul juge que ce
// dépôt accepte pour de l'audio.
//
// 🔴 LA CRÊTE MENT, et ce dépôt l'a payé quatre fois dans la seule recette de
// E2 : un résidu de tampon a rendu trois relevés de crête non nuls avec une
// amplitude nulle. **On juge un son à sa fréquence dominante, jamais à une
// crête seule ni à un compte d'octets.**
//
// ⚠️ ET LA RÉSOLUTION EST RENDUE AVEC LE RÉSULTAT. E1 laissait la sienne à
// inférer, et son propre document de résultats le compte comme une lacune.
//
//     node dominante.mjs <fichier.wav> [f_min] [f_max]
import fs from 'node:fs';

const chemin = process.argv[2];
const FMIN = Number(process.argv[3] ?? 80);
const FMAX = Number(process.argv[4] ?? 4000);
const b = fs.readFileSync(chemin);

// En-tête WAV : on cherche `fmt ` et `data` plutôt que de supposer 44 octets.
let pos = 12, taux = 0, canaux = 1, bits = 16, dOff = 0, dLen = 0;
while (pos + 8 <= b.length) {
    const id = b.toString('ascii', pos, pos + 4);
    const len = b.readUInt32LE(pos + 4);
    if (id === 'fmt ') { canaux = b.readUInt16LE(pos + 10); taux = b.readUInt32LE(pos + 12); bits = b.readUInt16LE(pos + 22); }
    if (id === 'data') { dOff = pos + 8; dLen = Math.min(len, b.length - dOff); break; }
    pos += 8 + len + (len % 2);
}
if (!dOff || bits !== 16) { console.log(JSON.stringify({ erreur: `WAV inattendu (bits=${bits})` })); process.exit(2); }

// Mixage mono, et fenêtre de Hann : sans elle, les bords de la fenêtre
// produisent une fuite spectrale large bande qu'on lirait comme du bruit.
const n = Math.floor(dLen / 2 / canaux);
const x = new Float64Array(n);
let crete = 0;
for (let i = 0; i < n; i += 1) {
    let s = 0;
    for (let c = 0; c < canaux; c += 1) s += b.readInt16LE(dOff + (i * canaux + c) * 2) / 32768;
    x[i] = s / canaux;
    if (Math.abs(x[i]) > crete) crete = Math.abs(x[i]);
}
for (let i = 0; i < n; i += 1) x[i] *= 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / (n - 1));

// Goertzel sur un balayage — pas de dépendance, comme `agent/src/spectre.rs`.
const PAS = taux / n;                       // la résolution, RENDUE ci-dessous
const goertzel = (f) => {
    const w = (2 * Math.PI * f) / taux, coef = 2 * Math.cos(w);
    let s1 = 0, s2 = 0;
    for (let i = 0; i < n; i += 1) { const s = x[i] + coef * s1 - s2; s2 = s1; s1 = s; }
    return Math.sqrt(s1 * s1 + s2 * s2 - coef * s1 * s2) / (n / 2);
};
let best = { f: 0, a: -1 };
for (let f = FMIN; f <= FMAX; f += Math.max(PAS, 1)) {
    const a = goertzel(f);
    if (a > best.a) best = { f, a };
}
// Raffinement autour du maximum, au dixième de pas.
for (let f = best.f - PAS; f <= best.f + PAS; f += PAS / 10) {
    if (f < FMIN) continue;
    const a = goertzel(f);
    if (a > best.a) best = { f, a };
}
console.log(JSON.stringify({
    fichier: chemin, taux, canaux, echantillons: n,
    duree_s: Number((n / taux).toFixed(3)),
    resolution_hz: Number(PAS.toFixed(3)),
    dominante_hz: Number(best.f.toFixed(1)),
    amplitude: Number(best.a.toFixed(6)),
    crete: Number(crete.toFixed(6)),
    // 🔴 Le détachement : de combien la dominante dépasse le plancher. C'est
    // lui qui distingue « un ton » de « du bruit dont un bac dépasse ».
    db: best.a > 0 ? Number((20 * Math.log10(best.a)).toFixed(1)) : -1000,
}));
