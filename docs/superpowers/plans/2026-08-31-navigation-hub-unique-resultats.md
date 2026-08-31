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

**Portée mesurée du chantier** — 🔴 **LA PLAGE EST EXPLICITE, ET CE N'EST PAS UN
DÉTAIL** : `git diff --stat 412e989..57f622a`, relancé le 31 août 2026. **Écrire
`..HEAD` rendrait d'autres nombres** dès le commit suivant, et un lecteur qui
suit la consigne « relance la commande » obtiendrait un écart sans en comprendre
la cause — mesuré : contre `HEAD` après la clôture, la même commande rend
**41 fichiers, 4 910 insertions, 1 047 suppressions**. ⚠️ **Le commit de clôture
n'est donc PAS compté ci-dessous**, à dessein : ces chiffres sont ceux des dix
tâches d'implémentation.

**37 fichiers**, **4 534 insertions**, **1 021 suppressions** — dont **12 créés**
(11 de source sous `client/src/`, le douzième étant le plan lui-même sous
`docs/`), **2 supprimés** (`hub/bureau.ts` et son test) et **1 renommé**
(`client/src/shell.css` → `client/src/bureau/bureau.css`, R077).

⚠️ **« 11 fichiers créés » était écrit ici au premier jet**, et
`git diff --name-status | grep '^A' | nl` en compte **12** : le plan versionné
n'avait pas été compté. Corrigé en relançant, pas en relisant.

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
| **1** | `client/src/jeton.ts` (doc de `poserAcces`) | 🔴 **La plus grave.** Affirmait que `rafraichirSiNecessaire` « n'a **aucun appelant de production** », que le `grep` cité « rend **CINQ** lignes — une définition et quatre usages de test, pas un appel », et que le seul chemin vers `/auth/moi` était un **rechargement à la main**. **Les trois ont été rendues fausses par la tâche 1 elle-même.** La commande citée, relancée, rend **SIX** lignes, dont `jeton.ts:271` — **un appel de production**. ⚠️ **CE NUMÉRO A ÉTÉ FAUX DANS LA PREMIÈRE VERSION DE CE DOCUMENT (`261`), ET C'EST LE NAUFRAGE DU 487 COMMIS DANS LE PARAGRAPHE MÊME QUI EN CORRIGE UNE INSTANCE.** `261` est `    base: string,` — un paramètre de la signature d'`assurerAccesFrais` ; il venait de `jeton.test.ts:261`, qui est une **assertion de test**, c'est-à-dire exactement la catégorie que ce constat existe pour distinguer d'un appel de production. Trouvé par la revue de la tâche 11, corrigé en **relançant la commande** plutôt qu'en relisant. 🔵 Aucune formule sur `shell*` ne pouvait l'attraper : le mot « shell » n'y figure pas. |
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
| `jeton.test.ts:266-280`, `parcours.test.ts:8 et 111`, `porteur.ts:8`, `hub/page.ts:292` | récits de rouge et notes d'histoire, **au passé et datés**. |
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
  🔴 **ET C'EST UN PIÈGE POUR LE PROCHAIN QUI CLONE, dit ici plutôt que
  découvert** (revue finale, 31 août 2026) : un fichier **ni suivi ni ignoré**
  n'existe que dans CET arbre de travail. Un clone neuf ne l'a pas, donc le hook
  de design **local** y rougira sur le faux positif `broken-image` du `<img>`
  inerte du `<template>` de `hub.html` — et rien, dans le dépôt, ne dira
  pourquoi. **Ce paragraphe est le seul endroit qui le dise.** ⚠️ La vague de
  correction du 31 août 2026 l'a **laissé tel quel**, sur consigne : ni commité,
  ni supprimé.

## 8. La REVUE FINALE, et la vague de correction qu'elle a déclenchée

**31 août 2026, après la clôture ci-dessus.** Une relecture du chantier entier
a trouvé **deux Critiques, cinq Important et sept Minor**. Ce § dit ce qui a été
**corrigé**, ce qui a été **déclaré** sur décision du propriétaire, et pourquoi
la frontière est là.

> 🔴 **LES DEUX CRITIQUES ÉTAIENT DES JONCTIONS, PAS DES RÈGLES — et c'est le
> constat le plus utile de cette revue.** Dans les deux cas, la règle était
> écrite, pure et testée ; c'est le **câblage** qui ne l'appelait pas. Un test
> de plus sur `assurerAccesFrais` ou sur `poserPrefixe` n'aurait rien vu.
> **Le § 5.2 ci-dessus se croyait exhaustif ; il ne cherchait que des
> affirmations fausses, jamais des appels manquants.**

### 8.1 🔴 Les deux Critiques — CORRIGÉES

| # | Le défaut, tel qu'il se mesurait | Le remède |
| --- | --- | --- |
| **C1** | **Le WebSocket de la session de contrôle ne recevait jamais de jeton frais.** `hub/page.ts` passait `jeton: acces`, **une chaîne figée au chargement**, or `ouvrirLaSession` ne court, pour un suiveur, **qu'au moment de sa promotion** — potentiellement des heures plus tard —, et `DUREE_JETON_ACCES_MS` vaut **dix minutes** (`plateforme/src/identite/jeton.ts`). Il présentait donc un jeton expiré, `garde.verifier` rendait `motif: 'expire'`, `estPlacePrise` rendait `false`, le refus s'affichait puis `canalDeControlePerdu()` l'écrasait par « Rechargez la page ». 🔴 **La promotion — la seule chose qui justifie toute l'élection — ne pouvait pas fonctionner en usage réel.** Le **pont** souffrait du même défaut par un autre chemin : `fichiers/canal.ts` lisait `jetonAcces()`, le contenu **BRUT** du coffre. | `DepsBureauPage.jeton: string` devient **`jetonFrais(): Promise<string \| undefined>`** — un fournisseur, jamais une valeur. La séquence de promotion descend dans **`porteur.ts::promouvoir`**, pure et testée : redemander le jeton, installer le pont, ouvrir le socket ; sans jeton, **aucun socket** et un message actionnable. `DepsFichiers` reçoit le même fournisseur, appelé **après** le sélecteur de répertoire (un `await` avant `showDirectoryPicker()` consommerait l'activation utilisateur) et passé à `OptionsCanal.jeton`. |
| **C2** | **Le hub ne récupérait jamais le préfixe de VM, alors que `GET /vm` le lui rendait déjà.** `poserPrefixe` n'avait **qu'un seul** appelant de production — `connexion.ts:167`, dans `chercherLaSession`, qui ne court **que sur la page de connexion**. Or ce chantier fait précisément qu'un visiteur derrière Pomerium obtienne son jeton **sur le hub** (`assurerAccesFrais` → `/auth/moi`) **sans jamais passer par `connexion.html`**. `lirePrefixe()` rendait `''` : `composer('', 'bureau')` rendait `bureau`, `sessionDuPont()` rendait `fichiers`, `nomDuVerrou('')` rendait `nivuus-bureau`. **L'agent annonçait sur `<prefixe>:bureau` et le hub écoutait `bureau` : aucun `fenetre-ouverte` n'arrivait jamais** ; et le verrou n'étant pas préfixé, la protection que `porteur.ts` et `porteur-dom.ts` déclarent bruyamment offrir était **vacueuse dans le chemin nominal**. | La décision descend dans **`prefixe.ts::prefixeDeLaVm`**, pure et testée (⚠️ la **chaîne vide vaut « effacer »**, elle ne fait pas lever `poserPrefixe` : un service qui annonce `prefixe: ''` n'est pas une programmation fausse du client) ; `retenirLePrefixe` l'applique au coffre. `hub/page.ts::peupler` l'appelle sur `vm.prefixe`, **avant** `installerLeBureau`. Un garde ③ de `parcours.test.ts` fige **l'ORDRE** des deux appels dans la source de la page servie à la racine. |

🔴 **ATTRIBUTION HONNÊTE DE C2 : LE DÉFAUT PRÉEXISTE À CE CHANTIER.** Il date
du correctif Pomerium du **30 août 2026** — le jour où le hub a su obtenir un
jeton sans passer par l'écran de connexion —, et `shell-page.ts` en souffrait
**aussi**. Il devient *critique* parce que ce chantier fait du hub **la surface
unique**, donc la seule à composer un nom de session. Ce n'est pas une
régression de `navigation-hub-unique` ; c'en est une **conséquence**.

### 8.2 Les Important et les Minor — CORRIGÉES

| # | Le défaut | Le remède |
| --- | --- | --- |
| **I1** | **Un onglet qui rejoint après stabilisation ne recevait jamais rien.** `diffuserSiChange` ne poste que sur **changement** d'empreinte, et le porteur **ignorait** tout message du canal. En régime — trois fenêtres, rien qui bouge —, un second onglet montrait une liste **vide, pour toujours**, contredisant la spec §4 (« Les onglets non porteurs l'affichent à l'identique »). | Tout onglet poste `batirDemande()` **au montage** ; le porteur, à sa réception, remet son empreinte à `''` pour que la diffusion reparte — **y compris pour une liste vide**, dont l'empreinte `'[]'` diffère de `''`. La règle « ce message est-il une demande d'état ? » vit dans `porteur.ts::estDemandeEtat`, à côté de `lireEtat`. |
| **I3** | **Un porteur démis par `estPlacePrise` ne relâchait pas son verrou** : la promesse `new Promise<never>(() => {})` n'est jamais résolue — une `Promise<never>` ne peut se régler que par un REJET —, donc **sa partition n'aurait plus jamais eu de porteur**. | `elire` rend une `Election` exposant `relacher()`, et la promesse tenue devient `Promise<void>`, résolue **exactement une fois**. La branche `estPlacePrise` l'appelle en repartant en suiveur. ⚠️ **L'onglet ne se remet PAS dans la file** : se redemander le verrou le reprendrait dans l'instant et la boucle refus → relâche → reprise tournerait sans fin. **Limite déclarée** : cet onglet-ci reste suiveur jusqu'à son rechargement ; ce qui est gagné est que le verrou est **LIBRE**. |
| **I4** | **`await peupler()` précédait `installerLeBureau` sans garde.** Une panne réseau sur `GET /vm` remontait non rattrapée (`catalogue.ts` déclare qu'une panne d'ENVIRONNEMENT remonte telle quelle), `#message` avait déjà été vidé, et l'utilisateur voyait une page **blanche, sans bureau et sans explication**. Avant ce chantier le bureau vivait ailleurs et survivait à une panne du catalogue : **ce couplage est neuf**. | `try/catch` autour de `peupler()`, message de danger, **et le bureau installé quand même**. ⚠️ **L'interaction avec C2 est le point délicat** : le préfixe vient justement de l'appel qui peut échouer — on installe donc **avec ce qu'on sait**, c'est-à-dire le préfixe déjà au coffre. |
| **M1** | Les JSDoc de `section?` dans `bureau/fenetres-dom.ts` et `bureau/fichiers-dom.ts` disaient « `undefined` quand la page n'a pas de pli — c'est le cas de `shell.html` » : **faux depuis la tâche 9**. L'unique appelant passait **toujours** `section`, donc l'optionnel n'était que du code mort justifié par une phrase fausse. | Les deux paramètres deviennent **obligatoires** ; c'est `tsc --noEmit` qui juge. |
| **M2** | Des commentaires devenus faux, **cherchés par le SENS** — voir § 8.4, qui donne le compte, la règle de sélection et la raison de chaque occurrence laissée. | |
| **M3** | **`JSON.parse(evenement.data)` était NU** dans l'écouteur du socket de contrôle : une trame non-JSON y levait, dans un gestionnaire d'événement. `relais.ts` a lui-même dû ajouter le garde symétrique. | `porteur.ts::lireTrame`, jumelle de `webrtc.ts::parseSignalingMessage` : objet **non nul et non tableau** avant toute lecture de propriété. |
| **M4** | **`porteur-dom.ts` déclare « AUCUNE RÈGLE ICI » et ce n'était pas exact** : le choix du rappel `rouvrir` (`porteur ? bureau.rouvrir : window.open`) restait une décision de produit prise dans le câblage. | La décision descend dans `porteur.ts::ouvertureParLeBureau`, et l'en-tête **nomme désormais ce qui reste** (les textes affichés, le branchement des écouteurs) plutôt que de promettre ce que le fichier ne tient pas. 🔵 Effet de bord recherché : l'URL `/index.html?session=…` était écrite **deux fois** dans le fichier ; elle l'est une. |
| **M6** | **Un onglet suiveur était muet sur son propre état** : le seul texte qui l'expliquait était écrit dans `#etat-fichiers`, **à l'intérieur d'un `<details>` replié**, et `#statut` restait vide. | `#statut` porte « Bureau tenu par un autre onglet. » — **ce n'est pas un message d'erreur** : la décision « aucune erreur au second onglet » n'interdit pas d'informer, et cette ligne est ce qui rend I6 (ci-dessous) compréhensible. Un onglet promu plus tard l'écrase par « connexion du bureau… » dès `ouvrirLaSession`. |
| **M7** | **Le garde ① de `parcours.test.ts` pouvait être satisfait par un COMMENTAIRE.** Il cherchait la sous-chaîne `fenetre-ouverte` dans le texte source ; `porteur.ts` porte ce mot **en prose** et vit dans la fermeture d'imports de la racine, si bien que supprimer la branche réelle l'aurait laissé **VERT**. La faiblesse était déclarée comme préexistante (§ 6.2) — elle était devenue **ARMÉE** par ce chantier. | Ancré sur la **syntaxe** : `/\.type === 'fenetre-ouverte'/`. **Et le garde est lui-même éprouvé**, sur deux chaînes fabriquées dans le test — jamais sur la prose du dépôt, qui n'est pas stable. ⚠️ **Sa limite est dite** : un commentaire qui RECOPIERAIT le code le satisferait encore. |

**Et la recommandation qui valait correction** : `porteur.ts::promouvoir` rend le
cycle de promotion **testable**, ce que la spec §3 annonçait déjà (`ouvrirSocket`
comme dépendance injectée) et que l'implémentation n'avait pas suivi — c'est ce
qui laissait C1, I2 et I3 hors de portée de tout test.

⚠️ **CE QUE L'INJECTION N'A PAS FAIT, ET POURQUOI — ÉCART ASSUMÉ AVEC LE
BRIEF.** Il demandait d'injecter `ouvrirSocket` et `installerPont` dans
**`DepsBureauPage`**. Ils sont injectés dans **`DepsPromotion`**
(`porteur.ts`), un cran plus bas. La raison est mesurable : `installerLeBureau`
lit `window.addEventListener` et manipule des `HTMLElement`, donc **ne peut pas
tourner sous Node**, et `client/` n'a **ni jsdom ni happy-dom, par convention
écrite**. Les injecter là les aurait laissés **aussi intestables qu'avant** —
c'est-à-dire manqué le seul but de la recommandation. Ce qui doit être testé
descend dans un module pur ; c'est fait.

### 8.3 Les rouges VUES pendant cette vague — et **laquelle** a rougi

🔴 **Chaque contrôle neuf a été vu ROUGE par mutation ciblée du produit, puis
restauré depuis une COPIE NOMMÉE** — jamais par `git checkout --`.

| Mutation | L'assertion qui a rougi |
| --- | --- |
| `promouvoir` reçoit un jeton figé au lieu d'appeler `jetonFrais` | **`le jeton doit etre REDEMANDE a la promotion: expected +0 to be 1`** — *et*, dans le même relevé, `expected [ 'pont', 'socket' ] to deeply equal [ 'sans-jeton' ]` |
| `DepsBureauPage` retrouve un champ `jeton: string` | `tsc` : **`TS2578: Unused '@ts-expect-error' directive`** (`porteur-dom.test.ts:91`) — la directive **EST** l'assertion |
| `prefixeDeLaVm` n'efface plus jamais | **`expected { action: 'poser', prefixe: '' } to deeply equal { action: 'effacer' }`**, et `expected [Function] to not throw an error but 'Error: préfixe vide refusé…' was thrown` |
| `retenirLePrefixe` retiré de `hub/page.ts` | **`un seul appel à retenirLePrefixe attendu: expected [] to have a length of 1 but got +0`** |
| `estDemandeEtat` rend toujours `false` | `expected false to be true` sur « reconnait la demande qu un onglet neuf pose au montage » |
| `Election::relacher` redevient inerte | **`le verrou doit etre RENDU quand on le relache: expected false to be true`** |
| `lireTrame` sans garde de forme | **`expected null to be undefined`** — le cas `"null"`, celui qui fait lever `null.type` |
| le garde ① retombe sur `includes('fenetre-ouverte')` | `expected true to be false` sur « REFUSE une simple mention en prose » |
| `section` omise chez les deux appelants | `tsc` : **deux `TS2345`**, `porteur-dom.ts:213` et `:330` |

🔴 **UNE ROUGE A D'ABORD ROUGI POUR LA MAUVAISE RAISON, ET LE TEST A ÉTÉ
CORRIGÉ PLUTÔT QUE SON VERDICT ACCEPTÉ.** L'épreuve de `relacher` faisait
`await tenue` : la mutation la faisait **EXPIRER** au lieu d'échouer, et une
expiration ne dit pas **quelle** assertion a rougi. Le test laisse désormais
courir deux microtâches puis **ASSERTE**.

🔴 **ET UNE RESTAURATION A EFFACÉ UN CONTRÔLE NEUF SANS RIEN DIRE.** La copie
nommée de `parcours.test.ts` avait été prise **avant** l'ajout du garde ③ ;
restaurer après la rouge du garde ① l'a donc **supprimé**. Rien ne l'a signalé —
ni `tsc`, ni la suite, qui est restée **verte**. **Ce qui l'a attrapé est le
COMPTE de tests** (665 attendu, **664** rendu), relu contre le relevé précédent.
⚠️ **La leçon est neuve et vaut d'être inscrite : une copie nommée VIEILLIT, et
restaurer depuis une copie prise avant l'addition qu'on vient d'écrire est une
perte SILENCIEUSE.** Rafraîchir la copie après chaque addition, et **juger la
restauration sur le compte de tests**, jamais sur le seul « vert ».

### 8.4 M2 — le second balayage, par le SENS

🔴 **RÈGLE DE SÉLECTION, ÉNONCÉE AVANT DE COMPTER** (un sous-ensemble sans règle
énoncée est un sous-ensemble *choisi*, même quand on ne l'a pas choisi) : *est à
corriger toute occurrence qui **AFFIRME**, **au présent** et **sans qualificatif
de date**, un fait sur `shell-page.ts`, `shell.html`, `shell.css` ou « la
page-shell » qui est **FAUX au 31 août 2026** après ce chantier.*

**Trouvées : 103** — `grep -rn "shell-page\|page-shell\|shell\.css\|shell\.html"`
sur `client/src`, `client/outils` et `client/*.html`, hors `*.test.ts`.
**Corrigées : 11.** **Laissées : 92.**

⚠️ **CE 103 EST LE COMPTE *AVANT* CORRECTION, ET LA MÊME COMMANDE RELANCÉE
REND AUJOURD'HUI 106.** Ce n'est pas une contradiction : **corriger une
affirmation périmée consiste ici à la barrer et à dire ce qu'elle disait**, ce
qui ÉCRIT le mot qu'on cherchait. Les onze corrections nomment donc toutes
`shell-page.ts` ou `shell.html` — au **passé**, et datées. **Dit ici plutôt que
laissé au prochain lecteur, qui relancerait la commande et lirait un écart sans
en comprendre la cause** : c'est le patron que le § 2 de ce document a déjà payé
sur `..HEAD`.

| # | Où | Ce qui était faux AUJOURD'HUI |
| --- | --- | --- |
| 1 | `connexion.ts:6` | la convention des fichiers non testés invoquait `shell-page.ts` — **seize lignes de redirection**, qui ne sont plus un précédent de câblage |
| 2 | `connexion.ts:105` | « Même arbitrage que `shell-page.ts` » sur les classes invisibles au §7.9 — ce fichier ne porte plus **aucun** `CLASSE_DE_TON` |
| 3 | `hub/page.ts:5` | même liste de convention |
| 4 | `hub/cartes.ts:10` | même liste de convention |
| 5 | `design/selecteur-theme.ts` | 🔴 **les DEUX moitiés étaient fausses** : « Sur la PAGE-SHELL … `shell-page.ts` y redirige tout visiteur sans jeton ». Le sélecteur est câblé depuis `hub/page.ts` (`#themes`), et c'est `hub/page.ts::demarrer` qui redirige |
| 6-7 | `design/primitives.css` (×2) | « **DEUX** SURFACES DU PRODUIT LE LIENT — `client/shell.html` et … », **et la transcription du §7.9 qui l'accompagne**. Relevé **RELANCÉ** : la commande rend **TROIS** surfaces, `index.html`, `hub.html`, `connexion.html` |
| 8 | `design/primitives/message.css:22` | « `role="status"` … (`client/shell.html` ×2) ». Mesuré : `shell.html` **0**, `hub.html` **4**, `index.html` **3**, `connexion.html` **1** |
| 9 | `design/primitives/surface.css:15` | le bouton « Rouvrir » situé dans « `client/shell.html`, `<template id="modele-fenetre">` » — `grep -ln` ne rend plus que `hub.html` |
| 10 | `bureau/bureau.css:38` | 🔴 « **ELLE SE LIE DEPUIS `shell.html`** » — c'est `client/hub.html:19` qui porte le `<link>` |
| 11 | `shell.ts:1` | le fichier **s'ouvrait** sur « La page-shell : le bureau. C'est elle qui… », au présent |

**Laissées, et pourquoi** — la règle ci-dessus les exclut toutes :

| Combien | Ce que c'est | La raison |
| --- | --- | --- |
| ~40 | mentions déjà **qualifiées** par la tâche 10 (« avant que le hub ne devienne la seule surface ») | elles disent le passé, et le disent |
| ~25 | « la page-shell » employé comme **NOM DE RÔLE** dans `fichiers/*`, `viewport-dom.ts`, `resize-dom.ts`, `main.ts`, `style.css`, `design/theme.ts` | le rôle existe toujours — il a changé de titulaire. `shell.ts` porte désormais **une note en tête** qui le dit une fois pour toutes, plutôt que 25 corrections qui déborderaient de ce chantier. ⚠️ **`viewport-dom.ts` fait exception et A été corrigé** : son en-tête est load-bearing pour la limite I5 |
| ~15 | énoncés **datés** de S1–S4, du lot 33, du 20 août | exacts à leur date, et ils le disent |
| ~10 | `redirection.ts`, `manifeste.ts`, `shell.html`, `hub.html`, `hub.css`, `surfaces-baties.mjs`, `classes-employees.mjs` | ils **décrivent la redirection elle-même** : `shell.html` y est le sujet, pas une affirmation périmée |
| 2 | `bureau/bureau.css:75-76` | 🔴 **transcription VERBATIM d'une sortie d'outil passée** — la corriger **fabriquerait une pièce**, ce que le dépôt interdit nommément |

### 8.5 🔴 Ce qui a été DÉCLARÉ, pas corrigé — décisions du propriétaire

- 🔴 **I2 — LA PROMOTION EFFACE LA LISTE QUE LE SUIVEUR AFFICHAIT.** Un onglet
  promu bascule sur la branche `porteur`, dont `bureau.liste()` est
  **structurellement vide** (son `bureau` n'a jamais reçu de `fenetre-ouverte`,
  faute de socket avant la promotion) : il peint `[]` au premier tour.
  **L'effet net, dit en clair : fermer l'onglet porteur prive définitivement
  les autres onglets de la liste des fenêtres.** La correction supposerait une
  méthode neuve sur `client/src/shell.ts` — semer le `bureau` avec l'état reçu
  —, **que la spec §6 gèle nommément**. C'est une décision de conception qui
  appartient au propriétaire, pas à une vague de correction. **Déclarée dans
  `porteur.ts::fenetresAPeindre`.**
- 🔴 **I5 — UNE FENÊTRE ROUVERTE DEPUIS UN SUIVEUR N'ANNONCE JAMAIS SON
  VIEWPORT.** `viewport-dom.ts` poste vers `window.opener`, donc vers le
  suiveur ; `bureau.viewportRecu` y retourne immédiatement (sa table `connues`
  est vide) et `envoyer` est un no-op faute de socket. **Conséquence, celle du
  lot 33 : le recadrage et la taille restent ceux de la session précédente, et
  rien ne le trace.** ⚠️ **Un commentaire incomplet sur une limite déclarée
  vaut une preuve fausse** : la clause a été ajoutée au bloc de
  `porteur-dom.ts` qui documentait déjà la limite du suiveur, **et** en tête de
  `viewport-dom.ts`.
- 🔴 **I6 — « LANCER » DEPUIS UN SUIVEUR FAIT PARAÎTRE LA FENÊTRE DANS L'ONGLET
  PORTEUR**, hors activation utilisateur ; et si le bloqueur de pop-ups
  intervient, le message d'échec s'affiche **dans l'onglet que l'utilisateur ne
  regarde pas**. 🔴 **DÉCISION : « Lancer » N'EST PAS DÉSACTIVÉ SUR UN
  SUIVEUR** — `POST /lancer` ne porte aucun rôle exclusif et fonctionne
  parfaitement ; le désactiver priverait l'utilisateur d'une fonction qui
  marche, pour une gêne d'ergonomie. **La spec §4 a été CORRIGÉE par une note
  datée** (elle affirmait « Chaque onglet ouvre ses propres fenêtres depuis ses
  propres clics » : vrai de « Rouvrir », **faux de « Lancer »**, qui est
  pourtant le geste principal). M6 rend le comportement explicable.
- ⚠️ **M5 — L'ICÔNE DU MANIFESTE EXPIRE.** La spec §5 liste « lecture d'icône »
  parmi les points d'appel d'`assurerAccesFrais`, mais l'icône passe par une
  **URL SIGNÉE valable cinq à six minutes** (lot 16), qui n'est **pas** un
  jeton porteur et qu'aucun rafraîchissement ne renouvelle. Cliquer
  « Installer » sur un hub ouvert depuis plus longtemps publie donc un
  manifeste **sans icône** tout en affichant « est prête à être installée ».
  Le remède serait de **re-lister les applications** avant de publier, donc de
  refrapper les URL signées : une addition de conception, non prise ici.

### 8.6 Ce que cette vague n'établit PAS

- 🔴 **TOUJOURS AUCUNE RECETTE NAVIGATEUR, TOUJOURS AUCUN HUMAIN.** Tout ce que
  le § 6.1 dit reste vrai mot pour mot. **Les remèdes C1, I1, I3 et I6 sont
  précisément ceux dont l'effet ne se voit qu'avec deux onglets et du temps** —
  c'est-à-dire exactement ce qu'aucun contrôle de ce dépôt ne joue.
- 🔴 **I4 N'EST GARDÉ PAR AUCUN TEST.** `hub/page.ts` n'est pas testé
  unitairement, par convention déclarée. Le `try/catch` a été **lu**, jamais
  exercé : aucune panne de `GET /vm` n'a été simulée.
- ⚠️ **LA JONCTION C1 EST GARDÉE PAR UN CONTRAT DE TYPE, PAS PAR UN
  COMPORTEMENT.** `porteur-dom.test.ts` fige que `DepsBureauPage` n'expose plus
  de jeton scalaire, et `porteur.test.ts` fige que `promouvoir` redemande son
  jeton — mais **rien ne vérifie que `installerLeBureau` appelle réellement
  `promouvoir`** : ce fichier ne peut pas tourner sous Node.
- ⚠️ **LE CHEMIN `sansJeton` N'A JAMAIS COURU EN PRODUCTION**, ni celui du pont
  (`lecteurEchoue` sur jeton expiré) : ils sont testés, jamais exercés.
- 🔴 **`client/src/bureau/porteur-dom.ts` EST À 488 LIGNES**, contre 327 avant
  cette vague. **Il est à sa porte** : le prochain qui y ajoute quoi que ce soit
  doit **extraire d'abord**, dans une tâche dédiée — jamais comprimer. ⚠️ Ce
  nombre est un **relevé daté**, jamais une source de vérité : la commande de
  `CLAUDE.md`, relancée, ne rend que `agent/src/windows_source.rs` (**630**).

## 9. Ce qu'il faut retenir en une phrase

Le hub tient désormais la session de contrôle, montre ses fenêtres et son pont
fichiers, élit un porteur par Web Locks sans jamais montrer d'erreur aux autres
onglets, et vérifie la fraîcheur de son jeton **avant chaque usage** — et la
seule chose que personne n'a faite, c'est **l'ouvrir dans un navigateur**.

> 🔴 **CETTE PHRASE ÉTAIT FAUSSE SUR SA MOITIÉ LA PLUS IMPORTANTE, ET C'EST LA
> REVUE FINALE QUI L'A ÉTABLI** (31 août 2026). « Avant chaque usage » ne valait
> **ni pour l'ouverture du WebSocket** — que la spec §5 liste pourtant
> nommément parmi les points d'appel — **ni pour le pont fichiers**. Elle est
> devenue vraie par les correctifs du § 8.1, pas par sa rédaction.
> ⚠️ **Et la dernière proposition, elle, n'a pas changé** : personne n'a ouvert
> cette page dans un navigateur.
