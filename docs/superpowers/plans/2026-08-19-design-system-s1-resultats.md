# Sous-bloc S1 — le socle : tokens, thèmes, amorce, et les sept contrôles : résultats

**Plan :** `docs/superpowers/plans/2026-08-19-design-system-s1.md` (commit `f4cb8c0`).
**Spec :** `docs/superpowers/specs/2026-08-19-design-system-design.md` (commit `5b6b830`).
**Journaux :** `docs/superpowers/plans/journaux-design-s1/`.

**Commit de mesure : `604f91c`.** Toutes les valeurs de ce document sortent
d'une exécution réelle à ce commit, sauf mention contraire explicite.

⚠️ **L'ARBRE EST PARTAGÉ, ET IL A BOUGÉ PENDANT LA RECETTE.** Le chantier E
(microphone) a commité `604f91c` **entre la première et la seconde salve de
mesures**, initialement prises à `c9bb8a7`. **Toutes les mesures de ce document
ont donc été REPRISES à `604f91c`** plutôt que recopiées — c'était le geste le
moins cher, et le seul honnête. Ce que ce commit du voisin touche dans
`client/`, relevé par `git diff --stat c9bb8a7..604f91c -- client/` : **un seul
fichier, `client/recette/micro-e1.mjs` (332 lignes)**, un instrument de banc
sous `client/recette/`, **hors du périmètre de chacun des sept contrôles** —
vérifié par la sortie des contrôles eux-mêmes (§7.2 balaie `client/src/` et les
entrées Vite ; §7.6 nomme ses six fichiers balayés), pas supposé.

---

## 1. Les sept contrôles — verdict, et nombre d'exécutions

**Deux exécutions de chacun. Aucun taux n'est revendiqué nulle part** : les
sept contrôles sont **déterministes**, et deux exécutions y établissent la
**reproductibilité**, jamais une fréquence. La spec §9 l'écrit, et la question
« combien de fois sur combien » ne doit pas être empruntée à une campagne qui,
elle, l'aurait posée.

| # | Contrôle | Verdict | Le chiffre, **relevé** | Exéc. |
| --- | --- | --- | --- | --- |
| §7.1 | les contrastes tiennent les seuils WCAG | **VERT** | **50 paires vérifiées, 0 échec, minimum global 3,16** | 2 |
| §7.2 | aucune couleur littérale hors `tokens.css` | **VERT** | **0** occurrence, **46 fichiers balayés** — contre **neuf** sur l'arbre intact | 2 |
| §7.3 | toute surface bâtie porte les tokens | **VERT sur ses DEUX assertions** | A : 0 échec sur 4 pages ; B : 0 échec, **évaluée sur 4 pages** — contre 1 seule avant | 2 |
| §7.4 | les trois blocs déclarent le même ensemble | **VERT** | **0 écart** ; racine 47 tokens, media-clair 13, attribut-clair 13 | 2 |
| §7.5 | la bascule atteint les N fenêtres | **VERT** | **10 tests passés** (`theme.test.ts`) | 2 |
| §7.6 | aucun token orphelin, aucun `var()` non déclaré | **VERT, sous une liste d'attente nommée** | inclusion ① : **0 écart** ; inclusion ② : **28 orphelins, dont 28 en attente déclarée** — voir §3 | 2 |
| §7.7 | le poids CSS ne dérive pas | **VERT** | **3 503 octets** sur un plafond de **12 288** (marge 8 785) | 2 |

**Agrégateur** (`npm run design:verifier`) : **6/6 contrôles verts**, aux deux
exécutions. Le septième, §7.5, est un test unitaire et tourne dans `npm test` —
c'est pourquoi on lit six verdicts et non sept.

**Comptes**, aux deux exécutions : `client` **179 tests**, `proto` **37**,
`client typecheck` **exit 0**, `npm run build` **exit 0**.

**`scripts/verify-all.sh` : exit 0 sur ses DIX étapes.** ⚠️ **Aucune étape
étrangère n'a échoué** — ni `cargo test --workspace`, ni `cargo clippy`, ni
`proto`, ni les trois `plateforme` (Postgres compris). C'est à signaler parce
que le plan prévoyait le contraire : P2 avait vu son témoin tomber « sur un
test Rust du voisin », et le risque était réel puisque le chantier E a du
travail non commité dans `agent/`. Il ne s'est pas réalisé.

---

## 2. Chaque ROUGE, avec son message VERBATIM

🔴 **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Les rouges
ci-dessous sont **rejouées à la recette** et versées dans
`journaux-design-s1/rouges-rejouees.log`, sauf indication contraire.

⚠️ **Les rouges des tâches 1 à 3 ne sont PAS rejouables**, et il faut le dire :
elles portaient sur l'**arbre intact** — neuf couleurs littérales, deux
surfaces sans feuille —, et l'arbre est corrigé depuis. **Leur message verbatim
n'a survécu nulle part** : elles ont été jouées dans un espace de travail que
ce dépôt ne verse pas, et seule leur trace de commit subsiste. **C'est
exactement la perte que D9 a payée six fois**, à plus petite échelle, et c'est
la raison pour laquelle tout ce qui suit est versé dans git.

### §7.7 — le plafond abaissé (aucun fichier touché)

```
feuilles émises : 2
somme : 3503 octets
plafond : 1024 octets (arbitraire, voir l'en-tête)
DÉPASSEMENT de 2479 octets
exit=1
```

### §7.4 — un token retiré du SEUL bloc `media-clair`

```
  bloc racine : 47 token(s)
  bloc media-clair : 12 token(s)
  bloc attribut-clair : 13 token(s)
écarts : 1
  ÉCART  media-clair : --succes manquant
exit=1
```

🔴 **CETTE ROUGE A D'ABORD ÉCHOUÉ À ÊTRE ROUGE, et c'est le constat le plus
utile de la recette.** Une première perturbation ancrait sur les chaînes
`@media (prefers-color-scheme: light)` et `:root[data-theme="clair"]` — qui
apparaissent **d'abord dans le commentaire d'en-tête de `tokens.css`**. La
découpe portait donc sur le commentaire, **rien n'était retiré**, et le
contrôle rendait `écarts : 0`, `exit=0`. **Une perturbation qui ne perturbe
rien se lit exactement comme un contrôle qui ne mord pas** ; elle n'a été
attrapée qu'en relisant la sortie au lieu de la supposer. La version versée
ancre sur le sélecteur **suivi d'une accolade en début de ligne**, et porte une
`assert` qui refuse de jouer si l'ancre est introuvable.

### §7.1 — `--texte-faible` perturbé vers un gris trop clair

```
ÉCHEC clair texte-faible/fond-0 = 2.19 < 4.5
ÉCHEC clair texte-faible/fond-1 = 2.04 < 4.5
ÉCHEC clair texte-faible/fond-2 = 1.88 < 4.5
paires vérifiées : 50
échecs : 3
minimum global : 1.88
exit=1
```

### §7.2 — une couleur littérale dans `base.css`

```
client/src/design/base.css:56: background: #fff;
fichiers balayés : 46
couleurs littérales : 1
→ elles doivent vivre dans client/src/design/tokens.css
exit=1
```

### §7.3 — assertion A : `shell.html` perd sa feuille

```
pages bâties : 4 (connexion.html, design.html, index.html, shell.html)

assertion A — un <link rel="stylesheet"> par page : 1 échec(s)
  ÉCHEC A  shell.html : aucune feuille de style liée
assertion B — une feuille liée déclarant --fond-0 : 0 échec(s) (évaluée sur 3 page(s))
exit=1
```

**Et l'assertion B a été vue rouge SÉPARÉMENT, sur l'arbre d'avant la tâche 11**
— c'est la seule rouge des tâches 1 à 3 dont le verbatim ait survécu, ayant été
rejouée en tête de la tâche 11 :

```
assertion A — un <link rel="stylesheet"> par page : 2 échec(s)
  ÉCHEC A  connexion.html : aucune feuille de style liée
  ÉCHEC A  shell.html : aucune feuille de style liée
assertion B — une feuille liée déclarant --fond-0 : 0 échec(s) (évaluée sur 1 page(s))
```

⚠️ **Lire la ligne B avec attention : « 0 échec » y est un ZÉRO SANS PORTÉE.**
L'assertion B n'était évaluée que sur **une** page — les deux autres n'ayant
aucun lien, il n'y avait rien à résoudre. C'est le passage de **1 à 4 pages
évaluées** qui est le résultat de la tâche 11, pas le zéro, qui était déjà là.

### §7.5 — `appliquer('systeme')` POSE l'attribut au lieu de le retirer

```
AssertionError: expected true to be false // Object.is equality
Tests  2 failed | 8 passed (10)
```

C'est la rouge que le plan désigne : un `data-theme="systeme"` inventé **ne
casse rien visiblement** — le sélecteur de la requête média est
`:root:not([data-theme="sombre"])`, qu'un `"systeme"` traverse —, et c'est ce
qui le rendrait durable.

### §7.6 — QUATRE rouges, dont deux pour les deux sens de l'égalité

**Rouge A — un token perd son appelant** (`var(--r-2)` → `6px` dans `style.css`) :

```
inclusion ② — tout token déclaré a un appelant : 29 orphelin(s), dont 28 en attente déclarée
  NOUVEL ORPHELIN  --r-2  déclaré et appelé par personne
total : 1 écart(s)
exit=1
```

**Rouge A2 — un token de la LISTE gagne un appelant** (`--t-s` → `--t-xs`) :

```
  NOUVEL ORPHELIN  --t-s  déclaré et appelé par personne
  À RETIRER DE LA LISTE  --t-xs  a désormais un appelant (client/src/style.css) : la liste d'attente doit rétrécir
total : 2 écart(s)
exit=1
```

**Rouge B — un `var()` non déclaré** (`--fond-O`, lettre O au lieu du zéro) :

```
inclusion ① — tout var(--…) est déclaré : 1 écart(s)
  NON DÉCLARÉ  --fond-O  employé par client/src/design/base.css
exit=1
```

**Rouge C — celle qui éprouve l'EXCLUSION de la galerie**, et sans laquelle les
trois autres ne prouvent rien de la conception. La **même** rouge A, rejouée
avec `--sans-exclusion` :

```
inclusion ② — tout token déclaré a un appelant : 5 orphelin(s), dont 5 en attente déclarée
```

🔴 **La ligne `NOUVEL ORPHELIN --r-2` DISPARAÎT — `grep -c` rend 0.** Le défaut
devient **invisible**, la galerie employant `--r-2` comme elle emploie tout ;
les orphelins tombent de 29 à 5. **C'est la preuve que l'exclusion est porteuse
et non décorative.** *(La sortie globale reste 1, pour une autre raison qui
enfonce le clou : 23 des 28 tokens de la liste d'attente se déclarent alors
« à retirer », la galerie leur ayant donné un appelant fictif.)*

### Tâche 10 — le garde de l'amorce, et une rouge trouvée sur le garde LUI-MÊME

🔴 **Le garde entre `theme.ts` et `amorce-theme.js` a été VU VACUEUX avant
d'être bon.** Sa première rédaction assertait `toContain("'guac.theme'")`, et
le commentaire d'en-tête de l'amorce nommait la clé entre quotes : **les deux
assertions étaient donc satisfaites par le commentaire seul.** Mesuré en
remplaçant tout l'appel par `var t = null;` :

```
Tests  10 passed (10)      ← une amorce qui ne lit RIEN passait
```

Après correction — la clé ne s'écrit plus dans ce commentaire, et l'assertion
porte sur l'**appel** :

```
AssertionError: expected '/* L\'amorce anti-FOUC — posée AVANT …' to contain 'getItem(\'guac.theme\')'
AssertionError: expected [] to deeply equal [ 'guac.theme' ]
Tests  2 failed | 8 passed (10)
```

**Le greffon d'injection** a sa rouge propre, celle que la spec §11 nomme
(« le greffon Vite d'injection casse une entrée ») : un `handler` filtré sur
`index.html`, puis `grep -c 'guac.theme'` sur chaque `dist/*.html` —
**`1, 0, 0`**, défait ensuite, **`1, 1, 1`** (et `1, 1, 1, 1` après la tâche 12).

### Tâche 13 — la rouge de la DIXIÈME ÉTAPE, jouée sur le script réel

`background: #fff` dans `base.css`, puis `./scripts/verify-all.sh` :

```
==> client : npm run design:verifier
  …
═══ 4/6 contrôle(s) vert(s) ═══
  ÉCHEC  §7.2  aucune couleur littérale hors de tokens.css (sortie 1)
  ÉCHEC  §7.6  aucun token orphelin, aucun var() non déclaré (sortie 1)
ÉCHEC : client : npm run design:verifier
```

**Le script s'arrête là** : `proto : npm test` n'est jamais atteint. Et
l'agrégateur rapporte **les deux** contrôles tombés — c'est la preuve qu'il ne
court-circuite pas au premier échec, ce que son en-tête promet.

---

## 3. 🔴 LA DÉCISION DES 28 ORPHELINS, et pourquoi elle n'est pas un assouplissement

**Le fait :** à la fin de S1, `tokens.css` déclare **47** tokens et le produit
en appelle **19**. **28 sont orphelins**, et c'est **par construction** : S1
pose la palette entière, ce sont S2 (primitives), S3 (surfaces) et S4 (fenêtre
de session) qui l'emploieront. La seconde inclusion — aucun `var(--…)` non
déclaré — est **déjà verte** et l'a toujours été.

### Ce qui a été refusé, et sur MESURE

**Élaguer la palette à ce qui sert** était la voie évidente. Elle est refusée
sur un relevé, pas sur un goût :

```
node --input-type=module -e "import {PAIRES} from './src/design/contraste.ts'; …"
→ paires totales : 50
→ paires citant au moins un token sans appelant : 46
```

**Sur les 50 paires de contraste que le contrôle §7.1 vérifie, 46 citent au
moins un des 28 tokens sans appelant.** Neuf d'entre eux sont porteurs pour
§7.1 — `--alerte`, `--bord-fort`, `--danger`, `--fond-1`, `--fond-2`,
`--succes`, `--sur-accent`, `--texte`, `--texte-faible`. **Élaguer ferait
tomber §7.1 de 50 paires à 4** : on satisferait un contrôle en vidant l'autre,
et le sous-projet perdrait sa seule mesure d'accessibilité pour verdir un
compte. C'est nommément le geste que ce dépôt combat.

**Ne faire porter le contrôle que sur la seconde inclusion** a été refusé pour
une autre raison : rien n'aurait jamais forcé à rétablir la première, et le
§7.6 serait mort dans l'indifférence.

### La forme retenue : une LISTE D'ATTENTE EXACTE, jamais un seuil

`client/outils/tokens-orphelins.mjs` déclare `EN_ATTENTE_D_APPELANT`, **28
entrées nommées, datées, chacune portant le sous-bloc qui la consommera**. Le
contrôle exige l'**ÉGALITÉ** entre l'ensemble des orphelins et cette liste — pas
une inclusion, pas un plafond. **Il échoue donc dans les deux sens :**

| Ce qui arrive | Ce que le contrôle dit |
| --- | --- |
| un token orphelin **absent** de la liste | `NOUVEL ORPHELIN <token>` |
| un token de la liste qui **a gagné** un appelant | `À RETIRER DE LA LISTE <token> … la liste d'attente doit rétrécir` |

**La seconde moitié est celle qui compte : elle rend la liste
AUTO-NETTOYANTE.** Un seuil (« au plus 28 orphelins ») aurait pourri sur place ;
une liste dont chaque retrait est **forcé** par le contrôle rétrécit toute
seule, et **le jour où elle est vide, elle disparaît avec son encadré**. Les
deux sens ont été vus rouges (§2, rouges A et A2).

⚠️ **Ce que le contrôle mesure, dit sans le maquiller.** Il ne mesure **pas**
« la palette est-elle entièrement employée ? » — la réponse est non, et elle le
restera jusqu'à S4. Il mesure que **l'écart entre la palette et son emploi soit
CONNU, ÉNUMÉRÉ ET DÉCROISSANT**. C'est moins que ce que le §7.6 laissait
espérer, c'est écrit dans le script à l'endroit où on le lit, et **cela peut
échouer dès aujourd'hui**.

### Le cas particulier : `--police-mono`

La spec §4.3 laisse son sort ouvert — « si aucun appelant n'apparaît, le token
sort ». Il n'a **qu'un seul** appelant prévu, `#stats`, et **S1 ne l'a pas
câblé**. Ce n'est pas un oubli : `#stats` hérite aujourd'hui de `--police-ui`,
si bien que lui poser la pile monospace **changerait son apparence**, ce que S1
s'interdit nommément (« visuellement quasi neutre sur `index.html` »).

**C'est donc S4 — le seul sous-bloc qui a le droit de toucher la fenêtre de
session — qui tranche : ou il le câble, ou il le retire.** Son entrée de liste
le dit en toutes lettres, et **aucun sous-bloc n'a le droit de laisser cette
ligne en place sans décider**. C'est le seul des 28 dont le sort soit encore
ouvert ; les 27 autres ont un appelant nommé.

⚠️ **Une seconde raison, non anticipée, plaide pour le câblage** : la galerie
emploie `--police-mono` pour ses étiquettes de valeur, mais la galerie est
**exclue** du périmètre. Si S4 retirait le token, il faudrait aussi le retirer
de la galerie — l'inclusion ① le dirait (`NON DÉCLARÉ --police-mono`).

---

## 4. Les onze divergences D1…D11, et leur sort

| # | Objet | Sort |
| --- | --- | --- |
| D1 | la spec annonce trois contrôles rouges, il n'y en a que deux | **TENU** : §7.2 et §7.3 joués rouges sur l'arbre intact (T1, T2) ; §7.7 vert avec sa rouge par plafond abaissé ; les quatre autres n'avaient rien à lire avant T4 |
| D2 | trois longueurs sans cran dans les échelles | **TENU, et la dette est déclarée** : `padding: 6px`, `font-size: 18px`, `letter-spacing: 0.02em` restent littérales, avec leur justification dans `style.css:12-31`. **La clause « aucune longueur hors échelle » du §8 est FAUSSE à la fin de S1**, de trois valeurs exactement, et **aucun des sept contrôles ne mesure une longueur** |
| D3 | deux des neuf couleurs ne sont pas des noirs | **TENU** : six voiles hors thème, valeurs reprises **verbatim**, aucun raccord à `--danger`/`--alerte` |
| D4 | `--e-3` ne vaut 12 px que si `html` perd son `font-size` | **TENU** : `base.css` ne pose la police que sur `body`, et `reprise.test.ts` refuse tout `font-size` sur un sélecteur contenant `html`. ⚠️ **« À l'identique » vaut à racine 16 px, et là seulement** |
| D5 | la clé `localStorage` | **TENU** : `guac.theme`, préfixée comme `guac.jeton.*` de P2. Le test de robustesse emploie **la vraie clé voisine** (`guac.jeton.acces`), pas une clé inventée |
| D6 | un `.mjs` peut importer un `.ts` sur Node v24.9.0 | **TENU, et EXPLOITÉ DEUX FOIS** : trois contrôles partagent `tokens.ts`, et `tokens-orphelins.mjs` importe en plus **`vite.config.ts`** pour lire la liste des entrées au lieu de la recopier. Contrainte « effaçable » respectée |
| D7 | `vite.config.ts` n'est pas typechecké | **TENU, non corrigé** : le greffon est validé par `npm run build` seul. ⚠️ **Une erreur de type y est un échec de build, jamais une erreur `tsc`** — mesuré à l'exécution : un `*/` accidentel dans un commentaire (la sous-chaîne `src/**/*.ts`) a fait échouer le build avec `ERROR: Unexpected "*"`, et `tsc` n'en aurait rien dit |
| D8 | le mot-clé `red` lève un faux positif sur du français | **TENU** : §7.2 retire les commentaires d'abord, et rend **9** sur l'arbre intact, pas 11 |
| D9 | `color-scheme` doit suivre le thème | **TENU, et CORROBORÉ EN NAVIGATEUR RÉEL** : `colorScheme: 'light'` et champ `type="password"` en rendu natif clair sur `connexion.html`. Voir `journaux-design-s1/corroboration-deux-fenetres.md` |
| D10 | l'arbre est partagé | **TENU, et l'événement s'est produit** : le voisin a commité `604f91c` en pleine recette ; toutes les mesures ont été **reprises**, jamais recopiées. Aucun `git add -A`, chaque commit à pathspec explicite |
| D11 | la galerie est une entrée Vite, exclue de §7.6 seulement | **TENU** : `design.html` est la 4ᵉ entrée, **incluse** dans §7.2 et §7.3 (assertion B évaluée sur 4 pages), **exclue** de la moitié « employé » de §7.6 — et l'exclusion est **prouvée porteuse** par la rouge C |

---

## 5. Les huit relevés périmés de la spec — valeur juste, et son commit

Corrigés par le plan à `8ad03a2`, **revérifiés ici à `604f91c`** là où S1 les a
fait bouger à nouveau.

| # | Ce que la spec écrit | Valeur juste (plan, `8ad03a2`) | Après S1 (`604f91c`) |
| --- | --- | --- | --- |
| 1 | « tout le style tient en **85 lignes** » | **139** | **181** (`style.css`) + **199** (`tokens.css`) + **73** (`base.css`) + **8** (`socle.css`) |
| 2 | « **cinq** valeurs littérales employées comme couleurs » | **neuf** | **zéro** — c'est l'objet de §7.2 |
| 3 | « `vite.config.ts:10-12` déclare exactement **deux** entrées » | **trois**, en `:15-19` | **quatre** — `design.html` s'ajoute |
| 4 | « **aucun écran de connexion** […] la moitié navigateur de P2 est à venir » | **elle est arrivée** | et **elle a sa feuille** depuis la tâche 11 |
| 5 | « aucun jeton dans la poignée de main » | `shell-page.ts:62` en envoie un | inchangé par S1 |
| 6 | « le produit pèse **25 737** octets, dont **1 055** de CSS » | **31 701** / **1 429** | CSS : **3 503** octets, en **deux** actifs (`main-*.css` 1 493 + `socle-*.css` 2 010) |
| 7 | « la fenêtre de session contient exactement **quatre** éléments » | **cinq** | inchangé par S1 |
| 8 | « le plus gros fichier est `verify-webrtc.mjs` à **497** (marge 3) » | **494** (marge **6**) | **494** — **intouché par S1**, comme le plan l'exigeait |

---

## 6. Les tailles, relevées PAR LA COMMANDE, à `604f91c`

**Dépôt entier, fichiers de plus de 500 lignes — la table de dette est
INCHANGÉE, à deux lignes :**

```
1536 agent/src/encode.rs
 630 agent/src/windows_source.rs
```

**Aucun fichier de `client/` ne dépasse 500 lignes.** Les plus gros :

| Fichier | Lignes | Remarque |
| --- | --- | --- |
| `client/verify-webrtc.mjs` | **494** (marge **6**) | **intouché par S1** — la leçon de P2 (« une addition de commentaire peut annuler une extraction ») est respectée |
| `client/src/main.ts` | **451** | ⚠️ **du VOISIN** : 392 au relevé D10, porté là par le chantier E. Relevé, attribué, pas repris à notre compte |
| `client/src/webrtc.session.test.ts` | 419 | voisin |
| `client/src/micro.test.ts` | 411 | voisin |
| `client/recette/micro-e1.mjs` | 332 | voisin, né à `604f91c` |

**Les fichiers de S1**, tous sous la porte de 300 que la spec §10 arme :

| Fichier | Lignes |
| --- | --- |
| `client/outils/tokens-orphelins.mjs` | **233** |
| `client/design.html` | **231** |
| `client/outils/couleurs-litterales.mjs` | **228** |
| `client/src/design/tokens.css` | **199** |
| `client/src/design/galerie.ts` | **193** |
| `client/src/style.css` | **181** |
| `client/src/design/tokens.ts` | **176** |
| `client/src/design/theme.test.ts` | **156** |
| `client/src/design/contraste.ts` | **150** |
| `client/src/design/tokens.test.ts` | **133** |
| `client/src/design/contraste.test.ts` | **114** |
| `client/outils/surfaces-baties.mjs` | **109** |
| `client/vite.config.ts` | **106** |
| `client/src/design/theme.ts` | **100** |
| `client/src/design/reprise.test.ts` | **95** |
| `scripts/verify-all.sh` | **92** |
| `client/outils/poids-css.mjs` | **89** |
| `client/src/design/base.css` | **73** |
| `client/outils/verifier-design.mjs` | **60** |
| `client/outils/contraste.mjs` | **44** |
| `client/outils/blocs-de-theme.mjs` | **44** |
| `client/connexion.html` | **36** |
| `client/src/design/amorce-theme.js` | **35** |
| `client/index.html` | **26** |
| `client/shell.html` | **15** |
| `client/src/design/socle.css` | **8** |

**Poids CSS : 3 503 octets**, à opposer à la ligne de base **1 429** (avant S1)
et au plafond **12 288**. Marge **8 785**. La hausse de **+2 074** est la
palette entière, `base.css` et le socle partagé entre quatre pages.

⚠️ **Le `<style>` en ligne de `client/design.html` N'EST PAS COMPTÉ** par §7.7,
qui ne pèse que `dist/assets/*.css`. La galerie n'étant pas du produit, ce n'est
pas une lacune — mais c'est une portée, et elle est dite.

---

## 7. Ce que S1 n'établit PAS

- **Aucun taux.** Deux exécutions par contrôle, sur des contrôles
  **déterministes** : elles établissent la **reproductibilité**, rien de plus.
- 🔴 **Aucun jugement visuel n'a été porté sur aucune valeur.** Les **huit**
  jugements humains du §8 restent entiers : que la direction soit « sobre » et
  « pro », que `#7aa2f7` soit le bon bleu, que le ratio **1,2** soit le bon, que
  **14 px** soit assez dense, que le pas de **4 px** soit le bon, que `--bord`
  ait été employé là où il fallait, que le plafond de **12 Kio** soit au bon
  endroit, que la galerie montre ce qu'il faut regarder. **Aucun ne deviendra
  une mesure**, et ils rejoignent la liste déjà longue du dépôt — `BPP_MIN`,
  `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`, les
  paramètres `scrypt` de P2.
- **Que le thème atteigne les N fenêtres du PRODUIT** : la corroboration porte
  sur **deux fenêtres de la galerie**, pas sur une page-shell qui ouvre N
  sessions par `window.open`.
- **Rien de l'anti-FOUC OBSERVÉ.** L'amorce est prouvée **injectée** (`1` par
  page bâtie), **placée** (après `<meta charset>`, avant le module) et
  **exécutée** (par isolation sur `connexion.html`, qui n'a aucun module de
  thème). **Qu'aucun éclair de mauvais thème ne soit visible n'est mesuré par
  rien** — cela demanderait une capture chronométrée, que le §7.8 écarte.
- **Trois longueurs restent hors échelle** (D2), et **aucun contrôle ne mesure
  les longueurs**.
- **La reprise à l'identique vaut à racine 16 px, et là seulement** (D4).
- **Aucune primitive, aucune surface habillée** : c'est S2 et S3. `shell.html`
  et `connexion.html` reçoivent les **tokens**, pas des primitives — et leur
  rendu **change**, ce qui est l'objet, la neutralité n'étant promise que sur
  `index.html`.
- **Rien du manifest PWA, de `theme-color`, du Window Controls Overlay** : ils
  appartiennent à ②, et `client/` n'a aucun manifest.
- **Aucune vérification hors d'un Chromium de bureau** : rien de Firefox, de
  Safari, du mobile.
- **L'accessibilité au-delà du contraste** : navigation clavier complète,
  lecteurs d'écran, `prefers-reduced-motion`, cibles tactiles. **Le contraste
  est mesuré ; le reste ne l'est pas.** *`prefers-reduced-motion` est le moins
  cher des quatre et le premier à prendre* — nommé, non pris.
- **Rien du HiDPI** : `deviceScaleFactor` n'a pas été exercé. Legs ouvert depuis
  D9/D10.
- **Aucune internationalisation** : le produit est en français, et rien ne dit
  que la mise en page survit à une langue plus longue.
- **Le legacy n'est pas touché** et **aucun contrôle ne le balaie** : deux
  directions visuelles coexistent dans le dépôt, et coexisteront jusqu'au
  remplacement.
- **Les 28 tokens de la liste d'attente ne sont employés par rien**, donc rien
  n'établit qu'ils rendent bien — leur seule épreuve est arithmétique (§7.1).
- ⚠️ **La galerie n'a AUCUN test**, et son module `galerie.ts` non plus : ce
  qu'il aurait de testable (parser des tokens) vit dans `tokens.ts`, qui est
  testé. **Une galerie qui cesserait de rendre une famille entière ne serait
  attrapée par aucun contrôle** — seulement par l'œil, ce qui est précisément
  son statut d'instrument de jugement humain.

---

## 8. Les journaux versés

Tous dans `docs/superpowers/plans/journaux-design-s1/`.

⚠️ **UNE SEULE FAMILLE DE LECTURE, la plus simple de tous les sous-blocs.**
**Mesuré, pas supposé** : `grep -lP '\x1b\[' journaux-design-s1/*` rend **la
liste vide** — **aucune séquence ANSI**, dans aucun fichier. `file` rend
« UTF-8 text » ou « ASCII text » partout, et **aucun fichier ne porte de
`\r`** : pas de CRLF. **Ils se `grep`ent à plat, sans `sed`, sans `-a`.**

| Fichier | Ce qu'il porte |
| --- | --- |
| `commit.txt` | `604f91c`, le commit de toutes les mesures |
| `build-{1,2}.log` | les deux `npm run build` |
| `design-verifier-{1,2}.log` | les deux passes des six contrôles — **c'est la pièce principale** |
| `vitest-client-{1,2}.log` | 179 tests, deux fois |
| `vitest-proto-{1,2}.log` | 37 tests, deux fois |
| `typecheck-{1,2}.log` | `tsc --noEmit`, deux fois |
| `theme-7-5-{1,2}.log` | le contrôle §7.5 seul, 10 tests, deux fois |
| `verify-all.log` | les **dix** étapes, `exit=0` |
| `poids-css.txt` | les deux actifs CSS et leurs octets |
| `rouges-rejouees.log` | **les six rouges rejouées à la recette, verbatim, avec la remise en état vérifiée contre HEAD** |
| `corroboration-deux-fenetres.md` | la corroboration **HORS CRITÈRE** en navigateur réel |
