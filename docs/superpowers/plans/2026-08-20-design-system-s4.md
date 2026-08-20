# Sous-bloc S4 — la fenêtre de session : l'écran terminal, et la fin de ⑥

**Date** : 20 août 2026
**Spécification** : `docs/superpowers/specs/2026-08-19-design-system-design.md`
(commit `5b6b830`), §5.2 (le périmètre de la fenêtre de session), §6 (le
découpage, ligne S4), §7 (les contrôles), §8 (les jugements humains), §10 (les
tailles).
**Sous-blocs précédents, tous clos** :
`2026-08-19-design-system-s1.md` / `-s1-resultats.md`,
`-s2.md` / `-s2-resultats.md`, `-s3.md` / `-s3-resultats.md`, plus leurs trois
sections de `CLAUDE.md` (§🎨 S1, §🎨 S2, §🎨 S3). **Ils font autorité sur l'état
réel**, et ce plan les prend comme relevé plutôt que comme intention.
**Statut** : plan, à exécuter.
**Portée** : la fenêtre de session (`client/index.html`, `client/src/style.css`
et les modules qui l'écrivent), et les trois legs de ⑥ que seul S4 a le droit de
solder.

⛔ **S4 EST LE DERNIER SOUS-BLOC DE ⑥.** Tout ce qu'il ne solde pas reste ouvert
sans sous-bloc assigné — le §14 en dresse la liste, et c'est le seul endroit du
sous-projet où elle sera complète.

⛔ **AUCUNE TÂCHE DE S4 N'EMPLOIE LA VM WINDOWS.** ⑥ est un sous-projet
navigateur (spec §9), et la VM est occupée par la recette du sous-bloc G1.

---

## 0. Comment ce document a été écrit

Il n'a **modifié aucun fichier de code**. Chaque affirmation sur l'existant porte
son `fichier:ligne`, **relu après avoir été écrit**. Les sorties de commande
citées au §2 ont été obtenues en les **lançant**, jamais reconstituées ; la seule
sonde neuve (§2.5) a tourné depuis un script jetable du répertoire de travail,
**aucun fichier du dépôt n'a été touché** — c'est la forme que S2 avait employée
pour ses mesures de conception (`/tmp/s2probe`).

⚠️ **Un plan est une source de contrôles vacueux, et ce sous-projet en a la
preuve deux fois** : celui de S1 annonçait neuf occurrences là où il y en avait
onze **et attribuait l'écart à une cause fausse** ; celui de S2 a prescrit une
rouge **impossible selon le bloc choisi**. Chaque contrôle prescrit ci-dessous
porte donc, en plus de son énoncé, **l'état exact qui le rend rouge** et
**la raison pour laquelle cet état est atteignable**. Quand cet état a été
mesuré, le chiffre est donné ; quand il ne l'a pas été, la phrase le dit.

---

## 1. Ce que la spec confie à S4, et les trois questions qu'elle lui laisse

La ligne S4 du §6 de la spec livre trois choses :

1. **l'écran plein cadre des états terminaux** (§5.2, famille 1), « en
   réemployant la distinction que `status.ts` porte déjà » ;
2. **la reprise des trois bandeaux sur les primitives « message »** ;
3. **la mise en page conditionnelle au Window Controls Overlay**, déclarée
   « livrée sans pouvoir être exercée » (§9).

Et trois sous-blocs lui ont laissé, chacun en le nommant, ce qu'ils ne
pouvaient pas décider :

| Legs | Qui le pose | Où il vit aujourd'hui |
| --- | --- | --- |
| **`--police-mono` : câbler ou retirer** | spec §4.3, legs n°1 de S1, n°5 de S2, n°1 de S3 | `client/src/design/tokens.css:130`, `client/outils/tokens-orphelins/attente.mjs:208-224` |
| **les longueurs hors échelle**, et la clause §8 qu'elles rendent fausse | S1 (D2), S2 (§⑩), S3 (§11 legs n°2) | `client/src/style.css:12-34`, spec §8 |
| **le raccordement sémantique du micro** | S1, jamais repris depuis | `client/src/design/tokens.css:91-96` |

**Ce plan tranche les trois**, et il ajoute ce que la lecture du code a trouvé et
qu'aucun document ne portait : **un défaut de contraste réel dans le thème
clair de la fenêtre de session** (§4).

---

## 2. L'état des lieux, RELEVÉ le 20 août 2026

### 2.1 Les huit contrôles sont verts, et voici leurs chiffres

`npm --prefix client run design:verifier`, sortie 0 :

```
═══ 7/7 contrôle(s) vert(s) ═══
  §7.5 (la bascule de thème) est un test unitaire : il tourne dans `npm test`.
```

Les chiffres qu'il imprime, et qui sont la ligne de base de S4 :

| Contrôle | Relevé |
| --- | --- |
| §7.2 | `fichiers balayés : 64` / `couleurs littérales : 0` |
| §7.6 | `48 token(s) déclaré(s)` / `13 fichier(s)` / `47 token(s) employé(s)` / `1 orphelin(s), dont 1 en attente déclarée` / `total : 0 écart(s)` |
| §7.9 | `déclarées : 52` / `employées : 51` / `familles : bouton, champ, message, surface` / `total : 0 écart(s)` |
| §7.3 | `pages bâties : 5` / A `0 échec(s)` / B `0 échec(s) (évaluée sur 5 page(s))` |
| §7.7 | `546 + 1493 + 2730 + 1063 + 2179` = `somme : 8011 octets` / `plafond : 12288` / `marge : 4277` |

`npm --prefix client test` → **`Test Files 26 passed (26)` / `Tests 258 passed
(258)`**. `npm --prefix client run typecheck` → sortie 0, aucune ligne.

**Ces cinq lignes sont identiques, chiffre pour chiffre, à celles que le document
de résultats de S3 publie.** Rien n'a bougé dans `client/` depuis sa clôture.

### 2.2 Le §7.9 relève que la fenêtre de session n'emploie AUCUNE primitive

```
② A — les primitives atteignent le PRODUIT :
  client/index.html : aucune famille
  client/shell.html : bouton, message, surface
  client/connexion.html : bouton, champ, message, surface
```

**C'est voulu par S3**, dont le critère ③ mesurait que la fenêtre de session
n'avait pas bougé d'un octet. **S4 est le sous-bloc qui referme cette ligne**, et
c'est la seule des trois surfaces du produit à ne porter aucune classe.

### 2.3 La fenêtre de session porte CINQ éléments, pas quatre

`client/index.html:15-23` : `#remote` (l. 15), `#status` (l. 16), `#stats`
(l. 17), `#micro` (l. 22, `hidden` par défaut, chantier E), `#fullscreen`
(l. 23). **La spec §5.2 en compte quatre** — le cinquième est arrivé avec le
chantier microphone, et la divergence n°7 de S1 l'a déjà relevée.

Les deux écrivains d'un état **terminal**, et ils sont exactement deux :

- `client/src/main.ts:159` — un `statut.afficher(…)` portant `{ terminal: true }`, dont le
  message est « session terminée : » suivi du motif ;
- `client/src/main.ts:448` — un `statut.afficher(…)` portant `{ terminal: true }`, dont le
  message est « échec : » suivi de l'erreur.

La distinction vit déjà dans le modèle : `client/src/status.ts:29`
(`terminal?: boolean`), gardée par **onze** tests de `client/src/status.test.ts`.

### 2.4 Aucun manifeste PWA n'existe dans ce dépôt

```
$ grep -rn 'titlebar-area\|windowControlsOverlay\|display_override\|manifest' \
    client/src client/*.html client/vite.config.ts
client/src/viewport.test.ts:22: … dont le défaut se manifesterait bien plus
client/vite.config.ts:36: … se manifeste comme un ÉCHEC DE BUILD
```

**Deux faux positifs de français, et rien d'autre.** Le même balayage sur
`plateforme/src` ne rend que trois occurrences du verbe « se manifester ». Le
manifeste est planifié par le sous-projet ④ — `docs/superpowers/specs/2026-08-19-gestion-apps-design.md:670`
et `:760-774`, sous-bloc **G5**, « La PWA par application » — et **G5 n'est pas
implémenté**. *(La spec de ⑥ l'attribue à ② ; le cadrage §5 ② l'y range bien
— `2026-07-27-refonte-produit-design.md:120-121` — et ④ le reprend en G5. Les
deux sont vrais ; ce qui importe ici est qu'aucun des deux n'a livré.)*

### 2.5 🔵 La sonde des longueurs : HUIT occurrences pour SIX valeurs

Sonde de plan, script jetable, **aucun fichier du dépôt touché**. Elle applique
la règle de compte de S3 — commentaires retirés, `var(--…)` neutralisé, les
remplissages de fenêtre exclus — et y ajoute l'exclusion du **zéro** (§3.2) :

```
client/src/style.css      padding: 6px var(--e-3)        → « 6px »
client/src/style.css      padding: 6px var(--e-3)        → « 6px »
client/src/style.css      letter-spacing: 0.02em         → « 0.02em »
client/src/style.css      font-size: 18px                → « 18px »
client/src/style.css      font-size: 18px                → « 18px »
client/src/shell.css      max-inline-size: 72rem         → « 72rem »
client/src/shell.css      grid-template-columns: … 18rem → « 18rem »
client/src/connexion.css  max-inline-size: 26rem         → « 26rem »
total hors token, hors remplissage, hors zéro : 8
```

🔴 **HUIT OCCURRENCES POUR SIX VALEURS, ET LES DEUX NOMBRES SONT VRAIS DE CHOSES
DIFFÉRENTES.** S3 a publié **six** en comptant des **valeurs distinctes**
(`journaux-design-s3/longueurs-hors-echelle.log`, dont la règle de compte est
énoncée avant le compte) ; un contrôle, lui, compte des **occurrences**, parce
qu'il ne peut pas dédupliquer sans décider que deux `6px` sont le même. **C'est
la divergence D5 de S2 rejouée sur une autre grandeur** — « ni dix ni dix-sept ne
se suffit sans dire lequel on compte ». **Les deux nombres devront figurer dans
le document de résultats**, et le contrôle du §3.2 imprimera les deux.

### 2.6 Les tailles, par la commande

Dépôt entier, fichiers de plus de 500 lignes : **DEUX**, la dette gelée —
`agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630**. **Aucun
fichier de `client/`.**

Les plus gros de `client/`, relevés le 20 août 2026 :
`client/verify-webrtc.mjs` **494** (marge **6**, intouché depuis S1),
`client/src/main.ts` **451**, `client/src/webrtc.session.test.ts` **419**,
`client/src/micro.test.ts` **411**, `client/src/webrtc.ts` **360**,
`client/src/fullscreen.test.ts` **327**, `client/src/design/primitives.test.ts`
**283** (porte **300**, marge **17**), `client/src/style.css` **184**,
`client/outils/tokens-orphelins/attente.mjs` **227** (seuil d'extraction
conditionnel **240**, marge **13**).

⚠️ **`client/src/main.ts` est à 451, au-dessus de la porte de 300 que ⑥ se donne
(spec §10), et il n'est PAS de ⑥** : il valait 392 au relevé D10, le chantier E
l'a porté là. S4 doit y écrire quelques lignes de câblage (tâche 9) : **la
tâche mesure avant et après, et si son addition dépasse dix lignes, elle
extrait** au lieu de grossir un fichier déjà au-dessus de la porte.

---

## 3. Les trois décisions que S4 doit prendre, et leur raison

### 3.1 🔴 `--police-mono` est CÂBLÉ sur `#stats`, et la liste d'attente devient VIDE

**Décision : câbler, pas retirer.** Quatre raisons, dans l'ordre où elles pèsent :

1. **La spec le désigne, et son appelant existe.** Le §4.3 nomme `#stats` comme
   son unique appelant prévu, et donne la raison : le bandeau « affiche des
   nombres qui changent » et « porte déjà `font-variant-numeric: tabular-nums`
   pour la même raison ». Ce n'est pas un token cherchant un emploi ; c'est un
   emploi qui attendait le droit de changer une apparence.
2. **Le seul obstacle était un interdit qui tombe avec S4.** `attente.mjs:213-218`
   l'écrit : « le câbler CHANGERAIT l'apparence de `#stats` … S1 s'interdit
   nommément de changer l'apparence d'`index.html` ». **S4 est le sous-bloc qui a
   le droit de la changer** — c'est la définition même de sa ligne au §6.
3. **Le retirer coûterait plus qu'il ne rend.** Ses deux appelants de galerie
   (`client/design.html:102` et `:135`) partiraient avec lui, et la galerie
   cesserait de montrer la seconde des deux piles que la spec §4.3 déclare. Le
   design system perdrait sa pile monospace **entière** pour vider une ligne.
4. **Il fait disparaître une des trois longueurs hors échelle**, et pour une
   raison de fond, pas de comptage — voir le §3.2, point ②.

**Conséquence mesurable, et c'est le critère ② de la recette** : la liste
d'attente passe de **une** entrée à **zéro**. `client/outils/tokens-orphelins.mjs`
imprimera `0 orphelin(s), dont 0 en attente déclarée`.

⚠️ **La liste VIDE ne disparaît PAS, et l'énoncé de S1 qui le promettait est
corrigé plutôt qu'exécuté.** S1 écrit « le jour où elle est vide, elle
disparaît ». **Supprimer `attente.mjs` supprimerait l'égalité elle-même** : c'est
elle qui fait rougir `NOUVEL ORPHELIN` pour tout token futur déclaré sans
appelant, et S4 en ajoute quatre (§3.3, §4). Le fichier reste, sa `Map` est vide,
et **la doctrine part avec sa donnée** — c'est la règle d'extraction que ce
fichier porte déjà. **Il doit donc MAIGRIR** ; la tâche 6 mesure et publie.

### 3.2 Les longueurs hors échelle : S4 fait tomber les SIX, et la clause devient un CONTRÔLE

**C'est la décision la plus lourde du sous-bloc, et elle a un coût qui est
écrit.**

**Ce que S4 fait des trois de `client/src/style.css`** — celles que S1, S2 et S3
lui assignent nommément :

| Valeur | Où | Ce que S4 en fait | Par quelle tâche |
| --- | --- | --- | --- |
| `6px` (×2) | `style.css:78` (`#status`), `:94` (`#stats`) | **disparaît** — les deux bandeaux prennent `.message`, dont le remplissage est `var(--e-2) var(--e-3)` (`primitives/message.css:54`) | T5 |
| `0.02em` | `style.css:99` (`#stats`) | **part avec le câblage de `--police-mono`** | T6 |
| `18px` (×2) | `style.css:110` (`#fullscreen`), `:143` (`#micro`) | **devient `var(--t-xl)`** (20 px à racine 16), la propriété que la spec §5.2 protège étant désormais gardée par un test | T7 |

⚠️ **Le cas de `0.02em` n'est PAS un tour de passe-passe, et il faut dire
pourquoi.** Le crénage et la pile monospace servent **la même fin** — lire des
nombres qui changent —, et la spec §4.3 désigne la pile monospace comme le
moyen. Le crénage est en outre **antérieur au design system** : il vient de
`bd78d75` (« overlay de mesure et recette du jalon 1 »), relevé par
`git log -S 'letter-spacing' -- client/src/style.css`. ⚠️ **Que le résultat soit
plus lisible est un JUGEMENT HUMAIN** (§10, n°16) : aucune commande ne le dira.

⚠️ **Le cas de `18px` touche une justification chiffrée, et la spec l'interdit à
demi.** Le §5.2 écrit : « Toute reprise du bouton en S4 doit conserver cette
propriété » — *cette propriété* étant `pointer-events: none`
(`style.css:129`) —, « changer sa taille change la zone qu'il occupe, et la
justification est écrite en fonction d'elle ». **S4 conserve la propriété et la
transforme en commande** (tâche 7, garde `client/src/style.test.ts`), et
**corrige le chiffre du commentaire** : la hauteur devient
`var(--e-2) + var(--e-2) + var(--t-xl)` = 8 + 8 + 20 = **36 px exactement à
racine 16**, un **calcul** et non une mesure ; **la largeur cesse d'être
chiffrée**, l'avance du glyphe `⛶` n'étant connue de personne ici. ⚠️ **Que
20 px soit la bonne taille est un jugement humain** (§10, n°17).

**Ce que S4 fait des trois de `shell.css` et `connexion.css`** — celles qui ne
lui sont assignées par personne :

**Décision : elles deviennent des tokens de CONTENANT, à valeur strictement
égale.** `--contenant-page: 72rem`, `--contenant-colonne: 18rem`,
`--contenant-carte: 26rem`, déclarés dans `tokens.css` et employés dans le même
commit (tâche 8).

*Justification.* Les trois sont des **mesures de contenant** — largeur maximale
de page, largeur minimale de colonne de grille, largeur de carte —, et le §4.4
n'échelonne que la typographie et l'espacement : **aucune échelle ne prétend les
couvrir**, exactement comme aucune ne couvre `100vw`. Le journal de S3 l'a
relevé de lui-même (« l'étiquette de `connexion.css` est plus large qu'elle »)
sans en tirer de conséquence. Un design system qui n'a pas de nom pour la
largeur de ses pages en manque un ; et **c'est la dernière occasion de le lui
donner**, ⑥ finissant ici.

*Coût, assumé, et il est double.*
- **S4 touche deux surfaces d'un sous-bloc CLOS.** C'est déclaré, et borné à
  une substitution valeur → token : **aucune valeur ne change**, donc aucune
  apparence. ⚠️ **« Aucune apparence » est un ARGUMENT, pas une mesure** — aucun
  œil ne passera (§10), et les octets bâtis de `shell-*.css` et
  `connexion-*.css` **changeront** (l'indirection `var()` est plus longue que le
  littéral). La recette publie les deux poids.
- **Trois tokens de plus pour trois appelants.** Ce n'est pas une échelle, c'est
  une **famille nommée à trois membres**, et le dire est plus honnête que de
  prétendre à un pas. ⚠️ **Que ces trois-là méritent des tokens est un jugement
  humain** (§10, n°21).

*L'alternative écartée, et pourquoi.* Laisser les trois et **corriger la
clause** — écrire que « aucune longueur hors échelle » ne porte que sur la
typographie et l'espacement. Écartée parce que **redéfinir une règle pour
publier un zéro est exactement le geste que ce dépôt combat** ; S3 a publié six
*et* neuf plutôt que de choisir la règle qui l'arrangeait.

🔴 **Et le vrai livrable de cet axe n'est aucune de ces six corrections : c'est
que la clause CESSE D'ÊTRE UNE DETTE D'ÉNONCÉ.** Jusqu'ici, « aucun des huit
contrôles ne mesure une longueur » est écrit **cinq fois sous cette formule
exacte** (`style.css:33`, `primitives.test.ts:192`, `reprise.test.ts:41`,
`primitives.css:29`, `tokens.css:157`) — et **davantage sous d'autres
tournures**, que la tâche 12 balaie **par le sens**. S4 livre **§7.10** (tâche 2), le premier
contrôle du sous-projet qui mesure une longueur, **et il naît ROUGE de huit
occurrences sur l'arbre intact** — sa preuve d'atteignabilité est l'arbre
lui-même, comme celle de §7.9 ② A en S3.

### 3.3 🔴 Le Window Controls Overlay : la règle est livrée, et AUCUN critère de recette ne la couvre

**La question posée est : comment livrer du code qu'aucune recette ne peut
exercer, sans prescrire un critère que rien ne peut rendre rouge ?**

**Le fait qui gouverne, et il est mesuré (§2.4) : il n'existe aucun manifeste
dans ce dépôt.** Sans `display_override: ["window-controls-overlay"]`, les
variables d'environnement `titlebar-area-*` ne sont **jamais définies** ; il n'y
a donc **aucun état atteignable** dans lequel la règle agisse. Un critère de
recette qui prétendrait l'exercer serait vacueux **par construction**, et pas
faute d'effort.

**Décision : livrer la règle, et ne prescrire AUCUN critère de comportement.**
Ce que S4 livre à la place est un **garde de forme dont la rouge est
atteignable**, et dont l'énoncé est exactement ce qu'il peut prouver :

> **§ garde WCO — aucune règle conditionnelle au WCO ne change la mise en page
> tant que le WCO est inactif.** Trois assertions :
> ① tout `env(titlebar-area-*)` des feuilles de `client/src/` porte le repli
> `0px` ; ② **aucune** `@media (display-mode: window-controls-overlay)`
> n'existe dans ces feuilles ; ③ **atteignabilité** — il existe au moins un
> `env(titlebar-area-` à lire, sans quoi ① et ② sont verts en ne mesurant rien.

**Pourquoi ② interdit une requête média plutôt que de la vérifier** : un repli
neutralise un `env()`, **rien ne neutralise un bloc `@media`**. Une règle
conditionnelle au WCO écrite en `@media` pourrait donc changer la mise en page
d'aujourd'hui sans qu'aucune commande ne le dise. L'interdire rend la propriété
« inerte aujourd'hui » **totale** au lieu de partielle.

**Ce que ce garde prouve** : que le code livré est **sans effet aujourd'hui**, et
que sa perturbation a une **conséquence réelle** — écrire
`env(titlebar-area-height, 8px)` décale le bandeau de 8 px **maintenant**, sur
le produit tel qu'il tourne. **Ce n'est donc pas un contrôle qui valide sa
propre écriture.**

**Ce que ce garde ne prouve PAS, et qui est déclaré au §11** : rien du
comportement sous WCO. La règle n'a **jamais été rendue** dans une fenêtre à
barre de titre superposée, et elle ne le sera pas avant que ④ G5 ne pose le
manifeste. **C'est un legs nommé, avec son destinataire** (§14).

**Ce que la règle contient, et rien de plus** — deux déclarations :

- `#status` descend sous la barre :
  `inset-block-start: calc(env(titlebar-area-height, 0px) + var(--e-3))` ;
- l'écran terminal de la tâche 9 en fait autant sur son remplissage de tête.

⚠️ **Ce que S4 NE livre PAS du WCO, et le dit** : aucune zone de glissement
(`-webkit-app-region: drag`), aucun emploi de `navigator.windowControlsOverlay`,
aucun repositionnement des trois éléments du bas — le WCO n'occupe que le haut de
la zone client.

---

## 4. 🔴 Le défaut que la lecture a trouvé : l'encre de la fenêtre de session est ILLISIBLE en thème clair

**Aucun document de ⑥ ne le porte, et aucun des huit contrôles ne peut le
voir.** Il est établi par trois lignes du dépôt, lues l'une après l'autre :

1. `client/src/design/base.css:57` — `body { … color: var(--texte-fort) }` ;
2. `client/src/design/tokens.css:197` et `:223` — dans les **deux** blocs
   clairs, `--texte-fort: #10131a`, une encre quasi noire ;
3. `client/src/design/tokens.css:98` — `--voile-flottant: rgb(0 0 0 / 0.72)`,
   **hors thème**, donc noir dans les deux thèmes.

**Sous le thème clair, les cinq éléments de la fenêtre de session écrivent du
quasi-noir sur un voile quasi-noir.** `#status` et `#stats` héritent l'encre du
`body` ; `#fullscreen` (`style.css:112`) et `#micro` (`:145`) la déclarent
explicitement.

**Ce n'est pas une régression du produit d'origine : c'est un effet de bord de
S1.** Avant lui, `style.css` posait `color-scheme: dark` en dur et une encre
unique `#e6e8eb` : **la fenêtre de session n'avait pas de thème clair**. S1 lui
en a donné un — c'est le socle, et c'était juste — sans que rien ne remarque que
les voiles, eux, ne suivent pas le thème.

**Pourquoi aucun contrôle ne le voit** : `tokens.css:86-89` place les six voiles
**hors des paires de contraste**, avec sa raison — « leur lisibilité dépend de la
vidéo qui est dessous, qui n'est pas connaissable ». **L'argument est juste pour
le voile ; il ne l'est pas pour l'encre**, qui, elle, est parfaitement
connaissable.

**Le remède (tâche 4) : un septième token hors thème, `--sur-voile: #e6e8eb`**,
employé par les cinq éléments et par la variante flottante de la tâche 5.

- **Aucun changement d'apparence dans le thème sombre** : `#e6e8eb` est
  exactement `--texte-fort` de la racine (`tokens.css:55`), et exactement
  l'encre que la fenêtre de session portait avant S1.
- **`COULEURS_HORS_THEME` passe de six à sept**
  (`client/src/design/tokens.ts:191-197`), **dans le même commit**, sans quoi la
  clause ③ de §7.4 rougit « couleur de la racine sans contrepartie claire ».
- **Une paire de contraste de plus** : `--sur-voile` sur `--video-letterbox`,
  **52 → 53**. C'est la **seule** région où le fond sous l'encre est connu — les
  bandes que laisse `object-fit: contain`. ⚠️ **Elle ne mesure QUE cette
  région** ; au-dessus de l'image, le fond reste inconnaissable et la réserve du
  §11 de la spec tient entière.
- **Prédiction du plan, à confirmer par le contrôle et non à recopier** :
  `#e6e8eb` sur `#000000` vaut **≈ 17**, très au-dessus du minimum global
  actuel (**3,16**), qui ne devrait donc pas bouger. **Si le contrôle rend autre
  chose, c'est le contrôle qui a raison.**

⚠️ **Que `#e6e8eb` soit la bonne encre sur un voile est un jugement humain**
(§10, n°18) : seule sa lisibilité **sur la bande noire** est mesurée.

---

## 5. Le CONTRAT d'apparence — tout ce qui n'y figure pas est une régression

C'est la forme que S3 a employée (§5.1 de son plan), et elle a tenu : ses sept
lignes ont été livrées, et le huitième changement, hors plan, a été **déclaré**
plutôt que dissimulé. Les voici pour S4.

| # | Surface | Ce qui change | Tâche |
| --- | --- | --- | --- |
| ① | `#status`, `#stats` | remplissage vertical **6 → 8 px**, rayon **`--r-2` → `--r-1`** (6 → 4 px), bordure présente mais **transparente** | T5 |
| ② | `#stats` | **un bandeau vide disparaît** au lieu de rester un cadre (`.message:empty`) — il est vide à l'ouverture (`index.html:17`) | T5 |
| ③ | `#stats` | **pile monospace**, crénage retiré | T6 |
| ④ | `#fullscreen`, `#micro` | glyphe **18 → 20 px** (`var(--t-xl)`) | T7 |
| ⑤ | les cinq éléments | l'encre cesse de suivre le thème : **aucun changement en SOMBRE**, **réparation d'un défaut en CLAIR** | T4 |
| ⑥ | la fenêtre de session | un état **terminal** ouvre un **écran plein cadre** au lieu de rester dans le bandeau | T9 |
| ⑦ | la fenêtre de session, **sous WCO seulement** | `#status` et l'écran descendent sous la barre de titre. **Inatteignable aujourd'hui** | T10 |
| ⑧ | `shell.html`, `connexion.html` | **RIEN.** T8 ne substitue que des valeurs identiques | T8 |
| ⑨ | `primitives.html` (galerie) | une variante de plus est rendue : `.message--flottant` | T5 |

⚠️ **Le rayon de ① n'est pas compensé, et c'est délibéré.** Le compenser par une
règle d'`ID` qui écrase la primitive recréerait exactement la divergence que le
design system existe pour empêcher. **Le déclarer coûte une ligne ; le
compenser coûterait la règle.**

---

## 6. Les tâches

**Convention commune à toutes** :
- **un commit par tâche**, message `design(s4): …`, **pathspec explicite** —
  jamais `git add -A`, l'arbre est partagé ;
- **un token qui naît, naît avec son appelant dans le MÊME commit** : le §7.6
  exige l'**ÉGALITÉ**, et le sortir de la liste sans écrire l'appelant rend
  `NOUVEL ORPHELIN` ;
- **toute rouge suit le harnais du §9, sans exception** ;
- **`unset -f chpwd` avant toute collecte de journal** (piège de S2, repayé par
  S3), et les outils de `client/outils/` **se lancent depuis la racine du
  dépôt**, jamais depuis `client/`.

---

### T1 — Extraire le lecteur de CSS de `primitives.test.ts`, AVANT toute addition

**Objet.** Sortir de `primitives.test.ts` la machinerie qui lit une feuille
(blanchiment des commentaires, découpe en règles, en sélecteurs et en
déclarations) vers un module pur, pour que les gardes neufs de S4 la
réemploient au lieu de la recopier.

**Fichiers.** Créés : `client/src/design/css.ts`, `client/src/design/css.test.ts`.
Modifié : `client/src/design/primitives.test.ts`.

**Pourquoi en premier.** `primitives.test.ts` est à **283 lignes pour une porte à
300, marge 17** — la marge la plus serrée de `client/` après
`verify-webrtc.mjs`. S2 et S3 ont tous deux payé « une addition de commentaire
annule une extraction ». **On extrait avant d'ajouter**, comme la tâche 6 de D9
et les tâches 1 à 3 de D10.

⚠️ **DIVERGENCE DÉCLARÉE.** Le point de chute que S2 puis S3 nomment pour ce
fichier est « scinder par objet — les gardes de forme d'un côté, les gardes de
famille de l'autre, **sans séparer G5 de sa source** ». **Cette extraction-ci est
différente** : elle sort l'**outil**, pas les gardes, et laisse G5 auprès de sa
source. Elle est **complémentaire**, pas substitutive ; le point de chute nommé
reste ouvert et reste le bon si le fichier regrossit.

**Ce que la tâche doit prouver.**
- `primitives.test.ts` rend **exactement les mêmes verdicts** qu'avant : G1 à G7,
  **9 tests passés**, aucun changement de message.
- `css.test.ts` couvre les trois propriétés qui font la valeur du module :
  ① un commentaire **ne déclare rien** (blanchiment) ; ② une déclaration
  s'extrait avec sa propriété et sa valeur ; ③ un sélecteur composé se découpe
  sur `,` `>` `+` `~` et l'espace.
- **La marge de `primitives.test.ts` est RELEVÉE, avant et après.**

**Ce qui rend ROUGE.** Dans `css.test.ts` : faire rendre au blanchiment le texte
d'origine (retirer le remplacement des `/* … */`) → l'assertion ① tombe, et le
message nomme la déclaration fantôme. **Le diff est non vide par construction**
(une ligne de code retirée) ; le harnais du §9 l'exige de toute façon.

**Sorties de liste d'attente : aucune.**

---

### T2 — **§7.10**, le contrôle qui mesure une longueur, et qui NAÎT ROUGE

**Objet.** Le premier contrôle de ⑥ qui mesure une longueur : **toute longueur
d'une feuille de SURFACE passe par un token**, sauf une liste d'exceptions
**close et nommée**.

**Fichiers.** Créé : `client/src/design/longueurs.test.ts`. Aucun autre.

**Portée exacte, et sa frontière est écrite dans le fichier.**
- Il balaie les **feuilles de surface** : `client/src/style.css`,
  `client/src/shell.css`, `client/src/connexion.css`, **plus toute feuille de
  `client/src/session/`** (celle que la tâche 9 crée) — la liste est
  **dérivée**, jamais énumérée : toutes les `*.css` de `client/src/` **hors**
  `client/src/design/`, dont G4 s'occupe déjà.
- **Il ne double PAS G4** : G4 garde `primitives.css` et ses quatre familles ;
  §7.10 garde les surfaces. La frontière est écrite dans les deux fichiers.

**Les trois exceptions, closes, chacune avec sa raison.**
1. **Les remplissages de fenêtre** — `100vw`, `100vh`, `100dvh`. C'est la règle
   de compte que `style.css` applique depuis S1 et que le journal de S3 a
   énoncée **avant** de compter : « un remplissage de fenêtre n'est pas une
   valeur hors échelle ».
2. **Le zéro** — `0`, `0px`. Aucune échelle n'a de cran nul, et le repli des
   `env(…)` du WCO (§3.3) en porte un par construction.
3. **`tokens.css`** — c'est la source unique ; ses littéraux **sont** l'échelle.
   *(Il est de toute façon hors de la portée dérivée ci-dessus.)*

**Ce qu'il imprime, succès compris** — « un contrôle de dérive dont on ne lit
jamais la valeur ne sert qu'à passer » (`poids-css.mjs`) : le nombre
d'**occurrences** et le nombre de **valeurs distinctes**, avec leur `fichier`,
leur propriété et la valeur fautive. **Les deux nombres, jamais un seul** (§2.5).

**Garde d'atteignabilité, obligatoire.** Une assertion d'absence est verte sur un
fichier vide — c'est G5, et c'est le piège que ce dépôt a payé en S2. §7.10 doit
donc **exiger d'avoir lu quelque chose** : au moins une feuille trouvée, et au
moins une déclaration à longueur **passant par un token** (`var(--e-3)`,
`var(--r-2)`…). **Sa rouge se joue en vidant `client/src/style.css`**, pas en y
ajoutant une valeur.

**Blanchiment, et sa contre-épreuve.** Les commentaires sont retirés **avant**
analyse (le module de T1). **Contre-épreuve exigée, où le VERT est le résultat
attendu** : une longueur littérale écrite **dans un commentaire** de `style.css`
laisse §7.10 **vert**. C'est le pendant exact des deux contre-épreuves de S3.

🔴 **IL NAÎT ROUGE SUR L'ARBRE INTACT — huit occurrences, six valeurs (§2.5), et
c'est SA PREUVE D'ATTEIGNABILITÉ.** La branche le porte rouge de T2 à T8, comme
S1 a porté §7.2 rouge de sa tâche 1 à sa tâche 9, et S3 §7.9 ② A de sa tâche 2 à
sa tâche 4. **Conséquence à déclarer et à ne pas subir** : `npm test`, donc
`scripts/verify-all.sh`, **sont rouges pendant sept commits**. La recette (T11)
verse **et** la rouge de naissance **et** le vert de clôture.

**Où il est numéroté.** **§7.10**, comme §7.9 : une **ADDITION DE PLAN**,
déclarée telle, inscrite dans la spec par la tâche 12. Il est un **test
unitaire**, comme §7.5 — donc `design:verifier` continue de dire **7/7** et
`verify-all.sh` garde ses **dix-huit** en-têtes. **Le compte devient : neuf
contrôles, sept scripts, deux tests unitaires.**

⚠️ **Douze places du dépôt disent « les huit contrôles », et cinq disent
« aucun ne mesure une longueur ».** Elles deviennent fausses ici. **Elles sont
corrigées par la tâche 12, énumérées AVANT d'être touchées, une par une par
numéro de ligne** — jamais par substitution globale (§ tâche 12).

**Sorties de liste d'attente : aucune.**

---

### T3 — Durcir §7.9 ② A : les TROIS surfaces du produit, et il NAÎT ROUGE lui aussi

**Objet.** L'assertion ② A exige aujourd'hui qu'**au moins une** surface du
produit emploie **au moins une** famille. S4 la durcit : **chacune des trois
surfaces en emploie au moins une**.

**Fichier.** Modifié : `client/outils/classes-employees.mjs`. Aucun autre.

**Pourquoi maintenant, et pas dans le commit qui habille.** Le durcir dans le
même commit que T5 le rendrait **vert dès sa naissance**, donc jamais vu rouge
sur l'arbre. Le durcir **avant** le fait naître rouge sur `client/index.html :
aucune famille` (§2.2) — **l'arbre lui-même est sa preuve d'atteignabilité**,
et c'est exactement ce que S3 a fait de ② A.

**Ce que la tâche doit prouver.** La sortie nomme **la surface** qui n'emploie
rien, pas seulement le fait qu'il en existe une. Le relevé par surface, déjà
imprimé, ne change pas de forme.

**Ce qui rend ROUGE (après T5).** Retirer les classes de primitive de
**`client/index.html`** seul : la surface retombe à « aucune famille ». ⚠️ **La
rouge doit ne retirer QUE des classes déclarées** — S3 a dû refaire la sienne
pour avoir introduit des classes inventées, si bien que l'`exit=1` venait de la
clause ① et non de ② A. **Lire QUELLE assertion a rougi, jamais seulement le code
de sortie.**

**Sorties de liste d'attente : aucune.**

---

### T4 — `--sur-voile` : réparer l'encre du thème clair, et la 53ᵉ paire

**Objet.** Poser le septième token hors thème et l'employer sur les cinq
éléments de la fenêtre de session (§4).

**Fichiers.** Modifiés : `client/src/design/tokens.css` (déclaration),
`client/src/design/tokens.ts` (`COULEURS_HORS_THEME`),
`client/src/design/contraste.ts` (la paire), `client/src/style.css` (les
appelants). Éventuellement `client/design.html` si la galerie rend les voiles.

**Ce que la tâche doit prouver, et chaque chiffre est RELEVÉ dans la sortie du
contrôle, jamais recopié de ce plan.**
- §7.1 rend **53 paires**, **0 échec**, et **son minimum global** — que le plan
  prédit inchangé à **3,16** et que la tâche **relève**.
- §7.4 rend **0 écart** : `blocs-de-theme.mjs` imprime désormais
  « ③ couleurs de racine ⊆ clair sauf **7** hors-thème nommés ».
- §7.6 rend **0 écart** : le token naît avec ses appelants.
- §7.2 rend **0** : la valeur vit dans `tokens.css`, seule autorisée.

**Ce qui rend ROUGE — trois mutations, une à la fois.**
1. **Retirer `--sur-voile` de `COULEURS_HORS_THEME`** → §7.4 clause ③ :
   « `--sur-voile` est une couleur de la racine sans contrepartie claire ».
   ⚠️ **C'est la rouge la plus instructive** : elle montre que le token est bien
   *classé*, et pas seulement *écrit*.
2. **Remplacer `#e6e8eb` par une encre sombre** (`#1a1a1a`) → §7.1 rend un échec
   sur la paire neuve, et **le minimum global chute** — la sortie le dit.
3. **Retirer les appelants de `style.css` sans retirer le token** → §7.6 rend
   `NOUVEL ORPHELIN --sur-voile`.

**Sorties de liste d'attente : aucune** — mais **une entrée serait ajoutée** si
l'appelant n'était pas écrit dans le même commit, et c'est la rouge n°3.

---

### T5 — La variante `.message--flottant`, et la fenêtre de session emploie enfin une primitive

**Objet.** Livrer la reprise des bandeaux sur la famille `message` que la spec
§6 confie à S4, **sans perdre le voile** que son §5.2 déclare comme exception
nommée.

**Fichiers.** Modifiés : `client/src/design/primitives/message.css` (la
variante), `client/primitives.html` (la galerie la rend), `client/index.html`
(le balisage), `client/src/style.css` (le remplissage littéral part).

🔴 **LA TENSION QUE CETTE TÂCHE RÉSOUT, ET QUE LA SPEC NE VOIT PAS.** Le §6 de
la spec demande « la reprise des trois bandeaux sur les primitives *message* » ;
son §5.2 exige que ces bandeaux portent `--voile-flottant`, « d'où
`--voile-flottant`, qui est l'exception nommée au point 2 de §3 ». Or
`primitives/message.css:55` pose `background: var(--fond-1)` — **un fond opaque
qui suit le thème**. Poser `.message` nu sur `#status` mettrait, en thème clair,
un panneau **clair et opaque** au-dessus d'une image vidéo — c'est-à-dire
exactement ce que `tokens.css:81-84` interdit en une phrase : « un encadrement
clair autour d'une image vidéo se lit comme un défaut d'affichage ».

**La variante `.message--flottant` tient les deux** : elle hérite de `.message`
son remplissage, son rayon, son interligne et sa règle `:empty`, et remplace le
fond par `var(--voile-flottant)`, l'encre par `var(--sur-voile)` (T4) et la
couleur de bordure par `transparent` — **l'un des quatre mots-clés que §7.2
autorise**, avec `currentColor`, `inherit` et `none`.

⚠️ **Elle vit dans `primitives/message.css`, pas dans `style.css`, et c'est le
même arbitrage que S3 a rendu pour `.message:empty`** : une règle qui décrit la
famille remonte dans la famille. **Divergence possible à déclarer si l'exécution
en décide autrement.**

⚠️ **Elle n'ouvre AUCUNE cinquième famille** : les familles sont dérivées des
**fichiers** de `client/src/design/primitives/` (`classes-employees.mjs`), et
cette tâche n'en crée aucun. **Une tâche qui poserait un fichier de plus dans ce
répertoire créerait une famille que §7.9 ③ B exigerait de la galerie** — à
savoir avant d'y écrire.

**Le balisage** : `#status` et `#stats` reçoivent
`class="message message--flottant"`. `#stats` garde `font-size: var(--t-s)`
dans `style.css` (l'`ID` l'emporte sur la classe, et `--t-s` est son cran).

**Ce que la tâche doit prouver.**
- §7.9 rend `client/index.html : message` — **là où il rendait « aucune
  famille »** —, et **② A durcie (T3) passe au vert**.
- §7.9 ① rend **0 écart** : `message--flottant` est déclarée.
- §7.9 ③ B : la famille `message` reste rendue par `primitives.html`, et le
  compte de ses classes rendues **augmente d'une**.
- §7.10 **perd deux occurrences** (les deux `6px`) : le contrôle l'imprime.
- §7.2 rend **0** : `transparent` est autorisé.

**Ce qui rend ROUGE.**
- Retirer `class="message message--flottant"` de `client/index.html` → **② A
  durcie**, avec la surface nommée. ⚠️ Ne retirer **que** des classes déclarées.
- Écrire `class="message message--flottante"` (faute de frappe) → **① NON
  DÉCLARÉE**, ce qu'aucun autre contrôle ne verrait.
- Retirer la variante de `primitives.html` → **③ B ne rougit PAS** (la famille
  reste rendue par ses quatre autres classes). **Le dire est important** : le
  sens « toute classe déclarée est employée » n'existe pas (§14), et la galerie
  reste tenue par l'œil.

**Sorties de liste d'attente : aucune.**

---

### T6 — `--police-mono` sur `#stats`, et la liste d'attente devient VIDE

**Objet.** Trancher le legs que trois sous-blocs se sont passé (§3.1).

**Fichiers.** Modifiés : `client/src/style.css` (la pile monospace, le crénage
retiré), `client/outils/tokens-orphelins/attente.mjs` (l'entrée sort,
`SOUS_BLOCS_CLOS` gagne `'S4'`), `client/src/design/tokens.css` (l'encadré du
token cesse de dire que son sort est ouvert).

**Ce que la tâche doit prouver.**
- §7.6 rend `48 token(s) déclaré(s)` / `48 token(s) employé(s)` /
  **`0 orphelin(s), dont 0 en attente déclarée`** / `total : 0 écart(s)`.
  ⚠️ **Le compte de déclarés sera de 49 après T4 et de 52 après T8** : la tâche
  **relève** ce que le contrôle imprime **à son commit**, et ne recopie pas ce
  plan.
- §7.10 **perd une occurrence** (`0.02em`).
- **`attente.mjs` MAIGRIT** : la doctrine de l'entrée part **avec** l'entrée
  (c'est la règle d'extraction que le fichier porte). Taille **relevée avant et
  après**, publiée. ⚠️ **Il est à 227 pour un seuil d'extraction à 240** : si,
  contre toute attente, la tâche le fait croître au-delà, **elle extrait, elle
  ne compresse pas**.

**Ce qui rend ROUGE — les deux sens, et c'est ce qui fait la valeur de cette
liste.**
1. **Retirer la ligne `font-family` de `style.css` sans remettre l'entrée** →
   `NOUVEL ORPHELIN --police-mono`.
2. **Remettre l'entrée dans `attente.mjs` sans retirer l'appelant** →
   `À RETIRER DE LA LISTE --police-mono`.
3. **Ajouter une entrée nommant `'S1'`** → clause ③, `SOUS-BLOC CLOS` — la
   mitigation que S3 a construite continue de mordre après que la liste est
   vide, **et c'est la raison pour laquelle le fichier reste**.

**Sortie de liste d'attente : `--police-mono`, la dernière, dans CE commit.**

---

### T7 — Le garde de `pointer-events`, PUIS `18px → var(--t-xl)`

**Objet.** Transformer en commande la propriété que la spec §5.2 protège, **avant**
de toucher la taille dont elle dépend.

**Fichiers.** Créé : `client/src/style.test.ts`. Modifié :
`client/src/style.css`.

**L'ordre à l'intérieur de la tâche est portant, et c'est la leçon de la tâche 6
de D9** : le garde s'écrit **et se voit rouge** d'abord, la taille change
ensuite. Un garde écrit après le changement ne prouve pas qu'il aurait attrapé
la régression.

**Ce que le garde assied.**
- ① `#fullscreen[data-actif="true"]` déclare `pointer-events: none`.
  ⚠️ **Ancré sur la ligne de DÉCLARATION entière**, jamais sur la sous-chaîne
  `pointer-events` — le commentaire de `style.css:124-128` la nomme, et un garde
  cherchant une sous-chaîne serait satisfait par le commentaire du fichier qu'il
  analyse. **C'est le piège que ce dépôt a payé trois fois** (S1 sur `CLE_THEME`,
  S2 sur G1/G5, S3 sur la rouge n°16).
- ② **atteignabilité** : le fichier lu déclare au moins une règle, sinon ① est
  vert sur une feuille vide.

**Ce que la modification fait.** `font-size: 18px` → `font-size: var(--t-xl)`
sur `#fullscreen` et `#micro`. Le commentaire de `style.css:124-128` est repris :
la hauteur devient **36 px exactement à racine 16** (`8 + 8 + 20`), **déclarée
comme un CALCUL**, et **la largeur cesse d'être chiffrée** (§3.2).

**Ce qui rend ROUGE.** Retirer la déclaration `pointer-events: none` de
`style.css` → ① tombe en nommant le sélecteur. **Vérifier que le diff est non
vide** : le commentaire voisin porte les mêmes mots, et une mutation qui ne
toucherait que lui rendrait `exit=0` — c'est littéralement le cas que le
blanchiment existe pour distinguer.

**Ce que le garde ne dit PAS.** Il ne dit rien de la **zone réellement occupée**
par le bouton : aucune page n'est ouverte, aucun pixel n'est mesuré, et
l'avance du glyphe reste inconnue. **Il garde la propriété, pas la géométrie.**

**Sorties de liste d'attente : aucune.**

---

### T8 — Les trois mesures de contenant deviennent des tokens, et §7.10 passe au VERT

**Objet.** Solder les trois dernières longueurs hors token, celles de `shell.css`
et `connexion.css` (§3.2).

**Fichiers.** Modifiés : `client/src/design/tokens.css` (trois déclarations dans
`:root`), `client/src/shell.css`, `client/src/connexion.css`.

**Les trois tokens**, à valeur **strictement égale** :
`--contenant-page: 72rem` (`shell.css:61`, largeur maximale de la page),
`--contenant-colonne: 18rem` (`shell.css:114`, largeur minimale d'une colonne de
la grille de cartes), `--contenant-carte: 26rem` (`connexion.css:55`, largeur de
la carte de connexion).

⚠️ **Ils vivent dans la racine SEULE**, comme les échelles, et **la clause ③ de
§7.4 ne les concerne pas** : elle est restreinte aux **couleurs**, et
`estUneCouleur` (`client/src/design/tokens.ts:200-203`) décide **sur la valeur**
— `72rem` n'en est pas une. **Vérifié par lecture ; la tâche le confirme par la
sortie du contrôle.**

**Ce que la tâche doit prouver.**
- §7.10 rend **0 occurrence, 0 valeur** : **la clause §8 « aucune longueur hors
  échelle » cesse d'être fausse**, sous la règle de compte de S3 **et** sous celle
  du contrôle. **Les deux comptes sont publiés.**
- §7.6 rend **0 écart** : trois tokens de plus, trois appelants dans le même
  commit.
- **Les poids bâtis de `shell-*.css` et `connexion-*.css` sont relevés avant et
  après** : ils **changent**, l'indirection étant plus longue que le littéral, et
  ⚠️ **le prétendre invariant serait faux**. §7.7 reste sous le plafond.

**Ce qui rend ROUGE.** Réintroduire `max-inline-size: 72rem` littéral dans
`shell.css` → §7.10 le nomme avec sa propriété et son fichier. Diff non vide par
construction.

⚠️ **SEPT LIGNES DE COMMENTAIRE MENTIONNENT CES TROIS VALEURS, ET ELLES NE SE
TRAITENT PAS TOUTES PAREIL.** Relevées le 20 août 2026 par
`grep -n '72rem\|18rem\|26rem' client/src/shell.css client/src/connexion.css`,
lignes de déclaration exclues (`shell.css:61` et `:114`, `connexion.css:55`,
qui sont l'objet même de la tâche) :

- **CINQ deviennent fausses et se corrigent DANS CE COMMIT** — `shell.css:22`,
  `:54`, `:109`, `:113`, `connexion.css:21` : elles affirment toutes, au
  **présent**, que ces mesures « ne passent par aucun cran d'échelle » ou sont
  « hors échelle ». Après cette tâche, elles passent par un token nommé.
  **Corriger une affirmation dans le commit qui la réfute coûte moins cher que
  de la retrouver en revue.**
- ⚠️ **DEUX sont des énoncés DATÉS et restent VRAIS COMME HISTOIRE** —
  `shell.css:48` (« la tâche 5 [de S3] en a ajouté une sans reprendre cette
  phrase ») et `connexion.css:30` (« … ET C'EST `26rem` (revue transverse de
  S3) »). **Les barrer les rendrait faux.** Elles reçoivent une annotation
  datée, pas une réécriture — c'est la règle que `CLAUDE.md` applique à ses
  propres relevés périmés.

**Ce que la tâche NE fait PAS.** Elle ne change **aucune valeur**, ne touche
**aucun balisage**, et **ne porte aucun jugement** sur ces trois largeurs :
⚠️ **qu'elles soient les bonnes reste un jugement humain**, et l'était déjà.

⚠️ **DIVERGENCE DÉCLARÉE, la plus lourde du sous-bloc** : S4 modifie deux
feuilles d'un sous-bloc **CLOS**. La raison est au §3.2, le coût est écrit, et
l'alternative écartée est nommée.

**Sorties de liste d'attente : aucune.**

---

### T9 — L'écran plein cadre des états terminaux

**Objet.** Livrer la famille 1 du §5.2 : un état terminal cesse d'occuper six
lignes dans un bandeau de 12 px de marge.

**Fichiers.** Créés : `client/src/ecran-terminal.ts`,
`client/src/ecran-terminal.test.ts`, `client/src/session/etat-terminal.css`.
Modifiés : `client/index.html` (le balisage), `client/src/status.ts` (la cible
optionnelle), `client/src/style.css` (l'`@import`), `client/src/main.ts` (le
câblage et le ton).

**Ce qu'est un état terminal, et ce qui n'en est pas un — mesuré, pas décidé.**
La frontière existe déjà dans le modèle : `status.ts` distingue `terminal` de
`persistant` et d'ordinaire (`client/src/status.ts:26-35`), et **exactement deux
appels** passent `terminal: true` (`main.ts:159`, `main.ts:448`). **Tout le
reste reste au bandeau** — les onze autres appels de `main.ts` relevés par
`grep`, dont les messages persistants du lien dégradé et du sommeil.
⚠️ **S4 ne déplace AUCUNE frontière** : il rend visible celle que le code porte.

**L'architecture, et pourquoi le point d'écriture unique ne bouge pas.**
`status.ts` existe précisément pour qu'« aucun appelant ne puisse oublier la
garde » (`status.ts:11-14`). **L'écran se branche donc DANS `creerStatut`**, par
une seconde cible **injectée et optionnelle**, et non par un appelant de plus
dans `main.ts` : les deux sites terminaux ne changent que pour porter leur
**ton**. Le module `ecran-terminal.ts` est **pur, dépendances injectées,
testable sans DOM** — la convention de `status.ts`, `audio.ts` et `fullscreen.ts`.

**Les deux tons.** `session terminée : …` est une fin **normale** (l'utilisateur
a fermé l'application) ; `échec : …` est une **erreur**. L'écran porte donc
`.message` neutre dans le premier cas et `.message--danger` dans le second.
⚠️ **DUPLICATION DÉCLARÉE, non résorbée** : `client/src/shell.ts:23` déclare
déjà un type `Ton` et `client/src/connexion.ts:98` une table
`CLASSE_DE_TON`. **S4 ne les unifie pas** — cela toucherait deux surfaces
closes, sans critère capable d'attraper une régression et sans œil pour la voir.
**Legs n°8 du §14.**

**Le balisage, et le piège de spécificité qu'il porte.**

```html
<div id="fin" class="ecran" hidden>
  <div class="carte ecran__carte">
    <h1 class="ecran__titre">…</h1>
    <p class="message" id="fin-raison" role="status"></p>
  </div>
</div>
```

🔴 **`[hidden]` PERD CONTRE UNE CLASSE.** Une règle `.ecran { display: grid }`
l'emporte en spécificité sur le `[hidden] { display: none }` de la feuille de
l'agent utilisateur : **l'écran serait visible dès le chargement**, sur toutes les
sessions. La feuille doit donc porter `.ecran[hidden] { display: none }`
explicitement, **et le garde de `client/src/style.test.ts` l'exige** — assertion
③, ancrée sur la règle entière.

**Où vit la feuille.** `client/src/session/etat-terminal.css`, **le point de
chute que la spec §10 nomme d'avance**, et **créé d'emblée plutôt qu'après
coup** : `style.css` est à 184 lignes, ce dépôt écrit de longs commentaires, et
l'extraction faite **avant** l'addition est la doctrine (S2 sur `primitives.css`,
D10 sur trois fichiers).
⚠️ **Elle est `@import`ée par `style.css`, pas liée depuis `index.html`.**
L'encadré de `style.css:37-52` mesure qu'un `@import` **ré-inline** la feuille
dans chaque entrée qui la traverse — c'est vrai, **et sans conséquence ici** :
`etat-terminal.css` n'a **qu'un seul consommateur**, il n'y a rien à partager.
C'est la forme exacte de `primitives.css:82-85`, qui `@import`e ses quatre
familles.

**Ce que la tâche doit prouver.**
- **Tests neufs** : ① un message `terminal` lève l'écran **et** y écrit le
  texte ; ② un message **ordinaire** ne le lève pas ; ③ un message **persistant**
  non plus ; ④ le ton `danger` pose `message--danger` et le ton neutre ne pose
  rien ; ⑤ un second terminal **remplace** le premier.
- **Les onze tests de `status.test.ts` passent inchangés** — la cible est
  optionnelle, et sans elle le comportement d'aujourd'hui est intact. **Si l'un
  d'eux doit changer, c'est que la garde terminale a été affaiblie.**
- §7.9 ① rend **0 écart** : `.ecran`, `.ecran__carte`, `.ecran__titre` sont
  déclarées par `session/etat-terminal.css`, `carte` et `message` par les
  primitives.
- **Le balisage arrive dans le bâti** : `dist/index.html` porte `id="fin"`.
- **`main.ts` est mesuré avant et après** (§2.6).

**Ce qui rend ROUGE.**
- Faire lever l'écran par un message **ordinaire** → le test ② tombe. ⚠️ **C'est
  le sens inverse, et c'est lui qui compte** : un écran qui se lève toujours
  passerait le test ①.
- Retirer `.ecran[hidden] { display: none }` → assertion ③ du garde.
- Retirer l'écriture du texte dans l'écran → le test ① tombe **en nommant le
  texte attendu**, pas seulement la visibilité.

**Ce que S4 ne met PAS dans cet écran, et le déclare.** **Aucune action** — ni
« réessayer », ni « fermer ». Une action est un comportement de produit, qui
appartient à ②, et l'inventer ici la ferait naître sans recette. ⚠️ **Qu'un écran
terminal sans action soit la bonne forme est un jugement humain** (§10, n°20).

**Sorties de liste d'attente : aucune.**

---

### T10 — Le WCO : la règle, et le garde qui prouve qu'elle est INERTE

**Objet.** Livrer la mise en page conditionnelle du §5.2 famille 2, dans la seule
forme que ce dépôt peut défendre (§3.3).

**Fichiers.** Modifiés : `client/src/style.css` (`#status`),
`client/src/session/etat-terminal.css` (le remplissage de tête),
`client/src/style.test.ts` (les trois assertions du garde).

**Ce que la tâche doit prouver.**
- Les **trois assertions** du §3.3, dont la **troisième est le garde
  d'atteignabilité** — sans un `env(titlebar-area-` à lire, les deux premières
  sont vertes en ne mesurant rien.
- §7.10 reste vert : le repli `0px` est **explicitement exclu** (§ T2,
  exception n°2), et cette exception est écrite **avant** cette tâche, pas
  ajustée pour elle.

**Ce qui rend ROUGE.** Écrire `env(titlebar-area-height, 8px)` → assertion ①.
⚠️ **Cette mutation a une conséquence RÉELLE sur le produit d'aujourd'hui** — le
bandeau descend de 8 px sur toutes les sessions —, et c'est ce qui distingue ce
garde d'un contrôle qui validerait sa propre écriture.
Second rouge : ajouter `@media (display-mode: window-controls-overlay) { … }` →
assertion ②.

**Ce que la tâche N'A PAS le droit de prescrire.** **Aucun critère de recette.**
Aucun état où la règle agit n'est atteignable (§2.4), et un critère vacueux est
pire qu'un critère absent : il se lit comme une preuve. **La ligne
correspondante du §11 le déclare, et le §14 nomme son destinataire.**

**Sorties de liste d'attente : aucune.**

---

### T11 — La recette

**Objet.** Mesurer, **deux exécutions par critère**, et verser tous les journaux
**dans git**.

⚠️ **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les
contrôles de ⑥ sont **déterministes** (spec §9) ; la question « combien de fois
sur combien » **ne se pose pas ici et ne doit pas être empruntée** à une campagne
qui, elle, l'aurait posée.

**Les sept critères, chacun avec le chiffre qu'il doit RELEVER :**

| # | Critère | Ce qui est relevé |
| --- | --- | --- |
| ① | **les NEUF contrôles sont verts** | `design:verifier` → `7/7`, `exit=0` ; `npm test` → **N passed**, dont §7.5 et **§7.10** |
| ② | **la liste d'attente est VIDE** | §7.6 → `0 orphelin(s), dont 0 en attente déclarée`, `0 écart` ; **le nombre de tokens déclarés et employés, égaux** |
| ③ | 🔴 **la fenêtre de session A BOUGÉ, et le critère ③ de S3 tombe** | `git diff --stat` **non vide** sur `client/index.html` et `client/src/style.css` ; `dist/index.html` **diffère** de la base ; le hachage de `main-*.css` **diffère**. **C'est le sens inverse du critère ③ de S3, et il se mesure par le même montage** |
| ④ | **la fenêtre de session emploie des primitives** | §7.9 → `client/index.html : message` (et `surface` si l'écran terminal l'emploie), **les TROIS surfaces couvertes**, `0 écart` |
| ⑤ | 🔴 **plus aucune longueur hors token** | §7.10 → **0 occurrence, 0 valeur**, contre **8 occurrences / 6 valeurs** à la naissance du contrôle. **Les deux comptes publiés** |
| ⑥ | **les contrastes** | §7.1 → **53 paires**, `0 échec`, **le minimum global relevé** |
| ⑦ | **le poids CSS sous le plafond** | §7.7 → la somme, le plafond **12 288**, la marge, **et le détail par feuille**, comparés à la base **8 011** |
| — | le **jugement visuel** | ⛔ **NON PORTÉ** — voir le §10 |

**Le montage du critère ③, et il est repris de S3 à l'identique.** La base est le
**parent du premier commit de S4**, jamais une date : S3 a dû corriger son plan
sur ce point (« des chantiers voisins ont touché `client/` entre les deux »).
`git archive <base> client proto` vers un arbre jetable **hors du dépôt**,
`node_modules` lié depuis `client/`, `npx vite build`, puis comparaison des
hachages `sha256`.
🔵 **Et le montage doit savoir voir une IDENTITÉ, pas seulement une différence** :
`socle-*.css` **ne doit PAS changer de hachage** si aucune tâche n'a touché
`tokens.css` — or T4 et T8 le touchent, donc **il changera**. La contre-épreuve
est ailleurs : **`shell-*.css` et `connexion-*.css` changent** (T8) et
**`primitives-*.css` change** (T5) ; **un actif qui ne changerait sur rien ne
prouverait rien**. La recette **énumère les cinq actifs et dit pour chacun
pourquoi il a changé ou non**.

⚠️ **`client/src/design/amorce-theme.js` part VERBATIM dans chaque page bâtie,
commentaires compris** (piège de S3). Si une tâche ou la revue y corrige un mot,
**les cinq pages de `dist/` diffèrent, à taille éventuellement égale**. À savoir
avant de comparer des actifs, et à déclarer si cela arrive.

**Les rouges à verser** — chacune suit le harnais du §9 :

| Contrôle | Mutation | Ce que la sortie doit dire |
| --- | --- | --- |
| §7.10 ① | réintroduire `padding: 6px …` dans `style.css` | l'occurrence, avec son fichier et sa propriété |
| §7.10 ② (atteignabilité) | **vider** `client/src/style.css` | « aucune déclaration lue » — **pas** `0 écart` |
| §7.10 blanchiment | écrire `6px` dans un **commentaire** | ⚠️ **VERT attendu** — contre-épreuve |
| §7.9 ② A durcie | retirer les classes de `client/index.html` **seul** | la **surface nommée**, et `① 0 écart` |
| §7.9 ① | `class="message--flottante"` | `NON DÉCLARÉE message--flottante` |
| §7.6 sens 1 | retirer `font-family: var(--police-mono)` | `NOUVEL ORPHELIN --police-mono` |
| §7.6 sens 2 | remettre l'entrée dans `attente.mjs` | `À RETIRER DE LA LISTE` |
| §7.6 clause ③ | une entrée nommant `'S1'` | `SOUS-BLOC CLOS` |
| §7.4 clause ③ | retirer `--sur-voile` de `COULEURS_HORS_THEME` | « couleur de la racine sans contrepartie claire » |
| §7.1 | encre du voile en sombre | l'échec de la paire neuve, **et le minimum qui chute** |
| §7.2 | une couleur littérale dans `session/etat-terminal.css` | le `fichier:ligne` |
| §7.3 | délier une feuille d'`index.html` | l'assertion tombée, A ou B |
| §7.7 | un `@font-face` en `data:` | `DÉPASSEMENT de … octets` |
| garde `pointer-events` | retirer la déclaration | le sélecteur nommé |
| garde `[hidden]` | retirer `.ecran[hidden]` | la règle nommée |
| garde WCO ① | repli à `8px` | le repli nommé |
| garde WCO ② | ajouter la requête média | la requête nommée |
| écran terminal, **sens inverse** | lever l'écran sur un message ordinaire | le test ② tombe |
| §7.5 | supprimer l'application locale | `2 failed \| 8 passed` |

⚠️ **La rouge du §7.4 clause ③ demande une précaution mesurée par S3** : sa
mutation doit atteindre **les deux** blocs clairs si elle porte sur une couleur
de thème, l'un étant indenté de huit espaces sous son `@media`. Ici la mutation
porte sur une **liste TypeScript**, pas sur le CSS — le piège ne s'applique pas,
**et le dire évite de le contourner à l'aveugle**.

**Les journaux versés** sous
`docs/superpowers/plans/journaux-design-s4/`, avec un
`familles-de-lecture.txt` **mesuré** — ⚠️ **écrit avec un heredoc entre quotes**,
jamais avec `echo`, qui en zsh interpréterait `\x1b` et `\r` et **polluerait le
fichier avec ce qu'il décrit** (S3 a dû le refaire).

---

### T12 — La revue transverse de fin de branche — OBLIGATOIRE

**Objet.** Trouver les affirmations du dépôt **devenues fausses dans leur propre
branche**. Elles ont toutes la même forme : **correctes des deux côtés prises
séparément**, et **aucune revue par tâche ne peut structurellement les voir**.

**Le barème, et il monte** : **cinq** en D7, **trois** en D8, **six** en D9,
**douze** en D10, **sept** en D11, **huit** en P1, **dix** en P2, **cinq** en S1,
**neuf** sur le chantier E, **douze** en P3, **douze** en S2, **onze** en F1,
**huit** en P4, **treize** en S3. **Trouver moins que la moitié du dernier
relevé est un signe que la revue n'a pas eu lieu, pas que la branche est
propre.**

**Trois cibles sont NOMMÉES D'AVANCE, parce qu'elles sont déjà mesurées.**

**① Le compte des contrôles : « huit » devient « neuf ».**

🔴 **CE PLAN NE DONNE PAS LE NOMBRE DE PLACES À CORRIGER, ET C'EST DÉLIBÉRÉ.**
Le plan de S1 en annonçait **neuf** là où il y en avait **onze**, *et attribuait
l'écart à une cause fausse* ; ce qu'il aurait fallu faire est ce qui est prescrit
ici. **Deux relevés du 20 août 2026, et ils ne disent pas la même chose** :

```
$ grep -rn 'huit contrôle' client/src client/outils client/*.html | wc -l
10
$ grep -rn 'huit'          client/src client/outils client/*.html | wc -l
20
```

**DIX contre VINGT, et la moitié de l'écart n'est PAS un contrôle** : le mot
« huit » nomme aussi les **huit crans d'espacement** (`tokens.ts:129`, qui ne
doit **pas** bouger) et les **huit jugements humains du §8** — un **autre**
compte, faux pour une **autre** raison, puisqu'ils sont **quinze avant S4** et
que le §10 dit combien après. **La tâche énumère elle-même, verse son
énumération, et ne se fie à aucun nombre écrit ici.**

⚠️ **Et le balayage se fait par le SENS, pas par la formule** : une place peut
dire « les huit », « des huit », « HUIT contrôles », ou compter sans le mot.
**C'est celle qu'on n'a pas listée qui survit.**

⚠️ **Les « sept » de `verifier-design.mjs` et de `classes-employees.mjs` ne
bougent PAS** : ils parlent des **sept SCRIPTS** que l'agrégateur lance, et
**sept scripts, neuf contrôles** reste vrai. **C'est exactement le piège que S3 a
rencontré** — treize places à corriger et **quatre** où la même phrase était
juste.

**② « Aucun contrôle ne mesure une longueur » devient faux — et le nombre de
places DÉPEND DE LA FORMULE, ce qui est le piège lui-même.**

```
$ grep -rn 'mesure une longueur' client/src client/outils | wc -l
5
$ grep -rn 'longueur' client/src client/outils | grep -iE 'mesur|aucun' | wc -l
10
```

**CINQ contre DIX**, et le second relevé remonte des places que le premier
manque — `tokens.css:4`, `style.css:27`, `connexion.css:29`, `shell.css:45`,
`primitives.test.ts:189` et `:191`. **Balayer sur la CHOSE NIÉE — une mesure de
longueur — et non sur une tournure**, en énumérant les variantes : « aucun ne
mesure », « n'en mesure aucune », « aucune commande ne », « hors échelle ».
⚠️ **Toutes ne deviennent pas fausses** : celles qui parlent de la portée de
**G4** (« il ne dit pas que le BON token a été choisi ») restent **vraies**, et
§7.10 hérite exactement de la même limite (§8). **La tâche trie, elle ne
substitue pas.**

**③ La promesse de raccordement du micro** — `tokens.css:95-96` :
« Le raccordement sémantique du micro appartient au sous-bloc S4. »
🔴 **S4 NE LE FAIT PAS, ET LA PHRASE EST CORRIGÉE PLUTÔT QU'EXÉCUTÉE.** Raccorder
`--voile-micro-actif` à `--danger` ferait suivre au bouton **le thème du
produit** alors qu'il est posé sur une vidéo qui n'en suit aucun — **c'est mot
pour mot l'argument que le même encadré écrit six lignes plus haut** pour
justifier que les six voiles soient hors thème. **La phrase promettait ce que sa
propre page réfute.** Elle devient un constat : le micro reste hors thème, et
c'est sa raison.

**La méthode, et elle est PRESCRITE, pas suggérée :**

1. **Énumérer AVANT d'éditer** — `grep -n` sur chaque formule, sortie **versée**
   dans un journal, comme S3 l'a fait
   (`revue-transverse-enumeration.log`).
2. **Corriger UNE PAR UNE, PAR NUMÉRO DE LIGNE.** 🔴 **Jamais par substitution
   globale** : S3 a trouvé treize places à corriger **et quatre où la même
   phrase est juste** ; une substitution globale aurait abîmé les quatre.
3. **Relire place par place APRÈS l'édition.** « Une substitution qui ne dit pas
   combien d'occurrences elle a touchées est une affirmation de complétude non
   vérifiée » — c'est la doctrine que `CLAUDE.md` a payée sept fois.
4. **Balayer par le SENS, pas par la formule** : une négation se dit de
   plusieurs façons, et c'est celle qu'on n'a pas listée qui survit.
5. **Chercher aussi ce que ce plan-ci a rendu faux** : les documents de S1, S2 et
   S3 promettent chacun `--police-mono` à S4, et le §11 de S3 déclare que « c'est
   le dernier sous-bloc où [la fenêtre de session n'a pas bougé] sera vrai ».

⚠️ **LA REVUE TRANSVERSE EST ELLE-MÊME UNE SOURCE DE CROISSANCE** : celle de S2 a
fait tomber la marge de `primitives.test.ts` de 30 à 17, celle de S3 a ajouté
**+54 lignes de commentaire**. **Les tailles se relèvent APRÈS elle, jamais
avant** — une table relevée en début de ronde serait fausse à la fin de la même
ronde.

---

### T13 — `CLAUDE.md`

**Objet.** Une section `## 🎨 Sous-projet ⑥ Design system — sous-bloc S4 : la
fenêtre de session (20 août 2026)`, placée **après** celle de S3.

**Ce qu'elle porte, et rien de moins** : les familles de lecture des journaux
**mesurées** ; les sept critères avec leur **nombre d'exécutions** ; les trois
décisions du §3 et le défaut du §4 ; les rouges et **celles qui ont dû être
refaites** ; les pièges neufs ; le relevé de tailles **par la commande** ; ce que
S4 n'établit pas ; **et ce que ⑥ laisse ouvert après lui** (§14).

⚠️ **Les tailles sont relevées PAR LA COMMANDE, APRÈS la dernière édition de la
tâche 12**, jamais recopiées de ce plan ni du document de résultats :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

⚠️ **« Corrigé à sa place » est une affirmation de COMPLÉTUDE, et elle se vérifie
en énumérant les places AVANT de l'écrire** : `grep -n '<le nombre>' CLAUDE.md`.
Ce dépôt a payé sept fois le naufrage du « 487 », dont une fois **dans la vague
même qui le corrigeait ailleurs**.

---

### T14 — Le document de résultats

**Objet.** `docs/superpowers/plans/2026-08-20-design-system-s4-resultats.md`.

🔴 **LA PREUVE D'UNE AFFIRMATION NE VIT PAS DANS UN RAPPORT GITIGNORÉ.** D10 a
établi **par la commande** que l'espace de travail de D9 a disparu, emportant
**six** constats de revue définitivement perdus. **Tout ce que la recette relève
est versé dans git**, et ce document porte l'analyse.

Il porte, dans cet ordre : ce que S4 a fait et le fait qui gouverne ; les sept
critères avec leur nombre d'exécutions ; les trois décisions et le défaut de
contraste ; le contrat d'apparence **et tout changement hors contrat, déclaré** ;
les rouges, **dont celles qui ont dû être refaites et pourquoi** ; les tailles
**après** la revue ; **les jugements humains, avec leur total** ; ce que S4
n'établit pas ; **et ce que ⑥ laisse ouvert** — la liste étant, pour la seule
fois du sous-projet, **complète**.

---

## 7. Le graphe de dépendances

```
T1  (extraire le lecteur de CSS)
 ├──> T2  (§7.10, NAÎT ROUGE : 8 occurrences / 6 valeurs)
 │     ├──> T5 (−2 occurrences)   ┐
 │     ├──> T6 (−1 occurrence)    ├─> §7.10 VERT à la fin de T8
 │     ├──> T7 (−2 occurrences)   │
 │     └──> T8 (−3 occurrences)   ┘
 └──> T7, T9, T10  (les gardes réemploient le lecteur)

T3  (§7.9 ② A durcie, NAÎT ROUGE) ──> T5 (l'éteint)

T4  (--sur-voile, 53ᵉ paire) ──> T5 (la variante l'emploie)

T5 ──> T9  (les deux touchent client/index.html)
T7 ──> T10 (T7 crée client/src/style.test.ts, T10 y ajoute)
T9 ──> T10 (T10 pose l'env() dans la feuille que T9 crée)

T1..T10 ──> T11 (recette) ──> T12 (revue) ──> T13 (CLAUDE.md) ──> T14 (résultats)
```

**Chemin critique** : `T1 → T2 → T8` pour l'axe des longueurs,
`T4 → T5 → T9 → T10` pour l'axe de la fenêtre de session. **T3 et T6 sont
indépendants** de tout sauf de T5 (pour T3) et de T2 (pour T6).

⚠️ **T2 et T3 rendent l'arbre ROUGE pendant sept commits**, délibérément et
comme S1 et S3 l'ont fait. **`scripts/verify-all.sh` est rouge pendant cette
fenêtre**, et le document de résultats le déclare plutôt que de le taire.

---

## 8. Les contrôles après S4 — NEUF, dont deux tests unitaires

| # | Ce qu'il vérifie | Forme | État à la fin de S4 |
| --- | --- | --- | --- |
| §7.1 | les contrastes tiennent les seuils WCAG | script | **53** paires (52 + celle de `--sur-voile`) |
| §7.2 | aucune couleur littérale hors `tokens.css` | script | 0 |
| §7.3 | toute surface bâtie porte les tokens | script | 5 pages, A et B vertes |
| §7.4 | les trois blocs de thème ne divergent pas | script | **7** hors-thème nommés |
| §7.5 | la bascule de thème atteint les N fenêtres | **test unitaire** | inchangé |
| §7.6 | aucun token orphelin, aucun `var()` non déclaré | script | **liste d'attente VIDE** |
| §7.7 | le poids CSS ne dérive pas | script | sous 12 288, marge relevée |
| §7.9 | une primitive atteint le produit — **les TROIS surfaces** | script | durci par T3, éteint par T5 |
| **§7.10** | **aucune longueur hors token dans une feuille de surface** | **test unitaire** | **NEUF — addition de plan, comme §7.9** |

**Sept scripts, neuf contrôles.** `design:verifier` continue de dire `7/7` ;
`scripts/verify-all.sh` garde ses **dix-huit** en-têtes ; `npm test` en porte
**deux**. ⚠️ **Ni « sept » ni « neuf » ne se suffit sans dire lequel on
compte** — c'est la divergence D5 de S2, qui se rejoue à chaque addition.

⚠️ **§7.10 est une ADDITION DE PLAN, pas de la spécification**, exactement comme
§7.9 l'était pour S3. Il est **inscrit dans la spec** par la tâche 12, « parce
qu'un contrôle qui ne vit que dans un plan de sous-bloc se perd ».

⚠️ **Ce que §7.10 NE dit PAS, et il doit l'écrire lui-même** : *que le BON token
a été choisi.* C'est la limite exacte de G4, et elle ne bouge pas. `padding:
var(--e-8)` sur un bandeau serait vert et absurde. **Le bon emploi reste une
règle de revue**, comme celui de `--bord` contre `--bord-fort` (spec §4.5).

---

## 9. Le harnais des rouges — non négociable

🔴 **SUR SEIZE ROUGES DE S3, QUATRE NE PROUVAIENT RIEN — et deux d'entre elles ne
mutaient RIEN DU TOUT** (0 ligne de diff, `exit=0`). « Une rouge qui rougit pour
la mauvaise raison est **indiscernable d'une bonne** si l'on ne lit que son
`exit=1`. »

**Toute rouge de S4 suit cette séquence, sans exception :**

1. `sha256sum` du fichier **avant** ;
2. la mutation, **une seule à la fois** ;
3. 🔴 **PREUVE QUE LE DIFF EST NON VIDE** — `git diff --numstat <fichier>`, et
   **le journal la porte**. Une mutation à diff vide est **refusée**, elle n'est
   pas rejouée « pour voir » ;
4. le contrôle, et **on lit QUELLE assertion a rougi**, jamais seulement le code
   de sortie ;
5. `git checkout -- <fichier>` ;
6. `sha256sum` **après**, **identique** ;
7. `git status --porcelain client/` **vide**.

🔴 **`git checkout` NE RESTAURE PAS UN FICHIER NON SUIVI, ET IL EFFACE UN FICHIER
SUIVI NON COMMITÉ.** Toute rouge se joue donc **sur un arbre commité** : la tâche
commite d'abord, mute ensuite, et **la preuve de restauration est le `sha256`,
pas la confiance**.

🔴 **UN GARDE PEUT ÊTRE SATISFAIT PAR LE COMMENTAIRE DU FICHIER QU'IL ANALYSE —
trois occurrences dans ce sous-projet** (S1 sur `CLE_THEME`, S2 sur G1 et G5,
S3 sur sa rouge n°16). **Tout garde neuf de S4 blanchit les commentaires
d'abord**, ou ancre sur la **ligne de code entière** — jamais sur une
sous-chaîne. **Et le blanchiment s'éprouve DANS LES DEUX SENS**, comme S3 :
① une valeur interdite écrite dans un commentaire laisse **VERT** ;
② une déclaration écrite **seulement** dans un commentaire **ne compte pas comme
déclarée**.

🔴 **UN CONTRÔLE D'ABSENCE EST VERT SUR UN FICHIER VIDE.** Chaque garde neuf
porte son **assertion d'atteignabilité**, et **sa rouge se joue en VIDANT le
fichier**, pas en y ajoutant quelque chose. C'est G5, et sans lui « quatre gardes
sur cinq ne prouveraient rien » (mesuré par S2).

⚠️ **Vitest court-circuite le CSS par défaut, `?raw` compris** : un
`import css from './x.css?raw'` rendrait la chaîne **vide**, et un garde qui la
parserait **passerait au vert en ne mesurant rien**. `test: { css: true }` est
posé dans `client/vite.config.ts`, **et il n'y a délibérément PAS de
`client/vitest.config.ts`**, qui prendrait le pas sur lui **sans rien dire**.
**Ne pas le déplacer.**

---

## 10. Les jugements humains que S4 ajoute — et le total

**Quinze à la fin de S3** : huit de la spec §8, trois de S2, quatre de S3.
**S4 en ajoute SEPT**, et **aucun des vingt-deux ne deviendra une mesure.**

| # | Ce qui n'est pas mesuré | Ce qui est mesuré à la place |
| --- | --- | --- |
| 16 | que la **pile monospace** de `#stats` se lise mieux que le crénage qu'elle remplace | rien. La spec §4.3 la **désigne** ; elle ne la mesure pas |
| 17 | que **20 px** soit la bonne taille des deux boutons de coin | qu'elle passe par un cran de l'échelle (**§7.10**), et que `pointer-events: none` survive (garde T7). **Pas la zone occupée** |
| 18 | que `#e6e8eb` soit la **bonne encre** sur un voile | **son contraste sur la bande noire** (§7.1, paire neuve). ⚠️ Rien au-dessus de l'image |
| 19 | qu'un **écran plein cadre** soit la bonne forme pour un état terminal, contre un bandeau | rien. C'est la décision de la spec §5.2, et c'est un goût |
| 20 | que cet écran doive rester **SANS ACTION** | rien |
| 21 | que les **trois mesures de contenant** méritent des tokens plutôt que de rester littérales | qu'aucune échelle du §4.4 ne les couvre (**relevé**), et que leur valeur ne change pas |
| 22 | que le **rayon** de `#status`/`#stats` passant de 6 à 4 px soit acceptable | rien. Déclaré au contrat §5, non compensé |

⚠️ **Ce total de vingt-deux est une PRÉDICTION de ce plan.** Le document de
résultats **relève** le nombre réel : une tâche qui prend une décision
esthétique non prévue ici en ajoute un, et **le taire serait le déguiser en
mesure**.

🔴 **ET LE JUGEMENT VISUEL N'A JAMAIS ÉTÉ PORTÉ SUR CE SOUS-PROJET — AUCUNE PAGE
DE ⑥ N'A ÉTÉ OUVERTE DANS UN NAVIGATEUR, EN S1, EN S2 NI EN S3.** S4 **ne le
portera pas non plus** : ce n'est pas un critère, et un agent qui prendrait une
capture d'écran ne porterait pas un jugement — **il produirait une image que
personne n'a regardée**. **S4 est la dernière occasion de ⑥ de le porter, et il
la laisse passer en le déclarant.**

---

## 11. Ce que S4 n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, contrôles
  **déterministes** : reproductibilité, rien de plus. **Et cette question ne doit
  pas être empruntée** à une campagne qui, elle, l'aurait posée.
- 🔴 **Rien du Window Controls Overlay en fonctionnement.** Aucun manifeste
  n'existe (§2.4, mesuré), donc **aucun état où la règle agit n'est
  atteignable**. Le garde prouve qu'elle est **inerte aujourd'hui**, et **rien
  d'autre**. **Aucun critère de recette ne la couvre**, délibérément.
- 🔴 **Aucun jugement visuel**, et les **vingt-deux** jugements humains attendent
  tous un œil.
- **La lisibilité des voiles sur une vidéo quelconque** reste non mesurée —
  seule la bande noire du `letterbox` l'est (§4). La réserve du §11 de la spec
  tient entière, et **la composition alpha n'est pas outillée**.
- **La zone réellement occupée par les deux boutons de coin** : rien n'est
  mesuré, l'avance du glyphe reste inconnue, et le commentaire cesse de la
  chiffrer.
- **Que `.message--flottant` reste lisible** au-dessus d'une image claire : même
  raison, et c'est la ligne « `--voile-flottant` ne suffit pas sur une vidéo très
  claire » du §11 de la spec.
- **L'anneau de focus reste vérifié NON EFFACÉ, jamais VISIBLE** — legs de S2 et
  de S3, non levé.
- **L'accessibilité au-delà du contraste et du mouvement réduit** : clavier
  complet, lecteurs d'écran, cibles tactiles, ordre de tabulation. L'écran
  terminal porte `role="status"` sur son message, **et rien de plus n'est
  éprouvé**.
- **Rien hors d'un Chromium de bureau** : ni Firefox, ni Safari, ni mobile,
  **ni HiDPI** (`deviceScaleFactor` = 1 partout).
- **Aucune internationalisation** : rien ne dit qu'un écran terminal survit à un
  motif d'erreur plus long dans une autre langue.
- **Le legacy n'est pas touché**, et **aucun contrôle ne le balaie** : deux
  directions visuelles coexistent dans le dépôt jusqu'au remplacement.
- **La bascule de thème entre deux fenêtres RÉELLES du produit** — une
  page-shell et les N sessions qu'elle ouvre par `window.open` — n'est toujours
  pas éprouvée.
- **Le plafond de 12 288 octets n'est calibré par rien**, et il le reste.
- **`galerie.ts` et `galerie-primitives.ts` n'ont toujours aucun test.**

---

## 12. Les risques

| Risque | Mitigation, ou déclaration |
| --- | --- |
| **§7.10 rouge pendant sept commits rend `verify-all.sh` rouge, et un chantier voisin l'impute à lui** | Déclaré ici, dans le message de commit de T2, et dans le document de résultats. C'est ce que S1 et S3 ont fait de leurs contrôles nés rouges |
| **`.message` sur un bandeau flottant met un fond opaque et clair sur la vidéo** | C'est la tension du §T5, et la variante `--flottant` la résout. ⚠️ **Si l'exécution trouve mieux, elle diverge et le déclare** |
| **La variante flottante crée par mégarde une cinquième famille** | Elle ne crée **aucun fichier** dans `client/src/design/primitives/` — les familles sont dérivées des fichiers. Nommé dans T5 |
| **T8 change l'apparence de deux surfaces closes sans que personne ne le voie** | Substitution valeur → token, **à valeur strictement égale**. ⚠️ **C'est un argument, pas une mesure** ; les poids bâtis, eux, changent et sont relevés |
| **L'écran terminal se lève à tort et masque une session vivante** | Le test du **sens inverse** (T9, rouge n°1) : un message ordinaire ne le lève pas. C'est le sens qui compte |
| **`[hidden]` perd contre `.ecran` et l'écran est visible dès le chargement** | La règle explicite et son garde (T9, T10 assertion ③) |
| **La recette mesure un `dist/` périmé** | `design:verifier` bâtit d'abord (`verifier-design.mjs:24-27`), et la recette **relève le hachage des cinq actifs** |
| **`amorce-theme.js` part verbatim et fait différer les cinq pages bâties** | Piège de S3, nommé dans T11. **À savoir avant de comparer des actifs** |
| **Un journal se pollue lui-même** (`echo` en zsh interprète `\x1b` et `\r`) | Heredoc entre quotes, exigé dans T11 |
| **Un hook `chpwd` injecte un `ls` dans chaque journal** | `unset -f chpwd` avant toute collecte, et **aucun `cd` dans un sous-shell** |
| **Une rouge ne perturbe rien et se lit comme un contrôle qui ne mord pas** | Le harnais du §9, étape 3, **obligatoire** |
| **L'arbre est partagé et bouge pendant la recette** | La base du critère ③ est **le parent du premier commit de S4**, jamais une date. Toute mesure reprise l'est **entièrement**, jamais recopiée |
| **La VM est occupée par la recette de G1** | ⛔ **S4 n'en a aucun besoin** : ⑥ est un sous-projet navigateur (spec §9) |

---

## 13. Les tailles prévues, et les marges à surveiller

**Relevé du 20 août 2026, par la commande** (§2.6). **Ce tableau est une
PRÉVISION** ; le relevé qui fait foi est celui de la tâche 13, pris **après** la
revue transverse.

| Fichier | Aujourd'hui | Porte | Ce que S4 lui fait |
| --- | --- | --- | --- |
| `client/src/design/primitives.test.ts` | **283** | 300 (marge **17**) | **MAIGRIT** (T1) — c'est la raison d'être de T1 |
| `client/outils/tokens-orphelins/attente.mjs` | **227** | seuil d'extraction **240** (marge **13**) | **MAIGRIT** (T6) — la doctrine part avec sa donnée |
| `client/src/style.css` | **184** | 300 | +/− ; l'écran terminal est **extrait d'emblée** (T9) |
| `client/src/design/tokens.css` | **232** | 300 | +4 tokens et leurs commentaires (T4, T8) — **à surveiller** |
| `client/src/main.ts` | **451** | ⚠️ **déjà au-dessus de la porte de ⑥, et pas de ⑥** | **au plus dix lignes** (T9), mesurées avant et après ; au-delà, **extraire** |
| `client/verify-webrtc.mjs` | **494** | 500 (marge **6**) | ⛔ **INTOUCHÉ**, comme en S1, S2 et S3 |
| `client/src/design/css.ts` | — | 300 | **neuf** (T1) |
| `client/src/design/longueurs.test.ts` | — | 300 | **neuf** (T2) |
| `client/src/style.test.ts` | — | 300 | **neuf** (T7, grossi par T9 et T10) |
| `client/src/ecran-terminal.ts` + `.test.ts` | — | 300 | **neufs** (T9) |
| `client/src/session/etat-terminal.css` | — | 300 | **neuf** (T9) — le point de chute que la spec §10 nomme |

🔴 **AUCUN FICHIER NEUF NE NAÎT AU-DESSUS DE 300**, et **toute addition
substantielle à un fichier proche de sa porte s'accompagne d'une EXTRACTION,
jamais d'une compression** : ce dépôt a franchi le plafond trois fois en D10 et
deux fois en D9, et l'a rattrapé **après**, dont deux fois par une compression
qu'il interdit nommément.

⚠️ **UNE ADDITION DE COMMENTAIRE PEUT ANNULER UNE EXTRACTION, et S2 l'a payé DEUX
fois dans le même fichier.** T1 rend de la marge à `primitives.test.ts` ; **la
revue transverse la reprendra en partie** — c'est ce qui est arrivé en S2 (30 →
17) et en S3 (+54 lignes). **Le relevé de T13 est celui d'APRÈS.**

---

## 14. Ce que ⑥ laisse ouvert APRÈS S4 — la liste complète

**S4 étant le dernier sous-bloc, rien de ce qui suit n'a de destinataire dans
⑥.** Chacun porte, quand il en a un, le chantier qui pourrait le prendre.

**Ce que S4 solde** (et que cette liste ne porte donc plus) : `--police-mono`,
les six longueurs hors échelle et la clause §8, l'écran terminal, la reprise des
bandeaux sur les primitives, le raccordement sémantique du micro (**tranché
non**, §T12 ③), et le défaut d'encre du thème clair (§4).

**Ce qui reste :**

1. ⛔ **Le WCO n'a jamais été rendu.** La règle est livrée et prouvée **inerte** ;
   son comportement est inconnu. **Destinataire nommé : la recette de ④ G5**
   (`2026-08-19-gestion-apps-design.md:760`), qui pose le manifeste — elle doit
   regarder la fenêtre de session sous une barre de titre superposée.
2. ⛔ **Le hub.** Il n'existe pas, son contenu dépend de ④, et **⑥ ne le livre
   pas** (spec §6, §9). Les tokens et les primitives l'attendent.
3. ⛔ **Aucune primitive « lien », aucune ancre** dans aucune entrée. Poser une
   famille sans appelant serait le code mort que §7.6 refuse — **et ce sera
   toujours vrai tant qu'aucune surface n'aura de lien**.
4. ⛔ **`galerie.ts` et `galerie-primitives.ts` sans test.** §7.9 ③ B attrape
   « une famille cesse d'être rendue » ; **il n'attrape pas un module de galerie
   cassé**.
5. ⛔ **Le legs n°9 de S2** : `lireBlocsDeTheme` ne sait nommer que **trois**
   blocs, donc `tokens.css` ne peut accueillir aucune autre requête média. Le
   jour où un chantier en voudra une, **c'est le lecteur qui doit apprendre à
   nommer ses blocs**, pas la règle qui doit se contorsionner.
6. ⛔ **Le plafond de poids CSS n'est calibré par rien** — « un garde-fou contre
   une addition massive, pas une cible de budget » (spec §7.7).
7. ⛔ **Le sens « toute classe déclarée est employée » de §7.9 n'existe pas.** Il
   exigerait une seconde liste d'attente. **C'est `primitives.html` et l'œil qui
   le tiennent — et l'œil n'est pas passé.**
8. ⛔ **`Ton` et `CLASSE_DE_TON` sont dupliqués** entre `client/src/shell.ts:23`,
   `client/src/connexion.ts:98` et le module de l'écran terminal. **S4 ne les
   unifie pas** (T9), et le point de chute d'une unification serait la couche
   `design/`.
9. ⛔ **La lisibilité d'un voile sur une vidéo quelconque n'est pas outillée.**
   La composition alpha est un calcul **pur**, donc à portée de
   `client/src/design/contraste.ts` ; S4 ne l'écrit pas, et mesure seulement la
   bande noire.
10. ⛔ **L'anneau de focus n'a jamais été vu VISIBLE**, en particulier sur un
    `.bouton--principal` dont le fond est `--accent`.
11. ⛔ **`prefers-reduced-motion` est le seul des quatre manques d'accessibilité
    qui soit pris.** Clavier complet, lecteurs d'écran et cibles tactiles ne le
    sont pas.
12. ⛔ **`client/verify-webrtc.mjs` est à 494 pour une porte à 500 — marge 6**, et
    **la divergence de convention que D10 a signalée n'a jamais été tranchée** :
    le § « Portée » de `CLAUDE.md` ne liste que `client/src/`, la commande de
    vérification l'attrape quand même. **C'est une décision de convention, et
    elle appartient au propriétaire du dépôt.**
13. ⛔ **Le défaut à deux réglages de `build-agent.sh` / `run-agent.sh`** ne
    concerne pas ⑥ — ses journaux sont propres — mais il reste **non corrigé**
    pour les chantiers qui passent par la VM.
14. 🔴 **AUCUN JUGEMENT VISUEL N'A ÉTÉ PORTÉ SUR ⑥, D'UN BOUT À L'AUTRE.** Les
    **vingt-deux** jugements humains attendent tous un œil, et **le sous-projet
    se termine sans qu'aucune de ses pages ait été ouverte dans un navigateur**.
    ⚠️ **Ce n'est pas une lacune d'exécution : c'est la conséquence assumée du
    §7.8**, qui écarte la comparaison d'images parce que les polices système
    rendent différemment d'une machine à l'autre. **La galerie existe pour cela,
    et personne ne l'a regardée.**
