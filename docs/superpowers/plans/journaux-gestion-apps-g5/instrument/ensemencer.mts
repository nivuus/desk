// L'ENSEMENCEMENT DE LA BASE DE RECETTE — sous-bloc G5, tâche 14.
//
// 🔴 IL ÉCRIT PAR LES MODULES DU DÉPÔT, JAMAIS PAR UN `INSERT` PARALLÈLE
// (décision D7 du plan). `depot/application.ts::appliquer` et
// `apps/icones.ts::ouvrirMagasin` sont LE CODE QUE LE CHEMIN DE L'AGENT
// EMPRUNTE : un `INSERT` écrit ici n'éprouverait que lui-même, et divergerait
// en silence le jour où le schéma changerait.
//
// 🔴 IL POSE UNE APPLICATION TÉMOIN À ICÔNE 128×128, ET C'EST CE QUI REND LA
// ROUGE DU CRITÈRE ① JOUABLE (décision D8). Le magasin ne connaît qu'une
// taille — `agent/src/apps/icone/extraction.rs:45`, `const COTE: i32 = 256;` —
// et l'icône est adressée par le sha256 de ces octets précis ; redimensionner
// côté plateforme exigerait une dépendance de décodage PNG que
// `plateforme/src/base/migrations/0005-icones.sql:43-47` écarte nommément.
// **La ROUGE consiste donc à bâtir le manifeste de CETTE application-là.**
//
// ⚠️ 128 EST SOUS LES DEUX SEUILS — celui de 192 qu'écrit la conception de ④,
// et celui de **144** que Chromium NOMME dans son relevé (porte P0, mesuré
// deux fois). Un successeur qui poserait 160 se croirait sous le seuil et
// obtiendrait un VERT.

import { createHash } from 'node:crypto';
import { ouvrirBase } from '../../../../../plateforme/src/base/ouvrir';
import { REPERTOIRE_MIGRATIONS, appliquerMigrations } from '../../../../../plateforme/src/base/migrations';
import { lireConfig } from '../../../../../plateforme/src/config';
import { ouvrirMagasin } from '../../../../../plateforme/src/apps/icones';
import { appliquer } from '../../../../../plateforme/src/depot/application';
import { lireParNom } from '../../../../../plateforme/src/depot/vm';
import type { Application } from '../../../../../proto/ts/plateforme';
import { aplatPng } from './png.mjs';

const nomVm = process.argv[2];
if (nomVm === undefined) {
    console.error('usage : ensemencer.ts <nom-de-vm>');
    process.exit(2);
}

const config = lireConfig(process.env);
const base = await ouvrirBase(config);
await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, Date.now());

const vm = await lireParNom(base, nomVm);
if (vm === undefined) {
    console.error(`la VM ${nomVm} n'existe pas : l'enrôler d'abord`);
    process.exit(2);
}

const magasin = ouvrirMagasin(config.repertoireIcones, (c) => console.log(`magasin : ${c}`));

/// Une icône d'un côté donné, et son empreinte — celle par laquelle la route
/// l'adresse.
function icone(cote: number, teinte: [number, number, number]): { empreinte: string; octets: Buffer } {
    const octets = aplatPng(cote, teinte);
    return { empreinte: createHash('sha256').update(octets).digest('hex'), octets };
}

// ⚠️ TROIS TEINTES DISTINCTES, pour qu'une icône mal appariée se voie : deux
// applications qui montreraient la même image seraient indiscernables si
// toutes trois étaient bleues.
const grande = icone(256, [0x7a, 0xa2, 0xf7]);
const grandeBis = icone(256, [0x9e, 0xce, 0x6a]);
const petite = icone(128, [0xf7, 0x76, 0x8e]);

for (const i of [grande, grandeBis, petite]) magasin.ecrire(i.empreinte, i.octets);

function app(nom: string, empreinte: string | null, cote: number | null): Application {
    const cible = `C:\\\\G5\\\\${nom}.exe`;
    return {
        cle: createHash('sha256').update(`${cible}||`).digest('hex'),
        nom,
        chemin: `C:\\\\G5\\\\${nom}.lnk`,
        cible,
        arguments: '',
        repertoire: '',
        icone: empreinte,
        source_max: cote === null ? 'non-mesuree' : { pixels: cote },
    };
}

const aInserer: Application[] = [
    app('Bloc-notes', grande.empreinte, 256),
    app('Calculatrice', grandeBis.empreinte, 256),
    // 🔴 LE TÉMOIN DE LA ROUGE DE ① : son icône fait 128, sous le seuil.
    app('Temoin-128', petite.empreinte, 128),
    // Une application SANS icône : le hub doit la montrer quand même, et le
    // manifeste doit OMETTRE `icons` plutôt que d'en inventer un.
    app('Sans-icone', null, null),
];

await appliquer(base, vm.id, { aInserer, aMettreAJour: [], aMarquerDisparues: [], aRessusciter: [] }, Date.now());

console.log(
    JSON.stringify(
        {
            vm: { id: vm.id, nom: vm.nom },
            applications: aInserer.map((a) => ({ nom: a.nom, icone: a.icone, source_max: a.source_max })),
            empreintes: { grande: grande.empreinte, grandeBis: grandeBis.empreinte, petite: petite.empreinte },
        },
        null,
        2,
    ),
);
