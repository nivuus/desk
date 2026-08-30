// `GET /applications` et `POST /application/:id/lancer`, éprouvées À TRAVERS un
// serveur HTTP réel, dans le style de `routes-vm.test.ts` et de
// `routes-auth.test.ts`.
//
// 🔴 LE REGISTRE EST RÉEL, LE SOCKET EST UN DOUBLE. C'est ce qui permet
// d'éprouver les trois issues du lancement — succès, agent absent, expiration —
// sans monter d'agent : le double répond, ou se tait. Ce que fait un VRAI socket
// est éprouvé ailleurs, par `agents/canal-apps.test.ts`.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createServer, type Server } from 'node:http';
import { DELAI_LANCEMENT_MS, RegistreAgents, type SocketAgent } from '../agents/registre';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { parseDepuisLaPlateforme } from '../../../proto/ts/plateforme';
import { creerUtilisateur } from '../depot/utilisateur';
// 🔴 LE HARNAIS EST EXTRAIT, ET IL L'A ÉTÉ AVANT L'ADDITION de la famille de
// la route d'icône : ce fichier était à 480 lignes pour un plafond de 500.
import {
    app,
    attribuer,
    avec,
    jetonDe,
    MS,
    ORIGINE,
    poserApp,
    poserVm,
    SECRET,
} from './routes-harnais';
import { signerUrlIcone, verifierUrlIcone } from '../apps/url-icone';
import { servirApplications } from './routes-applications';

let base: Pilote | undefined;
let http: Server | undefined;
let registre = new RegistreAgents();
let maintenant = MS;

afterEach(async () => {
    if (http) await new Promise<void>((r) => http!.close(() => r()));
    http = undefined;
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

/// Monte un serveur qui ne porte QUE cette route, plus le 404 générique de
/// `serveur.ts` reproduit mot pour mot : c'est ainsi qu'un `false` rendu par
/// `servirApplications` devient observable.
async function servir(nom: string, origineClient?: string): Promise<string> {
    base = await baseNeuve(nom);
    registre = new RegistreAgents();
    maintenant = MS;
    const b = base;
    http = createServer((req, rep) => {
        void servirApplications(req, rep, {
            base: b,
            secretJeton: SECRET,
            origineClient,
            registre,
            maintenant: () => maintenant,
        })
            .then((servie) => {
                if (servie) return;
                rep.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
                rep.end('introuvable\n');
            })
            .catch((cause) => {
                rep.writeHead(500, { 'content-type': 'application/json; charset=utf-8' });
                rep.end(JSON.stringify({ refus: 'interne', cause: String(cause) }));
            });
    });
    await new Promise<void>((r) => http!.listen(0, '127.0.0.1', () => r()));
    const a = http!.address();
    return `http://127.0.0.1:${typeof a === 'object' && a ? a.port : 0}`;
}

/// Un socket qui répond à tout ordre par l'issue donnée — ou qui se tait.
function agentQuiRepond(issue: 'raccourci' | 'cible' | 'echec' | null): SocketAgent {
    return {
        readyState: 1,
        send(donnees: string) {
            if (issue === null) return;
            const ordre = parseDepuisLaPlateforme(donnees);
            if (ordre.type !== 'lancer') return;
            // Sur le tour de boucle suivant, comme le ferait un vrai socket.
            setTimeout(() => registre.resoudre(ordre.demande, issue), 0);
        },
        close() {},
    };
}

describe(`routes /applications, moteur=${MOTEUR}`, () => {
    it("rend `false` sur un chemin étranger : le 404 du serveur suit", async () => {
        // 🔴 Rendre `true` ferait manger à cette route les 404 de toutes les
        // autres, et un chemin inconnu répondrait un corps JSON d'application.
        const url = await servir('apps-etranger');
        const r = await fetch(`${url}/rien-du-tout`);
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('introuvable\n');
    });

    it('🔴 le motif de `/application/:id/lancer` est ANCRÉ DES DEUX BOUTS', async () => {
        // 🔴 Un `startsWith` ouvrirait « une famille entière de chemins que
        // personne n'a décidés » (`serveur.ts`).
        //
        // 🔴 C'EST LE CORPS QUI DISCRIMINE, PAS LE CODE. Une première rédaction
        // n'assertait que `404`, et une mutation `startsWith('/application')`
        // LUI A SURVÉCU : la route mangeait alors toute la famille et rendait
        // son PROPRE 404 typé, indiscernable du 404 générique tant qu'on ne
        // lisait que le statut. Le corps `introuvable\n` est celui de
        // `serveur.ts`, et il ne peut être rendu que si la route a bien décliné.
        const url = await servir('apps-ancre');
        const declines = [
            `${url}/application/x/lancer/y`,
            `${url}/application/x`,
            `${url}/applicationsdetournees`,
            `${url}/applications/x`,
        ];
        for (const cible of declines) {
            const r = await fetch(cible, { method: 'POST' });
            expect([cible, r.status, await r.text()]).toEqual([cible, 404, 'introuvable\n']);
        }
        // Et le chemin JUSTE est bien servi — sans ce témoin, les assertions
        // ci-dessus seraient vraies d'une route qui ne sert RIEN.
        expect((await fetch(`${url}/application/x/lancer`, { method: 'POST' })).status).not.toBe(404);
    });

    it('SANS en-tête `Authorization`, rend 401', async () => {
        // 🔴 Les deux routes de P1/P2 sont ouvertes par construction, et rien
        // dans ce dépôt n'authentifiait une requête HTTP avant P4. L'omettre
        // ici rendrait le catalogue de toute VM lisible par n'importe qui.
        const url = await servir('apps-sans-jeton');
        const r = await fetch(`${url}/applications?vm=v-1`);
        expect(r.status).toBe(401);
        expect(await r.json()).toEqual({ refus: 'jeton-absent' });
    });

    it("🔴 avec un jeton d'AGENT, refuse — agent et humain sont signés par le MÊME secret", async () => {
        // 🔴 Accepter tout jeton valide rouvrirait E5 de P3 : sans le claim de
        // type, les deux identités sont INTERCHANGEABLES. Un agent compromis
        // lirait alors le catalogue de son propre utilisateur, et le lancerait.
        const url = await servir('apps-jeton-agent');
        const r = await fetch(`${url}/applications?vm=v-1`, {
            headers: avec(jetonDe('RhH1x2QmTz9kLpVbNc7dAw', 'agent')),
        });
        expect(r.status).toBe(403);
        expect(await r.json()).toEqual({ refus: 'jeton-agent' });
    });

    it('avec un jeton EXPIRÉ, refuse — et le test fait AVANCER l’horloge', async () => {
        // 🔴 Une horloge figée rendrait ce cas inerte : il lirait un état
        // final au lieu de voir la transition. Le jeton est signé à `MS`, et
        // la requête est servie bien après son expiration.
        const url = await servir('apps-jeton-expire');
        const jeton = jetonDe('u-1');
        maintenant = MS + 24 * 60 * 60 * 1000;
        const r = await fetch(`${url}/applications?vm=v-1`, { headers: avec(jeton) });
        expect(r.status).toBe(401);
        expect(await r.json()).toEqual({ refus: 'jeton-expire' });
    });

    it('sert la requête préalable `OPTIONS`, sans laquelle rien n’est atteignable', async () => {
        // ⚠️ Les deux routes exigent `Authorization`, ce qui rend la requête
        // NON SIMPLE : le navigateur émet d'abord un `OPTIONS`, et un 404 lui
        // ferait abandonner sans jamais envoyer la vraie requête. AUCUN test
        // Node ne peut voir la politique d'origine — c'est cette assertion, et
        // rien d'autre, qui tient l'en-tête.
        const url = await servir('apps-options', ORIGINE);
        const r = await fetch(`${url}/applications`, {
            method: 'OPTIONS',
            headers: { origin: ORIGINE },
        });
        expect(r.status).toBe(204);
        expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
    });

    it('rend les applications de la VM demandée, et rien d’autre', async () => {
        const url = await servir('apps-liste');
        await poserVm(base!, 'v-1');
        await poserVm(base!, 'v-2');
        const u = await attribuer(base!, 'v-1', 'a@exemple.test');
        await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        await poserApp(base!, 'v-2', 'Excel', 'c-2');

        const r = await fetch(`${url}/applications?vm=v-1`, { headers: avec(jetonDe(u)) });
        expect(r.status).toBe(200);
        const corps = (await r.json()) as { applications: Array<{ nom: string }> };
        expect(corps.applications.map((a) => a.nom)).toEqual(['Firefox']);
    });

    it("🔴 frappe l'URL SIGNÉE de l'icône, et `null` quand il n'y en a pas", async () => {
        // 🔴 DÉCISION DU PROPRIÉTAIRE DU DÉPÔT, 30 AOÛT 2026 : c'est ICI, sous
        // le jeton porteur et APRÈS le contrôle d'appartenance de la VM,
        // qu'une URL d'icône est frappée — jamais librement. Ce test fige ce
        // chaînage : sans lui, on pourrait déplacer la frappe sur une route
        // ouverte sans que rien ne le dise.
        const url = await servir('apps-icone-url');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'a@exemple.test');
        const empreinte = 'a'.repeat(64);
        await poserApp(base!, 'v-1', 'Avec', 'c-1', empreinte, { pixels: 256 });
        await poserApp(base!, 'v-1', 'Sans', 'c-2');

        const r = await fetch(`${url}/applications?vm=v-1`, { headers: avec(jetonDe(u)) });
        const corps = (await r.json()) as {
            applications: Array<{ id: string; nom: string; icone_url: string | null }>;
        };
        const parNom = new Map(corps.applications.map((a) => [a.nom, a]));
        // Sans icône : `null`, jamais une URL qui rendrait 404.
        expect(parNom.get('Sans')!.icone_url).toBe(null);
        // Avec icône : EXACTEMENT ce que la règle du produit frappe — jamais
        // une URL réécrite ici, qui n'éprouverait qu'elle-même.
        const avecIcone = parNom.get('Avec')!;
        expect(avecIcone.icone_url).toBe(
            signerUrlIcone(avecIcone.id, 'v-1', empreinte, SECRET, maintenant),
        );
        // 🔴 ET ELLE SE VÉRIFIE : la signature frappée est celle que la route
        // d'icône acceptera. Un chaînage qui frapperait avec une AUTRE clé
        // rendrait une URL bien formée et systématiquement refusée.
        const p = new URL(avecIcone.icone_url!, 'http://interne');
        expect(verifierUrlIcone(avecIcone.id, p.searchParams, SECRET, maintenant)).toEqual({
            ok: true,
            vm: 'v-1',
        });
    });

    it("🔴 une VM ÉTRANGÈRE répond EXACTEMENT comme une VM INCONNUE", async () => {
        // 🔴 CE TEST A ÉTÉ RETOURNÉ. Il épinglait `403 {refus:'vm-etrangere'}`,
        // c'est-à-dire la décision D9 du plan de G1 — un ORACLE
        // D'ÉNUMÉRATION : le code de retour confirmait à qui n'y a pas droit
        // qu'une VM existe. Le propriétaire du dépôt a tranché pour le refus
        // INDISTINGUABLE de `routes-vm.ts`, et le test épingle désormais
        // l'indistinguabilité elle-même.
        //
        // 🔴 LES DEUX CORPS SONT COMPARÉS CARACTÈRE POUR CARACTÈRE, pas
        // seulement les deux statuts : un test qui ne lirait que `404` serait
        // satisfait par le MAUVAIS 404 — celui, générique, de `serveur.ts` —
        // exactement le piège que le test d'ancrage du motif, plus haut dans ce
        // fichier, a déjà payé une fois.
        //
        // ⚠️ CE TEST POSE `vm.utilisateur_id` À LA MAIN, puisque rien ne le
        // remplit avant P4 — `npm run admin:agent` laisse la colonne NULL.
        const url = await servir('apps-etrangere');
        await poserVm(base!, 'v-1');
        await attribuer(base!, 'v-1', 'proprietaire@exemple.test');
        const autre = await creerUtilisateur(base!, 'autre@exemple.test', 'x', MS);
        await poserApp(base!, 'v-1', 'Firefox', 'c-1');

        const entetes = avec(jetonDe(autre));
        const etrangere = await fetch(`${url}/applications?vm=v-1`, { headers: entetes });
        const inconnue = await fetch(`${url}/applications?vm=jamais-vue`, { headers: entetes });

        expect([etrangere.status, await etrangere.text()]).toEqual([
            inconnue.status,
            await inconnue.text(),
        ]);
        expect(etrangere.status).toBe(404);
    });

    it("🔴 le corps du refus NE PORTE AUCUNE TRACE du cas réel", async () => {
        // 🔴 C'EST L'OBJET MÊME DE LA DÉCISION : la ligne de journal distingue,
        // la réponse HTTP jamais. Sans cette assertion, un champ de diagnostic
        // ajouté « pour aider » rétablirait l'oracle sans qu'aucun test ne
        // rougisse — les deux corps resteraient de même FORME tout en
        // différant, et le test ci-dessus les comparant l'un à l'autre le
        // verrait, mais celui-ci le dit par son nom.
        const url = await servir('apps-etrangere-muette');
        await poserVm(base!, 'v-1');
        await attribuer(base!, 'v-1', 'proprietaire@exemple.test');
        const autre = await creerUtilisateur(base!, 'autre@exemple.test', 'x', MS);
        await poserApp(base!, 'v-1', 'Firefox', 'c-1');

        const r = await fetch(`${url}/applications?vm=v-1`, { headers: avec(jetonDe(autre)) });
        const corps = await r.text();
        expect(corps).toBe(JSON.stringify({ refus: 'vm-inconnue' }));
        expect(corps).not.toContain('etrangere');
        expect(corps).not.toContain('proprietaire');
    });

    it("🔴 la LIGNE DE JOURNAL, elle, distingue les deux cas", async () => {
        // 🔴 C'EST LA CONTREPARTIE EXPLICITE DU REFUS INDISTINGUABLE. Sans
        // elle, uniformiser coûterait à l'exploitant tout le diagnostic : « la
        // VM n'existe pas » et « elle est à quelqu'un d'autre » se liraient
        // pareil des DEUX côtés, et plus personne ne pourrait distinguer une
        // erreur de saisie d'une tentative d'énumération.
        //
        // CE QUI REND CE CONTRÔLE ROUGE, et l'état est ATTEIGNABLE : une ligne
        // qui disparaîtrait, une ligne qui nommerait le même cas dans les deux
        // situations, ou une ligne qui tairait l'identifiant de la VM.
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const url = await servir('apps-journal-distingue');
        await poserVm(base!, 'v-1');
        await attribuer(base!, 'v-1', 'proprietaire@exemple.test');
        const autre = await creerUtilisateur(base!, 'autre@exemple.test', 'x', MS);

        const entetes = avec(jetonDe(autre));
        await fetch(`${url}/applications?vm=v-1`, { headers: entetes });
        const apresEtrangere = traces.join(' | ');
        await fetch(`${url}/applications?vm=jamais-vue`, { headers: entetes });
        const apresInconnue = traces.join(' | ').slice(apresEtrangere.length);

        expect(apresEtrangere).toContain('cas=etrangere');
        expect(apresEtrangere).toContain('v-1');
        expect(apresInconnue).toContain('cas=inconnue');
        expect(apresInconnue).toContain('jamais-vue');
        // Et les deux lignes ne sont PAS la même : sans ce témoin, une ligne
        // unique disant toujours « refus » satisferait les quatre assertions
        // ci-dessus dès lors qu'elle porterait les deux mots.
        expect(apresInconnue).not.toContain('cas=etrangere');
    });

    it('🔴 une VM NON ATTRIBUÉE est servie, ET la ligne de journal est ÉMISE', async () => {
        // 🔴 SERVIR EN SILENCE RENDRAIT L'ABSENCE D'ISOLATION INVISIBLE. Tant
        // qu'aucune VM n'est attribuée, tout utilisateur authentifié voit
        // toutes les VMs — ce n'est PAS une isolation, et la ligne de journal
        // est ce qui rend l'état visible à l'opérateur. Le test LIT LA TRACE,
        // pas seulement le code de réponse.
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const url = await servir('apps-non-attribuee');
        await poserVm(base!, 'v-1');
        const u = await creerUtilisateur(base!, 'quiconque@exemple.test', 'x', MS);
        await poserApp(base!, 'v-1', 'Firefox', 'c-1');

        const r = await fetch(`${url}/applications?vm=v-1`, { headers: avec(jetonDe(u)) });
        expect(r.status).toBe(200);
        expect(traces.join(' | ')).toContain('vm non attribuee');
        expect(traces.join(' | ')).toContain('v-1');
    });

    it('rend 404 sur une application INCONNUE', async () => {
        const url = await servir('apps-lancer-inconnue');
        const u = await creerUtilisateur(base!, 'u@exemple.test', 'x', MS);
        const r = await fetch(`${url}/application/jamais-vue/lancer`, {
            method: 'POST',
            headers: avec(jetonDe(u)),
        });
        expect(r.status).toBe(404);
        expect(await r.json()).toEqual({ refus: 'application-inconnue' });
    });

    it('🔴 rend 503 quand l’agent est ABSENT, jamais 200', async () => {
        // 🔴 Rendre 200 ferait afficher au hub un succès pour un lancement qui
        // n'a PAS eu lieu — la panne la plus difficile à diagnostiquer qui
        // soit, parce que rien nulle part ne la contredit.
        const url = await servir('apps-lancer-absent');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'u@exemple.test');
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');

        const r = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: avec(jetonDe(u)),
        });
        expect(r.status).toBe(503);
        expect(await r.json()).toEqual({ refus: 'agent-injoignable' });
    });

    it("🔴 rend 504 quand l'agent NE RÉPOND PAS, jamais 202 sans attendre", async () => {
        // 🔴 Rendre 202 sans attendre ferait passer le critère de recette sur
        // un binaire qui n'a RIEN lancé : la plateforme dirait « c'est parti »
        // pour un ordre dont personne n'a jamais vu l'issue.
        const url = await servir('apps-lancer-delai');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'u@exemple.test');
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        // Un agent inscrit, mais MUET.
        registre.inscrire('v-1', agentQuiRepond(null));

        const debut = Date.now();
        const r = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: avec(jetonDe(u)),
        });
        expect(r.status).toBe(504);
        expect(await r.json()).toEqual({ refus: 'delai' });
        // ⚠️ ET IL A RÉELLEMENT ATTENDU : sans cette borne, une route qui
        // rendrait 504 immédiatement passerait le test tout en n'ayant laissé
        // aucune chance à l'agent.
        expect(Date.now() - debut).toBeGreaterThanOrEqual(DELAI_LANCEMENT_MS - 50);
    }, 20_000);

    it("🔴 un lancement réussi rend L'ISSUE, jamais un booléen", async () => {
        // 🔴 Aplatir l'issue en booléen ferait perdre au critère de recette
        // toute discrimination : `raccourci` contre `cible` est ce qui dit si
        // c'est bien le `.lnk` qu'on a lancé, ou une cible reconstruite.
        const url = await servir('apps-lancer-ok');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'u@exemple.test');
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        registre.inscrire('v-1', agentQuiRepond('raccourci'));

        const r = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: avec(jetonDe(u)),
        });
        expect(r.status).toBe(200);
        expect(await r.json()).toEqual({ issue: 'raccourci' });
    });

    it("un lancement en ÉCHEC côté agent rend 200 et l'issue `echec`, jamais une erreur HTTP", async () => {
        // ⚠️ L'ORDRE A ABOUTI : la plateforme a fait son travail, et l'agent a
        // répondu. Rendre une 5xx dirait que le SERVICE a échoué, ce qui est
        // faux — et le distinguer de `agent-injoignable` est tout l'intérêt
        // d'avoir une issue plutôt qu'un booléen.
        const url = await servir('apps-lancer-echec');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'u@exemple.test');
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        registre.inscrire('v-1', agentQuiRepond('echec'));

        const r = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: avec(jetonDe(u)),
        });
        expect(r.status).toBe(200);
        expect(await r.json()).toEqual({ issue: 'echec' });
    });

    it("🔴 lancer une application d'une VM ÉTRANGÈRE répond comme une VM INCONNUE", async () => {
        // Sans cette garde, l'identifiant d'application suffirait à lancer un
        // programme sur la machine de quelqu'un d'autre — et l'agent, lui,
        // n'a aucun moyen de savoir qui a demandé.
        //
        // 🔴 CE TEST A ÉTÉ RETOURNÉ, pour la même raison que son jumeau de la
        // liste : il épinglait `403 {refus:'vm-etrangere'}`, l'oracle
        // d'énumération que le propriétaire du dépôt a tranché contre. La
        // garde, elle, n'a pas bougé d'un pouce — seul le refus qu'elle rend
        // change, et le témoin de son EFFET est que l'agent inscrit ne reçoit
        // rien.
        //
        // 🔴 LE CORPS EST COMPARÉ, jamais le seul statut : `404` seul serait
        // rendu par le 404 générique de `serveur.ts` aussi bien que par
        // celui-ci. Le témoin employé est le refus que l'AUTRE route de ce
        // fichier rend sur une VM vraiment inconnue — ce qui éprouve du même
        // coup « un seul motif, un seul code » ENTRE LES DEUX ROUTES.
        //
        // ⚠️ POURQUOI LE TÉMOIN VIENT DE L'AUTRE ROUTE, ET NON DE CELLE-CI :
        // `application.vm_id` porte `REFERENCES vm(id)` sans `ON DELETE`
        // (migration 0003) — une application dont la VM n'existe pas est
        // INSÉRABLE nulle part, la contrainte rougit. Le verdict `inconnue`
        // est donc INATTEIGNABLE sur `/application/:id/lancer` : la seule VM
        // que cette route puisse lire est celle que l'application désigne, et
        // elle existe par construction. *(Une première rédaction de ce test
        // posait une application orpheline pour servir de témoin ; SQLite l'a
        // refusée par `FOREIGN KEY constraint failed`, et c'est ce rouge qui a
        // corrigé la supposition.)*
        const url = await servir('apps-lancer-etrangere');
        await poserVm(base!, 'v-1');
        await attribuer(base!, 'v-1', 'proprietaire@exemple.test');
        const autre = await creerUtilisateur(base!, 'autre@exemple.test', 'x', MS);
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        registre.inscrire('v-1', agentQuiRepond('raccourci'));

        const entetes = avec(jetonDe(autre));
        const etrangere = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: entetes,
        });
        const temoin = await fetch(`${url}/applications?vm=jamais-vue`, { headers: entetes });

        expect([etrangere.status, await etrangere.text()]).toEqual([
            temoin.status,
            await temoin.text(),
        ]);
        expect(etrangere.status).toBe(404);
    });

    it('refuse la MÉTHODE sur un chemin qui existe, plutôt qu’un 404', async () => {
        // Le chemin EXISTE, c'est la méthode qui ne convient pas : un 404
        // ferait chercher une route absente. Même choix que `routes-vm.ts`.
        const url = await servir('apps-methode');
        expect((await fetch(`${url}/applications`, { method: 'POST' })).status).toBe(405);
        expect((await fetch(`${url}/application/x/lancer`)).status).toBe(405);
    });

    it('exige le paramètre `vm`, et le dit', async () => {
        const url = await servir('apps-sans-vm');
        const u = await creerUtilisateur(base!, 'u@exemple.test', 'x', MS);
        const r = await fetch(`${url}/applications`, { headers: avec(jetonDe(u)) });
        expect(r.status).toBe(400);
        expect(await r.json()).toEqual({ refus: 'vm-absente' });
    });
});
