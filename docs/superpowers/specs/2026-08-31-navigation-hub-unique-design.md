# Navigation : le hub devient la SEULE surface — conception

**31 août 2026.** Demande du propriétaire du dépôt, verbatim :

> « Il faut améliorer la navigation : on devrait jamais avoir à aller sur
> shell.html. On peut pas supprimer shell.html ? Et surtout si je vais sur
> hub.html, ça valide et rafraîchis ma connexion »

---

## 1. Le défaut, tel qu'il se mesure aujourd'hui

Le produit a **deux** surfaces que l'utilisateur doit connaître, et la
répartition de leurs fonctions n'est le résultat d'aucune décision : elle est
le sédiment du lot 14 (« sers le hub à la racine ») et du correctif du 30 août
qui lui a ajouté un lien.

| Page | Ce qu'elle fait | Ce qui lui manque |
| --- | --- | --- |
| `hub.html`, servi à `/` | catalogue, dépôt d'installeurs, thème. Obtient un jeton par Pomerium **si le coffre est vide** (`jeton.ts::assurerAcces`) | n'ouvre **aucune** session de signaling : elle ne voit jamais `fenetre-ouverte` |
| `shell.html` | tient le rôle `client` sur la session de contrôle — **la seule page qui reçoive `fenetre-ouverte`** et ouvre les pop-ups de session ; porte « Mes fichiers » | ne tente **jamais** Pomerium : sans jeton elle renvoie sur `connexion.html`, y compris derrière un proxy qui aurait pu l'identifier |

**Deux défauts distincts en découlent, et les confondre serait une erreur :**

🔴 **① L'utilisateur doit savoir qu'une seconde surface existe.** Le lien
« Mon bureau » et l'ouverture au clic sur « Lancer » (`hub/page.ts`) sont des
correctifs qui rendent le chemin praticable, pas une conception. Le
propriétaire demande qu'il disparaisse.

🔴 **② `assurerAcces` ne regarde pas si le jeton est PÉRIMÉ.** Il rend le
contenu du coffre dès qu'il n'est pas vide. `expireAvant` est écrit et testé ;
`rafraichirSiNecessaire` est écrit et testé et **n'a aucun appelant de
production** — `jeton.ts` le dit lui-même, `grep` à l'appui. Le symptôme est un
hub qui s'ouvre normalement puis échoue sur chaque appel, sans que rien ne
relie l'échec à l'expiration. C'est la seconde moitié de la phrase du
propriétaire.

⚠️ **Ce que le correctif du 30 août avait déjà nommé sans le trancher** :
`hub/bureau.ts` écrit en toutes lettres que « fusionner les deux surfaces est
l'autre voie du diagnostic : c'est une décision de conception qui appartient au
propriétaire, pas à un correctif ». **Cette décision est prise ici, le 31 août
2026, par le propriétaire du dépôt.**

## 2. Les trois décisions prises, et leur raison

| Question | Décision | Raison retenue |
| --- | --- | --- |
| Deux onglets du hub, alors que le rôle `client` est **exclusif** (`plateforme/src/signaling/appariement.ts::declarer`) | **Le premier garde la session** ; les autres affichent le même état et ne montrent aucune erreur | un utilisateur qui ouvre un second onglet n'a rien fait de fautif ; lui montrer « un client est déjà connecté » serait lui rendre un état interne |
| Le fichier `shell.html` | **Redirection permanente** vers `/`, paramètres conservés | les PWA installées portent `shell.html?app=<id>` dans un manifeste `blob:` qu'elles **ne reliront jamais** (legs G5 déclaré) : supprimer le fichier les casse définitivement |
| Quand vérifier la fraîcheur du jeton | **Avant chaque usage**, test local puis réseau seulement si nécessaire | couvre le chargement, chaque lancement et la réouverture du socket, sans minuterie à calibrer — or ce dépôt n'a calibré **aucune** de ses constantes |

## 3. L'élection du porteur — `client/src/bureau/porteur.ts`

**Primitive : Web Locks.** Chaque onglet demande le verrou
`nivuus-bureau-<préfixe>` en mode `exclusive`. Celui qui l'obtient est
**porteur** : il ouvre le WebSocket en rôle `client` et tient le verrou tant
qu'il vit. À la fermeture de l'onglet le verrou est libéré par le navigateur
lui-même, et **un onglet en attente devient porteur sans qu'aucun code ne
l'orchestre**.

🔵 **C'est la sémantique demandée, prise telle quelle plutôt que reconstruite** :
une élection écrite à la main sur `BroadcastChannel` devrait détecter la mort
d'un pair, ce qu'aucun événement ne signale — c'est exactement le trou que
`setInterval(redessiner, 1000)` bouche déjà dans `shell-page.ts`, faute de
mieux.

**Le module est PUR et testé**, dépendances injectées (`demanderVerrou`,
`diffuser`, `ouvrirSocket`, `maintenant`) — la convention de `accent-dom.ts` et
`presse-papier-dom.ts`, que `shell-page.ts` ne suit pas et dont son propre
en-tête déclare le manque comme un legs. Le câblage DOM vit à côté, dans
`bureau/porteur-dom.ts`.

**Repli quand `navigator.locks` n'existe pas** : l'onglet tente l'ouverture ; si
la plateforme répond `type:'error'` avec le motif d'appariement, il **bascule en
non-porteur silencieusement**. Tout autre motif de refus reste affiché — le
frein de volume (`trop-de-requetes`, `retryApresS`) doit continuer de se voir.

⚠️ **Le nom du verrou porte le préfixe de VM** (`prefixe.ts::lirePrefixe`,
`composer`), comme le nom de session. Sans lui, deux VMs différentes dans deux
onglets s'excluraient l'une l'autre — le défaut exact que P3 a corrigé sur le
nom de session, réintroduit par la porte de derrière.

## 4. Ce que le canal entre onglets transporte

`BroadcastChannel('nivuus-bureau-<préfixe>')` transporte **l'état, et rien
d'autre** : la liste des fenêtres connues, l'état du pont fichiers. Les onglets
non porteurs l'affichent à l'identique.

🔴 **AUCUN ORDRE D'OUVERTURE N'Y TRANSITE, ET C'EST LE POINT DE CONCEPTION.**
`window.open` exige une activation utilisateur **dans l'onglet qui a le geste** ;
faire relayer un clic de l'onglet B vers le porteur A ferait ouvrir A hors
activation, donc bloqué par le navigateur. Ce serait déplacer le mur d'un cran —
la faute que `hub/bureau.ts` a explicitement refusé de commettre. ~~**Chaque
onglet ouvre ses propres fenêtres depuis ses propres clics.**~~

> ⚠️ **CORRECTION DATÉE DU 31 AOÛT 2026, revue finale — cette dernière phrase
> est VRAIE de « Rouvrir » et FAUSSE de « Lancer », qui est pourtant le geste
> PRINCIPAL.** Elle est barrée plutôt qu'effacée, comme ce dépôt le fait
> partout ailleurs : une spec qui affirme faux se corrige par une note datée,
> elle ne se réécrit pas en silence.
>
> `POST /application/:id/lancer` est une **route HTTP**, que n'importe quel
> onglet peut appeler ; c'est ensuite l'AGENT qui annonce `fenetre-ouverte` sur
> la session de contrôle, donc **au porteur seul**, lequel fait `window.open`
> **hors activation utilisateur**. Deux effets, tous deux mesurables par un
> humain et par personne d'autre : ① la pop-up sort d'un **autre onglet** que
> celui où l'on a cliqué ; ② si le bloqueur de pop-ups intervient, le message
> d'échec s'affiche **dans l'onglet que l'utilisateur ne regarde pas**.
>
> 🔴 **DÉCISION DU PROPRIÉTAIRE DU DÉPÔT : « Lancer » N'EST PAS DÉSACTIVÉ SUR
> UN SUIVEUR.** `POST /lancer` ne porte **aucun rôle exclusif** et fonctionne
> parfaitement depuis n'importe quel onglet ; le désactiver priverait
> l'utilisateur d'une fonction qui marche, pour une gêne d'ergonomie. Ce qui
> est livré à la place est **une ligne d'état** dans `#statut` du suiveur
> (« Bureau tenu par un autre onglet. », `bureau/porteur-dom.ts`), pour que le
> comportement cesse d'être inexplicable.
>
> 🔵 **LE POINT DE CONCEPTION DU §4 N'EST PAS TOUCHÉ** : aucun **ordre**
> d'ouverture ne transite par le `BroadcastChannel`. La demande d'état ajoutée
> par cette même revue (`porteur.ts::batirDemande`) est une demande de
> **diffusion**, jamais d'ouverture — aucune activation utilisateur n'est en
> jeu.

Le repli déjà livré et déjà testé est conservé **mot pour mot** : la carte de
fenêtre et son bouton « Rouvrir » (`shell.ts::rouvrir`), qui est un geste. Il
devient **strictement meilleur qu'aujourd'hui** : la liste vit désormais sur la
page où l'on a cliqué « Lancer », au lieu d'une autre fenêtre qu'il fallait
penser à ouvrir.

### 4.1 La disposition — révélation progressive

**Décision du propriétaire.** Le catalogue reste le sujet de la page ; ce que le
hub absorbe ne le concurrence pas :

```
┌─ Applications ──────────────────┐
│ [Notepad] [Paint] [Chrome] …    │
│ ┌─ Installer un logiciel ─────┐ │
│ └─────────────────────────────┘ │
│ Mes fenêtres (2)                │  ← la section est ABSENTE si la liste est vide
│  • Bloc-notes      [Rouvrir]    │
│  • Paint           ouverte      │
│ ▸ Mes fichiers                  │  ← repliée ; se déplie d'elle-même si un pont est monté
└─────────────────────────────────┘
```

⚠️ **« Absente » et non « vide »** : une section « Mes fenêtres » montrant en
permanence « aucune fenêtre ouverte » serait du bruit sur l'état NOMINAL d'un
hub qu'on vient d'ouvrir.

⚠️ **« Mes fichiers » est repliée, jamais retirée** : le bouton
« Choisir mon dossier » doit rester atteignable en un geste, `showDirectoryPicker`
exigeant une activation utilisateur transitoire — un dépliage suivi d'un clic
reste deux gestes distincts, ce qui est licite. Elle se déplie d'elle-même quand
un pont est monté, pour que le compteur d'écritures dues et le bouton
« Reprendre l'enregistrement » ne soient jamais cachés derrière un pli au moment
où ils comptent.

🔴 **Les crochets d'instrument survivent au dépliage** : `data-dues`,
`data-vues` et `data-retenues` restent portés par les mêmes éléments, aux mêmes
noms. Les pilotes de recette lisent ces attributs et **jamais le texte** — piège
de F1, payé neuf minutes sur deux messages qui partageaient une sous-chaîne. Un
élément replié reste dans le DOM et reste lisible ; un élément **retiré** ne
l'est pas, et c'est pourquoi le pli est un pli et non un rendu conditionnel.

## 5. La fraîcheur du jeton — `jeton.ts`

Un seul point d'entrée neuf remplace `assurerAcces` (qui n'a qu'un appelant) :

```ts
assurerAccesFrais(coffre, base, appel, maintenant, margeMs): Promise<string | undefined>
```

Dans cet ordre, et chaque étape n'est tentée que si la précédente échoue :

1. le coffre porte un jeton **et** `!expireAvant(jeton, maintenant + margeMs)` →
   le rendre, **sans aucun réseau** ;
2. `rafraichirSiNecessaire` — le chemin du mode `motdepasse`, **déjà écrit,
   déjà testé, aujourd'hui appelé par personne** ;
3. `accesParPomerium` → `GET /auth/moi` — le mode `pomerium`, celui de la
   production ;
4. `undefined` → l'appelant redirige vers `connexion.html?suite=…`.

**Appelée avant chaque usage** : peuplement du catalogue, lancement, lecture
d'icône, et **ouverture du WebSocket**. Un hub laissé ouvert huit heures
continue de fonctionner ; un jeton périmé dans le coffre cesse d'être un état
dont on ne sort que par un rechargement à la main.

⚠️ **`margeMs` est une constante NON CALIBRÉE, et elle est déclarée comme
telle** — comme les quarante autres que ce dépôt n'a jamais calibrées. Elle vaut
`30_000` : assez pour qu'un appel parti avec un jeton valide n'arrive pas
expiré, sans forcer un aller-retour à chaque geste.

🔴 **CE QUE CETTE RÈGLE NE FAIT PAS** : elle ne vérifie **aucune signature**. Le
navigateur n'a pas le secret. `expireAvant` le dit déjà dans son propre
commentaire, et cette spec ne le contredit pas : ce qu'on évite ici est un
aller-retour inutile et un échec inexpliqué, jamais une décision d'autorisation
— celle-ci reste au service, sur chaque poignée de main.

## 6. Le découpage des modules — payé AVANT l'addition

`hub/page.ts` est à **373** lignes et doit absorber deux sections ;
`shell-page.ts` est à **500 EXACTEMENT**, donc à sa porte. La règle du dépôt est
« extraire, jamais comprimer », **dans une tâche dédiée jouée avant celle qui
ajoute**.

| Fichier | État | Rôle |
| --- | --- | --- |
| `hub/cartes.ts` | **neuf**, extrait de `page.ts` | le balisage d'une carte d'application |
| `bureau/porteur.ts` | **neuf**, pur, testé | élection Web Locks, diffusion d'état |
| `bureau/porteur-dom.ts` | **neuf** | le câblage : socket, `BroadcastChannel`, DOM |
| `bureau/fenetres-dom.ts` | **neuf**, repris de `shell-page.ts` | la liste « Mes fenêtres » |
| `bureau/fichiers-dom.ts` | **neuf**, repris de `shell-page.ts` | le pont ProjFS et ses quatre boutons |
| `shell.ts` (381) | **INCHANGÉ** | la règle du bureau est déjà pure et testée : elle est réutilisée telle quelle, pas réécrite |
| `shell-page.ts` (500) | **vidé** | devient la redirection du §7 |

🔴 **Les tailles ci-dessus sont un RELEVÉ DU 31 AOÛT 2026, pas une source de
vérité.** La commande de `CLAUDE.md` est relancée **après la dernière édition de
la ronde**, revue transverse comprise, et le tableau de dette corrigé dans le
même mouvement.

## 7. `shell.html`, `shell.css`, et le contrôle des classes

`shell.html` devient une page de redirection **sans aucune UI** :

```js
location.replace('/' + location.search);
```

Son balisage — les deux sections, le `<template id="modele-fenetre">` — **migre
dans `hub.html`**, et `shell.css` migre dans `hub.css` **dans le même
mouvement**.

🔴 **LES DEUX VONT ENSEMBLE, SOUS PEINE D'UN CONTRÔLE VERT SUR UN PRODUIT
CASSÉ.** `client/outils/classes-employees.mjs` porte
`SURFACES_PRODUIT = ['client/index.html', 'client/shell.html',
'client/connexion.html']` — **`hub.html` n'y figure pas**. Vider `shell.html`
sans toucher cette liste rendrait toutes les classes de `shell.css` orphelines,
et laisserait le hub **hors de tout contrôle §7.9**. La liste devient
`['client/index.html', 'client/hub.html', 'client/connexion.html']` dans le
même commit.

⚠️ **`shell.html` reste dans `rollupOptions.input` de `vite.config.ts`** : une
page absente de cette liste ne sort pas du build **et rien ne le dit** (relevé
du 19 août 2026, cité dans le fichier lui-même). Une redirection non bâtie
serait un 404 pour toute PWA installée.

## 8. Le manifeste PWA — `id` et `start_url` divergent désormais

```
id:        ${base}/shell.html?app=${app}    ← INCHANGÉ
start_url: ${base}/?app=${app}              ← migre vers la racine
```

🔴 **`id` PORTE L'IDENTITÉ, ET LE CHANGER N'EST PAS UNE MISE À JOUR : C'EST UNE
SECONDE APPLICATION, LA PREMIÈRE DEVENANT ORPHELINE.** `manifeste.ts` dit déjà
que `scope` est partagé et que c'est `id` qui distingue les applications entre
elles ; cette spec ajoute la raison de ne pas y toucher.

🔵 **Et c'est ce qui justifie la redirection PERMANENTE du §7** : le manifeste
étant publié en `blob:`, une PWA installée ne le relit **jamais** — legs G5,
déclaré. Elle ouvrira `shell.html` pour toujours. La redirection n'est pas une
transition, c'est le chemin définitif de ces installations-là.

`start_url` reste dans le `scope` (`${base}/`), condition que Chromium vérifie.
`file_handlers[].action` pointe **déjà** `hub.html?app=` — cette divergence
existante disparaît d'elle-même.

`connexion.ts` voit son défaut changer : `params.get('suite') ?? 'shell.html'`
devient `?? '/'`.

## 9. Les contrôles

| Ce qu'on éprouve | Comment |
| --- | --- |
| L'élection | tests de `porteur.ts` : un porteur ouvre le socket ; un second onglet ne l'ouvre pas ; la libération du verrou promeut l'attente ; le repli sans `navigator.locks` bascule sans afficher d'erreur ; un refus d'une AUTRE cause reste affiché |
| La fraîcheur | tests de `jeton.ts` : jeton frais → **aucun appel réseau** ; jeton périmé → rafraîchissement ; sans rafraîchissement → `/auth/moi` ; les deux échouent → `undefined` **et coffre vidé** |
| Le manifeste | test figeant `id !== start_url`, et `start_url` dans le `scope` |
| La redirection | test sur la règle pure de composition de l'URL de suite (paramètres conservés) |
| Le non-régressé | `cd client && npx vitest run`, **puis** `npx vitest run --dir ../proto` (deux commandes, jamais une), `tsc --noEmit`, `env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh` |

🔴 **CHAQUE CONTRÔLE NEUF EST VU ROUGE AVANT D'ÊTRE VU VERT**, par mutation
ciblée du produit, restaurée depuis une **copie nommée** — jamais par
`git checkout --`, qui restaure HEAD et a déjà effacé du travail non commité
dans ce dépôt.

## 10. Ce que cette conception n'établit PAS

- 🔴 **AUCUNE RECETTE NAVIGATEUR NE SERA JOUÉE.** Le rôle `client` est exclusif
  par session et le propriétaire est connecté : un pilote lui prendrait sa
  place. Les contrôles sont `vitest`, `tsc --noEmit` et `verify-all.sh`. **Le
  jugement d'usage appartient au propriétaire**, et rien ici ne le remplace.
- ⚠️ **La reconnexion du WebSocket après une coupure réseau reste DUE** — legs
  déclaré du lot 34, `shell.ts::canalDeControlePerdu` affiche « Rechargez la
  page ». L'élection ne couvre que la **fermeture d'un onglet**, pas la perte du
  socket d'un onglet vivant.
- ⚠️ **Le legs « une fenêtre `Vivante` n'est jamais redite à une page-shell qui
  arrive » n'est PAS fermé.** Le canal entre onglets le contourne entre onglets
  **vivants** ; après un rechargement complet, la liste est de nouveau vide.
  C'est une décision du propriétaire, dossier au § 8 des résultats du lot 34.
- ⚠️ **L'installabilité du hub reste non établie**, et `/hub.webmanifest` ne
  déclare toujours aucun `icons`.
- ⚠️ **Le comportement de plus de deux onglets n'est pas mesuré** : Web Locks
  garantit la file, ce lot ne l'éprouve que par ses tests, jamais sur un vrai
  navigateur avec trois onglets.
