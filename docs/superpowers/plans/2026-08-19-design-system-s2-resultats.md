# Sous-bloc S2 — les primitives : bouton, champ, surface, message : recette

**Plan :** `docs/superpowers/plans/2026-08-19-design-system-s2.md` (commit `0e27ee6`).
**Spécification :** `docs/superpowers/specs/2026-08-19-design-system-design.md` (commit `5b6b830`).
**Sous-bloc précédent :** S1, livré et clos — recette `1b4ac3b`, journaux
`docs/superpowers/plans/journaux-design-s1/`.
**Journaux de cette recette :** `docs/superpowers/plans/journaux-design-s2/`.

**Binaire jugé** : il n'y en a pas — ⑥ est un sous-projet **navigateur**.
Ce qui est jugé est l'arbre au commit **`070b48b`** (la tâche 8), relevé le
**20 août 2026**. Les tâches 1 à 7 sont datées du 19 août ; les tâches 8, 9 et
10 du 20 — **c'est la date de leurs relevés, et elle n'est pas alignée sur
celle du plan.**

⛔ **AUCUNE TÂCHE DE S2 N'A EMPLOYÉ LA VM WINDOWS**, et ce n'est pas une
omission : la spec §9 déclare que ⑥ n'a aucune recette sur VM. Tout ce qui suit
tourne sur l'hôte.

⚠️ **L'ARBRE EST PARTAGÉ PAR CINQ CHANTIERS.** `git status --porcelain` au
moment de la recette (versé, `journaux-design-s2/commit.txt`) porte des
modifications non commitées d'un voisin dans `agent/` — `agent/Cargo.toml`,
`agent/src/pont.rs`, plus quatre chemins non suivis. **Aucune n'est de S2**, et
aucune n'a fait tomber quoi que ce soit (voir le §6).

---

## 1. Les quatre critères — verdicts, avec leur nombre d'exécutions

🔴 **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les
contrôles de ⑥ sont **déterministes** (spec §9) : la question « combien de fois
sur combien » ne se pose pas ici, et **elle n'est pas empruntée** à une campagne
qui, elle, l'aurait posée. Les deux exécutions de chaque commande rendent des
sorties **identiques**, chiffre pour chiffre.

| # | Critère | Verdict | Exéc. | Le chiffre, **relevé** |
| --- | --- | --- | --- | --- |
| ① | les sept contrôles restent verts ; §7.1 compte **52** paires, minimum **3,16** | **TENU** | **2** | `6/6 contrôle(s) vert(s)`, `exit=0` ; §7.1 `paires vérifiées : 52`, `échecs : 0`, `minimum global : 3.16` ; §7.5 `10 passed` |
| ② | la liste d'attente a **RÉTRÉCI**, de **28** à **10**, chaque sortie due à un appelant écrit | **TENU** | **2** | `48 token(s) déclaré(s)`, `38 token(s) employé(s)`, `10 orphelin(s), dont 10 en attente déclarée`, `total : 0 écart(s)`, `exit=0` |
| ③ | **aucun sélecteur de `primitives.css` ne peut s'appliquer à `index.html`** | **TENU** | **2** | `primitives.test.ts` : `9 passed (9)` — G1 à G7 |
| ④ | le poids CSS reste sous le plafond, et le chiffre est relevé | **TENU** | **2** | `somme : 6374 octets`, `plafond : 12288`, `marge : 5914`, `feuilles émises : 3` |
| — | le **jugement visuel** sur `primitives.html` | ⛔ **NON PORTÉ** — voir le §4 | **0** | — |

**Détail des sept contrôles**, relevé identique aux deux exécutions
(`design-verifier-{1,2}.log`) :

| Contrôle | Relevé |
| --- | --- |
| §7.1 contrastes | 52 paires, 0 échec, minimum global 3,16 |
| §7.2 couleurs littérales | 56 fichiers balayés, **0** couleur littérale hors `tokens.css` |
| §7.3 surfaces bâties | assertion A 0 échec, assertion B 0 échec, **évaluée sur 5 pages** |
| §7.4 blocs de thème | racine **48**, media-clair **14**, attribut-clair **14**, **0 écart** |
| §7.5 bascule de thème | test unitaire, `10 passed` (`theme-7-5-{1,2}.log`) |
| §7.6 tokens orphelins | 48 déclarés / 38 employés / **10** orphelins = **10** en attente, 0 écart |
| §7.7 poids CSS | 1 493 + 2 702 + 2 179 = **6 374** octets, plafond 12 288, marge 5 914 |

🔴 **LE CRITÈRE ③ MÉRITE D'ÊTRE LU DEUX FOIS : IL NE DIT PAS « S2 NE CHANGE
RIEN À LA FENÊTRE DE SESSION ».** `tokens.css` a gagné un token, donc
`socle-*.css` **change d'octets** (2 010 → 2 179, relevé) et `index.html` le
charge. Ce que ③ établit, c'est qu'**aucune règle de `primitives.css` ne peut
sélectionner un élément d'`index.html`** — ce qui est la seule chose qui compte
pour le rendu, et la seule chose qu'une commande sache dire. **Le formuler
autrement serait affirmer au-delà du relevé.**

**Suites existantes, relevées aux deux exécutions** : `client` **196** tests
(22 fichiers), `proto` **79** tests (4 fichiers), `typecheck` `exit 0` des deux
côtés. ⚠️ **Le plan annonçait `client` 187 (21 fichiers) et `proto` 70 (3
fichiers) à `56b975a`.** S2 ajoute 9 tests côté `client` (les 9 de
`primitives.test.ts` — contraste passant de 50 à 52 paires sans changer le
nombre de *tests*). **Le mouvement de `proto` (70 → 79, 3 → 4 fichiers) N'EST
PAS DE S2** : aucune tâche de ce sous-bloc n'a touché `proto/`.

---

## 2. Chaque rouge jouée, avec son message d'échec verbatim

🔴 **UN CONTRÔLE QU'ON N'A JAMAIS VU ROUGE N'EST PAS UN CONTRÔLE**, et
**une rouge ne vaut que pour l'assertion qu'elle fait tomber** — `expect`
interrompt le test à la première. Les comptes `n failed | m passed` ci-dessous
sont la preuve que chaque mutation n'a fait tomber **que** l'assertion visée.

### 2.1 Une rouge par critère de recette — jouée par la tâche 9

Journaux : `rouge-1-contraste.log`, `rouge-2-orphelins.log`, `rouge-3-g1.log`,
`rouge-4-poids.log`. **L'arbre a été restauré après chacune**, et la
restauration est vérifiée par `git status --porcelain client/` **vide**.

| Critère | Mutation | Message verbatim | Sortie |
| --- | --- | --- | --- |
| ① | `--accent-survol` clair porté de `#2650b4` à `#6f9bf5` | `ÉCHEC clair sur-accent/accent-survol = 2.73 < 4.5` — `paires vérifiées : 52`, `échecs : 1`, `minimum global : 2.73` | `exit=1` |
| ② | la ligne `--police-mono` retirée de `EN_ATTENTE_D_APPELANT` | `NOUVEL ORPHELIN  --police-mono  déclaré et appelé par personne` — `10 orphelin(s), dont 9 en attente déclarée`, `total : 1 écart(s)` | `exit=1` |
| ③ | `button { color: var(--texte); }` ajouté à `primitives/bouton.css` | `AssertionError: sélecteurs d’élément nus dans primitives.css: expected [ 'button' ] to deeply equal []` | `Tests  1 failed \| 8 passed (9)` |
| ④ | plafond abaissé à 6 000 octets (**aucun fichier touché**) | `somme : 6374 octets` / `plafond : 6000 octets` / `DÉPASSEMENT de 374 octets` | `exit=1` |

⚠️ **La rouge ④ ne mute aucun fichier** : `poids-css.mjs` accepte `--plafond`.
Elle établit que l'état rouge est **atteignable**, elle n'établit pas qu'un CSS
de 12 289 octets se bâtirait — ce n'est pas la même chose, et il ne faut pas la
lire ainsi.

⚠️ **La rouge ② est jouée dans UN SEUL des deux sens** du contrôle. L'autre —
« une entrée de la liste qui a gagné un appelant » — a été joué **dix-huit fois
en conditions réelles** par les tâches 2 à 5, chacune ayant lu les lignes
`À RETIRER DE LA LISTE` avant de retirer, et il l'est encore par
`orphelins-sans-exclusion.log` (voir le §2.3).

### 2.2 Les gardes de forme, rouges **rejouées** par la tâche 9

Journal : `rouges-rejouees.log`. ⚠️ **Ce sont des REJEUX.** Les tâches 2 à 6
avaient joué ces rouges à leur date ; **leurs messages ne sont versés nulle
part**, et les reconstituer de mémoire serait fabriquer une pièce. Ceux
ci-dessous ont été relevés le 20 août 2026 sur l'arbre de la recette, **une
mutation à la fois**, arbre restauré après chacune.

| Garde | Mutation | Message verbatim | Sortie |
| --- | --- | --- | --- |
| G1 | `button { color: var(--texte); }` | `sélecteurs d’élément nus dans primitives.css: expected [ 'button' ] to deeply equal []` | `1 failed \| 8 passed` |
| G2 | `.bouton:focus-visible { outline: none; }` | `effacements de l’anneau de focus: expected [ 'outline: none' ] to deeply equal []` | `1 failed \| 8 passed` |
| G3 | l'encre du désactivé remplacée par `opacity: 0.5` | `compositions d’exécution dans primitives.css: expected [ 'opacity: 0.5' ] to deeply equal []` | `1 failed \| 8 passed` |
| G4 | `var(--r-1)` remplacé par `4px` | `longueurs littérales dans primitives.css: expected [ Array(1) ] to deeply equal []` | `1 failed \| 8 passed` |
| G5 | les **quatre** feuilles de famille vidées de leurs règles, en-têtes gardés | `primitives.css ne déclare AUCUNE règle : G1 à G4 sont alors verts en ne mesurant rien: expected 0 to be greater than 0` | `4 failed \| 5 passed` |
| G6 (champ) | idem G5 | `états ou parties absents de la famille champ: expected [ '.champ__etiquette', …(6) ] to deeply equal []` | idem |
| G6 (surface) | idem G5 | `parties absentes de la famille carte: expected [ '.carte__titre', '.carte__corps' ] to deeply equal []` | idem |
| G6 (message) | idem G5 | `tons absents de la famille message: expected [ '.message--succes', …(2) ] to deeply equal []` | idem |
| G7 | la règle `@media (prefers-reduced-motion: reduce)` retirée de `base.css`, **le commentaire gardé** | `aucune requête @media (prefers-reduced-motion: reduce) dans base.css, commentaires blanchis: expected -1 to be greater than -1` | `1 failed \| 8 passed` |

🔴 **G5 EST LE SEUL GARDE D'ATTEIGNABILITÉ, ET SA ROUGE LE MONTRE** : sur des
feuilles vidées, **G1 à G4 restent VERTS** (`4 failed | 5 passed` — les quatre
tombés sont G5 et les trois G6). Sans G5, quatre gardes sur cinq ne prouveraient
rien. C'est la classe de défaut dont ce dépôt a attrapé quatre exemplaires sur
le seul sous-bloc D10, dont trois écrits par un plan.

🔴 **LA ROUGE DE G7 EST JOUÉE EN NE RETIRANT QUE LA RÈGLE, ET C'EST CE QUI LA
REND PROBANTE** : l'en-tête de `base.css` **recopie en toutes lettres** la
chaîne `@media (prefers-reduced-motion: reduce)` (elle fait partie du relevé D3
qu'il porte). Un garde qui la chercherait dans le texte brut serait satisfait
par ce commentaire. Le blanchiment est ce qui l'en empêche. **Voir le §5.1 : ce
qui suit cette phrase dans le code était plus fort que le relevé, et il est
corrigé.**

### 2.3 La rouge de l'exclusion des galeries

Journal : `orphelins-sans-exclusion.log`, `exit=1`, **9 écarts**. Sans les deux
exclusions, `design.html` et `primitives.html` font tomber neuf lignes
`À RETIRER DE LA LISTE` — dont `--e-5`, `--e-6`, `--e-7`, `--lh-large`,
`--t-3xl`, `--t-xs`, `--r-plein`, `--police-mono` —, **tous employés par la
seule mise en page d'une page de démonstration**. C'est la preuve, mesurée, que
l'exclusion est **porteuse** : sans elle, la liste d'attente rétrécirait sans
que le produit ait gagné un seul appelant.

---

## 3. La liste d'attente : 28 → 10, entrée par entrée

🔴 **LE FAIT LE PLUS RÉUTILISABLE DE S2 : LA LISTE RÉTRÉCIT PARCE QU'UN CONTRÔLE
LA FORCE.** Le contrôle §7.6 exige l'**ÉGALITÉ** entre l'ensemble des orphelins
et la liste nommée, donc il échoue **dans les deux sens**. Chaque retrait a été
**lu dans la sortie du contrôle** — la ligne `À RETIRER DE LA LISTE <token>` —
et **jamais deviné**. Relevé par `git show <commit> -- client/outils/tokens-orphelins.mjs`.

| Tâche | Commit | Tokens retirés | Compte |
| --- | --- | --- | --- |
| T2 — bouton | `78eb679` | `--fond-1`, `--fond-2`, `--bord`, `--bord-fort`, `--texte`, `--texte-faible`, `--sur-accent`, `--r-1`, `--duree-1`, `--trait` | **10** |
| T3 — champ | `b00099a` | `--danger`, `--t-l` | **2** |
| T4 — surface | `21393e0` | `--t-xl`, `--lh-serre`, `--e-4`, `--r-3` | **4** |
| T5 — message | `4c5d9e2` | `--succes`, `--alerte` | **2** |
| T7 — galerie | `b9c706d` | **aucun** — elle ajoute une **exclusion**, pas un appelant | **0** |
| | | **total retiré** | **18** |

**28 − 18 = 10.** Vérifié par la commande, pas par addition à la main.

### 🔴 Une contradiction interne du plan, relevée et tranchée en faveur du contrôle

Le tableau de prédiction du plan (l. 120-129) annonçait **T2 = 8, T3 = 2,
T4 = 5, T5 = 3**. Le relevé donne **T2 = 10, T3 = 2, T4 = 4, T5 = 2**.

**L'écart n'est pas une dérive d'exécution : le plan se contredisait
lui-même.** Son tableau assignait `--bord` à T4 et `--texte` à T5, tandis que le
**Step 4 de T2** — la table des variantes de bouton, l. 892-894 — prescrivait
`trait --bord` pour le bouton secondaire désactivé et `encre --texte` pour le
bouton discret au repos. **Les deux ne pouvaient pas être vraies ensemble.**

L'implémenteur a suivi **le contrôle**, comme le plan l'ordonne lui-même
(l. 131-138 : « **Le contrôle dit quoi retirer ; ce plan dit seulement à quoi
s'attendre** »). C'est la bonne règle, et c'est ce qui fait que le total, lui,
est tombé juste : **la prédiction se trompait sur la répartition, jamais sur la
somme.**

### Les dix qui restent, et les trois re-étiquetages

| Token | Annotation après S2 | Re-étiqueté ? |
| --- | --- | --- |
| `--t-xs` | S3 ou plus tard — la mention légale et les étiquettes | 🔴 **oui**, S2 → S3+ |
| `--e-1` | S3 ou plus tard — l'écart interne d'une étiquette | 🔴 **oui**, S2 → S3+ |
| `--r-plein` | S3 ou plus tard — les pastilles et les boutons ronds | 🔴 **oui**, S2 → S3+ |
| `--t-2xl` | S3 — le titre de l'écran de connexion | non |
| `--t-3xl` | S3 — le titre du hub | non |
| `--lh-large` | S3 — les paragraphes longs | non |
| `--e-5` | S3 — la gouttière entre cartes | non |
| `--e-6` | S3 — la marge des sections | non |
| `--e-7` | S3 — la marge de tête des surfaces | non |
| `--police-mono` | S4 — `#stats`, OU RETRAIT | non |

**La motivation des trois re-étiquetages est la même, et elle est écrite auprès
d'eux** (`client/outils/tokens-orphelins/attente.mjs`) : les trois décrivent une
famille **ÉTIQUETTE / PASTILLE**. Le §6 de la spec borne S2 à **quatre**
familles — bouton, champ, surface, message —, « ce dont un écran de connexion a
besoin », et un écran de connexion n'a ni étiquette ni pastille. **S1 avait
prédit S2 ; la prédiction était fausse.** Fabriquer une pastille dans le seul
but de vider trois lignes aurait été **vider un contrôle pour en verdir un
autre**, le geste que la liste d'attente refuse dans son propre encadré.

🔴 **UN RE-ÉTIQUETAGE N'EST VU PAR AUCUN CONTRÔLE**, et c'est **le point le plus
faible de S2**. Le contrôle compare des **ensembles de noms** ; changer « S2 »
en « S3 » ne déclenche rien, dans aucun des deux sens.

⚠️ **Le plan déclarait « aucune mitigation technique n'est proposée : ce serait
un contrôle de plus sur des chaînes de prose, et il ne pourrait pas échouer
utilement » (l. 369, l. 1379-1380, et sa table des risques). CETTE AFFIRMATION
EST FAUSSE, et une mitigation PARTIELLE est nommée ici** — voir le §5.2.

### Le relevé de S1 refait, et ce que son zéro dit

S1 refusait d'élaguer la palette sur mesure : « sur les 50 paires de contraste,
**46** citent au moins un token de cette liste ». **Refait par la tâche 8, même
commande, le 20 août 2026, sur les dix entrées restantes :**

```
paires totales : 52 | citant un token en attente : 0
```

**ZÉRO — et ce zéro dit l'inverse de ce qu'on croirait y lire.** Il ne réfute
pas S1 : il montre que **la décision de S1 a tenu jusqu'au bout**. Aucune des
quatorze couleurs par thème n'est plus orpheline ; les dix entrées restantes
sont typographiques, d'espacement, de rayon et de police, et les paires de
contraste ne citent que des couleurs. **Élaguer en S1 aurait retiré des couleurs
que S2 emploie aujourd'hui.**

⚠️ **COROLLAIRE, ÉCRIT SUR PLACE ET NON DÉDUIT PLUS TARD : cet argument NE
PROTÈGE PLUS RIEN.** « Élaguer ferait tomber §7.1 de 50 paires à 4 » était la
raison mesurée qui gardait la palette entière ; un élagage n'atteindrait
aujourd'hui plus aucune couleur. **Ce qui protège les dix restants n'est plus
qu'une chose — leur annotation, et le sous-bloc qui la porte.** C'est-à-dire
exactement ce que rien ne contrôle.

### La taille du fichier : la prédiction du plan est FAUSSE, et c'est dit

Le plan attendait de T8 un `tokens-orphelins.mjs` **plus court** qu'à `56b975a`
(233 lignes), les 18 entrées retirées devant le faire maigrir. **Relevé par la
commande avant extraction :**

| | à `56b975a` | après T2…T8 | delta |
| --- | --- | --- | --- |
| entrées de liste | 28 | 10 | **−18** |
| lignes de commentaire | 104 | 153 | **+49** |
| lignes de code | 87 | 93 | +6 (la seconde exclusion, `--sans-exclusion`) |
| lignes vides | 14 | 14 | 0 |
| **total** | **233** | **270** | **+37** |

**Les 18 entrées retirées ont été plus qu'annulées par du commentaire** — en
petit, la leçon que ce dépôt a payée en grand : « une addition de commentaire
peut annuler une extraction ». Aucun des trois blocs neufs n'est du remplissage
(la raison **mesurée** de la seconde exclusion, l'encadré des re-étiquetages, le
relevé de S1 refait au lieu d'être effacé), et **les raboter aurait échangé une
vérité contre un nombre**, ce que `CLAUDE.md` interdit nommément.

🔴 **ET LA PREMIÈRE RÉDACTION DE CE CONSTAT, ÉCRITE EN TÊTE DU FICHIER, L'A
PORTÉ À 296 — MARGE 4 : un encadré qui dénonçait la dérive la produisait.** Le
remède du dépôt a été appliqué — **extraire, jamais compresser** : la liste part
avec **toute** sa doctrine vers `client/outils/tokens-orphelins/attente.mjs`
(**136** lignes), et `tokens-orphelins.mjs` retombe à **202**, sous les 233
attendus. C'est le patron de `serveur/instances.rs`, qui a emporté `TAMPON` avec
le commentaire qui le justifie.

---

## 4. ⛔ Le jugement visuel : NON PORTÉ

Le plan prévoyait d'ouvrir `dist/primitives.html` dans un Chromium de l'hôte,
dans les deux thèmes, et de regarder les quatre familles. **Cela n'a pas été
fait, et c'est déclaré plutôt que remplacé par un « probablement ».**

**Ce n'est de toute façon PAS un critère** : c'est le **huitième** jugement
humain du §8, et un agent qui prendrait une capture d'écran n'en porterait pas
un — il produirait une image que personne n'a regardée.

Ce qui **a** été fait, et qui est une **mesure** et non un jugement
(`primitives-html-structure.log`) : compter les occurrences de classes dans le
`dist/primitives.html` bâti (11 839 octets). Les dix classes attendues y sont
toutes présentes — `bouton--principal` ×2, `bouton--secondaire` ×2,
`bouton--discret` ×2, `champ__etiquette` ×3, `champ--erreur` ×1, `carte__titre`
×1, `separateur` ×1, `message--succes` ×1, `message--alerte` ×1,
`message--danger` ×1. **Cela dit que la page mentionne les quatre familles. Cela
ne dit RIEN de son apparence.**

**Les huit jugements humains du §8 restent entiers, et S2 en ajoute TROIS :**

1. que `#93b4f9` / `#2650b4` soient les **bonnes** valeurs de survol — seul leur
   contraste est mesuré (9,39 et 7,27, calculés ; 52 paires vertes) ;
2. que l'état **actif** dit « par le retour au repos » soit lisible — le survol
   se retire sous le doigt, et c'est un arbitrage, perceptible seulement pendant
   l'appui ;
3. que **quatre tons de message distingués par la seule encre** suffisent.

**Aucun des onze ne deviendra une mesure.**

---

## 5. Ce que la revue a trouvé, et qui ne se voit pas d'une tâche

### 5.1 Une affirmation de G7 que sa propre seconde assertion réfute — MESURÉE

`primitives.test.ts`, dans le corps de G7, dit : « Un garde qui la chercherait
dans le texte brut **resterait VERT** sur un `base.css` dont la règle a été
retirée. »

**Mesure** (`rouges-rejouees.log`, dernier bloc) : double mutation — la règle
retirée de `base.css` **et** `sansCommentaires()` neutralisé en identité.

```
× G1 — aucun sélecteur d’élément nu … expected [ '/*', …(889) ] to deeply equal []
× G7 — base.css neutralise les transitions … 
  AssertionError: la requête de mouvement réduit ne porte aucune déclaration:
  expected [ '--duree-1' ] to include 'transition-duration'
      Tests  2 failed | 7 passed (9)
```

**G7 ne reste PAS vert.** Sa **première** assertion est bien satisfaite par le
commentaire — jusque-là le raisonnement tenait — mais sa **seconde** trouve
alors le bloc du relevé D3, dont la déclaration est `--duree-1` et non
`transition-duration`, et elle tombe.

**L'énoncé juste est donc : sans blanchiment, la PREMIÈRE assertion de G7
resterait satisfaite ; le garde, lui, tomberait quand même.** Écrit par T6,
réfuté par sa propre seconde assertion écrite dans le même commit — et
invisible à une revue par tâche, faute d'avoir jamais neutralisé le
blanchiment.

⚠️ **Portée** : cela ne réfute **pas** le blanchiment, qui reste **strictement
nécessaire** — la mesure montre qu'il est le seul rempart de **G1** (889
compounds parasites, dont `/*`) et donc de **G5**, qui lit les mêmes préludes.

### 5.2 Le plan déclarait qu'aucune mitigation du re-étiquetage n'est possible : c'est faux

Une mitigation **partielle** existe, et elle **pourrait échouer utilement** :
*aucune entrée de la liste d'attente ne doit nommer un sous-bloc déjà clos.*
Elle aurait viré au rouge à la fin de S2 sur les trois entrées annotées « S2 »,
**forçant la décision au lieu de la laisser à une règle de revue.**

**Elle ne couvre pas tout, et c'est pour cela qu'elle est « partielle »** : elle
juge le **sous-bloc nommé**, jamais le **contenu** de l'annotation — un « S3 —
la gouttière entre cartes » changé en « S3 — n'importe quoi » lui échapperait.
Et elle exige que le dépôt sache quel sous-bloc est courant, ce qu'aucun fichier
ne dit aujourd'hui.

**Elle n'est pas construite ici** (hors du périmètre de S2), et elle est
**léguée** — voir le §8.

### 5.3 🔴 Un trou du contrôle §7.4, qui survivra à ce sous-bloc

`ecartsEntreBlocs` (`client/src/design/tokens.ts`) compare **clair ⇄ clair** et
**clair ⊆ racine**. Il ne compare **jamais racine ⊆ clair**.

**Conséquence : un token de couleur oublié dans les deux blocs clairs lui est
INVISIBLE.** Le contrôle attrape « déclaré dans un bloc clair et pas dans
l'autre » et « déclaré dans un bloc clair et pas à la racine » ; il n'attrape pas
« déclaré à la racine seule ».

🔵 **MESURÉ, PAS RAISONNÉ** (20 août 2026) : `--accent-survol` retiré des
**deux** blocs clairs et laissé à la racine seule —

```
$ node client/outils/blocs-de-theme.mjs
  bloc racine : 48 token(s)
  bloc media-clair : 13 token(s)
  bloc attribut-clair : 13 token(s)
écarts : 0
exit=0
```

**Quarante-huit contre treize, et zéro écart.** L'arbre a été restauré, et la
restauration vérifiée par `git status --porcelain` vide.

⚠️ **Cela a une conséquence directe sur la rouge §7.4 que le plan prescrivait à
T2** (l. 845-851 : poser `--accent-survol` dans **un seul** bloc, et attendre
`exit=1` avec deux `ÉCART … manquant`). **Selon le bloc choisi, cette rouge
était IMPOSSIBLE** — la mesure ci-dessus le montre. Elle n'est rouge que si le
token est posé dans **un seul des deux blocs clairs**, et le libellé du plan ne
le disait pas.

**C'est un fait de conception, pas un défaut de S2** : le contrôle a été écrit
en S1 et fait exactement ce que son nom dit — les *blocs de thème* ne divergent
pas entre eux. **Il est légué tel quel**, nommé.

### 5.4 La clause « aucune longueur hors échelle » du §8 de la spec

Elle est **fausse depuis S1**, de trois valeurs exactement, et
`client/src/style.css:27-31` le déclare déjà. **S2 ne la change pas** : il n'en
ajoute aucune — le garde **G4** l'interdit dans `primitives.css` et sa rouge est
versée (§2.2) — et il n'en retire aucune, les trois vivant dans la fenêtre de
session que **seul S4** a le droit de toucher. **La clause reste fausse jusqu'à
S4.**

⚠️ **Et G4 ne dit pas ce qu'on aimerait qu'il dise** : il vérifie qu'une
longueur passe par un token, **jamais que le bon token a été choisi**. Aucun des
sept contrôles ne mesure une longueur.

---

## 6. `verify-all.sh` : dix étapes, dix-sept en-têtes, et aucune ne tombe

**Lancé en entier**, journal versé (`verify-all.log`, **3 186** lignes) :

```
verify-all exit=0
```

**Relevé par la commande, et non plus dérivé comme dans le plan** :

| Grandeur | Valeur | Commande |
| --- | --- | --- |
| étapes du script | **10** | `grep -c '^etape "' scripts/verify-all.sh` |
| en-têtes `==>` d'une **exécution complète** | **17** | `grep -c '^==> ' verify-all.log` |
| en-têtes `==>` d'une passe `design:verifier` | **7** | `grep -c '^==>' design-verifier-1.log` |

**10 + 7 = 17**, et cette fois les trois nombres sont **relevés**, pas sommés :
la neuvième étape (`client : npm run design:verifier`) imprime sept en-têtes de
son côté — un `npm run build` plus les six contrôles —, le septième contrôle du
socle (§7.5) étant un test unitaire qui tourne dans `client : npm test`.
🔴 **Ni « dix » ni « dix-sept » ne se suffit sans dire lequel on compte.**

⚠️ **AUCUNE ÉTAPE ÉTRANGÈRE N'EST TOMBÉE, et c'est à relever plutôt qu'à taire** :
`cargo test --workspace` a passé sur un `agent/` que le chantier voisin du pont
de fichiers modifiait au même instant (deux fichiers modifiés, quatre chemins
non suivis — `commit.txt`), et `plateforme : npm run test:postgres` a trouvé son
instance. **Il n'y a donc rien à attribuer à personne**, contrairement à P2.
*(Un `warning: constant ERROR_GEN_FAILURE is never used` apparaît sous `cargo
clippy` : il vient du travail non commité du voisin dans `agent/`, il n'est pas
une erreur, et le script sort à 0.)*

---

## 7. Tailles relevées **par la commande**, au commit `070b48b`

⚠️ **Chaque ligne a été mesurée au moment de l'écrire, jamais relue d'un
document.** Journal : `tailles.txt`.

**Dépôt entier, fichiers de plus de 500 lignes** — **DEUX**, les deux lignes de
la dette gelée, **aucun fichier de `client/`** :

```
1536 agent/src/encode.rs
 630 agent/src/windows_source.rs
```

**Les fichiers de S2, nommément** (porte d'action **300** pour tout fichier neuf
de ⑥, spec §10 ; porte **240** pour `primitives.css`) :

| Fichier | Lignes | Porte | Marge |
| --- | --- | --- | --- |
| `client/src/design/primitives.test.ts` | **270** | 300 | **30** |
| `client/design.html` | **231** | 300 | 69 — **non touché par S2** |
| `client/src/design/tokens.css` | **225** | 300 | 75 |
| `client/outils/tokens-orphelins.mjs` | **202** | 300 | 98 |
| `client/primitives.html` | **199** | 300 | 101 |
| `client/src/style.css` | **181** | 300 | 119 |
| `client/src/design/galerie.ts` | **162** | 300 | 138 |
| `client/src/design/contraste.ts` | **155** | 300 | 145 |
| `client/outils/tokens-orphelins/attente.mjs` | **136** | 300 | 164 |
| `client/src/design/base.css` | **129** | 300 | 171 |
| `client/src/design/contraste.test.ts` | **125** | 300 | 175 |
| `client/vite.config.ts` | **112** | 300 | 188 |
| `client/src/design/theme.ts` | **107** | 300 | 193 |
| `client/src/design/primitives/bouton.css` | **103** | 300 | 197 |
| `client/src/design/selecteur-theme.ts` | **84** | 300 | 216 |
| `client/src/design/primitives/champ.css` | **75** | 300 | 225 |
| `client/src/design/primitives.css` | **73** | **240** | 167 |
| `client/src/design/primitives/message.css` | **52** | 300 | 248 |
| `client/src/design/primitives/surface.css` | **44** | 300 | 256 |
| `client/src/design/galerie-primitives.ts` | **18** | 300 | 282 |

⚠️ **La marge la plus étroite de S2 est celle de `primitives.test.ts` : 30.**
Toute addition substantielle à ce fichier appelle une **extraction**, jamais une
compression.

⚠️ **`client/verify-webrtc.mjs` vaut 494 (marge 6), inchangé** — **il n'est
touché par aucune tâche de S2**, et il n'a pas bougé.

⚠️ **`client/src/main.ts` vaut 451**, et **ce n'est pas de S2** : il était à 392
en D10 et le chantier E l'a porté là. Aucune tâche de S2 ne le touche.

✅ **`primitives.css` a été EXTRAIT AVANT de franchir sa porte, et c'est la
première fois dans ce dépôt.** Relevé par `git cat-file -p <commit>:<chemin>` :
**154** lignes après T2 (bouton), **230** après T3 (champ) — marge **10** —,
puis T4 a **extrait avant d'écrire** sa famille, ce qui l'a ramené à **72**, et
T5 à **73**. Il n'est plus qu'une liste de quatre `@import` vers
`design/primitives/{bouton,champ,surface,message}.css`. **Le plafond n'a jamais
été franchi**, là où ce dépôt l'a franchi trois fois en D10 et deux fois en D9,
et rattrapé deux fois par une compression qu'il interdit. C'est la première des
deux extractions de S2 ; la seconde est `tokens-orphelins/attente.mjs`, et
celle-là est venue **après** coup.

**Poids CSS bâti** : `main-*.css` 1 493 + `primitives-*.css` 2 702 +
`socle-*.css` 2 179 = **6 374** octets, plafond **12 288**, marge **5 914**.
Base S1 : **3 503**. **S2 ajoute 2 871 octets**, dont 2 702 pour la seule
feuille des primitives — **que S2 ne lie à aucune page du produit** (§8).
⚠️ **Le plafond de 12 288 n'est calibré par rien**, et il le reste.

---

## 8. Ce que S2 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, sur des contrôles
  **déterministes** : reproductibilité, rien de plus.
- 🔴 **Aucun jugement visuel n'est une mesure**, et le jugement visuel de S2
  **n'a pas été porté du tout** (§4). Les **huit** jugements humains du §8
  restent entiers, **augmentés des trois que S2 ajoute** — les valeurs de
  survol, l'état actif dit par le retour au repos, et quatre tons distingués par
  la seule encre.
- ⛔ **AUCUNE SURFACE DU PRODUIT N'EMPLOIE UNE PRIMITIVE.** À la fin de S2,
  `primitives.css` n'est liée que par `primitives.html`, qui n'est pas du
  produit. **Les dix-huit tokens sortis de la liste ont un appelant ÉCRIT, pas
  un pixel RENDU** — le périmètre « employé » de §7.6 est le **fichier**, jamais
  la surface. C'est S3 qui referme cet écart.
- **Rien n'est éprouvé hors d'un Chromium de bureau** : ni Firefox, ni Safari,
  ni mobile. **Rien du HiDPI.**
- **L'accessibilité au-delà du contraste et du mouvement réduit** n'est pas
  mesurée : navigation clavier complète, lecteurs d'écran, cibles tactiles
  minimales, ordre de tabulation.
- **Aucune primitive ne porte de rôle ARIA** : les primitives sont du CSS, et la
  sémantique reste au balisage que S3 écrira.
- **L'anneau de focus est vérifié NON EFFACÉ, jamais VISIBLE** : G2 interdit
  `outline: none`, et rien ne vérifie que l'anneau se voit sur le fond de chaque
  primitive — en particulier sur un bouton principal focalisé, dont le fond est
  `--accent`. `outline-offset` l'en écarte de 2 px, **c'est un raisonnement, pas
  une mesure**.
- **Aucun contrôle ne mesure une longueur** : G4 vérifie qu'une longueur passe
  par un token, **jamais que le bon token a été choisi**.
- **Les trois longueurs hors échelle de `style.css` restent**, et la clause
  « aucune longueur hors échelle » du §8 **reste fausse** jusqu'à S4.
- **`--police-mono` n'est toujours pas câblé** : S4 tranche, et S2 ne l'a pas
  rouvert — aucune de ses quatre familles n'a besoin d'une pile monospace.
- **Le plafond de 12 288 octets n'est calibré par rien.**
- **`tokens.ts` ne sait nommer que trois blocs** : `tokens.css` ne peut donc
  accueillir aucune autre requête média — mesuré, quinze écarts (D3).
- 🔴 **§7.4 ne compare jamais racine ⊆ clair** (§5.3) : un token de couleur
  oublié dans les **deux** blocs clairs lui est invisible.
- 🔴 **Les trois re-étiquetages ne sont vus par aucun contrôle**, et une
  mitigation partielle existe pourtant (§5.2) — elle n'est pas construite.
- **Aucune internationalisation** : rien ne dit qu'un bouton survit à un libellé
  plus long.
- **Le legacy n'est pas touché** — `assets/`, `web/`, `src/`, `index.js` —, et
  aucun contrôle ne le balaie.
- ⚠️ **Ni `primitives.html` ni `galerie-primitives.ts` n'ont de test**, comme
  `design.html` et `galerie.ts` : **une galerie qui cesserait de rendre une
  famille entière ne serait attrapée par aucun contrôle**, seulement par l'œil —
  et l'œil n'est pas passé (§4).
- **Aucune variable d'environnement** n'est introduite par S2, et aucune n'est
  lue. L'absence est déclarée parce qu'une absence se déclare.

---

## 9. Les onze divergences du plan, et leur sort

| # | Objet | Sort |
| --- | --- | --- |
| D1 | la liste rangeait `--succes`/`--alerte`/`--danger` en S3, la spec les confie à S2 | **la spec l'a emporté** ; les trois sont sortis en T3 et T5, et le contrôle l'aurait forcé de toute façon |
| D2 | l'état survol n'a aucun token, et `color-mix(…, black)` est **refusé par §7.2, mesuré** | **`--accent-survol`**, un seul token neuf, trois blocs, deux paires de plus. 52 paires, minimum **inchangé à 3,16** — la prédiction du plan a tenu. Contrastes **relancés le 20 août par `rapportDeContraste`** : sombre repos **7,73** → survol **9,39** ; clair repos **5,72** → survol **7,27**. Les deux survols **augmentent** le contraste |
| D3 | `prefers-reduced-motion` **ne peut pas** vivre dans `tokens.css` — mesuré, **quinze écarts** | la règle vit dans `base.css` ; garde **G7**, rouge versée |
| D4 | `design.html` à 231 pour une porte à 300 : les quatre familles n'y entrent pas | **cinquième entrée Vite**, `primitives.html` (199) ; `design.html` **non touché** |
| D5 | `verify-all.sh` : **dix** étapes, **dix-sept** en-têtes | **relevé, plus dérivé** — §6 |
| D6 | trois tokens annotés S2 décrivent une famille que la spec ne confie pas à S2 | **re-étiquetés avec leur raison** (§3), et le plan avait tort de dire qu'aucune mitigation n'est possible (§5.2) |
| D7 | le périmètre « employé » de §7.6 est le **fichier**, jamais la surface | **tenu, et déclaré sans le maquiller** (§8) |
| D8 | `--police-mono` reste à S4 | **non rouvert**, et la raison est inscrite auprès de l'entrée |
| D9 | les comptes de tests bougent sous les voisins | **arrivé** : `proto` 70 → 79 (3 → 4 fichiers), **pas de notre fait** |
| D10 | aucun contrôle ne mesure une longueur | **G4**, rouge versée — et sa portée est déclarée étroite (§5.4) |
| D11 | l'anneau de focus existe déjà ; le risque est de l'**effacer** | **G2**, rouge versée. Ce qu'il ne dit pas est au §8 |

---

## 10. Les journaux versés

Tous sous `docs/superpowers/plans/journaux-design-s2/`.

**Familles de lecture — MESURÉES, pas supposées** (commandes au §3 de la tâche 9
du plan) :

- `grep -lP '\x1b\['` sur le répertoire entier : **aucun fichier**, `exit=1`.
  **Aucune séquence ANSI** — ces journaux se `grep`ent à plat, sans `sed`.
- `file` : **« ASCII text »** ou **« Unicode text, UTF-8 text »** partout, trois
  fichiers portant en plus « with very long lines ».
- `grep -lc $'\r'` : **aucun fichier**, `exit=1`. **Aucun `\r` : LF partout.**

**Il n'y a donc qu'UNE famille de lecture**, contrairement aux trois de D9 et
aux quatre de D8 : ce sont des sorties `npm`/`node` sur l'hôte, jamais du
PowerShell distant.

| Fichier | Contenu |
| --- | --- |
| `commit.txt` | `HEAD`, `git status --porcelain`, horodatage |
| `build-{1,2}.log` | `npm run build` |
| `design-verifier-{1,2}.log` | les six contrôles + le build |
| `vitest-client-{1,2}.log` | 196 tests, 22 fichiers |
| `vitest-proto-{1,2}.log` | 79 tests, 4 fichiers |
| `typecheck-{1,2}.log` | `tsc --noEmit` |
| `orphelins-{1,2}.log` | §7.6 |
| `contraste-{1,2}.log` | §7.1 |
| `theme-7-5-{1,2}.log` | §7.5, 10 tests |
| `primitives-{1,2}.log` | G1 à G7, 9 tests |
| `poids-css-{1,2}.log` | §7.7 |
| `orphelins-sans-exclusion.log` | la rouge de l'exclusion, 9 écarts, `exit=1` |
| `poids-css.txt` | les feuilles bâties et leur taille |
| `rouge-{1..4}-*.log` | une rouge par critère de recette |
| `rouges-rejouees.log` | G2, G3, G4, G5, G7 rejouées + la mesure du blanchiment |
| `primitives-html-structure.log` | le contrôle **structurel** de la galerie — **pas** le jugement visuel |
| `trou-7-4.log` | la mesure du trou de §7.4 (§5.3) |
| `primitives-css-taille.log` | `primitives.css` commit par commit — 154 / 230 / 72 / 73 |
| `contraste-survol.log` | les quatre contrastes de `--accent-survol`, relancés |
| `tailles.txt` | le relevé de tailles du §7 |
| `verify-all.log` | l'exécution complète, `exit=0` |

---

## 11. Ce que S2 lègue

**À S3 :**

1. ⛔ **Aucune surface du produit n'emploie une primitive** — `primitives.css`
   n'est liée que par sa galerie. C'est S3 qui la lie à `shell.html` et
   `connexion.html`, et qui referme l'écart entre « un appelant écrit » et « un
   pixel rendu ».
2. ⛔ **Neuf entrées de la liste d'attente pour S3**, dont **trois
   re-étiquetées** vers une famille étiquette/pastille que S3 posera peut-être.
3. ⛔ **Le sélecteur de thème du PRODUIT reste à écrire** :
   `selecteur-theme.ts` est un **instrument**, pas du produit, et il n'a pas de
   test — statut déclaré, pas subi.
4. ⛔ **Ni `primitives.html` ni `galerie-primitives.ts` n'ont de test.**

**À S4 :**

5. ⛔ **`--police-mono`** — la seule entrée dont le sort soit encore ouvert : ou
   S4 le câble sur `#stats`, ou il le retire.
6. ⛔ **Les trois longueurs hors échelle de `style.css`**, et la clause du §8
   qu'elles rendent fausse.

**Sans sous-bloc assigné :**

7. 🔴 **Une mitigation partielle du re-étiquetage est possible** (§5.2), et le
   plan affirmait le contraire. Elle exige que le dépôt sache quel sous-bloc est
   courant.
8. 🔴 **§7.4 ne compare jamais racine ⊆ clair** (§5.3) : un token de couleur
   oublié dans les deux blocs clairs lui est invisible.
9. ⛔ **`tokens.ts` ne sait nommer que trois blocs** — le jour où un sous-bloc
   voudra surcharger un token sous une autre requête média, c'est
   `lireBlocsDeTheme` qui doit apprendre à nommer ses blocs, pas la règle qui
   doit se contorsionner.
10. ⛔ **Le plafond de 12 288 octets n'est calibré par rien**, et S2 en consomme
    **6 374**.
11. ⛔ **`primitives.test.ts` est à 270 pour une porte à 300, marge 30** : la
    plus étroite de S2. Toute addition substantielle appelle une extraction.
12. ⛔ **Le jugement visuel de S2 n'a pas été porté**, et les **onze** jugements
    humains attendent tous un œil.
