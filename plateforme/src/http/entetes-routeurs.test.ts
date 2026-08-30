// Les en-têtes de sécurité, éprouvés SUR CHAQUE ROUTEUR séparément.
//
// 🔴 UN `it()` PAR ROUTEUR, ET JAMAIS UN TEST GLOBAL. Un test unique qui
// balaierait « au moins une route les porte » passerait dès qu'UN SEUL routeur
// les pose — et c'est exactement la leçon que la rouge ①A de P2 a payée :
// `expect` interrompt le test à la première assertion qui tombe, si bien
// qu'une assertion groupée n'éprouve que sa première ligne. La rouge de cette
// tâche consiste à retirer l'étalement d'UN SEUL routeur, et à vérifier que
// SON test tombe pendant que les quatre autres restent verts.
//
// ⚠️ CE FICHIER EXISTE PLUTÔT QUE CINQ BLOCS ÉPARPILLÉS DANS LES CINQ FICHIERS
// DE TEST DE ROUTE, et c'est une divergence assumée avec le plan de P5 (qui
// prévoyait « quelques assertions » dans `routes-vm.test.ts` et
// `routes-session.test.ts`). La propriété éprouvée est TRANSVERSE — « toute
// réponse JSON du service » —, et une propriété transverse dispersée en cinq
// endroits est celle qu'un sixième routeur n'ira jamais rejoindre. G1 vient
// d'ajouter un routeur sans que personne ne s'en aperçoive côté P5 : c'est
// précisément le mode de défaillance que ce fichier rend visible.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { creerUtilisateur } from '../depot/utilisateur';
import { hacher } from '../identite/mot-de-passe';
import { ENTETES_SECURITE } from './entetes';
import { demarrerServeur, type ServicePlateforme } from './serveur';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const MOT_DE_PASSE = 'un-mot-de-passe-ordinaire-42';
const MS = 1_787_136_773_742;

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
    // 🔴 TÂCHE 3 : `servirAuth` se RETIRE désormais en mode `pomerium` — voir
    // son garde. TROIS cas de ce fichier traversent `/auth/connexion`
    // ((1) GET→405, (6) POST→200, (8) OPTIONS→204) et exigent donc
    // `auth: 'motdepasse'`, sinon ils rencontreraient un 404 au lieu de la
    // réponse de `routes-auth`.
    //
    // ⚠️ « LE 404 GÉNÉRIQUE » ÉTAIT LE MOT JUSQU'AU 22 AOÛT 2026, ET IL NE
    // L'EST PLUS : la garde de mode rend désormais ce 404 ELLE-MÊME
    // (`http/introuvable.ts`), parce que le repli SPA du servant de page
    // l'avalait. Le corps et les en-têtes sont les mêmes à l'octet près — ce
    // qui change est QUI répond, et ces trois cas-ci ne s'en aperçoivent pas.
    //
    // ⚠️ CE COMMENTAIRE A ÉCRIT « LES CINQ AUTRES », ET LE COMPTE ÉTAIT FAUX
    // (revue transverse, 21 août 2026) — dans un commentaire dont tout l'objet
    // est la complétude. Ce fichier porte **ONZE** `it()`, non huit : les trois
    // numérotés HORS SÉQUENCE — `(6bis)`, `(6ter)`, `(6quater)`, ajoutés par G2
    // et G3 — n'avaient pas été comptés. **Trois** cas prennent `motdepasse`,
    // donc **HUIT AUTRES** sont indifférents à cette valeur. Recompté par
    // `grep -c '^\s*it(' <ce fichier>` -> `11`, et par
    // `grep -c "^\s*const url = await servir(.*motdepasse" <ce fichier>` ->
    // `3`. 🔴 **Ces
    // deux commandes se relancent ; ce compte ne se recopie pas** — c'est le
    // « naufrage du 487 » de `CLAUDE.md`, et la numérotation hors séquence en
    // est ici la cause mécanique.
    //
    // 🔴 LA SECONDE COMMANDE EST ANCRÉE EN DÉBUT DE LIGNE SUR `const url =
    // await servir(`, ET C'EST LA TROISIÈME RÉDACTION : LES DEUX PREMIÈRES SE
    // SONT POLLUÉES ELLES-MÊMES, ET LA MESURE L'A DIT À CHAQUE FOIS.
    //   ① `grep -c "motdepasse' }"` -> **5** au lieu de 3 : il comptait les
    //      lignes de CE commentaire, qui cite le motif.
    //   ② `grep -c "servir(.*auth: 'motdepasse'"` -> **4** au lieu de 3 : il
    //      comptait **sa propre citation**, deux lignes plus haut.
    // L'ancre `^\s*const` est la seule forme qui ne puisse pas se compter,
    // une ligne de commentaire commençant toujours par `//`. Un fichier qui
    // documente ses invariants CONTIENT les motifs qu'il décrit — piège maison
    // de `CLAUDE.md`, payé DEUX fois de plus ici même.
    //
    // ⚠️ AUCUN `it()` NE VISE `/auth/moi`, et le contrôle qui l'établissait
    // avait EXACTEMENT le même défaut : « `grep -n 'auth/moi'` ne rend rien »
    // était vrai à l'écriture, et faux dès que la phrase le citant est entrée
    // dans le fichier. Le contrôle qui vaut ÉCARTE les commentaires —
    // `grep -n 'auth/moi' <ce fichier> | grep -vc '^\s*[0-9]*:\s*//'` doit
    // rendre **0**.
    //
    // **Décision, tranchée cas par cas et non en bloc** : le `CONFIG`
    // PARTAGÉ reste à `pomerium` (le défaut du produit, `config.ts`), et LES
    // TROIS SEULS cas qui en ont besoin reçoivent `{ ...CONFIG, auth:
    // 'motdepasse' }` localement — jamais l'inverse, qui aurait changé le
    // mode des HUIT autres pour une raison qui ne les concerne pas.
    //
    // ⚠️ CETTE PHRASE DISAIT « ... y compris pour un futur test de /auth/moi
    // qui rejoindrait ce fichier sans le relire », ET C'EST DEVENU FAUX
    // (tâche 6, revue « round de correction 1 », 22 août 2026) : ce fichier
    // reçoit maintenant SANS LE DIRE UN SECOND PIÈGE. `CONFIG.proxyDeConfiance`
    // vaut `new Set()` — personne n'est déclaré de confiance —, et la garde
    // de `routes-identite.ts` refuse alors TOUT pair, y compris la boucle
    // locale d'où ces tests se connectent. Un futur test de `/auth/moi`
    // ajouté ici SANS RELIRE CE COMMENTAIRE recevrait `401
    // pair-non-de-confiance` quel que soit l'en-tête d'identité posé, et
    // devrait poser `proxyDeConfiance: new Set(['127.0.0.1'])` localement
    // pour observer autre chose que cette garde. Code non touché : ce
    // fichier ne construit toujours aucune requête vers `/auth/moi`.
    auth: 'pomerium',
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

async function servir(nom: string, config: Config = CONFIG): Promise<string> {
    base = await baseNeuve(nom);
    await creerUtilisateur(base, 'ada@exemple.test', await hacher(MOT_DE_PASSE), MS);
    service = await demarrerServeur(config, base);
    return `http://127.0.0.1:${service.port}`;
}

/// Vérifie les DEUX en-têtes, nommément, sur une réponse.
function porteLesEntetes(r: Response, quoi: string): void {
    for (const [cle, valeur] of Object.entries(ENTETES_SECURITE)) {
        expect(r.headers.get(cle.toLowerCase()), `${quoi} : en-tête ${cle}`).toBe(valeur);
    }
}

describe('les en-têtes de sécurité, un routeur à la fois', () => {
    it('(1) `routes-auth` les pose — y compris sur une réponse d’ERREUR', async () => {
        // ⚠️ `auth: 'motdepasse'` LOCAL : sans lui, `servirAuth` se RETIRE
        // (tâche 3) et cette requête rencontrerait le 404 générique, jamais
        // le 405 de `routes-auth`.
        const url = await servir('entetes-auth', { ...CONFIG, auth: 'motdepasse' });
        // ⚠️ SUR UNE ERREUR, et c'est délibéré : une réponse d'erreur porte
        // souvent PLUS d'information qu'une réponse normale, et c'est celle
        // qu'un correctif hâtif oublierait.
        const r = await fetch(`${url}/auth/connexion`, { method: 'GET' });
        expect(r.status).toBe(405);
        porteLesEntetes(r, '405 de /auth/connexion');
    });

    it('(2) `routes-vm` les pose', async () => {
        const url = await servir('entetes-vm');
        const r = await fetch(`${url}/vm`);
        // 401 : aucun porteur présenté. La réponse d'erreur porte les en-têtes.
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 de /vm');
    });

    it('(3) `routes-session` les pose', async () => {
        const url = await servir('entetes-session');
        const r = await fetch(`${url}/session`, { method: 'POST' });
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 de /session');
    });

    it('(4) `routes-applications` les pose — le CINQUIÈME routeur, ajouté par G1', async () => {
        // ⚠️ CE ROUTEUR N'EST PAS DANS LE PLAN DE P5, qui compte « les quatre
        // routeurs ». G1 l'a livré entre la rédaction du plan et son
        // exécution. Sans cet `it()`, la propriété « toute réponse JSON du
        // service » serait FAUSSE le jour même de sa livraison.
        const url = await servir('entetes-applications');
        const r = await fetch(`${url}/applications`);
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 de /applications');
    });

    it('(5) `routes-sante` les pose', async () => {
        const url = await servir('entetes-sante');
        const r = await fetch(`${url}/sante`);
        expect(r.status).toBe(200);
        porteLesEntetes(r, '200 de /sante');
    });

    it('(6) 🔴 la réponse 200 de `/auth/connexion` porte `Cache-Control: no-store`', async () => {
        // 🔴 C'EST LA SEULE RÉPONSE DU SERVICE QUI PORTE DES JETONS — l'accès
        // ET le rafraîchissement, en clair dans son corps JSON. Un cache
        // intermédiaire, ou simplement le disque du navigateur, les
        // retiendrait. Un test générique qui n'éprouverait que les réponses
        // d'erreur passerait à côté de celle-ci, qui est la seule qui compte
        // vraiment.
        // ⚠️ `auth: 'motdepasse'` LOCAL — voir le cas (1) : sans lui, cette
        // route n'existe pas et la requête rendrait 404, pas 200.
        const url = await servir('entetes-jetons', { ...CONFIG, auth: 'motdepasse' });
        const r = await fetch(`${url}/auth/connexion`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ email: 'ada@exemple.test', motdepasse: MOT_DE_PASSE }),
        });
        expect(r.status).toBe(200);
        expect(r.headers.get('cache-control')).toBe('no-store');
        porteLesEntetes(r, '200 de /auth/connexion');
    });

    it('(6bis) `routes-icone` les pose — LE SIXIÈME ROUTEUR', async () => {
        // 🔴 G2 AJOUTE LE SIXIÈME ROUTEUR, et l'en-tête de ce fichier nomme le
        // précédent : « G1 vient d'ajouter un routeur sans que personne ne
        // s'en aperçoive côté P5 ». Ne pas rejouer le défaut que ce fichier
        // existe pour empêcher.
        const url = await servir('entetes-icone');
        // Sans jeton : un 401, donc une réponse d'ERREUR — celle qu'un
        // correctif hâtif oublierait.
        const r = await fetch(`${url}/icone/${'a'.repeat(64)}`, { method: 'PUT' });
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 de /icone/:sha256');

        // Et sur l'autre chemin de ce même routeur.
        //
        // ⚠️ CE CHEMIN NE REND PLUS 401 MAIS 400 DEPUIS LE 30 AOÛT 2026, et
        // ce n'est pas une régression : le `GET` n'exige plus `Authorization`
        // — il exige une URL SIGNÉE (décision du propriétaire du dépôt, voir
        // `routes-icone.ts`). Sans les paramètres de signature, la requête est
        // INCOMPLÈTE, pas non authentifiée. **Ce que ce test éprouve est
        // inchangé** : que les en-têtes de sécurité soient posés sur une
        // réponse d'ERREUR de ce routeur.
        const g = await fetch(`${url}/application/x/icone?e=${'a'.repeat(64)}`);
        expect(g.status).toBe(400);
        porteLesEntetes(g, '400 de /application/:id/icone');
    });

    it('(6ter) `routes-televersement` les pose — LE SEPTIÈME ROUTEUR', async () => {
        // 🔴 G3 AJOUTE LE SEPTIÈME, et l'en-tête de ce fichier nomme les deux
        // précédents : G1 a livré le cinquième « sans que personne ne s'en
        // aperçoive côté P5 », G2 le sixième. C'est la troisième fois, et le
        // fichier n'existe que pour que ce soit la dernière.
        const url = await servir('entetes-televersement');
        // Sans jeton : une réponse d'ERREUR, celle qu'un correctif hâtif
        // oublierait — et sur la route d'ÉTAT, qui est la seule des quatre
        // qu'un `GET` sans corps atteigne.
        const r = await fetch(`${url}/televersement/inexistant`);
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 de /televersement/:id');
    });

    it('(6quater) `routes-installation` les pose — LE HUITIÈME ROUTEUR', async () => {
        // ⚠️ CELUI-CI NE SERT QUE L'AGENT, et son refus emprunte donc le
        // porteur d'AGENT et non celui de l'utilisateur. Deux gardes
        // différentes, une seule propriété transverse : c'est précisément le
        // genre d'écart par lequel un routeur échappe à un balayage.
        const url = await servir('entetes-installation');
        const r = await fetch(`${url}/televersement/inexistant/contenu`);
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 de /televersement/:id/contenu');
    });

    it('(7) le 404 générique et le 500 les portent aussi', async () => {
        // Le 404 ne vient d'aucun routeur : il est écrit dans `serveur.ts`.
        // Sans lui, un chemin inconnu serait la seule réponse du service à ne
        // pas porter `nosniff`.
        const url = await servir('entetes-404');
        const r = await fetch(`${url}/chemin-qui-n-existe-pas`);
        expect(r.status).toBe(404);
        porteLesEntetes(r, '404 générique');
    });

    it('(8) la réponse préalable OPTIONS les porte aussi', async () => {
        // ⚠️ `auth: 'motdepasse'` LOCAL — voir le cas (1) : en mode `pomerium`,
        // `servirAuth` se retire AVANT même sa branche OPTIONS (le garde
        // précède tout le reste de la fonction), et ce OPTIONS rencontrerait
        // le 404 générique au lieu du 204 préalable.
        const url = await servir('entetes-options', { ...CONFIG, auth: 'motdepasse' });
        const r = await fetch(`${url}/auth/connexion`, { method: 'OPTIONS' });
        expect(r.status).toBe(204);
        porteLesEntetes(r, '204 préalable');
    });
});
