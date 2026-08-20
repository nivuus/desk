// La boucle du canal `/agent`, sur de VRAIS `WebSocket`, port 0 attribué par
// le système et horloge INJECTÉE — même montage que `garde-fil.test.ts`, et
// pour la même raison : deux des sept critères ci-dessous exigent que le temps
// AVANCE entre deux messages, et une horloge figée les rendrait inertes.
//
// 🔴 CE FICHIER TRAVERSE DEUX MODULES À DESSEIN. Le second test prend le jeton
// que le canal délivre et le présente à la GARDE du relais. C'est le seul
// endroit du dépôt où la chaîne complète — enrôlement, signature, claim de
// type, préfixe de session — est éprouvée bout en bout ; chacun de ses maillons
// est juste tout seul, et c'est précisément la classe de défaut qui franchit
// une frontière de tâche que ce dépôt paie à chaque branche.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocket, WebSocketServer } from 'ws';
import {
    PLATEFORME_VERSION,
    encodeBattement,
    encodeEnroler,
} from '../../../proto/ts/plateforme';
import { baseNeuve, piloteCompteur } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { demarrerServeur, type ServicePlateforme } from '../http/serveur';
import { garde as fabriquerGarde } from '../identite/garde';
import { DUREE_JETON_ACCES_MS, verifierJeton } from '../identite/jeton';
import { ProprieteDeSession } from '../signaling/propriete';
import { servirLeCanalAgent } from './canal';
import { RegistreAgents } from './registre';
import { ECHECS_MAX_ADRESSE, ECHECS_MAX_COMPTE, Frein } from '../securite/frein';
import {
    attendreVu,
    enrolerUneVm,
    ouvrir,
    ouvrirUrl,
    P,
    SECRET,
    SECRET_VM,
    T0,
} from './canal-harnais';

let base: Pilote | undefined;
let wss: WebSocketServer | undefined;
let service: ServicePlateforme | undefined;
let maintenant = T0;

afterEach(async () => {
    if (wss) {
        for (const socket of wss.clients) socket.terminate();
        await new Promise<void>((r) => wss!.close(() => r()));
        wss = undefined;
    }
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

/// Monte le canal sur un port attribué par le système, avec l'horloge du
/// fichier. On attend `listening` : lire `address()` avant que le socket ne
/// soit lié rendrait `null`, et le test se connecterait à un port inexistant.
async function demarrer(p: Pilote, frein: Frein = new Frein()): Promise<number> {
    maintenant = T0;
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    servirLeCanalAgent(wss, {
        base: p,
        secretJeton: SECRET,
        maintenant: () => maintenant,
        registre: new RegistreAgents(),
        frein,
        // Aucun proxy declare : `adresseSource` ignorera donc tout
        // `X-Forwarded-For`, et la cle d'adresse sera celle du pair reel.
        proxyDeConfiance: new Set(),
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

describe('la boucle du canal /agent', () => {
    it('`enroler` avec le BON secret rend `enrole`, avec préfixe et jeton', async () => {
        base = await baseNeuve('canal-enrole');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));

        const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));
        expect(rep.type).toBe('enrole');
        expect(rep.v).toBe(PLATEFORME_VERSION);
        expect(rep.prefixe).toBe(P);
        expect(typeof rep.jeton).toBe('string');
        // L'expiration est EXACTE, pas « supérieure à zéro » : c'est
        // l'assertion qui interdit un `Date.now()` caché dans le canal.
        expect(rep.expire_a).toBe(T0 + DUREE_JETON_ACCES_MS);
        pair.socket.terminate();
    });

    it('🔴 le jeton rendu est VÉRIFIABLE PAR LA GARDE, et de type `agent`', async () => {
        // 🔴 CE TEST TRAVERSE DEUX MODULES À DESSEIN. La rouge est de signer
        // sans le claim de type : le jeton resterait un JWT parfaitement
        // valide, et la garde le refuserait pour le rôle `agent` — un canal
        // qui délivre des jetons que rien n'accepte. Chacun des deux modules
        // est juste tout seul ; c'est leur JONCTION qui n'est éprouvée qu'ici.
        base = await baseNeuve('canal-jeton-garde');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));
        const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));

        // Le SUJET du jeton est le PRÉFIXE, et son type est `agent`.
        expect(verifierJeton(rep.jeton, SECRET, T0)).toEqual({
            ok: true,
            sujet: P,
            type: 'agent',
        });

        // Et la garde du relais l'accepte pour une session que ce préfixe
        // porte — c'est-à-dire pour l'usage RÉEL du jeton.
        const g = fabriquerGarde(SECRET, () => T0, new ProprieteDeSession());
        expect(g.verifier({ role: 'agent', session: `${P}:bureau`, jeton: rep.jeton })).toEqual({
            ok: true,
        });
        // TÉMOIN, dans la même exécution : la garde refuse ce même jeton pour
        // une session que le préfixe ne porte PAS. Sans lui, l'acceptation
        // ci-dessus serait vraie d'une garde qui ne regarderait rien.
        expect(g.verifier({ role: 'agent', session: 'bureau', jeton: rep.jeton }).ok).toBe(false);
        pair.socket.terminate();
    });

    it('🔴 `enroler` au MAUVAIS secret rend `refus` et FERME le socket, sans écrire le secret au journal', async () => {
        // 🔴 LA FERMETURE EST LA MOITIÉ QUI COMPTE. Un pair refusé qui
        // garderait son socket ouvert pourrait réessayer sans limite sur la
        // même connexion : c'est le déni de service que P5 doit freiner, et
        // c'est ici gratuit à fermer.
        base = await baseNeuve('canal-mauvais-secret');
        await enrolerUneVm(base, 'v-1');
        const journal = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const pair = await ouvrir(await demarrer(base));

        const rep = await pair.dire(encodeEnroler('v-1', 'un-autre-secret-de-la-vraie-longueur'));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'enrolement' });

        await pair.ferme;
        // ENVOYER PUIS FERMER, jamais l'inverse : un pair qui verrait une
        // fermeture sans motif ne saurait pas s'il doit se corriger ou
        // réessayer.
        expect(pair.ordre).toEqual(['message', 'close']);
        expect(pair.socket.readyState).toBe(WebSocket.CLOSED);

        // ⚠️ LE JOURNAL NOMME LA VM, JAMAIS LE SECRET. On balaie sur le NOM du
        // champ et non sur sa valeur : chercher la valeur ferait passer un
        // journal qui écrirait « secret= » suivi d'autre chose.
        const lignes = journal.mock.calls.map((c) => String(c[0])).join('\n');
        expect(lignes).toContain('v-1');
        expect(lignes).not.toContain('secret');
    });

    it('🔴 un message de version PLATEFORME_VERSION + 1 est refusé, motif `version`', async () => {
        // 🔴 C'est la moitié TypeScript du critère ③. L'omettre ferait
        // interpréter les champs d'un message d'une version future avec le
        // sens de la nôtre.
        base = await baseNeuve('canal-version');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));

        const rep = await pair.dire(
            JSON.stringify({ type: 'battement', v: PLATEFORME_VERSION + 1 }),
        );
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'version' });
        // ⚠️ ET LE SOCKET SE FERME, décision de ce module et non du plan : un
        // pair qui ne parle pas notre version ne réussira JAMAIS sur cette
        // connexion, et le laisser ouvert le ferait boucler à la place de sa
        // reprise à repli exponentiel.
        await pair.ferme;
        expect(pair.ordre).toEqual(['message', 'close']);
    });

    it('🔴 `battement` AVANT `enroler` est refusé, motif `sequence`, et ne délivre AUCUN jeton', async () => {
        // 🔴 LES DEUX MOITIÉS COMPTENT, et la seconde est la vraie. Un
        // `battement-recu` rendu à un anonyme porterait un JETON — c'est-à-dire
        // que le canal signerait une identité d'agent pour un pair qui n'a
        // présenté aucun secret. C'est exactement la fuite d'E12 sous une autre
        // forme.
        base = await baseNeuve('canal-sequence');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));

        const rep = await pair.dire(encodeBattement());
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        expect(rep.jeton).toBeUndefined();
        // Le socket reste OUVERT : contrairement au mauvais secret, une erreur
        // de séquence se corrige — le pair peut s'enrôler puis reprendre. Même
        // partage que le relais entre le message malformé et la poignée de
        // main refusée.
        expect(pair.socket.readyState).toBe(WebSocket.OPEN);
        pair.socket.terminate();
    });

    it('🔴 `battement` après `enroler` AVANCE `vu_a` en base', async () => {
        // 🔴 Ne rien écrire laisserait `vu_a` figé, et la VM serait
        // éternellement `injoignable` (`agents/fraicheur.ts`) alors qu'elle
        // bat. L'horloge AVANCE entre les deux messages : sans cela, un
        // `vu_a` simplement recopié de l'enrôlement passerait le test.
        base = await baseNeuve('canal-vu-a');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));

        await pair.dire(encodeEnroler('v-1', SECRET_VM));
        await attendreVu(base, 'v-1', (vu) => vu === T0, 'posé à l’enrôlement');

        maintenant = T0 + 30_000;
        await pair.dire(encodeBattement());
        expect(await attendreVu(base, 'v-1', (vu) => vu === T0 + 30_000, 'avancé au battement')).toBe(
            T0 + 30_000,
        );
        pair.socket.terminate();
    });

    it('🔴 `battement` rend un jeton FRAIS, valide à un instant où le précédent est EXPIRÉ', async () => {
        // 🔴 Rendre le même jeton ferait tomber l'agent à l'expiration du
        // premier — dix minutes après l'enrôlement —, sans qu'il le voie venir.
        // Comparer les deux `expire_a` ne suffit pas à le dire : ce qui le dit
        // est qu'à l'instant où l'ANCIEN est refusé, le NOUVEAU passe.
        base = await baseNeuve('canal-jeton-frais');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));

        const premier = await pair.dire(encodeEnroler('v-1', SECRET_VM));
        maintenant = T0 + 30_000;
        const second = await pair.dire(encodeBattement());

        expect(second.type).toBe('battement-recu');
        expect(second.expire_a).toBe(T0 + 30_000 + DUREE_JETON_ACCES_MS);
        expect(Number(second.expire_a)).toBeGreaterThan(Number(premier.expire_a));

        // L'instant EXACT où le premier meurt : la borne est franche
        // (`maintenant >= exp`), donc le premier est refusé et le second passe.
        const instantCritique = T0 + DUREE_JETON_ACCES_MS;
        expect(verifierJeton(premier.jeton, SECRET, instantCritique)).toEqual({
            ok: false,
            motif: 'expire',
        });
        expect(verifierJeton(second.jeton, SECRET, instantCritique).ok).toBe(true);
        pair.socket.terminate();
    });
});

describe('le canal, CÂBLÉ dans le service entier', () => {
    it('🔴 le chemin `/agent` du service sert réellement le canal', async () => {
        // 🔴 SANS CE TEST, OUBLIER L'APPEL DANS `http/serveur.ts` NE ROUGIRAIT
        // NULLE PART. La tâche 13 n'éprouve que la MONTÉE du chemin `/agent` —
        // un `WebSocketServer` qui accepte la connexion et n'écoute rien la
        // passerait. C'est la panne muette exacte que `serveur.ts` invoque déjà
        // pour rendre `base` et `garde` REQUISES.
        //
        // L'horloge n'est pas injectable ici (`demarrerServeur` lit `Date.now`),
        // donc l'expiration n'est pas assertée sur une valeur exacte : ce test
        // mesure le CÂBLAGE, les sept ci-dessus mesurent la boucle.
        const config: Config = {
            hote: '127.0.0.1',
            port: 0,
            base: 'sqlite',
            urlBase: ':memory:',
            secretJeton: SECRET,
            // Aucun proxy declare : voir `config.ts`, l'ensemble vide est le
            // defaut et signifie « ne croire l'adresse annoncee par personne ».
            proxyDeConfiance: new Set(),
        };
        base = await baseNeuve('canal-service');
        await enrolerUneVm(base, 'v-1');
        service = await demarrerServeur(config, base);

        const surLeChemin = await ouvrirUrl(`ws://127.0.0.1:${service.port}/agent`);
        const rep = await surLeChemin.dire(encodeEnroler('v-1', SECRET_VM));
        expect(rep.type).toBe('enrole');
        expect(rep.prefixe).toBe(P);
        surLeChemin.socket.terminate();
    });
});

describe('le frein du canal /agent', () => {
    /// Une tentative d'enrôlement complète : ouvrir, dire, lire le refus.
    ///
    /// ⚠️ UN SOCKET NEUF À CHAQUE FOIS, et ce n'est pas du zèle : le motif
    /// `enrolement` FERME le socket (`MOTIFS_FERMANTS`), et réutiliser le pair
    /// mesurerait un socket mort.
    async function tenter(port: number, vm: string, secret: string): Promise<Record<string, unknown>> {
        const pair = await ouvrir(port);
        const rep = await pair.dire(encodeEnroler(vm, secret));
        pair.socket.terminate();
        return rep;
    }

    it('(a) la n+1ᵉ tentative sur la MÊME VM est refusée par le frein', async () => {
        base = await baseNeuve('canal-frein-vm');
        await enrolerUneVm(base, 'v-1');
        const port = await demarrer(base);
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            expect((await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret')).type).toBe('refus');
        }
        // Le refus est le MÊME (voir (c)) ; ce qui a changé est son COÛT,
        // que (e) mesure.
        expect((await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret')).type).toBe('refus');
    }, 30000);

    it("(b) la n+1ᵉ depuis la MÊME adresse, VMs toutes DISTINCTES, est freinée", async () => {
        const reel = await baseNeuve('canal-frein-adresse');
        base = reel;
        const compteur = piloteCompteur(reel);
        const port = await demarrer(compteur.pilote);
        // Aucun budget de VM ne peut mordre : chaque nom est essayé UNE fois.
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i++) {
            await tenter(port, `inconnue-${i}`, 'peu-importe');
        }
        compteur.remettre();
        expect((await tenter(port, 'encore-une-autre', 'peu-importe')).type).toBe('refus');
        // 🔴 Le discriminant : la tentative freinée n'a RIEN lu en base.
        expect(compteur.acces()).toBe(0);
    }, 60000);

    it('(c) 🔴 le refus freiné est le MÊME MESSAGE que le refus d’enrôlement', async () => {
        // 🔴 UN MOTIF `frein` DISTINCT RENDRAIT À L'ATTAQUANT L'INFORMATION
        // « cette VM existe et je l'ai fait déclencher » : c'est exactement
        // l'ORACLE D'ÉNUMÉRATION que `agents/enrolement.ts` ferme sur trois
        // paragraphes, rouvert par la porte du frein. Les deux refus sont
        // produits DANS LE MÊME TEST et comparés objet pour objet.
        base = await baseNeuve('canal-frein-oracle');
        await enrolerUneVm(base, 'v-1');
        const port = await demarrer(base);

        const refusNonFreine = await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        }
        const refusFreine = await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        expect(refusFreine).toEqual(refusNonFreine);
    }, 30000);

    it('(d) le JOURNAL, lui, distingue les deux', async () => {
        // Même partage que `identite/garde.ts` : `message` sur le fil,
        // `journal` chez nous. Le demandeur n'apprend rien ; l'exploitant, si.
        const avertir = vi.spyOn(console, 'warn').mockImplementation(() => {});
        base = await baseNeuve('canal-frein-journal');
        await enrolerUneVm(base, 'v-1');
        const port = await demarrer(base);
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        }
        await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        const lignes = avertir.mock.calls.map((c) => String(c[0]));
        // Le refus d'enrôlement ordinaire, écrit par `enrolement.ts`.
        expect(lignes.some((l) => l.includes('enrôlement refusé pour la VM v-1'))).toBe(true);
        // Et la ligne du frein, qui n'existe QUE côté exploitant.
        const freinees = lignes.filter((l) => l.startsWith('frein '));
        expect(freinees.length).toBeGreaterThanOrEqual(1);
        expect(freinees[0]).toContain('route=/agent');
        expect(freinees[0]).toContain('adresse=');
    }, 30000);

    it('(e) 🔴 le refus freiné ne lit RIEN en base — donc ne dérive aucun `scrypt`', async () => {
        // 🔴 C'EST LE POINT DE TOUTE LA TÂCHE. `verifierEnrolement` lit
        // `agent_enrole` PUIS dérive une empreinte `scrypt`, à mémoire dure et
        // délibérément chère (68 ms mesurés le 20 août 2026). Un frein posté
        // APRÈS ne protège rien : il compte des tentatives qu'il a déjà payées,
        // et un attaquant épuise le service sans jamais deviner un secret.
        const reel = await baseNeuve('canal-frein-sans-scrypt');
        base = reel;
        await enrolerUneVm(reel, 'v-1');
        const compteur = piloteCompteur(reel);
        const port = await demarrer(compteur.pilote);
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        }
        compteur.remettre();
        await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        expect(compteur.acces()).toBe(0);
    }, 30000);

    it('(f) un enrôlement RÉUSSI remet le compteur de la VM à zéro', async () => {
        // Même règle que `/auth/connexion` : le succès efface la clé de la VM,
        // JAMAIS celle de l'adresse — sinon un attaquant qui possède une VM
        // valide se blanchirait entre deux rafales.
        base = await baseNeuve('canal-frein-succes');
        await enrolerUneVm(base, 'v-1');
        const port = await demarrer(base);
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) {
            await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        }
        expect((await tenter(port, 'v-1', SECRET_VM)).type).toBe('enrole');
        // Sans la remise à zéro, la dernière de cette seconde série serait
        // freinée, donc n'atteindrait jamais la base.
        const reel2 = base;
        const compteur = piloteCompteur(reel2);
        void compteur;
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) {
            expect((await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret')).type).toBe('refus');
        }
        // Et le succès reste possible : la VM n'est pas verrouillée.
        expect((await tenter(port, 'v-1', SECRET_VM)).type).toBe('enrole');
    }, 30000);
});
