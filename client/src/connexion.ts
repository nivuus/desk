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

import { poser } from './jeton';
import { effacerPrefixe, poserPrefixe } from './prefixe';

const params = new URLSearchParams(window.location.search);
// La MÊME convention que le signaling de `shell-page.ts` : un paramètre de
// requête, sinon l'hôte de la page et le port 8080. Inventer une seconde
// convention obligerait à savoir laquelle s'applique où.
const plateformeUrl = params.get('plateforme') ?? `http://${window.location.hostname}:8080`;
// Où l'on repart une fois connecté. Le paramètre existe pour que l'écran
// puisse renvoyer vers la page qui a exigé la connexion, et pas seulement vers
// la shell.
const suite = params.get('suite') ?? 'shell.html';

const formulaire = document.querySelector<HTMLFormElement>('#connexion')!;
const champEmail = document.querySelector<HTMLInputElement>('#email')!;
const champMotDePasse = document.querySelector<HTMLInputElement>('#motdepasse')!;
const bouton = document.querySelector<HTMLButtonElement>('#valider')!;
const message = document.querySelector<HTMLDivElement>('#message')!;

formulaire.addEventListener('submit', async (evenement) => {
    evenement.preventDefault();
    bouton.disabled = true;
    message.textContent = 'connexion…';

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
            message.textContent = `refusé : ${corps?.refus ?? reponse.status}`;
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

        message.textContent = 'recherche de votre machine…';
        const session = await fetch(`${plateformeUrl}/session`, {
            method: 'POST',
            // 🔴 L'EN-TÊTE `Authorization` REND LA REQUÊTE NON SIMPLE, donc
            // soumise à une requête préalable `OPTIONS`. C'est le défaut que la
            // recette de la tâche 8 a trouvé et fermé côté service ; il est
            // rappelé ici parce qu'aucun test de ce répertoire ne peut le voir.
            headers: { authorization: `Bearer ${corps.acces}` },
        });
        const sien = await session.json().catch(() => undefined);

        // ① Un préfixe est un préfixe, qu'il vienne d'un 200 ou d'un 503.
        if (typeof sien?.prefixe === 'string') {
            // ⚠️ CE `poserPrefixe` PEUT LEVER, et c'est voulu : il ne le fait que
            // sur une chaîne vide, c'est-à-dire sur un service qui aurait
            // délivré un préfixe qui n'en est pas un. L'exception traverse alors
            // le `catch` réseau ci-dessous, dont le message CITE la cause en
            // entier — le mot « injoignable » est alors imprécis, la phrase
            // qu'il encadre ne l'est pas. Déclaré plutôt que découvert.
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
            message.textContent = `${sien?.motif ?? sien?.refus ?? session.status}${etat}${aveu}`;
            return;
        }

        window.location.href = suite;
    } catch (cause) {
        // Un échec RÉSEAU se dit comme tel : sur une autre origine, c'est le
        // symptôme d'une `PLATEFORME_ORIGINE_CLIENT` absente côté service
        // (`plateforme/src/config.ts`), et le confondre avec un refus
        // d'identifiants enverrait chercher le défaut au mauvais endroit.
        message.textContent = `plateforme injoignable (${String(cause)})`;
    } finally {
        bouton.disabled = false;
    }
});
