# Sous-bloc S2 — les primitives : bouton, champ, surface, message : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** poser les quatre familles de primitives d'interface dont un écran de
connexion a besoin — **bouton**, **champ**, **surface**, **message** —, les
exprimer **entièrement en tokens** du socle posé par S1, et **faire rétrécir la
liste d'attente des 28 tokens orphelins de 28 à 10 dans les commits mêmes qui
mettent chaque token en service**.

**Architecture:** trois couches, reprises telles quelles de S1.
① Les **valeurs** vivent en CSS sous `client/src/design/` — `primitives.css`
s'ajoute à `tokens.css` et `base.css`, et **ne déclare aucune valeur** : tout
vient de `var(--…)`. ② Les **règles pures** restent en TypeScript
(`contraste.ts` gagne deux paires ; aucune règle neuve). ③ Les **contrôles**
restent sous `client/outils/` et **ne changent pas de forme** : leur périmètre
attrape `primitives.css` sans qu'aucune ligne ne soit ajoutée à leurs scripts —
c'est vérifié plus bas, pas supposé.

**Tech Stack:** Vite 6.4.3, TypeScript 5.5, Vitest 2, Node v24.9.0 — **aucune
dépendance neuve**, ni de production ni de développement.

**Spec :** `docs/superpowers/specs/2026-08-19-design-system-design.md`
(commit `5b6b830`), §3 (les cinq contraintes de direction), §4.4 et §4.5 (les
échelles et la palette), §6 « S2 », §7 (les sept contrôles), §8 (les huit
jugements humains), §10 (les tailles prévues).

**Sous-bloc précédent :** S1, **livré et clos** — plan `f4cb8c0`, recette
`1b4ac3b`, section `CLAUDE.md` `a98d27c`, journaux
`docs/superpowers/plans/journaux-design-s1/`. **S1 fait autorité sur l'état
réel**, et ce plan ne réécrit aucune de ses décisions.

**Ce plan ne couvre QUE S2.** Aucune surface habillée (S3), rien de la fenêtre
de session (S4). En particulier : **`client/index.html`, `client/shell.html`,
`client/connexion.html` et `client/src/style.css` ne reçoivent AUCUNE classe de
primitive**, `#stats` ne reçoit pas `--police-mono`, et les trois longueurs
hors échelle de `style.css` ne sont pas reprises. Les primitives naissent
**définies et montrées**, pas **appliquées** — et le §« Ce que S2 n'établit
PAS » le dit sans le raboter.

---

## Constantes de mesure — relevées le 19 août 2026, et à quel commit

🔴 **TOUT RELEVÉ DE CE PLAN PORTE SON COMMIT, parce que l'arbre est PARTAGÉ.**
Cinq chantiers y travaillent en parallèle (microphone, presse-papier, pont de
fichiers, gestion d'apps, plateforme P3). **Sauf mention contraire, les relevés
ci-dessous ont été pris à `56b975a`**, `git status --porcelain` ne portant
qu'un `?? .playwright-mcp/`.

| Grandeur | Valeur relevée | Commande |
| --- | --- | --- |
| tokens déclarés par `tokens.css` | **47** | `node client/outils/tokens-orphelins.mjs` |
| tokens **employés** par les feuilles de production | **19** | idem |
| **orphelins** | **28**, dont **28 en attente déclarée** — `exit=0` | idem |
| paires de contraste vérifiées | **50**, **0 échec**, minimum global **3.16** | `node client/outils/contraste.mjs` |
| poids CSS bâti | **3 503** octets (`main-*.css` 1 493 + `socle-*.css` 2 010), plafond **12 288**, marge **8 785** | `npm run design:verifier` |
| surfaces bâties | **4** — `connexion.html`, `design.html`, `index.html`, `shell.html` ; assertions A et B **vertes**, B évaluée sur **4** pages | idem |
| couleurs littérales hors `tokens.css` | **0** | idem |
| écarts entre blocs de thème | **0** ; racine **47**, media-clair **13**, attribut-clair **13** | idem |
| agrégateur `design:verifier` | **6/6 contrôles verts**, `exit=0` | idem |
| tests `client` | **187** (21 fichiers) | `cd client && npx vitest run` |
| tests `proto` | **70** (3 fichiers) | `cd proto && npx vitest run` |
| `cd client && npm run typecheck` | **exit 0** | `tsc --noEmit` |
| étapes de `scripts/verify-all.sh` | **DIX** | `grep -c '^etape "' scripts/verify-all.sh` |
| en-têtes `==>` d'une passe `design:verifier` | **SEPT** (1 `npm run build` + 6 contrôles) | `npm run design:verifier \| grep -c '^==>'` |
| fichiers de plus de 500 lignes, dépôt entier | **deux** — `agent/src/encode.rs` 1536, `agent/src/windows_source.rs` 630 | la commande de `CLAUDE.md` §« Conventions de code » |
| plus gros fichier de `client/` | `client/verify-webrtc.mjs` **494** (marge **6**) | idem, filtré `^client/` |

**Les fichiers que S2 va toucher, mesurés le même jour :**

| Fichier | Lignes | Porte |
| --- | --- | --- |
| `client/outils/tokens-orphelins.mjs` | **233** | 300 — et S2 le fait **maigrir** |
| `client/design.html` | **231** | **300 — marge 69** |
| `client/src/design/tokens.css` | **207** | 300 |
| `client/src/design/galerie.ts` | **193** | 300 |
| `client/src/design/contraste.ts` | **150** | 300 |
| `client/src/design/contraste.test.ts` | **114** | 300 |
| `client/src/design/base.css` | **73** | 300 |
| `client/src/style.css` | **181** | 300 — **une seule ligne de commentaire y change** |

---

## Ce que S1 lègue, et que ce plan doit tenir

Les six legs de la section `CLAUDE.md` de S1, et ce que S2 en fait :

| # | Leg de S1 | Sort sous S2 |
| --- | --- | --- |
| 1 | `--police-mono` n'a qu'un appelant prévu, **S4 tranche** | **LAISSÉ À S4**, et la raison est réaffirmée — voir la divergence D8 |
| 2 | les trois longueurs hors échelle de `style.css` | **LAISSÉES À S4** ; S2 **n'en ajoute aucune** — voir la divergence D10 |
| 3 | le plafond de 12 Kio n'est calibré par rien | **INCHANGÉ, non calibré**, et S2 relève ce qu'il consomme |
| 4 | `prefers-reduced-motion` nommé et non pris | 🔵 **PRIS PAR S2**, et l'endroit où le poser est **mesuré** — voir D3 |
| 5 | les trois énoncés faux laissés dans la spec et le plan S1 | **INCHANGÉS** : un index durable ne se commite pas avec une spec (même décision que le legs n°11 de D8) |
| 6 | **la liste d'attente doit RÉTRÉCIR à chaque sous-bloc**, et rien ne force S2 à en consommer | 🔴 **C'EST L'OBJET CENTRAL DE CE PLAN** : chaque tâche nomme les entrées qu'elle retire, et la retenue est un **échec de tâche**, pas un reste à faire |

---

## 🔴 La liste d'attente : 28 aujourd'hui, 10 après S2 — et qui sort quand

**Le fait qui commande ce plan.** `client/outils/tokens-orphelins.mjs:90-135`
déclare `EN_ATTENTE_D_APPELANT`, **28 entrées nommées**, et le contrôle §7.6
exige l'**ÉGALITÉ** entre l'ensemble des orphelins et cette liste. Il échoue
donc **dans les deux sens** :

| Ce qui arrive | Ce que le contrôle dit |
| --- | --- |
| un orphelin **absent** de la liste | `NOUVEL ORPHELIN <token>  déclaré et appelé par personne` |
| une entrée de la liste qui **a gagné** un appelant | `À RETIRER DE LA LISTE <token>  a désormais un appelant (…) : la liste d'attente doit rétrécir` |

**Conséquence directe et non négociable : tout token que S2 met en service
sort de la liste DANS LE MÊME COMMIT.** Ce n'est pas un ménage de fin de
sous-bloc — c'est la seule façon dont chaque commit reste vert, et la seconde
moitié du contrôle est écrite précisément pour l'imposer.

**La liste relevée, telle qu'elle est aujourd'hui**, groupée par tâche
consommatrice **prévue** :

| Tâche | Tokens que la tâche met en service | Compte |
| --- | --- | --- |
| **T2 — bouton** | `--fond-1`, `--fond-2`, `--bord-fort`, `--sur-accent`, `--texte-faible`, `--r-1`, `--trait`, `--duree-1` | **8** |
| **T3 — champ** | `--t-l`, `--danger` | **2** |
| **T4 — surface** | `--bord`, `--r-3`, `--e-4`, `--t-xl`, `--lh-serre` | **5** |
| **T5 — message** | `--succes`, `--alerte`, `--texte` | **3** |
| **T8 — re-tags motivés** | `--t-xs`, `--e-1`, `--r-plein` — **annotés S2, mais aucune des quatre familles ne les emploie** (D6) | **3** |
| **restent en attente** | `--t-2xl`, `--t-3xl`, `--lh-large`, `--e-5`, `--e-6`, `--e-7` (**S3**) ; `--police-mono` (**S4**) | **7** |

**28 = 8 + 2 + 5 + 3 + 3 + 7.** Vérifié par addition sur la liste relevée.

⚠️ **CE TABLEAU EST UNE PRÉDICTION, PAS UN RELEVÉ, et il ne doit pas être
recopié dans un commit.** La source de vérité est la sortie du contrôle :
chaque tâche lance `node client/outils/tokens-orphelins.mjs` **après** avoir
écrit son CSS, **lit les lignes `À RETIRER DE LA LISTE`**, et retire
**exactement celles-là**. Retirer une entrée que le contrôle n'a pas nommée
ferait apparaître un `NOUVEL ORPHELIN` au commit suivant ; en laisser une que
le contrôle nomme laisse la tâche rouge. **Le contrôle dit quoi retirer ; ce
plan dit seulement à quoi s'attendre.**

⚠️ **Un token neuf n'entre JAMAIS sur cette liste.** `--accent-survol` (D2)
naît **avec son appelant**, dans le même commit : le déclarer d'abord et
l'employer ensuite ferait passer §7.6 au rouge entre deux commits, sur un
`NOUVEL ORPHELIN` qui n'apprendrait rien à personne.

⚠️ **Le contrôle ne voit PAS l'annotation d'une entrée.** Il compare des
ensembles de noms ; changer « S2 — le fond des cartes » en « S3 — … » ne
déclenche rien. **Tout re-tag est donc une RÈGLE DE REVUE**, et T8 exige que
chacun porte sa raison. C'est la porte par laquelle on assouplirait cette
liste sans qu'aucune commande ne le dise.

---

## Divergences relevées entre la spec, le legs de S1 et le code réel — tranchées AVANT d'écrire une ligne

Onze points, trouvés en lisant et en mesurant. Chacun est tranché ici, avec
son coût, pour qu'aucune tâche ne les redécouvre à l'exécution.

### D1 — 🔴 La liste d'attente range `--succes`, `--alerte` et `--danger` en S3 ; la spec les confie à S2

`client/outils/tokens-orphelins.mjs:99-101` :

```
['--succes', 'S3 — l’écran d’état « connecté »'],
['--alerte', 'S3 — le bandeau d’avertissement'],
['--danger', 'S3 — l’écran d’état terminal, et le message d’erreur de connexion'],
```

La spec §6, S2, écrit : « **message** (les quatre tons : neutre, succès,
alerte, danger) ». **Les deux ne peuvent pas être vraies ensemble.**

**Tranché : la SPEC l'emporte, l'annotation cède.** Une annotation de liste
d'attente est une **prédiction** écrite par S1 sur ce que S2 emploierait ; le
découpage en sous-blocs, lui, est une décision de conception. S2 consomme donc
les trois (T3 pour `--danger`, T5 pour `--succes` et `--alerte`), et **le
contrôle le forcera de toute façon** : dès que `primitives.css` écrit
`var(--succes)`, la ligne `À RETIRER DE LA LISTE --succes` tombe.

*Coût* : trois entrées de la liste ont été annotées d'un sous-bloc faux
pendant tout S1, sans qu'aucune commande ne puisse le dire — c'est exactement
la portée déclarée trois paragraphes plus haut.

### D2 — 🔴 La spec exige l'état SURVOL du bouton principal et ne donne aucune couleur pour le dire

Spec §6, S2 : « **bouton** (principal, secondaire, discret ; états
**survol**/actif/désactivé/focus) ». Spec §4.5 : « **treize** tokens de couleur
par thème », et le fond du bouton principal est `--accent`. **Aucun token ne
dit « `--accent`, mais survolé ».**

Quatre voies ont été pesées :

| Voie | Verdict |
| --- | --- |
| `filter: brightness(1.1)` | **écartée** — la couleur résultante est **composée à l'exécution**, et la spec §7.1 déclare que le contrôle « ne vérifie pas les couleurs composées à l'exécution ». L'état le plus fréquent du produit serait le seul **non mesuré** |
| `opacity` sur le fond | **écartée**, même raison |
| `color-mix(in oklab, var(--accent) 88%, black)` | **écartée, ET LE CONTRÔLE §7.2 LA REFUSE — mesuré, pas supposé** (voir ci-dessous) |
| **un token neuf `--accent-survol`** | **RETENUE** |

**La mesure qui écarte `color-mix`**, jouée le 19 août 2026 sur un arbre jetable
(`--racine /tmp/s2probe`, aucun fichier du dépôt touché) :

```
$ node client/outils/couleurs-litterales.mjs --racine /tmp/s2probe
/tmp/s2probe/src/x.css:1: .b { background: color-mix(in oklab, var(--accent) 88%, black); }
fichiers balayés : 1
couleurs littérales : 1
→ elles doivent vivre dans client/src/design/tokens.css
exit=1
```

Le mot-clé `black` est cherché **du côté valeur d'une déclaration CSS**
(`couleurs-litterales.mjs`, garde ③) : la voie du mélange est donc fermée par
un contrôle, pas par un avis.

**Décision : UN SEUL token neuf, `--accent-survol`, déclaré dans les TROIS
blocs de `tokens.css`, et DEUX paires de contraste de plus.**

⚠️ **Un seul, et pas deux** : l'état **actif** (enfoncé) ne reçoit pas de
token. Il se dit, pour **toutes** les variantes, par le **retour à la valeur de
repos** — le survol se retire sous le doigt. C'est un arbitrage, il est
perceptible uniquement pendant l'appui, et **il rejoint les huit jugements
humains du §8** : aucune commande ne dira qu'il est le bon.

**Les valeurs proposées, et leur contraste CALCULÉ par la fonction du dépôt**
(`rapportDeContraste` de `client/src/design/contraste.ts`, lancée le 19 août
2026) :

| Thème | `--accent` (repos) | `--sur-accent` sur repos | `--accent-survol` proposé | `--sur-accent` sur survol |
| --- | --- | --- | --- | --- |
| sombre | `#7aa2f7` | **7,73** | `#93b4f9` | **9,39** |
| clair | `#2f5fd0` | **5,72** | `#2650b4` | **7,27** |

Les deux survols **augmentent** le contraste par rapport au repos — le sombre
en éclaircissant l'accent sous une encre sombre, le clair en assombrissant
l'accent sous une encre blanche. **Prédiction, à VÉRIFIER par le contrôle
§7.1 : le minimum global doit rester 3,16** (il vient de `clair
bord-fort/fond-2`, très en dessous de 7,27). **Si la prédiction est fausse,
c'est la valeur qui change, jamais le seuil.**

⚠️ **Le CHOIX des deux teintes est un jugement humain**, exactement comme le
choix de `#7aa2f7` l'était (§8, ligne 2). Seul leur contraste est mesuré.

*Coût, à écrire dans les tâches* : `tokens.css` passe de treize à **quatorze**
couleurs par thème, ce qui rend **trois phrases fausses** — et elles sont
corrigées dans le commit qui les rend fausses, pas plus tard :
`client/design.html:192` (« Couleurs — treize par thème »),
`client/src/style.css:5` (« Les treize couleurs par thème »),
`client/src/design/tokens.css:44` (« Palette SOMBRE — treize couleurs »).
La spec §4.5 (l. 402) et §6 (l. 590) portent le même « treize » : **elles ne
sont PAS modifiées** — ce plan ne modifie pas la spec.
Et `client/src/design/galerie.ts:49-53` porte une liste `COULEURS` **écrite à
la main** : sans l'y ajouter, le token neuf **n'apparaîtrait pas dans la
galerie**, alors que la galerie est l'instrument du jugement humain qui doit le
juger.

### D3 — 🔴 `prefers-reduced-motion` NE PEUT PAS se déclarer dans `tokens.css` : mesuré

Le legs n°4 de S1 nomme `prefers-reduced-motion` « le moins cher des quatre
manques d'accessibilité, et le premier à prendre ». S2 est le sous-bloc qui
**introduit les transitions** (`--duree-1` sur le survol des primitives) : c'est
donc lui qui doit le prendre.

La voie évidente — neutraliser les **durées** dans `tokens.css` — est
**refusée sur mesure**. Relevé le 19 août 2026 sur une copie jetable
(`/tmp/s2-rm.css`, **aucun fichier du dépôt touché**), après y avoir ajouté un
bloc `@media (prefers-reduced-motion: reduce) { :root { --duree-1: 0.01ms;
--duree-2: 0.01ms; } }` :

```
$ node client/outils/blocs-de-theme.mjs --fichier /tmp/s2-rm.css
fichier : /tmp/s2-rm.css
  bloc racine : 47 token(s)
  bloc media-clair : 13 token(s)
  bloc attribut-clair : 13 token(s)
  bloc media-clair : 2 token(s)
écarts : 15
  ÉCART  attribut-clair : --duree-1 manquant
  ÉCART  attribut-clair : --duree-2 manquant
  ÉCART  media-clair : --accent manquant
  … (douze autres)
exit=1
```

**`lireBlocsDeTheme` (`client/src/design/tokens.ts:65-89`) nomme `media-clair`
TOUT bloc `:root` situé sous un `@media` qui ne porte pas
`[data-theme="clair"]`** — il n'y a pas de troisième nom. Un second `@media`
produit donc un **quatrième bloc** que §7.4 compare à la racine, et le contrôle
tombe avec **quinze** écarts.

**Décision : la règle vit dans `client/src/design/base.css`, jamais dans
`tokens.css`.** Elle y neutralise les transitions du produit entier, ce qui
inclut la seule transition existante — `#status` (`style.css:79`,
`transition: opacity var(--duree-2)`). **C'est l'effet voulu de la requête
média**, et il ne se produit que pour un utilisateur qui l'a demandée.

⚠️ **Portée**, à écrire dans le code : ce que S2 prend, c'est la **suppression
des transitions**, pas la gestion complète du mouvement réduit. Il n'y a
aucune animation dans le produit aujourd'hui (`grep -rn '@keyframes\|animation:'
client/src` rend vide, à vérifier au moment de l'écrire) ; **le jour où il y en
aura une, cette règle ne la couvrira pas d'elle-même.**

⚠️ **Legs neuf, nommé et non pris** : `tokens.ts` ne sait pas distinguer deux
requêtes média différentes. Le jour où un sous-bloc voudra vraiment surcharger
un token sous une autre condition, c'est `lireBlocsDeTheme` qui doit apprendre
à nommer ses blocs — pas la règle qui doit se contorsionner.

### D4 — La galerie `design.html` est à 231 lignes pour une porte à 300 : les quatre familles n'y entrent pas

`client/design.html` fait **231** lignes (mesuré). Sa porte est armée à **300**
par la spec §10 et par son propre en-tête (l. 36-37 : « **PORTE ARMÉE À 300
LIGNES** […] au-delà, scinder par famille, au même rythme que les primitives
de S2 »). Quatre familles avec leurs variantes et leurs états, en balisage,
n'entrent pas dans 69 lignes.

**Décision : une CINQUIÈME entrée Vite, `client/primitives.html`, naît — et
`design.html` n'est pas touché d'une ligne.** C'est la scission décidée
**avant** l'addition, jamais après ; le dépôt a franchi le plafond **trois fois
en D10** et **deux fois en D9**, et l'a rattrapé chaque fois après, dont deux
fois par une **compression** que `CLAUDE.md` interdit nommément.

⚠️ **Et elle doit être ajoutée à `EXCLUS` de `tokens-orphelins.mjs`**, pour la
raison **exacte** qui y fait déjà vivre `design.html` : une page de
démonstration emploie des tokens **par construction** (sa propre mise en page
en emploie une poignée), et l'inclure ferait sortir de la liste d'attente des
tokens que **le produit n'appelle pas**. Voir T7, qui joue la rouge
correspondante.

### D5 — 🔴 `scripts/verify-all.sh` compte DIX étapes, pas dix-sept — et les deux comptes sont vrais de choses différentes

Le cahier des charges de ce plan affirmait « **il compte DIX-SEPT étapes, pas
dix** : les sept contrôles de S1 s'y sont ajoutés ». **Relevé par la
commande :**

```
$ grep -c '^etape "' scripts/verify-all.sh
10
$ cd client && npm run design:verifier | grep -c '^==>'
7
```

**Le script a DIX étapes.** Une **exécution complète** affiche **dix-sept**
en-têtes `==>` : les dix du script, plus les **sept** que sa neuvième étape
(`client : npm run design:verifier`) imprime de son côté — un `npm run build`
plus les **six** contrôles. Le septième contrôle du socle, §7.5, est un test
unitaire et tourne dans l'étape `client : npm test`.

**`CLAUDE.md` porte déjà cette précision**, ajoutée par la revue transverse de
P3 : « **ni "dix" ni "dix-sept" ne se suffit sans dire lequel on compte** ».
Ce plan compte **dix étapes**, et le dit chaque fois qu'il le dit.
*(Les 17 ci-dessus sont 10 + 7, une somme dérivée de deux mesures ; ce plan
n'a pas lancé `verify-all.sh` en entier — voir les contraintes globales.)*

### D6 — Trois tokens annotés « S2 » décrivent une famille que la spec ne confie PAS à S2

`--t-xs` (« la mention légale et les étiquettes »), `--e-1` (« l'écart interne
d'une étiquette ») et `--r-plein` (« les pastilles et les boutons ronds »)
désignent tous trois une famille **étiquette / pastille**. Le §6 de la spec
borne S2 à **quatre** familles — bouton, champ, surface, message — et ajoute
que cette liste est bornée par « ce dont un écran de connexion a besoin », **et
non par un catalogue *a priori*** : un écran de connexion n'a pas d'étiquette
ni de pastille.

**Décision : ces trois entrées sont RE-TAGUÉES en T8, avec leur raison
écrite** — et **jamais** consommées par une primitive inventée pour les
consommer. Fabriquer une pastille dans le seul but de vider une ligne de liste
serait « satisfaire un contrôle en vidant l'autre », le geste que la liste
d'attente elle-même refuse dans son encadré (`tokens-orphelins.mjs:70-79`).

⚠️ **Un re-tag n'est vu par AUCUN contrôle** (voir plus haut). C'est le point
le plus faible de ce plan, et il est nommé comme tel.

### D7 — Le périmètre « employé » de §7.6 est le FICHIER, jamais la surface

`tokens-orphelins.mjs:172` :

```js
const feuilles = fichiersCss(join(racine, 'client/src')).filter((f) => f !== source);
```

Toute feuille de `client/src/` entre dans le périmètre « employé », **qu'une
page du produit la lie ou non**. `primitives.css` y entrera donc dès sa
naissance, sans qu'aucune ligne de contrôle ne change — **c'est ce qui permet à
S2 de faire rétrécir la liste sans toucher à une seule surface.**

⚠️ **Et c'est aussi la portée exacte à déclarer, sans la maquiller** : à la fin
de S2, dix-huit tokens auront « un appelant » dans une feuille que **seule la
galerie lie**. Le contrôle mesure qu'un token est **écrit quelque part dans le
produit**, pas qu'une surface le **rende**. C'est S3 qui referme cet écart en
liant `primitives.css` à `shell.html` et `connexion.html`.

**Ce qui a été pesé et écarté** : faire entrer `primitives.css` dans
`socle.css`, donc dans les quatre pages. Écarté parce que la fenêtre de session
ne rend aucune primitive (spec §5.2 : cinq éléments, dont aucune carte ni
aucun champ) et que c'est la surface où la première image doit arriver vite
(§4.3). *Coût de ce choix* : jusqu'à S3, aucune page du produit ne charge les
primitives.

### D8 — `--police-mono` reste à S4, et la raison est celle de S1

La consigne de ce plan demandait de ne pas le reprendre « sans dire pourquoi ».
**Décision : S2 n'y touche pas.** Trois raisons, dans l'ordre :

1. son seul appelant prévu est `#stats`, qui vit dans `client/src/style.css`,
   c'est-à-dire dans la **fenêtre de session** — que **seul S4** a le droit de
   toucher (S1 l'a inscrit dans l'entrée de liste elle-même :
   « **Aucun autre sous-bloc n'a le droit de laisser cette ligne en place sans
   décider** », `tokens-orphelins.mjs:125-134`) ;
2. aucune des quatre familles de S2 n'a besoin d'une pile monospace : ni un
   bouton, ni un champ, ni une carte, ni un message n'affiche de nombre qui
   change ;
3. **le lui donner un appelant de complaisance dans `primitives.css`** — par
   exemple une classe `.mono` que personne n'emploierait — retirerait de la
   liste la **seule entrée dont le sort soit encore ouvert**, et lui ferait
   perdre son statut de décision en attente. C'est le même geste que D6 refuse.

⚠️ **Si la galerie des primitives employait `--police-mono`, cela n'y
changerait rien** : `primitives.html` est exclue du périmètre « employé »
(D4).

### D9 — Les comptes de tests ont bougé sous les voisins

S1 relevait `client` **179** et `proto` **37**. Aujourd'hui : `client` **187**
(21 fichiers), `proto` **70** (3 fichiers). **Aucun de ces mouvements n'est de
notre fait.** Chaque tâche qui ajoute des tests **annonce le compte attendu
AVANT de le mesurer**, et un écart se relève et s'attribue au voisin, jamais à
soi. *(C'est le geste qui a permis à un implémenteur de D10 de s'apercevoir
qu'un `Write` avait écrasé un test d'une tâche antérieure.)*

### D10 — Aucun contrôle ne mesure une longueur, et S2 en écrit beaucoup

`client/src/style.css:27-31` le déclare déjà : « la clause "aucune longueur
hors échelle" du §8 est **FAUSSE** à la fin de ce sous-bloc, et de trois
valeurs exactement […] aucun contrôle n'en souffre, puisque aucun des sept ne
mesure une longueur ».

**S2 écrit des rembourrages, des rayons et des épaisseurs sur quatre familles.
Si l'une sort de l'échelle, RIEN ne le dira.** Décision : **toute longueur de
`primitives.css` est un `var(--e-*)`, `var(--r-*)`, `var(--trait*)` ou
`var(--t-*)`** ; les seules exceptions autorisées sont `0`, `100%`, `1` et les
unités relatives de mise en page (`auto`, `1fr`). **C'est une règle de revue —
et T2 en fait aussi un garde de test**, parce qu'une règle de revue non
outillée sur quatre familles ne survivrait pas à S3.

⚠️ **Et S2 ne reprend PAS les trois longueurs de `style.css`** : les aligner
changerait l'apparence de la fenêtre de session, et pour le `18px` cela
invaliderait la justification chiffrée de la zone ~40×36 px du bouton plein
écran (`style.css:121-126`, spec §5.2). **C'est S4.**

### D11 — L'anneau de focus existe DÉJÀ, globalement — et le vrai risque est de l'effacer

`client/src/design/base.css:70-73` :

```css
:focus-visible {
    outline: var(--trait-focus) solid var(--accent);
    outline-offset: var(--trait-focus);
}
```

**Aucune primitive n'a donc à déclarer un anneau de focus** : elles l'ont
toutes, y compris celles qui n'existent pas encore. Le risque n'est pas
l'absence, c'est la **suppression** — `outline: none` posé sur un bouton
« pour faire propre » est le défaut d'accessibilité le plus banal du métier, et
**aucun des sept contrôles ne le verrait**.

**Décision : `primitives.css` ne peut contenir ni `outline: none` ni
`outline: 0`, et un garde de test le vérifie** (T2). C'est le seul endroit de
S2 où l'accessibilité clavier reçoit un outil plutôt qu'une intention.

⚠️ **Ce que le garde ne dit pas** : que l'anneau soit **visible** sur le fond
de chaque primitive. `--accent` est mesuré contre `--fond-0`, `--fond-1` et
`--fond-2` au seuil 4,5 (§7.1, `contraste.ts:63-71`) — donc au-dessus du seuil
3 que WCAG 1.4.11 demande à un indicateur de focus —, mais **rien ne vérifie
que l'anneau n'est pas posé sur un fond `--accent`** (un bouton principal
focalisé). `outline-offset` l'en écarte de 2 px, ce qui le pose sur le fond de
la page ; **c'est un raisonnement, pas une mesure**.

---

## Global Constraints

- **Plafond de 500 lignes** (`CLAUDE.md`, §« Conventions de code »), **et porte
  d'action à 300** pour tout fichier neuf de ⑥ (spec §10). Relevé par la
  commande à `56b975a` : le dépôt entier n'a que **deux** fichiers au-dessus —
  les deux lignes de la dette gelée —, et le plus gros de `client/` est
  `client/verify-webrtc.mjs` à **494** (marge **6**).
  ⚠️ **`client/verify-webrtc.mjs` n'est touché par AUCUNE tâche de ce plan.**
  La leçon de P2 tient en une phrase : « **une addition de commentaire peut
  annuler une extraction** » — sa revue transverse a failli y ramener le
  fichier **exactement à 497**, le chiffre d'avant l'extraction qu'elle venait
  de payer.
  **Portes armées d'avance pour les fichiers de S2** :
  `design/primitives.css` **à 240 lignes** (voir la procédure d'extraction
  ci-dessous), `client/primitives.html` **à 300** (scinder par famille),
  `design/primitives.test.ts` **à 300**.
- 🔴 **`primitives.css` : la porte est à 240, pas à 300, et voici pourquoi.**
  La spec §10 arme 300. Une famille coûte de 40 à 80 lignes ; une porte à 300
  laisserait donc la dernière famille la franchir **pendant** son écriture,
  c'est-à-dire exactement le cas que ce dépôt a payé cinq fois. **Chaque tâche
  de famille commence par `wc -l client/src/design/primitives.css`, et si le
  compte est ≥ 240, elle EXTRAIT AVANT d'écrire une ligne** :
  `design/primitives/{bouton,champ,surface,message}.css`, et `primitives.css`
  devient quatre `@import` — la forme exacte de `design/socle.css`.
  *(L'`@import` fait ré-inliner la feuille dans chaque entrée qui la traverse ;
  `primitives.css` n'étant liée que par une page en S2, cela ne duplique rien.
  Voir l'encadré mesuré de `client/src/style.css:34-49`.)*
- **Jamais `git add -A`.** L'arbre est partagé par **cinq** chantiers ; chaque
  commit porte sa pathspec explicite, et **jamais `git commit --amend`**.
- ⛔ **Aucune tâche de ce plan n'emploie la VM Windows.** ⑥ est un sous-projet
  **navigateur**, et la spec §9 déclare qu'il n'a **aucune recette sur VM**.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** son implémentation et **vu échouer**, et le
  message d'échec attendu est écrit dans la tâche.
  🔴 **Et une rouge ne vaut que pour l'assertion qu'elle fait tomber** :
  `expect` interrompt le test à la première. **Tout garde à plusieurs
  assertions se joue en autant de rouges qu'il a d'assertions.**
  🔴 **Une perturbation qui ne perturbe rien se lit exactement comme un
  contrôle qui ne mord pas.** S1 l'a payé deux fois : une rouge de §7.4 qui
  découpait un **commentaire** au lieu du bloc, et un garde d'amorce satisfait
  par le commentaire d'en-tête du fichier qu'il analysait. **Tout garde de ce
  plan qui cherche une sous-chaîne RETIRE D'ABORD LES COMMENTAIRES**, et la
  tâche le dit.
  ⚠️ **Ce plan est une source de contrôles vacueux comme les autres** : D10 en
  a attrapé **quatre**, dont **trois écrits par le plan lui-même**. Le doute
  porte sur ce document.
- **Aucun taux ne sera revendiqué.** Les contrôles de ⑥ sont **déterministes**
  (spec §9) : deux exécutions y établissent la **reproductibilité**, jamais une
  fréquence, et la question « combien de fois sur combien » **ne doit pas être
  empruntée** à une campagne qui, elle, l'aurait posée.
- **Aucune dépendance neuve**, ni de production ni de développement.
- **La suite existante reste verte, et le compte est nommé d'avance** :
  **187** tests `client` (21 fichiers) et **70** tests `proto` à `56b975a`. S2
  **ajoute** des tests et n'en retire aucun ; chaque tâche qui en ajoute
  **annonce le compte attendu AVANT de le mesurer**.
- 🔴 **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`** : la racine
  Vitest est `client/`. Tous les tests de S2 vivent sous `client/src/design/`.
- 🔴 **Il n'y a délibérément PAS de `client/vitest.config.ts`.** `test: { css:
  true }` est posé dans `client/vite.config.ts:87-105` ; un `vitest.config.ts`
  **prendrait le pas sur lui sans rien dire**, et un `import css from
  './x.css?raw'` rendrait alors la chaîne **vide** — un garde qui parserait ce
  texte **passerait au vert en ne mesurant rien**. **Ne pas le déplacer.**
  Tous les gardes de S2 lisent leur CSS par `?raw` et **dépendent de cette
  ligne**.
- **La référence de vérification est `scripts/verify-all.sh`**, **dix étapes**
  (D5). ⚠️ **Ce plan ne prétend PAS qu'il sort à 0 aujourd'hui** : il n'a pas
  été lancé pendant la rédaction — sa première étape est `cargo test
  --workspace` sur un `agent/` que trois chantiers voisins modifient, et son
  étape `plateforme : npm run test:postgres` exige une instance Postgres.
  **Si une étape étrangère tombe à la recette : le déclarer, relever
  `git status`, et juger S2 sur les étapes `client` et `proto`** — jamais
  masquer, jamais imputer.
- **L'ancien code est hors périmètre** : `assets/`, `web/`, `src/`, `index.js`
  ne sont ni lus ni modifiés, et **aucun contrôle ne les balaie** (spec §4.6).
- ⛔ **Aucune surface du produit n'est habillée** : `index.html`, `shell.html`,
  `connexion.html` et `style.css` ne reçoivent **aucune classe de primitive**.
  `style.css` ne reçoit qu'**une** correction de commentaire (D2), et
  `connexion.html:10-25` — qui annonce que « **S3** les pose, et c'est lui qui
  retirera ce bloc » — **n'est pas touché**.

---

## Structure des fichiers

```
client/
  primitives.html                          NEUF  — la galerie des primitives, 5ᵉ entrée Vite (T7)
  design.html                              MOD   — « treize » → « quatorze » (T2), et RIEN d'autre
  vite.config.ts                           MOD   — la 5ᵉ entrée (T7)
  src/
    style.css                              MOD   — une ligne de commentaire (T2)
    design/
      primitives.css                       NEUF  — les quatre familles (T2 à T5)
      primitives.test.ts                   NEUF  — les gardes de forme (T2 à T6)
      tokens.css                           MOD   — `--accent-survol` ×3 blocs (T2)
      base.css                             MOD   — `prefers-reduced-motion` (T6)
      contraste.ts                         MOD   — +2 paires, 50 → 52 (T2)
      contraste.test.ts                    MOD   — le compte 50 → 52 (T2)
      galerie.ts                           MOD   — `boutons()` EXTRAIT (T1), `COULEURS` +1 (T2)
      selecteur-theme.ts                   NEUF  — l'extraction de T1
      galerie-primitives.ts                NEUF  — l'entrée de `primitives.html` (T7)
  outils/
    tokens-orphelins.mjs                   MOD   — la liste rétrécit (T2..T5, T8), `EXCLUS` +1 (T7)
docs/superpowers/plans/
  2026-08-19-design-system-s2-resultats.md            NEUF (T9)
  journaux-design-s2/                                 NEUF (T9)
CLAUDE.md                                             MOD  (T10)
```

⚠️ **Aucun script de `client/outils/` ne change de forme**, et c'est un
résultat, pas une chance : `primitives.css` entre dans le périmètre de §7.2 et
de §7.6 **par les chemins déjà écrits** (`client/src/**/*.css`), et
`primitives.html` entre dans celui de §7.2 et §7.3 **par la liste des entrées
Vite, que les deux scripts lisent au lieu de la recopier**. Les seules
modifications de `tokens-orphelins.mjs` sont **des données** : la liste
d'attente et `EXCLUS`.

---

## Interfaces partagées

### La convention de nommage des classes — française, BEM, et sans aucun sélecteur d'élément

```
.bouton                     .bouton--principal  .bouton--secondaire  .bouton--discret
.champ                      .champ--erreur
.champ__etiquette  .champ__saisie  .champ__aide  .champ__erreur
.carte                      .carte__titre  .carte__corps
.separateur
.message                    .message--succes  .message--alerte  .message--danger
```

🔴 **AUCUN SÉLECTEUR D'ÉLÉMENT NU dans `primitives.css`** — ni `button`, ni
`input`, ni `a`, ni `*`, ni `:root`. **C'est ce qui garantit que S2 ne change
rien à `client/index.html`**, dont les cinq éléments (`#remote`, `#status`,
`#stats`, `#micro`, `#fullscreen`) ne portent aucune classe de primitive — et
dont deux sont des `<button>`. Un `button { … }` dans `primitives.css`
changerait l'apparence de la fenêtre de session **sans qu'aucun des sept
contrôles ne le dise**.

**La règle, telle que le garde de T2 la vérifie** : après retrait des
commentaires et des enveloppes `@media`, **chaque compound de chaque sélecteur
contient au moins un `.`**. `.bouton:hover` passe ; `.carte > .carte__titre`
passe (deux compounds, deux points) ; `button` échoue ; `input[type="text"]`
échoue ; `*` échoue. Les pseudo-classes (`:hover`, `:active`, `:disabled`,
`:focus-visible`) et les pseudo-éléments (`::placeholder`) sont libres **tant
qu'ils sont attachés à une classe**.

### `client/src/design/selecteur-theme.ts` — l'extraction de T1

```ts
// PUR de toute galerie : il ne connaît ni token, ni couleur, ni mise en page.
import type { Theme } from './theme';

export interface Coffre { getItem(c: string): string | null; setItem(c: string, v: string): void; }
export interface Racine { setAttribute(n: string, v: string): void; removeAttribute(n: string): void; }

/**
 * Pose les trois boutons de thème dans `hote`, applique l'état stocké, et
 * branche l'écoute de `storage`. `apres` est appelé après chaque changement,
 * quelle qu'en soit l'origine — c'est par là que la galerie de tokens redessine.
 */
export function installerSelecteurDeTheme(
    hote: HTMLElement,
    racine: Racine & HTMLElement,
    coffre: Coffre,
    apres: () => void,
): void;
```

⚠️ **C'est du code d'INSTRUMENT, pas du produit, et il n'a pas de test** —
même statut que `galerie.ts`, et pour la même raison : ce qu'il aurait de
testable (la machine à trois états) vit déjà dans `theme.ts`, qui porte les
**dix** tests du contrôle §7.5. **Ce statut est déclaré, pas subi.**
⚠️ **Il n'est PAS le sélecteur de thème du produit** : celui-là appartient à
**S3** (spec §5.2, famille 3), qui décidera s'il le réemploie ou non.

### `client/src/design/contraste.ts` — deux paires de plus

```ts
// pairesDuTheme() gagne UNE ligne :
paires.push({ theme, encre: '--sur-accent', fond: '--accent-survol', seuil: SEUIL_TEXTE });
// → 26 paires par thème, 52 au total.
```

⚠️ **`--accent-survol` n'entre PAS dans `ENCRES`** : ce n'est pas une encre,
c'est un fond, et l'ajouter à `ENCRES` créerait six paires qui n'ont aucun sens
(un survol de bouton n'est jamais du texte sur `--fond-1`).

---

## Les états livrés par S2, et ceux qui ne le sont pas

| État | Livré ? | Comment il se dit |
| --- | --- | --- |
| **repos** | ✅ | fond, encre, trait, rayon — tous en tokens |
| **survol** | ✅ | principal : `--accent-survol` ; neutres : `--fond-1` → `--fond-2` |
| **actif** (enfoncé) | ✅ **par le RETOUR au repos** | le survol se retire sous le doigt. ⚠️ **Arbitrage, pas mesure** — il rejoint les jugements du §8 |
| **focus** | ✅ **par héritage** | `base.css:70-73`, global. Les primitives ne le redéclarent pas, et **ne peuvent pas l'effacer** (garde de T2, D11) |
| **désactivé** | ✅ **par les tokens, jamais par l'opacité** | encre `--texte-faible` sur `--fond-1`/`--fond-2`, `cursor: not-allowed`. ⚠️ **Une opacité rendrait la couleur effective NON MESURÉE** par §7.1 |
| **erreur** (champ) | ✅ | trait et message en `--danger` |
| **mouvement réduit** | ✅ (T6) | `@media (prefers-reduced-motion: reduce)` dans `base.css` — legs n°4 de S1 |
| ❌ **chargement / en cours** | non | aucun indicateur de progression : la spec §6 ne le confie pas à S2, et le produit n'en a aucun |
| ❌ **`:visited`** | non | S2 ne livre pas de primitive « lien » — la spec §3 réserve l'accent au lien, S3 le posera |
| ❌ **sélection, indéterminé, lecture seule** | non | ni case à cocher, ni interrupteur : un écran de connexion n'en a pas |
| ❌ **cibles tactiles minimales** | non | spec §9, « l'accessibilité au-delà du contraste » n'est pas mesurée |
| ❌ **lecteurs d'écran, navigation clavier complète** | non | idem. Les primitives sont du CSS : elles n'ajoutent **aucun** rôle ARIA, et n'en retirent aucun |

---

# Famille ⓪ — l'extraction, AVANT l'addition qui l'exigera

### Task 1 : `design/selecteur-theme.ts` — extraire les boutons de thème de la galerie

**Objet :** sortir les trois boutons de thème de `galerie.ts` pour qu'une
seconde galerie puisse les réemployer **sans recopier quinze lignes**, et le
faire **avant** que cette seconde galerie n'existe.

**Files:**
- Create: `client/src/design/selecteur-theme.ts`
- Modify: `client/src/design/galerie.ts`

**Interfaces:**
- Consumes: `theme.ts` (`CLE_THEME`, `appliquer`, `choisir`, `surStockageModifie`, `themeStocke`).
- Produces: `installerSelecteurDeTheme(hote, racine, coffre, apres)`.

**Tokens sortis de la liste d'attente : AUCUN.** Cette tâche ne touche aucun
CSS.

- [ ] **Step 1 : relever l'état AVANT**

```bash
cd /home/mallanic/Projects/Guacamole
git rev-parse --short HEAD
wc -l client/src/design/galerie.ts
node client/outils/tokens-orphelins.mjs | tail -3
```
**Attendu à `56b975a`** : `galerie.ts` **193** lignes ; §7.6 `exit=0`, **28
orphelins, 28 en attente déclarée**. Si les comptes diffèrent, un voisin a
bougé : **le noter, et continuer avec le compte relevé** (D9).

- [ ] **Step 2 : écrire `selecteur-theme.ts`**

Il porte, **et rien d'autre** : la création des trois boutons
(`systeme`/`clair`/`sombre`), leur `aria-pressed`, l'appel à `choisir()`,
l'application initiale par `appliquer(racine, themeStocke(coffre))`, et
l'écoute de `storage` filtrée sur `CLE_THEME`.

⚠️ **La transposition est VERBATIM**, pas une réécriture : le corps de
`boutons()` (`galerie.ts:155-176`) et les deux blocs de fin de fichier
(`galerie.ts:178-188`) se déplacent, `rendre()` devenant le paramètre `apres`.
**Comparer les deux textes mot pour mot avant de commiter** — c'est ce que D9
a exigé pour `sommeil/registre.rs`, et c'est la seule façon de savoir qu'une
extraction n'a rien changé.

⚠️ **En-tête obligatoire** : ce module est un **instrument**, il n'a pas de
test, et le sélecteur de thème du **produit** appartient à S3.

- [ ] **Step 3 : `galerie.ts` appelle l'extraction**

`galerie.ts` perd `boutons()`, l'`appliquer` de démarrage et l'écouteur
`storage` ; il appelle
`installerSelecteurDeTheme(vide('themes'), racine, localStorage, rendre)`.

- [ ] **Step 4 : la preuve que rien n'a bougé**

```bash
cd client && npm run typecheck && npm test 2>&1 | grep -E 'Test Files|Tests '
npm run design:verifier | tail -5
wc -l src/design/galerie.ts src/design/selecteur-theme.ts
```
**Attendu : `typecheck` exit 0 ; `Tests  187 passed` — le MÊME compte qu'au
Step 1**, cette tâche n'ajoutant aucun test ; **6/6 contrôles verts** ;
`galerie.ts` **allégé** d'environ 35 lignes.

🔴 **La rouge de cette tâche est une rouge de NON-RÉGRESSION, et elle se joue
sur le seul appelant existant** : commenter l'appel à
`installerSelecteurDeTheme` dans `galerie.ts`, bâtir, ouvrir
`dist/design.html`, constater que **les trois boutons de thème ont disparu**,
défaire. **Sans cette rouge, une extraction qui n'installerait rien passerait
tous les contrôles** — `design.html` porte `<p id="themes"></p>` vide, et
aucun des sept ne regarde le DOM.

- [ ] **Step 5 : commit**

```bash
git add client/src/design/selecteur-theme.ts client/src/design/galerie.ts
git commit -m "design(s2): les boutons de theme sortent de la galerie, avant la seconde" \
  -- client/src/design/selecteur-theme.ts client/src/design/galerie.ts
```

---

# Famille ① — les quatre primitives, une famille par tâche

### Task 2 : `primitives.css` naît avec la famille BOUTON, le token `--accent-survol`, et les gardes de forme

**Objet :** poser le fichier, sa première famille, le seul token neuf de S2, et
**les quatre gardes qui protégeront les trois familles suivantes**.

**Files:**
- Create: `client/src/design/primitives.css`, `client/src/design/primitives.test.ts`
- Modify: `client/src/design/tokens.css`, `client/src/design/contraste.ts`,
  `client/src/design/contraste.test.ts`, `client/src/design/galerie.ts`,
  `client/design.html`, `client/src/style.css`,
  `client/outils/tokens-orphelins.mjs`

**Tokens que cette tâche doit sortir de la liste — PRÉDICTION, à confirmer par
la sortie du contrôle :** `--fond-1`, `--fond-2`, `--bord-fort`,
`--sur-accent`, `--texte-faible`, `--r-1`, `--trait`, `--duree-1` (**8**).

- [ ] **Step 1 : les gardes de forme, écrits AVANT le CSS, et vus rouges**

`client/src/design/primitives.test.ts` lit `./primitives.css?raw` — **et cette
lecture ne rend un texte non vide que grâce à `test: { css: true }` de
`client/vite.config.ts:104`** ; l'en-tête du fichier le dit.

Il **retire d'abord les commentaires** (`/* … */`), puis assert :

1. **G1 — aucun sélecteur d'élément nu** : chaque compound de chaque sélecteur
   contient au moins un `.` (voir « Interfaces partagées »).
2. **G2 — l'anneau de focus n'est jamais effacé** : aucune déclaration
   `outline` de valeur `none` ou `0` (D11).
3. **G3 — aucun état ne se dit par une composition d'exécution** : aucune
   déclaration `opacity:` ni `filter:` (D2 — une couleur composée à
   l'exécution échappe à §7.1).
4. **G4 — aucune longueur hors échelle** : toute valeur portant une unité
   (`px`, `rem`, `em`, `ms`) est **dans un `var(--…)`** ; les littéraux
   autorisés sont `0`, `100%`, `1` et les mots-clés (D10).
5. **G5 — le garde d'atteignabilité** : le fichier déclare **au moins une**
   règle, et la famille `.bouton` y figure.

🔴 **G5 EXISTE PARCE QUE G1 À G4 SONT DES TESTS D'ABSENCE, ET QU'UN TEST
D'ABSENCE EST VERT SUR UN FICHIER VIDE.** Sans lui, un `primitives.css` réduit
à son en-tête passerait quatre gardes sur cinq — c'est la classe de défaut dont
ce dépôt a attrapé quatre exemplaires sur le seul sous-bloc D10, dont trois
écrits par un plan.

🔴 **ET LE RETRAIT DES COMMENTAIRES N'EST PAS UN DÉTAIL** : l'en-tête de
`primitives.css` expliquera *pourquoi* `outline: none` est interdit, donc
**écrira la chaîne `outline: none`**. Un garde qui chercherait la sous-chaîne
sans blanchir les commentaires serait satisfait par sa propre justification —
c'est **mot pour mot** ce qui est arrivé au garde de l'amorce en S1
(`CLAUDE.md`, pièges de S1 : « **UN GARDE PEUT ÊTRE SATISFAIT PAR SON PROPRE
COMMENTAIRE** »).

**Les CINQ rouges, une par garde, jouées séparément** — `expect` s'arrêtant à
la première, un test qui les grouperait n'en éprouverait qu'une :

| Garde | Mutation | Attendu |
| --- | --- | --- |
| G1 | ajouter `button { color: var(--texte); }` | échec nommant `button` |
| G2 | ajouter `outline: none;` dans `.bouton:focus-visible` | échec nommant la déclaration |
| G3 | remplacer l'encre du désactivé par `opacity: 0.5` | échec nommant `opacity` |
| G4 | remplacer `var(--r-1)` par `4px` | échec nommant `4px` |
| G5 | **vider** `primitives.css` de ses règles (garder l'en-tête) | échec « aucune règle » **et** « `.bouton` absente » |

⚠️ **La rouge de G3 doit être jouée sur une déclaration RÉELLE d'un état**, pas
sur une ligne ajoutée n'importe où : c'est ce qui distingue « le garde attrape
`opacity` » de « le garde attrape ce qu'on lui donne ».

- [ ] **Step 2 : le token `--accent-survol`, dans les TROIS blocs**

`tokens.css` : `--accent-survol: #93b4f9;` dans `:root` (après `--accent`),
`--accent-survol: #2650b4;` dans **les deux** blocs clairs.

🔴 **La rouge de §7.4 se joue en le posant dans UN SEUL bloc** :

```bash
node client/outils/blocs-de-theme.mjs
```
**Attendu : `exit=1`**, avec deux lignes `ÉCART … --accent-survol manquant`.
Poser les trois, relancer, **`écarts : 0`** et `racine : 48 token(s)`.

⚠️ **Ne PAS le poser dans un `@media (prefers-reduced-motion)` ni dans aucun
autre bloc conditionnel** — voir D3, où la mesure montre que `tokens.ts` y
verrait un quatrième bloc et rendrait quinze écarts.

- [ ] **Step 3 : les deux paires de contraste, et le compte qui les prouve**

`contraste.ts` : une ligne dans `pairesDuTheme`.
`contraste.test.ts` : **`50` → `52` aux lignes `:49` et `:90`**, et le
commentaire de `:45-48` qui énumère la composition des paires reçoit son
quatrième terme.

🔴 **La rouge est NATURELLE, et il faut la voir dans cet ordre** : modifier
`contraste.ts` **d'abord**, lancer `npx vitest run src/design/contraste.test.ts`,
et voir **deux** échecs (`expected 52 to have length 50` et
`expected 52 to be 50`). Puis corriger le test. **Modifier les deux ensemble
priverait le compte de tout pouvoir** — un `PAIRES` silencieusement raccourci
par une faute de frappe passerait.

```bash
node client/outils/contraste.mjs
```
**Attendu : `paires vérifiées : 52`, `échecs : 0`, `minimum global : 3.16`.**
🔴 **Le minimum DOIT rester 3,16** : il vient de `clair bord-fort/fond-2`, et
les deux paires neuves rendent 9,39 et 7,27 (calculées à la rédaction de ce
plan par `rapportDeContraste`). **S'il change, c'est que les valeurs proposées
ne sont pas celles qui ont été posées** — le relever, pas le raboter.

🔴 **La rouge de §7.1 sur le token neuf** : porter `--accent-survol` clair à
`#8fa8e0`, relancer.
**Attendu : `ÉCHEC clair sur-accent/accent-survol = … < 4.5`, `exit=1`.**
Remettre. **Sans cette rouge, les deux paires neuves seraient déclarées et
jamais éprouvées** — le contrôle compterait 52 en ne discriminant que sur 50.

- [ ] **Step 4 : la famille BOUTON**

Trois variantes, quatre états, **et rien d'autre** :

| | repos | survol | actif | désactivé |
| --- | --- | --- | --- | --- |
| `--principal` | fond `--accent`, encre `--sur-accent` | fond `--accent-survol` | **retour au repos** | fond `--fond-2`, encre `--texte-faible` |
| `--secondaire` | fond `--fond-1`, encre `--texte-fort`, trait `--trait` `--bord-fort` | fond `--fond-2` | **retour au repos** | fond `--fond-1`, encre `--texte-faible`, trait `--bord` |
| `--discret` | fond `transparent`, encre `--texte` | fond `--fond-1` | **retour au repos** | encre `--texte-faible` |

Commun : `border-radius: var(--r-1)`, `padding: var(--e-2) var(--e-3)`,
`font: inherit`, `font-size: var(--t-m)`, `cursor: pointer`,
`transition: background var(--duree-1)`, et `cursor: not-allowed` sur
`:disabled`.

⚠️ **`transparent` est autorisé par §7.2** (`couleurs-litterales.mjs`), et
c'est la seule valeur de couleur non-`var()` que `primitives.css` a le droit
d'écrire.
⚠️ **Le trait du bouton secondaire est `--bord-fort`, pas `--bord`** : il porte
l'information « ceci est un contrôle », et la spec §4.5 réserve `--bord` aux
séparateurs **purement décoratifs**. **Aucune commande ne le vérifie** —
c'est la règle de revue que le §8 nomme.

- [ ] **Step 5 : les trois phrases que le token neuf rend fausses, corrigées ICI**

`client/design.html:192`, `client/src/style.css:5`,
`client/src/design/tokens.css:44` — « treize » → « quatorze ».
`client/src/design/galerie.ts:49-53` — `'--accent-survol'` s'ajoute à
`COULEURS`, **après `--accent`**.

```bash
grep -rn 'treize' client/design.html client/src/style.css client/src/design/tokens.css
```
**Attendu : aucune ligne.** *(La spec porte le même mot en `§4.5` et `§6` :
elle n'est PAS modifiée — ce plan ne modifie pas la spec, et le §« Ce que S2
n'établit PAS » le déclare.)*

🔴 **Ce Step est la contre-mesure au patron que la revue transverse de D10 a
attrapé douze fois** : *une affirmation écrite par une tâche et réfutée par une
autre tâche de la MÊME branche.* Ici la tâche qui la réfute est celle qui
l'écrit — **il n'y a donc aucune excuse à la laisser.**

- [ ] **Step 6 : faire rétrécir la liste — en LISANT le contrôle, pas ce plan**

```bash
node client/outils/tokens-orphelins.mjs
```
**Attendu AVANT retrait : `exit=1`**, avec autant de lignes
`À RETIRER DE LA LISTE` que de tokens que la famille bouton vient d'employer.
**Retirer exactement ces entrées de `EN_ATTENTE_D_APPELANT`**, relancer.
**Attendu APRÈS : `exit=0`**, `20 token(s) restent en attente` si la prédiction
de 8 est juste.

⚠️ **Si le contrôle nomme un token que ce plan n'avait pas prévu, c'est le
plan qui a tort** : retirer l'entrée nommée, et **le noter pour le document de
résultats**. ⚠️ **Si un `NOUVEL ORPHELIN` apparaît, c'est que le token neuf a
été déclaré sans appelant** — vérifier que `primitives.css` écrit bien
`var(--accent-survol)`.

- [ ] **Step 7 : le compte de tests, annoncé AVANT d'être mesuré**

`primitives.test.ts` ajoute **cinq** tests (G1 à G5). **Attendu : `Tests  192
passed`** (187 + 5). Un écart se relève et s'explique **avant** de commiter.

```bash
cd client && npm test 2>&1 | grep -E 'Test Files|Tests ' && npm run typecheck && npm run design:verifier | tail -3
wc -l src/design/primitives.css src/design/primitives.test.ts
```

- [ ] **Step 8 : commit**

```bash
git add client/src/design/primitives.css client/src/design/primitives.test.ts \
        client/src/design/tokens.css client/src/design/contraste.ts \
        client/src/design/contraste.test.ts client/src/design/galerie.ts \
        client/design.html client/src/style.css client/outils/tokens-orphelins.mjs
git commit -m "design(s2): la famille bouton, le token de survol, et cinq gardes de forme" \
  -- client/src/design/primitives.css client/src/design/primitives.test.ts \
     client/src/design/tokens.css client/src/design/contraste.ts \
     client/src/design/contraste.test.ts client/src/design/galerie.ts \
     client/design.html client/src/style.css client/outils/tokens-orphelins.mjs
```

---

### Task 3 : la famille CHAMP — étiquette, aide, erreur, et les états

**Objet :** le champ de saisie dont l'écran de connexion a besoin — deux
`<input>` et leurs étiquettes (`client/connexion.html:26-43`) —, avec son état
d'erreur.

**Files:**
- Modify: `client/src/design/primitives.css`, `client/src/design/primitives.test.ts`,
  `client/outils/tokens-orphelins.mjs`

**Tokens prédits :** `--t-l` (le libellé — spec §4.4 : « intertitres, libellés
de champ »), `--danger` (**2**). ⚠️ `--danger` est annoté **S3** dans la liste :
c'est la divergence **D1**, et c'est la spec qui l'emporte.

- [ ] **Step 0 : la porte de 240 lignes**

```bash
wc -l client/src/design/primitives.css
```
**Si le compte est ≥ 240 : EXTRAIRE AVANT d'écrire une ligne** (voir les
contraintes globales). **Jamais compresser** — S1 l'a fait deux fois et la
revue a exigé l'extraction les deux fois.

- [ ] **Step 1 : la famille**

`.champ` (l'enveloppe), `.champ__etiquette` (`--t-l`, `--texte`),
`.champ__saisie` (fond `--fond-1`, encre `--texte-fort`, trait `var(--trait)
solid var(--bord-fort)`, rayon `--r-1`, rembourrage `var(--e-2) var(--e-3)`),
`.champ__saisie::placeholder` (`--texte-faible`), `.champ__aide`
(`--t-s`, `--texte-faible`), `.champ__erreur` (`--t-s`, `--danger`).

États : `:hover` (trait inchangé, fond `--fond-2`), `:focus-visible` — **rien à
écrire, l'anneau est global** (D11) —, `:disabled` (encre `--texte-faible`,
fond `--fond-1`, `cursor: not-allowed`), et `.champ--erreur .champ__saisie`
(trait `--danger`).

⚠️ **`.champ__erreur` porte le message, `.champ--erreur` porte l'état.** Le
tiret double est le modificateur, le souligné double l'élément : une confusion
ici produit une règle qui ne s'applique jamais, et **aucun contrôle ne verrait
une règle morte**. *(D9 a payé exactement cela sur un `~` qui ne pouvait pas
s'appliquer — `style.css:170-181` en porte encore le récit.)*

- [ ] **Step 2 : le garde qui s'ajoute — G6, les états déclarés**

`primitives.test.ts` gagne une assertion : **pour chaque famille livrée, les
sélecteurs d'état attendus existent**. Pour `champ` : `:disabled`,
`.champ--erreur`, `::placeholder`.

🔴 **Rouge : retirer la règle `.champ__saisie:disabled`**, relancer.
**Attendu : échec nommant l'état manquant.** ⚠️ **Retirer le COMMENTAIRE qui
mentionne `:disabled` ne doit RIEN changer** — le vérifier, c'est la seule
façon de savoir que le garde lit du code et non de la prose (leçon de S1).

- [ ] **Step 3 : faire rétrécir la liste**

```bash
node client/outils/tokens-orphelins.mjs
```
Retirer **exactement** les entrées que la sortie nomme. **Attendu après :
`exit=0`, 18 token(s) en attente** si la prédiction tient.

- [ ] **Step 4 : compte de tests, contrôles, commit**

**Attendu : `Tests  193 passed`** (192 + 1). Puis :

```bash
git add client/src/design/primitives.css client/src/design/primitives.test.ts \
        client/outils/tokens-orphelins.mjs
git commit -m "design(s2): la famille champ, son etat d'erreur, et deux tokens de moins en attente" \
  -- client/src/design/primitives.css client/src/design/primitives.test.ts \
     client/outils/tokens-orphelins.mjs
```

---

### Task 4 : la famille SURFACE — carte et séparateur

**Objet :** la carte que la liste de fenêtres de la page-shell deviendra en S3
(`client/shell.html:12`, `<ul id="fenetres">`), et le séparateur.

**Files:**
- Modify: `client/src/design/primitives.css`, `client/src/design/primitives.test.ts`,
  `client/outils/tokens-orphelins.mjs`

**Tokens prédits :** `--bord`, `--r-3`, `--e-4`, `--t-xl`, `--lh-serre` (**5**).

- [ ] **Step 0 : la porte de 240 lignes** — voir T3, Step 0.

- [ ] **Step 1 : la famille**

`.carte` (fond `--fond-1`, trait `var(--trait) solid var(--bord)`, rayon
`--r-3`, rembourrage `--e-4`), `.carte__titre` (`--t-xl`, `--lh-serre`,
`--texte-fort`), `.carte__corps` (`--texte`), `.separateur`
(`border-block-start: var(--trait) solid var(--bord)`, marge `--e-4`).

🔴 **Le trait de la carte est `--bord`, et c'est une DÉCISION à écrire dans le
code** : une carte **inerte** est un conteneur décoratif, que WCAG 1.4.11
exempte. ⚠️ **Une carte CLIQUABLE porte `--bord-fort`** — son contour devient
alors le contour d'un contrôle. **S2 ne livre pas de carte cliquable** ; c'est
S3 qui en aura besoin pour la liste de fenêtres, et **c'est à lui de poser
`--bord-fort`**. Aucune commande ne le vérifiera : la spec §4.5 le déclare
« règle de revue » et le §8 la range dans les huit jugements.

- [ ] **Step 2 : le garde G6 s'étend à la famille**, avec sa rouge.

- [ ] **Step 3 : faire rétrécir la liste** — **attendu : 13 en attente**.

- [ ] **Step 4 : compte de tests, contrôles, commit**

**Attendu : `Tests  194 passed`** (193 + 1 pour l'extension de G6). Puis :

```bash
git add client/src/design/primitives.css client/src/design/primitives.test.ts \
        client/outils/tokens-orphelins.mjs
git commit -m "design(s2): la famille surface, et le trait decoratif qui n'est pas celui d'un controle" \
  -- client/src/design/primitives.css client/src/design/primitives.test.ts \
     client/outils/tokens-orphelins.mjs
```

---

### Task 5 : la famille MESSAGE — les quatre tons

**Objet :** les quatre tons que la spec §6 confie à S2, et que
`client/connexion.html:44` (`<div id="message" role="status">`) attend.

**Files:**
- Modify: `client/src/design/primitives.css`, `client/src/design/primitives.test.ts`,
  `client/outils/tokens-orphelins.mjs`

**Tokens prédits :** `--texte` (le ton neutre), `--succes`, `--alerte` (**3**).
⚠️ Les deux derniers sont annotés **S3** : divergence **D1**.

- [ ] **Step 0 : la porte de 240 lignes** — voir T3, Step 0.

- [ ] **Step 1 : la famille**

`.message` (fond `--fond-1`, trait `var(--trait) solid var(--bord)`, rayon
`--r-1`, rembourrage `var(--e-2) var(--e-3)`, encre `--texte`), puis
`.message--succes`, `.message--alerte`, `.message--danger` qui ne changent
**que l'encre et le trait**.

🔴 **LE TON NE SE DIT PAS PAR LE FOND, ET C'EST UNE DÉCISION MESURÉE.** Un fond
teinté par `--succes` / `--alerte` / `--danger` serait un fond **que les
50 (désormais 52) paires de §7.1 ne connaissent pas** : le contrôle mesure ces
trois tokens comme des **encres** sur `--fond-0/1/2`
(`contraste.ts:63-71`), jamais comme des fonds. Les employer comme fond
placerait le texte du message sur une couleur **non mesurée**, exactement ce
que le briefing de ce plan interdit.
*Coût* : les quatre tons se distinguent par l'encre et le trait, pas par une
plage colorée. C'est cohérent avec le point 3 du §3 (« la hiérarchie se fait
par la taille et la valeur, jamais par la couleur ») et avec le point 1 (une
couleur sémantique « ne peut apparaître que pour dire un état »).

⚠️ **Les trois tons sémantiques sont mesurés sur `--fond-1`** — le fond du
message — et le pire cas connu de la palette est précisément là :
`--succes` sur `--fond-2` en clair rend **4,59** (spec §4.5). **Le relever à
la recette plutôt que de le supposer.**

- [ ] **Step 2 : le garde G6 s'étend**, avec sa rouge : retirer
  `.message--alerte`, voir l'échec nommer le ton manquant.

- [ ] **Step 3 : faire rétrécir la liste** — **attendu : 10 en attente**.

- [ ] **Step 4 : compte de tests, contrôles, commit**

**Attendu : `Tests  195 passed`** (194 + 1 pour l'extension de G6). Puis :

```bash
git add client/src/design/primitives.css client/src/design/primitives.test.ts \
        client/outils/tokens-orphelins.mjs
git commit -m "design(s2): la famille message, quatre tons dits par l'encre et jamais par le fond" \
  -- client/src/design/primitives.css client/src/design/primitives.test.ts \
     client/outils/tokens-orphelins.mjs
```

---

# Famille ② — l'accessibilité prise, et l'instrument du jugement

### Task 6 : `prefers-reduced-motion` — le legs n°4 de S1, et l'endroit MESURÉ où le poser

**Objet :** neutraliser les transitions pour qui a demandé du mouvement
réduit, **dans `base.css` et non dans `tokens.css`**, et écrire la mesure qui
l'impose.

**Files:**
- Modify: `client/src/design/base.css`, `client/src/design/primitives.test.ts`

**Tokens sortis de la liste : AUCUN.**

- [ ] **Step 1 : rejouer la mesure de D3, pour que la tâche la porte**

```bash
cp client/src/design/tokens.css /tmp/s2-rm.css
cat >> /tmp/s2-rm.css <<'EOF'
@media (prefers-reduced-motion: reduce) { :root { --duree-1: 0.01ms; } }
EOF
node client/outils/blocs-de-theme.mjs --fichier /tmp/s2-rm.css; echo "exit=$?"
rm -f /tmp/s2-rm.css
```
**Attendu : `exit=1`, un QUATRIÈME bloc nommé `media-clair`, et une quinzaine
d'écarts.** ⚠️ **Aucun fichier du dépôt n'est touché** : la mesure se joue sur
une copie.

- [ ] **Step 2 : la règle, dans `base.css`**

```css
@media (prefers-reduced-motion: reduce) {
    *,
    *::before,
    *::after {
        transition-duration: 0.01ms !important;
    }
}
```

⚠️ **Trois choses à écrire dans l'en-tête, et aucune n'est facultative** :
① **pourquoi ce n'est pas dans `tokens.css`** — la sortie du Step 1, recopiée ;
② **ce que cette règle ne couvre pas** — il n'y a aucune `animation` dans
`client/src` aujourd'hui (`grep -rn '@keyframes\|animation:' client/src`, à
lancer et à recopier), et **cette règle ne la couvrirait pas** ;
③ **`!important` est ici l'usage prévu de la propriété**, et non un
contournement de spécificité : la règle doit l'emporter sur toute transition
déclarée ailleurs, y compris dans une feuille qui n'existe pas encore.

⚠️ **`0.01ms` et non `0`** : une durée nulle empêche l'événement
`transitionend` d'être émis, ce que du code futur pourrait attendre. **C'est un
raisonnement repris de l'usage courant, pas une mesure faite ici.**

⚠️ **Ce que cela change dans le produit, dit sans le maquiller** : la seule
transition existante est `#status` (`style.css:79`,
`transition: opacity var(--duree-2)`), et elle cesse — **pour les seuls
utilisateurs qui ont demandé du mouvement réduit**. Ce n'est pas une entorse à
une promesse de S1 : la neutralité que S1 promettait portait sur
`index.html` **à réglages système inchangés**, et elle est close depuis.

- [ ] **Step 3 : le garde, et sa rouge**

`primitives.test.ts` gagne **G7** : `base.css`, **commentaires retirés**,
contient un bloc `@media (prefers-reduced-motion: reduce)` **portant au moins
une déclaration**.

🔴 **Le retrait des commentaires est ici EXIGÉ ET SUFFISANT À FAIRE ÉCHOUER LE
GARDE NAÏF** : l'en-tête du Step 2 nomme `prefers-reduced-motion` **trois
fois**. Un garde qui chercherait la sous-chaîne dans le texte brut serait vert
sur un `base.css` **dont la règle a été retirée**. Le jouer : retirer la
règle **en gardant l'en-tête**, relancer, **voir l'échec** ; c'est cette
rouge-là qui compte, pas celle où l'on retire tout.

- [ ] **Step 4 : la non-régression de `reprise.test.ts`**

⚠️ `reprise.test.ts:87` découpe `base.css` par
`/([^{}]+)\{([^{}]*)\}/g`, **une expression qui ne sait pas imbriquer**.
L'ajout d'un `@media` change ce qu'elle voit. **Attendu : le test reste vert**
— sa cible (`html, body`) est hors de tout `@media` —, et **le vérifier est le
seul contenu de ce Step**.

```bash
cd client && npx vitest run src/design/reprise.test.ts src/design/primitives.test.ts
```

- [ ] **Step 5 : compte de tests, contrôles, commit**

**Attendu : `Tests  196 passed`** (195 + 1 pour G7). ⚠️ **Ce nombre est une
PRÉDICTION en chaîne** — 187 + 5 (T2) + 1 (T3) + 1 (T4) + 1 (T5) + 1 (T6) —
et il est faux dès qu'une famille demande deux assertions au lieu d'une, ou
qu'un voisin ajoute un test. **L'annoncer AVANT de mesurer, et expliquer tout
écart plutôt que le rattraper.**

```bash
git add client/src/design/base.css client/src/design/primitives.test.ts
git commit -m "design(s2): le mouvement reduit, et la mesure qui interdit de le poser dans tokens.css" \
  -- client/src/design/base.css client/src/design/primitives.test.ts
```

---

### Task 7 : `client/primitives.html` — la 5ᵉ entrée Vite, et son exclusion du §7.6

**Objet :** l'instrument du jugement humain sur les primitives — une page qui
les montre toutes, dans les deux thèmes —, **sans toucher `design.html`**.

**Files:**
- Create: `client/primitives.html`, `client/src/design/galerie-primitives.ts`
- Modify: `client/vite.config.ts`, `client/outils/tokens-orphelins.mjs`

**Tokens sortis de la liste : AUCUN** — et c'est **le point de cette tâche**
(voir Step 3).

- [ ] **Step 1 : la page**

Elle lie `/src/design/socle.css` **et** `/src/design/primitives.css`, porte les
trois boutons de thème (`<p id="themes"></p>`), et rend **chaque variante de
chaque famille dans chacun de ses états** — y compris les états que le pointeur
ne peut pas montrer côte à côte : un bouton `disabled` réel, un `.champ--erreur`
réel, les quatre tons de message.

⚠️ **Le `<style>` en ligne ne porte QUE la mise en page de la page de
démonstration**, jamais une primitive — c'est la même clause que
`client/design.html:41-44`. Et il n'y écrit **aucune couleur littérale** : §7.2
balaie cette page comme les autres, dès qu'elle est déclarée à Vite.

⚠️ **Porte armée à 300 lignes** : au-delà, scinder par famille.

`galerie-primitives.ts` est un module d'entrée de quelques lignes : il appelle
`installerSelecteurDeTheme(document.getElementById('themes')!,
document.documentElement, localStorage, () => {})`. **Aucun `getComputedStyle`,
aucune lecture de token** : cette page ne montre pas des valeurs, elle montre
des composants.

- [ ] **Step 2 : la 5ᵉ entrée, et la rouge qui prouve qu'elle est nécessaire**

`client/vite.config.ts`, bloc `rollupOptions.input` : `primitives:
'primitives.html'`.

🔴 **Rouge de l'oubli, à jouer AVANT d'ajouter la ligne** : bâtir sans elle.
```bash
cd client && npm run build >/dev/null 2>&1 && ls dist/primitives.html; echo "exit=$?"
```
**Attendu : `exit=2` ou l'absence du fichier, ET `npm run build` sorti à 0.**
C'est l'angle mort que `vite.config.ts:69-73` documente déjà : « **une page
absente de cette liste ne sort pas du build, et RIEN ne le dit** ». Ajouter la
ligne, rebâtir, **`ls dist/primitives.html` réussit**.

- [ ] **Step 3 : 🔴 l'exclusion du périmètre « employé », et la rouge qui prouve qu'elle porte**

`tokens-orphelins.mjs`, `EXCLUS` : `'client/primitives.html'`, avec **la raison
pour laquelle ce fichier NE PEUT PAS faire échouer le contrôle** — pas la
raison pour laquelle il est gênant (c'est la clause écrite à
`tokens-orphelins.mjs:41-46`).

🔴 **La rouge qui l'éprouve, et sans laquelle l'exclusion serait
décorative** — le drapeau existe déjà, aucun code à modifier :

```bash
node client/outils/tokens-orphelins.mjs --sans-exclusion; echo "exit=$?"
```
**Attendu : `exit=1`**, avec des lignes `À RETIRER DE LA LISTE` nommant des
tokens que **seules les deux galeries** emploient (`--e-5`, `--e-6`, `--t-2xl`,
`--lh-large`…). **Puis la même commande sans le drapeau : `exit=0`.**

🔴 **Lire ce que cette rouge dit exactement** : sans exclusion, les galeries
donneraient un appelant **fictif** à des tokens que le produit n'emploie pas,
et la liste d'attente rétrécirait **sans que rien n'ait été livré**. C'est le
mécanisme précis par lequel ce sous-projet pourrait se mentir à lui-même, et
c'est pourquoi l'exclusion est **portée par une rouge et non par une phrase**.

⚠️ **Vérifier que `primitives.css` reste, elle, DANS le périmètre** : la sortie
du contrôle doit la lister parmi les fichiers balayés. Si elle en sortait, les
dix-huit retraits des tâches T2 à T5 deviendraient faux d'un coup.

- [ ] **Step 4 : les contrôles, sur cinq pages**

```bash
cd client && npm run design:verifier | tail -20
```
**Attendu** : §7.3 « pages bâties : **5** », assertions A et B **vertes**, B
**évaluée sur 5 pages** ; §7.2 **0** couleur littérale ; §7.7 la somme relevée
**sous 12 288** — la noter, c'est le chiffre de la recette.

- [ ] **Step 5 : commit**

```bash
git add client/primitives.html client/src/design/galerie-primitives.ts \
        client/vite.config.ts client/outils/tokens-orphelins.mjs
git commit -m "design(s2): la galerie des primitives, cinquieme entree, et son exclusion eprouvee" \
  -- client/primitives.html client/src/design/galerie-primitives.ts \
     client/vite.config.ts client/outils/tokens-orphelins.mjs
```

---

# Famille ③ — solder la liste

### Task 8 : la liste d'attente — les re-tags motivés, et le compte final

**Objet :** ramener `EN_ATTENTE_D_APPELANT` à son état juste, et **écrire
pourquoi** chaque entrée qui reste, reste.

**Files:**
- Modify: `client/outils/tokens-orphelins.mjs`

- [ ] **Step 1 : le relevé, avant tout**

```bash
node client/outils/tokens-orphelins.mjs
```
**Attendu : `exit=0`**, `48 token(s) déclaré(s)`, et **10 en attente** si les
prédictions de T2 à T5 ont tenu. **Tout écart se relève et s'explique ici, pas
dans le document de résultats.**

- [ ] **Step 2 : les trois re-tags, chacun avec sa raison (D6)**

`--t-xs`, `--e-1` et `--r-plein` sont annotés « S2 » et **aucune des quatre
familles ne les emploie**. Leur annotation devient, en substance :

> `S3 (ou plus tard) — famille ÉTIQUETTE/PASTILLE : le §6 de la spec borne S2
> à quatre familles, « ce dont un écran de connexion a besoin », et un écran de
> connexion n'a ni étiquette ni pastille. **S1 avait prédit S2 ; la prédiction
> était fausse, et fabriquer une primitive pour la vérifier aurait été vider un
> contrôle pour en verdir un autre.**`

🔴 **Ces trois re-tags ne sont vus par AUCUN contrôle**, et c'est le point le
plus faible de S2. L'en-tête de la liste reçoit donc un encadré qui le dit :
*changer l'annotation d'une entrée n'échoue jamais ; c'est une règle de revue,
et toute annotation modifiée doit porter la raison ET le sous-bloc qui l'a
modifiée.*

- [ ] **Step 3 : l'encadré de tête est remis à jour, sans être raboté**

Le commentaire de `tokens-orphelins.mjs:56` dit « **28 tokens** » ; il en dira
**10**. Le relevé qui refuse l'élagage (« sur les 50 paires, 46 citent au moins
un token sans appelant ») est **DATÉ du 19 août 2026 à S1** et devient
**faux** : il y a désormais **52** paires et **10** orphelins.

🔴 **Ne PAS effacer ce relevé : le DATER et le refaire.** Un relevé daté reste
vrai comme histoire ; un présent devient faux. Le refaire :

```bash
cd client && node --input-type=module -e "
import { PAIRES } from './src/design/contraste.ts';
const attente = [/* les noms restants */];
const cite = PAIRES.filter(p => attente.includes(p.encre) || attente.includes(p.fond));
console.log('paires totales :', PAIRES.length, '| citant un token en attente :', cite.length);
"
```
⚠️ **Le nouveau chiffre sera BAS** — les tokens restants sont pour l'essentiel
typographiques et d'espacement, que les paires ne citent pas. **Cela ne réfute
pas la décision de S1** : elle portait sur la palette **entière** au moment où
elle a été prise, et c'est justement parce qu'elle a tenu que la liste a pu
rétrécir. **L'écrire, plutôt que de laisser croire que S1 s'était trompé.**

- [ ] **Step 4 : le contrôle, et la taille du fichier**

```bash
node client/outils/tokens-orphelins.mjs; echo "exit=$?"
wc -l client/outils/tokens-orphelins.mjs
```
**Attendu : `exit=0`**, et un fichier **plus court** qu'à `56b975a` (233).

- [ ] **Step 5 : commit**

```bash
git add client/outils/tokens-orphelins.mjs
git commit -m "design(s2): la liste d'attente tombe de vingt-huit a dix, et trois re-tags motives" \
  -- client/outils/tokens-orphelins.mjs
```

---

# Famille ④ — la recette, et la mémoire

### Task 9 : recette S2 — quatre critères mesurés, deux exécutions chacun, pièces versées

**Objet :** juger S2 sur ce qui se mesure, **et déclarer séparément ce qui ne
se mesure pas**.

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-design-system-s2-resultats.md`,
  `docs/superpowers/plans/journaux-design-s2/`

🔴 **Toute preuve d'une affirmation portée dans `CLAUDE.md` doit être VERSÉE
DANS GIT.** D9 a perdu **six** constats de revue parce que leur preuve vivait
dans `.superpowers/sdd/`, gitignoré et jamais commité ; la tâche 18 de D10 a
établi **par la commande** que le répertoire a disparu, et les six sont
**définitivement perdus**.

- [ ] **Step 1 : les quatre critères MESURÉS, et le cinquième point qui n'en est pas un**

| # | Critère | Comment il est jugé | Exéc. |
| --- | --- | --- | --- |
| ① | **les sept contrôles restent verts**, et §7.1 compte **52** paires, minimum **3,16** | `npm run design:verifier` + `npx vitest run src/design/theme.test.ts` | 2 |
| ② | **la liste d'attente a RÉTRÉCI**, de **28** à **10**, et chaque sortie est due à un appelant écrit dans `primitives.css` | sortie de `tokens-orphelins.mjs` : déclarés / employés / orphelins / en attente | 2 |
| ③ | **aucun sélecteur de `primitives.css` ne peut s'appliquer à `index.html`** | les gardes G1 à G7 de `primitives.test.ts` | 2 |
| ④ | **le poids CSS reste sous le plafond**, et le chiffre est relevé | `poids-css.mjs` — base **3 503**, plafond **12 288** | 2 |
| — | **le jugement visuel sur `primitives.html`** | ⛔ **CE N'EST PAS UN CRITÈRE.** C'est le **huitième** jugement humain du §8, et il est rapporté comme tel | 1 |

⚠️ **Deux exécutions établissent la REPRODUCTIBILITÉ, jamais un taux.** Les
contrôles de ⑥ sont **déterministes** (spec §9), et « combien de fois sur
combien » ne se pose pas ici — **ne pas l'emprunter** à une campagne qui, elle,
l'aurait posée.

🔴 **Le critère ③ mérite d'être lu deux fois : il ne dit PAS « S2 ne change
rien à la fenêtre de session ».** `tokens.css` a gagné un token, donc
`socle-*.css` **change d'octets** et `index.html` le charge. Ce que ③ établit,
c'est qu'**aucune règle de `primitives.css` ne peut sélectionner un élément
d'`index.html`** — ce qui est la seule chose qui compte pour le rendu, et la
seule chose qu'une commande sache dire. **Le formuler autrement serait
affirmer au-delà du relevé.**

- [ ] **Step 2 : DEUX exécutions de chaque, journalisées**

```bash
mkdir -p docs/superpowers/plans/journaux-design-s2
J=docs/superpowers/plans/journaux-design-s2
git rev-parse --short HEAD > $J/commit.txt
git status --porcelain >> $J/commit.txt
for i in 1 2; do
  (cd client && npm run build)                              > $J/build-$i.log 2>&1
  (cd client && npm run design:verifier)                    > $J/design-verifier-$i.log 2>&1
  (cd client && npx vitest run)                             > $J/vitest-client-$i.log 2>&1
  (cd client && npm run typecheck)                          > $J/typecheck-$i.log 2>&1
  (cd proto  && npx vitest run)                             > $J/vitest-proto-$i.log 2>&1
  node client/outils/tokens-orphelins.mjs                   > $J/orphelins-$i.log 2>&1
  node client/outils/contraste.mjs                          > $J/contraste-$i.log 2>&1
  (cd client && npx vitest run src/design/theme.test.ts)    > $J/theme-7-5-$i.log 2>&1
  (cd client && npx vitest run src/design/primitives.test.ts) > $J/primitives-$i.log 2>&1
done
node client/outils/tokens-orphelins.mjs --sans-exclusion > $J/orphelins-sans-exclusion.log 2>&1
find client/dist/assets -name '*.css' -printf '%s\t%p\n'  > $J/poids-css.txt
./scripts/verify-all.sh > $J/verify-all.log 2>&1; echo "verify-all exit=$?" >> $J/verify-all.log
```

⚠️ **`verify-all.sh` peut échouer à une étape ÉTRANGÈRE** — `cargo test
--workspace` sur un `agent/` que trois chantiers modifient, ou
`plateforme : npm run test:postgres` si l'instance est absente. **Si cela
arrive : le déclarer, relever `git status`, et juger S2 sur les étapes `client`
et `proto`** — jamais masquer, jamais imputer. **C'est arrivé à P2**, dont le
témoin est tombé « sur un test Rust du voisin ».

- [ ] **Step 3 : la famille de lecture des journaux, MESURÉE et non supposée**

```bash
grep -lP '\x1b\[' docs/superpowers/plans/journaux-design-s2/* ; echo "ansi exit=$?"
file docs/superpowers/plans/journaux-design-s2/* | sed 's/.*: //' | sort -u
grep -lc $'\r' docs/superpowers/plans/journaux-design-s2/* 2>/dev/null
```
**Attendu, d'après S1** : liste vide pour les séquences ANSI, « UTF-8 » ou
« ASCII » partout, aucun `\r`. ⚠️ **Le vérifier et DIRE le résultat**, pas le
supposer : ce sont des sorties `npm`/`node` sur l'hôte, jamais du PowerShell
distant.

- [ ] **Step 4 : le jugement humain, rapporté comme tel**

Ouvrir `dist/primitives.html` dans un Chromium de l'hôte, **dans les deux
thèmes** (les trois boutons), et regarder les quatre familles.

⛔ **CE N'EST PAS UN CRITÈRE, et le document de résultats doit le dire à
l'endroit même où il le rapporte.** Ce qu'aucun regard ne dira : que la
direction soit « sobre » et « pro », que `#93b4f9` soit le bon survol, que le
ratio 1,2 soit le bon, que 14 px soit assez dense, que le pas de 4 px soit le
bon, que `--bord` ait été employé là où il fallait, que le plafond de 12 Kio
soit au bon endroit, que la galerie montre ce qu'il faut regarder. **Ce sont
les huit jugements humains du §8, et aucun ne deviendra une mesure.**
**Si le jugement n'est pas porté, il est déclaré non porté** — un
« probablement » vaut moins qu'un « non mesuré ».

- [ ] **Step 5 : écrire le document de résultats**

`docs/superpowers/plans/2026-08-19-design-system-s2-resultats.md`, sur le
modèle de celui de S1. Il porte **au minimum** :

1. le verdict de chaque critère **avec son nombre d'exécutions** ;
2. **chaque ROUGE jouée, avec son message d'échec verbatim** — et pour les
   gardes à plusieurs assertions, **la preuve que chaque rouge n'a fait tomber
   QUE l'assertion visée** ;
3. **le tableau de la liste d'attente : 28 → 10**, entrée par entrée, avec la
   tâche qui l'a retirée et **les trois re-tags avec leur raison** ;
4. les **onze divergences** D1…D11 de ce plan, avec leur sort ;
5. les tailles de fichiers **relevées par la commande**, avec `HEAD` ;
6. la somme CSS relevée, opposée à la base **3 503** et au plafond **12 288** ;
7. un renvoi nommé vers chaque journal versé ;
8. le § « Ce que S2 n'établit PAS », **sans le raboter** :

- **Aucun taux.** Deux exécutions par critère, sur des contrôles
  **déterministes** : reproductibilité, rien de plus.
- 🔴 **Aucun jugement visuel n'est une mesure**, et les **huit** jugements
  humains du §8 restent entiers — augmentés de **trois** que S2 ajoute : que
  `--accent-survol` soit le bon survol, que l'état actif dit « par le retour au
  repos » soit lisible, et que quatre tons de message distingués par la seule
  encre suffisent.
- ⛔ **AUCUNE SURFACE DU PRODUIT N'EMPLOIE UNE PRIMITIVE.** À la fin de S2,
  `primitives.css` n'est liée que par `primitives.html`, qui n'est pas du
  produit. Les dix-huit tokens sortis de la liste ont **un appelant écrit**,
  pas **un pixel rendu** — c'est S3 qui referme cet écart (D7).
- **Rien n'est éprouvé hors d'un Chromium de bureau** : ni Firefox, ni Safari,
  ni mobile. **Rien du HiDPI.**
- **L'accessibilité au-delà du contraste et du mouvement réduit** : navigation
  clavier complète, lecteurs d'écran, cibles tactiles minimales, ordre de
  tabulation. **Le contraste est mesuré ; le reste ne l'est pas.**
- **Aucune primitive ne porte de rôle ARIA** : les primitives sont du CSS, et
  la sémantique reste au balisage que S3 écrira.
- **Aucun contrôle ne mesure une longueur** : G4 vérifie qu'une longueur passe
  par un token, **jamais que le bon token a été choisi**.
- **Les trois longueurs hors échelle de `style.css` restent**, et la clause
  « aucune longueur hors échelle » du §8 **reste fausse** jusqu'à S4.
- **`--police-mono` n'est toujours pas câblé** : S4 tranche (D8).
- **Aucune internationalisation** : rien ne dit qu'un bouton survit à un
  libellé plus long.
- **Le legacy n'est pas touché**, et aucun contrôle ne le balaie.
- ⚠️ **Ni `primitives.html` ni `galerie-primitives.ts` n'ont de test**, comme
  `design.html` et `galerie.ts` : **une galerie qui cesserait de rendre une
  famille entière ne serait attrapée par aucun contrôle**, seulement par l'œil.

- [ ] **Step 6 : commit**

```bash
git add docs/superpowers/plans/2026-08-19-design-system-s2-resultats.md \
        docs/superpowers/plans/journaux-design-s2
git commit -m "recette(s2): quatre criteres, deux executions chacun, et la liste tombee a dix" \
  -- docs/superpowers/plans/2026-08-19-design-system-s2-resultats.md \
     docs/superpowers/plans/journaux-design-s2
```

---

### Task 10 : revue transverse de fin de branche, et `CLAUDE.md`

**Objet :** chercher les affirmations — commentaires, documents, en-têtes —
devenues **FAUSSES dans la branche elle-même**, puis écrire la section S2 de
`CLAUDE.md`.

**Files:**
- Modify: `CLAUDE.md`, et tout fichier dont un commentaire est devenu faux

⚠️ **Cette tâche n'est jamais facultative.** La revue transverse a trouvé
**cinq** défauts en D7, **trois** Critiques en D8, **six** en D9, **douze** en
D10, **sept** en D11, **huit** en P1, **dix** en P2, **cinq** en S1 et **neuf**
sur le chantier E. Ils ont tous la même forme : **corrects des deux côtés pris
séparément**, faux ensemble. **Une revue par tâche ne peut structurellement pas
les voir.**

- [ ] **Step 1 : la cible propre de cette revue — S2 produit trois classes d'énoncés faux**

**Classe A — les phrases que le token neuf rend fausses.** T2 en corrige trois
(D2). **Les chercher toutes, pas seulement celles que ce plan a nommées** :

```bash
grep -rn 'treize\|13 couleurs\|47 token\|28 orphelin\|50 paire\|cinquante paire' \
  client CLAUDE.md docs/superpowers/specs docs/superpowers/plans \
  --include="*.ts" --include="*.css" --include="*.html" --include="*.mjs" --include="*.md" \
  | grep -v node_modules | grep -v '/dist/'
```

🔴 **« Corrigé à sa place » est une affirmation de COMPLÉTUDE, et elle se
vérifie en ÉNUMÉRANT les places AVANT d'écrire.** Le naufrage du « 487 » s'est
rejoué **neuf fois** dans ce dépôt, dont deux fois **à l'intérieur de la ronde
qui le dénonçait**. Le geste qui manquait tient en une commande : `grep -n`,
lancé **avant** l'édition **et relu place par place après**.

**Classe B — les phrases que les primitives rendent fausses.** Candidats connus
d'avance, chacun à relire :

| Fichier:ligne | Ce qu'il dit aujourd'hui | Ce que S2 en fait |
| --- | --- | --- |
| `client/connexion.html:10-25` | « la page ne porte AUCUNE primitive […] c'est **S3** qui les pose » | **reste vrai** — S2 les définit, il ne les applique pas. **À ne PAS toucher** |
| `client/design.html:41-44` | « Ces règles sont la mise en page […] pas une primitive — **S2 est le sous-bloc qui en pose** » | ⚠️ **au FUTUR, et il a eu lieu** : à reformuler en relevé daté |
| `client/design.html:36-37` | « PORTE ARMÉE À 300 […] scinder par famille, **au même rythme que les primitives de S2** » | ⚠️ **la scission a eu lieu, et autrement** que par un découpage de cette page : `primitives.html` est née à côté |
| `client/src/design/galerie.ts:150-154` | « le PREMIER ET SEUL appelant de `choisir()` du sous-bloc S1 » | ⚠️ **il ne l'appelle plus** — T1 l'a extrait |
| `client/src/design/base.css:63-69` | « Le **SEUL** ajout d'apparence de ce sous-bloc » | **reste vrai** — c'est de **S1** qu'il parle, au passé ; **le vérifier plutôt que le supposer** |
| `client/outils/tokens-orphelins.mjs:56-89` | « **28 tokens** », et le relevé « 46 des 50 paires » | ⚠️ **T8 le date et le refait** — vérifier qu'il l'a fait |
| `client/src/design/tokens.css:94-104` | l'encadré `--police-mono` | **reste vrai** — S2 ne le câble pas (D8). Vérifier qu'il ne dit rien de S2 |

**Classe C — le patron que D10 a payé sept fois** : *une affirmation écrite par
une tâche et réfutée par une autre tâche de la MÊME branche.* Candidat le plus
probable ici : **un en-tête de T2 qui décrirait la famille bouton comme « la
seule famille de `primitives.css` »**, ou un commentaire de T2 sur les états
qui serait réfuté par T6 (le mouvement réduit change ce que « transition »
signifie). **Relire les en-têtes de T2 après T6, pas avant.**

- [ ] **Step 2 : le relevé de tailles, PAR LA COMMANDE, APRÈS la dernière édition**

🔴 **Une table mesurée en début de ronde est fausse à la fin de la même ronde** —
D8 a commis cette erreur en croyant bien faire.
🔴 **ET IL FAUT DIRE À QUEL COMMIT** : l'arbre est partagé par cinq chantiers.

```bash
git rev-parse --short HEAD
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | grep '^client/' | xargs wc -l 2>/dev/null | sort -rn | head -16
```

⚠️ **Mesurer CHAQUE ligne de la table au moment de l'écrire, jamais la
relire.** D11 a écrit **cinq** chiffres sans les mesurer, et **les cinq étaient
faux**. **Toucher une ligne d'un tableau de comptes oblige à remesurer son
compte**, même quand ce n'est pas l'objet de l'édition.
⚠️ **Et si un fichier de plus de 500 lignes apparaît qui n'est pas de S2, le
dire sans se l'attribuer** — `client/src/main.ts` était à 392 en D10 et à 451
après le chantier E.
⚠️ **`client/verify-webrtc.mjs` doit valoir 494** : s'il a bougé, ce n'est pas
S2, **et le dire**.

- [ ] **Step 3 : écrire la section `CLAUDE.md`**

Une section `## 🎨 Sous-projet ⑥ Design system — sous-bloc S2 : les primitives
(19 août 2026)`, placée **après** la section S1, portant :

1. les renvois — plan, spec (`5b6b830`), résultats, journaux, **avec leur
   famille de lecture MESURÉE** (T9 Step 3) ;
2. ⛔ **« Aucune tâche de S2 n'a employé la VM Windows »**, et pourquoi ;
3. **les quatre critères** avec leur verdict et leur nombre d'exécutions, **et
   le jugement visuel déclaré HORS CRITÈRE** ;
4. 🔴 **le fait le plus réutilisable : la liste d'attente RÉTRÉCIT parce qu'un
   contrôle la force**, 28 → 10 en six commits, chaque retrait **lu dans la
   sortie du contrôle et jamais deviné** ;
5. 🔴 **les deux mesures qui ont tranché une conception** — `color-mix(…,
   black)` refusé par §7.2, et un bloc `@media` de plus dans `tokens.css` qui
   fait rendre **quinze écarts** à §7.4 —, avec leur sortie verbatim ;
6. **les onze divergences D1…D11** en une phrase chacune, dont **D5** : le
   compte d'étapes de `verify-all.sh` est **dix**, une exécution en affiche
   **dix-sept**, et **ni l'un ni l'autre ne se suffit sans dire lequel on
   compte** ;
7. les **variables d'environnement : AUCUNE** — et le dire, parce qu'une
   absence se déclare ;
8. les **pièges neufs**, dont : un garde de forme est un test d'**absence**,
   donc vert sur un fichier vide, d'où G5 ; un garde qui cherche une
   sous-chaîne est satisfait par son propre commentaire, d'où le blanchiment ;
   `tokens.ts` ne sait nommer que trois blocs, donc `tokens.css` ne peut
   accueillir aucune autre requête média ; une page absente de `vite.config.ts`
   ne sort pas du build **et rien ne le dit** ;
9. **ce que S2 n'établit PAS**, repris du document de résultats **sans le
   raboter**, dont les **huit** jugements humains du §8 **et les trois que S2
   ajoute** ;
10. **le relevé de tailles du Step 2**, avec son `HEAD` ;
11. **les legs**, dont : ⛔ aucune surface du produit n'emploie une primitive
    (S3) ; ⛔ `--police-mono` (S4) ; ⛔ les trois longueurs hors échelle (S4) ;
    ⛔ les **dix** entrées restantes de la liste d'attente, avec leur sous-bloc ;
    ⛔ les **trois re-tags que rien ne contrôle** ; ⛔ le plafond de 12 Kio
    toujours non calibré ; ⛔ `tokens.ts` incapable de nommer un quatrième bloc.

- [ ] **Step 4 : commit**

```bash
git add CLAUDE.md   # plus tout fichier dont un commentaire a été corrigé, NOMMÉ
git commit -m "docs(s2): la section S2, et la revue transverse de fin de branche" -- CLAUDE.md
```

---

## Ordre et dépendances

```
  T1 selecteur-theme (extraction)
      │
      v
  T2 primitives.css + BOUTON + --accent-survol + G1..G5   [§7.1 : 50 -> 52]
      │
      ├──> T3 CHAMP     ──┐
      │                   │   (séquentielles : même fichier,
      ├──> T4 SURFACE   ──┤    même liste d'attente, même test)
      │                   │
      └──> T5 MESSAGE   ──┘
                          │
                          v
                     T6 prefers-reduced-motion (base.css)
                          │
                          v
                     T7 primitives.html + 5e entree + EXCLUS
                          │
                          v
                     T8 la liste d'attente soldee  [28 -> 10]
                          │
                          v
                     T9 recette ──> T10 revue transverse + CLAUDE.md
```

**Ce qui se parallélise : RIEN, ou presque.** C'est une propriété de ce
sous-bloc, et il vaut mieux la dire que la découvrir :

- **T3, T4 et T5 touchent les MÊMES TROIS FICHIERS** — `primitives.css`,
  `primitives.test.ts` et la liste d'attente de `tokens-orphelins.mjs`. Les
  paralléliser produirait trois conflits garantis, et surtout **trois retraits
  concurrents sur la même liste**, dont aucun ne verrait ce que les autres ont
  retiré. **Séquentielles, sans exception.**
- **T1 et T2 touchent tous deux `galerie.ts`** (l'un `boutons()`, l'autre
  `COULEURS`). **T1 avant T2.**
- **T6 peut se faire à tout moment après T2** (il ne touche que `base.css` et
  ajoute un test), mais il est placé **après T5** pour que la revue transverse
  relise les en-têtes des familles **après** que le mouvement réduit a changé ce
  que « transition » signifie (T10, classe C).
- **T7 après T5** : la galerie ne peut pas montrer des familles qui n'existent
  pas.
- **T8 après T7** : c'est le premier moment où la liste est dans son état final.
- **T9 après T8**, **T10 après T9** — et T10 relève ses tailles **après sa
  propre dernière édition**.

⚠️ **Trois tâches touchent des fichiers que les chantiers voisins modifient** :
T2 (`client/src/style.css`, une ligne de commentaire — le chantier E y a ajouté
`#micro`), T7 (`client/vite.config.ts`) et T10 (`CLAUDE.md`, que le sous-bloc
P3 de la plateforme édite en ce moment). **Chacune relit le fichier à
l'instant de l'écrire**, et ne se fie pas à l'inventaire de ce document.

---

## Risques

| Risque | Mitigation, ou déclaration |
| --- | --- |
| **L'arbre est partagé par cinq chantiers**, dont un qui travaille dans `client/src/` | Chaque relevé porte son commit ; **jamais `git add -A`**, jamais `--amend` ; T10 relève les tailles après sa dernière édition et **attribue au voisin ce qui est du voisin** |
| 🔴 **Les gardes G1 à G4 sont des tests d'ABSENCE**, verts sur un fichier vide | **G5** est le garde d'atteignabilité, et il est joué rouge en vidant le fichier de ses règles. **Sans G5, quatre gardes sur cinq ne prouvent rien** |
| 🔴 **Un garde satisfait par son propre commentaire** — payé deux fois en S1 | Tout garde blanchit les commentaires **avant** de chercher, et sa rouge se joue en retirant **la règle** en gardant le commentaire |
| **`--accent-survol` est un token de plus dans une palette que la spec fixe à treize** | Divergence D2, tranchée avec ses trois voies écartées dont **une mesurée**. Le contraste est mesuré (52 paires) ; **la teinte est un jugement humain**, et elle rejoint le §8 |
| **La neutralité d'`index.html` repose sur « aucun sélecteur d'élément nu »** | Garde **G1**, joué rouge sur `button { … }`. ⚠️ **Si G1 tombait par une réécriture du test, deux `<button>` de la fenêtre de session changeraient d'apparence sans qu'aucun autre contrôle ne le dise** |
| **Le poids CSS dérive** | §7.7, plafond **12 288**, base **3 503**. Estimation : les feuilles bâties de S1 pèsent ~7,2 octets par ligne source après minification, donc ~250 lignes de primitives ≈ **2 000 octets**. **Estimation, pas mesure** : la recette relève le chiffre réel |
| **`primitives.css` franchit la porte** | Porte à **240**, pas à 300, et **chaque tâche de famille la mesure AVANT d'écrire**. Le point de chute est nommé : `design/primitives/{bouton,champ,surface,message}.css`. **Extraire, jamais compresser** — S1 a compressé deux fois et la revue a exigé l'extraction les deux fois |
| **La liste d'attente rétrécit sans que rien n'ait été livré** | C'est ce que l'**exclusion** de `primitives.html` empêche, et T7 Step 3 le prouve par une rouge — `--sans-exclusion` fait apparaître des `À RETIRER DE LA LISTE` que seules les galeries justifieraient |
| 🔴 **Les trois re-tags de T8 ne sont vus par aucun contrôle** | Déclaré comme **le point le plus faible de S2**, dans le code et dans `CLAUDE.md`. Aucune mitigation technique n'est proposée : ce serait un contrôle de plus sur des chaînes de prose, et il ne pourrait pas échouer utilement |
| **`verify-all.sh` tombe sur une étape étrangère** | Le déclarer, relever `git status`, juger S2 sur `client` et `proto`. **C'est arrivé à P2** |
| **Le jugement visuel est pris pour une mesure** | Il est déclaré **hors critère** dans la recette, dans le document de résultats et dans `CLAUDE.md`, aux trois endroits où il est rapporté. **Les huit jugements du §8 deviennent onze**, et aucun ne se transformera en mesure |
| **`prefers-reduced-motion` posé au mauvais endroit** | Divergence D3, **mesurée** : la voie évidente fait rendre **quinze écarts** à §7.4. La mesure est rejouée par T6 Step 1 |
