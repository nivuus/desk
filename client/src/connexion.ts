// Câblage de l'écran de connexion : un formulaire DOM d'un côté,
// `POST /auth/connexion` de l'autre. Aucune règle ici — elles sont dans
// `jeton.ts`, qui est testé.
//
// ⚠️ CE FICHIER N'EST PAS TESTÉ UNITAIREMENT, et c'est DÉCLARÉ plutôt que
// subi : c'est la même convention que `shell-page.ts` et `main.ts`, qui ne le
// sont pas non plus. Ce qui la rend tenable est la clause qui l'accompagne :
// **toute règle que ce fichier porterait doit descendre dans `jeton.ts`**. Si
// une condition apparaît ici, c'est qu'elle est au mauvais endroit.
//
// 🔴 CE FICHIER DEMANDE AUSSI SA SESSION, ET LES DEUX BRANCHES QUI SUIVENT
// SONT DU CÂBLAGE, PAS DES RÈGLES — c'est ce qui les autorise ici malgré la
// clause ci-dessus. ⚠️ LE PLAN DE P4 SE CONTREDISAIT SUR CE POINT — sa tâche
// 14 interdit toute condition dans ce fichier, puis en prescrit les branches
// —, l'implémenteur l'a signalé sans le trancher, et LA REVUE TRANSVERSE DE P4
// L'A ARBITRÉ ICI (20 août 2026) : ce que la clause interdit est qu'une RÈGLE
// vive dans un fichier non testé, pas qu'un `if` y apparaisse. Le critère qui
// départage est REPRODUCTIBLE : une condition est une règle si la changer
// change ce que le PRODUIT décide ; elle est du câblage si elle ne fait que
// router une décision déjà prise ailleurs, et testée là-bas. Les deux branches
// ci-dessous relèvent du second cas — elles lisent une décision que
// `routes-session.ts` a prise et que ses tests couvrent. **La clause est donc
// resserrée, pas assouplie**, et le prochain `if` qui apparaîtra ici doit
// passer ce critère ou descendre. Les règles vivent aux deux bouts, et toutes deux sont
// testées : ce qu'un préfixe a le droit d'être est dans `prefixe.ts`
// (`poserPrefixe` LÈVE sur la chaîne vide), et ce que valent 200, 409 et 503
// est dans `plateforme/src/http/routes-session.ts`. Ce qui reste ici décide
// seulement d'ÉCRIRE, d'EFFACER, ou de NE RIEN TOUCHER — et la troisième issue
// est la raison pour laquelle il n'y a que deux branches :
//
//   ① le corps porte un `prefixe` — 200 comme 503 — : on l'écrit. Il est connu
//     et juste dans les deux cas, et la page en a besoin pour ne pas rejoindre
//     l'espace de noms partagé en attendant que la VM revienne ;
//   ② le service dit `aucune-vm` : on efface. Laisser en place le préfixe d'une
//     VM qu'on n'a plus ferait ouvrir des sessions au nom d'une autre machine ;
//   ③ tout le reste — jeton refusé, méthode, panne — ne dit RIEN de
//     l'attribution : le coffre n'est pas touché. Effacer sur un 401 perdrait
//     un préfixe encore juste ; c'est une absence de branche, et elle est
//     délibérée.
//
// ⚠️ ON NE REDIRIGE QUE SUR 200, ET LE COÛT EST ÉCRIT ICI PLUTÔT QUE DÉCOUVERT :
// un développeur sans VM enrôlée RESTE sur cet écran. Rediriger vers une shell
// qui rejoindrait l'espace de noms partagé serait précisément ce que ce
// sous-bloc existe pour éviter. Le mode d'essai local passe par `?prefixe=` sur
// l'URL de la shell (`prefixe.ts`), et il fonctionne toujours — le coffre est
// vide, donc la chaîne de requête reprend la main.
//
// ⚠️ LE MESSAGE D'ÉCHEC EST CELUI DU SERVICE, TEL QUEL. Il ne distingue pas
// « courriel inconnu » de « mot de passe faux » (`plateforme/src/http/
// routes-auth.ts` : un message qui les distinguerait serait un oracle
// d'énumération de comptes). Enrichir le texte ici défairait cette propriété
// depuis le seul endroit où personne ne penserait à la chercher.

import { accesDeReponse, poser, poserAcces } from './jeton';
import type { Ton } from './shell';
import { installerSelecteurDeThemeAuDOM } from './design/selecteur-theme';
import { effacerPrefixe, poserPrefixe } from './prefixe';
import { adressePlateforme } from './adresse-plateforme';

const params = new URLSearchParams(window.location.search);
// La MÊME convention que le signaling de `shell-page.ts` : un paramètre de
// requête, sinon L'ORIGINE DE LA PAGE. Inventer une seconde convention
// obligerait à savoir laquelle s'applique où.
//
// 🔴 CE N'EST PLUS `http://<hôte>:8080`, ET LE CHANGEMENT N'EST PAS COSMÉTIQUE.
// Derrière le proxy TLS de `deploiement/nginx.conf`, la page est servie en
// `https://` sur 443 : un `http://…:8080` y serait du contenu mixte, refusé
// par le navigateur, et rien n'écoute 8080 depuis l'extérieur de toute façon.
// La règle vit dans `adresse-plateforme.ts`, qui est PUR et testé.
const plateformeUrl = adressePlateforme(window.location, params.get('plateforme'));
// Où l'on repart une fois connecté. Le paramètre existe pour que l'écran
// puisse renvoyer vers la page qui a exigé la connexion, et pas seulement vers
// la shell.
const suite = params.get('suite') ?? 'shell.html';

const formulaire = document.querySelector<HTMLFormElement>('#connexion')!;
const champEmail = document.querySelector<HTMLInputElement>('#email')!;
const champMotDePasse = document.querySelector<HTMLInputElement>('#motdepasse')!;
const bouton = document.querySelector<HTMLButtonElement>('#valider')!;
const message = document.querySelector<HTMLDivElement>('#message')!;

// Le sélecteur de thème — extension raisonnée de la spec §5.2, justifiée dans
// l'en-tête de `design/selecteur-theme.ts`.
installerSelecteurDeThemeAuDOM(document.querySelector<HTMLElement>('#themes')!);

/* ── LE TON DU BANDEAU : UNE TABLE, PAS UNE RÈGLE ─────────────────────────
   🔴 AUCUNE CONDITION N'EST AJOUTÉE À CE FICHIER, et c'est la clause de son
   en-tête. Les branches ci-dessous existaient toutes AVANT le sous-bloc S3 ;
   il ne fait que donner à chacune la classe de ton qui lui correspond. Le
   critère de la revue transverse de P4 s'applique tel quel : une condition est
   une RÈGLE si la changer change ce que le produit décide. Changer un ton ne
   change aucune décision — ni le jeton posé, ni le préfixe écrit, ni la
   redirection. C'est de la présentation.

   ⚠️ CES TROIS CLASSES SONT INVISIBLES AU CONTRÔLE §7.9, limite connue et
   déclarée : il ne lit que les littéraux de `classList.add('…')` et de
   `className = '…'`, jamais une classe qui transite par une variable. Elles
   sont bien déclarées par `design/primitives/message.css` et employées par
   `primitives.html` — c'est la galerie et l'œil qui le disent ici, pas la
   commande. Même arbitrage que `shell-page.ts`. */
const CLASSE_DE_TON: Record<Ton, string> = {
    neutre: '',
    succes: 'message--succes',
    alerte: 'message--alerte',
    danger: 'message--danger',
};

function afficher(texte: string, ton: Ton): void {
    message.textContent = texte;
    message.classList.remove('message--succes', 'message--alerte', 'message--danger');
    const classe = CLASSE_DE_TON[ton];
    if (classe !== '') message.classList.add(classe);
}

/// Ce qui suit l'obtention d'un jeton, quel que soit le chemin qui l'a obtenu.
///
/// ⚠️ CE N'EST PAS UNE RÈGLE, C'EST DU CÂBLAGE — au sens du critère posé en
/// tête de ce fichier : ces branches ne font que router une décision prise par
/// `routes-session.ts` et couverte par SES tests.
///
/// 🔴 CE COMMENTAIRE A ÉCRIT « LA CLAUSE RESTE DONC RESSERRÉE, PAS
/// ASSOUPLIE », ET C'ÉTAIT UNE AFFIRMATION DE COMPLÉTUDE FAUSSE — corrigée
/// plutôt qu'effacée (revue transverse, 21 août 2026). La phrase était vraie
/// des branches DÉPLACÉES dans cette fonction-ci, et fausse de la fonction
/// NEUVE écrite juste en dessous : `tenterPomerium` y avait ajouté, dans le
/// même commit, une garde sur `corps.acces` qui, elle, était une RÈGLE au sens
/// du critère. La branche assouplissait donc la clause dans le geste même où
/// elle affirmait la resserrer. **La règle est depuis descendue dans
/// `jeton.ts` (`accesDeReponse`), où des tests la tiennent** ; ce qui reste
/// ici, et là-dessous, est du câblage. Ce paragraphe ne dit plus rien de ce
/// que ce fichier contiendra demain : le critère, lui, reste la seule chose à
/// appliquer au prochain `if` qui y apparaîtra.
///
/// ⚠️ CORPS DÉPLACÉ VERBATIM. Trois substitutions, et TROIS SEULEMENT :
///   ① `corps.acces` devient le paramètre `acces` ;
///   ② les `return` de sortie anticipée restent des `return` — la fonction
///      rend `void`, donc leur sens ne change pas ;
///   ③ le `catch` et le `finally` du `submit` RESTENT chez l'appelant : les
///      déplacer ici ferait réactiver `bouton.disabled = false` sur le chemin
///      Pomerium, où aucun bouton n'a jamais été désactivé.
async function chercherLaSession(acces: string): Promise<void> {
    afficher('recherche de votre machine…', 'neutre');
    const session = await fetch(`${plateformeUrl}/session`, {
        method: 'POST',
        // 🔴 L'EN-TÊTE `Authorization` REND LA REQUÊTE NON SIMPLE, donc
        // soumise à une requête préalable `OPTIONS`. C'est le défaut que la
        // recette de la tâche 8 a trouvé et fermé côté service ; il est
        // rappelé ici parce qu'aucun test de ce répertoire ne peut le voir.
        headers: { authorization: `Bearer ${acces}` },
    });
    const sien = await session.json().catch(() => undefined);

    // ① Un préfixe est un préfixe, qu'il vienne d'un 200 ou d'un 503.
    if (typeof sien?.prefixe === 'string') {
        // ⚠️ CE `poserPrefixe` PEUT LEVER, et c'est voulu : il ne le fait que
        // sur une chaîne vide, c'est-à-dire sur un service qui aurait
        // délivré un préfixe qui n'en est pas un. L'exception traverse alors
        // vers l'appelant (`submit`), dont le `catch` réseau attrape le
        // message qui CITE la cause en entier — le mot « injoignable » est
        // alors imprécis, la phrase qu'il encadre ne l'est pas. Déclaré
        // plutôt que découvert.
        poserPrefixe(window.localStorage, sien.prefixe);
    } else if (sien?.motif === 'aucune-vm') {
        // ② Aucune VM : le coffre est nettoyé, sans quoi le préfixe d'hier
        // survivrait à l'attribution qu'on vient de perdre.
        //
        // 🔴 CE LITTÉRAL EST UNE COPIE, ET RIEN NE LA CONFRONTE À SA
        // SOURCE (relevé à la revue transverse de P4, non corrigé). Sa
        // source canonique est `MOTIFS` dans
        // `plateforme/src/orchestration/refus.ts`, un tableau `as const`
        // dont le type DÉRIVE, précisément pour qu'ajouter un motif sans
        // lui donner son code HTTP soit une erreur de compilation. Cette
        // propriété s'arrête à la frontière du paquet : `client/` ne peut
        // pas importer de `plateforme/`, et le seul paquet partagé est
        // `proto/`, que P4 s'interdit de toucher (sa version appartient au
        // sous-bloc G1). CONSÉQUENCE À CONNAÎTRE : renommer `aucune-vm`
        // côté service laisserait ce test toujours faux, donc le préfixe
        // périmé au coffre — une panne MUETTE, que ni `npm run typecheck`
        // ni aucun test de ce dépôt ne verrait. Le remède est de faire
        // descendre `MOTIFS` dans `proto/ts` ; il est LÉGUÉ, pas fait.
        effacerPrefixe(window.localStorage);
    }

    if (!session.ok) {
        // Le motif du service, tel quel — et pour `agent-injoignable`, ce
        // que le service AVOUE ne pas savoir faire. Le cadrage promet « VM
        // injoignable -> le hub l'indique, propose redémarrage » ; avec le
        // backend statique le hub indique, et dit qu'il ne sait pas
        // redémarrer. Taire cet aveu ferait attendre un bouton qui n'existe
        // pas.
        const etat = sien?.etat ? ` (état : ${sien.etat})` : '';
        const aveu =
            sien?.redemarrage?.possible === false
                ? ` — la plateforme ne sait pas la redémarrer (${sien.redemarrage.motif}, backend ${sien.redemarrage.backend})`
                : '';
        afficher(`${sien?.motif ?? sien?.refus ?? session.status}${etat}${aveu}`, 'danger');
        return;
    }

    window.location.href = suite;
}

/// Demande l'identité au service AVANT de montrer le formulaire.
///
/// 🔴 C'EST LE 404 QUI PORTE LE MODE JUSQU'ICI, et c'est pourquoi cette page
/// n'a aucune variable de mode à connaître. Elle est bâtie statiquement par
/// Vite et ne peut lire aucune configuration du serveur : elle DEMANDE. Un
/// 404 signifie « ce montage authentifie par mot de passe » ; un 200, « le
/// proxy m'a déjà identifié ».
///
/// 🔴 CETTE PROMESSE A ÉTÉ MORTE SANS BRUIT, ET ELLE EST RÉPARÉE CÔTÉ SERVICE,
/// PAS ICI (22 août 2026). Le servant de fichiers statiques de la plateforme,
/// chaîné en dernier, replie tout chemin sans extension sur `index.html` :
/// avec `PLATEFORME_PAGE` armée, `/auth/moi` rendait `200 text/html` en mode
/// `motdepasse` — donc « le proxy m'a déjà identifié », ce qui est FAUX. Cette
/// page ne cassait que par ACCIDENT : le `.catch(() => undefined)` de
/// `reponse.json()` faisait retomber `accesDeReponse` sur `undefined`, donc le
/// formulaire, au bon endroit pour une mauvaise raison. La garde de mode de
/// `plateforme/src/http/routes-identite.ts` rend désormais le `404`
/// ELLE-MÊME ; rien ne change ici.
///
/// ⚠️ TOUT ÉCHEC RETOMBE SUR LE FORMULAIRE, y compris un échec réseau. C'est
/// le repli le moins surprenant : l'utilisateur voit un écran sur lequel il
/// peut agir, plutôt qu'une page vide dont rien ne dit ce qu'elle attend.
async function tenterPomerium(): Promise<boolean> {
    try {
        const reponse = await fetch(`${plateformeUrl}/auth/moi`);
        if (!reponse.ok) return false;
        // 🔴 LA VALIDATION DU CORPS VIT DANS `jeton.ts`, ET NON ICI. Elle y a
        // été FAITE DESCENDRE par la revue transverse du chantier
        // `auth-pomerium` (21 août 2026) : c'est une RÈGLE au sens du critère
        // de l'en-tête de ce fichier — la retirer fait écrire la chaîne
        // `"undefined"` au coffre, envoyer `Bearer undefined`, et laisser le
        // coffre EMPOISONNÉ —, et une règle ne vit pas dans un fichier non
        // testé. `accesDeReponse` et ses cinq tests la tiennent désormais.
        const acces = accesDeReponse(await reponse.json().catch(() => undefined));
        if (acces === undefined) return false;
        poserAcces(window.localStorage, acces);
        await chercherLaSession(acces);
        return true;
    } catch {
        return false;
    }
}

formulaire.addEventListener('submit', async (evenement) => {
    evenement.preventDefault();
    bouton.disabled = true;
    afficher('connexion…', 'neutre');

    try {
        const reponse = await fetch(`${plateformeUrl}/auth/connexion`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({
                email: champEmail.value,
                motdepasse: champMotDePasse.value,
            }),
        });
        const corps = await reponse.json().catch(() => undefined);

        if (!reponse.ok) {
            // Le motif du service, tel quel — voir l'en-tête.
            afficher(`refusé : ${corps?.refus ?? reponse.status}`, 'danger');
            return;
        }

        poser(window.localStorage, {
            acces: corps.acces,
            rafraichissement: corps.rafraichissement,
        });
        // Le mot de passe ne survit pas à la connexion : le champ est vidé
        // avant de quitter la page, pour qu'un retour arrière du navigateur ne
        // le retrouve pas rempli.
        champMotDePasse.value = '';

        await chercherLaSession(corps.acces);
    } catch (cause) {
        // Un échec RÉSEAU se dit comme tel : sur une autre origine, c'est le
        // symptôme d'une `PLATEFORME_ORIGINE_CLIENT` absente côté service
        // (`plateforme/src/config.ts`), et le confondre avec un refus
        // d'identifiants enverrait chercher le défaut au mauvais endroit.
        afficher(`plateforme injoignable (${String(cause)})`, 'danger');
    } finally {
        bouton.disabled = false;
    }
});

// ⚠️ Le formulaire est CACHÉ le temps de la tentative, puis remontré si elle
// échoue : l'afficher d'abord ferait clignoter un écran de connexion sur un
// montage qui n'en demande aucun.
formulaire.hidden = true;
afficher('identification…', 'neutre');
void tenterPomerium().then((abouti) => {
    if (abouti) return;
    formulaire.hidden = false;
    afficher('', 'neutre');
});
