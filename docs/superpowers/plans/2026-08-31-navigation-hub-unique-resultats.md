# Navigation : le hub devient la SEULE surface — résultats

**31 août 2026.** Chantier `navigation-hub-unique`, branche `package-nivuus`.
Onze tâches, **dix-neuf commits**, de `412e989` **exclu** à `57f622a` — plus le
commit de clôture qui porte ce document.

> 🔴 **CE DOCUMENT EST VERSIONNÉ, ET C'EST LE POINT.** Ce dépôt a perdu **six
> constats de revue** avec un espace de travail gitignoré ; il a aussi constaté
> deux fois qu'un legs qui ne vit que dans un relevé daté est un legs perdu.
> Tout ce qui devait survivre à ce chantier est ici.
>
> ⚠️ **Aucun chiffre de ce document ne se recopie sans relancer sa commande.**
> Il est **daté**, comme tous ses voisins.

---

## 1. La demande, verbatim

> « Il faut améliorer la navigation : on devrait jamais avoir à aller sur
> shell.html. On peut pas supprimer shell.html ? Et surtout si je vais sur
> hub.html, ça valide et rafraîchis ma connexion »

Deux défauts distincts, et les confondre aurait été une erreur : ① l'utilisateur
devait connaître une **seconde surface** ; ② `assurerAcces` **ne regardait pas
si le jeton était périmé**. La conception est dans
[`docs/superpowers/specs/2026-08-31-navigation-hub-unique-design.md`](../specs/2026-08-31-navigation-hub-unique-design.md),
le plan dans [`2026-08-31-navigation-hub-unique.md`](2026-08-31-navigation-hub-unique.md),
et le journal de bord — décisions, `Ruling:`, constats différés — dans
`.superpowers/sdd/2026-08-31-navigation-hub-unique/progress.md`.

## 2. Ce qui a été fait, tâche par tâche

| # | Ce qu'elle a livré | Commits |
| --- | --- | --- |
| **0** | Le plan, puis son **amendement par le scan de pré-vol** — rulings R2 (aucun `jsdom`) et R3 (la classe conditionnelle en deux littéraux) | `f936b57`, `d7dcbb4` |
| **1** | `jeton.ts::assurerAccesFrais` — quatre étapes, chacune tentée seulement si la précédente échoue ; branchement du seul appelant | `38fa9f2`, `c449da2` |
| **2** | **Extraction préalable** `hub/cartes.ts`, jouée **AVANT** l'addition qu'elle préparait | `7c7322d` |
| **3** | Le refus d'appariement porte un **motif typé** `role-occupe`, côté plateforme | `4fae1d2` |
| **4** | `bureau/porteur.ts` — l'élection Web Locks, **pure et testée** | `a830a48` |
| **5** | `hub.html` absorbe le balisage du bureau ; `hub.html` entre dans `SURFACES_PRODUIT` | `2d763d1` |
| **6** | La liste des fenêtres : `bureau/fenetres.ts` (RÈGLE, pure) + `fenetres-dom.ts` (câblage) | `573a87a`, `afbcf11` |
| **7** | `bureau/fichiers-dom.ts` — le pont fichiers, extrait **VERBATIM** | `8658a1e`, `e676e6d` |
| **8** | **Le hub devient porteur** : `bureau/porteur-dom.ts`, retrait du lien « Mon bureau », suppression de `hub/bureau.ts` | `add2319`, `83b86a4` |
| **9** | `shell.html` devient une **redirection permanente** ; `shell.css` → `bureau/bureau.css` | `74fa8b3`, `8d0fcb5` |
| **10** | Le manifeste : `start_url` migre vers la racine, **`id` reste figé** ; balayage des commentaires | `a100e46`, `fc70199`, `57f622a` |
| **11** | Cette revue transverse, ce document, et la ligne d'index | *ce commit* |

**Portée mesurée du chantier** (`git diff --stat 412e989..HEAD`, relancé le
31 août 2026) : **37 fichiers**, **4 534 insertions**, **1 021 suppressions** —
dont **11 fichiers créés**, **2 supprimés** (`hub/bureau.ts` et son test) et
**1 renommé** (`client/src/shell.css` → `client/src/bureau/bureau.css`, R077).

## 3. Les rouges VUES — et **laquelle** a rougi

🔴 **« Vue rouge » ne suffit pas : une rouge qui rougit pour la mauvaise raison
ne prouve rien, et elle est indiscernable d'une bonne si l'on ne lit que son
code de sortie.** Chaque rouge ci-dessous est nommée par **l'assertion qui a
échoué**, telle que le journal de bord la porte.

| Tâche | La mutation | L'assertion qui a rougi, et son message |
| --- | --- | --- |
| **1** | `MARGE_FRAICHEUR_MS` mutée | le compteur `appels` — **`expected 1 to be +0`** : un jeton frais avait déclenché un aller-retour réseau, ce que l'étape ① existe pour éviter |
| **4** | le repli d'élection élargi | **« ne reconnaît PAS un refus d'une autre cause »** — `expected true to be false` : le frein de volume (`trop-de-requetes`) doit rester **visible**, seul le motif d'appariement bascule en suiveur |
| **5** | *(aucune mutation)* — 🔵 **le contrôle §7.9 a rougi TOUT SEUL**, en découvrant le hub : `hub__section` / `hub__section-titre` **employées et non déclarées**. C'est exactement ce que l'ajout de `hub.html` à `SURFACES_PRODUIT` devait révéler. **Corrigé en déclarant les classes, jamais en assouplissant le contrôle.** | |
| **6** | la condition de visibilité de la section | **`expect(sectionVisible([])).toBe(false)`** — `expected true to be false` : une section « Mes fenêtres » vide est **absente**, pas vide |
| **8** | `installerLeBureau` débranché | `parcours.test.ts` ① — la revue a établi **statiquement** que le test réécrit peut encore échouer, et pour la bonne raison |

⚠️ **Ce que cette table ne prétend pas** : les tâches 2, 3, 7, 9 et 10 sont des
extractions, des transcriptions ou des corrections de commentaire. Elles n'ont
**introduit aucune rouge neuve**, et sont gardées par `tsc --noEmit`, la suite
existante et `design:verifier` — c'est dit ici plutôt que masqué par un tableau
qui aurait l'air complet.

## 4. 🔴 Les QUATRE défauts que les revues ont trouvés — et qui venaient TOUS du PLAN

**C'est le constat le plus utile du chantier.** La boucle de revue a servi, et
elle a servi **contre le plan**, pas contre les implémenteurs : dans les quatre
cas, l'implémenteur avait reproduit exactement ce que mon brief lui donnait.

| # | Le défaut | Ce qu'il aurait coûté |
| --- | --- | --- |
| **①** | **Le callback de rafraîchissement sans `try/catch` ni validation de forme** (tâche 1). Le chemin jumeau `/auth/moi` a les deux — `accesParPomerium` un `try/catch`, `accesDeReponse` une validation — et mon brief prescrivait ni l'un ni l'autre. **Aggravant** : cette tâche est le **PREMIER appelant de production** de `rafraichirSiNecessaire`, et le chemin est vivant en mode `motdepasse`. | Une panne réseau (hors ligne, DNS, CORS) serait remontée en exception non gérée jusqu'à un `void demarrer()`. Remède : garde + `paireDeReponse` réutilisant `accesDeReponse`, **5 tests neufs**. |
| **②** | **Les classes de pastille rendues INVISIBLES au contrôle §7.9** (tâche 6). Mon brief prescrivait `classList.add(variable)` ; `design/classes.ts::classesEmployeesTs` ne reconnaît **que** `classList.add('<littéral>')`, et son propre commentaire le dit. `bureau__pastille--ouverte/--fermee` ne subsistaient plus que dans un type, un ternaire et un test. | Deux classes déclarées en CSS et **orphelines sans que rien ne le dise** — l'extraction les avait rendues telles en silence. Remède : la règle rend un **booléen**, le câblage fait le `if/else` avec **deux littéraux**, et le contrôle ajouté (`grep` rendant DEUX lignes) empêche que le vert soit vacueux. |
| **③** | **Un bloc de commentaire PARAPHRASÉ au lieu d'être déplacé verbatim** (tâche 7). Mon brief donnait un texte réécrit du bloc `showDirectoryPicker()` ; **deux clauses ont été perdues** — « et jamais depuis un message de canal » (le POURQUOI de la contrainte) et la phrase entière « Même contrainte que `window.open()`, que cette page connaît déjà ». Mon brief se trompait aussi de **famille de marqueur** (« cinq blocs 🔴 » là où le code en porte quatre 🔴 et deux ⚠️). | Le savoir qui justifie une contrainte d'activation utilisateur, effacé par une extraction qui se déclarait « verbatim ». Remède : texte récupéré par `git show afbcf11:` et recopié intégralement ; **aucune ligne de code touchée** par le correctif. |
| **④** | **Un littéral de couleur `'#000'` dans des tests** (tâche 10). Les cinq tests que je prescrivais employaient `'#000'` comme couleur de fond — **ce que le contrôle §7.2 interdit**, car il balaie aussi les `.ts`. | Le contrôle §7.2 aurait rougi sur du code écrit par le plan lui-même. Trouvé et corrigé **par l'implémenteur**, en réutilisant la constante `FOND` du fichier. |

🔵 **La leçon générale, et elle est neuve :** ce dépôt écrit déjà qu'« un plan
n'immunise pas contre le patron du contrôle qui ne peut pas échouer : il en est
une source ». Ce chantier l'établit une seconde fois, et **par un mécanisme
différent** : le plan est écrit **avant** d'avoir relu les conventions qu'il
enfreint, et un implémenteur consciencieux les enfreint **fidèlement**. La
parade qui a fonctionné n'est pas une meilleure rédaction du plan, c'est la
**revue par un tiers qui relit le code contre les conventions**, pas contre le
brief.

## 5. La revue transverse — ce qu'elle a mesuré

### 5.1 Les tailles, relevées APRÈS la dernière édition de la ronde

🔴 **Relevé après ma propre dernière édition de source, pas avant** — une table
mesurée en début de ronde est fausse à la fin de la même ronde. Commande de
`CLAUDE.md`, **relancée** :

```
630 agent/src/windows_source.rs
```

**C'est la seule ligne.** Le tableau de dette de `CLAUDE.md` est donc **exact**
et n'a demandé aucune correction.

⚠️ **UNE PRÉMISSE DU CAHIER DES CHARGES DE CETTE TÂCHE ÉTAIT FAUSSE, ET JE LA
CORRIGE PLUTÔT QUE DE L'APPLIQUER.** Il annonçait que « `client/src/shell-page.ts`
figurait au tableau de dette à 500 lignes ». **Il n'y a jamais figuré** :
`grep -c 'shell-page' CLAUDE.md` rend **0**. Ce qui est vrai, et mesuré :

| | à la base `412e989` | aujourd'hui |
| --- | --- | --- |
| `client/src/shell-page.ts` | **500** — le plafond EXACT, donc à sa porte | **16** |

Le fichier ne portait pas de dette *inscrite*, il portait une dette *réelle* que
`CLAUDE.md` n'avait jamais enregistrée. Il n'y a rien à retirer d'un tableau ;
il y a un fait à consigner, et il l'est ici.

**Ce qui reste à surveiller** (relevé daté du 31 août 2026, **jamais une source
de vérité** — la source est la commande, relancée) : `agent/src/encode/arret.rs`
reste à **500 pile**, et `client/src/main.ts` est retombé à **462** après
l'extraction du lot 33. **La marge regagnée par une extraction se reperd si on
la traite comme acquise** — payé six fois par ce dépôt.

### 5.2 Les affirmations devenues fausses — **10 corrigées**

**La tâche 10 en avait corrigé 12**, par un `grep` sur
`shell.html|page-shell|shell-page`. J'ai cherché **par le SENS**, et j'ai trouvé
ce que cette formule ne pouvait pas voir. **Énumérées avant d'écrire, relues une
par une après.**

| # | Où | Ce qui était faux |
| --- | --- | --- |
| **1** | `client/src/jeton.ts` (doc de `poserAcces`) | 🔴 **La plus grave.** Affirmait que `rafraichirSiNecessaire` « n'a **aucun appelant de production** », que le `grep` cité « rend **CINQ** lignes — une définition et quatre usages de test, pas un appel », et que le seul chemin vers `/auth/moi` était un **rechargement à la main**. **Les trois ont été rendues fausses par la tâche 1 elle-même.** La commande citée, relancée, rend **SIX** lignes, dont `jeton.ts:261` — **un appel de production**. 🔵 Aucune formule sur `shell*` ne pouvait l'attraper : le mot « shell » n'y figure pas. |
| **2** | `client/outils/classes-employees.mjs` | « `shell-page.ts`, **trois lignes** » — **mesuré : 16**. Le naufrage du 487, dans un fichier écrit le jour même. |
| **3** | `client/outils/surfaces-baties.mjs` | la **même** affirmation, dans un second fichier — c'est pourquoi « corriger là où on nous l'a montrée » ne suffit jamais. |
| **4** | `client/src/hub/hub.css` (en-tête) | inventaire **au présent** : « `shell.css` pour le bureau ». Le fichier n'existe plus sous ce nom, et « quatre **surfaces** » a cessé d'être vrai le jour où le hub a absorbé le bureau. |
| **5** | `client/src/connexion.css` (en-tête) | même inventaire au présent. |
| **6** | `client/src/connexion.css` | « `shell-page.ts` y redirige tout visiteur sans jeton » — **c'est `hub/page.ts` depuis la tâche 8**. ⚠️ La tâche 10 ne pouvait pas la voir : son balayage n'a touché que des `.ts`. |
| **7** | `client/src/connexion.css` | pointeur « énuméré dans l'en-tête de `shell.css` » vers un fichier renommé. |
| **8** | `client/src/connexion.css` | comparaison **au présent** avec une règle « de `shell.css` ». |
| **9** | `client/src/design/primitives/champ.css` | « la pastille d'état de la page-shell **le consomme** (`client/src/shell.css`) » — verbe au présent, chemin mort. |
| **10** | `CLAUDE.md` | 🔴 **« LA RACINE `/` SERT LA PAGE DE SESSION … Décision non prise (servir le hub, ou rediriger) — elle appartient au propriétaire. »** **PÉRIMÉ**, et pas par ce chantier : par le **lot 14**, qui a posé `const PAGE = 'hub.html'` dans `plateforme/src/http/page/resolution.ts`. **C'est le patron du legs `403/404`, payé une TROISIÈME fois** — un legs consolidé à la main qui affirme une question ouverte que le produit avait déjà tranchée. |

**Laissées, et pourquoi** — toutes ne sont pas des erreurs : ce dépôt écrit son
histoire au passé, et c'est légitime.

| Ce qui reste | La raison de le laisser |
| --- | --- |
| `client/src/bureau/bureau.css:75-76` — « employé par `client/src/shell.css` » | 🔴 **C'est une TRANSCRIPTION VERBATIM d'une sortie d'outil passée.** La corriger **fabriquerait une pièce** : le dépôt l'interdit nommément. |
| `client/outils/tokens-orphelins/attente.mjs:156` | relevé **daté du 20 août 2026**, exact à sa date, et il le dit lui-même. |
| `client/src/design/longueurs.test.ts:52` | mesure datée réfutant le plan de S4 ; ⚠️ **vérifié** que la portée du contrôle est **dérivée par glob** (`import.meta.glob`), donc le mécanisme n'est pas touché par le renommage — seule la prose nomme l'ancien chemin. |
| `client/src/design/tokens/echelles.css:123`, `primitives/message.css:40`, `connexion.css:41` | énoncés **datés** de S3/S4, au passé (« le sous-bloc S3 en avait besoin », « leçon payée à la tâche 4 de S3 »). |
| `bureau.css:2`, `hub.css:191`, `hub.html:16` | **déjà qualifiés** par la date du renommage — le travail de la tâche 9. |
| ~40 mentions de `shell-page.ts` | **déjà qualifiées** par la tâche 10 (« avant que le hub ne devienne la seule surface »). |
| `jeton.test.ts:265-280`, `parcours.test.ts:8 et 111`, `porteur.ts:8`, `hub/page.ts:292` | récits de rouge et notes d'histoire, **au passé et datés**. |
| les `docs/superpowers/specs/*` | documents datés : un plan et une conception décrivent ce qu'on allait faire, pas ce qui est. |

### 5.3 La suite complète, depuis un shell PROPRE

```
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

🔴 **Le `env -u` n'est pas décoratif** : avec `TURN_URL`/`TURN_SECRET` dans
l'environnement, six tests de signaling échouent — `verify-all.sh` n'est **pas
hermétique**.

**Verdict : `Les 10 étapes sont passées.`** (code de sortie **0**).

⚠️ **J'ANNONCE LES ÉTAPES, PAS LES EN-TÊTES**, et je dis lequel : le script en
compte **dix**, l'exécution affiche **18** en-têtes `==>` (`grep -c '^==>'`, mesuré
— les huit de plus viennent de `client : npm run design:verifier`). **Les deux
sont vrais de choses différentes.**

| Étape | Ce qu'elle a rendu |
| --- | --- |
| `cargo test --workspace` | **1 143** + **114** passés, **0** échec |
| `cargo clippy --workspace` | passé |
| `client : npm test` | **58** fichiers, **640** tests |
| `client : npm run typecheck` | passé |
| `client : npm run design:verifier` (8 contrôles) | **0 écart** partout ; §7.10 : **6** feuilles de surface ; §7.3 : **6** pages bâties — `connexion.html, design.html, hub.html, index.html, primitives.html, shell.html` |
| `npm run build` | passé — 🔵 **`shell.html` EST BIEN DANS LE BUILD** : une redirection non bâtie serait un 404 pour toute PWA installée |
| `proto : npm test` | **9** fichiers, **308** tests |
| `proto : npm run typecheck` | passé |
| `plateforme : npm run test:sqlite` | **67** fichiers, **750** tests |
| `plateforme : npm run test:postgres` | **67** fichiers, **750** tests |
| `plateforme : npm run typecheck` | passé |

## 6. 🔴 Ce que ce lot n'établit PAS

**Le §10 de la spec, repris intégralement, PLUS ce que les revues ont découvert
en chemin.**

### 6.1 Repris du §10 de la conception

- 🔴 **AUCUNE RECETTE NAVIGATEUR N'A ÉTÉ JOUÉE.** Le rôle `client` est **exclusif
  par session** (`plateforme/src/signaling/appariement.ts::declarer`) et le
  propriétaire est connecté : **un pilote lui prendrait sa place**. Les contrôles
  joués sont `vitest`, `tsc --noEmit` et `verify-all.sh`, et rien d'autre.
  **Le jugement d'usage appartient au propriétaire, et rien ici ne le remplace.**
- 🔴 **AUCUN HUMAIN N'A OUVERT LA PAGE.** Ni l'élection à deux onglets, ni la
  révélation progressive des deux sections, ni la redirection de `shell.html`
  dans un navigateur réel n'ont été **regardées**. C'est la même limite que
  celle qui pèse sur ⑥ d'un bout à l'autre (« vingt-cinq jugements humains
  attendent un œil »).
- ⚠️ **LE COMPORTEMENT DE PLUS DE DEUX ONGLETS N'EST MESURÉ PAR RIEN.** Web Locks
  garantit la file ; ce lot ne l'éprouve **que par ses tests**, jamais sur un
  vrai navigateur avec trois onglets. La promotion en cascade — le porteur
  meurt, le premier attendant prend, il meurt, le second prend — n'a **jamais
  couru**.
- ⚠️ **LA RECONNEXION DU WEBSOCKET APRÈS UNE COUPURE RÉSEAU RESTE DUE** — legs
  déclaré du **lot 34**. `shell.ts::canalDeControlePerdu` affiche « Rechargez la
  page ». **L'élection ne couvre que la FERMETURE D'UN ONGLET**, pas la perte du
  socket d'un onglet vivant : un porteur dont le socket tombe reste porteur, tient
  le verrou, et **empêche un onglet sain de prendre sa place**.
- ⚠️ **LE LEGS « une fenêtre `Vivante` n'est jamais redite à une page-shell qui
  arrive » N'EST PAS FERMÉ.** Le canal entre onglets le contourne entre onglets
  **vivants** ; après un **rechargement complet**, la liste est de nouveau vide.
  Décision du propriétaire, dossier au § 8 des résultats du lot 34.
- ⚠️ **L'INSTALLABILITÉ DU HUB RESTE NON ÉTABLIE**, et `/hub.webmanifest` ne
  déclare toujours **aucun** `icons`.

### 6.2 Ce que les revues ont découvert EN PLUS

- 🔴 **LA JONCTION « le porteur meurt, un suiveur est promu » N'EST ÉPROUVÉE QUE
  PAR SES TESTS.** Le test exerce la promesse Web Locks par une fausse
  implémentation ; la revue de la tâche 4 a tracé à la main que la promesse
  « ne se résout jamais » et que l'élection ne peut pas passer par un **artefact
  d'ordonnancement**. C'est une vérification **statique**, pas une mesure.
- ⚠️ **LE PONT FICHIERS SUIT L'ÉLECTION DEPUIS LA TÂCHE 8, ET CE CHEMIN N'A
  JAMAIS COURU.** La revue a établi que le pont ouvre **son propre socket** sur
  une session fixe par VM, avec le rôle `client`, lui aussi exclusif — donc qu'il
  devait suivre le porteur. La conséquence livrée (le pont s'installe dans
  `devenirPorteur`, **donc aussi à une promotion tardive** ; sur un suiveur,
  bouton désactivé + texte d'état) est **testée, jamais exercée**.
- ⚠️ **`MODULES[cle].includes('fenetre-ouverte')` de `parcours.test.ts` CHERCHE
  UNE SOUS-CHAÎNE DANS LE TEXTE SOURCE, COMMENTAIRES COMPRIS.** Un futur
  commentaire portant ce mot rendrait ce garde **vert sans traitement**.
  **Faiblesse PRÉEXISTANTE, pas une régression** de ce chantier — dite ici plutôt
  que découverte par le suivant ; elle mériterait un ancrage sur la syntaxe.
- ⚠️ **SUR `trop-de-requetes`, LE SERVEUR FERME LE SOCKET AUSSITÔT ET
  `canalDeControlePerdu` ÉCRASE LE BANDEAU PORTANT `retryApresS`.** Comportement
  **HÉRITÉ** de `shell-page.ts`, pas une régression — mais il vide en pratique la
  distinction que la spec §3 pose entre « le motif d'appariement bascule en
  suiveur » et « tout autre motif reste **affiché** ».

## 7. Les legs neufs — **nommés**

- 🔴 **`.bureau` EST CONSERVÉE ARTIFICIELLEMENT DANS `bureau/bureau.css`, ET
  C'EST LE SECOND CAS DU MÊME ANGLE MORT QUE `--accent-fenetre`.** Plus aucune
  classe ne l'emploie (le hub porte son propre conteneur, `.hub`), mais sa seule
  déclaration — `margin-block: var(--e-7)` — est **l'UNIQUE emploi de `--e-7`
  dans tout le dépôt** (vérifié : `grep -rn -- '--e-7\b' client/src` ne rend que
  la déclaration du token, l'énumération de `galerie.test.ts`, et cette règle).
  La retirer ferait tomber `--e-7` en orphelin et **échouer le contrôle §7.6**.
  🔴 **Le mécanisme est exactement celui de `--accent-fenetre` : le contrôle
  compte un `var(--…)` que PERSONNE NE PEINT comme un « emploi ».** La règle du
  dépôt dit qu'un piège payé deux fois remonte : **il est inscrit dans
  `CLAUDE.md`**, aux pièges transverses.
- 🔴 **UN ONGLET SUIVEUR QUI ROUVRE UNE FENÊTRE NE PRÉVIENT PAS LE PORTEUR**,
  dont la liste dira « fermée » **en permanence**. Chaque onglet ouvre ses
  propres fenêtres depuis ses propres clics — **aucun ordre d'ouverture ne
  transite par le canal**, et c'est le point de conception du §4 : `window.open`
  exige une activation utilisateur **dans l'onglet qui a le geste**. Le handle
  `window` du porteur reste donc fermé, et **rien ne le rouvre**. ⚠️ **Limite de
  conception assumée, déjà écrite dans le code** (`bureau/porteur-dom.ts`) —
  jamais un oubli.
- ⚠️ **UNE EXCLUSION NOMMÉE A ÉTÉ AJOUTÉE AU CONTRÔLE §7.3** — `EXCLUS_DE_A` dans
  `client/outils/surfaces-baties.mjs` — parce que `shell.html`, devenue une
  redirection, **ne lie plus aucune feuille, à dessein**. 🔵 **La revue l'a
  validée sur quatre critères** : nommée et bornée à `shell.html` seul, justifiée
  dans le code, le contrôle **mord encore ailleurs**, et elle suit le patron
  existant d'`EXCLUS` dans `tokens-orphelins.mjs`. C'est la bonne réponse à un
  contrôle rendu **structurellement inapplicable** — « la raison pour laquelle un
  fichier NE PEUT PAS faire échouer le contrôle, jamais celle pour laquelle il
  gênerait ». ⚠️ Elle reste comptée dans `pages bâties` : **seule l'assertion A
  l'ignore**.
- ⚠️ **UN FICHIER `.impeccable/config.json` NON TRACKÉ SUBSISTE DANS L'ARBRE.**
  Posé à la tâche 2 pour taire un faux positif `broken-image` d'un hook de design
  **local**, sur le `<img>` **inerte** d'un `<template>` de `hub.html`. **Il n'est
  pas commité, et il n'est pas dans `.gitignore`** : il apparaît en `??` dans
  `git status`. **Le retirer ferait rougir le hook ; le commiter ajouterait au
  dépôt une configuration d'outil que le propriétaire n'a pas demandée.** 🔴 **La
  décision lui appartient**, et elle est offerte ici plutôt que prise en douce.

## 8. Ce qu'il faut retenir en une phrase

Le hub tient désormais la session de contrôle, montre ses fenêtres et son pont
fichiers, élit un porteur par Web Locks sans jamais montrer d'erreur aux autres
onglets, et vérifie la fraîcheur de son jeton **avant chaque usage** — et la
seule chose que personne n'a faite, c'est **l'ouvrir dans un navigateur**.
