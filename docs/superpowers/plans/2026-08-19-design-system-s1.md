# Sous-bloc S1 — le socle : tokens, thèmes, amorce, et les sept contrôles : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** poser la source unique de valeurs visuelles du nouveau produit —
tokens en CSS, trois états de thème propagés entre N fenêtres, amorce
anti-FOUC injectée depuis une source unique, polices système à coût nul — et
**les sept contrôles qui empêchent ce socle de diverger**, dont deux sont
rouges sur l'arbre intact et se jouent **avant** d'écrire une ligne de style.

**Architecture:** trois couches, et une frontière nette entre elles.
① Les **règles pures** vivent en TypeScript sous `client/src/design/`
(parseur de tokens, calcul de contraste, machine à trois états du thème) :
aucun DOM, aucun `fs`, dépendances injectées — la pratique que
`client/src/status.ts:16-17` et `client/src/jeton.ts:4-10` ont déjà posée dans
ce paquet. ② Les **valeurs** vivent en CSS sous `client/src/design/`
(`tokens.css`, `base.css`, `socle.css`) : source unique, aucun miroir
TypeScript. ③ Les **contrôles** vivent sous `client/outils/` en `.mjs`, lisent
le disque, et **importent les règles pures du ① plutôt que de les recopier**.

**Tech Stack:** Vite 6.4.3, TypeScript 5.5, Vitest 2, Node v24.9.0 — **aucune
dépendance neuve**, ni de production ni de développement.

**Spec :** `docs/superpowers/specs/2026-08-19-design-system-design.md`
(commit `5b6b830`), §4, §6 « S1 », §7, §8, §10.

**Ce plan ne couvre QUE S1.** Aucune primitive d'interface (S2), aucune
surface habillée (S3), rien de la fenêtre de session au-delà du passage de ses
valeurs aux tokens (S4). En particulier : **aucun bouton, aucun champ, aucune
carte, aucun écran d'état terminal, aucun sélecteur de thème visible.**
`client/design.html` naît en S1 comme **galerie de tokens** et rien d'autre.

---

## Constantes de mesure — relevées le 19 août 2026, et à quel commit

🔴 **TOUT RELEVÉ DE CE PLAN PORTE SON COMMIT, parce que l'arbre est PARTAGÉ.**
Le chantier E (microphone) travaille dans `client/src/` et dans `agent/` **en
ce moment même**. Ce n'est pas une précaution de style : pendant la rédaction
de ce plan, `HEAD` est passé de **`d280745`** à **`8ad03a2`**, et
`client/src/webrtc.test.ts` — mesuré à **517 lignes**, donc **au-dessus du
plafond de 500**, sous `d280745` — a été scindé par le voisin
(`8ad03a2`, « remanie(e1): webrtc.test.ts franchit 500, ses tests de session
sortent ») en **127** + **419**. **Un chiffre relevé sans son commit est
invérifiable**, et celui-là aurait été faux en moins d'une heure.

**Sauf mention contraire, les relevés ci-dessous sont ceux de `8ad03a2`.**
✅ **Et ils tiennent encore à `cd049c5`**, deux commits plus loin : le voisin
a poursuivi dans `agent/` et `scripts/`, jamais dans `client/` —
`git diff --stat 8ad03a2..cd049c5 -- client/` rend **vide**. C'est vérifié, pas
supposé, et c'est la seule chose qui autorise à réutiliser ces chiffres.

| Grandeur | Valeur relevée | Commande |
| --- | --- | --- |
| `client/src/style.css` | **139** lignes | `wc -l` |
| `client/index.html` / `shell.html` / `connexion.html` | **21** / **14** / **35** | `wc -l` |
| entrées Vite déclarées | **trois** (`client/vite.config.ts:15-19`) | lecture |
| fichiers CSS du paquet client | **un seul**, `client/src/style.css` | `find client -name '*.css' -not -path '*/node_modules/*' -not -path '*/dist/*'` |
| couleurs littérales **employées comme valeur** dans `style.css` | **neuf** (l. 26, 35, 51, 68, 73, 101, 106, 114, 118) | `grep -nE '#[0-9a-fA-F]{3,8}\b\|rgb\(\|rgba\(\|hsl\(\|hsla\('` — 11 correspondances, dont 2 sont les déclarations de token l. 3-4 |
| poids de `dist/` | **31 701** octets | `npm run build && du -sb dist` |
| dont CSS | **1 429** octets (`dist/assets/main-nAqQO_Wk.css`) | `find dist/assets -name '*.css' -printf '%s\t%p\n'` |
| surfaces bâties **sans** feuille de style | **deux** : `dist/shell.html`, `dist/connexion.html` | boucle `grep -q 'rel="stylesheet"'` sur `dist/*.html` |
| tests `client` | **145** (16 fichiers) | `cd client && npx vitest run` |
| tests `proto` | **37** (2 fichiers) | `cd proto && npx vitest run` |
| `cd client && npm run typecheck` | **exit 0** | `tsc --noEmit` |
| étapes de `scripts/verify-all.sh` | **neuf** | `grep -c '^etape "' scripts/verify-all.sh` |
| fichiers de plus de 500 lignes, dépôt entier | **deux** — `agent/src/encode.rs` 1536, `agent/src/windows_source.rs` 630 | la commande de `CLAUDE.md` §« Conventions de code » |
| plus gros fichier de `client/` | `client/verify-webrtc.mjs` **494** (marge **6**) | idem, filtré `^client/` |
| Node / Vite | **v24.9.0** / **6.4.3** | `node --version`, `npx vite --version` |
| `client/node_modules/@types/` | **`estree` seul** — ni `@types/node`, ni `jsdom`, ni `happy-dom` | `ls` |

---

## Ce que la spec relève et qui est PÉRIMÉ — huit points, avec la valeur juste

La spec a été écrite pendant que le sous-bloc P2 livrait la moitié navigateur
de l'authentification, et pendant que le chantier E livrait le bouton micro.
**Elle n'est fausse nulle part par négligence : elle est datée d'avant.** Les
huit lignes ci-dessous sont corrigées ici **et nulle part ailleurs** — ce plan
ne modifie pas la spec.

| # | Ce que la spec écrit | Où | Ce que la commande rend (`8ad03a2`) |
| --- | --- | --- | --- |
| 1 | « Tout le style du nouveau produit tient en **85 lignes** » | §2.1 | **139** — le chantier E y a ajouté `#micro`, ses trois états et son encadré |
| 2 | « **cinq** valeurs littérales employées comme couleurs (l. 26, 35, 51, 68, 73) » | §2.4 | **neuf** — les quatre neuves sont l. **101**, **106**, **114** (`rgb(220 38 38 / 0.85)`), **118** (`rgb(120 53 15 / 0.85)`) |
| 3 | « `client/vite.config.ts:10-12` déclare exactement **deux** entrées » | §2.5 | **trois**, en `vite.config.ts:15-19` — `main`, `shell`, **`connexion`** |
| 4 | 🔴 « **aucun écran de connexion** […] la moitié navigateur de P2 est **à venir** » | §2.5 | **elle est arrivée** : `client/connexion.html` (35 l.) et `client/src/connexion.ts` existent et sont bâtis |
| 5 | « **aucun jeton dans la poignée de main** : `shell-page.ts:45` envoie encore `{ role: 'client', session }` » | §2.5 | `shell-page.ts:62` envoie `{ role: 'client', session: SESSION_DE_CONTROLE, **jeton** }` |
| 6 | « le produit entier pèse **25 737** octets, dont **1 055** de CSS » | §2.3 | **31 701**, dont **1 429** de CSS |
| 7 | « la fenêtre de session contient exactement **quatre** éléments (`index.html`, l. 10-13) » | §5.2 | **cinq** — `#remote` (:10), `#status` (:11), `#stats` (:12), **`#micro` (:17)**, `#fullscreen` (:18) |
| 8 | « le plus gros fichier est `client/verify-webrtc.mjs` à **497** (marge **3**) » | §10 | **494** (marge **6**) — la revue transverse de P2 l'a porté là après extraction |

**Deux citations de la spec ont en outre changé de ligne sans changer de
sens**, et sont rétablies ici pour que personne ne les recopie : `stats.ts:105`
est aujourd'hui `stats.ts:172`, et `main.ts:136` / `main.ts:405-406` sont
`main.ts:159` / `main.ts:449`. **Les citations `style.css:2`, `:3-4`, `:18`,
`:21`, `:26`, `:29`, `:31`, `:34`, `:35`, `:37`, `:45`, `:51`, `:52`, `:57`,
`:68`, `:73`, `:79-83` et `:84` de la spec sont EXACTES** — vérifiées une à
une : le chantier E a ajouté ses lignes **après** la l. 85, donc sans décaler
les précédentes. `client/src/status.ts:16-17` est exacte aussi.

⚠️ **Ce que ces huit points changent pour S1, en une phrase chacun** :

- le n°4 **avance** l'urgence que la spec §6.1 énonçait : l'écran de connexion
  n'est plus « à venir », il est **là et nu**, et il porte déjà un commentaire
  qui attend S1 (`client/connexion.html:9-13` : « AUCUNE DIRECTION VISUELLE
  ICI, et c'est délibéré : elle appartient au sous-projet ⑥ ») ;
- le n°3 fait passer le contrôle §7.3 de **une** surface fautive à **deux** ;
- le n°2 fait passer le contrôle §7.2 de **cinq** occurrences à **neuf**, et
  **deux d'entre elles ne sont pas des noirs** (`rgb(220 38 38 / 0.85)`,
  `rgb(120 53 15 / 0.85)`) : elles ne se promeuvent donc **pas** en
  `--voile-flottant`, contrairement aux sept autres. Voir la divergence D3.

---

## Divergences relevées entre la spec et le code réel, tranchées AVANT d'écrire une ligne

Onze points, trouvés en lisant et en mesurant. Chacun est tranché ici, avec son
coût, pour qu'aucune tâche ne les redécouvre à l'exécution.

### D1 — 🔴 La spec annonce TROIS contrôles rouges aujourd'hui, et n'en nomme que DEUX

Le §7 de la spec ouvre par : « Trois sont **rouges aujourd'hui sans rien
casser** — ce n'est pas une hypothèse, c'est le relevé du §2. »

**Relevé par `grep -n "ROUGE aujourd'hui\|rouges aujourd" spec`** : quatre
occurrences, dont une est la phrase d'annonce elle-même et une autre un renvoi
depuis §4.1. **Les contrôles nommément déclarés rouges aujourd'hui sont
exactement DEUX** : §7.2 (« ROUGE aujourd'hui, mesuré (§2.4) ») et §7.3
(« ROUGE aujourd'hui, mesuré (§2.2) »).

Les cinq autres portent une rouge **par perturbation** (« Retirer un token d'un
seul bloc », « Une faute de frappe dans un `var(--fond-O)` », « Ajouter un
`@font-face` »), ce qui est autre chose que « rouge sans rien casser ».

**Tranché — et c'est une correction, pas une lecture** :

| Contrôle | État sur l'arbre INTACT (`8ad03a2`) | Peut-il seulement TOURNER aujourd'hui ? |
| --- | --- | --- |
| §7.2 couleurs littérales | 🔴 **ROUGE, neuf occurrences** | **oui** — il balaie `client/src/`, l'exclusion de `tokens.css` est un no-op tant que le fichier n'existe pas |
| §7.3 surfaces bâties | 🔴 **ROUGE, sur ses DEUX assertions** — deux `dist/*.html` sans feuille, et aucun CSS émis ne déclare `--fond-0` | **oui** — il balaie `dist/` |
| §7.7 poids | 🟢 **VERT** — 1 429 < 12 288 | **oui** |
| §7.1 contraste | — | **non** : il parse `tokens.css`, qui n'existe pas |
| §7.4 égalité des trois blocs | — | **non** : même raison |
| §7.5 bascule de thème | — | **non** : `theme.ts` n'existe pas |
| §7.6 tokens orphelins | — | **non** : même raison que §7.1 |

**Le troisième rouge annoncé n'existe pas.** Quatre contrôles sur sept ne
peuvent pas rendre de verdict sur l'arbre d'aujourd'hui — non parce qu'ils
seraient verts, mais parce qu'**ils n'ont rien à lire**. Un script qui échoue
sur un fichier absent produit un plantage, pas une mesure ; **le compter comme
« rouge » serait exactement le déguisement que ce dépôt combat.**

⚠️ **Et c'est pourquoi l'ordre des tâches de ce plan est ce qu'il est** : les
trois contrôles qui **peuvent** tourner aujourd'hui (§7.2, §7.3, §7.7) sont
écrits **en premier**, Tasks 1 à 3, et joués **sur l'arbre intact**, avant
qu'une seule ligne de style n'ait bougé. C'est la « rouge gratuite » que P2 a
obtenue en sortant le service de P1 dans un `git worktree` : elle ne dépend
d'aucune modification de notre part, donc elle ne peut pas être flattée.

### D2 — 🔴 Trois longueurs d'aujourd'hui n'ont AUCUN cran dans les échelles de la spec

La spec promet deux choses qui, prises ensemble, ne tiennent pas :
« **S1 doit être visuellement quasi neutre sur `index.html`** » (§6, S1) et
« aucune longueur hors échelle » (§8, ligne du pas de 4 px).

**Inventaire complet des longueurs littérales de `client/src/style.css`**, par
`grep -noE '[0-9]+(\.[0-9]+)?(px|rem|em|s|ms)\b'` puis `uniq -c` :

| Littéral | Occ. | Lignes | Cran correspondant |
| --- | --- | --- | --- |
| `12px` | **12** | 31, 32, 33, 47, 48, 49, 53, 59, 60, 64, 91, 97 — dont quatre dans un `padding` | `--e-3` = 0.75rem **et** `--t-s` = 0.75rem ✅ |
| `6px` | **6** | 34, 50, 63, 96 (`border-radius`) ; **33, 49 (`padding: 6px 12px`)** | `--r-2` = 6px pour les quatre premiers ✅ ; 🔴 **AUCUN cran** pour les deux `padding` — l'échelle va 4, 8, 12, 16 |
| `8px` | **2** | 64, 97 | `--e-2` = 0.5rem ✅ |
| `18px` | **2** | 65, 98 | 🔴 **AUCUN cran** — l'échelle typographique donne 11, 12, 14, 16, 20, 24, 32 |
| `64px` | **1** | 93 | `--e-8` = 4rem ✅ |
| `14px` | **1** | 18 | `--t-m` = 0.875rem ✅ |
| `0.3s` | **1** | 37 | `--duree-2` = 300ms ✅ |
| `0.02em` | **1** | 54 | 🔴 **AUCUN token** — la spec ne déclare aucune famille de crénage |

**Tranché : les trois valeurs hors échelle restent des littéraux dans
`style.css`, et elles sont DÉCLARÉES par un commentaire qui dit pourquoi.**

*Justification, et elle est écrite dans le code d'aujourd'hui.*
`client/src/style.css:79-83` justifie le `pointer-events: none` de la l. 84 par
une géométrie chiffrée — « une zone de ~40×36 px en bas à droite […] avale les
clics destinés au jeu ». Cette zone est le produit de `padding: 8px 12px` et
`font-size: 18px`. **Aligner `18px` sur `--t-l` (16px) ou `--t-xl` (20px)
changerait la zone**, donc invaliderait une justification écrite ; et la spec
§5.2 l'interdit nommément (« Toute reprise du bouton en S4 doit conserver cette
propriété ; changer sa taille change la zone qu'il occupe »). Le `6px` vertical
des deux bandeaux et le `0.02em` de `#stats` tombent sous la même règle : S1
est **quasi neutre**, et une neutralité qui bougerait trois longueurs n'en est
pas une.

*Coût, assumé.* Le §8 de la spec compte déjà « que le pas de **4 px**
d'espacement soit le bon » parmi ses jugements humains, en écrivant que ce qui
est mesuré à la place est « qu'il soit **unique** — aucune longueur hors
échelle ». **Cette dernière clause est fausse à la fin de S1**, et de trois
valeurs exactement. Elle le restera jusqu'à S4, qui est le sous-bloc qui a le
droit de toucher ces éléments. **Aucun contrôle automatisé n'en souffre** : les
sept contrôles portent sur les couleurs, les contrastes, les ensembles de noms,
la bascule et le poids — **aucun ne mesure les longueurs**. C'est une dette
d'énoncé, nommée, pas une dette de code.

### D3 — Deux des neuf couleurs littérales NE SONT PAS des noirs

La spec (§4.5) prévoit deux tokens hors thème pour absorber les littéraux :
`--video-letterbox: #000` et `--voile-flottant: rgb(0 0 0 / 0.72)`. Ils
couvrent l. 26, 35, 51 — et, par extension, les opacités voisines 0.55 et 0.75
des l. 68, 73, 101, 106.

**Ils ne couvrent pas `style.css:114` `rgb(220 38 38 / 0.85)` ni `:118`
`rgb(120 53 15 / 0.85)`** : ce sont les états `actif` et `refuse` du bouton
micro, et le commentaire qui les précède (`style.css:109-113`) dit qu'ils
existent pour que l'état de capture « se voie sans ambiguïté, et de loin ».

**Tranché : la couche de voiles gagne cinq tokens hors thème, pas deux**, et
l'arbitrage est de **reprendre les valeurs à l'identique**, jamais de les
raccorder à `--danger` / `--alerte` :

| Token | Valeur, reprise **verbatim** | Remplace |
| --- | --- | --- |
| `--video-letterbox` | `#000` | `:26` |
| `--voile-flottant` | `rgb(0 0 0 / 0.72)` | `:35`, `:51` |
| `--voile-bouton` | `rgb(0 0 0 / 0.55)` | `:68`, `:101` |
| `--voile-bouton-survol` | `rgb(0 0 0 / 0.75)` | `:73`, `:106` |
| `--voile-micro-actif` | `rgb(220 38 38 / 0.85)` | `:114` |
| `--voile-micro-refuse` | `rgb(120 53 15 / 0.85)` | `:118` |

*Pourquoi PAS `--danger` et `--alerte`.* `--danger` sombre vaut `#f07a7a` et
clair `#c02b2b` (spec §4.5) : les employer ferait **changer de couleur** le
bouton micro, et sa couleur suivrait le thème du produit alors qu'elle est
posée sur une vidéo dont le contenu ne suit aucun thème (spec §5.1). Ce serait
un changement d'apparence, que S1 s'interdit. **Le raccordement sémantique du
bouton micro appartient à S4**, qui reprend les trois bandeaux sur les
primitives « message ».

⚠️ **Conséquence sur §7.1 :** ces six tokens sont **hors thème**, donc **hors
des 50 paires de contraste** — comme la spec le prévoit déjà pour les deux
qu'elle nommait. Leur lisibilité sur une vidéo quelconque n'est garantie par
rien, et la spec §11 le déclare déjà (« `--voile-flottant` ne suffit pas sur
une vidéo très claire — non mesuré »). **S1 n'améliore ni n'aggrave ce point.**

### D4 — 🔴 `--e-3` ne vaut 12 px QUE si `html` perd son `font-size`, et « à l'identique » a une portée

La spec (§4.4) écrit : « `--e-3` vaut exactement les 12 px des décalages
actuels des bandeaux (`style.css:31` et suivantes) : la reprise est à
l'identique, pas à l'approximation. »

**`--e-3` vaut `0.75rem`, et `rem` se mesure sur la racine.** Or
`client/src/style.css:18` porte `font: 14px/1.5 …` dans un bloc dont le
sélecteur est `html, body` (l. 11-12) : **aujourd'hui, `html` a une taille de
police de 14 px**, donc `1rem` = 14 px et `0.75rem` = **10,5 px** — pas 12.

**Tranché, et c'est une dépendance porteuse entre deux tâches** : la Task 8
(`base.css`) doit **retirer `html` du sélecteur qui porte la taille de police**
et ne la poser que sur `body`, laissant la racine au défaut du navigateur.
C'est ce que la spec §4.4 exige par ailleurs (« la racine n'est JAMAIS forcée »,
« Pas de `html { font-size: 62.5% }`, pas de `font-size` en pixels sur
`<html>` ») — mais elle ne dit nulle part que **le chiffre 12 de son propre
tableau en dépend**. **Si la Task 9 passait aux tokens avant que la Task 8
n'ait libéré la racine, tous les décalages des bandeaux passeraient de 12 à
10,5 px, et rien ne le dirait.**

⚠️ **Et « à l'identique » a une portée qu'il faut écrire** : la reprise est
identique **à racine 16 px, c'est-à-dire au réglage par défaut du navigateur, et
là seulement**. Pour un utilisateur qui a agrandi sa police, les décalages
d'aujourd'hui (`12px` absolus) **ne bougent pas** tandis que ceux de demain
(`0.75rem`) **grandissent avec son réglage**. **C'est le changement de
comportement voulu** — c'est l'argument d'accessibilité du §4.4 — mais c'en est
un, et le dire « à l'identique » sans réserve serait faux.

### D5 — La clé `localStorage` : la spec dit `theme`, le dépôt a déjà une convention

La spec (§4.2) écrit : « L'état vit dans `localStorage`, sous la clé `theme` ».

Le dépôt en a déjà deux, posées par P2 : `client/src/jeton.ts:37-38` déclare
`CLE_ACCES = 'guac.jeton.acces'` et `CLE_RAFRAICHISSEMENT =
'guac.jeton.rafraichissement'`.

**Tranché : la clé est `guac.theme`.** Une clé nue `theme` sur la même origine
que le hub, la page-shell et N fenêtres de session est une collision qui
attend ; le préfixe existe déjà et ne coûte rien.

🔵 **Et cette divergence en révèle une propriété utile**, que la spec ne pouvait
pas connaître : le §7.5 prévoit un cas de robustesse « un `storage` sur une
**autre** clé ne fait rien ». **Ce cas n'est plus hypothétique.**
`client/src/connexion.ts:57` écrit les deux clés de jeton au moment de la
connexion — donc **toute fenêtre voisine reçoit réellement un événement
`storage` portant `guac.jeton.acces`**, et un gestionnaire de thème qui ne
filtrerait pas la clé poserait `data-theme` à partir d'un JWT. **Le test de
robustesse doit employer cette clé-là, pas une clé inventée.**

### D6 — 🔵 Les contrôles peuvent importer les règles typées : mesuré, sans dépendance

Le §7.1 de la spec pose le point de conception : le contrôle de contraste
« **parse `tokens.css`** — il ne recopie aucune valeur », parce qu'« un contrôle
qui a sa propre copie des valeurs valide sa copie ». La même exigence vaut pour
§7.4 et §7.6, qui parsent le même fichier.

Restait à savoir **où vit ce parseur**. `client/` n'a ni `tsx`, ni `ts-node`, ni
`@types/node` (`ls client/node_modules/@types/` rend **`estree` seul**) : un
outil `.mjs` ne peut pas, en principe, importer un module `.ts`.

**Mesuré le 19 août 2026 sur Node v24.9.0 — il le peut, nativement :**

```
$ node runner.mjs          # runner.mjs fait: import { lireTokens } from './regle.ts'
tokens: [ [ '--fond-0', '#0b0d10' ], [ '--e-3', '0.75rem' ] ]
exit=0
```

**Tranché : les règles pures vivent en `.ts` sous `client/src/design/`** — donc
typecheckées par `npm run typecheck` et testées par la suite Vitest existante —
**et les scripts `client/outils/*.mjs` les importent.** Aucune dépendance
neuve, aucune copie des valeurs, aucun parseur en double.

⚠️ **Le coût, mesuré lui aussi : le retrait de types NE TYPECHECKE PAS, et il
refuse le TypeScript non effaçable.** Un `export enum T { A, B }` importé de la
même façon fait planter Node :

```
$ node runenum.mjs
.../enum.ts:1
export enum T { A, B }
```

**Les modules de `client/src/design/` importés par un outil doivent donc rester
en TypeScript « effaçable »** : pas d'`enum`, pas de `namespace`, pas de
propriétés de constructeur, pas de décorateurs. C'est une contrainte réelle, et
elle est facile à respecter — le reste de `client/src/` l'est déjà, par accident
plutôt que par règle. Un commentaire de tête de chaque module concerné la porte.

### D7 — 🔴 `client/vite.config.ts` n'est PAS typechecké, et S1 y met du code

`client/tsconfig.json:12` déclare `"include": ["src/**/*.ts",
"../proto/ts/**/*.ts"]`. **`client/vite.config.ts` n'est sous aucun des deux
globs** : `npm run typecheck` ne le voit pas.

**Tranché, et non corrigé** : le greffon `transformIndexHtml` de la Task 10 est
validé **par `npm run build` seul**. Élargir l'`include` à la racine du paquet
attraperait aussi `verify-webrtc.mjs` et `recette/*.mjs`, qui sont du
JavaScript sans types — un chantier de typage qui n'est pas celui-ci.
**Conséquence à écrire dans la tâche** : une erreur de type dans le greffon se
manifeste comme un échec de build, jamais comme une erreur `tsc`, et le §7.3
est ce qui rattrape un greffon qui « marche » sans injecter.

### D8 — Le mot-clé `red` du contrôle §7.2 lève un faux positif sur du français

Le §7.2 prescrit de chercher, entre autres, « les mots-clés
`black`/`white`/`red` ».

**Mesuré sur l'arbre intact :**

```
$ grep -rnoE 'black|white|\bred\b' client/src --include="*.css" --include="*.ts"
client/src/resize.ts:7:red
client/src/resize.test.ts:30:red
```

`client/src/resize.ts:7` est un commentaire :
« *re**d**éclenche que si l'élément change encore de taille* ». Le `\b` après
`red` est satisfait parce que le caractère suivant est `é`, que `grep` ne
compte pas comme caractère de mot.

**Tranché : le contrôle §7.2 ne balaie pas le texte brut ; il retire d'abord
les commentaires**, puis ne cherche les mots-clés que **du côté valeur** d'une
déclaration CSS (`propriété: …;`). Les notations `#rrggbb`, `rgb(`, `hsl(`
restent, elles, cherchées partout — aucune ne collisionne avec du français.

⚠️ **C'est une exigence d'atteignabilité, pas de confort** : un contrôle qui
naît rouge sur deux commentaires innocents est un contrôle qu'on assouplira, et
la spec §11 nomme précisément ce mode de défaillance (« Le contrôle §7.2
devient pénible et on l'assouplit »). **La Task 1 doit prouver que le contrôle
corrigé rend bien 9, et non 11.**

### D9 — `color-scheme: dark` en dur doit devenir dynamique, et l'écran de connexion le rend visible

`client/src/style.css:2` porte `color-scheme: dark`, et c'est la **seule**
occurrence de tout mécanisme de thème dans `client/` :

```
$ grep -rn "prefers-color-scheme\|data-theme\|color-scheme" client \
    --include="*.ts" --include="*.css" --include="*.html" | grep -v node_modules | grep -v "/dist/"
client/src/style.css:2:    color-scheme: dark;
```

**Tranché : `color-scheme` suit le thème**, déclaré dans les trois blocs de
`tokens.css` au même titre qu'une couleur — `dark` dans `:root`, `light` dans
la requête média et dans `[data-theme="clair"]`.

*Pourquoi cela compte davantage qu'hier* : `client/connexion.html:16-30` porte
deux `<input>` (dont un `type="password"`) et un `<button>`. **`color-scheme`
est ce qui décide de l'apparence native de ces contrôles** — fond, texte, coche
de remplissage automatique. Une page claire dont les champs restent en rendu
sombre est le défaut le plus visible que S1 puisse introduire, et il n'est
attrapé par aucun des sept contrôles : il appartient à la galerie et au
jugement humain (§8).

⚠️ **`color-scheme` n'est PAS un token `--*`** : il ne compte donc ni dans
l'égalité d'ensembles de §7.4, ni dans les orphelins de §7.6. Sa présence dans
les trois blocs est vérifiée **par un test de `tokens.ts`**, à part.

### D10 — L'arbre est partagé, et `client/src/style.css` est le fichier du voisin

Le chantier E (microphone) a modifié `client/src/style.css` au commit `7e9b438`
(« ajoute(e1): le bouton micro, ses etats, et les mesures montantes ») et a
scindé `client/src/webrtc.test.ts` au commit `8ad03a2`, **pendant la rédaction
de ce plan**.

**Tranché, trois règles d'exécution** :

1. **Jamais `git add -A`, jamais `git add client/src`** : nommer les fichiers,
   un par un. Ce dépôt a déjà eu un commit qui ne compilait pas pour l'avoir
   fait.
2. **La Task 9 (passage de `style.css` aux tokens) est celle qui risque le
   conflit.** Elle relit le fichier au moment de l'écrire, et **elle promeut
   toutes les couleurs littérales qu'elle y trouve, pas les neuf de ce plan**.
   Si le voisin en a ajouté, elles se promeuvent aussi — et le contrôle §7.2
   est ce qui l'établit, pas l'inventaire de ce document.
3. **Tout compte de tests, de lignes ou d'octets porté dans un rapport ou dans
   `CLAUDE.md` porte son `git rev-parse --short HEAD`.**

### D11 — La galerie est une entrée Vite de plus, et le contrôle §7.6 doit l'exclure — mais pas §7.3

La spec le dit pour §7.6 (« La galerie `client/design.html` est **EXCLUE** de la
moitié "employé" […] l'inclure rendrait ce contrôle **incapable d'échouer** »).
Elle ne dit rien de §7.3.

**Tranché : la galerie est INCLUSE dans §7.3** — c'est une surface bâtie comme
les autres, elle doit charger les tokens, et l'y inclure ne peut rien flatter
(§7.3 vérifie qu'une page **charge** les tokens, pas qu'elle les emploie).
**Elle est EXCLUE de §7.2 ?** Non : la galerie ne doit écrire aucune couleur
littérale non plus, sans quoi elle montrerait autre chose que les tokens. Elle
est donc **incluse partout sauf dans la moitié « employé » de §7.6**, et
l'exclusion est écrite **au seul endroit qui la justifie**.

⚠️ **`client/probe-coalesced.html`, `client/recette/latency-test.html` et
`client/recette/scroll-test.html` existent et ne sont PAS des entrées Vite** :
ils ne sortent pas dans `dist/`, donc §7.3 ne les voit pas, et ils ne reçoivent
pas l'amorce. C'est correct — ce sont des instruments de banc, hors produit —
et `client/vite.config.ts:9-13` porte déjà l'avertissement qui l'explique.

---

## Global Constraints

- **Plafond de 500 lignes** (`CLAUDE.md`, §« Conventions de code », dont le
  § Portée liste `client/src/`). **Relevé par la commande à `8ad03a2`** : le
  dépôt entier n'a que **deux** fichiers au-dessus — les deux lignes de la
  dette gelée —, et le plus gros de `client/` est `client/verify-webrtc.mjs` à
  **494** (marge **6**).
  ⚠️ **`client/verify-webrtc.mjs` n'est touché par AUCUNE tâche de ce plan**,
  et il ne doit pas l'être. La leçon de P2 est fraîche et tient en une phrase :
  « **une addition de commentaire peut annuler une extraction** » — la revue
  transverse de P2 a failli y ramener le fichier **exactement à 497**, le
  chiffre d'avant l'extraction que P2 venait de payer.
  **Portes armées d'avance** pour les fichiers que S1 fait naître, d'après les
  tailles prévues de la spec §10 : `design/tokens.css` **à 300 lignes**
  (scinder en `tokens/couleurs.css` + `tokens/echelles.css`),
  `client/design.html` **à 300** (scinder par famille), `style.css` **à 300**
  (sortir la mise en page des bandeaux dans `session/bandeaux.css`).
  **Le seuil d'action est à 300, jamais à 500** — c'est la décision de la spec
  §10, prise parce que ce dépôt a franchi le plafond **trois fois en D10 et
  deux fois en D9**, et l'a rattrapé chaque fois **après**, dont deux fois par
  une **compression** que `CLAUDE.md` interdit nommément.
- **Jamais `git add -A`** — voir D10.
- ⛔ **Aucune tâche de ce plan n'emploie la VM Windows.** Un agent concurrent y
  conduit le chantier microphone. Ce n'est pas une gêne : ⑥ est un sous-projet
  navigateur, et la spec §9 déclare déjà qu'il n'a **aucune recette sur VM**.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** son implémentation et **vu échouer**, et le
  message d'échec attendu est écrit dans la tâche.
  🔴 **Et une rouge ne vaut que pour l'assertion qu'elle fait tomber.** P2 a
  payé exactement cela : son plan annonçait qu'une rouge ferait tomber « les
  **DEUX** assertions » d'un critère ; **elle n'en faisait tomber qu'une**,
  parce que `expect` interrompt le test à la première — si bien que la seconde
  n'était éprouvée par rien, et qu'il a fallu jouer une huitième rouge pour
  elle seule. **Dans ce plan, tout critère à deux assertions se joue en DEUX
  rouges, une par assertion**, et cela vaut nommément pour §7.3 (lien /
  déclaration) et pour §7.5 (fenêtre écrivante / fenêtre voisine).
  ⚠️ **Ce plan est une source de contrôles vacueux comme les autres** : D10 en a
  attrapé **quatre**, dont **trois écrits par le plan lui-même**. Le doute
  porte sur ce document.
- **Aucun taux ne sera revendiqué.** Chaque énoncé de résultat porte son nombre
  d'exécutions. Règle de recette : **deux exécutions par critère, pas une.**
  ⚠️ Les sept contrôles de S1 sont **déterministes** : deux exécutions y
  établissent la reproductibilité, jamais une fréquence — et la spec §9
  l'écrit (« La question "combien de fois sur combien" ne se pose pas — et **ne
  doit pas être empruntée** à une campagne qui, elle, l'aurait posée »).
- **Aucune dépendance neuve**, ni de production ni de développement. C'est le
  principe §4.2 du cadrage, écrit après le naufrage de `fuse-native`, et P2 en a
  fourni le témoin en livrant un sous-bloc entier d'authentification à
  dépendances constantes. Le calcul WCAG est une vingtaine de lignes
  d'arithmétique ; le parseur de tokens, une expression régulière.
- **La suite existante reste verte, et le compte est nommé d'avance** :
  **145** tests `client` et **37** tests `proto` à `8ad03a2`. S1 **ajoute** des
  tests et n'en retire aucun ; chaque tâche qui en ajoute **annonce le compte
  attendu AVANT de le mesurer**. *(C'est le geste qui a permis à un
  implémenteur de D10 de s'apercevoir qu'un `Write` avait écrasé un test d'une
  tâche antérieure : le compte est sorti à 452 au lieu des 453 annoncés.)*
- 🔴 **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`** : la racine
  Vitest est `client/`, et il n'y a **aucun `vitest.config.*`** dans le paquet.
  Un test posé hors de `client/` ne tournerait pas, **et personne ne le
  verrait**. Tous les tests de S1 vivent sous `client/src/design/`.
- **La référence de vérification est `scripts/verify-all.sh`**, qui compte
  **neuf** étapes à `8ad03a2` (`grep -c '^etape "'`). ⚠️ **Ce plan ne prétend
  PAS qu'il sort à 0 aujourd'hui** : il n'a pas été lancé pendant la rédaction,
  parce que sa première étape est `cargo test --workspace` sur un `agent/` que
  le chantier voisin modifie en ce moment — et P2 a précisément vu son témoin
  échouer à cette étape « sur un test Rust du voisin ». Ce qui **est** relevé,
  et qui est de notre périmètre : `client` **145 passed**, `proto` **37
  passed**, `client typecheck` **exit 0**.
- **L'ancien code est hors périmètre** : `assets/`, `web/`, `src/`, `index.js`
  ne sont ni lus ni modifiés, et **aucun contrôle ne les balaie** (spec §4.6 :
  un contrôle qui ne peut pas devenir vert n'est pas un contrôle).

---

## Structure des fichiers

```
client/
  design.html                        NEUF  — la galerie, 4ᵉ entrée Vite (T12)
  vite.config.ts                     MOD   — le greffon d'amorce (T10)
  package.json                       MOD   — le script `design:verifier` (T13)
  index.html                         MOD   — lie le socle en plus de style.css (T11)
  shell.html                         MOD   — reçoit sa feuille, pour la 1ʳᵉ fois (T11)
  connexion.html                     MOD   — idem (T11)
  src/
    style.css                        MOD   — cesse de déclarer des couleurs (T9)
    design/
      tokens.css                     NEUF  — LA source unique de valeurs (T7)
      base.css                       NEUF  — remise à zéro, défauts d'élément (T8)
      socle.css                      NEUF  — @import tokens + base (T8)
      amorce-theme.js                NEUF  — le script en ligne, source unique (T10)
      tokens.ts                      NEUF  — parseur + égalité d'ensembles (T4)
      tokens.test.ts                 NEUF
      contraste.ts                   NEUF  — WCAG 2.1 + les 50 paires (T5)
      contraste.test.ts              NEUF
      theme.ts                       NEUF  — les trois états (T6)
      theme.test.ts                  NEUF
  outils/
    couleurs-litterales.mjs          NEUF  — §7.2 (T1)
    surfaces-baties.mjs              NEUF  — §7.3 (T2)
    poids-css.mjs                    NEUF  — §7.7 (T3)
    contraste.mjs                    NEUF  — §7.1, importe design/contraste.ts (T5)
    blocs-de-theme.mjs               NEUF  — §7.4, importe design/tokens.ts (T7)
    tokens-orphelins.mjs             NEUF  — §7.6, importe design/tokens.ts (T12)
    verifier-design.mjs              NEUF  — lance les six, sort non nul au 1ᵉʳ échec (T13)
scripts/verify-all.sh                MOD   — une dixième étape (T13)
docs/superpowers/plans/
  2026-08-19-design-system-s1-resultats.md      NEUF (T14)
  journaux-design-system-s1/                    NEUF (T14)
```

⚠️ **`client/outils/` est du `.mjs` non typechecké**, comme `client/recette/`
qui existe déjà. C'est délibéré : ces scripts n'ont pas de logique — **toute
règle qu'ils porteraient doit descendre dans `client/src/design/*.ts`**, qui
est typechecké et testé. C'est mot pour mot la clause que
`client/src/connexion.ts:5-9` s'applique à lui-même (« toute règle que ce
fichier porterait doit descendre dans `jeton.ts`, qui est testé »). **Si une
condition apparaît dans un `.mjs`, c'est qu'elle est au mauvais endroit.**

---

## Interfaces partagées

```ts
// client/src/design/tokens.ts — PUR, sans DOM, sans fs, EFFAÇABLE (D6)
export interface BlocDeTheme {
    /// 'racine' | 'media-clair' | 'attribut-clair'
    nom: string;
    /// nom de token (avec les deux tirets) → valeur littérale, telle qu'écrite
    tokens: Map<string, string>;
}
/// Découpe le texte de `tokens.css` en ses trois blocs de thème.
export function lireBlocsDeTheme(css: string): BlocDeTheme[];
/// Tous les tokens déclarés, quel que soit le bloc.
export function tokensDeclares(css: string): Set<string>;
/// Tous les `var(--…)` référencés par un texte CSS.
export function tokensReferences(css: string): Set<string>;
/// Les écarts d'ensembles entre blocs, dans les DEUX sens. Vide = conforme.
export function ecartsEntreBlocs(blocs: BlocDeTheme[]): string[];

// client/src/design/contraste.ts — PUR, EFFAÇABLE
export interface Paire { theme: string; encre: string; fond: string; seuil: number; }
export function luminanceRelative(couleur: string): number;
export function rapportDeContraste(a: string, b: string): number;
/// Les 50 paires DÉCLARÉES (spec §4.5), jamais un produit cartésien.
export const PAIRES: readonly Paire[];
export interface Echec { paire: Paire; rapport: number; }
export function evaluer(blocs: BlocDeTheme[]): { verifiees: number; echecs: Echec[]; minimum: number };

// client/src/design/theme.ts — PUR, dépendances INJECTÉES, sans DOM
export type Theme = 'systeme' | 'clair' | 'sombre';
export const CLE_THEME = 'guac.theme';          // D5
export interface Coffre { getItem(c: string): string | null; setItem(c: string, v: string): void; }
export interface Racine { setAttribute(n: string, v: string): void; removeAttribute(n: string): void; }
/// Lit le coffre. Toute valeur inconnue — y compris `null` — rend 'systeme'.
export function themeStocke(coffre: Coffre): Theme;
/// Pose `data-theme`, ou le RETIRE pour 'systeme' (son absence SIGNIFIE 'systeme').
export function appliquer(racine: Racine, theme: Theme): void;
/// Écrit ET applique localement. `storage` ne se déclenche pas chez l'écrivain.
export function choisir(coffre: Coffre, racine: Racine, theme: Theme): void;
/// Réagit à un événement `storage` d'une AUTRE fenêtre. N'écrit RIEN.
export function surStockageModifie(racine: Racine, cle: string | null, valeur: string | null): void;
```

⚠️ **`appliquer(racine, 'systeme')` RETIRE l'attribut**, il ne pose pas
`data-theme="systeme"`. C'est ce que la structure CSS de la spec §4.2 exige :
le sélecteur de la requête média est `:root:not([data-theme="sombre"])`, et un
`data-theme="systeme"` y passerait — mais `:root[data-theme="clair"]` ne
s'appliquerait pas, et rien ne casserait **visiblement**. Un attribut inventé
qui ne casse rien est exactement le genre d'écart qui survit dix sous-blocs.
**Un test de `theme.ts` l'assure, et sa rouge est de poser l'attribut.**

---

# Famille ⓿ — les trois contrôles qui peuvent être rouges AUJOURD'HUI

**Ces trois tâches ne modifient AUCUN fichier de style.** Elles écrivent les
contrôles et les jouent sur l'arbre intact, pour que leur rouge ne doive rien à
une casse volontaire. C'est la « rouge gratuite » de P2 (`git worktree add` du
service de P1), transposée : ici il n'y a même pas de worktree à faire, l'état
rouge **est** l'état du dépôt.

### Task 1 : `§7.2` — aucune couleur littérale hors de `tokens.css`, ROUGE sur l'arbre intact

**Objet :** écrire le balayage des couleurs littérales, et le voir rendre
**neuf** sur `client/src/` intact — pas onze, pas cinq.

**Files:**
- Create: `client/outils/couleurs-litterales.mjs`
- Modify: aucun

**Interfaces:**
- Consumes: rien.
- Produces: un exécutable de contrôle, sortie non nulle si une occurrence.

- [ ] **Step 1 : relever l'état AVANT, et le verser**

```bash
cd /home/mallanic/Projects/Guacamole
git rev-parse --short HEAD
grep -nE '#[0-9a-fA-F]{3,8}\b|rgb\(|rgba\(|hsl\(|hsla\(' client/src/style.css
```
**Attendu (relevé à `8ad03a2`) : onze correspondances**, dont les l. 3 et 4
sont les déclarations `--surface` et `--text` — donc **neuf** valeurs
littérales employées comme couleur, aux l. **26, 35, 51, 68, 73, 101, 106,
114, 118**. Si le compte diffère, c'est que le chantier voisin a bougé : **le
noter, et continuer avec le compte relevé** (D10).

- [ ] **Step 2 : écrire le contrôle, avec le traitement des commentaires que D8 impose**

`client/outils/couleurs-litterales.mjs` :

- **périmètre** : `client/src/**/*.{css,ts}` et `client/*.html`, **moins**
  `client/src/design/tokens.css` ; les fichiers `*.test.ts` sont **inclus**
  (une couleur codée en dur dans un test est une valeur en double comme une
  autre) ;
- **retire d'abord les commentaires** (`/* … */` pour CSS, `//` et `/* … */`
  pour TS) — voir D8 ;
- **cherche partout** : `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(`, `rgba(`,
  `hsl(`, `hsla(` ;
- **cherche du seul côté valeur** d'une déclaration : `black`, `white`, `red` ;
- **autorise** : `currentColor`, `transparent`, `inherit`, `none`, `var(--…)` ;
- **imprime chaque occurrence en `fichier:ligne: texte`**, puis le total, et
  sort **1** s'il y en a.

⚠️ **L'en-tête du fichier porte la raison du traitement des commentaires**, avec
la sortie de `grep` de D8 recopiée : sans elle, le prochain lecteur simplifiera
le script et réintroduira les deux faux positifs.

- [ ] **Step 3 : la ROUGE, qui est l'état du dépôt**

```bash
node client/outils/couleurs-litterales.mjs; echo "exit=$?"
```
**Attendu : `exit=1`, et exactement neuf occurrences listées**, aux lignes du
Step 1. **Le compte est le contrôle** : s'il rend **onze**, le traitement des
commentaires n'a pas été fait et le script attrape `resize.ts:7` ; s'il rend
**cinq**, il ne balaie pas au-delà de la l. 85 et rate le bouton micro.

- [ ] **Step 4 : prouver que le script peut aussi rendre VERT**

Un contrôle qui ne rendrait *jamais* vert serait aussi vain qu'un contrôle qui
ne rendrait jamais rouge, et rien dans les Steps précédents ne l'établit —
`client/src/` est rouge de bout en bout.

```bash
mkdir -p /tmp/s1-vert/src && printf ':root { color: var(--fond-0); }\n' > /tmp/s1-vert/src/a.css
node client/outils/couleurs-litterales.mjs --racine /tmp/s1-vert; echo "exit=$?"
```
**Attendu : `exit=0`, zéro occurrence.** Le drapeau `--racine` existe pour ce
seul usage et il est documenté comme tel.

- [ ] **Step 5 : commit**

```bash
git add client/outils/couleurs-litterales.mjs
git commit -m "design(s1): le controle des couleurs litterales, rouge sur l'arbre intact" \
  -- client/outils/couleurs-litterales.mjs
```

---

### Task 2 : `§7.3` — toute surface bâtie porte les tokens, et DEUX rouges pour DEUX assertions

**Objet :** écrire le contrôle des surfaces bâties, et le voir rouge sur
**chacune de ses deux assertions séparément**.

**Files:**
- Create: `client/outils/surfaces-baties.mjs`

- [ ] **Step 1 : relever l'état AVANT**

```bash
cd client && npm run build >/dev/null 2>&1
for f in dist/*.html; do grep -q 'rel="stylesheet"' "$f" && echo "OK   $f" || echo "SANS $f"; done
grep -l -- '--fond-0' dist/assets/*.css; echo "fond-0 trouve dans: $?"
```
**Attendu (relevé à `8ad03a2`)** :
```
SANS dist/connexion.html
OK   dist/index.html
SANS dist/shell.html
```
et **aucun** CSS ne déclare `--fond-0`. **Deux surfaces fautives, pas une** —
la spec §2.2 n'en connaissait qu'une (voir le tableau des relevés périmés,
n°3 et n°4).

- [ ] **Step 2 : écrire le contrôle, avec sa PORTÉE renforcée**

La spec prescrit « chaque `dist/*.html` doit référencer une feuille de style,
et **l'ensemble** des CSS émis doit contenir la déclaration `--fond-0` ».

⚠️ **Pris à la lettre, la seconde assertion est plus faible qu'elle n'en a
l'air** : elle porte sur l'**union** de tous les CSS émis. Une page pourrait
lier une feuille sans tokens et passer, du moment qu'*une autre* page en lie
une qui en a. **Le contrôle écrit ici résout les liens page par page** :

1. **assertion A** — chaque `dist/*.html` porte au moins un
   `<link rel="stylesheet" href="…">` ;
2. **assertion B** — pour **chaque** page, **au moins une des feuilles qu'elle
   lie** déclare `--fond-0`.

C'est strictement plus fort que la lettre de la spec, et cela ne coûte qu'une
résolution de `href` relatif. **Les deux assertions s'impriment séparément et
sont comptées séparément** — voir la contrainte globale sur les rouges.

*Portée honnête, à écrire dans l'en-tête du script* : le contrôle vérifie
qu'une surface **charge** les tokens, pas qu'elle les **emploie**. Une page qui
chargerait la feuille et écrirait ses propres couleurs passerait §7.3 et
tomberait sur §7.2. **Les deux se complètent ; aucun ne suffit.**

- [ ] **Step 3 : les DEUX rouges, séparément**

```bash
cd client && npm run build >/dev/null 2>&1 && cd ..
node client/outils/surfaces-baties.mjs; echo "exit=$?"
```
**Attendu : `exit=1`**, et le rapport doit nommer **les deux assertions
séparément** :
- **A rouge** : `dist/shell.html` et `dist/connexion.html`, aucun lien ;
- **B rouge** : `dist/index.html` lie bien `main-*.css`, mais **aucune** de ses
  feuilles ne déclare `--fond-0`.

🔴 **C'est le point qui distingue ce contrôle d'une formule.** L'assertion B est
rouge **sur une page qui passe l'assertion A** : `dist/index.html` porte bien un
`<link rel="stylesheet">` (`client/index.html:7`). Si le rapport ne mentionne
que les deux pages sans lien, l'assertion B n'a pas été évaluée, et elle ne le
sera plus jamais une fois A verte. **C'est exactement la rouge ①A-bis que P2 a
dû ajouter après coup, parce que sa première rouge masquait la seconde
assertion derrière la première.**

- [ ] **Step 4 : commit**

```bash
git add client/outils/surfaces-baties.mjs
git commit -m "design(s1): le controle des surfaces baties, deux assertions et deux rouges" \
  -- client/outils/surfaces-baties.mjs
```

---

### Task 3 : `§7.7` — le poids ne dérive pas ; VERT aujourd'hui, et sa rouge jouée quand même

**Objet :** poser le plafond de poids CSS, relever la ligne de base, et
**prouver que le contrôle peut échouer** — le seul des trois qui soit vert sur
l'arbre intact.

**Files:**
- Create: `client/outils/poids-css.mjs`

- [ ] **Step 1 : la ligne de base, relevée**

```bash
cd client && npm run build >/dev/null 2>&1 && find dist/assets -name '*.css' -printf '%s\t%p\n'
```
**Attendu (relevé à `8ad03a2`) : `1429  dist/assets/main-nAqQO_Wk.css`**, un
seul fichier. ⚠️ **Ce n'est PAS le 1 055 de la spec §2.3** — voir le tableau des
relevés périmés, n°6.

- [ ] **Step 2 : le contrôle**

Somme des octets de `dist/assets/*.css`, comparée au plafond **12 288** (12 Kio)
de la spec §7.7. Sortie non nulle au-dessus. Le script imprime **la somme et le
plafond**, toujours, y compris en cas de succès : un contrôle de dérive dont on
ne lit jamais la valeur ne sert qu'à passer.

⚠️ **L'en-tête du script déclare que le plafond est ARBITRAIRE**, mot pour mot
d'après la spec §7.7 : « Il n'est adossé à aucune mesure de performance : c'est
un garde-fou contre une addition massive, pas une cible de budget. » Il rejoint
la liste des constantes non calibrées du dépôt — `BPP_MIN`, `FACTEUR_FOCUS`,
`PART_DORMANTE_BPS`, `TAILLE_MAX_SORTIE`.

⚠️ **Le nom du fichier CSS partagé n'est PAS prévisible.** Mesuré au prototype :
quand plusieurs pages lient la même feuille, Vite émet un actif partagé dont le
nom vient d'un morceau JavaScript voisin — le prototype de S1 l'a vu sortir sous
le nom **`jeton-BtI6TA8C.css`**. **Le contrôle balaie `dist/assets/*.css` et ne
présume d'aucun nom** ; un script qui chercherait `socle-*.css` ne trouverait
rien.

- [ ] **Step 3 : la ROUGE, par perturbation, et remise en état**

```bash
node client/outils/poids-css.mjs; echo "exit=$?"            # attendu 0, somme 1429
node client/outils/poids-css.mjs --plafond 1024; echo "exit=$?"   # attendu 1
```
Le drapeau `--plafond` existe **pour cette rouge** et il est documenté comme
tel. **Cette rouge-là ne salit rien** : elle n'écrit aucun fichier et ne demande
aucune remise en état — c'est ce qui la rend rejouable à volonté par la recette.

⚠️ **Elle éprouve la COMPARAISON, pas la SOMME.** La rouge de la somme est celle
que la spec nomme (« ajouter un `@font-face` avec une police en `data:` URI »),
et elle **n'est pas jouée ici** : elle demanderait d'écrire une police dans une
feuille, donc de salir `client/src/` pour un contrôle. **Ce que le contrôle
établit est donc : la somme est lue, et le dépassement est refusé.** Que la
somme soit la bonne est établi par le Step 1, en la comparant à la sortie de
`find`. Dit autrement plutôt que caché.

- [ ] **Step 4 : commit**

```bash
git add client/outils/poids-css.mjs
git commit -m "design(s1): le plafond de poids CSS, arbitraire et declare" \
  -- client/outils/poids-css.mjs
```

---

# Famille ① — les règles pures, avant tout fichier de style

### Task 4 : `design/tokens.ts` — le parseur, et l'égalité des trois blocs (§7.4)

**Objet :** écrire le parseur que **trois** contrôles partagent (§7.1, §7.4,
§7.6), pur et testé, pour qu'aucun d'eux n'ait sa propre copie des valeurs.

**Files:**
- Create: `client/src/design/tokens.ts`, `client/src/design/tokens.test.ts`

**Interfaces:**
- Consumes: rien.
- Produces: `lireBlocsDeTheme`, `tokensDeclares`, `tokensReferences`,
  `ecartsEntreBlocs` (§« Interfaces partagées »).

- [ ] **Step 1 : annoncer le compte de tests attendu, AVANT de le mesurer**

Base à `8ad03a2` : **145**. Cette tâche ajoute **9** tests → **154** attendus.

- [ ] **Step 2 : écrire `tokens.test.ts` et le voir ROUGE**

Les neuf tests, et **ce qui rend chacun rouge** :

| Test | Rouge |
| --- | --- |
| découpe un CSS de démonstration en **exactement trois** blocs nommés | rendre 1 ou 4 blocs |
| le bloc `racine` est celui **sans condition** | confondre `:root` et `:root[data-theme="clair"]` |
| `tokensDeclares` rend l'**union** des trois blocs | rendre le seul `:root` |
| `tokensReferences` trouve `var(--a)`, `var(--b, fallback)` et **ignore** ce qui est en commentaire | attraper un `var(--x)` commenté |
| `ecartsEntreBlocs` rend **vide** sur trois blocs identiques | rendre un écart |
| un token **manquant dans le bloc média** est signalé | l'omettre |
| un token **en trop dans le bloc attribut** est signalé | ne tester l'inclusion que dans un sens |
| l'écart est nommé **avec le bloc et le token** | rendre un booléen |
| `color-scheme` est présent dans les **trois** blocs (D9) — vérifié à part, ce n'est pas un `--*` | l'omettre d'un bloc |

```bash
cd client && npx vitest run src/design/tokens.test.ts 2>&1 | tail -10
```
**ROUGE attendue : `Failed to resolve import "./tokens"`** — l'absence, pas la
logique. La rouge **de la logique** est au Step 4.

- [ ] **Step 3 : écrire `tokens.ts`**

TypeScript **effaçable** (D6) : aucun `enum`, aucun `namespace`. L'en-tête porte
cette contrainte et sa raison mesurée, avec la sortie de Node recopiée.

⚠️ **Le parseur travaille sur du TEXTE, jamais sur un fichier** : il ne connaît
ni `fs` ni chemin. C'est ce qui le rend testable dans un paquet qui n'a pas
`@types/node` (le typecheck rejetterait `import { readFileSync } from
'node:fs'` — P2 l'a mesuré sur `Buffer`, `TS2580`). **La lecture du disque
appartient aux `.mjs` de `client/outils/`.**

- [ ] **Step 4 : voir la ROUGE de la LOGIQUE, celle qui compte**

Remplacer temporairement le corps d'`ecartsEntreBlocs` par une inclusion **dans
un seul sens** (le bloc média ⊆ le bloc racine), relancer :
```bash
cd client && npx vitest run src/design/tokens.test.ts 2>&1 | grep -E "✓|×|Tests "
```
**Attendu : le test « un token en trop dans le bloc attribut est signalé »
échoue, les huit autres passent.** Défaire, relancer : `Tests 9 passed (9)`.

⚠️ **Ce step n'est pas décoratif.** L'inclusion à un seul sens est le défaut
**naturel** de ce contrôle — c'est celui qu'on écrit sans y penser — et il
laisserait passer exactement la dérive que §7.4 existe pour empêcher, la
palette claire étant déclarée **deux fois** (spec §4.2).

- [ ] **Step 5 : `client/outils/blocs-de-theme.mjs`**

Le script `.mjs` qui lit `client/src/design/tokens.css` et appelle
`ecartsEntreBlocs`. **Il ne peut pas encore tourner** — `tokens.css` naît en
Task 7 — et l'écrire ici est délibéré : la tâche qui écrit la source des valeurs
trouve son contrôle déjà là.

- [ ] **Step 6 : compter, puis commiter**

```bash
cd client && npx vitest run 2>&1 | grep -E "Tests "   # attendu: 154 passed
```
```bash
git add client/src/design/tokens.ts client/src/design/tokens.test.ts \
        client/outils/blocs-de-theme.mjs
git commit -m "design(s1): le parseur de tokens, et l'egalite des trois blocs dans les deux sens" \
  -- client/src/design/tokens.ts client/src/design/tokens.test.ts client/outils/blocs-de-theme.mjs
```

---

### Task 5 : `design/contraste.ts` — WCAG 2.1, et les 50 paires DÉCLARÉES (§7.1)

**Objet :** calculer les contrastes depuis la source unique, et le prouver sur
des vecteurs dont la valeur est connue indépendamment de notre code.

**Files:**
- Create: `client/src/design/contraste.ts`, `client/src/design/contraste.test.ts`,
  `client/outils/contraste.mjs`

**Interfaces:**
- Consumes: `BlocDeTheme` de `tokens.ts`.
- Produces: `luminanceRelative`, `rapportDeContraste`, `PAIRES`, `evaluer`.

- [ ] **Step 1 : compte attendu — 154 → 161 (7 tests neufs)**

- [ ] **Step 2 : les tests, ancrés sur des vecteurs EXTÉRIEURS**

🔴 **Un test de contraste écrit avec les couleurs du produit valide le produit
contre lui-même.** Les trois premiers tests emploient donc des vecteurs dont la
valeur est fixée par la norme WCAG 2.1 elle-même, et non par nos tokens :

| Test | Valeur attendue | Pourquoi elle est indiscutable |
| --- | --- | --- |
| `rapportDeContraste('#000000', '#ffffff')` | **21** | maximum absolu de l'échelle |
| `rapportDeContraste('#ffffff', '#ffffff')` | **1** | minimum absolu |
| symétrie : `rapport(a,b) === rapport(b,a)` sur trois couples | — | la formule est `(L+0.05)/(l+0.05)` avec `L ≥ l` ; l'oublier casse ici |
| `luminanceRelative` applique la correction gamma, pas une moyenne linéaire | `#808080` → **≈ 0,2159**, pas 0,5 | c'est **la** faute classique, et elle rend des rapports plausibles mais faux |
| `PAIRES.length` vaut **50**, et ce sont des paires **déclarées** | 50 | un produit cartésien en donnerait bien davantage — et inclurait `--bord`, que la spec §4.5 exclut délibérément |
| `evaluer` rend `echecs: []` sur une palette conforme | — | |
| `evaluer` **nomme** le thème, l'encre, le fond et le rapport de chaque échec | — | un booléen ne dirait pas quoi corriger |

⚠️ **`--bord` n'est PAS dans les paires, et c'est une DÉCISION**, pas un oubli :
il rend **1,45** (sombre) et **1,40** (clair) sur `--fond-0`, et il est réservé
aux séparateurs purement décoratifs, que WCAG 1.4.11 exempte. **Un test assure
qu'aucune paire ne porte `--bord`** — sans quoi un successeur bien intentionné
l'ajouterait et rendrait le contrôle rouge pour toujours, donc bon à assouplir.
**Le corollaire est écrit dans l'en-tête** : aucune commande ne peut vérifier
qu'on n'a pas employé `--bord` là où il fallait `--bord-fort`. C'est une
**règle de revue**, et la spec §8 la nomme comme telle.

- [ ] **Step 3 : écrire `contraste.ts`, puis `client/outils/contraste.mjs`**

Le `.mjs` lit `tokens.css`, appelle `lireBlocsDeTheme` puis `evaluer`, imprime
**le nombre de paires, le nombre d'échecs et le minimum global**, et sort non nul
si un échec. **Il ne connaît aucune couleur.**

- [ ] **Step 4 : la ROUGE de la LOGIQUE — la correction gamma retirée**

Remplacer `luminanceRelative` par une moyenne linéaire des canaux, relancer :
```bash
cd client && npx vitest run src/design/contraste.test.ts 2>&1 | grep -E "✓|×|Tests "
```
**Attendu : le test de `#808080` échoue** (`expected 0.5 to be close to 0.2159`),
et **le test `#000`/`#fff` PASSE quand même** — 21 reste 21 sans gamma.
🔴 **C'est le point** : la rouge la plus évidente ne discrimine pas. Sans le
vecteur `#808080`, le contrôle validerait une formule fausse sur toutes les
couleurs intermédiaires, c'est-à-dire sur les 50 paires réelles.

- [ ] **Step 5 : commit**

```bash
git add client/src/design/contraste.ts client/src/design/contraste.test.ts \
        client/outils/contraste.mjs
git commit -m "design(s1): le contraste WCAG lu depuis la source unique, et le vecteur qui discrimine" \
  -- client/src/design/contraste.ts client/src/design/contraste.test.ts client/outils/contraste.mjs
```

---

### Task 6 : `design/theme.ts` — trois états, et DEUX assertions séparées (§7.5)

**Objet :** la machine à trois états, avec l'application locale **et** la
réaction à `storage`, **chacune vue rouge SEULE**.

**Files:**
- Create: `client/src/design/theme.ts`, `client/src/design/theme.test.ts`

- [ ] **Step 1 : compte attendu — 161 → 169 (8 tests neufs)**

- [ ] **Step 2 : les huit tests, et le piège qu'ils existent pour attraper**

⚠️ **Le piège, écrit par la spec §4.2 et non négociable : l'événement `storage`
ne se déclenche PAS dans le document qui a écrit.** La fenêtre qui change le
thème doit donc l'appliquer **elle-même**, en plus d'écrire — et c'est
précisément la seule fenêtre que l'utilisateur regarde.

| # | Test | Assertion |
| --- | --- | --- |
| 1 | `choisir(coffre, racine, 'clair')` **écrit** `guac.theme` | fenêtre écrivante — écriture |
| 2 | `choisir(coffre, racine, 'clair')` **pose `data-theme="clair"` localement** | fenêtre écrivante — application |
| 3 | `surStockageModifie(racine, 'guac.theme', 'sombre')` pose `data-theme="sombre"` | fenêtre voisine |
| 4 | `surStockageModifie` **n'écrit rien** dans le coffre | fenêtre voisine — pas de boucle |
| 5 | `surStockageModifie(racine, 'guac.jeton.acces', '…')` **ne touche à rien** | robustesse — D5 |
| 6 | `themeStocke` rend `'systeme'` sur une valeur inconnue (`'bleu'`) | robustesse |
| 7 | `themeStocke` rend `'systeme'` sur `null` | robustesse |
| 8 | `appliquer(racine, 'systeme')` **RETIRE** l'attribut au lieu de le poser | voir §« Interfaces partagées » |

🔴 **Les tests 1 et 2 sont DEUX `it()` SÉPARÉS, jamais deux `expect` du même
test.** C'est la leçon que P2 a payée : « `expect` interrompt le test à la
première assertion, si bien que la seconde n'était éprouvée par rien ». Un
`it()` qui vérifierait l'écriture *puis* l'attribut s'arrêterait à l'écriture,
et **l'oubli de l'application locale — le défaut naturel de ce mécanisme — se
cacherait derrière elle**. Le test 5 emploie `guac.jeton.acces`, une clé que
`client/src/connexion.ts:57` écrit **réellement** (D5), pas une clé inventée.

- [ ] **Step 3 : écrire `theme.ts`, dépendances INJECTÉES**

Il n'y a **ni `window`, ni `localStorage`, ni `document`** sous Vitest ici :
`client/` n'a **aucun `vitest.config.*`** et **aucun `jsdom`**
(`ls client/node_modules/@types/` rend `estree` seul). `Coffre` et `Racine` sont
donc des **paramètres**, sur le patron exact de `client/src/jeton.ts:4-10`, qui
porte déjà la raison en toutes lettres. **Un `const racine =
document.documentElement` en tête de module suffirait à rendre ce fichier
impossible à charger sous Node, donc impossible à tester.**

- [ ] **Step 4 : DEUX rouges, une par assertion — et c'est le cœur de la tâche**

**Rouge A — l'application locale retirée** (le défaut naturel) : dans
`choisir`, ne faire qu'écrire dans le coffre.
```bash
cd client && npx vitest run src/design/theme.test.ts 2>&1 | grep -E "✓|×|Tests "
```
**Attendu : le test 2 échoue SEUL. Les tests 1, 3, 4, 5, 6, 7, 8 passent.**
🔴 **Si un autre test tombe aussi, les assertions ne sont pas assez séparées**,
et il faut les séparer davantage avant de continuer.

**Rouge B — le filtre de clé retiré** : dans `surStockageModifie`, appliquer
quelle que soit la clé.
**Attendu : le test 5 échoue SEUL** — `data-theme` posé à partir d'un JWT.

**Rouge C — l'attribut posé au lieu d'être retiré** : `appliquer(…, 'systeme')`
pose `data-theme="systeme"`.
**Attendu : le test 8 échoue SEUL.**

Défaire les trois, relancer : `Tests 8 passed (8)`.

- [ ] **Step 5 : écrire, dans l'en-tête, CE QUE CE CONTRÔLE NE PROUVE PAS**

Mot pour mot d'après la spec §7.5 : « **il n'établit pas que le navigateur
déclenche bien `storage` entre deux fenêtres réelles.** Il éprouve **notre**
gestionnaire, pas la plateforme. » La confirmation à deux fenêtres réelles est
prévue **hors critère**, en corroboration, à la Task 14.

- [ ] **Step 6 : commit**

```bash
git add client/src/design/theme.ts client/src/design/theme.test.ts
git commit -m "design(s1): les trois etats du theme, et les deux moities que storage ne relie pas" \
  -- client/src/design/theme.ts client/src/design/theme.test.ts
```

---

# Famille ② — les valeurs, et la reprise à l'identique

### Task 7 : `design/tokens.css` — la source unique, et §7.1 + §7.4 passent au vert

**Objet :** écrire LA source unique de valeurs, et voir les deux contrôles qui
l'attendaient rendre leur premier verdict.

**Files:**
- Create: `client/src/design/tokens.css`

- [ ] **Step 1 : le contenu, d'après la spec, sans rien inventer**

Trois blocs, dans cet ordre (spec §4.2, **sombre d'abord**) :
```css
:root { /* palette SOMBRE, sans condition */ }
@media (prefers-color-scheme: light) { :root:not([data-theme="sombre"]) { /* CLAIRE */ } }
:root[data-theme="clair"] { /* CLAIRE */ }
```

- les **treize** couleurs × deux thèmes du tableau de la spec §4.5, **verbatim** ;
- `color-scheme: dark` dans le premier bloc, `light` dans les deux autres (D9) ;
- les **six** tokens hors thème du tableau de D3, déclarés **une seule fois**
  dans `:root` et **jamais redéfinis** ;
- les **sept** crans typographiques, les **trois** interlignes, les **huit**
  crans d'espacement, les **quatre** rayons, les **deux** durées, les **deux**
  épaisseurs de trait, et les **deux** piles de polices (spec §4.3, §4.4).

⚠️ **`--fond-0` sombre et `--texte-fort` sombre sont EXACTEMENT les deux valeurs
déjà en place** — `#0b0d10` et `#e6e8eb`, `client/src/style.css:3-4`. **Un test
de la Task 9 le compare caractère pour caractère** ; ce n'est pas une
affirmation de cette tâche.

⚠️ **Porte armée : si `tokens.css` atteint 300 lignes**, le scinder en
`design/tokens/couleurs.css` et `design/tokens/echelles.css`, `tokens.css`
n'étant plus qu'un `@import` des deux. La spec §10 le prévoit à ~180 lignes ;
**le seuil d'action est à 300, et il se décide avant l'addition qui le
franchirait**, jamais après (D9 et D10 ont payé ce geste cinq fois).

- [ ] **Step 2 : §7.4 rend son premier verdict**

```bash
node client/outils/blocs-de-theme.mjs; echo "exit=$?"
```
**Attendu : `exit=0`, aucun écart entre les trois blocs.**

- [ ] **Step 3 : la ROUGE de §7.4, sur le VRAI fichier**

Retirer `--texte-faible` du seul bloc `[data-theme="clair"]`, relancer.
**Attendu : `exit=1`, et le rapport nomme le bloc et le token.** Remettre.

⚠️ **Cette rouge-ci n'est pas celle de la Task 4.** Celle-là éprouvait la
fonction sur un CSS de démonstration ; celle-ci éprouve **le câblage** —
que le script lit le bon fichier, et que le fichier a bien trois blocs
reconnaissables par le parseur. **Un parseur juste sur un fichier qu'il ne lit
pas est vert pour rien.**

- [ ] **Step 4 : §7.1 rend son premier verdict, sur les vraies couleurs**

```bash
node client/outils/contraste.mjs; echo "exit=$?"
```
**Attendu, d'après le relevé de la spec §4.5 :
`paires vérifiées : 50 / échecs : 0 / minimum global : 3.16`** (clair
`bord-fort`/`fond-2`).
🔴 **Si le minimum diffère, ce n'est PAS le contrôle qu'il faut ajuster** :
c'est soit une valeur mal recopiée du tableau, soit un désaccord réel avec le
relevé de la spec, **et il se déclare dans le document de résultats** plutôt
que de se résorber en changeant un seuil.

- [ ] **Step 5 : la ROUGE de §7.1, celle que la spec a déjà jouée**

Remplacer le seul `--texte-faible` clair (`#5c6675`) par `#7c8697`, relancer.
**Attendu, d'après le §7.1 de la spec :**
```
ÉCHEC clair texte-faible/fond-0 = 3.68 < 4.5
ÉCHEC clair texte-faible/fond-1 = 3.43 < 4.5
ÉCHEC clair texte-faible/fond-2 = 3.16 < 4.5
paires vérifiées : 50
échecs : 3
```
**Une perturbation d'un token suffit, et elle en fait tomber trois.** Remettre,
relancer, revérifier `échecs : 0`.

- [ ] **Step 6 : commit**

```bash
git add client/src/design/tokens.css
git commit -m "design(s1): tokens.css, source unique -- treize couleurs par theme et les echelles" \
  -- client/src/design/tokens.css
```

---

### Task 8 : `design/base.css` + `design/socle.css` — et la racine que `--e-3` exige

**Objet :** les défauts d'élément exprimés en tokens, **et le retrait du
`font-size` de `html` dont dépend l'égalité `--e-3` = 12 px** (D4).

**Files:**
- Create: `client/src/design/base.css`, `client/src/design/socle.css`

- [ ] **Step 1 : `base.css`, reprise de `style.css:7-19` en tokens**

- `* { box-sizing: border-box }` — repris tel quel de `style.css:7-9` ;
- `html, body { margin: 0; height: 100%; }` — repris de `style.css:11-19` ;
- 🔴 **`html` ne porte AUCUNE taille de police** — voir Step 2 ;
- `body { background: var(--fond-0); color: var(--texte-fort);
  font-family: var(--police-ui); font-size: var(--t-m);
  line-height: var(--lh-normal); }` ;
- `:focus-visible { outline: var(--trait-focus) solid var(--accent);
  outline-offset: 2px; }` — le seul ajout d'apparence de S1, et il est nommé :
  la spec §3 point 1 réserve l'accent à trois usages dont le focus, et
  l'écran de connexion en a besoin dès aujourd'hui.

⚠️ **`overflow: hidden` de `style.css:14` NE MONTE PAS dans `base.css`.** C'est
une propriété de la **fenêtre de session** — la vidéo occupe `100vw`/`100vh` —
et l'appliquer à la page-shell ou à l'écran de connexion **couperait leur
contenu dès qu'il dépasserait la fenêtre**, sans aucun message. Elle reste dans
`style.css`.

- [ ] **Step 2 : 🔴 la raison du retrait, écrite DANS le fichier**

`client/src/style.css:11-18` porte aujourd'hui `font: 14px/1.5 …` sur le
sélecteur `html, body` : **la racine vaut donc 14 px**, et `0.75rem` y vaudrait
**10,5 px**, pas 12. Le commentaire de `base.css` porte ce calcul, parce qu'un
successeur qui « rétablirait » la taille sur `html` pour la symétrie **rendrait
tous les décalages des bandeaux à 10,5 px sans qu'aucun contrôle ne le dise** —
les sept contrôles de S1 portent sur les couleurs, les ensembles de noms, la
bascule et le poids, **aucun ne mesure une longueur**.

Il porte aussi la **portée** de l'égalité (D4) : `--e-3` vaut 12 px **à racine
16 px, c'est-à-dire au réglage par défaut du navigateur**. Pour un utilisateur
qui a agrandi sa police, les décalages grandissent — là où les `12px` absolus
d'aujourd'hui ne bougeaient pas. **C'est le changement voulu par la spec §4.4,
et c'en est un.**

- [ ] **Step 3 : `socle.css`**

```css
@import './tokens.css';
@import './base.css';
```
C'est le **point d'entrée unique** que les trois HTML lieront (Task 11).
**Mesuré au prototype** : Vite résout ces `@import` et émet **un seul** actif
CSS partagé entre les pages qui le lient — 359 octets pour un socle de
démonstration, sans duplication.

- [ ] **Step 4 : commit**

```bash
git add client/src/design/base.css client/src/design/socle.css
git commit -m "design(s1): base.css et le socle -- la racine cesse d'etre forcee" \
  -- client/src/design/base.css client/src/design/socle.css
```

---

### Task 9 : `style.css` cesse de déclarer des couleurs — §7.2 passe au vert

**Objet :** promouvoir les **neuf** couleurs littérales en tokens, **sans
changer une seule longueur**, et prouver la reprise à l'identique.

**Files:**
- Modify: `client/src/style.css`
- Create: `client/src/design/reprise.test.ts`

- [ ] **Step 1 : relire le fichier À CET INSTANT, pas l'inventaire de ce plan**

```bash
git rev-parse --short HEAD
grep -nE '#[0-9a-fA-F]{3,8}\b|rgb\(|rgba\(|hsl\(|hsla\(' client/src/style.css
```
D10 : le chantier E travaille dans ce fichier. **Toute couleur trouvée se
promeut**, y compris celles que ce plan ne connaît pas. Si une couleur neuve
n'entre dans aucun des six tokens hors thème de D3, **en déclarer un septième
plutôt que de la raccorder à un token sémantique** — la raison est en D3.

- [ ] **Step 2 : les remplacements, et RIEN d'autre**

- `:root` perd `--surface`, `--text` et `color-scheme` — ils vivent désormais
  dans `tokens.css`. **`var(--text)` (l. 17, 67, 100) devient
  `var(--texte-fort)`, et `var(--surface)` (l. 16) devient `var(--fond-0)`** ;
- `style.css` gagne `@import './design/socle.css';` **en première ligne**
  ⚠️ ou bien la liaison se fait depuis `index.html` — **voir la décision
  mesurée du Step 3** ;
- les neuf littérales passent aux six tokens du tableau de D3 ;
- 🔴 **AUCUNE longueur ne bouge** : les `12px`, `6px`, `8px`, `18px`, `64px`,
  `0.02em` et `0.3s` deviennent leur token **quand il existe**
  (`--e-3`, `--r-2`, `--e-2`, `--e-8`, `--duree-2`) et **restent littéraux
  quand il n'existe pas** (`padding: 6px …`, `font-size: 18px`,
  `letter-spacing: 0.02em`), avec le commentaire de D2 qui dit pourquoi et
  renvoie à `style.css:79-83`.

- [ ] **Step 3 : 🔵 la décision de liaison, prise sur une MESURE**

Deux arrangements sont possibles, et ils ne coûtent pas la même chose. **Les
deux ont été bâtis dans un prototype hors dépôt le 19 août 2026**, sur une copie
de `client/` à `d280745` :

| Arrangement | Somme `dist/assets/*.css` | Actifs CSS émis |
| --- | --- | --- |
| **A** — `style.css` fait `@import './design/socle.css'`, `index.html` ne lie que `style.css` | **2 146** octets | 2 : le socle **et une copie du socle** inlinée dans le CSS de `main` |
| **B** — `index.html` lie `socle.css` **puis** `style.css`, et `style.css` n'importe rien | **1 788** octets | 2, **sans duplication** : le socle partagé (359 o.) et `style.css` seul (1 429 o.) |

**Tranché : arrangement B.** L'`@import` fait ré-inliner le socle dans chaque
entrée qui le traverse ; le lier depuis le HTML laisse Vite le partager. **Le
coût de A est de +358 octets sur un socle de démonstration de 15 lignes** — sur
les ~180 lignes prévues par la spec §10, la duplication se compterait en
kilo-octets, contre un plafond de 12 288.

⚠️ **Ce que le prototype établit et ce qu'il n'établit pas** : il a été bâti
avec un `tokens.css` de **cinq** tokens, pas la palette réelle. **Les deux
sommes ci-dessus ne préjugent pas du chiffre final** ; ce qu'elles établissent
est **le rapport entre les deux arrangements**, qui ne dépend pas du contenu.
**La somme réelle est relevée à la Task 14, pas prédite ici.**

- [ ] **Step 4 : `reprise.test.ts` — la comparaison qui PROUVE la reprise**

🔴 **La spec demande « la comparaison qui le prouve, pas l'affirmation ».** Trois
assertions, toutes décidables par commande, sur `tokens.css` lu comme **texte**
et parsé par `tokens.ts` :

| # | Assertion | Rouge |
| --- | --- | --- |
| 1 | `--fond-0` du bloc `racine` vaut **exactement** `#0b0d10`, et `--texte-fort` **exactement** `#e6e8eb` — les deux valeurs de `style.css:3-4` à `8ad03a2` | changer un chiffre hexadécimal |
| 2 | la table de reprise est **complète et exacte** : `--e-3`→`0.75rem`, `--t-s`→`0.75rem`, `--e-2`→`0.5rem`, `--e-8`→`4rem`, `--t-m`→`0.875rem`, `--r-2`→`6px`, `--duree-2`→`300ms` — et **chaque valeur en `rem` rend, multipliée par 16, le littéral d'aujourd'hui** (12, 12, 8, 64, 14) | poser `--e-3: 0.8rem` (12,8 px) : plausible, et faux |
| 3 | les six tokens hors thème valent **verbatim** les six littérales de D3 | changer une opacité |

⚠️ **Ce que ces trois assertions établissent, écrit dans l'en-tête du test** :
**l'égalité des valeurs DÉCLARÉES**, à racine 16 px. **Pas** l'égalité des
pixels rendus — cela demanderait un moteur de rendu, et la spec §7.8 écarte la
comparaison d'images pour S1 à S4, la première de ses trois raisons étant
éliminatoire (les polices système rendent différemment d'une machine à l'autre).

⚠️ **La précondition du n°2 n'est pas dans ce test** : que la racine ne soit pas
forcée est une propriété de `base.css`, pas de `tokens.css`. **Un quatrième
test l'assure**, sur le texte de `base.css` : aucune règle dont le sélecteur
contient `html` ne porte `font-size` ni `font:`. **Sa rouge est de remettre
`html` dans le sélecteur du Step 2 de la Task 8** — et elle doit être jouée,
parce que c'est le seul lien mécanique entre les deux fichiers.

- [ ] **Step 5 : §7.2 rend son verdict, et il doit avoir CHANGÉ**

```bash
node client/outils/couleurs-litterales.mjs; echo "exit=$?"
```
**Attendu : `exit=0`, zéro occurrence** — contre neuf au Step 3 de la Task 1.

⚠️ **Un `exit=0` ici peut être une panne du script.** La Task 1 Step 4 a montré
qu'il sait rendre vert sur un répertoire propre ; ici il faut aussi qu'il ait
**réellement lu quelque chose**. Le script imprime **le nombre de fichiers
balayés** : le comparer à celui du Step 3 de la Task 1. **S'il a baissé, le
périmètre s'est cassé et le vert ne vaut rien.**

- [ ] **Step 6 : compter les tests, et commiter**

```bash
cd client && npx vitest run 2>&1 | grep -E "Tests "   # attendu: 169 + 4 = 173 passed
cd client && npm run typecheck; echo "exit=$?"        # attendu 0
```
```bash
git add client/src/style.css client/src/design/reprise.test.ts client/index.html
git commit -m "design(s1): la fenetre de session passe aux tokens, aucune longueur ne bouge" \
  -- client/src/style.css client/src/design/reprise.test.ts client/index.html
```

---

# Famille ③ — l'amorce, et les surfaces qui n'avaient rien

### Task 10 : l'amorce anti-FOUC, injectée depuis une source unique

**Objet :** poser `data-theme` **avant la première peinture**, dans **toutes**
les entrées, depuis **un seul** fichier.

**Files:**
- Create: `client/src/design/amorce-theme.js`
- Modify: `client/vite.config.ts`

- [ ] **Step 1 : `amorce-theme.js`, et pourquoi c'est du `.js`**

Une dizaine de lignes, synchrones, sans `import` : elles lisent `guac.theme` et
posent `data-theme` **si et seulement si** la valeur est `clair` ou `sombre`.
Le tout dans un `try` — un navigateur qui refuse le stockage (mode privé strict,
politique d'entreprise) lèverait sur `localStorage`, et une exception ici
**empêcherait la page entière de s'amorcer**.

⚠️ **Ce fichier n'est ni typechecké ni testé, et c'est déclaré** :
`client/tsconfig.json:12` n'inclut que `src/**/*.ts`, donc un `.js` en est hors.
**C'est pourquoi il ne porte aucune règle** — il ne connaît ni les trois états,
ni la clé au-delà de sa chaîne littérale. La logique vit dans `theme.ts`, qui
est typé et testé. **Sa duplication de la chaîne `'guac.theme'` est le seul
recouvrement, et il est assumé** : le script s'exécute avant tout module, donc
il ne peut rien importer. ⚠️ **Un test de `theme.test.ts` compare `CLE_THEME` au
texte de `amorce-theme.js`** et refuse s'ils ont divergé — c'est le patron
exact que P2 a employé entre `client/src/jeton.ts` et
`client/recette/jeton-recette.mjs`, et **son garde a été vu lever**.

- [ ] **Step 2 : le greffon, et l'endroit d'injection — MESURÉ**

```ts
transformIndexHtml: {
    order: 'pre',
    handler() { return [{ tag: 'script', children: source, injectTo: 'head' }]; },
}
```
`source` est lu **une fois**, par `readFileSync`, à la construction du greffon.

🔴 **`injectTo: 'head'` et non `'head-prepend'`, et c'est une mesure, pas un
goût.** Le prototype a bâti les deux. Avec `'head-prepend'` — **le défaut de
Vite**, `HtmlTagDescriptor.injectTo` étant documenté « default: 'head-prepend' »
dans `client/node_modules/vite/dist/node/index.d.ts:3166-3168` — le script sort
**avant `<meta charset>`** :

```
  <head>
    <script>(function () { ... })();
</script>

    <meta charset="UTF-8" />
```

Le fichier d'amorce porte un commentaire accentué ; il serait donc décodé
**avant** que l'encodage du document ne soit connu, et il repousse la
déclaration de charge utile plus loin dans les 1 024 premiers octets que la
spécification HTML lui accorde. Avec `'head'`, le script sort **après
`<meta charset>` et `<title>`, avant le module et la feuille de style** — ce que
le prototype montre :

```
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>Session distante</title>
      <script>(function () { ... })();
</script>
      <script type="module" crossorigin src="/assets/main-DfksZBtX.js"></script>
```

**Et cela suffit à l'anti-FOUC** : un script en ligne synchrone dans `<head>`
s'exécute avant que `<body>` ne soit analysé, donc avant la première peinture.
**Poser l'attribut plus tôt que cela n'achète rien et coûte le charset.**

- [ ] **Step 3 : la preuve que l'injection atteint TOUTES les entrées**

```bash
cd client && npm run build >/dev/null 2>&1
for f in dist/*.html; do printf "%s : " "$f"; grep -c "guac.theme" "$f"; done
```
**Attendu : `1` pour chacune des trois (quatre après la Task 12).**
**Mesuré au prototype : `1` pour `connexion.html`, `index.html` et
`shell.html`.**

- [ ] **Step 4 : la ROUGE — un greffon qui ne couvre qu'une entrée**

Remplacer le `handler()` par un `handler(_html, ctx)` qui ne rend le script que
si `ctx.filename` finit par `index.html`, rebâtir, relancer le Step 3.
**Attendu : `1`, `0`, `0`.** Défaire.

⚠️ **C'est la seule preuve que le greffon injecte partout**, et elle est
**exactement le risque que la spec §11 nomme** : « Le greffon Vite d'injection
casse une entrée ». Sans elle, le greffon pourrait ne couvrir qu'`index.html` —
la seule page qu'on regarde en développant — et les trois autres naîtraient
avec un éclair de mauvais thème que personne ne verrait en revue.

- [ ] **Step 5 : commit**

```bash
git add client/src/design/amorce-theme.js client/vite.config.ts
git commit -m "design(s1): l'amorce anti-FOUC, une source unique injectee dans toutes les entrees" \
  -- client/src/design/amorce-theme.js client/vite.config.ts
```

---

### Task 11 : `shell.html` et `connexion.html` reçoivent leur feuille — §7.3 passe au vert

**Objet :** donner une feuille de style aux **deux** surfaces qui n'en ont
aucune, et voir les **deux** assertions de §7.3 passer au vert.

**Files:**
- Modify: `client/shell.html`, `client/connexion.html`

- [ ] **Step 1 : la liaison**

Dans chacun des deux, avant `</head>` (`client/shell.html:7`,
`client/connexion.html:7`) :
```html
<link rel="stylesheet" href="/src/design/socle.css" />
```
**C'est la première feuille de style de la vie de `shell.html`** — le fichier
existe depuis D1 et n'en a jamais eu.

⚠️ **`client/connexion.html:9-13` porte un commentaire qui attend ce moment** :
« AUCUNE DIRECTION VISUELLE ICI, et c'est délibéré : elle appartient au
sous-projet ⑥. Cette page porte la structure et rien d'autre ; l'habiller
maintenant obligerait à défaire ce travail-là. » **S1 ne l'habille toujours
pas** — il lui donne les tokens, pas des primitives. Le commentaire **reste
vrai** et n'est pas touché ; c'est **S3** qui le retirera.

- [ ] **Step 2 : §7.3 rend son verdict, sur ses DEUX assertions**

```bash
cd client && npm run build >/dev/null 2>&1 && cd ..
node client/outils/surfaces-baties.mjs; echo "exit=$?"
```
**Attendu : `exit=0`**, et le rapport dit **explicitement** que **les deux**
assertions sont vertes pour **chacune** des pages. **Mesuré au prototype** :
les trois pages lient le socle partagé, et ce socle déclare bien `--fond-0`.

- [ ] **Step 3 : ce que S1 change à l'apparence de ces deux pages, et qui n'est PAS neutre**

🔴 **La spec écrit « S1 doit être visuellement quasi neutre sur `index.html` »
— sur `index.html`, et sur lui seul.** Sur `shell.html` et `connexion.html`, S1
**change le rendu**, et c'est son objet : elles passent du blanc par défaut du
navigateur au fond `--fond-0` avec l'encre `--texte-fort`, la police
`--police-ui`, la base `--t-m`, et un `color-scheme` qui suit le thème — donc
des champs de formulaire au rendu natif accordé (D9).

**Ce que cela impose, et qui est nommé plutôt que découvert** :

1. **Aucun test ne casse** — `client/src/connexion.ts` et
   `client/src/shell-page.ts` ne sont **pas testés unitairement**, par une
   convention que `connexion.ts:5-9` déclare. Les 145 tests portent sur des
   modules purs, dont **aucun ne lit de style**. **Le compte doit rester
   inchangé, et la tâche le vérifie.**
2. **Aucune fonction ne change** — le formulaire, ses `id`, son `submit` et son
   `#message` sont intouchés. **Seuls les deux `<link>` sont ajoutés**, et
   `git diff --stat` doit rendre **deux fichiers, deux lignes ajoutées, zéro
   retirée**.
3. **Le jugement sur ce nouveau rendu est HUMAIN**, et il appartient à la
   galerie (Task 12) et au §8. Aucun contrôle de S1 ne dit qu'une page est
   belle, lisible ou utilisable — **ils disent qu'elle charge les tokens.**

```bash
git diff --stat client/shell.html client/connexion.html
cd client && npx vitest run 2>&1 | grep -E "Tests "   # attendu: 173 passed, INCHANGÉ
```

- [ ] **Step 4 : commit**

```bash
git add client/shell.html client/connexion.html
git commit -m "design(s1): la page-shell et l'ecran de connexion recoivent leur feuille" \
  -- client/shell.html client/connexion.html
```

---

# Famille ④ — ce qui garde le socle après S1

### Task 12 : `§7.6` les orphelins, et la galerie — exclue de la moitié « employé »

**Objet :** le dernier contrôle, et l'instrument du jugement humain — écrits
**ensemble**, parce que l'un doit exclure l'autre.

**Files:**
- Create: `client/outils/tokens-orphelins.mjs`, `client/design.html`
- Modify: `client/vite.config.ts` (une quatrième entrée)

- [ ] **Step 1 : le contrôle, et l'exclusion qui le rend capable d'échouer**

Deux inclusions, **dans les deux sens**, entre `tokensDeclares(tokens.css)` et
`tokensReferences(feuilles de production)`.

🔴 **`client/design.html` est EXCLU de la moitié « employé ».** La spec §7.6 le
dit et dit pourquoi : « la galerie rend tous les tokens par construction, donc
l'inclure rendrait ce contrôle **incapable d'échouer** — il validerait la
galerie et rien d'autre ». **C'est la classe de défaut dont ce dépôt a attrapé
quatre exemplaires sur le seul sous-bloc D10**, dont trois écrits par un plan.

**Le périmètre « production » est donc, nommément** :
`client/src/style.css`, `client/src/design/base.css`, et toute feuille future
de `client/src/` — **jamais** `client/design.html`, **jamais**
`client/src/design/tokens.css` (qui déclare, il n'emploie pas).
**L'exclusion est écrite dans le script avec sa raison, pas seulement dans ce
plan.**

- [ ] **Step 2 : la galerie**

`client/design.html`, quatrième entrée de `client/vite.config.ts` — ⚠️ **une
page absente de cette liste ne sort pas du build et rien ne le dit**, comme
`client/vite.config.ts:9-13` l'avertit après l'avoir mesuré sur
`connexion.html`.

Elle lie `socle.css`, et rend **chaque token** : les treize couleurs en pastilles
avec leur nom et leur valeur, les six voiles sur un fond d'échantillon, les sept
crans typographiques sur une ligne de texte réelle, les huit d'espacement en
barres, les quatre rayons, les deux durées sur une transition déclenchée au
survol, et les deux piles de polices. **Elle porte les trois boutons de thème**,
câblés sur `choisir()` de `theme.ts` — c'est le premier et le seul appelant de
S1, et il vit sur une page qui n'est pas le produit.

⚠️ **Porte armée à 300 lignes** : au-delà, scinder par famille, au même rythme
que les primitives de S2 (spec §10).

- [ ] **Step 3 : la ROUGE de §7.6, dans les DEUX sens**

**Rouge A — un token déclaré et jamais employé.** La spec §4.3 nomme le cas
d'avance : `--police-mono` n'a **qu'un seul appelant prévu**, le bandeau
`#stats`. Retirer temporairement `font-family: var(--police-mono)` de
`style.css`, relancer.
**Attendu : `exit=1`, `--police-mono` signalé comme orphelin.** Remettre.

**Rouge B — un `var(--…)` non déclaré.** Écrire `var(--fond-O)` (lettre O au
lieu du zéro) dans `base.css`, relancer.
**Attendu : `exit=1`, `--fond-O` signalé comme non déclaré.** Remettre.

**Rouge C — celle qui éprouve l'EXCLUSION**, et sans laquelle les deux
premières ne prouvent rien de la conception : retirer temporairement
l'exclusion de `design.html` du périmètre « employé », **puis rejouer la
rouge A**.
🔴 **Attendu : `exit=0` — la rouge A devient VERTE**, parce que la galerie
emploie `--police-mono` comme elle emploie tout. **C'est la preuve que
l'exclusion est porteuse et non décorative.** Remettre l'exclusion, rejouer A,
revoir `exit=1`.

- [ ] **Step 4 : ce que la galerie N'EST PAS**

Écrit dans son en-tête, d'après la spec §7.8 et §8 : la galerie est
**l'instrument d'un jugement humain**, pas une preuve. La comparaison d'images
de référence est **écartée** pour S1 à S4, et sa première raison est
éliminatoire : les polices système rendent différemment d'une machine à l'autre,
si bien qu'une référence prise sur un poste échouerait sur le suivant **pour une
raison qui n'est pas un défaut** — il faudrait embarquer une police *pour rendre
la recette possible*, c'est-à-dire payer au bénéfice de l'instrument le coût que
la spec §4.3 refuse au bénéfice du produit.

**Ce que la galerie ne dit donc jamais** : que la palette soit sobre, que le
ratio 1,2 soit le bon, que 14 px soit assez dense, que `#7aa2f7` soit le bon
bleu. **Ce sont les huit lignes du §8, et aucune ne deviendra une mesure.**

- [ ] **Step 5 : commit**

```bash
cd client && npm run build >/dev/null 2>&1 && ls dist/design.html   # la 4ᵉ entrée sort
git add client/outils/tokens-orphelins.mjs client/design.html client/vite.config.ts
git commit -m "design(s1): les tokens orphelins, et la galerie exclue de la moitie employe" \
  -- client/outils/tokens-orphelins.mjs client/design.html client/vite.config.ts
```

---

### Task 13 : `verifier-design.mjs`, et la dixième étape de `verify-all.sh`

**Objet :** faire des sept contrôles une **commande du dépôt**, sans quoi
« appliqué en continu » (cadrage §5 ⑥) reste un vœu.

**Files:**
- Create: `client/outils/verifier-design.mjs`
- Modify: `client/package.json`, `scripts/verify-all.sh`

- [ ] **Step 1 : l'agrégateur**

`verifier-design.mjs` lance les six scripts, **imprime le verdict de chacun**,
et sort **non nul si l'un d'eux échoue** — jamais au premier, pour qu'un
opérateur voie tous les défauts d'un coup plutôt qu'un par relance.

⚠️ **Il bâtit d'abord** : `§7.3` et `§7.7` lisent `dist/`, et un `dist/` périmé
rendrait un verdict sur le build d'avant. **C'est le piège maison
« un pilote qui laisse un superviseur vivant fait relire le journal de la
tentative précédente », transposé** : on mesure l'état d'avant en croyant lire
le sien.

⚠️ **Le septième contrôle, §7.5, N'EST PAS dans cet agrégateur** : c'est un
test unitaire, il tourne dans `npm test`. **Le dire ici évite qu'un lecteur
compte six et conclue qu'il en manque un.**

- [ ] **Step 2 : `client/package.json`**

```json
"design:verifier": "node outils/verifier-design.mjs"
```
**Aucune dépendance ajoutée.**

- [ ] **Step 3 : `scripts/verify-all.sh` — une DIXIÈME étape**

Le script en compte **neuf** à `8ad03a2` (`grep -c '^etape "'`). La dixième se
place **après** `client : npm run typecheck` :

```bash
etape "client : npm run design:verifier"
(cd client && npm run design:verifier) || echec "client : npm run design:verifier"
```

⚠️ **`scripts/run-agent.sh` n'est PAS modifié**, et le piège maison « toute
variable neuve doit y être ajoutée » **ne s'applique pas** : S1 n'introduit
aucune variable d'environnement, ni pour l'agent ni pour le service. Même
situation qu'en P1 et P2, et dite pour la même raison.

- [ ] **Step 4 : la ROUGE de l'étape elle-même**

Casser une seule couleur (`background: #fff` dans `base.css`), lancer
`./scripts/verify-all.sh`.
**Attendu : `ÉCHEC : client : npm run design:verifier`, et le script s'arrête
là** — pas plus loin. Remettre, relancer.

🔴 **C'est la seule preuve que le branchement mord.** Un script ajouté qui rend
toujours 0 ferait exactement ce que ce dépôt appelle « un contrôle qu'on n'a
jamais vu rouge », et la spec §6.1 en fait la condition de sa propre
crédibilité : « **Sans ce contrôle, "appliqué en continu" est un vœu.** »

- [ ] **Step 5 : commit**

```bash
git add client/outils/verifier-design.mjs client/package.json scripts/verify-all.sh
git commit -m "design(s1): les sept controles deviennent une commande du depot" \
  -- client/outils/verifier-design.mjs client/package.json scripts/verify-all.sh
```

---

# Famille ⑤ — la recette, et la mémoire

### Task 14 : recette S1 — sept contrôles, deux exécutions chacun, pièces versées

**Objet :** juger S1 sur ses sept contrôles, **deux fois chacun**, et verser
les pièces dans git.

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-design-system-s1-resultats.md`,
  `docs/superpowers/plans/journaux-design-system-s1/`

🔴 **Toute preuve d'une affirmation portée dans `CLAUDE.md` doit être VERSÉE
DANS GIT.** D9 a perdu **six** constats de revue parce que leur preuve vivait
dans `.superpowers/sdd/`, gitignoré et jamais commité ; la tâche 18 de D10 a
établi **par la commande** que le répertoire a disparu, et les six sont
**définitivement perdus**.

- [ ] **Step 1 : le tableau des sept contrôles, à remplir par la MESURE**

| # | Contrôle | Comment il est jugé | La ROUGE, et où elle a été jouée |
| --- | --- | --- | --- |
| §7.1 | les contrastes tiennent les seuils WCAG | `node client/outils/contraste.mjs` — paires, échecs, minimum | T5 Step 4 (gamma retiré, et le vecteur `#808080` qui SEUL discrimine) ; T7 Step 5 (`--texte-faible` perturbé, 3 échecs) |
| §7.2 | aucune couleur littérale hors `tokens.css` | `node client/outils/couleurs-litterales.mjs` | **T1 Step 3 — sur l'arbre INTACT, neuf occurrences, sans rien casser** |
| §7.3 | toute surface bâtie porte les tokens | `node client/outils/surfaces-baties.mjs`, **deux assertions comptées séparément** | **T2 Step 3 — sur l'arbre INTACT : A rouge sur deux pages, B rouge sur une page qui passe A** |
| §7.4 | les trois blocs déclarent le même ensemble | `node client/outils/blocs-de-theme.mjs` | T4 Step 4 (inclusion à un seul sens) ; T7 Step 3 (sur le vrai fichier) |
| §7.5 | la bascule atteint les N fenêtres, **y compris celle qui l'a demandée** | `npx vitest run src/design/theme.test.ts` | T6 Step 4 — **trois rouges, chacune faisant tomber UN test et un seul** |
| §7.6 | aucun token orphelin, aucun `var()` non déclaré | `node client/outils/tokens-orphelins.mjs` | T12 Step 3 — **trois rouges, dont la C éprouve l'EXCLUSION de la galerie** |
| §7.7 | le poids ne dérive pas | `node client/outils/poids-css.mjs` | T3 Step 3 (plafond abaissé) — **et sa portée exacte y est déclarée** |

- [ ] **Step 2 : DEUX exécutions de chaque, journalisées**

```bash
mkdir -p docs/superpowers/plans/journaux-design-system-s1
J=docs/superpowers/plans/journaux-design-system-s1
git rev-parse --short HEAD > $J/commit.txt
for i in 1 2; do
  (cd client && npm run build)            > $J/build-$i.log 2>&1
  (cd client && npm run design:verifier)  > $J/design-verifier-$i.log 2>&1
  (cd client && npx vitest run)           > $J/vitest-client-$i.log 2>&1
  (cd client && npm run typecheck)        > $J/typecheck-$i.log 2>&1
  (cd proto  && npx vitest run)           > $J/vitest-proto-$i.log 2>&1
done
./scripts/verify-all.sh > $J/verify-all.log 2>&1; echo "verify-all exit=$?" >> $J/verify-all.log
grep -h "Tests  " $J/vitest-*.log
find client/dist/assets -name '*.css' -printf '%s\t%p\n' > $J/poids-css.txt
```

⚠️ **Deux exécutions établissent la REPRODUCTIBILITÉ, jamais un taux.** Les sept
contrôles sont **déterministes** : la question « combien de fois sur combien »
ne se pose pas, et la spec §9 interdit de l'emprunter à une campagne qui, elle,
l'aurait posée.

⚠️ **`verify-all.sh` peut échouer à sa PREMIÈRE étape sans que S1 y soit pour
quelque chose** : `cargo test --workspace` porte sur un `agent/` que le chantier
microphone modifie. **C'est arrivé à P2**, dont le témoin a échoué « sur un test
Rust du voisin ». **Si cela arrive : le déclarer, relever `git status`, et juger
S1 sur les étapes `client` et `proto`** — jamais masquer, jamais imputer.

- [ ] **Step 3 : la corroboration HORS CRITÈRE — deux fenêtres réelles**

La spec §7.5 le dit : les tests éprouvent **notre gestionnaire**, pas la
plateforme. Ouvrir `dist/design.html` dans deux fenêtres du même navigateur,
changer le thème dans l'une, observer l'autre.

🔴 **Ceci n'est PAS un critère, et le document de résultats doit le dire au
même endroit qu'il le rapporte.** C'est le même statut que les confirmations sur
VM réelle du sous-projet ⑤. **Si la corroboration n'est pas faite, elle est
déclarée non faite** — un « probablement » vaut moins qu'un « non mesuré ».

- [ ] **Step 4 : écrire le document de résultats**

`docs/superpowers/plans/2026-08-19-design-system-s1-resultats.md`, sur le modèle
des documents de résultats de P1 et P2. Il porte, **au minimum** :

1. le verdict de chaque contrôle **avec son nombre d'exécutions** ;
2. **chaque ROUGE jouée, avec son message d'échec verbatim** — c'est ce qui
   distingue un contrôle d'une formule ; et **pour §7.3, §7.5 et §7.6, la
   preuve que chaque rouge n'a fait tomber QUE l'assertion visée** ;
3. les **onze divergences** D1…D11 de ce plan, avec leur sort ;
4. les **huit relevés périmés** de la spec, avec la valeur juste et son commit ;
5. ce que S1 **n'établit pas** (§ ci-dessous), sans le raboter ;
6. les tailles de fichiers **relevées par la commande**, avec `HEAD` ;
7. la somme CSS relevée, opposée à la ligne de base **1 429** et au plafond
   **12 288** ;
8. un renvoi nommé vers chaque journal versé.

**§ « Ce que S1 n'établit PAS », à écrire sans le raboter :**

- **Aucun taux.** Deux exécutions par contrôle, sur des contrôles
  **déterministes** : elles établissent la reproductibilité, rien de plus.
- 🔴 **Aucun jugement visuel n'a été porté sur aucune valeur.** Les huit lignes
  du §8 de la spec restent entières : que la direction soit « sobre » et
  « pro », que `#7aa2f7` soit le bon bleu, que le ratio **1,2** soit le bon, que
  **14 px** soit assez dense, que le pas de **4 px** soit le bon, que `--bord`
  ait été employé là où il fallait, que le plafond de **12 Kio** soit au bon
  endroit, que la galerie montre ce qu'il faut regarder. **Aucune ne se
  transformera en mesure au fil des sous-blocs**, et elles rejoignent la liste
  déjà longue du dépôt — `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
  `HYSTERESIS`, `TAILLE_MAX_SORTIE`, les paramètres `scrypt` de P2.
- **Que le navigateur déclenche `storage` entre deux fenêtres réelles** : §7.5
  éprouve notre gestionnaire. La corroboration du Step 3 est **hors critère**.
- **Que le thème atteigne les N fenêtres du produit** : la corroboration porte
  sur deux fenêtres de la galerie, **pas** sur une page-shell qui ouvre N
  sessions par `window.open`.
- **Trois longueurs restent hors échelle** — `padding: 6px`, `font-size: 18px`,
  `letter-spacing: 0.02em` (D2) —, et **aucun contrôle ne mesure les longueurs**.
- **La reprise à l'identique vaut à racine 16 px, et là seulement** (D4).
- **Rien de l'anti-FOUC observé** : l'amorce est prouvée **injectée** (T10
  Step 3) et **placée** (T10 Step 2) ; **qu'aucun éclair de mauvais thème ne
  soit visible n'est mesuré par rien** — cela demanderait une capture
  chronométrée, que §7.8 écarte.
- **Aucune primitive, aucune surface habillée** : c'est S2 et S3.
- **Rien du manifest PWA, de `theme-color`, du Window Controls Overlay** : ils
  appartiennent à ②, et `client/` n'a **aucun** manifest (vérifié par `grep`).
- **Aucun appelant de `getComputedStyle`** : la règle de lecture d'une valeur
  depuis TypeScript (spec §4.1) est écrite **pour le premier qui en aura
  besoin**, et il n'y en a aucun.
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
  remplacement (cadrage §11, spec §4.6).

- [ ] **Step 5 : commit**

```bash
git add docs/superpowers/plans/2026-08-19-design-system-s1-resultats.md \
        docs/superpowers/plans/journaux-design-system-s1
git commit -m "recette(s1): les sept controles, deux executions chacun, pieces versees" \
  -- docs/superpowers/plans/2026-08-19-design-system-s1-resultats.md \
     docs/superpowers/plans/journaux-design-system-s1
```

---

### Task 15 : revue transverse de fin de branche, et `CLAUDE.md`

**Objet :** chercher les affirmations — commentaires, documents, en-têtes —
devenues **FAUSSES dans la branche elle-même**, puis écrire la section S1 de
`CLAUDE.md`.

**Files:**
- Modify: `CLAUDE.md`, et tout fichier dont un commentaire est devenu faux

⚠️ **Cette tâche n'est jamais facultative.** La revue transverse a trouvé
**cinq** défauts en D7, **trois** Critiques en D8, **six** en D9, **douze** en
D10, **sept** en D11, **huit** en P1 et **dix** en P2 — ces dix-là réparties
sur **vingt-trois places**, parce que « corrigé à sa place » est une
affirmation de **complétude**. Ils ont tous la même forme : **corrects des deux
côtés pris séparément**, faux ensemble. **Une revue par tâche ne peut
structurellement pas les voir.**

- [ ] **Step 1 : la cible propre de cette revue — S1 produit deux classes d'énoncés faux**

**Classe A : les phrases qui décrivent l'absence que S1 comble.** Elles sont
**énumérables**, et c'est ce qui rend ce balayage faisable :

```bash
grep -rn "color-scheme\|thème clair\|theme clair\|pas de thème\|seul fichier CSS\|aucune feuille" \
     client CLAUDE.md docs/superpowers/specs --include="*.ts" --include="*.css" \
     --include="*.html" --include="*.md" | grep -v node_modules | grep -v "/dist/"
grep -rn "85 lignes\|deux surfaces\|deux entrées\|25 737\|1 055\|couleurs littérales" \
     CLAUDE.md docs/superpowers
```

**Classe B : les commentaires du code que S1 rend faux.** Candidats connus
d'avance, chacun à relire :

| Fichier:ligne | Ce qu'il dit aujourd'hui | Ce que S1 en fait |
| --- | --- | --- |
| `client/connexion.html:9-13` | « AUCUNE DIRECTION VISUELLE ICI […] elle appartient au sous-projet ⑥ » | **reste vrai** — S1 donne les tokens, pas des primitives ; c'est **S3** qui le retirera. **À ne PAS toucher** |
| `client/src/style.css:79-83` | la zone de ~40×36 px du bouton plein écran | **reste vrai** — et D2 existe pour qu'il le reste. Vérifier qu'aucune longueur n'a bougé |
| `client/src/style.css:128-139` | l'encadré du micro sans règle d'atténuation | **reste vrai** — S1 ne change que ses couleurs |
| `client/vite.config.ts:9-13` | « une page absente de cette liste ne sort pas du build » | **reste vrai, et gagne un cas** : `design.html` |
| `client/src/jeton.ts:12-18` | l'arbitrage `localStorage` | **reste vrai**, et S1 y ajoute un second occupant — vérifier que rien n'y prétend l'exclusivité |

🔴 **La leçon que D10 a payée sept fois est le patron à chasser** : *une
affirmation écrite par une tâche et réfutée par une autre tâche de la MÊME
branche.* Candidat le plus probable ici : **un en-tête écrit aux Tasks 1 à 3 qui
dit qu'un contrôle est rouge — il l'était en écrivant, il ne l'est plus après
les Tasks 9 et 11.** Les relire **toutes les trois**, et remplacer « ce contrôle
est rouge » par « ce contrôle a été vu rouge le <date>, à `<commit>`, sur
`<n>` occurrences » — un relevé **daté** reste vrai comme histoire, un présent
devient faux.

- [ ] **Step 2 : le relevé de tailles, PAR LA COMMANDE, APRÈS la dernière édition**

🔴 **Une table mesurée en début de ronde est fausse à la fin de la même ronde** —
D8 a commis cette erreur en croyant bien faire.
🔴 **ET IL FAUT DIRE À QUEL COMMIT** : l'arbre est partagé avec le chantier E.

```bash
git rev-parse --short HEAD
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | grep '^client/' | xargs wc -l 2>/dev/null | sort -rn | head -12
```

⚠️ **Mesurer CHAQUE ligne de la table au moment de l'écrire, jamais la relire.**
D11 a écrit **cinq** chiffres sans les mesurer, et **les cinq étaient faux** ;
ils ont été attrapés en relançant `wc -l` sur les tables entières. **La défense
n'est pas de mieux se souvenir.**

⚠️ **Et si un fichier de plus de 500 lignes apparaît qui n'est pas de S1, le
dire sans se l'attribuer.** Ce cas s'est produit pendant la rédaction de ce
plan : `client/src/webrtc.test.ts` valait **517** à `d280745`, hors plafond, et
le chantier E l'a scindé de lui-même à `8ad03a2`. **Un chiffre du voisin se
relève et s'attribue au voisin.**

- [ ] **Step 3 : écrire la section `CLAUDE.md`**

Une section `## 🎨 Sous-projet ⑥ Design system — sous-bloc S1 : le socle …
(19 août 2026)`, placée **après** la section P2, portant :

1. les renvois — plan, spec (commit `5b6b830`), résultats, journaux, **avec
   leur famille de lecture** (les journaux de S1 sont des sorties `npm`/`node`
   sur l'hôte : **UTF-8, aucune séquence ANSI attendue** — le vérifier par
   `grep -lP '\x1b\[' docs/superpowers/plans/journaux-design-system-s1/*` et
   **dire le résultat**, pas le supposer) ;
2. ⛔ **« Aucune tâche de S1 n'a employé la VM Windows »**, et pourquoi ;
3. les **sept contrôles** avec leur verdict et leur nombre d'exécutions ;
4. 🔴 **le fait le plus réutilisable : Node v24.9.0 permet à un `.mjs`
   d'importer un `.ts`** (D6), avec sa contrainte « effaçable » et la sortie
   d'erreur de l'`enum` — c'est ce qui permet à trois contrôles de partager un
   parseur au lieu de le recopier, **sans dépendance** ;
5. le tableau des **huit relevés périmés** de la spec, avec leur valeur juste ;
6. les **onze divergences** D1…D11 en une phrase chacune ;
7. les **variables d'environnement** : **aucune** — et le dire, parce que le
   tableau du dépôt en compte beaucoup et qu'une absence se déclare ;
8. les **pièges neufs**, dont : `injectTo` par défaut place le script avant
   `<meta charset>` ; le nom de l'actif CSS partagé n'est pas prévisible ; le
   mot-clé `red` d'un contrôle de couleur lève un faux positif sur du français ;
   `client/vite.config.ts` n'est pas typechecké ;
9. **ce que S1 n'établit PAS**, repris du document de résultats **sans le
   raboter**, dont les **huit jugements humains** du §8 ;
10. **le relevé de tailles du Step 2**, avec son `HEAD` ;
11. **les legs**, dont : `--police-mono` n'a qu'un appelant ;
    `prefers-reduced-motion` nommé et non pris ; les trois longueurs hors
    échelle de D2 que **S4** doit reprendre ; le plafond de 12 Kio non calibré ;
    la corroboration à deux fenêtres réelles si elle n'a pas eu lieu.

- [ ] **Step 4 : commit**

```bash
git add CLAUDE.md   # plus tout fichier dont un commentaire a été corrigé, NOMMÉ
git commit -m "docs(s1): la section S1, et la revue transverse de fin de branche" -- CLAUDE.md
```

---

## Ordre et dépendances

```
  T1 §7.2 ──┐
  T2 §7.3 ──┼── (arbre INTACT, aucune modification de style)
  T3 §7.7 ──┘
      │
      ├──> T4 tokens.ts ──┬──> T5 contraste.ts ──┐
      │                   │                      │
      │                   └──────────────────────┼──> T7 tokens.css
      │                                          │        │
      ├──> T6 theme.ts ──────────────────────────┼────────┤
      │                                          │        v
      │                                          │   T8 base.css + socle.css
      │                                          │        │
      │                                          │        v
      │                                          │   T9 style.css aux tokens  [§7.2 VERT]
      │                                          │        │
      │                                          │        v
      └──────────────────────────────────────────┴──> T10 amorce + greffon
                                                           │
                                                           v
                                                      T11 shell + connexion  [§7.3 VERT]
                                                           │
                                                           v
                                                      T12 §7.6 + galerie
                                                           │
                                                           v
                                                      T13 verify-all (10ᵉ étape)
                                                           │
                                                           v
                                                      T14 recette ──> T15 revue + CLAUDE.md
```

**Ce qui se parallélise** :

- **T1, T2, T3 ensemble** — trois fichiers distincts, aucune dépendance, et
  **aucun ne modifie une ligne de style**. Ce sont les trois seules tâches dont
  le verdict porte sur l'arbre intact : **elles doivent toutes être faites avant
  T7**, faute de quoi leur rouge devient inobservable.
- **T4, T6 ensemble** — `tokens.ts` et `theme.ts` ne se connaissent pas. T5
  dépend de T4 (il consomme `BlocDeTheme`).
- **T5 et T6 ensemble**, une fois T4 faite.

**Ce qui ne se parallélise PAS, et pourquoi** :

- 🔴 **T8 avant T9, sans exception.** T8 libère la racine de son `font-size` ;
  sans elle, tous les décalages passés aux tokens par T9 valent 10,5 px au lieu
  de 12, **et aucun contrôle ne le dit** (D4).
- **T7 avant T8** : `base.css` référence des tokens qui doivent exister, sans
  quoi §7.6 serait rouge pour une raison sans intérêt.
- **T9 avant T11** : le §7.3 vert exige que les feuilles existent et déclarent
  `--fond-0`.
- **T12 après T9 et T11** : §7.6 balaie les feuilles **de production**, qui
  doivent être dans leur état final.
- **T13 après T12** : l'agrégateur lance les six scripts, ils doivent exister.
- **T14 après T13**, **T15 après T14** — et T15 relève ses tailles **après sa
  propre dernière édition**.

⚠️ **Une seule tâche touche un fichier que le chantier voisin modifie** : T9
(`client/src/style.css`). Elle relit le fichier à l'instant de l'écrire (D10,
T9 Step 1), et **ne se fie pas à l'inventaire de ce document**.
