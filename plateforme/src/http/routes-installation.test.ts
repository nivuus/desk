// Les trois routes de l'INSTALLATION, éprouvées À TRAVERS un serveur HTTP réel
// et un magasin de tranches RÉEL — dans le style de `routes-applications.test.ts`
// et de `routes-icone.test.ts`.
//
// 🔴 LE MAGASIN N'EST PAS UN DOUBLE, ET C'EST LE POINT. Les octets sont
// réellement découpés en tranches sur un disque temporaire, et le test compare
// le corps de la réponse au fichier d'origine, OCTET POUR OCTET : c'est ce qui
// éprouve la concaténation en flux plutôt que la promesse qu'elle a lieu.
//
// 🔴 CE FICHIER PORTE LES QUATRE MUTATIONS QUE LE PLAN PRESCRIT, et la première
// est la plus importante du sous-bloc : **un jeton d'agent de la VM `B` ne doit
// pas obtenir l'installeur destiné à `A`**. Elle a son `it()` nommé, et il porte
// son TÉMOIN — l'agent de `A`, lui, obtient les octets —, sans quoi l'assertion
// serait vraie d'une route qui ne sert jamais rien.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createHash, randomBytes } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { ouvrirMagasinTranches, type MagasinTranches } from '../apps/magasin-tranches';
import { RegistreAgents } from '../agents/registre';
import { enroler, marquerVu } from '../depot/agent';
import { creer as creerInstallation, terminer, avancer } from '../depot/installation';
import { creer as creerTeleversement, sceller } from '../depot/televersement';
import { creerUtilisateur } from '../depot/utilisateur';
import { plan } from '../../../proto/ts/tranches';
import { avec, demonter, jetonDe, monterRoute, MS, ORIGINE, poserVm, attribuer, type Montage } from './routes-harnais';
import { servirInstallation } from './routes-installation';

let m: Montage | undefined;
let racine: string | undefined;
/// ⚠️ IL S'APPELLE `tranches` DANS LES DÉPENDANCES, PAS `magasin`, ET LE NOM EST
/// PORTEUR : `serveur.ts` porte DÉJÀ un `magasin` — celui des ICÔNES de G2 —
/// dans le même objet. Le harnais suit ce nom pour que le câblage éprouvé ici
/// soit celui du service.
let tranches: MagasinTranches | undefined;
let maintenant = MS;

afterEach(async () => {
    await demonter(m);
    m = undefined;
    if (racine !== undefined) rmSync(racine, { recursive: true, force: true });
    racine = undefined;
    tranches = undefined;
    vi.restoreAllMocks();
});

/// Monte le serveur qui ne porte QUE cette route, plus le 404 générique de
/// `serveur.ts` reproduit mot pour mot par `monterRoute` : c'est lui qui rend
/// observable un `false` rendu par `servirInstallation`.
async function servir(nom: string, origineClient?: string): Promise<string> {
    maintenant = MS;
    racine = mkdtempSync(join(tmpdir(), `g3-${nom}-`));
    tranches = ouvrirMagasinTranches(racine, () => {});
    m = await monterRoute(nom, (req, rep, base) =>
        servirInstallation(req, rep, {
            base,
            secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
            origineClient,
            tranches: tranches!,
            // ⚠️ UN REGISTRE VIDE, ET C'EST LE CAS QUI COMPTE ICI : aucun socket
            // n'est inscrit, donc `pousser` rend `false` et l'ordre reste
            // `en_attente` en base — exactement le filet que
            // `reemettreLesInstallations` livrera au prochain enrôlement. Ces
            // tests éprouvent la ROUTE, jamais la livraison, qui demande un
            // socket vivant (recette, §5quater).
            registre: new RegistreAgents(),
            maintenant: () => maintenant,
        }));
    return m.url;
}

/// Un téléversement RÉEL : la ligne en base, et ses tranches sur le disque.
///
/// ⚠️ LE DÉCOUPAGE PASSE PAR `proto/ts/tranches.ts::plan`, jamais par une
/// arithmétique locale : c'est la règle que la route emploie pour reconstruire
/// le flux, et deux arithmétiques indépendantes divergeraient un jour.
async function poserTeleversement(
    base: Pilote,
    utilisateurId: string,
    nom: string,
    octets: Buffer,
    { scelle = true, tailleTranche = 7 } = {},
): Promise<{ id: string; octets: Buffer }> {
    const ligne = await creerTeleversement(
        base,
        {
            utilisateurId,
            nom,
            taille: octets.length,
            sha256: createHash('sha256').update(octets).digest('hex'),
            tailleTranche,
        },
        MS,
    );
    for (const t of plan(octets.length, tailleTranche)) {
        const debut = t.n * tailleTranche;
        await tranches!.ecrire(ligne.id, t.n, Readable.from([octets.subarray(debut, debut + t.octets)]), 1 << 20);
    }
    if (scelle) await sceller(base, ligne.id, MS);
    return { id: ligne.id, octets };
}

/// Une VM enrôlée, vivante, attribuée à un utilisateur — et son préfixe, qui est
/// le SUJET du jeton d'agent.
async function poserVmVivante(base: Pilote, vmId: string, email: string, prefixe: string): Promise<string> {
    await poserVm(base, vmId);
    const u = await attribuer(base, vmId, email);
    await enroler(base, vmId, 'empreinte-opaque', prefixe);
    await marquerVu(base, vmId, MS);
    return u;
}

/// ⚠️ LE JETON PORTE L'IDENTIFIANT RÉEL de l'utilisateur, jamais un nom commode :
/// c'est lui que la route compare à `vm.utilisateur_id`, et un sujet inventé
/// ferait rendre `404 vm-inconnue` à des cas qui n'ont rien à voir — c'est ce
/// qu'une première rédaction de ce fichier a mesuré.
const CORPS = (u: string, vm: string, televersement: string) => ({
    method: 'POST',
    headers: avec(jetonDe(u)),
    body: JSON.stringify({ vm, televersement }),
});

describe(`routes /installation, moteur=${MOTEUR}`, () => {
    it("rend `false` sur un chemin étranger : le 404 du serveur suit", async () => {
        const url = await servir('inst-etranger');
        const r = await fetch(`${url}/rien-du-tout`);
        expect([r.status, await r.text()]).toEqual([404, 'introuvable\n']);
    });

    it('🔴 les DEUX motifs sont ANCRÉS DES DEUX BOUTS, et le CORPS discrimine', async () => {
        // 🔴 UN `startsWith` OUVRIRAIT UNE FAMILLE ENTIÈRE DE CHEMINS QUE
        // PERSONNE N'A DÉCIDÉS. Et c'est le CORPS qui juge, jamais le code :
        // G1 a mesuré qu'un `startsWith('/application')` laissait DIX-SEPT
        // tests verts, la route rendant SON PROPRE 404 typé, indiscernable du
        // 404 générique tant qu'on ne lit que le statut.
        const url = await servir('inst-ancre');
        const declines = [
            `${url}/installation/x/y`,
            `${url}/installations`,
            `${url}/installationsdetournees`,
            `${url}/televersement/x/contenu/y`,
            `${url}/televersement/x`,
            `${url}/televersements/x/contenu`,
        ];
        for (const cible of declines) {
            const r = await fetch(cible);
            expect([cible, r.status, await r.text()]).toEqual([cible, 404, 'introuvable\n']);
        }
        // Les trois chemins JUSTES sont bien servis — sans ce témoin, les
        // assertions ci-dessus seraient vraies d'une route qui ne sert RIEN.
        expect((await fetch(`${url}/installation/x`)).status).not.toBe(404);
        expect((await fetch(`${url}/televersement/x/contenu`)).status).not.toBe(404);
        expect((await fetch(`${url}/installation`, { method: 'POST' })).status).not.toBe(404);
    });

    it('sert la requête préalable `OPTIONS`, sans laquelle rien n’est atteignable', async () => {
        // ⚠️ Les trois routes exigent `Authorization`, ce qui rend la requête NON
        // SIMPLE : un 404 sur l'`OPTIONS` ferait abandonner le navigateur avant
        // la vraie requête. AUCUN test Node ne voit la politique d'origine —
        // c'est cette assertion, et rien d'autre, qui tient l'en-tête.
        const url = await servir('inst-options', ORIGINE);
        for (const c of ['/installation', '/installation/x', '/televersement/x/contenu']) {
            const r = await fetch(`${url}${c}`, { method: 'OPTIONS', headers: { origin: ORIGINE } });
            expect([c, r.status, r.headers.get('access-control-allow-origin')]).toEqual([c, 204, ORIGINE]);
        }
    });

    it('refuse la MÉTHODE sur un chemin qui existe, plutôt qu’un 404', async () => {
        const url = await servir('inst-methode');
        expect((await fetch(`${url}/installation`)).status).toBe(405);
        expect((await fetch(`${url}/installation/x`, { method: 'POST' })).status).toBe(405);
        expect((await fetch(`${url}/televersement/x/contenu`, { method: 'POST' })).status).toBe(405);
    });

    it('SANS en-tête `Authorization`, les trois routes rendent 401', async () => {
        const url = await servir('inst-sans-jeton');
        for (const [c, o] of [['/installation', { method: 'POST' }], ['/installation/x', {}],
            ['/televersement/x/contenu', {}]] as const) {
            const r = await fetch(`${url}${c}`, o);
            expect([c, r.status, await r.json()]).toEqual([c, 401, { refus: 'jeton-absent' }]);
        }
    });

    it("🔴 `POST /installation` refuse un jeton d'AGENT — MUTATION n°2, premier sens", async () => {
        // 🔴 Agent et humain sont signés par le MÊME secret : accepter tout jeton
        // valide rouvrirait E5 de P3, et un agent compromis ordonnerait
        // l'installation d'un logiciel sur la machine de son propriétaire.
        const url = await servir('inst-ordre-jeton-agent');
        const r = await fetch(`${url}/installation`, {
            method: 'POST',
            headers: avec(jetonDe('prefixe-quelconque', 'agent')),
            body: JSON.stringify({ vm: 'v-1', televersement: 't-1' }),
        });
        expect([r.status, await r.json()]).toEqual([403, { refus: 'jeton-agent' }]);
    });

    it("🔴 `GET …/contenu` refuse un jeton PORTEUR — MUTATION n°2, second sens", async () => {
        // 🔴 La moitié symétrique de la garde : un humain n'emprunte jamais le
        // chemin de l'agent. Si l'une des deux se relâche, les deux identités
        // redeviennent interchangeables.
        const url = await servir('inst-contenu-jeton-humain');
        const r = await fetch(`${url}/televersement/t-1/contenu`, { headers: avec(jetonDe('u-1')) });
        expect([r.status, await r.json()]).toEqual([403, { refus: 'jeton-utilisateur' }]);
    });

    it('refuse un corps mal formé, et le dit', async () => {
        const url = await servir('inst-forme');
        const entetes = avec(jetonDe('u-1'));
        for (const corps of ['pas du json', '{}', '{"vm":"v-1"}', '{"vm":"","televersement":"t"}', '[]']) {
            const r = await fetch(`${url}/installation`, { method: 'POST', headers: entetes, body: corps });
            expect([corps, r.status, await r.json()]).toEqual([corps, 400, { refus: 'forme' }]);
        }
    });

    it("🔴 une VM ÉTRANGÈRE répond EXACTEMENT comme une VM INCONNUE", async () => {
        // 🔴 Les deux corps sont comparés CARACTÈRE POUR CARACTÈRE : un test qui
        // ne lirait que `404` serait satisfait par le 404 GÉNÉRIQUE de
        // `serveur.ts`, qui n'est pas celui-ci.
        const url = await servir('inst-vm-etrangere');
        await poserVmVivante(m!.base, 'v-1', 'proprietaire@exemple.test', 'PREFIXE-A');
        const autre = await creerUtilisateur(m!.base, 'autre@exemple.test', 'x', MS);
        const tel = await poserTeleversement(m!.base, autre, 'setup.exe', Buffer.from('abc'));
        const opts = (vm: string) => ({
            method: 'POST',
            headers: avec(jetonDe(autre)),
            body: JSON.stringify({ vm, televersement: tel.id }),
        });
        const etrangere = await fetch(`${url}/installation`, opts('v-1'));
        const inconnue = await fetch(`${url}/installation`, opts('jamais-vue'));
        expect([etrangere.status, await etrangere.text()])
            .toEqual([inconnue.status, await inconnue.text()]);
        expect(etrangere.status).toBe(404);
    });

    it("🔴 un TÉLÉVERSEMENT étranger répond EXACTEMENT comme un inconnu, et le corps ne dit rien du cas réel", async () => {
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const url = await servir('inst-tel-etranger');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const autre = await creerUtilisateur(m!.base, 'autre@exemple.test', 'x', MS);
        const aLautre = await poserTeleversement(m!.base, autre, 'setup.exe', Buffer.from('abc'));
        const opts = (tid: string) => ({
            method: 'POST',
            headers: avec(jetonDe(u)),
            body: JSON.stringify({ vm: 'v-1', televersement: tid }),
        });
        const etranger = await fetch(`${url}/installation`, opts(aLautre.id));
        const corpsEtranger = await etranger.text();
        const inconnu = await fetch(`${url}/installation`, opts('jamais-vu'));
        expect([etranger.status, corpsEtranger]).toEqual([inconnu.status, await inconnu.text()]);
        expect(corpsEtranger).toBe(JSON.stringify({ refus: 'televersement-inconnu' }));
        // 🔴 LA CONTREPARTIE : la ligne de journal, elle, DISTINGUE — sans quoi
        // uniformiser coûterait à l'exploitant tout son diagnostic.
        const tout = traces.join(' | ');
        expect(tout).toContain('cas=etrangere');
        expect(tout).toContain('cas=inconnue');
        expect(corpsEtranger).not.toContain('etrangere');
    });

    it('🔴 un téléversement NON SCELLÉ est refusé EN AMONT, et AUCUNE ligne n’est écrite — MUTATION n°3', async () => {
        // 🔴 UN REFUS AU BON ENDROIT VAUT MIEUX QU'UN REFUS AU BON MOMENT :
        // sans lui, l'ordre partirait, l'agent tirerait un fichier PARTIEL et
        // n'apprendrait son empreinte fausse qu'après l'avoir écrit en entier.
        // ⚠️ Le test n'assère pas seulement le code : il vérifie qu'AUCUNE
        // installation n'a été créée, ce qu'un `409` rendu APRÈS l'écriture
        // laisserait passer.
        const url = await servir('inst-non-scelle');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abcdefghij'), { scelle: false });
        const r = await fetch(`${url}/installation`, {
            method: 'POST',
            headers: avec(jetonDe(u)),
            body: JSON.stringify({ vm: 'v-1', televersement: tel.id }),
        });
        expect([r.status, await r.json()]).toEqual([409, { refus: 'non-scelle' }]);
        const lignes = await m!.base.interroger('SELECT id FROM installation', []);
        expect(lignes).toEqual([]);
    });

    it("refuse une extension que l'agent n'exécutera pas, plutôt qu'un aller-retour de 800 Mo", async () => {
        const url = await servir('inst-extension');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        for (const nom of ['setup.bat', 'setup', 'setup.exe.txt', '.exe']) {
            const tel = await poserTeleversement(m!.base, u, nom, Buffer.from('abc'));
            const r = await fetch(`${url}/installation`, {
                method: 'POST',
                headers: avec(jetonDe(u)),
                body: JSON.stringify({ vm: 'v-1', televersement: tel.id }),
            });
            expect([nom, r.status, await r.json()]).toEqual([nom, 400, { refus: 'extension' }]);
        }
        // ⚠️ ET LE TÉMOIN, sans lequel ce test serait vrai d'une route qui refuse
        // TOUT : `.MSI` en majuscules passe, parce que Windows est insensible à
        // la casse.
        const bon = await poserTeleversement(m!.base, u, 'SETUP.MSI', Buffer.from('abc'));
        const r = await fetch(`${url}/installation`, {
            method: 'POST',
            headers: avec(jetonDe(u)),
            body: JSON.stringify({ vm: 'v-1', televersement: bon.id }),
        });
        expect(r.status).toBe(201);
    });

    it('🔴 une VM INJOIGNABLE rend 503, jamais 201 sur une ligne que personne ne recevra', async () => {
        // 🔴 Écrire la ligne en silence ferait afficher au hub une installation
        // « demandée » que personne ne recevra — rien nulle part ne la
        // contredirait. ⚠️ La transition est ASSIÉGÉE : le même montage rend 201
        // à `vu_a + SEUIL` et 503 une milliseconde plus tard.
        const url = await servir('inst-injoignable');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abc'));
        maintenant = MS + 90_000 + 1;
        const mort = await fetch(`${url}/installation`, CORPS(u, 'v-1', tel.id));
        expect([mort.status, await mort.json()]).toEqual([503, { refus: 'agent-injoignable' }]);
        maintenant = MS + 90_000;
        const vif = await fetch(`${url}/installation`, CORPS(u, 'v-1', tel.id));
        expect(vif.status).toBe(201);
    });

    it("un ordre accepté rend 201 et son identifiant, et la ligne naît `en_attente`", async () => {
        // ⚠️ `en_attente` N'EST PAS DÉCORATIF : c'est cet état, et lui seul, que
        // la réémission à l'enrôlement repousse à l'agent.
        const url = await servir('inst-ordre-ok');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abc'));
        const r = await fetch(`${url}/installation`, CORPS(u, 'v-1', tel.id));
        expect(r.status).toBe(201);
        const { id } = (await r.json()) as { id: string };
        const lignes = await m!.base.interroger<{ id: string; etat: string; vm_id: string }>(
            'SELECT id, etat, vm_id FROM installation WHERE id = ?', [id],
        );
        expect(lignes).toEqual([{ id, etat: 'en_attente', vm_id: 'v-1' }]);
    });

    it("`GET /installation/:id` rend l'état, l'issue, le motif et la queue de journal", async () => {
        const url = await servir('inst-etat');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abc'));
        const inst = await creerInstallation(m!.base, { vmId: 'v-1', televersementId: tel.id }, MS);
        await terminer(m!.base, inst.id, {
            issue: 'refusee', motif: 'elevation-requise', codeSortie: 740,
            journal: 'la queue', journalTronque: true,
        }, MS + 5);
        const r = await fetch(`${url}/installation/${inst.id}`, { headers: avec(jetonDe(u)) });
        expect(r.status).toBe(200);
        const vue = (await r.json()) as Record<string, unknown>;
        expect(vue).toMatchObject({
            id: inst.id, vm: 'v-1', televersement: tel.id, etat: 'terminee',
            issue: 'refusee', motif: 'elevation-requise', code_sortie: 740, journal: 'la queue',
        });
        // ⚠️ UN BOOLÉEN SUR LE FIL, jamais le `0`/`1` de SQLite : le hub n'a pas
        // à connaître une convention de stockage.
        expect(vue.journal_tronque).toBe(true);
    });

    it("🔴 une installation ÉTRANGÈRE répond EXACTEMENT comme une INCONNUE", async () => {
        // 🔴 Et le motif est `installation-inconnue`, JAMAIS `vm-inconnue` :
        // rendre le motif de la VM DIRAIT que l'installation, elle, existe.
        const url = await servir('inst-etat-etranger');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const autre = await creerUtilisateur(m!.base, 'autre@exemple.test', 'x', MS);
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abc'));
        const inst = await creerInstallation(m!.base, { vmId: 'v-1', televersementId: tel.id }, MS);
        const entetes = avec(jetonDe(autre));
        const etrangere = await fetch(`${url}/installation/${inst.id}`, { headers: entetes });
        const inconnue = await fetch(`${url}/installation/jamais-vue`, { headers: entetes });
        const corps = await etrangere.text();
        expect([etrangere.status, corps]).toEqual([inconnue.status, await inconnue.text()]);
        expect(corps).toBe(JSON.stringify({ refus: 'installation-inconnue' }));
    });

    it('🔴 UN AGENT DE LA VM `B` N’OBTIENT PAS L’INSTALLEUR DESTINÉ À `A` — MUTATION n°1', async () => {
        // 🔴 C'EST LA MUTATION LA PLUS IMPORTANTE DU SOUS-BLOC. Sans la
        // comparaison de VM, n'importe quelle VM enrôlée téléchargerait
        // l'installeur de n'importe quelle autre — le contenu qu'un utilisateur
        // a déposé pour SA machine et pour elle seule. L'autorisation n'est pas
        // « un agent valide », c'est « CET agent-LÀ ».
        //
        // 🔴 LE TÉMOIN EST DANS LE MÊME `it()`, ET IL EST OBLIGATOIRE : sans lui,
        // l'assertion de refus serait vraie d'une route qui ne sert JAMAIS rien.
        const url = await servir('inst-contenu-vm-etrangere');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        await poserVmVivante(m!.base, 'v-b', 'b@exemple.test', 'PREFIXE-B');
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', randomBytes(33));
        await creerInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);

        const deB = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: avec(jetonDe('PREFIXE-B', 'agent')),
        });
        // ⚠️ COMPARÉ AU REFUS D'UN TÉLÉVERSEMENT VRAIMENT INCONNU : le refus doit
        // être INDISTINGUABLE, sans quoi `B` apprend que ce contenu existe.
        const inconnu = await fetch(`${url}/televersement/jamais-vu/contenu`, {
            headers: avec(jetonDe('PREFIXE-B', 'agent')),
        });
        expect([deB.status, await deB.text()]).toEqual([inconnu.status, await inconnu.text()]);
        expect(deB.status).toBe(404);

        const deA = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: avec(jetonDe('PREFIXE-A', 'agent')),
        });
        expect(deA.status).toBe(200);
        expect(Buffer.from(await deA.arrayBuffer())).toEqual(tel.octets);
    });

    it("sert les octets EN FLUX, à l'identique, avec sa longueur et sans nom de fichier", async () => {
        // 🔴 LES TRANCHES NE SONT JAMAIS ASSEMBLÉES : le corps est la
        // CONCATÉNATION de cinq fichiers de disque, et l'égalité octet pour
        // octet est ce qui l'éprouve. ⚠️ AUCUN `Content-Disposition` : l'agent
        // connaît le nom, il l'a reçu dans l'ordre.
        const url = await servir('inst-contenu-flux');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        const octets = randomBytes(31);
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', octets, { tailleTranche: 7 });
        await creerInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);
        expect(tranches!.lister(tel.id).map((t) => t.n)).toEqual([0, 1, 2, 3, 4]);

        const r = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: avec(jetonDe('PREFIXE-A', 'agent')),
        });
        expect(r.status).toBe(200);
        expect(r.headers.get('content-type')).toBe('application/octet-stream');
        expect(r.headers.get('content-length')).toBe(String(octets.length));
        expect(r.headers.get('content-disposition')).toBeNull();
        expect(r.headers.get('x-content-type-options')).toBe('nosniff');
        expect(Buffer.from(await r.arrayBuffer())).toEqual(octets);
    });

    it("🔴 un contenu NON SCELLÉ est refusé 409 — et l'agent ÉTRANGER, lui, lit 404", async () => {
        // 🔴 L'ORDRE DES DEUX GARDES EST PORTANT : répondre `409` à un agent sans
        // droit lui APPRENDRAIT que ce téléversement existe. L'autorisation passe
        // donc AVANT l'état, et ce test l'assère en opposant les deux réponses.
        const url = await servir('inst-contenu-non-scelle');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        await poserVmVivante(m!.base, 'v-b', 'b@exemple.test', 'PREFIXE-B');
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', Buffer.from('abcdefghij'), { scelle: false });
        await creerInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);
        const deA = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: avec(jetonDe('PREFIXE-A', 'agent')),
        });
        expect([deA.status, await deA.json()]).toEqual([409, { refus: 'non-scelle' }]);
        const deB = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: avec(jetonDe('PREFIXE-B', 'agent')),
        });
        expect([deB.status, await deB.json()]).toEqual([404, { refus: 'televersement-inconnu' }]);
    });

    it("🔴 `en_cours` reste servi — c'est la REPRISE —, `terminee` ne l'est plus", async () => {
        // 🔴 N'accepter qu'`en_attente` rendrait toute reprise de transfert
        // impossible sans qu'aucune trace ne le dise : l'agent qui a rapporté une
        // progression puis perdu sa connexion ne pourrait plus rien tirer.
        const url = await servir('inst-contenu-etats');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', Buffer.from('abc'));
        const inst = await creerInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);
        const entetes = avec(jetonDe('PREFIXE-A', 'agent'));
        await avancer(m!.base, inst.id, { phase: 'transfert', octetsFaits: 1, octetsTotal: 3, ecouleMs: 1 }, MS);
        expect((await fetch(`${url}/televersement/${tel.id}/contenu`, { headers: entetes })).status).toBe(200);
        await terminer(m!.base, inst.id, {
            issue: 'reussie', motif: null, codeSortie: 0, journal: '', journalTronque: false,
        }, MS + 1);
        const apres = await fetch(`${url}/televersement/${tel.id}/contenu`, { headers: entetes });
        expect([apres.status, await apres.json()]).toEqual([404, { refus: 'televersement-inconnu' }]);
    });

    it("un préfixe SANS enrôlement est refusé comme tout le reste, et la trace le nomme", async () => {
        // ⚠️ Le jeton est parfaitement valide : c'est la VM qu'il désigne qui
        // n'existe plus. La réponse ne le dit pas ; le journal, si.
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const url = await servir('inst-contenu-sans-enrolement');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', Buffer.from('abc'));
        await creerInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);
        const r = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: avec(jetonDe('PREFIXE-JAMAIS-ENROLE', 'agent')),
        });
        expect([r.status, await r.json()]).toEqual([404, { refus: 'televersement-inconnu' }]);
        expect(traces.join(' | ')).toContain('cas=prefixe-sans-enrolement');
    });
});
