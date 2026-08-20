# Sous-bloc S3 — les deux surfaces existantes habillées : la page-shell et l'écran de connexion

**Spécification :** `docs/superpowers/specs/2026-08-19-design-system-design.md`
(commit `5b6b830`), §6 « S3 », §5.2 famille 3, §3, §4.5, §7, §8, §9, §10.
**Ce plan ne modifie pas la spec** — S1 et S2 ne l'ont pas modifiée non plus.

**Sous-blocs précédents, livrés et clos :**
- **S1** — le socle : `docs/superpowers/plans/2026-08-19-design-system-s1-resultats.md`,
  journaux `journaux-design-s1/` ;
- **S2** — les primitives : `docs/superpowers/plans/2026-08-19-design-system-s2-resultats.md`,
  journaux `journaux-design-s2/`.
**Ils font autorité sur l'état réel**, et ce plan les a relus avant d'écrire une
ligne. `CLAUDE.md` §« Sous-projet ⑥ … S1 » (l. 8500) et §« … S2 » (l. 8904)
portent le même relevé, en plus court.

**Commit de base de ce plan : `920a1eb`.** ⚠️ **L'arbre est partagé** : le
chantier ⑦ (pont de fichiers, F1) a commité `1cb29b8` et `8d508ca` **après** la
clôture de S2, et **ils touchent `client/shell.html` et
`client/src/shell-page.ts`** — deux des fichiers centraux de ce sous-bloc. Voir
le risque n°1.

**Document de résultats à produire :**
`docs/superpowers/plans/2026-08-19-design-system-s3-resultats.md`.
**Journaux à verser :** `docs/superpowers/plans/journaux-design-s3/`.

---

## 0. Ce que S3 fait, en une phrase

**S3 est le premier sous-bloc de ⑥ où l'apparence du produit bouge réellement.**
S1 s'interdisait tout changement d'apparence sur `index.html` ; S2 n'a lié ses
primitives à aucune surface du produit. S3 habille les **deux surfaces
existantes** — la page-shell et l'écran de connexion — et **referme l'écart que
S2 a nommé** : *« les dix-huit tokens sortis de la liste ont un appelant ÉCRIT,
pas un pixel RENDU »*.

---

## 1. Constantes de mesure — relevées le 20 août 2026, et par quelle commande

🔴 **Toutes les valeurs ci-dessous ont été obtenues en lançant la commande, à
`920a1eb`.** Aucune n'est recopiée d'un document. Ce plan a été écrit **sans
modifier aucun fichier de code** ; les deux mesures qui exigeaient une mutation
ont été jouées dans une **copie jetable** de l'arbre, hors du dépôt (§4.3, §4.4).

### 1.1 Les sept contrôles sont verts

`cd client && npm run design:verifier` → **`6/6 contrôle(s) vert(s)`**, `exit=0` :

| Contrôle | Relevé du 20 août 2026 |
| --- | --- |
| §7.1 contrastes | **52 paires**, **0 échec**, **minimum global 3.16** |
| §7.2 couleurs littérales | **61 fichiers balayés**, **0** couleur littérale |
| §7.3 surfaces bâties | **5 pages** ; assertion A **0 échec** ; assertion B **0 échec, évaluée sur 5 pages** |
| §7.4 blocs de thème | racine **48**, media-clair **14**, attribut-clair **14**, **0 écart** |
| §7.6 tokens orphelins | **48 déclarés / 38 employés**, **11 fichiers** au périmètre, **10 orphelins = 10 en attente**, **0 écart** |
| §7.7 poids CSS | 1 493 + 2 702 + 2 179 = **6 374 octets**, plafond **12 288**, **marge 5 914** |

Le septième, **§7.5**, est un test unitaire : il tourne dans `npm test`.

### 1.2 Les suites

`cd client && npm test` → **219 tests, 24 fichiers**, tous verts.
`cd proto && npm test` → **111 tests, 5 fichiers**, tous verts.

⚠️ **Ces deux comptes ont bougé sous des chantiers voisins depuis la clôture de
S2** (S2 relevait `client` 196 / 22 et `proto` 79 / 4) : `client/src/fichiers/`
est né entre-temps. **Aucune tâche de S3 n'en est responsable, et chaque tâche
qui ajoute des tests annonce le compte attendu AVANT de le mesurer.**

### 1.3 Les tailles

```
$ { git ls-files; git ls-files --others --exclude-standard; } \
    | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
    | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
   1536 agent/src/encode.rs
    630 agent/src/windows_source.rs
```

**Le dépôt entier n'a que DEUX fichiers au-dessus de 500 lignes** — les deux
lignes de la dette gelée — et **aucun de `client/`**. Les plus gros de `client/`,
relevés par la même commande :

| Fichier | Lignes | Porte | Marge |
| --- | --- | --- | --- |
| `client/verify-webrtc.mjs` | **494** | 500 | 🔴 **6** — ⚠️ **intouché par S3** |
| `client/src/main.ts` | **451** | 500 | 49 — voisin (chantier E), pas de S3 |
| `client/src/webrtc.session.test.ts` | 419 | 500 | 81 — voisin |
| `client/src/micro.test.ts` | 411 | 500 | 89 — voisin |
| `client/src/design/primitives.test.ts` | **283** | **300** | 🔴 **17** |
| `client/design.html` | **243** | 300 | 57 |
| `client/src/design/tokens.css` | **232** | 300 | 68 |
| `client/outils/couleurs-litterales.mjs` | **228** | 300 | 72 |
| `client/outils/tokens-orphelins.mjs` | **202** | 300 | 98 |
| `client/primitives.html` | **199** | 300 | 101 |
| `client/src/style.css` | **184** | 300 | 116 |
| `client/src/design/tokens.ts` | **176** | 300 | 124 |
| `client/src/shell-page.ts` | **167** | 300 | 133 |
| `client/src/design/galerie.ts` | **162** | 300 | 138 |
| `client/src/design/contraste.ts` | **156** | 300 | 144 |
| `client/src/design/theme.test.ts` | **156** | 300 | 144 |
| `client/outils/tokens-orphelins/attente.mjs` | **146** | 300 | 154 |
| `client/src/shell.test.ts` | **141** | 300 | 159 |
| `client/src/design/base.css` | **134** | 300 | 166 |
| `client/src/design/tokens.test.ts` | **133** | 300 | 167 |
| `client/src/shell.ts` | **124** | 300 | 176 |
| `client/vite.config.ts` | **112** | 300 | 188 |
| `client/src/design/theme.ts` | **107** | 300 | 193 |
| `client/src/design/primitives/bouton.css` | **103** | 300 | 197 |
| `client/src/design/selecteur-theme.ts` | **84** | 300 | 216 |
| `client/src/connexion.ts` | **75** | 300 | 225 |
| `client/src/design/primitives/champ.css` | **75** | 300 | 225 |
| `client/src/design/primitives.css` | **73** | **240** | 167 |
| `client/connexion.html` | **47** | 300 | 253 |
| `client/shell.html` | **32** | 300 | 268 |
| `client/index.html` | **26** | 300 | 274 |

🔴 **La marge la plus étroite de `client/` est `verify-webrtc.mjs` : 6.**
**Aucune tâche de S3 ne le touche.**
🔴 **La plus étroite parmi les fichiers de ⑥ est `primitives.test.ts` : 17**, et
son point de chute est **nommé par S2** — scinder par objet, les gardes de forme
(G1 à G5, G7) d'un côté, les trois gardes de famille (G6) de l'autre. **Voir la
clause conditionnelle des Global Constraints.**

⚠️ **Quatre chiffres publiés au §7 du document de résultats de S2 (relevé à
`070b48b`) ont été périmés par la revue transverse de S2 elle-même** (commit
`697735e`) : `primitives.test.ts` 270 → **283**, `design.html` 231 → **243**,
`tokens.css` 225 → **232**, `style.css` 181 → **184**. **Le tableau ⑨ de
`CLAUDE.md`, lui, est à `697735e` et il est EXACT** — revérifié par la commande
ci-dessus, ligne par ligne. **C'est ce tableau-là qu'il faut lire, pas celui du
document de résultats.**

### 1.4 `scripts/verify-all.sh`

```
$ grep -c '^etape "' scripts/verify-all.sh
10
```

**Dix étapes.** Une **exécution complète** affiche **dix-sept** en-têtes `==>`
(les 10 étapes plus les 7 de `design:verifier` : un build et six contrôles).
⚠️ **S3 ajoute un contrôle (§7.9, tâche 2) : ce dix-sept devient DIX-HUIT.**
**Le relever à la recette, jamais le sommer** — c'est la divergence D5 de S2,
et elle se rejoue à chaque addition.

⚠️ **Ce plan ne prétend PAS que `verify-all.sh` sort à 0 aujourd'hui** : il n'a
pas été lancé pendant la rédaction, sa première étape étant `cargo test
--workspace` sur un `agent/` que des chantiers voisins modifient, et son étape
`plateforme : npm run test:postgres` exigeant une instance Postgres. **Si une
étape étrangère tombe à la recette : le déclarer, relever `git status`, et juger
S3 sur les étapes `client` et `proto`** — jamais masquer, jamais imputer.

---

## 2. Ce que S2 lègue à S3, et ce que ce plan en fait

Les quatre legs « à S3 » du §⑪ de `CLAUDE.md`, plus les deux legs « sans
sous-bloc assigné » que ce plan revendique :

| # | Legs de S2 | Ce que S3 en fait |
| --- | --- | --- |
| 1 | ⛔ **Lier `primitives.css` aux surfaces du produit** — l'écart « appelant écrit / pixel rendu » | **PRIS** — tâches 4 et 5, **et** un contrôle neuf (§7.9, tâche 2) parce que **§7.6 ne peut pas voir cet écart** (§4.2) |
| 2 | ⛔ **Neuf entrées de liste d'attente**, dont trois re-étiquetées | **PRIS** — tâches 4, 5, 7. La consommation des neuf est **mesurée faisable** (§3.2) |
| 3 | ⛔ **Le sélecteur de thème du PRODUIT reste à écrire** | **PRIS** — tâche 6, et le défaut préexistant d'`aria-pressed` est corrigé |
| 4 | ⛔ **Ni `primitives.html` ni `galerie-primitives.ts` n'ont de test** | **PARTIELLEMENT PRIS** — l'assertion B de §7.9 (tâche 2) attrape « une famille cesse d'être rendue par `primitives.html` ». **Elle ne teste pas `galerie-primitives.ts`**, et c'est déclaré (§4.5) |
| 7 | 🔴 **Construire la mitigation partielle du re-étiquetage** | **PRIS** — tâche 7. Décision motivée au §4.4 |
| 8 | 🔴 **Fermer le trou de §7.4** (`racine ⊆ clair` jamais comparé) | **PRIS** — tâche 1. Décision motivée au §4.3 |

**Ce que S3 ne prend PAS, et pourquoi :**

| # | Legs | Pourquoi S3 ne le prend pas |
| --- | --- | --- |
| 5 | ⛔ **`--police-mono`** | **S4**, et lui seul : le câbler change l'apparence de `#stats`, dans la fenêtre de session, que S3 n'a pas le droit de toucher. **S3 ne rouvre pas cette entrée**, comme S2 ne l'a pas rouverte |
| 6 | ⛔ **Les trois longueurs hors échelle de `style.css`** | **S4** — elles vivent dans la fenêtre de session. La clause « aucune longueur hors échelle » du §8 **reste FAUSSE à la fin de S3**, de trois valeurs exactement |
| 9 | ⛔ **`tokens.ts` ne sait nommer que trois blocs** | S3 n'ajoute aucune requête média à `tokens.css`. ⚠️ **La tâche 1 touche `tokens.ts` sans toucher `lireBlocsDeTheme`** : le legs reste entier |
| 10 | ⛔ **Le plafond de 12 288 octets n'est calibré par rien** | **Il le reste.** S3 en consomme davantage, et le chiffre est relevé (§5) |
| 11 | 🔴 **`primitives.test.ts`, marge 17** | **Armé, pas pris d'office** : clause conditionnelle des Global Constraints — toute tâche qui y ajoute une assertion **extrait d'abord** |
| 12 | ⛔ **Le défaut à deux réglages de `build-agent.sh`** | ⛔ **Aucune tâche de S3 n'emploie la VM Windows** (spec §9) |

---

## 3. 🔴 La liste d'attente : dix aujourd'hui, et ce que S3 en consomme

### 3.1 L'état relevé, par la commande

```
$ node client/outils/tokens-orphelins.mjs
source        : client/src/design/tokens.css — 48 token(s) déclaré(s)
périmètre     : 11 fichier(s) — 38 token(s) employé(s)
inclusion ① — tout var(--…) est déclaré : 0 écart(s)
inclusion ② — tout token déclaré a un appelant : 10 orphelin(s), dont 10 en attente déclarée
total : 0 écart(s)
```

**Dix entrées**, lues dans `client/outils/tokens-orphelins/attente.mjs:120-144` :

| Token | Annotation, verbatim | Attribué à |
| --- | --- | --- |
| `--t-xs` | « S3 ou plus tard — la mention légale et les étiquettes » | **S3** (re-étiqueté par S2) |
| `--e-1` | « S3 ou plus tard — l'écart interne d'une étiquette » | **S3** (re-étiqueté) |
| `--r-plein` | « S3 ou plus tard — les pastilles et les boutons ronds » | **S3** (re-étiqueté) |
| `--t-2xl` | « S3 — le titre de l'écran de connexion » | **S3** ⚠️ voir D1 |
| `--t-3xl` | « S3 — le titre du hub » | **S3** ⚠️ voir D1 |
| `--lh-large` | « S3 — les paragraphes longs » | **S3** |
| `--e-5` | « S3 — la gouttière entre cartes » | **S3** |
| `--e-6` | « S3 — la marge des sections » | **S3** |
| `--e-7` | « S3 — la marge de tête des surfaces » | **S3** |
| `--police-mono` | « S4 — #stats, OU RETRAIT : le seul token dont le sort est encore ouvert » | **S4** |

**Neuf pour S3, un pour S4.**

### 3.2 🔵 Que les neuf soient consommables est MESURÉ, pas prédit

Le plan de S1 a annoncé neuf occurrences là où il y en avait onze et a
**attribué l'écart à une cause fausse** ; celui de S2 a prédit une répartition
que son propre tableau contredisait. **Ce plan ne prédit donc pas : il a joué
la consommation dans une copie jetable de l'arbre**, hors du dépôt, le 20 août
2026 — deux feuilles de surface plausibles (`src/shell.css`, `src/connexion.css`)
écrites, aucun fichier du dépôt touché, copie détruite après relevé :

```
$ node client/outils/tokens-orphelins.mjs
périmètre     : 13 fichier(s) — 47 token(s) employé(s)
inclusion ② — tout token déclaré a un appelant : 1 orphelin(s), dont 1 en attente déclarée
  À RETIRER DE LA LISTE  --e-1       … (client/src/shell.css)
  À RETIRER DE LA LISTE  --e-5       … (client/src/connexion.css, client/src/shell.css)
  À RETIRER DE LA LISTE  --e-6       … (client/src/connexion.css, client/src/shell.css)
  À RETIRER DE LA LISTE  --e-7       … (client/src/shell.css)
  À RETIRER DE LA LISTE  --lh-large  … (client/src/connexion.css, client/src/shell.css)
  À RETIRER DE LA LISTE  --r-plein   … (client/src/shell.css)
  À RETIRER DE LA LISTE  --t-2xl     … (client/src/shell.css)
  À RETIRER DE LA LISTE  --t-3xl     … (client/src/connexion.css)
  À RETIRER DE LA LISTE  --t-xs      … (client/src/shell.css)
total : 9 écart(s)     [exit=1, relevé séparément]
```

**Les NEUF sont nommés par le contrôle, et le seul orphelin restant est
`--police-mono`, celui de S4.** ⚠️ **Ce relevé
établit la FAISABILITÉ, pas l'obligation** : les feuilles jetables ne sont pas
celles que S3 écrira, et **le contrôle dit quoi retirer ; ce plan dit seulement
à quoi s'attendre** (la formule est de S2, et c'est elle qui a sauvé sa
tâche 8).

### 3.3 🔴 Prédiction par tâche — et l'invariant qui la gouverne

| Tâche | Tokens attendus | Compte |
| --- | --- | --- |
| **T4** — la page-shell | `--t-2xl`, `--e-5`, `--e-6`, `--e-7`, `--t-xs`, `--e-1`, `--r-plein` | **7** |
| **T5** — l'écran de connexion | `--t-3xl`, `--lh-large` | **2** |
| **total** | | **9** |
| **reste** | `--police-mono` | **1** |

🔴 **LE CONTRÔLE §7.6 EXIGE L'ÉGALITÉ, PAS UNE INCLUSION : chaque sortie de la
liste se fait DANS LE MÊME COMMIT que la mise en service du token.** Un commit
qui écrit `var(--e-5)` sans retirer `--e-5` de la liste fait rougir
`À RETIRER DE LA LISTE` ; un commit qui retire l'entrée sans écrire l'appelant
fait rougir `NOUVEL ORPHELIN`. **Les deux sens ont été vus rouges en S1
(rouges A et A2) — ce n'est pas une hypothèse.**

⚠️ **Si la répartition réelle diffère, SUIVRE LE CONTRÔLE et le déclarer**, pas
fabriquer un appelant pour faire tomber le compte. « Fabriquer une pastille dans
le seul but de vider trois lignes aurait été vider un contrôle pour en verdir un
autre » — `attente.mjs:66-68`. **La même phrase vaut pour S3.**

⚠️ **`--lh-large` est le plus fragile des neuf.** Il n'a de sens que sur un
paragraphe réellement long. Il en existe un, et il est déjà écrit :
`client/src/shell.ts:56-57` compose « *« X » n'a pas pu s'ouvrir : le navigateur
a bloqué la pop-up. Autorisez les pop-ups pour ce site, puis rouvrez la
fenêtre.* » — deux phrases. **Si l'implémenteur juge que `--lh-large` n'y a pas
sa place, il ne le consomme PAS : il re-étiquette avec sa raison, et la
mitigation de la tâche 7 rend ce re-étiquetage VISIBLE au lieu de silencieux.**
C'est exactement ce pour quoi la tâche 7 existe.

---

## 4. Divergences relevées, tranchées AVANT d'écrire une ligne

### D1 — 🔴 `--t-2xl` et `--t-3xl` sont INTERVERTIS entre la spec et la liste d'attente, et l'un des deux nomme une surface que ⑥ ne livre pas

**Le relevé, cité et relu :**

- `docs/superpowers/specs/2026-08-19-design-system-design.md:366` —
  `| --t-2xl | 1.5 | 24 | titre de page |`
- `…:367` — `| --t-3xl | 2 | 32 | titre d'écran de connexion |`
- `client/outils/tokens-orphelins/attente.mjs:126` —
  `['--t-2xl', 'S3 — le titre de l’écran de connexion'],`
- `client/outils/tokens-orphelins/attente.mjs:127` —
  `['--t-3xl', 'S3 — le titre du hub'],`

**Les deux affectations sont ÉCHANGÉES**, et l'annotation de `--t-3xl` va plus
loin : elle nomme le **hub**, que la spec §6 exclut explicitement — « **Le hub
ne figure PAS dans ce découpage.** Il n'existe pas […] et ⑥ ne le livre pas ».
**L'entrée `--t-3xl` attribue donc à S3 un appelant que S3 ne peut pas
écrire.**

**Tranché : la spec l'emporte.** `--t-3xl` (32 px) est le **titre de l'écran de
connexion**, `--t-2xl` (24 px) le **titre de page** — donc celui de la
page-shell. **Les deux sont alors consommables par S3**, et le relevé du §3.2 le
confirme par la commande. **La tâche 5 corrige les deux annotations au passage,
avec sa raison**, comme la règle de revue d'`attente.mjs:59-60` l'exige.

⚠️ **C'est la troisième fois dans ⑥ qu'une annotation de la liste d'attente
prédit mal le sous-bloc** — S1 avait prédit S2 pour trois entrées que S2 ne
pouvait pas prendre, et c'est ce qui a produit les trois re-étiquetages. **Le
mécanisme qui rend ces erreurs visibles est précisément l'objet de la tâche 7.**

### D2 — L'écran de connexion EXISTE : le repli que la spec §6 prévoyait n'a pas lieu d'être

La spec §6 écrit : « **S3 dépend d'un travail qui n'est pas le sien** : la moitié
navigateur de P2 n'existe pas (§2.5) », et prévoit un repli — « la page-shell
d'abord, l'écran de connexion quand il existe ».

**Elle existe.** `client/connexion.html` (**47** lignes) et
`client/src/connexion.ts` (**75**) sont livrés, `connexion` est la troisième
entrée de `client/vite.config.ts`, et la page **porte déjà `socle.css`** depuis
la tâche 11 de S1. **Le repli est SANS OBJET, et S3 livre les deux surfaces.**

⚠️ **`client/connexion.html:10-25` porte un bloc de commentaire qui annonce
lui-même sa fin** : « *la page ne porte AUCUNE primitive — ni bouton, ni champ,
ni carte habillée. C'est **S3** qui les pose, et c'est lui qui retirera ce
bloc.* » **La tâche 5 le retire**, et ce retrait est une preuve à lui seul.

### D3 — 🔴 Poser une classe ne sort AUCUN token de la liste : le périmètre de §7.6 est le `var(--…)`, pas le balisage

Relevé dans `client/outils/tokens-orphelins.mjs:141-156` : l'ensemble « employé »
est bâti par `tokensReferences(...)` sur toutes les `.css` de `client/src/` plus
les entrées Vite — c'est-à-dire par les **occurrences de `var(--…)`**. Écrire
`class="bouton bouton--principal"` dans `shell.html` **n'ajoute aucun appelant** :
les tokens du bouton ont déjà le leur, `primitives/bouton.css`, depuis S2.

**Conséquence directe sur la forme de S3 : habiller ne suffit pas à consommer.**
S3 doit écrire de **vraies feuilles de surface** portant la mise en page propre
à chaque page — c'est là, et là seulement, que `--e-5`, `--e-6`, `--e-7`,
`--t-2xl`, `--t-3xl`, `--t-xs`, `--e-1`, `--r-plein` et `--lh-large` trouvent
leur `var(--…)`. **Le §3.2 le mesure : les neuf sortent depuis
`src/shell.css` et `src/connexion.css`, jamais depuis le HTML.**

### D4 — Une feuille neuve sous `client/src/` entre SEULE dans les périmètres : aucun script à modifier

Mesuré dans la copie jetable : le périmètre de §7.6 passe de **11 à 13
fichiers** sans qu'une ligne de `tokens-orphelins.mjs` ne change
(`fichiersCss(join(racine, 'client/src'))`, l. 141). Idem pour §7.2, qui balaie
`client/src/**` (**61 → 63 fichiers balayés**, mesuré, `0` couleur littérale).
**Aucun script de `client/outils/` n'a à changer de forme pour les feuilles de
surface** — seul le contrôle NEUF de la tâche 2 s'ajoute à l'agrégateur.

### D5 — 🔵 Vite PARTAGE `primitives-*.css` entre les pages qui la lient, et `index.html` NE BOUGE PAS — mesuré, pas raisonné

Copie jetable, `primitives.css` liée depuis `shell.html` **et** `connexion.html`
en plus de `primitives.html`, plus deux feuilles de surface :

```
2702  dist/assets/primitives-GyKd6lAV.css     ← le MÊME hash qu'avant : un seul actif, partagé
2179  dist/assets/socle-ZRS7erzW.css          ← inchangé
1493  dist/assets/main-CXZaIG5L.css           ← inchangé
 758  dist/assets/shell-B5dNUQWk.css
 414  dist/assets/connexion-CADX2CJJ.css
somme = 7546   (contre 6374 sur l'arbre intact)

dist/index.html      : socle-ZRS7erzW.css  main-CXZaIG5L.css
dist/shell.html      : socle-ZRS7erzW.css  shell-B5dNUQWk.css  primitives-GyKd6lAV.css
dist/connexion.html  : socle-ZRS7erzW.css  connexion-CADX2CJJ.css  primitives-GyKd6lAV.css
dist/primitives.html : socle-ZRS7erzW.css  primitives-GyKd6lAV.css
dist/design.html     : socle-ZRS7erzW.css
```

**Deux faits, tous deux structurels :**

1. **`primitives-*.css` n'est PAS dupliquée** — hash identique sur les trois
   pages qui la lient, une seule entrée dans `dist/assets/`. C'est
   l'arrangement B que S1 a mesuré et écrit dans `client/src/style.css:39-52` :
   **lier depuis le HTML laisse Vite partager ; `@import` ré-inline.**
   **Corollaire de conception : les feuilles de surface se LIENT depuis leur
   HTML, elles ne s'importent jamais depuis une autre feuille.**
2. 🔴 **`index.html` garde EXACTEMENT les deux mêmes actifs, aux mêmes hachages
   de contenu** — `socle-ZRS7erzW.css` et `main-CXZaIG5L.css`. **Lier les
   primitives depuis la page-shell et l'écran de connexion ne touche pas d'un
   octet la fenêtre de session**, et ce n'est pas un raisonnement.

⚠️ **Le `+1 172` n'est PAS une prédiction du poids final** : les deux feuilles
jetables ne sont pas celles que S3 écrira. **Ce que ce relevé établit est le
partage et l'invariance d'`index.html`, pas un nombre.** Le poids réel se
relève à la recette.

### D6 — Le trou de §7.4 : **S3 le ferme** (décision, §4.3)

### D7 — La mitigation du re-étiquetage : **S3 la construit** (décision, §4.4)

### D8 — 🔴 §7.6 est structurellement incapable de voir l'écart qu'il a lui-même nommé : S3 ajoute un HUITIÈME contrôle

Ce que S2 déclare (`CLAUDE.md` §⑩, et `primitives.css:38-42`) : « **les
dix-huit tokens sortis ont un appelant ÉCRIT, pas un pixel RENDU** : le
périmètre « employé » de ce contrôle est le FICHIER, jamais la surface. **C'est
S3 qui referme cet écart.** »

**Mais aucun des sept contrôles ne peut dire que l'écart est refermé.** D3
ci-dessus le montre : §7.6 comptera exactement les mêmes 38 tokens employés
avant et après que `shell.html` ait reçu ses classes. §7.3 ne vérifie que le
**chargement**, jamais l'emploi — sa propre en-tête le dit
(`surfaces-baties.mjs:33-37`). §7.2 ne voit que des couleurs.

**Décision : S3 ajoute un contrôle, §7.9, que la spec ne prévoit pas.** Il est
déclaré comme une **addition de plan**, la spec n'étant pas modifiée. Sa forme,
sa portée et sa rouge sont à la tâche 2.

🔵 **Et son atteignabilité est ACQUISE SANS RIEN CASSER, mesurée le 20 août
2026 :**

```
$ cd client && grep -n 'class=' index.html shell.html connexion.html
AUCUNE
$ grep -rn "classList\.\|className" src/ | grep -v '/design/'
AUCUNE occurrence hors design/
```

**Zéro classe sur les trois surfaces du produit.** Le contrôle naît donc
**ROUGE sur l'arbre intact**, comme §7.2 et §7.3 en S1 — et il passe au vert
quand la tâche 4 habille la page-shell.

### D9 — `selecteur-theme.ts` porte un défaut PRÉEXISTANT, déclaré, que le promouvoir au produit oblige à corriger

`client/src/design/selecteur-theme.ts:59-64`, verbatim : « ⚠️ **IL NE RAPPELLE
PAS `marquer()`**, ET C'EST LA TRANSPOSITION VERBATIM DU COMPORTEMENT D'AVANT
L'EXTRACTION : un `aria-pressed` posé ici reste celui du thème d'avant tant que
l'utilisateur ne clique pas dans CETTE fenêtre. Le défaut est réel et il est
PRÉEXISTANT ».

Sur un **instrument**, c'est une gêne. **Sur le produit, c'est un mensonge
d'interface** : le multi-fenêtres est la raison d'être du mécanisme `storage`
(spec §4.2), et le cas où une fenêtre voisine change le thème est le cas
nominal. **La tâche 6 le corrige, et la rouge est jouée sur le code
d'aujourd'hui.**

### D10 — `client/` n'a NI jsdom NI happy-dom : tout module de produit se teste par injection

```
$ cat client/package.json          → devDependencies : typescript, vite, vitest
$ ls client/node_modules | grep -E '^(jsdom|happy-dom)$'   → (vide)
$ grep -rln '@vitest-environment' client/src/             → aucun
```

**Aucun environnement DOM.** C'est ce que `client/src/design/theme.ts:4-9`
écrit déjà, et ce que `client/src/fullscreen.ts:17-20` applique. **Conséquence
directe sur la tâche 6** : le sélecteur de thème du produit **ne peut pas lire
`document` ni `window` dans le chemin testé** ; ses dépendances sont des
paramètres, et ses doubles sont écrits à la main, comme `faireBouton()` de
`client/src/fullscreen.test.ts:39`.

⚠️ **Et `client/src/shell.test.ts:15` pose `afficher: () => {}`** — un double en
**NO-OP**. C'est exactement le patron que D10 a nommé : « **une source factice
qui implémente un effet de bord en NO-OP rend une famille entière de défauts
invisible aux tests d'hôte** ». **La tâche 3 remplace ce no-op par une
collecte**, sans quoi son ton ne serait vérifié par rien.

### D11 — S3 ne livre AUCUNE primitive « lien », et le plan de S2 l'annonçait

Le tableau des états de S2 écrit : « ❌ `:visited` | non | S2 ne livre pas de
primitive « lien » — la spec §3 réserve l'accent au lien, **S3 le posera** ».

**Relevé :** `grep -n '<a \|<a>' client/{index,shell,connexion,primitives,design}.html`
→ **aucune ancre dans aucune entrée**. **Ni la page-shell ni l'écran de
connexion n'ont de lien**, et la redirection vers la connexion se fait par
`window.location.replace` (`client/src/shell-page.ts:40`), pas par une ancre.

**Tranché : S3 ne livre pas de primitive « lien ».** Poser une famille sans un
seul appelant serait précisément le code mort que §7.6 existe pour refuser.
**C'est un legs, et il est nommé** au §9.

### D12 — `--police-mono` reste à S4, et S3 ne le rouvre pas

`attente.mjs:133-143` : « **C'est donc S4, le sous-bloc qui a le droit de
toucher la fenêtre de session, qui tranche : ou il le câble, ou il le retire.
Aucun autre sous-bloc n'a le droit de laisser cette ligne en place sans
décider.** » ⚠️ **La phrase se lit comme une injonction à S3.** Elle ne l'est
pas : sa clause d'ouverture nomme S4 explicitement, et son unique appelant prévu
(`#stats`) vit dans `index.html`. **S3 laisse l'entrée telle quelle, sans la
re-étiqueter** — et la mitigation de la tâche 7 le lui permet, S4 n'étant pas
clos.

### D13 — Les tailles publiées par le document de résultats de S2 sont périmées ; celles de `CLAUDE.md` ne le sont pas

Voir §1.3. **Quatre chiffres**, périmés par la revue transverse de S2
elle-même. **Ce plan a remesuré les trente et une lignes de son tableau.**

---

## 4bis. Les deux décisions que ce plan devait prendre

### 4.3 🔴 L'angle mort de §7.4 : **S3 le ferme**, et sa portée s'élargit

**Le fait, mesuré par S2 et versé** (`journaux-design-s2/trou-7-4.log`) :
`--accent-survol` retiré des **deux** blocs clairs et laissé à la racine seule
rend `bloc racine : 48 / media-clair : 13 / attribut-clair : 13`, **`écarts : 0`,
`exit=0`**. Le code le confirme :
`client/src/design/tokens.ts:160-174` compare **clair ⇄ clair** (① ) et
**clair ⊆ racine** (② ), **jamais racine ⊆ clair**.

**Décision : S3 le ferme.** Trois raisons, dans cet ordre :

1. **La rouge est ACQUISE, pas à inventer.** La mutation exacte est déjà jouée,
   versée et datée. Un contrôle dont on connaît d'avance la mutation qui le fait
   tomber est un contrôle qu'on ne peut pas se tromper en écrivant.
2. **S3 est le premier sous-bloc qui écrit des feuilles de surface**, donc le
   premier à pouvoir découvrir une couleur oubliée dans les blocs clairs — par
   l'œil, sur une page claire, ce qui est le pire des deux mondes.
3. **Le laisser « sans sous-bloc assigné » est la façon dont les dettes
   pourrissent dans ce dépôt**, et sa propre doctrine le dit. S2 l'a légué en le
   nommant ; S3 le prend.

**⚠️ Ce que cela COÛTE, et il faut le dire : la portée du contrôle §7.4
change.** Son énoncé n'est plus « les trois blocs déclarent le même ensemble de
noms » mais **« les deux blocs clairs sont identiques, et tout token de COULEUR
de la racine y est redéclaré, sauf les hors-thème nommés »**. **Un contrôle de
S1 dont un sous-bloc ultérieur élargit la portée doit le déclarer dans son
en-tête, dans `CLAUDE.md`, et dans le document de résultats.**

🔵 **La fermeture est ARITHMÉTIQUEMENT PROPRE, et c'est mesuré** (20 août 2026,
par `lireBlocsDeTheme` sur l'arbre intact) :

```
racine : 48 tokens ; dont couleurs : 20
clair  : 14 tokens
couleurs de la racine ABSENTES du bloc clair : 6
  --video-letterbox --voile-flottant --voile-bouton
  --voile-bouton-survol --voile-micro-actif --voile-micro-refuse
```

**Les six manquants sont EXACTEMENT les six voiles hors thème**, déjà nommés et
justifiés comme tels dans `client/src/design/tokens.css:80-102` — « Six voiles
HORS THÈME — déclarés une fois, jamais redéfinis ». **Il n'y a donc pas de cas
douteux à arbitrer** : 20 − 6 = 14 = le compte exact des deux blocs clairs, et
la fermeture rend **zéro écart** dès aujourd'hui.

**La forme retenue** : une **liste nommée de six hors-thème**, jamais un
prédicat de nom (`--voile-*`), parce qu'un préfixe est une convention qu'une
faute de frappe contourne. ⚠️ **Le coût assumé** : c'est une seconde copie d'un
fait déjà écrit dans le commentaire de `tokens.css`. **Ce que cela achète en
échange** : une couleur hors thème ajoutée sans être listée fait rougir le
contrôle, ce qui **force la question** « hors thème, ou blocs clairs oubliés ? »
au lieu de la laisser passer. C'est la forme de la liste d'attente, en plus
petit.

### 4.4 🔴 La mitigation du re-étiquetage : **S3 la construit**

**Le fait :** `attente.mjs:54-58` — « Le contrôle compare des ENSEMBLES DE NOMS.
Changer « S2 » en « S3 » dans une annotation ne déclenche rien, dans aucun des
deux sens, jamais. C'est **le point le plus faible de ce dispositif** ». Et
`attente.mjs:69-80` réfute le plan de S2 : une mitigation **partielle** existe —
*aucune entrée ne doit nommer un sous-bloc déjà clos* — et **elle n'est pas
construite**.

**Décision : S3 la construit.** Trois raisons :

1. **Elle vise exactement l'entrée qui en a besoin.** `--police-mono` est « le
   SEUL token dont le sort soit encore ouvert » et son entrée porte
   « **aucun sous-bloc n'a le droit de laisser cette ligne en place sans
   décider** » — une injonction en prose que **rien n'applique**. La mitigation
   l'applique, et c'est S4 qui la subira, comme prévu.
2. **S3 est le sous-bloc qui va re-étiqueter.** D1 oblige à corriger deux
   annotations, et `--lh-large` pourrait devoir en recevoir une troisième
   (§3.3). **Construire le garde-fou dans le sous-bloc qui s'apprête à en avoir
   besoin est la seule façon de savoir qu'il mord.**
3. **Le dépôt a une doctrine sur les affirmations réfutées** : le plan de S2 a
   écrit **trois fois** qu'aucune mitigation n'était possible, et cette
   affirmation est réfutée par écrit. **La laisser réfutée et non traitée serait
   la garder vivante.**

⚠️ **PARTIELLE, et le mot reste pesé.** Elle juge le **sous-bloc nommé**, jamais
le **contenu** — « S3 — la gouttière entre cartes » changé en « S3 — n'importe
quoi » lui échappe. Et elle **exige que le dépôt sache quels sous-blocs sont
clos**, ce qu'aucun fichier ne dit aujourd'hui : la tâche 7 le déclare, **dans
le même fichier que la liste**, et la clause de tenue est celle de la liste
elle-même — « ce nombre est tenu à jour par la tâche qui le rend faux »
(`attente.mjs:35`).

⚠️ **Après S3 elle garde UNE entrée**, et c'est une objection qu'il faut
écrire : un mécanisme pour une ligne. **Elle est prise quand même parce que
cette ligne-là est précisément celle dont la prose dit qu'elle ne doit pas
survivre à un sous-bloc sans décision** — et parce qu'une mitigation construite
après la faute qu'elle devait empêcher n'aurait plus rien à empêcher.

---

## 5. 🔴 Ce qui change VISUELLEMENT, et ce qui est repris à l'identique

**S1 et S2 s'interdisaient tout changement d'apparence. S3 ne le peut pas** —
c'est l'objet même du sous-bloc. La liste ci-dessous est donc **le contrat**, et
tout ce qui n'y figure pas est une régression.

### 5.1 Les changements d'apparence ASSUMÉS

| # | Surface | Avant (relevé) | Après | Statut |
| --- | --- | --- | --- | --- |
| ① | `shell.html` | quatre éléments nus : `<h1>`, `<div id="statut">`, `<button id="choisir-dossier">`, `<div id="etat-fichiers">`, `<ul id="fenetres">` sans aucune classe | page composée : titre en `--t-2xl`, sections espacées, **grille de cartes** pour les fenêtres, boutons de primitive | 🔴 **majeur, assumé** |
| ② | `connexion.html` | `<h1>` et trois `<p>` nus portant deux `<input>` et un `<button>` | **carte centrée**, titre en `--t-3xl`, champs `.champ`, action `.bouton--principal` | 🔴 **majeur, assumé** |
| ③ | les deux | aucun sélecteur de thème | **trois boutons de thème apparaissent** — élément neuf sur les deux surfaces | 🔴 **neuf, assumé** (§6.2) |
| ④ | `#statut` (shell) | texte nu, une seule apparence | **prend un TON** : neutre pour « bureau connecté », danger pour un refus ou une pop-up bloquée | assumé |
| ⑤ | `#etat-fichiers` (shell) | texte nu | **prend un ton** : neutre au montage, danger à l'échec | assumé |
| ⑥ | `#message` (connexion) | texte nu | **prend un ton** : neutre pendant « connexion… », danger au refus et à l'injoignable | assumé |
| ⑦ | les deux | anneau de focus déjà global depuis S1 | **inchangé**, mais désormais VISIBLE sur des contrôles habillés | conséquence assumée |

### 5.2 Les reprises à l'identique — et ce qui les prouve

| # | Ce qui NE change pas | Ce qui le prouve |
| --- | --- | --- |
| ① | 🔴 **La fenêtre de session** : `index.html`, `style.css`, les cinq éléments, les trois longueurs hors échelle, les six voiles | **mesuré** (D5) : `dist/index.html` garde `socle-*.css` et `main-*.css` **aux mêmes hachages de contenu** ; plus le garde de la tâche 2 sur l'ensemble des feuilles liées par `index.html` ; plus un `git diff` vide à la recette |
| ② | **La palette** : aucune couleur neuve, aucun token de couleur ajouté | §7.1 reste à **52 paires**, minimum **3,16**, et §7.4 (élargi) à **0 écart** |
| ③ | **Les primitives de S2** : aucune règle de `primitives/*.css` n'est modifiée | §7.6 : les 38 tokens employés ne bougent pas de ce côté ; les gardes G1 à G7 restent verts |
| ④ | **`--police-mono` et les trois longueurs hors échelle** | ils restent à S4 (D12, §2) |
| ⑤ | **Le legacy** — `assets/`, `web/`, `src/`, `index.js` | ni lus ni modifiés, et aucun contrôle ne les balaie (spec §4.6) |
| ⑥ | **Le comportement du produit** : aucune session, aucun jeton, aucun message de signaling ne change | les règles restent dans `shell.ts` et `jeton.ts` ; les seules règles neuves sont les tons (tâche 3), et elles sont testées |

🔴 **Le point ① est un CRITÈRE DE RECETTE, pas une intention** — voir la
tâche 8, critère ③.

---

## 6. Interfaces et décisions de conception

### 6.1 Les feuilles de surface — une par page, LIÉES depuis leur HTML

```
client/src/shell.css        NEUF — la feuille de la page-shell, et d'elle seule
client/src/connexion.css    NEUF — la feuille de l'écran de connexion, et d'elle seule
```

**Elles vivent à côté de `client/src/style.css`**, qui est déjà « la feuille de
la FENÊTRE DE SESSION, et d'elle seule » (`style.css:1`) : trois surfaces, trois
feuilles, une convention.

🔴 **Elles se LIENT depuis le HTML, elles ne s'importent JAMAIS.** C'est
l'arrangement B, mesuré par S1 (`style.css:39-52`) et **reconfirmé par ce plan**
(D5) : `@import` ré-inline, le lien laisse Vite partager. Chaque page lie, dans
cet ordre — **l'ordre porte la cascade** :

```html
<link rel="stylesheet" href="/src/design/socle.css" />        <!-- tokens + base -->
<link rel="stylesheet" href="/src/design/primitives.css" />   <!-- les quatre familles -->
<link rel="stylesheet" href="/src/shell.css" />               <!-- la surface -->
```

⚠️ **`index.html` NE reçoit PAS `primitives.css`**, et c'est le garde de la
tâche 2 qui l'empêche de la recevoir par inadvertance.

### 6.2 Le sélecteur de thème du PRODUIT — où il va, et sur quelles surfaces

**Ce que la spec dit** (§5.2, famille 3) : « Il n'a pas sa place dans la fenêtre
de session (une barre d'outils sur un jeu en plein écran est une régression) :
il **vit sur la page-shell et sur le futur hub**. La fenêtre de session
**suit**, elle ne choisit pas. »

**Décision, en trois points :**

1. **La page-shell : OUI** — la spec le prescrit nommément.
2. **La fenêtre de session : NON** — la spec l'interdit nommément. `index.html`
   n'est pas touché.
3. **L'écran de connexion : OUI, et c'est une EXTENSION RAISONNÉE de la spec,
   déclarée comme telle.** La spec ne le nomme pas parce qu'**il n'existait pas
   quand elle a été écrite** — son §2.5 le dit en toutes lettres : « 🔴 L'écran
   de connexion de P2 n'existe pas encore côté navigateur ». Or
   `client/src/shell-page.ts:36-41` **y redirige tout visiteur sans jeton** :
   c'est aujourd'hui **la première surface, et parfois la seule**, qu'un
   utilisateur non authentifié voie. Un défaut sombre qu'on ne peut pas changer
   avant de s'être connecté est exactement ce que la spec §11 range sous « le
   thème sombre par défaut surprend — réversible par un utilisateur en un clic
   **dès S3** ». **Coût : zéro** — même module, même trois boutons.

**Le module** : `client/src/design/selecteur-theme.ts` **est PROMU au produit**,
il n'est pas dupliqué. Trois conséquences, toutes obligatoires :

- **il gagne des tests** — le statut « code d'instrument sans test »
  (`selecteur-theme.ts:5-9`) cesse d'être vrai ;
- **ses dépendances sont INJECTÉES**, faute de tout environnement DOM (D10) :
  ni `document` ni `window` ne sont lus dans le chemin testé, sur le patron de
  `theme.ts` et de `fullscreen.ts` ;
- **le défaut d'`aria-pressed` sur `storage` est CORRIGÉ** (D9), et sa rouge est
  jouée sur le code d'aujourd'hui.

⚠️ **Les deux galeries emploient le même module** (`galerie.ts:161`,
`galerie-primitives.ts:18`) : la tâche 6 les adapte. **Il n'y a pas de seconde
copie** — c'est la règle que §4.1 de la spec applique aux valeurs, et il n'y a
pas de raison de l'accorder aux modules.

### 6.3 La convention de nommage des classes — celle de S2, prolongée

S2 a posé (plan S2, l. 600-626) :

```
.bouton     .bouton--principal  .bouton--secondaire  .bouton--discret
.champ      .champ--erreur      .champ__etiquette  .champ__saisie  .champ__aide  .champ__erreur
.carte      .carte__titre       .carte__corps
.separateur
.message    .message--succes    .message--alerte   .message--danger
```

**S3 nomme ses classes de surface sur le même patron, en français, BEM, et
préfixées par leur surface** — par exemple `.bureau`, `.bureau__titre`,
`.connexion__carte`. **Aucun sélecteur d'élément nu dans les feuilles de
surface non plus** : c'est ce qui garantit que `shell.css` ne peut pas
atteindre un élément d'`index.html` le jour où quelqu'un lierait la mauvaise
feuille.

🔴 **RÈGLE DE PLACEMENT, pour éviter la divergence entre les deux feuilles :
toute règle écrite à l'IDENTIQUE dans `shell.css` et dans `connexion.css` monte
dans `client/src/design/primitives/surface.css`**, auprès de `.carte` et de
`.separateur`, qui est déjà la famille de la mise en surface. **Elle n'y crée
PAS une cinquième famille** — `primitives.css` garde ses quatre `@import`, donc
**G5 et les trois G6 ne bougent pas**.

### 6.4 🔴 Le balisage des cartes vit dans le HTML, pas dans le TypeScript

`client/src/shell-page.ts:123-136` construit aujourd'hui ses `<li>` par
`createElement` et `textContent`. **S3 pose à la place un `<template>` dans
`shell.html`**, que `shell-page.ts` clone.

Trois raisons, dont la première décide :

1. **Toutes les classes restent dans le HTML**, donc dans le périmètre le plus
   simple du contrôle §7.9 — celui qui ne dépend d'aucune analyse de
   TypeScript ;
2. le balisage d'une carte se lit à l'endroit où l'on regarde une page ;
3. `shell-page.ts` reste ce que son en-tête dit qu'il est — « **Aucune règle
   ici** — elles sont dans `shell.ts`, qui est testé » (`shell-page.ts:1-2`).

⚠️ **Le contrôle §7.9 balaie quand même les littéraux de `classList.add('…')` et
`className = '…'` dans `client/src/**/*.ts`**, parce que `galerie.ts` en écrit
six aujourd'hui (`galerie.ts:66,68,71,74,83,124`) et qu'un futur auteur en
écrira. **Ce n'est pas parce que la convention dit « dans le HTML » que le
contrôle a le droit de ne regarder que là.**

### 6.5 🔴 L'ACCESSIBILITÉ : toute paire de couleurs neuve entre dans §7.1

**La règle, sans exception** : si S3 pose une encre sur un fond qui n'est pas
déjà l'une des paires déclarées de `client/src/design/contraste.ts:76-90`
(7 encres × 3 fonds au seuil 4,5 ; `--bord-fort` × 3 fonds au seuil 3 ;
`--sur-accent` sur `--accent` et sur `--accent-survol`), **la paire est ajoutée
à `PAIRES` dans le MÊME commit**, et le compte 52 change avec elle.

🔴 **La conséquence de conception, et elle est décidée ici : la pastille d'état
d'une fenêtre (ouverte / fermée) se dit par l'ENCRE, jamais par un fond
coloré.** Un `.pastille--ouverte { background: var(--succes) }` créerait la
paire `X sur --succes`, qui n'existe pas dans les 52. **C'est exactement la
décision que S2 a prise pour ses quatre tons de message** — « quatre tons
distingués par la seule encre » — et la reprendre coûte zéro paire neuve et
zéro token neuf.

⚠️ **Ce qui reste hors de portée de §7.1, et ne le sera pas davantage après
S3** : que le bon token ait été employé au bon endroit (`--bord` contre
`--bord-fort`) reste **une règle de revue**, la spec §8 la nomme, et
`contraste.ts:106-108` la répète.

---

## 7. Global Constraints

- **Plafond de 500 lignes** (`CLAUDE.md`, §« Conventions de code »), **et porte
  d'action à 300** pour tout fichier de ⑥ (spec §10), **240 pour
  `primitives.css`**. Relevé complet au §1.3.
- ⚠️ **`client/verify-webrtc.mjs` (494, marge 6) n'est touché par AUCUNE tâche
  de ce plan.** La leçon de P2 tient en une phrase : « une addition de
  commentaire peut annuler une extraction ».
- 🔴 **CLAUSE CONDITIONNELLE SUR `primitives.test.ts` (283, marge 17) : toute
  tâche qui y ajoute une assertion EXTRAIT D'ABORD**, dans le même commit,
  **avant** d'écrire l'addition — les gardes de forme (G1 à G5, G7) d'un côté,
  les trois gardes de famille (G6) de l'autre, comme S2 l'a nommé.
  ⚠️ **En le faisant : G5 compare la liste `FAMILLES` du test aux `@import` de
  `primitives.css`**, et c'est ce qui empêche une cinquième famille d'échapper
  à G1-G4. **La scission ne doit pas les séparer de leur source.**
  ⚠️ **Ce plan ne prévoit AUCUNE addition à ce fichier** : les gardes neufs de
  S3 vivent dans des fichiers neufs. La clause est armée pour le cas où
  l'exécution en trouverait une nécessaire.
- **Jamais `git add -A`.** L'arbre est partagé ; **chaque commit porte sa
  pathspec explicite**, et **jamais `git commit --amend`**.
- 🔴 **`git checkout` NE RESTAURE PAS un fichier non suivi.** Deux mutations y
  ont survécu dans ce dépôt en une seule journée, dont un `setTimeout(2000)`
  laissé dans une route de production. **Toute tâche COMMITE AVANT DE MUTER**,
  et vérifie la remise en état par `git status --porcelain <chemin>` **vide** —
  y compris pour les fichiers neufs qu'elle vient de créer.
- ⛔ **Aucune tâche de ce plan n'emploie la VM Windows.** ⑥ est un sous-projet
  **navigateur** et la spec §9 déclare qu'il n'a **aucune recette sur VM**.
  ⚠️ **Un chantier concurrent y conduit la recette du pont de fichiers** : ne
  pas s'en approcher.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** son implémentation et **vu échouer**, et le message
  d'échec attendu est écrit dans la tâche.
  🔴 **Et une rouge ne vaut que pour l'assertion qu'elle fait tomber** : `expect`
  interrompt le test à la première. **Tout garde à plusieurs assertions se joue
  en autant de rouges qu'il a d'assertions.**
  🔴 **Une perturbation qui ne perturbe rien se lit exactement comme un contrôle
  qui ne mord pas.** S1 l'a payé deux fois, S2 une : un garde qui cherche une
  sous-chaîne **RETIRE D'ABORD LES COMMENTAIRES**, et **une mutation vérifie
  qu'elle a bien muté** — la mesure de son ancre, pas sa supposition.
  ⚠️ **Ce plan est une source de contrôles vacueux comme les précédents** : le
  plan de S1 a annoncé neuf occurrences pour onze **en attribuant l'écart à une
  cause fausse** ; celui de S2 a prescrit une rouge **impossible** (§7.4, selon
  le bloc choisi) et s'est contredit entre deux de ses propres tableaux.
  **Le doute porte sur ce document.**
- 🔴 **Il n'y a délibérément PAS de `client/vitest.config.ts`.** `test: { css:
  true }` est posé dans `client/vite.config.ts:93-111` ; un `vitest.config.ts`
  **prendrait le pas sur lui sans rien dire**, et un `import css from
  './x.css?raw'` rendrait alors la chaîne **vide** — un garde qui parserait ce
  texte **passerait au vert en ne mesurant rien**. **NE PAS LE DÉPLACER.**
- **Aucune dépendance neuve**, ni de production ni de développement. **Aucun
  environnement DOM** (D10).
- **Aucun taux ne sera revendiqué.** Les contrôles de ⑥ sont **déterministes**
  (spec §9) : deux exécutions y établissent la **reproductibilité**, jamais une
  fréquence, et la question « combien de fois sur combien » **ne doit pas être
  empruntée** à une campagne qui, elle, l'aurait posée.
- **Les suites existantes restent vertes, et le compte est nommé d'avance** :
  **219** tests `client` (24 fichiers) et **111** tests `proto` (5 fichiers) à
  `920a1eb`. **Chaque tâche qui ajoute des tests annonce le compte attendu AVANT
  de le mesurer** — c'est ainsi que D10 a rattrapé un test écrasé par un
  `Write`.
- 🔴 **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`** : la racine
  Vitest est `client/`.
- **L'ancien code est hors périmètre** : `assets/`, `web/`, `src/`, `index.js`
  ne sont ni lus ni modifiés (spec §4.6).
- ⛔ **`client/index.html` et `client/src/style.css` NE SONT PAS MODIFIÉS**, à
  aucune tâche. C'est la fenêtre de session, et **seul S4 a le droit d'y
  toucher**. Le critère ③ de la recette le mesure.
- **Aucune variable d'environnement n'est introduite ni lue.** *L'absence est
  déclarée parce qu'une absence se déclare.*

---

## 8. Structure des fichiers

```
client/
  shell.html                               MOD   — liens, classes, <template>, sélecteur (T4, T6)
  connexion.html                           MOD   — liens, classes, carte, sélecteur (T5, T6)
  index.html                               ⛔ INTOUCHÉ
  src/
    shell.css                              NEUF  — la feuille de la page-shell (T4)
    connexion.css                          NEUF  — la feuille de l'écran de connexion (T5)
    style.css                              ⛔ INTOUCHÉ
    shell.ts                               MOD   — le TON du bandeau (T3)
    shell.test.ts                          MOD   — le ton, et la fin du double NO-OP (T3)
    shell-page.ts                          MOD   — clone du <template>, ton relayé (T4)
    connexion.ts                           MOD   — le ton du message (T5)
    design/
      tokens.ts                            MOD   — ecartsEntreBlocs : racine ⊆ clair (T1)
      tokens.test.ts                       MOD   — les tests de T1
      classes.ts                           NEUF  — la logique PURE du contrôle §7.9 (T2)
      classes.test.ts                      NEUF  — ses tests (T2)
      selecteur-theme.ts                   MOD   — promu au produit, deps injectées, aria-pressed (T6)
      selecteur-theme.test.ts              NEUF  — les tests que le legs n°3 réclame (T6)
      galerie.ts                           MOD   — s'adapte au module promu (T6)
      galerie-primitives.ts                MOD   — idem (T6)
      primitives/surface.css               MOD   — SI et seulement si une règle est identique aux deux feuilles (T5)
  outils/
    blocs-de-theme.mjs                     MOD   — la sortie dit la nouvelle portée (T1)
    classes-employees.mjs                  NEUF  — le contrôle §7.9 (T2)
    verifier-design.mjs                    MOD   — 6 → 7 contrôles (T2)
    tokens-orphelins.mjs                   MOD   — l'assertion de sous-bloc clos (T7)
    tokens-orphelins/attente.mjs           MOD   — la liste rétrécit (T4, T5), sa forme change (T7)
docs/superpowers/plans/
  2026-08-19-design-system-s3-resultats.md            NEUF (T8)
  journaux-design-s3/                                 NEUF (T8)
CLAUDE.md                                             MOD  (T9)
```

⚠️ **`client/vite.config.ts` N'EST PAS MODIFIÉ** : S3 n'ajoute aucune entrée
Vite. Les feuilles de surface entrent seules dans les périmètres de §7.2 et de
§7.6 (D4, mesuré).

---

# Les tâches

## Famille ⓪ — les contrôles, AVANT les surfaces qu'ils gardent

### Task 1 — Fermer l'angle mort de §7.4 : `racine ⊆ clair`, restreint aux couleurs

**Objet :** faire en sorte qu'un token de COULEUR déclaré à la racine et oublié
dans les **deux** blocs clairs ne soit plus invisible au contrôle §7.4.

**Files:**
- Modify: `client/src/design/tokens.ts` (`ecartsEntreBlocs`, l. 147-176)
- Modify: `client/src/design/tokens.test.ts`
- Modify: `client/outils/blocs-de-theme.mjs` (l'en-tête et la ligne de sortie
  disent la portée NOUVELLE)

**Ce qui est ajouté à `ecartsEntreBlocs` :** une troisième comparaison — **tout
token de la racine dont la VALEUR est une couleur, et qui n'est pas dans la
liste nommée des hors-thème, doit être déclaré dans les deux blocs clairs**.

- **« est une couleur »** se décide sur la valeur telle que `lireBlocsDeTheme`
  la rend : elle commence par `#`, par `rgb(`/`rgba(` ou par `hsl(`/`hsla(`.
  ⚠️ **Ne PAS décider sur le nom** : un préfixe est une convention qu'une faute
  de frappe contourne.
- **La liste des hors-thème est NOMMÉE, jamais dérivée d'un préfixe**, et elle
  porte sa raison auprès d'elle : `--video-letterbox`, `--voile-flottant`,
  `--voile-bouton`, `--voile-bouton-survol`, `--voile-micro-actif`,
  `--voile-micro-refuse` — les six que `tokens.css:80-102` déclare « HORS
  THÈME — déclarés une fois, jamais redéfinis ».

**Le relevé qui fonde la forme, à recopier dans le commentaire du code** (pris
le 20 août 2026 par `lireBlocsDeTheme` sur l'arbre intact) : racine **48**
tokens dont **20** couleurs ; blocs clairs **14** ; **20 − 6 = 14**, donc
**zéro écart dès aujourd'hui**.

**Tests, dans `tokens.test.ts` :**

1. **`ecartsEntreBlocs` rend un écart quand une couleur de la racine manque aux
   DEUX blocs clairs.** ⚠️ **Le vecteur doit être une couleur INTERMÉDIAIRE ou
   au moins non triviale**, sur un `tokens.css` construit dans le test (pas le
   vrai fichier), pour que le test dise ce qu'il dit.
2. **Un token hors thème de la liste ne produit AUCUN écart** — c'est ce qui
   rend le contrôle vivable.
3. **Un token NON-couleur de la racine (`--e-3`, `--police-ui`) ne produit aucun
   écart** — sans cette assertion, la fermeture rendrait 34 écarts sur l'arbre
   intact et serait rejetée en bloc.
4. **Une couleur NEUVE ajoutée à la racine, ni listée hors thème ni déclarée
   dans les blocs clairs, produit un écart** — c'est le cas que la fermeture
   existe pour attraper.
5. **Le contrôle rend `0 écart` sur le vrai `tokens.css`** (via `?raw`, qui
   dépend de `css: true`).

**La rouge, et son atteignabilité est ACQUISE :** la mutation exacte est déjà
jouée et versée par S2 —
`docs/superpowers/plans/journaux-design-s2/trou-7-4.log`, `--accent-survol`
retiré des **deux** blocs clairs :

```
  bloc racine : 48 token(s)
  bloc media-clair : 13 token(s)
  bloc attribut-clair : 13 token(s)
écarts : 0
exit=0
```

**Après cette tâche, la même mutation doit rendre `exit=1` et nommer
`--accent-survol` comme absent des deux blocs clairs.** ⚠️ **Rejouer la mutation
sur le vrai fichier, commit fait d'abord, arbre restauré ensuite, restauration
vérifiée par `git status --porcelain client/src/design/tokens.css` vide.**

**Ce qui rendrait ce test VACUEUX, et qu'il faut vérifier :** un prédicat « est
une couleur » qui rendrait `false` pour tout ne ferait jamais d'écart. **Le
test 4 est celui qui l'attrape** — il doit être vu rouge sur une implémentation
qui ne compare pas encore.

**Tokens sortis de la liste d'attente : AUCUN.**

**Ce que cette tâche NE fait PAS :** elle ne touche pas `lireBlocsDeTheme`, donc
**le legs n°9 de S2 — « `tokens.ts` ne sait nommer que trois blocs » — reste
entier**, et il faut le dire dans le commit.

**Ce qui change de PORTÉE, et doit être écrit à trois endroits :** l'énoncé du
contrôle §7.4 n'est plus « les trois blocs déclarent le même ensemble de noms »
mais « les deux blocs clairs sont identiques, et **toute couleur de la racine y
est redéclarée sauf les hors-thème nommés** ». Les trois endroits : l'en-tête de
`blocs-de-theme.mjs`, le document de résultats (T8), `CLAUDE.md` (T9).

---

### Task 2 — Le contrôle §7.9 : aucune classe employée n'est indéclarée, et une primitive atteint le produit

**Objet :** donner à ⑥ le contrôle qui manque — celui qui voit qu'une surface du
produit **emploie** réellement une primitive, ce dont §7.6 est structurellement
incapable (D8) — et attraper au passage la faute de frappe de classe, qu'aucun
contrôle ne voit aujourd'hui.

**Files:**
- Create: `client/src/design/classes.ts` — la logique **PURE** (parse, ensembles)
- Create: `client/src/design/classes.test.ts`
- Create: `client/outils/classes-employees.mjs` — le script, qui ne porte
  **aucune** règle de parsing
- Modify: `client/outils/verifier-design.mjs` — `CONTROLES` passe de **six** à
  **sept** entrées

⚠️ **La séparation logique pure / script est la convention du dépôt** :
`tokens-orphelins.mjs:13-14` — « Il ne porte AUCUNE règle de parsing :
`tokensDeclares` et `tokensReferences` vivent dans `tokens.ts`, qui est
typechecké et testé. »

**Ce que le contrôle mesure — trois assertions, comptées SÉPARÉMENT :**

- **Ensemble DÉCLARÉ** = les classes des sélecteurs de `client/src/**/*.css`
  **∪** celles des blocs `<style>` en ligne des entrées Vite.
  ⚠️ **Les `<style>` en ligne sont OBLIGATOIRES dans cet ensemble** :
  `client/design.html` déclare six classes que `galerie.ts` emploie
  (`pastille`, `echantillon`, `nom`, `valeur`, `ligne`, `barre` —
  `galerie.ts:66,68,71,74,83,124`). **Les omettre ferait naître ce contrôle
  rouge sur du code correct**, c'est-à-dire la pression à l'assouplissement que
  ce dépôt écarte depuis D8.
- **Ensemble EMPLOYÉ** = les classes des attributs `class="…"` des entrées Vite
  **∪** les littéraux de `classList.add('…')` et de `className = '…'` dans
  `client/src/**/*.ts`.
- **① employé ⊆ déclaré** — un `class="bouton--principale"` (faute de frappe)
  est une règle qui ne s'applique à rien, et le navigateur ne dit rien.
  **ROUGE : la faute de frappe.**
- **② A — ATTEIGNABILITÉ : au moins une classe de PRIMITIVE est employée par une
  SURFACE DU PRODUIT** (`index.html`, `shell.html`, `connexion.html`).
  🔵 **ROUGE SUR L'ARBRE INTACT, sans rien casser — mesuré le 20 août 2026** :
  `grep -n 'class=' client/{index,shell,connexion}.html` rend **AUCUNE**.
  **C'est cette assertion, et elle seule, qui referme l'écart de S2.**
- **③ B — chacune des quatre familles apparaît dans `client/primitives.html`.**
  Verte aujourd'hui ; **sa rouge se joue par mutation** (retirer d'une des
  quatre familles toutes ses occurrences de la page). Elle prend une part du
  legs n°4 de S2 : « une galerie qui cesserait de rendre une famille entière ne
  serait attrapée par aucun contrôle ».

**Le script imprime TOUJOURS ses comptes**, succès compris — nombre de classes
déclarées, employées, et **le détail par famille et par surface**. Un contrôle
de dérive dont on ne lit jamais la valeur ne sert qu'à passer
(`poids-css.mjs:5-8`).

**Tests dans `classes.test.ts` :** au moins un par fonction pure —
`classesDeclarees` sur un CSS jouet (sélecteurs composés, pseudo-classes,
pseudo-éléments, `@media`), `classesEmployees` sur un HTML jouet (`class` à
plusieurs valeurs, guillemets simples et doubles) et sur un TS jouet
(`classList.add`, `className =`), et **un test qui prouve que les commentaires
sont retirés d'abord** — sans quoi un `/* .bouton--principale */` compterait.

**Les rouges à jouer, dans l'ordre :**

| # | Mutation | Message attendu | Sortie |
| --- | --- | --- | --- |
| A | `class="bouton bouton--principale"` dans `shell.html` (après T4) | `NON DÉCLARÉE  bouton--principale  employée par client/shell.html` | `exit=1` |
| B | **aucune** — l'arbre intact | `AUCUNE classe de primitive employée par une surface du produit` | `exit=1` |
| C | toutes les occurrences de `champ` retirées de `primitives.html` | `famille CHAMP absente de client/primitives.html` | `exit=1` |
| D | `classList.add('bouton--discrete')` dans `galerie.ts` | `NON DÉCLARÉE  bouton--discrete  employée par client/src/design/galerie.ts` | `exit=1` |

🔴 **La rouge B est jouée EN PREMIER, avant toute autre tâche de surface, et
elle est l'état de l'arbre à ce commit.** **La branche porte donc un
`design:verifier` ROUGE entre cette tâche et la tâche 4** — c'est ce que S1 a
fait entre ses tâches 1 et 9 pour §7.2 et §7.3, et c'est le prix d'un contrôle
dont l'atteignabilité ne coûte aucune mutation. **Le déclarer dans le message de
commit.**

⚠️ **CE QUE CE CONTRÔLE NE DIT PAS, et qui doit vivre dans son en-tête :**

- **le sens inverse — toute classe déclarée est employée — N'EST PAS PRIS.** Il
  exigerait une seconde liste d'attente (`.bouton--discret` peut n'avoir aucun
  appelant à la fin de S3), et une liste d'attente de plus pour une famille
  déjà gardée par sa galerie serait un coût sans contrepartie. **C'est
  `primitives.html` et l'œil qui tiennent ce sens-là**, et c'est son statut
  d'instrument de jugement humain (spec §7.8).
- **une classe CALCULÉE à l'exécution est invisible** — `el.className = variable`,
  une concaténation, un `classList.toggle(nom)`. **La règle qui rend le contrôle
  utile est de conserver la convention § 6.4** : les classes s'écrivent en
  littéral, dans le HTML de préférence.
- **il ne juge d'AUCUNE apparence** : qu'une classe soit posée ne dit pas
  qu'elle rende bien. C'est le jugement humain, et il reste entier.

**Tokens sortis de la liste d'attente : AUCUN.**

---

## Famille ① — les surfaces

### Task 3 — Le TON du bandeau : une règle pure, dans `shell.ts`, testée

**Objet :** permettre à la page-shell de dire par une couleur qu'un message est
un refus, sans mettre cette décision dans le câblage non testé.

**Files:**
- Modify: `client/src/shell.ts` (`OptionsBureau.afficher`,
  `afficherEtatFichiers`)
- Modify: `client/src/shell.test.ts`

**Ce qui change :** `afficher` et `afficherEtatFichiers` reçoivent, **en plus du
texte, un TON** — l'un des quatre de la famille `message` de S2 : `neutre`,
`succes`, `alerte`, `danger`. La table, qui est la règle :

| Appel | Ton | Pourquoi |
| --- | --- | --- |
| `fenetreOuverte` → pop-up bloquée (`shell.ts:55-58`) | **danger** | l'utilisateur doit agir : autoriser les pop-ups |
| `refus(titre, motif)` (`shell.ts:76-78`) | **danger** | la fenêtre n'existera pas |
| `lecteurMonte(nom)` (`shell.ts:104-106`) | **succes** | un état positif, et le seul du produit |
| `lecteurDemonte()` (`shell.ts:108-118`) | **neutre** | 🔴 **la chaîne VIDE reste vide** — voir ci-dessous |
| `lecteurEchoue(motif)` (`shell.ts:120-122`) | **danger** | le partage a raté |
| « bureau connecté » (`shell-page.ts:142`) | **neutre** | câblage, hors de `shell.ts` |

🔴 **`lecteurDemonte()` continue d'émettre la CHAÎNE VIDE**, et le ton ne doit
pas donner une couleur à un bandeau vide. `shell.ts:109-117` explique pourquoi
la chaîne est vide — « un état de lecteur qui ne s'efface pas ferait croire à un
dossier toujours partagé alors qu'il ne l'est plus, ce qui est pire qu'un texte
périmé : c'est une **affirmation fausse sur une permission** ». **Ce
commentaire ne bouge pas, et le comportement non plus.**

**Tests, dans `shell.test.ts` :**

1. **un refus rend le ton `danger`** ;
2. **une pop-up bloquée rend le ton `danger`** ;
3. **un montage réussi rend le ton `succes`, un échec le ton `danger`** — les
   deux sont distincts, comme `shell.ts:38-41` l'exige déjà pour le texte ;
4. **un démontage rend la chaîne vide**, et **le bandeau n'affiche alors aucun
   ton** (assertion séparée : la vacuité du texte ne doit pas être confondue
   avec la neutralité du ton).

🔴 **AVANT TOUT : le double `afficher: () => {}` de `shell.test.ts:15` est
remplacé par une COLLECTE.** C'est un no-op, exactement le patron que D10 a
nommé — « une source factice qui implémente un effet de bord en NO-OP rend une
famille entière de défauts invisible aux tests d'hôte », **456 tests verts sur
un produit muet**. Tant qu'il est en place, **aucun** des quatre tests
ci-dessus ne peut échouer.

**La rouge :** avec la collecte en place et la règle non écrite, le test 1
rend `expected 'neutre' to be 'danger'`. **Chaque assertion se joue
séparément** — `expect` interrompt au premier échec.

**Compte de tests annoncé AVANT mesure :** `shell.test.ts` porte **12** tests —
**relevé le 20 août 2026 par `npx vitest run src/shell.test.ts`**, pas
supposé — et passe à **12 + 4 = 16** ; le total `client` de **219** à **223**.
⚠️ **Relancer la mesure avant d'écrire** : les chantiers voisins font bouger ce
compte, et une addition de leur fait ferait croire à une addition du nôtre.

**Tokens sortis de la liste d'attente : AUCUN.**

---

### Task 4 — La page-shell habillée

**Objet :** faire de `shell.html` une page composée — titre, sections, grille de
cartes — portée par les primitives de S2 et une feuille de surface qui met en
service sept tokens en attente.

**Files:**
- Create: `client/src/shell.css`
- Modify: `client/shell.html`
- Modify: `client/src/shell-page.ts`
- Modify: `client/outils/tokens-orphelins/attente.mjs` — **dans le même commit**

**Ce que la page devient :**

- `shell.html` lie, dans l'ordre : `design/socle.css`, `design/primitives.css`,
  `shell.css` ;
- un `<h1 class="bureau__titre">` en `--t-2xl` ;
- deux **sections** séparées par `--e-6` : « Mes fichiers » (le bouton
  `#choisir-dossier` devenu `.bouton .bouton--secondaire`, et `#etat-fichiers`
  devenu un `.message`) et « Mes fenêtres » (la liste) ;
- 🔴 **un `<template>` porte le balisage d'une carte de fenêtre**, cloné par
  `shell-page.ts` (§6.4) : `.carte`, `.carte__titre`, `.carte__corps`, une
  **pastille d'état** et un `.bouton .bouton--discret` « Rouvrir » ;
- la liste `#fenetres` devient une **grille** à gouttière `--e-5` ;
- `#statut` devient un `.message` dont la classe de ton vient de la tâche 3.

🔴 **La pastille se dit par l'ENCRE, jamais par un fond coloré** (§6.5) : un
fond `--succes` créerait une paire de contraste que les 52 ne portent pas.
**Aucune paire n'est ajoutée par cette tâche, et §7.1 doit rester à 52.**

🔴 **Le commentaire de `shell.html:20-24` est RETIRÉ** — il annonce
littéralement sa propre fin : « **C'est S3 qui referme cet écart, pour toutes
les surfaces à la fois.** » ⚠️ **Il a été vérifié mot pour mot avant d'écrire ce
plan**, et il est exact.

⚠️ **Le reste de `shell.html:13-19` NE bouge PAS** : c'est l'invariant du pont
de fichiers — « le bouton vit ICI et nulle part ailleurs […] **AUCUNE FENÊTRE
D'APPLICATION N'EST SPÉCIALE** ». **Habiller n'est pas déplacer.**

⚠️ **`shell-page.ts` ne gagne AUCUNE règle** : il clone, il remplit, il relaie
le ton que `shell.ts` lui donne. Son en-tête (l. 1-2) reste vrai. **Si une
condition apparaît dans ce fichier, elle est au mauvais endroit** — c'est la
clause que `connexion.ts:5-9` écrit pour son jumeau.

**Tokens attendus, retirés de la liste DANS CE COMMIT :** `--t-2xl`, `--e-5`,
`--e-6`, `--e-7`, `--t-xs`, `--e-1`, `--r-plein` — **sept**.
🔴 **Lire la sortie du contrôle et retirer exactement ce qu'elle nomme.** Le
contrôle dit quoi retirer ; ce plan dit seulement à quoi s'attendre.

**Les contrôles, après cette tâche :**

| Contrôle | Attendu |
| --- | --- |
| §7.9 assertion ② A | **PASSE AU VERT** — c'est le commit qui referme l'écart de S2 |
| §7.6 | **10 → 3** orphelins = 3 en attente, `0 écart` |
| §7.1 | **52** paires, inchangé |
| §7.2 | `0` couleur littérale |
| §7.3 | 5 pages, deux assertions vertes |
| §7.7 | **monte** — relever le chiffre, ne pas le prédire |

**Les rouges à jouer :**

1. **retirer `--e-5` de la liste sans écrire son appelant** →
   `NOUVEL ORPHELIN --e-5` ;
2. **écrire l'appelant sans retirer l'entrée** →
   `À RETIRER DE LA LISTE --e-5 …` ;
3. **poser `class="bouton bouton--principale"`** → §7.9 rouge A.

---

### Task 5 — L'écran de connexion habillé

**Objet :** faire de `connexion.html` une carte centrée, et retirer le bloc de
commentaire qui annonce que S3 le retirera.

**Files:**
- Create: `client/src/connexion.css`
- Modify: `client/connexion.html`
- Modify: `client/src/connexion.ts`
- Modify: `client/src/design/primitives/surface.css` — **seulement si** une
  règle est écrite à l'identique dans les deux feuilles de surface (§6.3)
- Modify: `client/outils/tokens-orphelins/attente.mjs` — **dans le même commit**

**Ce que la page devient :**

- elle lie `design/socle.css`, `design/primitives.css`, `connexion.css` ;
- une **carte** `.carte` centrée dans la fenêtre, largeur bornée ;
- un `<h1>` en **`--t-3xl`** — **la valeur de la spec §4.4:367**, voir D1 ;
- les deux `<p>` deviennent des `.champ` avec `.champ__etiquette` et
  `.champ__saisie` ; le `<button>` devient `.bouton .bouton--principal` ;
- `#message` devient un `.message` dont le ton suit les trois branches déjà
  écrites dans `connexion.ts:38,53,71` — `neutre` pour « connexion… »,
  **`danger`** pour « refusé : … » et pour « plateforme injoignable ».

🔴 **AUCUNE RÈGLE NEUVE dans `connexion.ts`.** Les trois branches existent déjà ;
la tâche leur associe une classe, elle n'ajoute aucune condition. **C'est la
clause de l'en-tête du fichier** (l. 5-9) : « toute règle que ce fichier
porterait doit descendre dans `jeton.ts` ».
⚠️ **Et le message d'échec reste CELUI DU SERVICE, tel quel** (`connexion.ts:11-15`) :
il ne distingue pas « courriel inconnu » de « mot de passe faux », et
l'enrichir ici défairait la propriété anti-énumération de comptes **depuis le
seul endroit où personne ne penserait à la chercher**. **Habiller n'est pas
reformuler.**

🔴 **Le bloc `connexion.html:10-25` est RETIRÉ**, comme il le demande lui-même :
« la page ne porte AUCUNE primitive […] C'est **S3** qui les pose, et c'est lui
qui retirera ce bloc. »

**Deux annotations de la liste d'attente sont CORRIGÉES dans ce commit, avec
leur raison** (D1) : `--t-3xl` cesse de nommer « le titre du hub », que ⑥ ne
livre pas, et retrouve l'emploi que la spec §4.4:367 lui donne ; `--t-2xl`
retrouve « titre de page ». **La règle de revue d'`attente.mjs:59-60` l'exige :
toute annotation modifiée porte sa raison et le sous-bloc qui l'a modifiée.**

**Tokens attendus, retirés DANS CE COMMIT :** `--t-3xl`, `--lh-large` — **deux**.
⚠️ **`--lh-large` est le fragile** (§3.3) : s'il n'a pas de paragraphe assez long
sur cette page ni sur la page-shell, **il n'est PAS consommé**, son annotation
est re-étiquetée avec sa raison, **et la tâche 7 rendra ce re-étiquetage
visible**. **Ne rien fabriquer pour faire tomber un compte.**

**Les contrôles, après cette tâche :** §7.6 rend **1** orphelin
(`--police-mono`) si les neuf sont sortis, **ou** le compte réel, `0 écart`
dans les deux cas.

**Les rouges à jouer :** les deux sens de §7.6, comme en T4, et **une rouge de
§7.1 si et seulement si une paire a été ajoutée** — ce que ce plan n'attend pas.

---

### Task 6 — Le sélecteur de thème du PRODUIT

**Objet :** poser le sélecteur de thème sur les deux surfaces du produit, en
promouvant `selecteur-theme.ts` du statut d'instrument à celui de produit — donc
en lui donnant des tests et en corrigeant le défaut qu'il déclare.

**Files:**
- Modify: `client/src/design/selecteur-theme.ts`
- Create: `client/src/design/selecteur-theme.test.ts`
- Modify: `client/src/design/galerie.ts`, `client/src/design/galerie-primitives.ts`
- Modify: `client/shell.html`, `client/connexion.html`
- Modify: `client/src/shell.css`, `client/src/connexion.css` — la mise en place
  du groupe de boutons

**Trois choses, dans cet ordre :**

1. 🔴 **Les dépendances deviennent INJECTÉES.** `client/` n'a **ni jsdom ni
   happy-dom** (D10, mesuré) : ni `document`, ni `window` ne sont lus dans le
   chemin testé. Le patron est celui de `client/src/fullscreen.ts:17-20` et de
   `client/src/design/theme.ts:4-9`, et les doubles s'écrivent à la main comme
   `faireBouton()` de `fullscreen.test.ts:39`. ⚠️ **`theme.ts` n'est PAS
   modifié** : `Coffre`, `Racine` et les cinq fonctions restent ce qu'elles
   sont, et le contrôle §7.5 reste à **10 tests**.
2. 🔴 **Le défaut d'`aria-pressed` est CORRIGÉ.** `selecteur-theme.ts:59-64`
   déclare que l'écoute de `storage` **ne rappelle pas `marquer()`**, et que
   « le défaut est réel et il est PRÉEXISTANT ». Sur un instrument, c'est une
   gêne ; **sur le produit, c'est une interface qui ment sur son propre état**,
   et le cas d'une fenêtre voisine qui change le thème est le cas nominal du
   multi-fenêtres (spec §4.2). **Le commentaire qui déclare le défaut est
   remplacé par celui qui déclare la correction et nomme le sous-bloc.**
3. **Le sélecteur est posé sur `shell.html` et sur `connexion.html`** — et
   **jamais sur `index.html`** (spec §5.2 famille 3 : « la fenêtre de session
   suit, elle ne choisit pas »). La justification de l'écran de connexion est
   au §6.2 et **elle est une extension raisonnée de la spec, déclarée**.

**Tests, dans `selecteur-theme.test.ts` :**

1. **un clic sur « clair » écrit la clé ET pose `data-theme` localement** — les
   deux moitiés, séparément, comme §7.5 les sépare ;
2. **`aria-pressed` suit le clic** ;
3. 🔴 **`aria-pressed` suit un événement `storage` venu d'une autre fenêtre** —
   **c'est la rouge de la correction**, et elle tombe sur le code d'aujourd'hui ;
4. **un `storage` sur la clé voisine `guac.jeton.acces` ne change rien** —
   `theme.ts:99-102` dit que `connexion.ts:57` écrit réellement cette clé, donc
   ce n'est pas une précaution théorique ; **employer la VRAIE clé voisine**,
   jamais une clé inventée (divergence D5 de S1) ;
5. **le module n'écrit rien dans le coffre en réagissant à `storage`** — sinon
   deux fenêtres se renverraient l'événement sans terme (`theme.ts:30-35`).
   ⚠️ **Si cette propriété est garantie par la SIGNATURE** (la fonction ne
   reçoit pas de `Coffre` sur ce chemin), **ne PAS écrire le test** : ce dépôt
   ne garde pas de contrôle incapable d'échouer, et `theme.ts:30-35` prend
   exactement cette décision pour la même propriété.

**La rouge principale, jouée sur le code d'aujourd'hui** : le test 3 rend
`expected 'false' to be 'true'` — l'attribut garde la valeur du thème d'avant.

**Compte de tests annoncé AVANT mesure :** `client` passe de **223** (après T3)
à **223 + 4 ou 5**. **L'annoncer, puis le mesurer.**

⚠️ **Les deux galeries changent d'appel**, et ce sont des instruments : **leur
apparence peut bouger, celle du produit ne le doit pas**. `galerie.ts:161`
passe par `vide('themes')` avant d'installer — **si le balisage migre vers le
HTML, `vide()` effacerait les boutons.** C'est le piège de cette tâche ; le
vérifier en ouvrant les deux galeries bâties.

**Tokens sortis de la liste d'attente : AUCUN attendu.** ⚠️ Si le groupe de
boutons consomme un token en attente, **le retirer dans ce commit** — le
contrôle le nommera.

---

### Task 7 — La mitigation du re-étiquetage, et la liste d'attente finale

**Objet :** rendre visible ce qu'aucun contrôle ne voit — qu'une entrée de la
liste d'attente change de sous-bloc — au moins pour le cas où elle nomme un
sous-bloc **déjà clos**.

**Files:**
- Modify: `client/outils/tokens-orphelins/attente.mjs`
- Modify: `client/outils/tokens-orphelins.mjs`

**Ce qui change de forme :** chaque entrée de `EN_ATTENTE_D_APPELANT` porte
désormais **le sous-bloc dans un champ structuré**, et non plus noyé dans une
phrase — par exemple `{ sousBloc: 'S4', raison: '#stats, OU RETRAIT : …' }`.
Le **même fichier** déclare `SOUS_BLOCS_CLOS` — **`S1`, `S2`, `S3`** à la fin de
ce sous-bloc — et sa clause de tenue est celle de la liste elle-même
(`attente.mjs:35`) : « **ce nombre est tenu à jour par la tâche qui le rend
faux**, jamais par une tâche de ménage plus tard ».

**L'assertion, dans `tokens-orphelins.mjs` :** aucune entrée ne nomme un
sous-bloc de `SOUS_BLOCS_CLOS`. Message :
`SOUS-BLOC CLOS  --x  nommait S2, qui est clos : décider ou re-étiqueter avec sa raison`.

**La rouge, et son atteignabilité :** annoter une entrée `sousBloc: 'S2'` →
`exit=1`. ⚠️ **Elle serait passée au rouge à la fin de S2 sur les trois entrées
annotées « S2 »**, et c'est exactement ce que `attente.mjs:72-75` affirme.
**Jouer la mutation, commit fait d'abord, arbre restauré ensuite.**

🔴 **CE QU'ELLE NE COUVRE PAS, et le mot « partielle » est pesé** — à écrire dans
le fichier :

- **elle juge le SOUS-BLOC NOMMÉ, jamais le CONTENU** : « S4 — la gouttière
  entre cartes » changé en « S4 — n'importe quoi » lui échappe ;
- **elle dépend d'une liste de sous-blocs clos tenue à la main** : un sous-bloc
  qui ne s'y déclare pas la neutralise. **C'est une règle de revue de plus, et
  elle est déclarée** ;
- **après S3 elle garde UNE entrée** — `--police-mono`. **C'est une objection, et
  la réponse est au §4.4** : c'est précisément l'entrée dont la prose dit
  qu'aucun sous-bloc n'a le droit de la laisser en place sans décider, et une
  mitigation construite après la faute qu'elle devait empêcher n'aurait plus
  rien à empêcher.

⚠️ **Cette tâche NE retire aucune entrée et n'en re-étiquette aucune** : elle
change la forme, pas le contenu. **Les retraits appartiennent aux tâches 4 et
5**, dans les commits qui écrivent les appelants — c'est l'invariant d'égalité
de §7.6.

⚠️ **Taille** : `attente.mjs` vaut **146** lignes pour une porte à 300. S2 a
payé **deux fois** dans ce fichier la leçon « une addition de commentaire annule
une extraction ». **Si l'addition de doctrine porte le fichier au-delà de 240,
extraire — jamais compresser**, et la doctrine part **avec** sa donnée.

**Tokens sortis de la liste d'attente : AUCUN.**

---

## Famille ② — la recette et la revue

### Task 8 — Recette S3 : cinq critères, deux exécutions chacun, pièces versées dans git

**Objet :** mesurer S3, et verser ses pièces dans git plutôt que dans un espace
de travail que le dépôt ne suit pas.

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-design-system-s3-resultats.md`
- Create: `docs/superpowers/plans/journaux-design-s3/`

🔴 **LA PREUVE D'UNE AFFIRMATION NE DOIT JAMAIS VIVRE DANS UN RAPPORT
GITIGNORÉ.** D10 a établi par la commande que l'espace de travail de D9
(`.superpowers/sdd/`) **a disparu**, emportant **six** constats de revue
définitivement perdus. **Tout ce que cette recette relève est versé sous
`journaux-design-s3/`, et le document de résultats porte l'analyse.**

**Les cinq critères, chacun DEUX exécutions :**

| # | Critère | Ce qui est relevé |
| --- | --- | --- |
| ① | **Les huit contrôles sont verts** | `npm run design:verifier` → `7/7 contrôle(s) vert(s)`, `exit=0` ; plus §7.5 dans `npm test`. **§7.1 doit rendre 52 paires et minimum 3,16** — s'il a bougé, une paire a été ajoutée et il faut le dire |
| ② | **La liste d'attente a RÉTRÉCI, et chaque sortie a son appelant écrit** | `tokens-orphelins.mjs` : le compte final, `0 écart`, et **le diff de `attente.mjs` commit par commit** (`git show <commit> -- client/outils/tokens-orphelins/attente.mjs`) |
| ③ | 🔴 **La fenêtre de session N'A PAS BOUGÉ** | (a) `git diff <base>..HEAD -- client/index.html client/src/style.css` **VIDE** ; (b) `dist/index.html` lie **exactement** `socle-*.css` et `main-*.css`, et **aucune autre** ; (c) les deux actifs portent les **mêmes hachages de contenu** qu'à la base — **relevés des deux côtés, jamais supposés** |
| ④ | **Les primitives atteignent le produit** | §7.9 : le **détail par famille et par surface**. ⚠️ **C'est un RELEVÉ, pas une assertion** — le contrôle n'exige qu'une famille ; le document dit lesquelles sont réellement employées |
| ⑤ | **Le poids CSS reste sous le plafond** | `poids-css.mjs` : la somme, le plafond, la marge, **et le delta contre les 6 374 octets de S2** |

⚠️ **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les
contrôles de ⑥ sont **déterministes** (spec §9) : la question « combien de fois
sur combien » **ne se pose pas ici et ne doit pas être empruntée** à une
campagne qui, elle, l'aurait posée.

**Aussi relevé, et versé :**

- `npm test` (`client` et `proto`), `npm run typecheck` des deux côtés ;
- **`scripts/verify-all.sh` en entier**, et **le compte d'en-têtes `==>`
  RELEVÉ** — il devient **dix-huit** avec le huitième contrôle. **Le mesurer,
  jamais le sommer** (D5 de S2) ;
- **toutes les rouges rejouées**, verbatim, une mutation à la fois, arbre
  restauré après chacune, **restauration vérifiée par `git status --porcelain`
  vide** ;
- **les familles de lecture des journaux, MESURÉES** : `grep -lP '\x1b\['`,
  `file`, `grep -lc $'\r'` — pas supposées ;
- **`git status --porcelain` et `HEAD`** dans `commit.txt`, avec l'état des
  chantiers voisins.

⚠️ **Un `chpwd` du shell de l'hôte injecte un `ls` en tête de chaque journal**
dès qu'un `cd` court dans un sous-shell : **`unset -f chpwd` avant toute
collecte**. S2 a dû reprendre sa première passe pour cela, et **un journal
pollué par l'instrument est une pièce fausse**.

⚠️ **Le jugement visuel n'est PAS un critère**, et s'il n'est pas porté il est
**déclaré non porté** — jamais remplacé par un « probablement », jamais par une
capture d'écran que personne n'a regardée. S2 ne l'a pas porté et l'a dit.

---

### Task 9 — Revue transverse de fin de branche, et `CLAUDE.md`

**Objet :** trouver les affirmations devenues fausses **dans la branche S3
elle-même**, et inscrire S3 dans `CLAUDE.md`.

**Files:**
- Modify: tout fichier portant une affirmation devenue fausse
- Modify: `CLAUDE.md`

🔴 **CETTE TÂCHE EST OBLIGATOIRE, ET LE BARÈME DIT POURQUOI.** Ce dépôt a
trouvé, par cette seule revue : **D7 cinq** défauts, **D8 trois**, **D9 six**,
**D10 douze**, **D11 sept**, **P1 huit**, **P2 dix**, **S1 cinq**, **le chantier
E neuf**, **P3 douze**, **S2 douze**. **Toutes ont la même forme : correctes des
deux côtés prises séparément**, et **aucune revue par tâche ne peut
structurellement les voir**.

**La cible propre à S3, nommée d'avance :**

1. **Les affirmations écrites par une tâche et réfutées par une autre de la
   MÊME branche.** Candidates connues : le commentaire de `shell.html:20-24`
   (« C'est S3 qui referme cet écart ») que la tâche 4 retire ; le bloc de
   `connexion.html:10-25` que la tâche 5 retire ; les en-têtes de
   `selecteur-theme.ts` (« ce module n'est pas du produit », « il n'a pas de
   test », « il ne rappelle pas `marquer()` ») que la tâche 6 rend tous faux.
   **Les trois derniers sont dans le MÊME fichier et doivent tomber ensemble.**
2. **Les chiffres.** `52` paires, `48` tokens, `10` orphelins, `6 374` octets,
   `dix-sept` en-têtes, les comptes de tests. **`grep -n '<le nombre>' <fichier>`
   ÉNUMÈRE LES PLACES AVANT D'ÉCRIRE, et la substitution est RELUE PLACE PAR
   PLACE APRÈS.** « Corrigé à sa place » est une affirmation de **complétude**,
   et le naufrage du « 487 » s'est rejoué **sept fois** dans ce dépôt, dont une
   fois à l'intérieur de la ronde qui le dénonçait.
3. **La portée élargie de §7.4** (tâche 1) : partout où le dépôt écrit « les
   trois blocs déclarent le même ensemble de noms ».
4. **Le compte de contrôles**, six → sept dans l'agrégateur, sept → huit dans
   la spec §7 telle que ce plan l'étend.
5. **Toute clause qui dit « aucune surface du produit n'emploie une
   primitive »**, dans `primitives.css`, dans `attente.mjs`, dans `CLAUDE.md`.

⚠️ **LA REVUE TRANSVERSE EST ELLE-MÊME UNE SOURCE DE CROISSANCE** : celle de S2
a ajouté ~48 lignes de commentaire et fait tomber la marge de
`primitives.test.ts` de **30 à 17**. **Relever les tailles APRÈS ses propres
éditions**, jamais avant — « une table relevée en début de ronde serait fausse à
la fin de la même ronde ».

**`CLAUDE.md` reçoit une section « Sous-projet ⑥ Design system — sous-bloc S3 »**
sur le modèle de celles de S1 et de S2, avec :

- les **cinq critères** et leur nombre d'exécutions ;
- **les changements d'apparence assumés** (§5.1) et les reprises à l'identique
  (§5.2) ;
- **la portée élargie de §7.4** et **le contrôle neuf §7.9**, avec sa rouge sur
  l'arbre intact ;
- **la liste d'attente finale**, et la mitigation ;
- **les jugements humains**, leur compte cumulé ;
- **les tailles RELEVÉES PAR LA COMMANDE**, après les dernières éditions :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

  ⚠️ **Et le tableau des fichiers de `client/`**, remesuré, **pas recopié de ce
  plan** — les quatre chiffres périmés de D13 montrent ce que coûte la recopie ;
- **ce que S3 n'établit PAS**, et **ce que S3 lègue**.

---

## 9. Ordre et dépendances

```
T1 (§7.4 fermé)  ─┐
T2 (§7.9 neuf)   ─┼──►  T4 (page-shell)  ──►  T5 (connexion)  ──►  T6 (sélecteur)  ──►  T7 (mitigation)
T3 (ton, shell.ts)┘                                                                            │
                                                                                               ▼
                                                                                     T8 (recette)  ──►  T9 (revue + CLAUDE.md)
```

**Parallélisables : T1, T2 et T3.** Elles ne partagent aucun fichier — `tokens.ts`
et `blocs-de-theme.mjs` pour T1 ; `classes.ts`, `classes-employees.mjs` et
`verifier-design.mjs` pour T2 ; `shell.ts` et `shell.test.ts` pour T3.
⚠️ **`git add` NOMINATIF obligatoire** si elles courent ensemble : la consigne
va à **tous** les agents actifs, pas au dernier dispatché.

**Strictement séquentielles : T4 → T5 → T7.** Les trois écrivent
`client/outils/tokens-orphelins/attente.mjs`, et §7.6 exige l'égalité à chaque
commit : deux tâches concurrentes sur cette liste **se rendraient rouges l'une
l'autre**.

**T6 après T4 et T5** : elle modifie `shell.html`, `connexion.html`,
`shell.css` et `connexion.css`, que T4 et T5 créent.

🔴 **La branche porte un `design:verifier` ROUGE entre T2 et T4** — l'assertion
② A de §7.9 est rouge sur l'arbre intact, et c'est sa preuve d'atteignabilité.
**C'est ce que S1 a fait entre ses tâches 1 et 9. Le déclarer dans le message de
commit de T2, et le vérifier vert à T4.**

---

## 10. Les jugements humains que S3 AJOUTE

La spec §8 en compte **huit**, S2 en a ajouté **trois** — total **onze** à
l'entrée de S3. **S3 en ajoute QUATRE, et aucun ne deviendra une mesure :**

| # | Ce qui n'est pas mesuré | Ce qui est mesuré à la place |
| --- | --- | --- |
| 12 | que la **grille de cartes** soit la bonne forme pour une liste de fenêtres, plutôt qu'une liste dense ou un tableau | rien. C'est une décision de mise en page |
| 13 | que la **carte de connexion centrée** soit à la bonne largeur | rien — seule sa gouttière et ses marges viennent de l'échelle |
| 14 | que l'état **ouverte / fermée** dit par la seule **encre** d'une pastille se distingue assez | **son contraste** : l'encre employée est l'une des sept, mesurée sur les trois fonds (§7.1). ⚠️ **Distinguer deux états n'est pas la même chose que lire un texte**, et WCAG ne le mesure pas ici |
| 15 | que **trois boutons côte à côte** soient la bonne forme de sélecteur de thème, plutôt qu'un `<select>` ou un interrupteur | rien |

⚠️ **`--lh-large` sur les messages longs n'est PAS compté ici** : ou il est
employé, et c'est un cran de l'échelle appliqué à un paragraphe long — ce que
l'échelle prévoit —, ou il ne l'est pas et il reste en attente. **Il n'y a pas
de jugement à porter sur un token qu'on emploie pour ce qu'il est.**

🔴 **Total à la fin de S3 : QUINZE jugements humains**, et il faut les
compter dans le document de résultats. Ils rejoignent la liste déjà longue du
dépôt — `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
`TAILLE_MAX_SORTIE`, les paramètres `scrypt` de P2.

---

## 11. Ce que S3 laisse à S4, et ce qu'il ne livre pas

**À S4 :**

1. ⛔ **`--police-mono`** — la seule entrée dont le sort soit encore ouvert : ou
   S4 le câble sur `#stats`, ou il le retire. **S3 ne l'a pas rouvert**, et la
   mitigation de T7 l'empêchera d'être re-étiqueté en silence.
2. ⛔ **Les trois longueurs hors échelle de `style.css`** (`padding: 6px`,
   `font-size: 18px`, `letter-spacing: 0.02em`), et la clause « aucune longueur
   hors échelle » du §8 qu'elles **rendent fausse jusqu'à S4**.
3. ⛔ **L'écran plein cadre des états terminaux** et le **Window Controls
   Overlay** (spec §6, S4). ⚠️ **WCO dépend du manifest de ②**, que ⑥ ne livre
   pas.
4. ⛔ **La fenêtre de session tout entière** — S3 ne l'a pas touchée, et le
   critère ③ de la recette le mesure.

**Ce que S3 ne livre pas, et qui n'a pas de sous-bloc :**

5. ⛔ **Le hub** — il n'existe pas, son contenu dépend de ④, et **⑥ ne le livre
   pas** (spec §6, §9). D1 corrige l'annotation qui prétendait le contraire.
6. ⛔ **Aucune primitive « lien »** — le plan de S2 l'annonçait pour S3 ;
   **aucune ancre n'existe dans aucune entrée** (D11, mesuré). Poser une famille
   sans appelant serait le code mort que §7.6 refuse.
7. ⛔ **Le sens « toute classe déclarée est employée »** de §7.9 (T2) : il
   exigerait une seconde liste d'attente. **C'est `primitives.html` et l'œil qui
   le tiennent.**
8. ⛔ **`galerie-primitives.ts` et `galerie.ts` n'ont toujours pas de test** —
   §7.9 assertion B attrape « une famille cesse d'être rendue », **elle
   n'attrape pas un module de galerie cassé**.
9. ⛔ **Le legs n°9 de S2 — `tokens.ts` ne sait nommer que trois blocs** — reste
   entier : T1 ne touche pas `lireBlocsDeTheme`.
10. ⛔ **Le plafond de 12 288 octets n'est calibré par rien**, et il le reste.

**Ce que S3 n'établira PAS, à écrire dans le document de résultats :**

- **aucun taux, nulle part** — deux exécutions sur des contrôles déterministes
  établissent la reproductibilité, rien de plus ;
- **rien hors d'un Chromium de bureau** : ni Firefox, ni Safari, ni mobile,
  **ni HiDPI** ;
- **l'accessibilité au-delà du contraste et du mouvement réduit** : navigation
  clavier complète, lecteurs d'écran, cibles tactiles minimales, ordre de
  tabulation. **Aucune primitive ne porte de rôle ARIA** — les primitives sont
  du CSS, et la sémantique reste au balisage ;
- **l'anneau de focus reste vérifié NON EFFACÉ, jamais VISIBLE** — et S3 est le
  premier sous-bloc où un `.bouton--principal` focalisé existe réellement sur
  une page du produit, sans que rien ne mesure qu'on le voie sur `--accent` ;
- **aucun contrôle ne mesure une longueur** — G4 vérifie qu'une longueur passe
  par un token, **jamais que le bon token a été choisi** ;
- **la bascule de thème entre deux fenêtres RÉELLES du produit** : S1 l'a
  corroborée **hors critère** entre deux onglets de la **galerie**, jamais entre
  une page-shell et les N sessions qu'elle ouvre par `window.open` ;
- **aucune internationalisation** — rien ne dit qu'une carte survit à un libellé
  plus long ;
- **le legacy n'est pas touché** et **aucun contrôle ne le balaie** : deux
  directions visuelles coexistent dans le dépôt jusqu'au remplacement.

---

## 12. Risques

| # | Risque | Mitigation, ou déclaration |
| --- | --- | --- |
| 1 | 🔴 **L'arbre est partagé, et le chantier du pont de fichiers touche EXACTEMENT `shell.html` et `shell-page.ts`** — ses commits `1cb29b8` et `8d508ca` sont postérieurs à la clôture de S2, et **sa recette tourne en ce moment sur la VM** | **Relire `shell.html` et `shell-page.ts` juste avant d'écrire**, jamais se fier à ce plan pour leur contenu. `git add` **nominatif**, jamais `-A`, jamais `--amend`. ⚠️ **L'invariant du pont — `shell.html:13-19`, « aucune fenêtre d'application n'est spéciale » — ne bouge pas** |
| 2 | **Une tâche fabrique un appelant pour vider une ligne de la liste d'attente** | Le §3.3 l'interdit nommément, et `attente.mjs:66-68` porte déjà la phrase. **La mitigation de T7 rend un re-étiquetage visible ; elle ne rend pas une fabrication visible.** C'est une règle de revue |
| 3 | **La fermeture de §7.4 (T1) rend le contrôle rouge sur l'arbre intact** parce que le prédicat « est une couleur » attrape un token qu'on croyait non coloré | **Mesuré avant d'écrire** : 20 couleurs, 14 dans les blocs clairs, 6 hors thème nommés, **zéro écart**. Si le compte diffère à l'exécution, **c'est le relevé qui a raison** et il faut le dire |
| 4 | 🔴 **Le contrôle §7.9 (T2) est un contrôle vacueux comme les autres** — s'il ne trouve aucune classe, il rend zéro écart et passe | **C'est exactement pourquoi l'assertion ② A existe**, et elle est **rouge sur l'arbre intact, mesuré**. G5 de S2 est le précédent : « sur des feuilles vidées, G1 à G4 restent VERTS » |
| 5 | **Le poids CSS approche le plafond de 12 288** | Mesuré au prototype : **6 374 → ~7 546** avec deux feuilles de surface plausibles, **marge ~4 742**. ⚠️ **Ce n'est pas une prédiction du chiffre final.** S'il approchait, **c'est le plafond qui est arbitraire** (spec §7.7), et le relever est une décision à déclarer, pas à prendre en silence |
| 6 | **La promotion de `selecteur-theme.ts` casse les deux galeries** | T6 les adapte et **les ouvre bâties**. ⚠️ Le piège précis est `galerie.ts:161`, qui **vide** son hôte avant d'installer |
| 7 | **Un `import css from './x.css?raw'` rend la chaîne VIDE sous Vitest** | `test: { css: true }` est dans `client/vite.config.ts`. **NE PAS créer de `client/vitest.config.ts`** : il prendrait le pas **sans rien dire**, et un garde qui parserait ce texte passerait au vert en ne mesurant rien |
| 8 | **Un garde est satisfait par son propre commentaire d'en-tête** | S1 l'a payé sur l'amorce, S2 sur G1/G5/G7 : **tout garde qui cherche une sous-chaîne retire d'abord les commentaires**, et **la mutation vérifie qu'elle a muté** |
| 9 | 🔴 **`git checkout` ne restaure pas un fichier non suivi** — deux mutations y ont survécu en une journée dans ce dépôt | **Commiter avant de muter**, et vérifier la remise en état par `git status --porcelain <chemin>` **vide**, y compris pour les fichiers neufs |
| 10 | **Le jugement visuel n'est pas porté** — S2 ne l'a pas porté | **Le déclarer**, jamais le remplacer par un « probablement » ni par une capture d'écran que personne n'a regardée. **Ce n'est pas un critère** |
| 11 | **Une étape étrangère de `verify-all.sh` tombe** (Postgres absent, `agent/` modifié par un voisin) | **Déclarer, relever `git status`, juger S3 sur les étapes `client` et `proto`** — jamais masquer, jamais imputer |
| 12 | **S3 change l'apparence, et une régression se cache dans un changement voulu** | C'est le §5 : **la liste des changements assumés est le CONTRAT**, et tout ce qui n'y figure pas est une régression. Le critère ③ de la recette mesure la seule surface qui ne doit pas bouger |
