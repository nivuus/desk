#!/usr/bin/env node
// Les WAV que Chrome joue à la place d'un microphone
// (`--use-file-for-fake-audio-capture`), pour le bloc E2 du chantier E.
//
// 🔵 POURQUOI DES FICHIERS ET PAS UN OSCILLATEUR. E1 injectait un
// `OscillatorNode` sur le `sender` par `replaceTrack` : cela n'exerçait NI
// `getUserMedia`, NI la permission, NI les trois contraintes de `micro.ts`
// (`echoCancellation`, `noiseSuppression`, `autoGainControl`), NI le pipeline
// audio de Chrome. La Décision 6 du plan E2 exige un vrai `getUserMedia` ; le
// seul moyen d'en obtenir un contenu CONNU sur une machine sans microphone est
// le périphérique factice de Chrome alimenté par un fichier.
//
// ⚠️ CHROME BOUCLE LE FICHIER. La durée n'est donc pas une borne de mesure,
// c'est une période. Un fichier court se répète, et sa fin se recolle à son
// début : d'où le nombre ENTIER de périodes ci-dessous, sans quoi le raccord
// produirait un craquement à chaque tour — un transitoire large bande que le
// juge verrait comme du bruit.
//
// ⚠️ 16 BITS PCM ENTIER, et non flottant : c'est ce que le périphérique
// factice de Chrome sait lire.
//
// Trois fichiers, et chacun a une raison d'être :
//
//   ton-440.wav      le ton continu — le cas nominal des critères ① et ③
//   ton-440-hache.wav  le même, HACHÉ (0,5 s de son / 0,25 s de silence). Il
//                    existe parce que la suppression de bruit de Chrome (NS3)
//                    est conçue pour effacer le bruit STATIONNAIRE, et qu'une
//                    sinusoïde continue en est un du point de vue de
//                    l'algorithme. **Lequel des deux passe est une MESURE, pas
//                    une conjecture** : c'est le step 1 de la tâche 11.
//   silence.wav      le silence NUMÉRIQUE — le critère ④ (DTX). Une piste
//                    vivante qui ne porte rien, ce qui n'est pas la même chose
//                    qu'une piste coupée.
//
//   ton-660.wav      un SECOND ton, pour le critère ⑤ : à deux fenêtres, deux
//                    fréquences distinctes rendent décidable « laquelle des
//                    deux écrit », là où deux fois 440 Hz ne le rendraient pas.
//
// Usage : node faire-wav-e2.mjs <répertoire de sortie>

import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const TAUX = 48000;
const CANAUX = 1;
const BITS = 16;

/// Écrit un WAV PCM 16 bits à partir d'échantillons flottants dans [-1, 1].
function ecrireWav(chemin, echantillons) {
    const octetsData = echantillons.length * 2 * CANAUX;
    const entete = Buffer.alloc(44);
    entete.write('RIFF', 0);
    entete.writeUInt32LE(36 + octetsData, 4);
    entete.write('WAVE', 8);
    entete.write('fmt ', 12);
    entete.writeUInt32LE(16, 16); // taille du bloc fmt
    entete.writeUInt16LE(1, 20); // PCM entier
    entete.writeUInt16LE(CANAUX, 22);
    entete.writeUInt32LE(TAUX, 24);
    entete.writeUInt32LE((TAUX * CANAUX * BITS) / 8, 28);
    entete.writeUInt16LE((CANAUX * BITS) / 8, 32);
    entete.writeUInt16LE(BITS, 34);
    entete.write('data', 36);
    entete.writeUInt32LE(octetsData, 40);

    const data = Buffer.alloc(octetsData);
    for (let i = 0; i < echantillons.length; i++) {
        // 32767 et non 32768 : la borne positive d'un entier 16 bits signé.
        // Écrêter plutôt que laisser déborder — un débordement change le SIGNE
        // et produit un craquement, pas une saturation.
        const v = Math.max(-1, Math.min(1, echantillons[i]));
        data.writeInt16LE(Math.round(v * 32767), i * 2);
    }
    writeFileSync(chemin, Buffer.concat([entete, data]));
    return octetsData + 44;
}

/// Un nombre ENTIER de périodes de `hz`, au plus proche de `secondesVisees`.
/// Voir l'en-tête : c'est ce qui rend le raccord de boucle silencieux.
function longueurEntiere(hz, secondesVisees) {
    const echantillonsParPeriode = TAUX / hz;
    const periodes = Math.round((secondesVisees * TAUX) / echantillonsParPeriode);
    return Math.round(periodes * echantillonsParPeriode);
}

function ton(hz, secondes, amplitude = 0.5) {
    const n = longueurEntiere(hz, secondes);
    const out = new Float64Array(n);
    for (let i = 0; i < n; i++) out[i] = amplitude * Math.sin((2 * Math.PI * hz * i) / TAUX);
    return out;
}

/// Le ton haché. ⚠️ Les fronts sont ADOUCIS sur 5 ms : un front raide est un
/// transitoire large bande, et le juge balaye 80 à 4000 Hz — on lui donnerait
/// de l'énergie partout, ce qui est exactement ce qu'on lui demande de ne pas
/// voir.
function tonHache(hz, secondes, msOn = 500, msOff = 250, amplitude = 0.5) {
    const base = ton(hz, secondes, amplitude);
    const nOn = Math.round((msOn * TAUX) / 1000);
    const nOff = Math.round((msOff * TAUX) / 1000);
    const nRampe = Math.round((5 * TAUX) / 1000);
    for (let i = 0; i < base.length; i++) {
        const phase = i % (nOn + nOff);
        if (phase >= nOn) {
            base[i] = 0;
        } else if (phase < nRampe) {
            base[i] *= phase / nRampe;
        } else if (phase > nOn - nRampe) {
            base[i] *= (nOn - phase) / nRampe;
        }
    }
    return base;
}

const sortie = process.argv[2];
if (!sortie) {
    console.error('usage : node faire-wav-e2.mjs <répertoire de sortie>');
    process.exit(2);
}
mkdirSync(sortie, { recursive: true });

const fichiers = [
    ['ton-440.wav', ton(440, 12)],
    ['ton-440-hache.wav', tonHache(440, 12)],
    ['ton-660.wav', ton(660, 12)],
    // ⚠️ Le silence est un nombre entier d'échantillons quelconque : il n'a
    // aucune période à respecter, et son raccord de boucle est muet par
    // construction.
    ['silence.wav', new Float64Array(TAUX * 12)],
];

for (const [nom, ech] of fichiers) {
    const octets = ecrireWav(join(sortie, nom), ech);
    console.log(`${nom} : ${ech.length} échantillons, ${(ech.length / TAUX).toFixed(4)} s, ${octets} octets`);
}
