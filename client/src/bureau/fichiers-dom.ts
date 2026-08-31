// LE LECTEUR « MES FICHIERS » — le câblage du pont ProjFS, à dépendances
// injectées.
//
// 🔴 EXTRAIT DE `shell-page.ts` LE 31 AOÛT 2026, VERBATIM. Les six blocs
// (quatre 🔴, deux ⚠️) qu'il porte documentent chacun un défaut MESURÉ :
// 🔴 l'activation utilisateur transitoire qu'exige `showDirectoryPicker()`,
// 🔴 le transtypage qui n'est PAS un contrôle, 🔴 l'ordre du `Bonjour` (F5),
// 🔴 l'attente de l'ouverture du canal (le `Bonjour` partait dans le vide, et
// AUCUNE écriture due n'aurait jamais été poussée), ⚠️ la garde `pont !== ce`
// (un écouteur qui survit à son objet), et ⚠️ la fermeture des flux au
// `close`. Aucun n'est décoratif : ils ont été déplacés à la lettre, jamais
// résumés.
//
// ⚠️ CE FICHIER N'EST PAS TESTÉ, ET NE PEUT PAS L'ÊTRE ICI : il ouvre un
// `RTCPeerConnection`, un `WebSocket` et un sélecteur de répertoire, dont
// aucun n'existe sous Node — et `client/` n'a ni jsdom ni happy-dom, par
// convention (`accent-dom.test.ts`). Tout ce qu'il porte de DÉCIDABLE est
// testé ailleurs : `shell.ts`, `fichiers/protocole.ts` et
// `fichiers/adaptateur.ts` sont purs et couverts.

import { creerAdaptateur } from '../fichiers/adaptateur';
import {
    choisirDossier,
    connecterCanalFichiers,
    sessionDuPont,
    type CanalFichiers,
} from '../fichiers/canal';
import { creerEcrivain, type RacineInscriptible } from '../fichiers/ecriture';
import type { RacineMutable } from '../fichiers/mutation';
import { creerMutateur } from '../fichiers/mutation-service';
import { creerServeur, trameBonjour, trameRafraichir } from '../fichiers/protocole';
import type { Bureau } from '../shell';

export interface DepsFichiers {
    bureau: Bureau;
    signalingUrl: string;
    /// 🔴 **UN FOURNISSEUR DE JETON FRAIS, ET IL N'Y EN AVAIT AUCUN** (critique
    /// ① de la revue finale du 31 août 2026). `fichiers/canal.ts` lisait
    /// `jetonAcces()` — le contenu **BRUT** du coffre, sans passer par
    /// `assurerAccesFrais` —, or « Choisir mon dossier » est un geste qui peut
    /// arriver n'importe quand après le chargement de la page, et un jeton
    /// d'accès vit **dix minutes** (`plateforme/src/identite/jeton.ts`).
    /// Le pont se voyait donc refuser sa session sans que rien ne relie
    /// l'échec à l'expiration.
    ///
    /// ⚠️ **APPELÉ APRÈS LE SÉLECTEUR DE RÉPERTOIRE, JAMAIS AVANT** : un
    /// `await` posé avant `showDirectoryPicker()` consommerait l'activation
    /// utilisateur transitoire, et le sélecteur serait refusé sans que rien ne
    /// le dise. Voir `monterLeLecteur`, où l'ordre est appliqué.
    jetonFrais(): Promise<string | undefined>;
    /// `?faute-fichiers=1` — variable de BANC, jamais une configuration
    /// livrée. Lue UNE fois par la page et passée ici, jamais relue : c'est la
    /// convention de `PLEIN_ECRAN` et de `PART_SONDAGE` côté agent — le
    /// mécanisme lit un drapeau qu'on lui donne. Et un utilisateur qui
    /// créerait un dossier au nom réservé ne casserait pas son propre pont.
    fautesArmees: boolean;
    boutonDossier: HTMLButtonElement;
    boutonRafraichir: HTMLButtonElement;
    boutonReprendre: HTMLButtonElement;
    /// Le `<details>` à déplier au clic.
    ///
    /// ⚠️ **OBLIGATOIRE DEPUIS LA REVUE FINALE (Minor ①).** Sa doc disait
    /// « `undefined` quand la page n'a pas de pli — c'est le cas de
    /// `shell.html` » : **faux depuis la tâche 9**, où `shell.html` est
    /// devenue une redirection sans aucune UI. L'unique appelant
    /// (`bureau/porteur-dom.ts`) passait TOUJOURS une section, si bien que
    /// l'optionnel n'était plus que du code mort justifié par une phrase
    /// fausse.
    section: HTMLDetailsElement;
}

export function installerLePont(deps: DepsFichiers): void {
    let pont: CanalFichiers | null = null;

    deps.boutonDossier.addEventListener('click', () => {
        // ⚠️ LE DÉPLIAGE EST AU CLIC, ET AVANT TOUT `await`. Le déplier au
        // montage RÉUSSI donnerait l'impression, sur une annulation du
        // sélecteur, que le clic n'a rien fait.
        deps.section.open = true;
        // 🔴 `showDirectoryPicker()` EXIGE UNE ACTIVATION UTILISATEUR TRANSITOIRE,
        // et c'est pourquoi il est appelé depuis ce gestionnaire, et
        // jamais depuis un message de canal. Le gestionnaire n'est pas `async` : un
        // `await` avant l'appel consommerait l'activation, et le sélecteur serait
        // refusé sans que rien ne le dise. Même contrainte que `window.open()`, que
        // cette page connaît déjà.
        void monterLeLecteur();
    });

    async function monterLeLecteur(): Promise<void> {
        // Un second clic remplace le dossier : l'ancien pont part d'abord, sans
        // quoi deux `PeerConnection` se disputeraient la session `…:fichiers` et
        // la seconde serait refusée par le relais.
        pont?.close();
        pont = null;
        deps.bureau.lecteurDemonte();

        const choix = await choisirDossier().catch((e: unknown) => {
            deps.bureau.lecteurEchoue((e as Error).message);
            return undefined;
        });
        // `null` = annulation délibérée, `undefined` = échec déjà signalé.
        if (choix === null || choix === undefined) return;

        // 🔴 **LE JETON EST REDEMANDÉ ICI, ET SEULEMENT ICI** — après le
        // sélecteur (qui exige l'activation utilisateur, donc aucun `await`
        // avant lui) et avant la poignée de main de signaling. C'est la
        // moitié « pont » de la critique ① de la revue finale : `canal.ts`
        // lisait le coffre BRUT, dont le jeton peut avoir expiré depuis le
        // chargement de la page.
        const jeton = await deps.jetonFrais();
        if (jeton === undefined) {
            deps.bureau.lecteurEchoue(
                'Votre session a expiré. Rechargez la page pour vous reconnecter.',
            );
            return;
        }

        // 🔴 LA MÊME POIGNÉE SERT À LIRE, À ÉCRIRE ET À MUTER.
        //
        // ❌ **CE TRANSTYPAGE N'EST PAS UN CONTRÔLE, ET F2 LE DÉCLARAIT COMME TEL.**
        // Ces lignes disaient : « le transtypage est le CONTRÔLE DE COMPATIBILITÉ
        // STRUCTURELLE de F2 : si la vraie poignée cessait de le satisfaire,
        // `tsc --noEmit` le dirait ICI ». **C'est faux, et c'est mesuré** :
        // `choix.racine` est typée `Racine`, et `RacineInscriptible` en est un
        // SOUS-type — un `as` vers un sous-type ASSERTE, il ne vérifie pas.
        // Ajouter à `RacineInscriptible` une méthode que
        // `FileSystemDirectoryHandle` n'a pas ne faisait rougir QUE le faux de
        // test.
        //
        // ✅ **LE CONTRÔLE RÉEL VIT DÉSORMAIS DANS `fichiers/canal.ts`**, sur la
        // VRAIE poignée, avant tout élargissement — et il a trouvé une
        // incompatibilité de F2 dès qu'il a été posé (voir
        // `journaux-pont-fichiers-f3/t9-controle-structurel-de-f2-vacueux.txt`).
        // Ces deux lignes-ci ne sont plus que du câblage.
        const racineInscriptible: RacineInscriptible = choix.racine as RacineInscriptible;
        const racineMutable: RacineMutable = choix.racine as RacineMutable;
        const ecrivain = creerEcrivain(racineInscriptible);
        const mutateur = creerMutateur(racineMutable);
        const serveur = creerServeur(
            creerAdaptateur(choix.racine, deps.fautesArmees),
            (m) => console.warn(m),
            {
                ecrivain,
                mutateur,
                onDues: (dues, retenues) => deps.bureau.ecrituresDues(dues, retenues),
                onEchecEcriture: (chemin, code) => deps.bureau.ecritureEchouee(chemin, code),
                onEchecMutation: (quoi, code) => deps.bureau.mutationEchouee(quoi, code),
                // 🔵 **L'INSTRUMENTATION QUE LA SPEC §3.5.1 EXIGE**, et elle part
                // par le journal parce que le navigateur est le seul à SAVOIR ce
                // qu'il a fait. Le pont, lui, journalise ce que LUI sait — voir la
                // divergence déclarée dans `protocole.ts`.
                onRenommagePorCopie: (de, vers, octets, entrees) => {
                    console.warn(
                        `renommage par copie « ${de} » → « ${vers} » : ${octets} octets, ` +
                            `${entrees} entree(s) — move() absente, repli LOCAL (zero octet sur le canal)`,
                    );
                },
            },
        );
        try {
            pont = await connecterCanalFichiers({
                signalingUrl: deps.signalingUrl,
                sessionId: sessionDuPont(),
                jeton,
                onStatus: (m) => console.info(m),
                traiter: (octets) => serveur.traiter(octets),
            });
        } catch (e) {
            deps.bureau.lecteurEchoue((e as Error).message);
            return;
        }
        deps.bureau.lecteurMonte(choix.nom);

        // ════════════════════════════════════════════════════════════════════
        // 🔴 **F5 — `Bonjour` PART ICI, ET L'ORDRE N'EST PAS INDIFFÉRENT.**
        //
        // Il est envoyé **APRÈS** que l'écrivain, le mutateur et l'adaptateur sont
        // posés et que le canal est ouvert — jamais avant. C'est lui, et lui seul,
        // qui déclenche la reprise des écritures dues côté pont : avant F5, celle-ci
        // courait au démarrage du FIL, c'est-à-dire *sans savoir si un navigateur
        // est là, ni lequel, ni sur quel répertoire*. F2 a mesuré, deux fois sur
        // deux, la poussée du rejeu **0,8 s AVANT** cette annonce de montage, puis
        // une expiration **+30,2 s** plus tard.
        //
        // ⚠️ **`choix.nom` est le `name` de la poignée de répertoire**, et c'est la
        // MÊME valeur qu'un répertoire choisi par `showDirectoryPicker()` ou par
        // OPFS rendrait : c'est ce qui permet à la recette d'éprouver la règle sans
        // le sélecteur. **Elle n'éprouve pas pour autant le modèle de permission**,
        // qui n'est appelé nulle part dans ce dépôt.
        //
        // ⚠️ **`forcer: false` au montage, TOUJOURS.** Forcer est un geste de
        // l'utilisateur, jamais un défaut : un `true` ici rendrait le bouton
        // « Reprendre » inatteignable et réintroduirait le danger du §6.4 cas 2.
        // ════════════════════════════════════════════════════════════════════
        // 🔴 **L'ANNONCE ATTEND L'OUVERTURE DU CANAL, ET C'EST UN DÉFAUT QUE SEUL
        // LE CHEMIN RÉEL POUVAIT MONTRER.**
        //
        // *La première rédaction envoyait ici même, sans attendre.*
        // `connecterCanalFichiers` rend dès que la réponse SDP est reçue ; le canal
        // de données, lui, s'ouvre **après**. Mesuré sur la VM, dans cet ordre :
        // « pont fichiers : réponse reçue » → **`canal fichiers ferme : annonce non
        // envoyee`** → « connecting » → « connected » → « canal fichiers ouvert ».
        // Le `Bonjour` partait dans le vide, **et donc AUCUNE écriture due n'aurait
        // jamais été poussée** — un silence, c'est-à-dire pire que les trente
        // secondes que F2 avait mesurées et que F5 existe pour supprimer.
        //
        // 🔵 **C'est mon propre `console.warn` qui l'a dénoncé.** Un envoi qui
        // aurait échoué en silence aurait laissé la recette verte sur ses critères
        // de cache et muette sur celui-ci.
        //
        // ⚠️ **LES DEUX BRANCHES SONT NÉCESSAIRES** : le canal peut être déjà
        // ouvert quand on arrive ici (rien ne l'interdit), et n'écouter que
        // `'open'` manquerait alors l'événement pour toujours.
        const envoyerAuPont = (trame: ArrayBuffer): void => {
            if (!pont) {
                console.warn('aucun pont : annonce non envoyee');
                return;
            }
            const canal = pont.canal;
            if (canal.readyState === 'open') canal.send(trame);
            else if (canal.readyState === 'connecting') {
                canal.addEventListener('open', () => canal.send(trame), { once: true });
            } else console.warn('canal fichiers ferme : annonce non envoyee');
        };
        envoyerAuPont(trameBonjour(choix.nom, false));
        deps.boutonRafraichir.onclick = () => envoyerAuPont(trameRafraichir());
        deps.boutonReprendre.onclick = () => {
            // **Reprendre est un `Bonjour` FORCÉ**, et non un verbe de plus : c'est
            // exactement « je confirme que ce répertoire est le bon ». Le pont
            // mémorise alors le nom annoncé, et le bouton disparaît à l'annonce
            // suivante — sans qu'aucun état local n'ait à être remis à zéro ici.
            envoyerAuPont(trameBonjour(choix.nom, true));
        };

        // ⚠️ LE DÉMONTAGE SUIT LA CONNEXION, PAS LE CANAL SEUL : un canal fermé sur
        // une connexion qui se rétablit serait rouvert par l'agent, alors qu'une
        // connexion `failed` ou `closed` est définitive pour ce pont-ci.
        //
        // ⚠️ LE PONT COURANT EST CAPTURÉ, et l'écouteur se tait s'il n'est plus
        // celui-là. Sans cette garde, la fermeture de l'ANCIEN pont — que le
        // remontage vient de provoquer — effacerait l'état du NOUVEAU : un
        // écouteur qui survit à son objet est le patron exact d'une course qu'on
        // ne voit qu'en cliquant deux fois.
        const ce = pont;
        ce.pc.addEventListener('connectionstatechange', () => {
            if (pont !== ce) return;
            const etat = ce.pc.connectionState;
            if (etat === 'failed' || etat === 'closed') deps.bureau.lecteurDemonte();
        });
        // ⚠️ **LES FLUX OUVERTS SE FERMENT AVEC LE CANAL.** Un flux
        // `createWritable()` laissé ouvert garde son fichier d'échange, et son
        // fichier de destination reste INCHANGÉ — la committaison est au `close()`.
        ce.canal.addEventListener('close', () => ecrivain.abandonner());
    }
}
