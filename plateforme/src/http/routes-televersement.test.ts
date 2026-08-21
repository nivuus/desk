// Les quatre routes du téléversement, éprouvées À TRAVERS un serveur HTTP réel.
//
// 🔴 LE CONTRÔLE DE CHEMIN COMPARE LE CORPS, JAMAIS LE SEUL STATUT : G1 a
// MESURÉ qu'un `startsWith('/application')` laissait DIX-SEPT tests VERTS — la
// route mangeait toute la famille et rendait SON PROPRE 404 typé, indiscernable
// du générique tant qu'on ne lisait que le statut. `monterRoute` reproduit le
// 404 de `serveur.ts` mot pour mot : c'est lui qui rend observable un `false`.
//
// ⚠️ LA FAMILLE « DÉPOSER » VIT CHEZ LE FRÈRE,
// `routes-televersement-tranches.test.ts` — extraction, jamais compression, ce
// fichier ayant atteint 504 lignes pour un plafond de 500. Les fixtures des
// deux viennent de `routes-televersement-harnais.ts`, jamais d'une copie.

import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MOTEUR } from '../base/harnais';
import { lireParId } from '../depot/televersement';
import { avec, jetonDe, MS } from './routes-harnais';
import { ENTETES_SECURITE } from './entetes';
import {
    TAILLE_TRANCHE,
    TELEVERSEMENT_MAX_OCTETS,
    TELEVERSEMENTS_EN_COURS_MAX,
} from './routes-televersement';
import {
    CONTENU,
    declarerChez,
    deposer,
    INCONNU,
    magasin,
    monter,
    nettoyer,
    PAS,
    poser,
    sceller,
    SHA,
    TRANCHES,
    utilisateur,
} from './routes-televersement-harnais';

afterEach(async () => {
    await nettoyer();
    vi.restoreAllMocks();
});

describe(`routes de téléversement, moteur=${MOTEUR}`, () => {
    /* ── LE CHEMIN ───────────────────────────────────────────────────── */

    it('🔴 ne mange QUE ses quatre chemins — le corps du 404 est comparé', async () => {
        // 🔴 SI CETTE MUTATION SURVIT, LE TEST EST FAUX, PAS LA MUTATION —
        // c'est la rouge que G1 a vue SURVIVRE. On compare le corps parce qu'un
        // routeur à préfixe peut rendre SON PROPRE 404 typé.
        const { url } = await monter('tel-chemins');
        for (const chemin of [
            '/televersementautre',
            '/televersement/',
            '/televersement/x/tranche',
            '/televersement/x/tranche/1/z',
            '/televersement/x/autre',
            '/televersements',
            '/applications',
        ]) {
            const r = await fetch(`${url}${chemin}`, { headers: avec(jetonDe('u1')) });
            expect(r.status, chemin).toBe(404);
            expect(await r.text(), chemin).toBe('introuvable\n');
        }
    });

    it('sert la requête préalable et pose les en-têtes de sécurité', async () => {
        const { url } = await monter('tel-options');
        const r = await fetch(`${url}/televersement`, { method: 'OPTIONS' });
        expect(r.status).toBe(204);
        for (const [nom, valeur] of Object.entries(ENTETES_SECURITE)) {
            expect(r.headers.get(nom), nom).toBe(valeur);
        }
        // Sur un REFUS aussi — c'est là qu'ils comptent le plus.
        const refus = await fetch(`${url}/televersement`, { method: 'POST' });
        expect(refus.status).toBe(401);
        expect(refus.headers.get('X-Content-Type-Options')).toBe('nosniff');
    });

    it('rend 405 sur la mauvaise méthode, jamais 404', async () => {
        const { url } = await monter('tel-methode');
        const jeton = jetonDe('u1');
        const cas: [string, string][] = [
            ['/televersement', 'GET'],
            ['/televersement/x', 'POST'],
            ['/televersement/x/sceller', 'GET'],
            ['/televersement/x/tranche/0', 'POST'],
        ];
        for (const [chemin, methode] of cas) {
            const r = await fetch(`${url}${chemin}`, { method: methode, headers: avec(jeton) });
            expect(r.status, chemin).toBe(405);
            expect(await r.json(), chemin).toEqual({ refus: 'methode' });
        }
    });

    it('exige un jeton, et refuse celui d’un agent', async () => {
        const { url } = await monter('tel-porteur');
        const sans = await fetch(`${url}/televersement`, { method: 'POST' });
        expect(sans.status).toBe(401);
        expect(await sans.json()).toEqual({ refus: 'jeton-absent' });

        const agent = await fetch(`${url}/televersement`, {
            method: 'POST',
            headers: avec(jetonDe('a1', 'agent')),
        });
        // 403 et non 401 : le jeton est VALIDE, il n'est pas celui d'un humain.
        expect(agent.status).toBe(403);
        expect(await agent.json()).toEqual({ refus: 'jeton-agent' });
    });

    /* ── ① DÉCLARER ──────────────────────────────────────────────────── */

    it('crée, et rend le pas et une liste de tranches VIDE', async () => {
        const { url, base } = await monter('tel-creer');
        const u = await utilisateur(base, 'ada@exemple.test');
        const r = await declarerChez(url, jetonDe(u), {
            nom: 'installeur.exe',
            taille: 42,
            sha256: SHA,
        });
        expect(r.status).toBe(201);
        const corps = (await r.json()) as Record<string, unknown>;
        expect(typeof corps.id).toBe('string');
        expect(corps.taille_tranche).toBe(TAILLE_TRANCHE);
        expect(corps.tranches_presentes).toEqual([]);
        expect(corps.scelle_a).toBe(null);
        // La ligne existe RÉELLEMENT, et elle porte le demandeur.
        const ligne = await lireParId(base, corps.id as string);
        expect(ligne?.utilisateur_id).toBe(u);
        expect(ligne?.taille_tranche).toBe(TAILLE_TRANCHE);
    });

    it('refuse une déclaration mal formée, champ par champ', async () => {
        const { url, base } = await monter('tel-creer-forme');
        const jeton = jetonDe(await utilisateur(base, 'bob@exemple.test'));
        const cas: [unknown, string][] = [
            [{ nom: '', taille: 1, sha256: SHA }, 'nom-invalide'],
            [{ taille: 1, sha256: SHA }, 'nom-invalide'],
            [{ nom: 'a', taille: -1, sha256: SHA }, 'taille-invalide'],
            [{ nom: 'a', taille: 1.5, sha256: SHA }, 'taille-invalide'],
            [{ nom: 'a', taille: '1', sha256: SHA }, 'taille-invalide'],
            [{ nom: 'a', taille: 1, sha256: 'trop-court' }, 'empreinte-invalide'],
            // ⚠️ MAJUSCULES REFUSÉES : deux graphies de la même empreinte se
            // compareraient FAUSSES au scellement.
            [{ nom: 'a', taille: 1, sha256: SHA.toUpperCase() }, 'empreinte-invalide'],
            [{ nom: 'a', taille: TELEVERSEMENT_MAX_OCTETS + 1, sha256: SHA }, 'trop-grand'],
        ];
        for (const [corps, motif] of cas) {
            const r = await declarerChez(url, jeton, corps);
            expect(((await r.json()) as { refus: string }).refus, JSON.stringify(corps)).toBe(motif);
        }
        // Ni un objet JSON, ni du JSON du tout : `forme`, et rien de plus.
        for (const brut of ['[1,2]', 'ceci-n-est-pas-du-json']) {
            const r = await declarerChez(url, jeton, brut);
            expect(r.status, brut).toBe(400);
            expect(await r.json(), brut).toEqual({ refus: 'forme' });
        }
    });

    it('borne le corps de la déclaration, et le quota de téléversements', async () => {
        const { url, base } = await monter('tel-creer-quota');
        const jeton = jetonDe(await utilisateur(base, 'cle@exemple.test'));

        const enorme = await declarerChez(url, jeton, {
            nom: 'x'.repeat(9000),
            taille: 1,
            sha256: SHA,
        });
        expect(enorme.status).toBe(413);
        expect(await enorme.json()).toEqual({ refus: 'corps-trop-grand' });

        const bon = () => declarerChez(url, jeton, { nom: 'a.exe', taille: 1, sha256: SHA });
        for (let i = 0; i < TELEVERSEMENTS_EN_COURS_MAX; i += 1) {
            expect((await bon()).status).toBe(201);
        }
        const trop = await bon();
        expect(trop.status).toBe(429);
        expect(await trop.json()).toEqual({
            refus: 'trop-de-televersements',
            maximum: TELEVERSEMENTS_EN_COURS_MAX,
        });
    });

    /* ── ② LA PROPRIÉTÉ ──────────────────────────────────────────────── */

    it('🔴 le téléversement d’AUTRUI rend le MÊME corps que l’inconnu', async () => {
        // 🔴 TROIS ROUGES SE JOUENT ICI : retirer la vérification de
        // propriétaire (200), rendre 403 (statut), ou rendre un 404 au CORPS
        // distinct — seule la comparaison du corps attrape la troisième.
        const { url, base } = await monter('tel-propriete');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const bob = await utilisateur(base, 'bob@exemple.test');
        const id = await poser(base, ada);

        const chez = await fetch(`${url}/televersement/${id}`, { headers: avec(jetonDe(bob)) });
        const absent = await fetch(`${url}/televersement/${INCONNU}`, {
            headers: avec(jetonDe(bob)),
        });
        expect(chez.status).toBe(404);
        expect(absent.status).toBe(404);
        const corpsChez = await chez.text();
        expect(corpsChez).toBe(await absent.text());
        expect(corpsChez).toBe(JSON.stringify({ refus: 'televersement-inconnu' }));
        // ⚠️ ET LE CORPS NE PORTE AUCUNE TRACE DU CAS RÉEL — c'est tout l'objet.
        expect(corpsChez).not.toContain('etranger');
        expect(corpsChez).not.toContain(ada);

        // Le propriétaire, lui, voit le sien.
        const sien = await fetch(`${url}/televersement/${id}`, { headers: avec(jetonDe(ada)) });
        expect(sien.status).toBe(200);
        expect((await sien.json()) as Record<string, unknown>).toMatchObject({
            id,
            taille: CONTENU.length,
            sha256: SHA,
            taille_tranche: PAS,
            tranches_presentes: [],
        });
    });

    it('journalise LEQUEL des deux cas, et les deux lignes diffèrent', async () => {
        const { url, base } = await monter('tel-journal');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const bob = await utilisateur(base, 'bob@exemple.test');
        const id = await poser(base, ada);
        const vu = vi.spyOn(console, 'warn').mockImplementation(() => {});

        await fetch(`${url}/televersement/${id}`, { headers: avec(jetonDe(bob)) });
        await fetch(`${url}/televersement/00000000-0000-4000-8000-000000000000`, {
            headers: avec(jetonDe(bob)),
        });
        const lignes = vu.mock.calls.map((c) => String(c[0]));
        expect(lignes.some((l) => l.includes('cas=etranger'))).toBe(true);
        expect(lignes.some((l) => l.includes('cas=inconnu'))).toBe(true);
    });

    it('refuse un identifiant qui n’est pas un UUID, avant tout accès au disque', async () => {
        const { url, base } = await monter('tel-id-invalide');
        const jeton = jetonDe(await utilisateur(base, 'ada@exemple.test'));
        for (const id of ['..%2F..%2Fetc', 'pas-un-uuid', SHA]) {
            const r = await fetch(`${url}/televersement/${id}`, { headers: avec(jeton) });
            expect(r.status, id).toBe(400);
            expect(await r.json(), id).toEqual({ refus: 'identifiant-invalide' });
        }
    });

    /* ── ④ SCELLER ───────────────────────────────────────────────────── */

    it('refuse de sceller tant qu’il manque une tranche', async () => {
        const { url, base } = await monter('tel-manquantes');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);
        await deposer(url, id, 0, TRANCHES[0], jeton);

        const r = await sceller(url, id, jeton);
        expect(r.status).toBe(409);
        expect(await r.json()).toEqual({ refus: 'tranches-manquantes', n: [1, 2] });
        expect((await lireParId(base, id))?.scelle_a).toBe(null);
    });

    it('🔴 le bon NOMBRE de tranches ne suffit pas : une mauvaise TAILLE est incohérente', async () => {
        // 🔴 LA ROUGE : remplacer `verdict` par « le compte est bon » fait
        // passer ce cas — les trois rangs sont là, la deuxième est trop courte.
        // Le refus doit être `tranches-incoherentes`, JAMAIS `-manquantes` :
        // redemander une tranche mal taillée ne la réparerait jamais.
        const { url, base } = await monter('tel-incoherentes');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);
        await deposer(url, id, 0, TRANCHES[0], jeton);
        await deposer(url, id, 1, Buffer.from('ab'), jeton); // deux octets pour quatre
        await deposer(url, id, 2, TRANCHES[2], jeton);
        expect(magasin.lister(id).length).toBe(3);

        const r = await sceller(url, id, jeton);
        expect(r.status).toBe(409);
        expect(await r.json()).toEqual({ refus: 'tranches-incoherentes', n: [1] });
        expect((await lireParId(base, id))?.scelle_a).toBe(null);
    });

    it('🔴 RECALCULE l’empreinte : des tranches de la bonne taille au mauvais contenu sont refusées', async () => {
        // 🔴 LA ROUGE : faire confiance au `sha256` ANNONCÉ scelle ce
        // téléversement, dont les octets ne sont pas les siens. Les trois
        // tranches ont la taille EXACTE du plan — seul un recalcul le voit.
        const { url, base } = await monter('tel-empreinte');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);
        await deposer(url, id, 0, Buffer.from('AAAA'), jeton);
        await deposer(url, id, 1, Buffer.from('BBBB'), jeton);
        await deposer(url, id, 2, Buffer.from('CC'), jeton);

        const r = await sceller(url, id, jeton);
        expect(r.status).toBe(409);
        expect(await r.json()).toEqual({ refus: 'empreinte' });
        // ⚠️ NI L'EMPREINTE ANNONCÉE NI LA RELUE NE TRAVERSENT.
        expect((await lireParId(base, id))?.scelle_a).toBe(null);
    });

    it('scelle quand tout concorde, refuse tout dépôt ensuite, et sceller deux fois réussit', async () => {
        const { url, base } = await monter('tel-sceller');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);
        for (let n = 0; n < TRANCHES.length; n += 1) {
            expect((await deposer(url, id, n, TRANCHES[n], jeton)).status).toBe(200);
        }

        const un = await sceller(url, id, jeton);
        expect(un.status).toBe(200);
        const attendu = { id, taille: CONTENU.length, sha256: SHA, scelle_a: MS };
        expect(await un.json()).toEqual(attendu);
        expect((await lireParId(base, id))?.scelle_a).toBe(MS);

        // 🔴 UN DÉPÔT POSTÉRIEUR ANNULERAIT LE SCELLEMENT SANS LE DIRE.
        const apres = await deposer(url, id, 0, Buffer.from('ZZZZ'), jeton);
        expect(apres.status).toBe(409);
        expect(await apres.json()).toEqual({ refus: 'deja-scelle' });

        // Sceller deux fois est un succès — un client qui réessaie doit
        // retrouver LA MÊME réponse, à la lettre.
        const deux = await sceller(url, id, jeton);
        expect(deux.status).toBe(200);
        expect(await deux.text()).toBe(JSON.stringify(attendu));
    });

    it('scelle un fichier VIDE sans exiger la moindre tranche', async () => {
        // Zéro tranche (`ceil(0 / pas)`), verdict `complet` sur une liste vide :
        // sceller un fichier sans contenu n'exige pas une trame sans contenu.
        const { url, base } = await monter('tel-vide');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const vide = createHash('sha256').update('').digest('hex');
        const id = await poser(base, ada, vide, 0);
        const r = await sceller(url, id, jetonDe(ada));
        expect(r.status).toBe(200);
        expect((await lireParId(base, id))?.scelle_a).toBe(MS);
    });

    it('un scellement d’AUTRUI rend le même 404 que l’inconnu', async () => {
        const { url, base } = await monter('tel-sceller-autrui');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const bob = await utilisateur(base, 'bob@exemple.test');
        const id = await poser(base, ada);
        const r = await sceller(url, id, jetonDe(bob));
        expect(r.status).toBe(404);
        expect(await r.text()).toBe(JSON.stringify({ refus: 'televersement-inconnu' }));
        expect((await lireParId(base, id))?.scelle_a).toBe(null);
    });

    it('un dépôt sur le téléversement d’autrui n’écrit RIEN', async () => {
        const { url, base } = await monter('tel-deposer-autrui');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const bob = await utilisateur(base, 'bob@exemple.test');
        const id = await poser(base, ada);
        const r = await deposer(url, id, 0, TRANCHES[0], jetonDe(bob));
        expect(r.status).toBe(404);
        expect(await r.text()).toBe(JSON.stringify({ refus: 'televersement-inconnu' }));
        expect(magasin.lister(id)).toEqual([]);
    });
});
