# Sous-projet ⑥ Design system — sous-bloc S4 : la fenêtre de session

**20 août 2026.** Dernier sous-bloc de ⑥.

Plan : `docs/superpowers/plans/2026-08-20-design-system-s4.md` (`4038a88`).
Spécification : `docs/superpowers/specs/2026-08-19-design-system-design.md`.
Journaux : `docs/superpowers/plans/journaux-design-s4/` — **une seule famille de
lecture**, UTF-8 valide, ni ANSI, ni `\r`, ni octet NUL, mesurée après la
dernière écriture (`familles-de-lecture.txt`).

🔴 **LA PREUVE D'UNE AFFIRMATION NE VIT PAS DANS UN RAPPORT GITIGNORÉ.** D10 a
établi par la commande que l'espace de travail de D9 avait disparu, emportant six
constats de revue définitivement perdus. Tout ce que la recette relève est versé
dans git ; ce document porte l'analyse.

---

## 1. Ce que S4 a fait, et le fait qui gouverne

S4 reprend la fenêtre de session — la seule surface que ⑥ n'avait jamais
touchée — et solde les trois questions que la spécification lui laissait.

**Le fait qui gouverne le sous-bloc n'est pas dans son plan : il a été trouvé en
lisant.** Sous le thème clair, les éléments de la fenêtre de session écrivaient
**du quasi-noir sur un voile quasi-noir**. `base.css` pose
`color: var(--texte-fort)` sur `body` ; en clair `--texte-fort` vaut `#10131a` ;
et les six voiles sont **hors thème**, donc noirs dans les deux. Ce n'est pas une
régression du produit d'origine, c'est un **effet de bord de S1** : avant lui,
`style.css` posait `color-scheme: dark` en dur et une encre unique — la fenêtre
de session **n'avait pas de thème clair**. S1 lui en a donné un, et rien n'a
remarqué que les voiles, eux, ne suivaient pas.

**Aucun des contrôles ne pouvait le voir**, et pour une raison écrite : les six
voiles sont hors des paires de contraste, « leur lisibilité dépend de la vidéo
qui est dessous, qui n'est pas connaissable ». **L'argument est juste pour le
voile ; il ne l'est pas pour l'encre**, qui, elle, est parfaitement connaissable.
Le remède est un septième token hors thème, `--sur-voile: #e6e8eb`, et une
**53ᵉ** paire de contraste — `--sur-voile` sur `--video-letterbox`, la **seule**
région où le fond sous l'encre soit connu : les bandes que laisse
`object-fit: contain`.

---

## 2. Les sept critères, avec le nombre d'exécutions dans chaque énoncé

⚠️ **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les neuf
contrôles de ⑥ sont **déterministes** (spec §9) : la question « combien de fois
sur combien » ne se pose pas ici, et **elle ne doit pas être empruntée** à une
campagne qui, elle, l'aurait posée.

Journaux : `recette-1.log`, `recette-2.log`. **Une troisième exécution**
(`recette-3-apres-revue.log`) a été prise **après la revue transverse**, et elle
ne sert qu'à dire ce que la revue a changé aux actifs bâtis (§6).

| # | Critère | Verdict | Exéc. | Le chiffre, **relevé** |
| --- | --- | --- | --- | --- |
| ① | les **NEUF** contrôles sont verts | **TENU** | 2 | `design:verifier` **7/7**, `exit=0` ; **279** tests sur **30** fichiers, dont §7.5 (`selecteur-theme.test.ts`, 8 tests) et §7.10 (`longueurs.test.ts`, 2 tests) |
| ② | la liste d'attente est **VIDE** | **TENU** | 2 | **52** tokens déclarés, **52** employés — **ÉGAUX** ; `0 orphelin(s), dont 0 en attente déclarée` ; `0 écart` |
| ③ | 🔴 la fenêtre de session **A BOUGÉ** | **TENU** | 2 | `git diff --stat` contre la base : **182 insertions / 43 suppressions** sur `index.html` et `style.css` ; **les CINQ** actifs CSS changent de hachage, et `dist/index.html` aussi |
| ④ | la fenêtre de session emploie des primitives | **TENU** | 2 | `client/index.html : message, surface` ; **les TROIS** surfaces couvertes (`shell.html` : bouton, message, surface ; `connexion.html` : bouton, champ, message, surface) ; `0 écart` |
| ⑤ | 🔴 plus aucune longueur hors token | **TENU** | 2 | §7.10 → **0 occurrence, 0 valeur**, contre **8 occurrences / 6 valeurs** à la naissance du contrôle |
| ⑥ | les contrastes | **TENU** | 2 | **53** paires, **0** échec, **minimum global 3,16** — inchangé |
| ⑦ | le poids CSS sous le plafond | **TENU** | 2 | **8 616** octets pour un plafond de **12 288**, marge **3 672** — contre **8 011** à la base |
| — | le **jugement visuel** | ⛔ **NON PORTÉ** | 0 | voir §8 |

**Le détail du critère ⑦, par feuille** (relevé) : `connexion` 563, `main` 1 849,
`primitives` 2 830, `shell` 1 098, `socle` 2 276.

**Le montage du critère ③.** La base est le **parent du premier commit de S4**
(`23e9b89`), **jamais une date** — S3 avait dû corriger son plan sur ce point,
des chantiers voisins ayant touché `client/` entre les deux. `git archive` vers
un arbre jetable **hors du dépôt**, `node_modules` lié depuis `client/`,
`npx vite build`, comparaison des `sha256`.

🔵 **Et le montage sait voir une IDENTITÉ, pas seulement une différence.** Les
cinq actifs sont énumérés, chacun avec **la raison de son changement** :

| Actif | Change ? | Pourquoi |
| --- | --- | --- |
| `socle-*.css` | **oui** | `tokens.css` : `--sur-voile` (T4) et les trois tokens de contenant (T8) |
| `primitives-*.css` | **oui** | la variante `.message--flottant` (T5) |
| `main-*.css` | **oui** | `style.css` (T5, T6, T7, T10) et l'`@import` de l'écran terminal (T9) — 1 493 → 1 849 octets |
| `shell-*.css` | **oui** | les mesures de contenant deviennent des tokens (T8) |
| `connexion-*.css` | **oui** | idem (T8) |
| `dist/index.html` | **oui** | le balisage `#fin` (T9) |

**Un actif qui ne changerait sur rien ne prouverait rien** : ici les cinq
changent, et T8 est précisément la tâche qui fait bouger les deux surfaces
**closes**, ce que rien d'autre n'aurait fait.

⚠️ **`client/src/design/amorce-theme.js` part VERBATIM dans chaque page bâtie**,
commentaires compris — piège hérité de S3. **Il n'a PAS été touché par les tâches
1 à 11** (vérifié : `git diff --stat 23e9b89 HEAD` vide sur ce fichier au moment
de la recette). **La revue transverse l'a touché**, et la troisième exécution en
relève la conséquence exacte (§6).

⚠️ **Les deux exécutions ne diffèrent que par l'ORDRE d'arrivée de deux lignes de
`vitest`** (fichiers de test exécutés en parallèle). **Aucun nombre ne change** ;
un `diff` qui neutralise les horodatages le montre.

---

## 3. Les trois décisions du sous-bloc

### 3.1 `--police-mono` est câblé, et la liste d'attente devient VIDE

Le token était déclaré depuis S1 avec **un seul appelant prévu** — `#stats` —, et
**trois sous-blocs se sont passé la décision** « le câbler ou le retirer », faute
d'avoir le droit de changer l'apparence de cette surface. S4 l'a, et il câble :
la spec §4.3 **désigne** ce bandeau, qui « affiche des nombres qui changent » et
porte déjà `font-variant-numeric` pour la même raison. Le retirer aurait emporté
ses deux appelants de galerie et fait perdre au design system sa pile monospace
**entière**, pour vider une ligne de liste.

**La liste vide NE DISPARAÎT PAS**, et l'énoncé de S1 qui le promettait — « le
jour où elle est vide, tout ce bloc disparaît avec elle » — est **corrigé plutôt
qu'exécuté** : supprimer le fichier supprimerait l'**ÉGALITÉ** de §7.6, celle qui
fait rougir `NOUVEL ORPHELIN` pour tout token futur. **Une `Map` vide est ce qui
rend ce contrôle strict.**

⚠️ **Que la pile monospace se lise mieux que le crénage qu'elle remplace est un
JUGEMENT HUMAIN.** Le crénage `0.02em` est parti avec, et ce n'est pas un tour de
passe-passe : les deux servent la **même fin** — lire des nombres qui changent —
et la spec désigne la pile comme le moyen.

### 3.2 Les six longueurs hors échelle tombent, et la clause devient un CONTRÔLE

La clause « aucune longueur hors échelle » du §8 de la spec était une **dette
d'énoncé** : fausse depuis S1, nommée comme telle, et **invérifiable** — « aucun
des huit contrôles ne mesure une longueur » était écrit à **cinq** endroits du
dépôt sous cette formule exacte.

S4 livre **§7.10** (`client/src/design/longueurs.test.ts`), **le premier contrôle
de ⑥ qui en mesure une**. Il est **né ROUGE sur l'arbre intact — 8 occurrences
pour 6 valeurs** —, et c'est sa preuve d'atteignabilité. **Les deux nombres,
jamais un seul** : ils sont vrais de choses différentes, un contrôle ne pouvant
pas dédupliquer sans décider que deux `6px` écrits à deux endroits sont le même.

**Sa portée est DÉRIVÉE, jamais énumérée** — toutes les `*.css` de `client/src/`
hors `client/src/design/`. Deux feuilles neuves y sont entrées **sans qu'une
ligne du contrôle ne change** : `session/etat-terminal.css` (T9) et
`session/boutons-de-coin.css` (revue transverse). Le relevé passe de **4** à
**5** feuilles de surface entre la première recette et la troisième.

### 3.3 🔴 Le WCO : la règle est livrée, et AUCUN critère de recette ne la couvre

**Le fait qui gouverne : il n'existe aucun manifeste dans ce dépôt.** Sans
`display_override: ["window-controls-overlay"]`, les variables `titlebar-area-*`
ne sont **jamais définies** ; il n'y a donc **aucun état atteignable** dans lequel
la règle agisse. Un critère de recette qui prétendrait l'exercer serait vacueux
**par construction**, et pas faute d'effort — **et un critère vacueux est pire
qu'un critère absent : il se lit comme une preuve.**

Ce qui est livré à la place est un **garde de forme** (`client/src/style.test.ts`)
à trois assertions :

- ① tout `env(titlebar-area-*)` porte le repli `0px` ;
- ② **aucune** `@media (display-mode: window-controls-overlay)` — parce qu'un
  repli neutralise un `env()`, mais que **rien ne neutralise un bloc `@media`** :
  une règle conditionnelle écrite ainsi changerait la mise en page **d'aujourd'hui**
  sans qu'aucune commande ne le dise ;
- ③ **atteignabilité** — sans un `env(titlebar-area-` à lire, ① et ② sont vertes
  en ne mesurant rien.

**La rouge de ce garde a une conséquence RÉELLE sur le produit d'aujourd'hui** :
écrire `env(titlebar-area-height, 8px)` descend le bandeau de 8 px **maintenant**,
sur toutes les sessions. **Ce n'est donc pas un contrôle qui valide sa propre
écriture.**

🔴 **CE GARDE PROUVE L'INERTIE, JAMAIS LE COMPORTEMENT.** La règle n'a **jamais**
été rendue dans une fenêtre à barre de titre superposée. **Le destinataire du legs
est nommé : la recette du sous-bloc G5 de la gestion d'apps**, celui qui pose le
manifeste — c'est à elle de regarder la fenêtre de session sous une barre
superposée.

---

## 4. Le contrat d'apparence, et ce qui en sort

Les neuf lignes du contrat du plan sont livrées. **Deux changements hors
contrat**, tous deux déclarés :

| # | Surface | Ce qui change | Au contrat ? |
| --- | --- | --- | --- |
| ① | `#status`, `#stats` | remplissage 6 → 8 px, rayon `--r-2` → `--r-1`, bordure présente mais **transparente** | oui |
| ② | `#stats` | un bandeau vide **disparaît** au lieu de rester un cadre | oui |
| ③ | `#stats` | pile monospace, crénage retiré | oui |
| ④ | `#fullscreen`, `#micro` | glyphe 18 → 20 px | oui |
| ⑤ | les éléments porteurs d'encre | l'encre cesse de suivre le thème : **rien ne change en SOMBRE**, un défaut est réparé en CLAIR | oui |
| ⑥ | la fenêtre de session | un état terminal ouvre un **écran plein cadre** | oui |
| ⑦ | sous WCO seulement | `#status` et l'écran descendent sous la barre — **inatteignable** | oui |
| ⑧ | `shell.html`, `connexion.html` | **RIEN** : T8 ne substitue que des valeurs identiques | oui |
| ⑨ | `primitives.html` | une variante de plus est rendue | oui |
| ⑩ | `primitives.html` | ⚠️ **HORS CONTRAT** : une classe `.damier` en `<style>` en ligne, pour que la variante flottante se lise **sur une image dont on ne sait rien**. Duplique le damier de `design.html` — duplication assumée, la mise en page d'une page de démonstration n'est pas une primitive | **non** |
| ⑪ | l'écran terminal | ⚠️ **HORS CONTRAT** : son titre est **dérivé du TON**, pas figé dans le HTML. Un titre statique serait **faux de l'un des deux cas** — « Session terminée » ment sur un échec où aucune session n'a commencé | **non** |

⚠️ **Le rayon de ① n'est pas compensé, et c'est délibéré.** Le compenser par une
règle d'`ID` qui écrase la primitive recréerait exactement la divergence que le
design system existe pour empêcher. **Le déclarer coûte une ligne ; le compenser
coûterait la règle.**

⚠️ **Et la reprise des bandeaux a demandé une VARIANTE, pas seulement une
classe** — c'est le fait de conception que la spécification ne voyait pas. Son §6
demande la reprise sur `.message` ; son §5.2 exige `--voile-flottant` sur ces
mêmes bandeaux. Or `.message` pose `background: var(--fond-1)`, **fond opaque qui
suit le thème** : posée nue, elle mettrait en thème clair un panneau clair et
opaque au-dessus de la vidéo, exactement ce que `tokens.css` interdit en une
phrase. **Les deux exigences de la spec se contredisaient**, et
`.message--flottant` les tient **toutes les deux** — résoudre, jamais abandonner
l'une des deux.

---

## 5. Les rouges

**Toute rouge suit le harnais du §9 du plan, en sept étapes**, dont l'étape ③ est
la **preuve que le diff est non vide** : sur seize rouges de S3, **quatre ne
prouvaient rien, et deux ne mutaient RIEN DU TOUT**. « Une rouge qui rougit pour
la mauvaise raison est indiscernable d'une bonne si l'on ne lit que son
`exit=1`. »

**Les rouges des tâches 9, 10 et 12, versées** (`rouges-t9.log`,
`rouges-t10.log`, `rouges-t12.log`), **toutes avec leur `git diff --numstat` non
vide et leur `sha256` identique après restauration** :

| Rouge | Mutation | Diff | Ce que la sortie a dit |
| --- | --- | --- | --- |
| T9-1, **sens inverse** | l'écran se lève sur un message ordinaire | 1/1 | tests ② **et** ③ tombent ; le test ① **passe** — c'est ce qui montre que le sens qui compte est l'inverse |
| T9-2 | retirer `.ecran[hidden] { display: none }` | 0/3 | « `session/etat-terminal.css` ne déclare pas `.ecran[hidden] { display: none }` » |
| T9-3 | retirer l'écriture du texte | 0/1 | tests ① et ⑤ tombent **en nommant le texte attendu**, pas seulement la visibilité |
| T10-① | repli à `8px` | 1/1 | « un `env(titlebar-area-*)` sans le repli `, 0px` change la mise en page AUJOURD'HUI » |
| T10-② | ajouter la requête média | 6/0 | la requête **nommée avec son fichier** |
| T10-③a | vider **une** feuille | 0/299 | ⚠️ **③ reste VERTE, à juste titre** — 1 `env()` subsiste. C'est la contre-épreuve du défaut de portée |
| T10-③b | vider **les deux** porteuses | 0/379 | ③ tombe : « aucun `env(titlebar-area-)` dans `client/src/` » |
| T10-blanchiment A | `env(…, 8px)` **dans un commentaire** | 3/0 | ⚠️ **VERT**, comme attendu |
| T10-blanchiment B | les deux déclarations **commentées** | 2/2 | ③ tombe : un commentaire **ne déclare pas** |
| T12 | vider `session/boutons-de-coin.css` | 0/138 | les **deux** assertions tombent — le garde ① **et** son atteignabilité neuve |

**Le blanchiment est éprouvé DANS LES DEUX SENS**, comme S3 l'exigeait : une
valeur interdite écrite en commentaire laisse **vert**, et une déclaration écrite
**seulement** en commentaire **ne compte pas comme déclarée**.

⚠️ **AUCUNE ROUGE N'A DÛ ÊTRE REFAITE** sur les tâches 9 à 12.

❌ **CE QUE CE DOCUMENT NE PEUT PAS ÉTAYER : les rouges des tâches 1 à 8.** Elles
ne sont rapportées que par leurs **messages de commit** — en git, donc permanents,
mais ce ne sont pas des journaux —, et **deux de ces huit commits (`aa27fb8`,
`1c05dde`) ne contiennent pas même le mot « rouge »**. Constat, pas reproche : le
plan prescrivait de verser les journaux à la tâche 11, et les tâches antérieures
n'en avaient pas la consigne. **La leçon, pour un plan suivant : la consigne de
verser doit valoir à la tâche qui joue la rouge, pas à celle qui recette.**

---

## 6. La revue transverse — vingt-sept affirmations, et un plafond franchi

Barème du dépôt : **cinq** en D7, trois en D8, six en D9, douze en D10, sept en
D11, huit en P1, dix en P2, cinq en S1, neuf sur le chantier E, douze en P3,
douze en S2, onze en F1, huit en P4, **treize** en S3, huit en G1.

**La méthode est prescrite, et elle a été suivie** : énumérer **avant** d'éditer
(journal versé, `revue-transverse-enumeration.log`), corriger **une par une, par
son texte exact, avec assertion d'unicité**, relire **place par place après**, et
balayer **par le SENS, pas par la formule**.

### 🔴 Le fait le plus net : trois affirmations déjà fausses le jour où elles ont été écrites

Les **trois** « aucun des huit contrôles » que la **tâche 4** a posés
(`design/tokens.css`, `style.css`, `design/contraste.ts`) **étaient faux au moment
même de leur écriture**. Le neuvième contrôle, §7.10, est né à la **tâche 2**
(`ee56e1e`), et `git merge-base --is-ancestor` établit qu'elle **précède** la
tâche 4 (`fb629ea`). **Une tâche a décrit la suite de contrôles telle qu'elle
était avant la tâche qui l'avait déjà changée, deux commits plus tôt, dans la même
branche.** C'est la forme exacte que la revue transverse existe pour attraper :
chaque tâche était correcte de ce qu'elle voyait.

### Le tri compte autant que les corrections

`grep -rniE 'huit contrôles?'` rendait **17** places — dont **deux** que le `grep`
sensible à la casse manquait (`contraste.ts`, `galerie.ts`), ce qui est la leçon
« balayer par le sens » sous sa forme la plus littérale.

- **QUINZE** étaient fausses **au présent** et sont corrigées ;
- **TROIS** sont des **citations en style direct** (« … » *était écrit*) et
  **restent justes** ;
- **NEUF autres emplois du mot « huit » nomment un AUTRE compte** — les huit crans
  d'espacement, les huit jugements humains du §8, les dix-huit tokens, huit
  fenêtres, huit occurrences —, **tous vérifiés INTACTS après coup**. Une
  substitution globale les aurait abîmés, comme S3 l'avait déjà mesuré.

**Sept d'entre elles étaient fausses EN SUBSTANCE**, pas seulement de compte :
« aucun ne mesure une longueur » ne l'est plus, et **la raison qui laisse
`client/src/design/` découvert est désormais une PORTÉE** — §7.10 l'exclut, G4 y
garde les quatre familles — **et non une absence de contrôle**.

### Ce qui a été tranché plutôt qu'exécuté

« Le raccordement sémantique du micro appartient au sous-bloc S4 » (écrit par S1).
**S4 le corrige, il ne l'exécute pas** : raccorder `--voile-micro-actif` à
`--danger` ferait suivre au bouton **le thème du produit** alors qu'il est posé
sur une vidéo qui n'en suit aucun — **mot pour mot l'argument que la même page
emploie six lignes plus haut** pour tenir les six voiles hors thème. **La phrase
promettait ce que sa propre page réfute.**

### La spécification est reprise

§7.10 **y est inscrit** (« parce qu'un contrôle qui ne vit que dans un plan de
sous-bloc se perd ») ; la spec passe de **huit à NEUF** contrôles et dit
désormais **sept scripts, neuf contrôles** ; la clause §8 « aucune longueur hors
échelle » **cesse d'être fausse** et devient mesurée ; la **réserve de portée** de
`--police-mono` est **levée** ; deux prévisions de son §10 et de son §5.2 sont
marquées **faites** ; et **deux numéros de ligne qui avaient dérivé sont retirés
plutôt que corrigés** — ce dépôt a déjà écrit qu'un numéro recopié survit à la
réalité qu'il décrivait.

### 🔴 Le plafond a été franchi, et rattrapé par une EXTRACTION

`client/src/style.css` est passé à **301 lignes pour une porte à 300** : la
tâche 9 y a posé l'`@import` de l'écran terminal, la tâche 10 son encadré WCO, et
la revue trois lignes de plus. **Les deux boutons de coin partent VERBATIM** vers
`client/src/session/boutons-de-coin.css` (**138**), et `style.css` retombe à
**194**.

Ce dépôt a franchi ce plafond **trois fois en D10 et deux fois en D9**, et l'a
rattrapé **deux fois par une compression qu'il interdit nommément**. Ici c'est une
extraction, et **le point de chute existait déjà** — `client/src/session/`, ouvert
par la tâche 9.

⚠️ **Et l'extraction a créé un trou de garde, aussitôt bouché.** La règle du
garde ① (`pointer-events: none`) a changé de fichier : **vider `style.css` seule
l'aurait laissé vert**. L'atteignabilité est donc **dédoublée**, et sa rouge est
versée. **C'est le défaut de portée que `longueurs.test.ts` avait mesuré sur sa
propre rouge, rejoué par une simple extraction** — à savoir : *une extraction
déplace une règle, donc déplace ce qu'un garde d'absence doit surveiller.*

### Ce que la revue a changé aux actifs bâtis

`recette-3-apres-revue.log`, **une exécution**. **Exactement DEUX hachages
bougent** : `dist/index.html` — parce que `client/src/design/amorce-theme.js`
part **verbatim** dans chaque page bâtie et que la revue y a corrigé un mot — et
`main-*.css` — parce que l'extraction réordonne les règles, **à taille
rigoureusement égale, 1 849 octets avant comme après**. **Les quatre autres actifs
CSS sont octet pour octet identiques** : c'est la contre-épreuve que la revue n'a
pas touché ce qu'elle ne devait pas.

---

## 7. Les tailles, relevées PAR LA COMMANDE, APRÈS la revue transverse

⚠️ **La revue transverse est une source de croissance connue** — celle de S2 a
fait tomber la marge de `primitives.test.ts` de 30 à 17, celle de S3 a ajouté
**+54 lignes**. **Le relevé qui fait foi est celui d'après**, jamais celui d'avant.

| Fichier | Lignes | Porte | Marge |
| --- | --- | --- | --- |
| 🔴 `client/src/design/tokens.css` | **300** | 300 | **0** |
| `client/src/style.test.ts` | 219 | 300 | 81 |
| `client/src/style.css` | **194** | 300 | 106 (était **301** avant l'extraction) |
| `client/src/session/boutons-de-coin.css` | 138 | 300 | 162 (**neuf**) |
| `client/src/ecran-terminal.test.ts` | 108 | 300 | 192 (**neuf**) |
| `client/src/ecran-terminal.ts` | 105 | 300 | 195 (**neuf**) |
| `client/src/session/etat-terminal.css` | 80 | 300 | 220 (**neuf**) |
| `client/src/design/primitives.test.ts` | 245 | 300 | 55 |
| `client/outils/tokens-orphelins/attente.mjs` | 221 | 240 | 19 |
| `client/src/main.ts` | **460** | — | +9 depuis la base (le plan en autorisait dix) |
| `client/verify-webrtc.mjs` | **494** | 500 | 🔴 **6** — ⛔ **intouché par S4**, comme par S1, S2 et S3 |

**Aucun fichier du dépôt entier ne dépasse sa porte du fait de S4** : la commande
de `CLAUDE.md` rend `encode.rs` 1536, `windows_source.rs` 630,
`proto/src/plateforme/tests.rs` 561, `proto/ts/plateforme.test.ts` 512 et
`agent/src/main.rs` 505 — **aucun n'appartient à ⑥**, et aucun n'a été touché.

🔴 **`design/tokens.css` est à 300 EXACTEMENT : sa marge est NULLE.** Son point de
chute est **nommé dans le fichier lui-même** — scinder en `tokens/couleurs.css` et
`tokens/echelles.css` — et **la prochaine addition l'exige, jamais une
compression**. ⚠️ **Cette injonction ne peut pas être écrite DANS le fichier
qu'elle concerne : l'y écrire le ferait franchir.** Elle vit donc ici et dans
`CLAUDE.md`. Le sous-bloc a **délibérément écarté** l'extraction : **SEPT
lecteurs nomment `tokens.css` par son chemin** — énumérés, pas comptés de tête :
`outils/contraste.mjs` (§7.1), `outils/couleurs-litterales.mjs` (§7.2, en
exclusion), `outils/blocs-de-theme.mjs` (§7.4), `outils/tokens-orphelins.mjs`
(§7.6), et trois `import … from './tokens.css?raw'` — `design/reprise.test.ts`,
`design/tokens.test.ts` et `design/galerie.ts`. Le legs n°9 de S2 —
`lireBlocsDeTheme` ne sait nommer que **trois** blocs — est en outre encore
ouvert. **La scission est un chantier, pas une fin de branche.**

---

## 8. Les jugements humains — VINGT-CINQ, et aucun n'a été porté

**Quinze à la fin de S3** (huit de la spec §8, trois de S2, quatre de S3 — relevé
par le document de résultats de S3 lui-même). **S4 en ajoute DIX, énumérés et non
recopiés**, chacun avec le fichier qui le déclare :

| # | Ce qui n'est pas mesuré | Où c'est déclaré |
| --- | --- | --- |
| 16 | que la **pile monospace** de `#stats` se lise mieux que le crénage qu'elle remplace | `client/src/style.css` |
| 17 | que **20 px** soit la bonne taille des deux boutons de coin | `client/src/style.css` |
| 18 | que `#e6e8eb` soit la **bonne encre** sur un voile | `client/src/design/contraste.ts` |
| 19 | qu'un **écran plein cadre** soit la bonne forme pour un état terminal | `client/src/ecran-terminal.ts` |
| 20 | que cet écran doive rester **SANS ACTION** | `client/src/ecran-terminal.ts` |
| 21 | que les **trois mesures de contenant** méritent des tokens | `client/src/design/tokens.css` |
| 22 | que le **rayon** passant de 6 à 4 px soit acceptable | `client/src/style.css` |
| 23 | ⚠️ **hors plan** — que la **ZONE occupée** par le bouton soit la bonne, distincte de sa taille | `client/src/style.test.ts` |
| 24 | ⚠️ **hors plan** — que `--e-8` reste la bonne marge du micro après ce changement | `client/src/style.css` |
| 25 | ⚠️ **hors plan** — que les **deux libellés de titre** de l'écran terminal soient les bons mots | `client/src/ecran-terminal.ts` |

⚠️ **Le plan en prévoyait SEPT ; il y en a DIX.** Les trois derniers sont des
décisions esthétiques prises à l'exécution, et **les taire les aurait déguisées en
mesures**. ⚠️ **Ce compte de dix est celui de MON énumération**, faite par
`git blame` sur chaque déclaration ; il ne coïncide pas avec un chiffre transmis
oralement à l'exécution, et **c'est l'énumération qui fait foi**, pas le chiffre.

🔴 **ET LE JUGEMENT VISUEL N'A JAMAIS ÉTÉ PORTÉ SUR ⑥, D'UN BOUT À L'AUTRE.**
Aucune page du sous-projet n'a été ouverte dans un navigateur, ni en S1, ni en S2,
ni en S3, ni en S4. **S4 était la dernière occasion, et il la laisse passer en le
déclarant.** Un agent qui prendrait une capture d'écran **ne porterait pas un
jugement** — il produirait une image que personne n'a regardée. Ce n'est pas une
lacune d'exécution : c'est la conséquence assumée du §7.8, qui écarte la
comparaison d'images parce que les polices système rendent différemment d'une
machine à l'autre. **La galerie existe pour cela, et personne ne l'a regardée.**

---

## 9. Ce que S4 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, contrôles
  **déterministes** : reproductibilité, rien de plus. **Et cette question ne doit
  pas être empruntée** à une campagne qui, elle, l'aurait posée.
- 🔴 **Rien du Window Controls Overlay en fonctionnement.** Le garde prouve qu'il
  est **inerte aujourd'hui**, et **rien d'autre**.
- 🔴 **Aucun jugement visuel**, et les **vingt-cinq** jugements humains attendent
  tous un œil.
- **La lisibilité des voiles sur une vidéo quelconque** reste non mesurée : seule
  la bande noire du `letterbox` l'est. **La composition alpha n'est pas outillée.**
- **La zone réellement occupée par les deux boutons de coin** : rien n'est mesuré,
  l'avance du glyphe `⛶` reste inconnue, et le commentaire **cesse de la
  chiffrer** plutôt que de remplacer une estimation par une autre.
- **Que `.message--flottant` reste lisible au-dessus d'une image claire.**
- **L'anneau de focus reste vérifié NON EFFACÉ, jamais VISIBLE** — legs de S2 et
  de S3, non levé.
- **L'accessibilité au-delà du contraste et du mouvement réduit** : clavier
  complet, lecteurs d'écran, cibles tactiles, ordre de tabulation. L'écran
  terminal porte `role="status"` sur son message, **et rien de plus n'est
  éprouvé**.
- **Rien hors d'un Chromium de bureau** : ni Firefox, ni Safari, ni mobile, **ni
  HiDPI**.
- **Aucune internationalisation** : rien ne dit qu'un écran terminal survit à un
  motif d'erreur plus long dans une autre langue.
- **Le legacy n'est pas touché**, et **aucun contrôle ne le balaie**.
- **La bascule de thème entre deux fenêtres RÉELLES du produit** n'est toujours
  pas éprouvée.
- **Le plafond de 12 288 octets n'est calibré par rien.**
- **`galerie.ts` et `galerie-primitives.ts` n'ont toujours aucun test.**
- **Les rouges des tâches 1 à 8 ne sont étayées que par des messages de commit.**

---

## 10. Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **`[hidden]` PERD CONTRE UNE RÈGLE D'AUTEUR, ET CE N'EST PAS UNE QUESTION DE
  SPÉCIFICITÉ.** Le `[hidden] { display: none }` qui rend l'attribut efficace vit
  dans la feuille de l'**agent utilisateur**, et la cascade compare l'**origine**
  avant la spécificité : n'importe quelle déclaration d'auteur portant `display`
  l'emporte, fût-elle moins spécifique. Un écran `hidden` avec
  `.ecran { display: grid }` serait **visible dès le chargement**.
- 🔴 **UNE EXTRACTION DÉPLACE CE QU'UN GARDE D'ABSENCE DOIT SURVEILLER.** Sortir
  une règle d'un fichier laisse le garde vert sur le fichier vidé. **Toute
  extraction qui déplace une règle gardée oblige à déplacer, ou à dédoubler, son
  assertion d'atteignabilité.**
- ⚠️ **UN CONTRÔLE À PORTÉE DÉRIVÉE NE SE VIDE PAS EN VIDANT UN FICHIER.** Sa
  rouge d'atteignabilité doit vider **tous** les porteurs. Une prescription de
  plan qui nomme un fichier unique est fausse d'une portée dérivée — mesuré deux
  fois dans ce sous-bloc, sur §7.10 puis sur le garde WCO.
- ⚠️ **UN `grep` SENSIBLE À LA CASSE MANQUE CE QUE LES MAJUSCULES CACHENT.** Deux
  des dix-sept places de la revue étaient écrites `HUIT CONTRÔLES`. **La formule
  n'est pas le sens.**
- ⚠️ **UN COMPTE DE MENTIONS N'EST PAS UN COMPTE DE CHOSES.** `grep -c 'jugement
  humain'` rend 22 lignes pour 25 jugements : certaines déclarations en portent
  deux, d'autres se répètent. **Énumérer par `git blame`, pas compter des lignes.**
- ⚠️ **ZSH NE DÉCOUPE PAS LES VARIABLES EN MOTS.** Un `git add $FICHIERS` y passe
  la liste entière comme **un seul chemin**, et échoue avec un message qui ne dit
  pas pourquoi. **Nommer les fichiers, ou employer un tableau.**
- ⚠️ **DES BACKTICKS DANS UN `echo` DE JOURNAL EXÉCUTENT UNE COMMANDE.** Un
  journal de rouge a porté `command not found: style.css` au milieu de sa prose.
  **Guillemets simples pour toute prose journalisée**, comme pour le heredoc.
- ⚠️ **UN FICHIER NE PEUT PAS CONTENIR SA PROPRE TAILLE FINALE.** La ligne que
  `familles-de-lecture.txt` porte sur lui-même est celle de sa rédaction
  **précédente** — conservée telle quelle plutôt que devinée.

---

## 11. Ce que ⑥ laisse ouvert après S4 — la liste complète

**S4 étant le dernier sous-bloc, rien de ce qui suit n'a de destinataire dans ⑥.**

**Ce que S4 solde** : `--police-mono` ; les six longueurs hors échelle et la
clause §8 ; l'écran terminal ; la reprise des bandeaux sur les primitives ; le
raccordement sémantique du micro (**tranché non**) ; et le défaut d'encre du thème
clair.

**Ce qui reste :**

1. ⛔ **Le WCO n'a jamais été rendu.** La règle est livrée et **prouvée inerte** ;
   son comportement est inconnu. **Destinataire nommé : la recette de ④ G5**, qui
   pose le manifeste.
2. ⛔ **Le hub n'existe pas** ; son contenu dépend de ④, et ⑥ ne le livre pas. Les
   tokens et les primitives l'attendent.
3. ⛔ **Aucune primitive « lien », aucune ancre** dans aucune entrée. Poser une
   famille sans appelant serait le code mort que §7.6 refuse.
4. ⛔ **`galerie.ts` et `galerie-primitives.ts` sans test.**
5. ⛔ **Legs n°9 de S2** : `lireBlocsDeTheme` ne sait nommer que **trois** blocs,
   donc `tokens.css` ne peut accueillir aucune autre requête média.
6. ⛔ **Le plafond de poids CSS n'est calibré par rien.**
7. ⛔ **Le sens « toute classe déclarée est employée » de §7.9 n'existe pas.**
   C'est `primitives.html` et l'œil qui le tiennent — **et l'œil n'est pas passé.**
8. ⛔ **`Ton` et `CLASSE_DE_TON` sont dupliqués** entre `client/src/shell.ts`,
   `client/src/connexion.ts` et `client/src/ecran-terminal.ts`. **S4 ne les unifie
   pas** — cela toucherait deux surfaces closes, sans critère capable d'attraper
   une régression et sans œil pour la voir. Point de chute d'une unification : la
   couche `design/`.
9. ⛔ **La lisibilité d'un voile sur une vidéo quelconque n'est pas outillée.** La
   composition alpha est un calcul **pur**, donc à portée de
   `client/src/design/contraste.ts`.
10. ⛔ **L'anneau de focus n'a jamais été vu VISIBLE**, en particulier sur un
    `.bouton--principal` dont le fond est `--accent`.
11. ⛔ **`prefers-reduced-motion` est le seul des quatre manques d'accessibilité
    qui soit pris.**
12. ⛔ **`client/verify-webrtc.mjs` est à 494 pour une porte à 500 — marge 6**, et
    **la divergence de convention que D10 a signalée n'a jamais été tranchée** :
    le § « Portée » de `CLAUDE.md` ne liste que `client/src/`, la commande de
    vérification l'attrape quand même. **C'est une décision de convention, et elle
    appartient au propriétaire du dépôt.**
13. ⛔ **Le défaut à deux réglages de `build-agent.sh` / `run-agent.sh`** ne
    concerne pas ⑥ — ses journaux sont propres — mais il reste **non corrigé**.
14. 🔴 **`design/tokens.css` est à 300 pour une porte à 300 : marge NULLE.** La
    prochaine addition exige l'**extraction** nommée (`tokens/couleurs.css` +
    `tokens/echelles.css`), **jamais une compression**. **SEPT lecteurs** de ce
    fichier par son chemin devront suivre — énumérés au §7.
15. ⛔ **Les rouges des tâches 1 à 8 n'ont pas de journal versé.** La consigne de
    verser doit valoir à la tâche qui joue la rouge, pas à celle qui recette.
16. 🔴 **AUCUN JUGEMENT VISUEL N'A ÉTÉ PORTÉ SUR ⑥, D'UN BOUT À L'AUTRE.** Les
    **vingt-cinq** jugements humains attendent tous un œil, et **le sous-projet se
    termine sans qu'aucune de ses pages ait été ouverte dans un navigateur.**
