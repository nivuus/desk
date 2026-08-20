# Sous-bloc S3 — résultats : les deux surfaces existantes habillées

**Recette jouée le 20 août 2026.** Plan :
`docs/superpowers/plans/2026-08-19-design-system-s3.md` (`b57e6b8`).
Spécification : `docs/superpowers/specs/2026-08-19-design-system-design.md`
(`5b6b830`) — **modifiée par S3**, et sur un seul point : la clause « aucune
longueur hors échelle » du §8, que la revue transverse corrige (voir le §6).
Journaux : `docs/superpowers/plans/journaux-design-s3/` — **52 fichiers**,
**UNE SEULE FAMILLE DE LECTURE**, et c'est **mesuré, pas supposé**
(`familles-de-lecture.txt`).

⛔ **AUCUNE TÂCHE DE S3 N'A EMPLOYÉ LA VM WINDOWS.** ⑥ est un sous-projet
navigateur, et la spec §9 lui refuse toute recette sur VM.

**Variables d'environnement introduites par S3 : AUCUNE.**
*L'absence est déclarée parce qu'une absence se déclare.*

---

## 0. Ce que S3 a fait, et le fait qui gouverne tout le reste

🔴 **S3 EST LE PREMIER SOUS-BLOC DE ⑥ OÙ L'APPARENCE DU PRODUIT BOUGE
RÉELLEMENT.** S1 s'interdisait tout changement d'apparence ; S2 n'a lié ses
primitives à aucune surface du produit et l'a écrit lui-même — *« les dix-huit
tokens sortis de la liste ont un appelant ÉCRIT, pas un pixel RENDU »*. **Cet
écart est refermé**, et c'est un contrôle qui le dit, pas une intention : le
§7.9 neuf relève, à chaque exécution,

```
② A — les primitives atteignent le PRODUIT :
  client/index.html : aucune famille
  client/shell.html : bouton, message, surface
  client/connexion.html : bouton, champ, message, surface
```

**Les quatre familles de S2 atteignent le produit.** `index.html` — la fenêtre
de session — n'en emploie aucune, **et c'est voulu** : le critère ③ existe pour
mesurer qu'elle n'a pas bougé d'un octet.

---

## 1. Les cinq critères — verdicts, avec leur nombre d'exécutions

🔴 **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les
contrôles de ⑥ sont **déterministes** (spec §9) : la question « combien de fois
sur combien » **ne se pose pas ici et n'est pas empruntée** à une campagne qui,
elle, l'aurait posée. Les deux exécutions rendent des sorties **identiques,
caractère pour caractère** — vérifié par `diff`, la seule ligne qui diffère
entre `design-verifier-1.log` et `design-verifier-2.log` étant `✓ built in
721ms` contre `303ms`.

| # | Critère | Verdict | Exéc. | Le chiffre, **relevé** |
| --- | --- | --- | --- | --- |
| ① | **les huit contrôles sont verts** | **TENU** | **2** | `7/7 contrôle(s) vert(s)`, `exit=0` ; §7.5 dans `npm test`. **§7.1 : 52 paires, 0 échec, minimum 3,16 — INCHANGÉ** |
| ② | **la liste d'attente a RÉTRÉCI**, 10 → 1 | **TENU** | **2** | **48** déclarés / **47** employés / **1** orphelin = **1** en attente, `0 écart` ; **13** fichiers au périmètre |
| ③ | 🔴 **la fenêtre de session N'A PAS BOUGÉ** | **TENU sur ses TROIS volets** | **2** | (a) diff de **0 octet** ; (b) `dist/index.html` lie `socle-ZRS7erzW.css` et `main-CXZaIG5L.css`, **et rien d'autre**, 2 `<link>` ; (c) les deux actifs sont **octet pour octet identiques** à la base, `dist/index.html` aussi |
| ④ | **les primitives atteignent le produit** | **RELEVÉ** | **2** | §7.9 : **52** classes déclarées / **51** employées, **0 écart** ; le détail par famille et par surface est au §3 |
| ⑤ | **le poids CSS reste sous le plafond** | **TENU** | **2** | **8 011** octets / plafond **12 288**, marge **4 277** ; **+1 637** contre les 6 374 de S2, **remesurés des deux côtés** |
| — | le **jugement visuel** des deux surfaces | ⛔ **NON PORTÉ** | **0** | voir le §5 |

**Suites :** `client` **258** tests / **26** fichiers, `proto` **130** / **5**,
`typecheck` `exit 0` des deux côtés, **deux exécutions chacun**.
⚠️ **Ces comptes ne sont pas tous de S3** : le plan relevait `client` 219/24 et
`proto` 111/5 à son écriture, et les chantiers voisins ont bougé les deux entre
les deux relevés. **Ce que S3 ajoute est nommé au §4.**

### Le critère ③ en détail, parce que c'est celui qui pourrait se croire acquis

Le volet (c) demande **les mêmes hachages de contenu qu'à la base**, « relevés
des deux côtés, jamais supposés ». **La base a donc été RECONSTRUITE** :
`git archive 88bc963 client proto` vers un arbre jetable hors du dépôt,
`node_modules` lié depuis `client/`, puis `npx vite build`
(`build-base-88bc963.log`). Base retenue : **`88bc963`**, parent du premier
commit de S3 — et non le `920a1eb` du plan, parce que **des chantiers voisins
ont touché `client/` entre les deux** (`connexion.ts`, `fichiers/adaptateur.ts`,
`prefixe.ts`, relevé). Prendre `920a1eb` aurait attribué à S3 le travail des
voisins.

```
BASE  aa669b75847d36a8261fcb305976618a5cf8dd748352cbfa3625cfef631b5efe  socle-ZRS7erzW.css
HEAD  aa669b75847d36a8261fcb305976618a5cf8dd748352cbfa3625cfef631b5efe  socle-ZRS7erzW.css
BASE  5168f3fe137ba39ad9df5b78251e9d0f13e338ba95c4fbf196289e92905a690c  main-CXZaIG5L.css
HEAD  5168f3fe137ba39ad9df5b78251e9d0f13e338ba95c4fbf196289e92905a690c  main-CXZaIG5L.css
```

🔵 **ET LE MONTAGE SAIT VOIR UN CHANGEMENT — c'est la contre-épreuve, dans la
même exécution** : entre la base et HEAD, **sept actifs de `dist/assets` ont
changé de nom** et **deux feuilles CSS neuves sont apparues** (`shell-*.css`,
`connexion-*.css`). Un montage qui rendrait « identique » sur tout ne prouverait
rien. Il rend « identique » sur exactement les deux fichiers de la fenêtre de
session, et « différent » sur les sept autres.

**Et le critère ③ a été vu ROUGE** (`rouge-12-critere-3.log`) : une seule
déclaration ajoutée à `style.css` fait échouer ses **trois** volets — diff de
**418 octets**, `dist/index.html` liant `main-BMQobqa0.css`, hachage passé de
`5168f3fe…` à `5135eae0…`.

🔴 **UN TROISIÈME RELEVÉ EXISTE, APRÈS LA REVUE TRANSVERSE, ET IL FAUT LE LIRE**
(`critere-3-session-3-apres-revue.log`). La revue a corrigé **un mot** dans un
**commentaire** de `client/src/style.css` (« aucun des sept ne mesure une
longueur » → « des huit ») : **le volet (a) n'est donc plus littéralement vide
après elle** — 875 octets de diff, dont une seule ligne de contenu. **Les volets
(b) et (c) sont inchangés**, et ce sont eux qui portent la propriété produit :
les deux actifs restent octet pour octet ceux de la base.

🔵 **ET CE TROISIÈME RELEVÉ A TROUVÉ AUTRE CHOSE, qui vaut d'être retenu** :
`dist/index.html` **diffère** de celui de la base, **à taille rigoureusement
égale (3 661 octets des deux côtés)**, et d'**une seule ligne**. La cause est
que `client/src/design/amorce-theme.js` **part VERBATIM dans chaque page bâtie,
commentaires compris** — le greffon `guac-amorce-theme` de `vite.config.ts` lit
son texte brut et le rend tel quel dans le `<head>`, sans aucun transform. **Le
fichier le dit de lui-même**, et la revue en a fait l'expérience : *un mot
corrigé dans un commentaire de CE fichier-là arrive dans les cinq pages bâties.*
⛔ **Le mot n'a pas été remis à « sept » pour faire passer le contrôle** : il
était faux, et ce dépôt n'échange pas une vérité contre un nombre.
⚠️ **La comparaison octet à octet de `dist/index.html` est une exigence que
CETTE recette s'est ajoutée**, pas une du plan — dont le volet (c) porte sur les
**deux actifs**, tous deux identiques.

---

## 2. La liste d'attente : 10 → 1, et chaque sortie a son appelant

**Relevé commit par commit** (`critere-2-attente.log`) :

| Commit | Tâche | Entrées | Ce qui sort |
| --- | --- | --- | --- |
| `88bc963` | *(base)* | **10** | — |
| `715e21e` | T1, §7.4 élargi | **10** | aucune |
| `45f5521` | T2, §7.9 neuf | **10** | aucune |
| `b854b32` | T3, le ton | **10** | aucune |
| `83f4bbf` | **T4, page-shell** | **3** | `--t-2xl` `--e-5` `--e-6` `--e-7` `--t-xs` `--e-1` `--r-plein` |
| `cdc5199` | **T5, connexion** | **1** | `--t-3xl` `--lh-large` |
| `7361405` | T6, sélecteur | **1** | aucune |
| `2bfc2b6` | T7, mitigation | **1** | aucune — elle change la **forme**, pas le contenu |

**Aucune entrée n'est ENTRÉE dans la liste** (`comm -13`, ensemble vide).

**L'appelant de chacune des neuf, relevé par `grep` dans le périmètre du
contrôle** — pas un seul n'a été fabriqué pour vider une ligne :

| Token | Appelant | Fichier:ligne |
| --- | --- | --- |
| `--e-1` | `padding` de la pastille | `shell.css:147` |
| `--e-5` | gouttière de cartes, marges de titres, `gap` | `connexion.css:39,49,81` ; `shell.css:57,61,108` |
| `--e-6` | marge des sections | `shell.css:71` |
| `--e-7` | marge de tête de la surface | `shell.css:55` |
| `--lh-large` | interligne du bandeau de connexion | `connexion.css:77` |
| `--r-plein` | forme de la pastille | `shell.css:145` |
| `--t-2xl` | titre de la page-shell | `shell.css:65` |
| `--t-3xl` | titre de l'écran de connexion | `connexion.css:57` |
| `--t-xs` | étiquette de la pastille | `shell.css:149` |

**La dixième reste : `--police-mono`**, et **S3 ne l'a pas rouverte**. Ses deux
seuls appelants vivent dans `client/design.html`, **exclu du périmètre de §7.6**
par construction — c'est précisément ce qui la maintient orpheline *au sens du
produit*. Elle est à S4 : ou il la câble sur `#stats`, ou il la retire.

### 🔵 La mitigation du re-étiquetage — S2 la déclarait impossible, elle mord

S2 écrivait, trois fois, qu'« aucune mitigation technique n'est possible » contre
le re-étiquetage silencieux d'une entrée. **C'est faux, et S3 l'a construite**
(T7) : le sous-bloc nommé est devenu un **champ structuré** (`sousBloc: 'S4'`),
et `SOUS_BLOCS_CLOS = {S1, S2, S3}` rend rouge toute entrée qui réclamerait un
sous-bloc déjà clos. **Vue rouge** (`rouge-7-7-6-sous-bloc-clos-mitigation-T7.log`) :

```
inclusion ③ — aucune entrée ne nomme un sous-bloc clos (S1, S2, S3) : 1 écart(s)
  SOUS-BLOC CLOS  --police-mono  nommait S3, qui est clos : décider ou re-étiqueter avec sa raison
```

⚠️ **Elle est PARTIELLE, et le mot est pesé** — elle juge le **sous-bloc nommé**,
jamais le **contenu** de l'annotation, et elle dépend d'une liste tenue à la
main. ⚠️ **Et après S3 elle ne garde qu'UNE entrée : un mécanisme pour une
ligne.** L'objection est réelle ; la réponse est qu'une mitigation construite
*après* la faute qu'elle devait empêcher n'aurait plus rien à empêcher.

---

## 3. Le critère ④ : ce que §7.9 relève réellement, famille par famille

⚠️ **C'EST UN RELEVÉ, PAS UNE ASSERTION.** Le contrôle n'exige qu'**au moins
une** famille atteignant **au moins une** surface du produit. Ce que le tableau
ci-dessous dit, aucun contrôle ne l'exige :

| Surface | bouton | champ | message | surface (`carte`) |
| --- | --- | --- | --- | --- |
| `client/index.html` | — | — | — | — |
| `client/shell.html` | ✅ `bouton`, `bouton--secondaire`, `bouton--discret` | — | ✅ `message` | ✅ `carte`, `carte__titre`, `carte__corps` |
| `client/connexion.html` | ✅ `bouton`, `bouton--principal` | ✅ `champ`, `champ__etiquette`, `champ__saisie` | ✅ `message` | ✅ `carte` |

**Trois familles sur quatre atteignent la page-shell ; les quatre atteignent
l'écran de connexion.** `champ` n'atteint pas la page-shell **parce qu'elle n'a
aucun champ de saisie** — ce n'est pas une lacune, c'est le balisage.

**Et l'assertion ③ B** garde le sens inverse : chaque famille reste **rendue par
la galerie** — `bouton` 4/4, `champ` 6/6, `message` 4/4, `surface` 4/4.

🔴 **L'assertion ② A a été ROUGE SUR L'ARBRE INTACT, et c'est sa preuve
d'atteignabilité** : le commit `45f5521` (T2) a livré le contrôle **rouge**, et
la branche l'a porté rouge jusqu'à `83f4bbf` (T4), qui l'a éteint en habillant la
page-shell. **C'est ce que S1 avait fait entre ses tâches 1 et 9.** La rouge est
rejouée à la recette (`rouge-3-7-9-aucune-famille-au-produit.log`), et elle
rend le message que le contrôle existe pour dire :

```
AUCUNE classe de primitive employée par une surface du produit :
les primitives ont un appelant ÉCRIT, pas un pixel RENDU
```

---

## 4. Ce qui a CHANGÉ d'apparence — le contrat, et un huitième point que le plan ne prévoyait pas

**Le §5.1 du plan est un CONTRAT : tout ce qui n'y figure pas est une
régression.** Ses sept lignes sont livrées.

| # | Surface | Changement | Statut |
| --- | --- | --- | --- |
| ① | `shell.html` | page composée : titre en `--t-2xl`, sections espacées, **grille de cartes**, boutons de primitive | livré |
| ② | `connexion.html` | **carte centrée**, titre en `--t-3xl`, champs `.champ`, action `.bouton--principal` | livré |
| ③ | les deux | **trois boutons de thème apparaissent** | livré |
| ④ | `#statut` | prend un **ton** — neutre / danger | livré, **testé** |
| ⑤ | `#etat-fichiers` | prend un **ton** | livré, **testé** |
| ⑥ | `#message` (connexion) | prend un **ton** | livré, **testé** |
| ⑦ | les deux | anneau de focus **inchangé**, désormais visible sur des contrôles habillés | conséquence |

🔵 **UN HUITIÈME CHANGEMENT, QUE LE PLAN NE PRÉVOYAIT PAS, ET IL EST DÉCLARÉ
PLUTÔT QUE DISSIMULÉ** : `.message:empty { display: none }`
(`primitives/message.css:44`). **Un bandeau vide disparaît, il ne devient pas un
cadre vide.** Sans elle, l'état initial des trois bandeaux du produit — et
l'effacement délibéré que fait `shell.ts::lecteurDemonte` — laisserait un
rectangle bordé sans texte, c'est-à-dire que *l'effacement se lirait comme un
défaut d'affichage*.
⚠️ **Elle touche une PRIMITIVE de S2, donc les quatre familles, donc la
galerie.** Deux choses la rendent acceptable, et la seconde est mesurée :
① le §6.3 du plan exige qu'une règle écrite à l'identique dans les deux feuilles
de surface **remonte** dans les primitives ; ② `primitives.html` **n'a aucun
`.message` vide** — vérifié avant l'écriture —, donc **aucune des quatre familles
ne change d'apparence dans la galerie**.
⚠️ **Divergence de point de chute, déclarée** : le plan nommait
`primitives/surface.css` ; la règle porte sur la famille `message` et vit dans
`primitives/message.css`, auprès de ce qu'elle décrit.

### Ce que S3 ajoute aux suites, et ce qui n'est pas de lui

`client` est passé de **219** à **258** tests (24 → 26 fichiers). Les fichiers de
tests neufs ou grossis par S3, relevés :
`design/classes.test.ts` (**163**, neuf), `design/selecteur-theme.test.ts`
(**171**, neuf), `design/tokens.test.ts` (133 → **241**), `shell.test.ts`
(141 → **216**). ⚠️ **Le reste du mouvement n'est pas de S3** : `proto` passe de
111 à 130 sans qu'aucune tâche de S3 ne touche `proto/`.

---

## 5. ⛔ Le jugement visuel : NON PORTÉ

**Personne n'a ouvert `dist/shell.html` ni `dist/connexion.html`.** C'est
déclaré, **jamais remplacé par un « probablement »**, et jamais par une capture
d'écran que personne n'a regardée : un agent qui en prendrait une ne porterait
pas un jugement, il produirait une image.

**Ce n'est pas un critère** — le plan le dit (§tâche 8, risque n°10), et S2 ne
l'avait pas porté non plus.

🔴 **S3 AJOUTE QUATRE JUGEMENTS HUMAINS, ET LE TOTAL PASSE À QUINZE.** La spec
§8 en comptait huit, S2 en a ajouté trois :

| # | Ce qui n'est pas mesuré | Ce qui est mesuré à la place |
| --- | --- | --- |
| 12 | que la **grille de cartes** soit la bonne forme pour une liste de fenêtres | rien. C'est une décision de mise en page |
| 13 | que la **carte de connexion centrée** soit à la bonne largeur | rien — seules sa gouttière et ses marges viennent de l'échelle |
| 14 | que l'état **ouverte / fermée** dit par la seule **encre** d'une pastille se distingue assez | **son contraste** : l'encre employée est l'une des sept, mesurée sur les trois fonds par §7.1. ⚠️ **Distinguer deux états n'est pas lire un texte**, et WCAG ne le mesure pas ici |
| 15 | que **trois boutons côte à côte** soient la bonne forme de sélecteur de thème | rien |

**Aucun des quinze ne deviendra une mesure.** Ils rejoignent la liste déjà longue
du dépôt — `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
`TAILLE_MAX_SORTIE`, les paramètres `scrypt` de P2.

---

## 6. Chaque contrôle a été vu ROUGE — seize mutations, deux contre-épreuves

**Toutes versées** (`rouge-*.log`, `contre-*.log`, table dans
`rouges-rejouees.log`), **une mutation à la fois**, avec la même discipline sans
exception : `sha256` avant → mutation → **PREUVE que le diff est non vide** →
contrôle → `git checkout --` → `sha256` après **identique** →
`git status --porcelain` **vide**.

| Rouge | Contrôle visé | Ce qui a rougi | exit |
| --- | --- | --- | --- |
| 1 | §7.4, **clause ③ neuve de S3** | `--bord-fort est une couleur de la racine sans contrepartie claire` | 1 |
| 2 | §7.9 ① | `NON DÉCLARÉE bureau__titre--inexistant` | 1 |
| 3 | §7.9 **② A** | `AUCUNE classe de primitive employée par une surface du produit` | 1 |
| 4 | §7.9 ③ B | `famille CHAMP absente de client/primitives.html` | 1 |
| 5 | §7.6 ① | `var(--nexiste-pas)` non déclaré | 1 |
| 6 | §7.6 ② | `NOUVEL ORPHELIN --police-mono` | 1 |
| 7 | §7.6 **③, la mitigation T7** | `SOUS-BLOC CLOS --police-mono nommait S3` | 1 |
| 8 | §7.1 | 3 échecs, minimum tombé de 3,16 à **1,39** | 1 |
| 9 | §7.2 | `shell.css:162: color: #ff00aa` | 1 |
| 10 | §7.7 | `DÉPASSEMENT de 33 413 octets` | 1 |
| 11 | §7.3 | `ÉCHEC B shell.html : … aucune ne déclare --fond-0` | 1 |
| 12 | **critère ③** | les trois volets échouent (voir §1) | — |
| 13 | §7.5 (test unitaire) | `2 failed \| 8 passed` sur `theme.test.ts` | 1 |
| 14 | T3, la règle de **ton** | `3 failed`, dont `expected 'neutre' to be 'danger'` | 1 |
| 15 | T6, le sélecteur **rappelle `marquer()`** | `1 failed \| 7 passed`, `expected 'false' to be 'true'` | 1 |
| 16 | §7.9 ①, **blanchiment côté DÉCLARATION** | `NON DÉCLARÉE bureau__fantome` | 1 |

**Deux contre-épreuves de blanchiment, où le VERT est le résultat attendu :**
une couleur littérale glissée dans un **commentaire CSS** laisse §7.2 vert ; un
`class="…"` et un `.selecteur` glissés dans un **commentaire HTML** laissent §7.9
vert. **Et la rouge n°16 est leur pendant exigeant** : une classe déclarée
*seulement* dans un commentaire CSS **ne compte pas comme déclarée**, donc
l'employer rougit. C'est le piège que ce dépôt a payé trois fois — *« un garde
satisfait par le commentaire du fichier qu'il analyse »* —, éprouvé **dans les
deux sens**.

### 🔴 QUATRE DES SEIZE ROUGES ONT DÛ ÊTRE REFAITES, ET C'EST LE RÉSULTAT DE MÉTHODE DU SOUS-BLOC

**Une rouge qui rougit pour la mauvaise raison ne prouve pas ce qu'on lui fait
dire.** Les quatre cas, déclarés plutôt que dissimulés :

- **1 (§7.4)** — la mutation portait `^    --bord-fort` (quatre espaces) et
  n'atteignait qu'**un** des deux blocs clairs, l'autre étant indenté de huit
  sous son `@media`. Le contrôle rougissait bien… **sur la clause ①
  PRÉEXISTANTE** (`clair ≡ clair`), pas sur la clause ③ que S3 ajoute. Corrigée
  en `^ *--bord-fort` : les deux blocs tombent à 13, la clause ① tient, et
  **seule** la clause ③ échoue.
- **3 (§7.9 ② A)** — elle ne mutait qu'**une** surface sur deux, et y introduisait
  des classes inventées. `connexion.html` gardant ses quatre familles, ② A
  restait **satisfaite** ; l'`exit=1` venait de la clause ① (classes non
  déclarées). Refaite sur **les deux** surfaces, en ne retirant **que** des
  classes déclarées : la clause ① rend `0 écart`, et l'unique écart est ② A.
- **4 (§7.9 ③ B)** et **15 (T6)** — leurs mutations **n'ont rien muté** (0 ligne
  de diff, `exit=0`). **Le harnais l'a dit lui-même**, et c'est exactement
  pourquoi son étape ③ existe : il refuse de compter une rouge dont le diff est
  vide.

**Le harnais est donc le vrai livrable de méthode de cette recette** : il rend
impossible de verser une rouge qui n'a rien perturbé.

---

## 7. `scripts/verify-all.sh` : DIX-HUIT en-têtes, `exit 0`, deux fois

```
$ grep -c '^==> ' verify-all.log
18
$ grep '^exit=' verify-all.log
exit=0
```

**DIX-HUIT, et c'est RELEVÉ, jamais sommé** — c'est la divergence D5 de S2, qui
se rejoue à chaque addition : dix étapes du script, plus les **huit** en-têtes
d'une passe de `design:verifier` (un build et **sept** contrôles). Le plan
prédisait dix-huit ; **la prédiction est confirmée par la mesure, elle ne la
remplace pas**. Les deux exécutions rendent la **même liste d'en-têtes**
(`diff` vide).

🔵 **AUCUNE ÉTAPE ÉTRANGÈRE N'EST TOMBÉE, ET IL FAUT DIRE POURQUOI C'EST
REMARQUABLE.** Le brief de cette recette annonçait `verify-all.sh` **ROUGE**,
sur `plateforme : npm run typecheck` — `agents/canal.ts:147`, du fait de
l'élargissement d'union livré par le sous-bloc **G1** (`3bb7487`). **Remesuré au
moment d'écrire : l'étape passe.** La réparation a été livrée pendant cette
recette par le chantier voisin, commit **`9da33a1` — « apps(g1) : les deux
branches du canal, et l'arbre redevient VERT »**.
⚠️ **Ce rouge n'a jamais été de S3, et sa réparation ne l'est pas davantage** :
aucune tâche de S3 ne touche `plateforme/`. **Il est nommé ici pour qu'aucun
lecteur ne le porte au débit de ⑥**, et parce qu'une recette qui trouverait
l'arbre vert sans dire qu'on l'attendait rouge laisserait croire qu'elle n'a
rien regardé.
⚠️ **`plateforme : npm run test:sqlite` avait échoué TRANSITOIREMENT** lors d'une
exécution antérieure au périmètre de S3, la suite repassant seule ensuite. **Il
n'a pas été revu ici** : les trois exécutions le passent. Rapporté tel quel, sans
diagnostic — S3 n'en a pas.

✅ **UNE TROISIÈME EXÉCUTION, APRÈS LES DEUX COMMITS DE S3, SERT DE TÉMOIN DE
CLÔTURE** (`verify-all-3-cloture.log`) : `exit=0`, **18** en-têtes, `client`
**258**, `proto` **130**, `plateforme` **338** deux fois (SQLite puis Postgres).
⚠️ **Elle tourne sur un arbre que le chantier voisin G1 a modifié entre les deux
commits de S3** (`a481f35`) : elle établit que **l'arbre entier** est vert à la
clôture, pas que S3 seul le soit — c'est le §11 des risques du plan, et il est
respecté en le disant plutôt qu'en l'imputant.

---

## 8. Tailles relevées **par la commande**, APRÈS la dernière édition

⚠️ **Relevé APRÈS la revue transverse**, jamais avant : « une table relevée en
début de ronde serait fausse à la fin de la même ronde ». Journal :
`journaux-design-s3/tailles.txt`, qui porte **les DEUX relevés** — celui d'avant
la revue et celui d'après, pour qu'on puisse voir ce qu'elle a coûté.
⚠️ **Ce document est commité AVANT les éditions de la revue** (tâche 8 puis
tâche 9, l'ordre du plan) : les chiffres ci-dessous sont donc ceux du commit
SUIVANT, et c'est dit plutôt que dissimulé. Les vérifier au commit de la
tâche 9, pas à celui-ci.

**Dépôt entier, fichiers de plus de 500 lignes : DEUX**, les deux lignes de la
dette gelée — `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs`
**630**. **Aucun fichier de `client/`.**

| Fichier | Lignes | Porte | Marge |
| --- | --- | --- | --- |
| `client/verify-webrtc.mjs` | **494** | 500 | 🔴 **6** — ⚠️ **intouché par S3** |
| `client/src/design/primitives.test.ts` | **283** | 300 | 🔴 **17** — **inchangé**, aucune tâche n'y a ajouté d'assertion |
| `client/src/design/tokens.ts` | **244** | 300 | 56 |
| `client/design.html` | **243** | 300 | 57 |
| `client/src/design/tokens.test.ts` | **241** | 300 | 59 |
| `client/outils/tokens-orphelins.mjs` | **235** | 300 | 65 |
| `client/src/design/tokens.css` | **232** | 300 | 68 |
| `client/src/design/classes.ts` | **227** | 300 | 73 |
| `client/outils/tokens-orphelins/attente.mjs` | **227** | 300 | 73 — 🔴 **13 avant le seuil d'extraction de 240** |
| `client/src/shell.test.ts` | **216** | 300 | 84 |
| `client/src/shell-page.ts` | **216** | 300 | 84 |
| `client/primitives.html` | **203** | 300 | 97 |
| `client/outils/classes-employees.mjs` | **202** | 300 | 98 |
| `client/src/design/selecteur-theme.ts` | **189** | 300 | 111 |
| `client/src/style.css` | **184** | 300 | 116 |
| `client/src/design/selecteur-theme.test.ts` | **171** | 300 | 129 |
| `client/src/shell.css` | **167** | 300 | 133 |
| `client/src/design/classes.test.ts` | **163** | 300 | 137 |
| `client/src/shell.ts` | **154** | 300 | 146 |
| `client/src/design/primitives/bouton.css` | **108** | 300 | 192 |
| `client/src/connexion.css` | **103** | 300 | 197 |
| `client/shell.html` | **88** | 300 | 212 |
| `client/src/design/primitives.css` | **85** | **240** | 155 |
| `client/src/design/primitives/champ.css` | **80** | 300 | 220 |
| `client/src/design/primitives/message.css` | **74** | 300 | 226 |
| `client/outils/verifier-design.mjs` | **68** | 300 | 232 |
| `client/connexion.html` | **66** | 300 | 234 |
| `client/outils/blocs-de-theme.mjs` | **64** | 300 | 236 |
| `client/src/design/primitives/surface.css` | **50** | 300 | 250 |

🔴 **LA REVUE TRANSVERSE EST ELLE-MÊME UNE SOURCE DE CROISSANCE, et le relevé
ci-dessus est celui d'APRÈS elle** — c'est pourquoi il diffère du premier
(`tailles.txt` porte les deux). Elle a ajouté **+54 lignes de commentaire** à
`client/` : `shell.css` 159 → **167**, `primitives.css` 73 → **85**,
`connexion.css` 93 → **103**, `bouton.css` 103 → **108**, `champ.css` 75 →
**80**, `surface.css` 44 → **50**, `message.css` 71 → **74**,
`primitives.html` 199 → **203**, `attente.mjs` 226 → **227**. **C'est le piège
que S2 avait déjà payé** — sa propre revue transverse avait fait tomber la marge
de `primitives.test.ts` de 30 à 17.
✅ **`primitives.test.ts` n'a PAS bougé cette fois : 283, marge 17** — la revue
n'y a changé que trois mots de commentaire, à longueur égale.

🔴 **LE FICHIER À SURVEILLER EST `attente.mjs` : 227 lignes, et son seuil
d'extraction conditionnel est à 240 — marge 13.** Le plan le nomme
(`§tâche 7`) : *« si l'addition de doctrine porte le fichier au-delà de 240,
extraire — jamais compresser, et la doctrine part avec sa donnée »*. S2 a payé
**deux fois** dans ce fichier la leçon « une addition de commentaire annule une
extraction » ; S3 l'a porté de 146 à **227** en y inscrivant la justification de
neuf sorties, deux annotations corrigées, la mitigation, et trois déictiques
cassés que la revue a repris (« les dix entrées ci-dessous » quand il n'en reste
qu'une). **La prochaine addition substantielle appelle l'extraction**, pas une
compression.

⚠️ **`client/verify-webrtc.mjs` (494, marge 6) est INCHANGÉ**, et aucune tâche de
S3 ne le touche. ⚠️ **`client/src/main.ts` vaut 451 et n'est pas de S3.**

**Poids CSS bâti**, relevé **après** la revue transverse et **inchangé par
elle** — les minificateurs CSS retirent les commentaires :
`546 + 1 493 + 2 730 + 1 063 + 2 179 = **8 011**` octets,
plafond **12 288**, marge **4 277**. **Base S2 remesurée sur l'arbre reconstruit,
pas reprise du document : 6 374** — donc **+1 637**, dont **+546** pour
`connexion.css`, **+1 063** pour `shell.css`, et **+28** pour `primitives.css`
(la règle `.message:empty`). **`main` et `socle` n'ont pas bougé d'un octet.**
⚠️ **Le plafond de 12 288 n'est calibré par rien**, et il le reste.

---

## 9. Ce que S3 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, contrôles
  **déterministes** : reproductibilité, rien de plus.
- 🔴 **Aucun jugement visuel n'a été porté**, et les **quinze** jugements humains
  attendent tous un œil.
- **Rien hors d'un Chromium de bureau** : ni Firefox, ni Safari, ni mobile,
  **ni HiDPI**. Aucune page n'a même été ouverte dans un navigateur.
- **L'accessibilité au-delà du contraste et du mouvement réduit** : navigation
  clavier complète, lecteurs d'écran, cibles tactiles minimales, ordre de
  tabulation. **Aucune primitive ne porte de rôle ARIA** — les primitives sont du
  CSS, et la sémantique reste au balisage (`role="group"`, `aria-labelledby`,
  `aria-pressed` vivent dans le HTML et dans `selecteur-theme.ts`).
- 🔴 **L'anneau de focus reste vérifié NON EFFACÉ, jamais VISIBLE** — et S3 est
  le premier sous-bloc où un `.bouton--principal` focalisé existe réellement sur
  une page du produit, **sans que rien ne mesure qu'on le voie sur `--accent`**.
- **Aucun contrôle ne mesure une longueur** : G4 vérifie qu'une longueur passe
  par un token, **jamais que le bon token a été choisi**.
- **La bascule de thème entre deux fenêtres RÉELLES du produit** n'est pas
  éprouvée. `selecteur-theme.test.ts` la couvre par **injection** (D10 : `client/`
  n'a ni jsdom ni happy-dom), et S1 l'avait corroborée hors critère entre deux
  onglets de la **galerie** — **jamais entre une page-shell et les N sessions
  qu'elle ouvre par `window.open`**.
- **Aucune internationalisation** — rien ne dit qu'une carte survit à un libellé
  plus long.
- **`galerie-primitives.ts` et `galerie.ts` n'ont toujours pas de test.** §7.9
  assertion ③ B attrape « une famille cesse d'être rendue » ; **elle n'attrape
  pas un module de galerie cassé**.
- **Le sens « toute classe déclarée est employée » n'existe pas** dans §7.9 : il
  exigerait une seconde liste d'attente. Le contrôle relève `52` déclarées pour
  `51` employées, **et ne juge pas cet écart**.
- **Aucune primitive « lien »** — aucune ancre n'existe dans aucune des deux
  entrées (D11, mesuré). Poser une famille sans appelant serait le code mort que
  §7.6 refuse.
- **Le legacy n'est pas touché** et **aucun contrôle ne le balaie** : deux
  directions visuelles coexistent dans le dépôt jusqu'au remplacement.
- **Le plafond de 12 288 octets n'est calibré par rien.**
- ⚠️ **Le legs n°9 de S2 reste entier** : `lireBlocsDeTheme` ne sait toujours
  nommer que **trois** blocs, donc `tokens.css` ne peut accueillir aucune autre
  requête média. T1 ne l'a pas touché.

---

## 10. Les journaux versés

**52 fichiers**, tous sous `docs/superpowers/plans/journaux-design-s3/`.
**Une seule famille de lecture** — aucune séquence ANSI, aucun `\r`, UTF-8
partout : ils se `grep`ent **à plat**, sans `sed`. **Mesuré**
(`familles-de-lecture.txt`), pas supposé.

⚠️ **`familles-de-lecture.txt` a dû être REFAIT, et la raison mérite d'être
lue** : sa première rédaction écrivait ses propres motifs de recherche avec
`echo "…\x1b\[…"` et `echo "…$'\r'…"`. En zsh, `echo` interprète les
échappements : le fichier a reçu un **vrai** octet ESC et un **vrai** retour
chariot, et `file(1)` l'a classé « with CR, LF line terminators, with escape
sequences ». **L'instrument se polluait lui-même**, et un lecteur pressé aurait
conclu que S3 verse des journaux CRLF.

🔴 **LA PREUVE D'UNE AFFIRMATION NE VIT PAS DANS UN RAPPORT GITIGNORÉ.** D10 a
établi par la commande que l'espace de travail de D9 (`.superpowers/sdd/`) **a
disparu**, emportant **six** constats de revue définitivement perdus. Tout ce que
cette recette relève est **versé dans git**, et ce document porte l'analyse.

---

## 11. Ce que S3 lègue

**À S4 :**

1. ⛔ **`--police-mono`** — la seule entrée de liste d'attente dont le sort soit
   encore ouvert : ou S4 le câble sur `#stats`, ou il le retire. **S3 ne l'a pas
   rouvert**, et la mitigation de T7 empêchera qu'il soit re-étiqueté en silence.
2. ⛔ **Les SIX longueurs hors échelle de `client/src/style.css`** — et **non
   trois**, comme la spec §8 et le document de résultats de S2 l'écrivaient : le
   compte a été **remesuré** par la tâche 1 de S3, et la clause §8 est corrigée
   (§12). Elles vivent dans la fenêtre de session, **que seul S4 a le droit de
   toucher**.
3. ⛔ **L'écran plein cadre des états terminaux** et le **Window Controls
   Overlay** (spec §6). ⚠️ **WCO dépend du manifest de ②**, que ⑥ ne livre pas.
4. ⛔ **La fenêtre de session tout entière.** S3 ne l'a pas touchée, et le
   critère ③ le **mesure** — c'est le dernier sous-bloc où cette phrase sera
   vraie.
5. ⛔ **`attente.mjs` à 226 lignes pour un seuil d'extraction à 240** : la
   prochaine addition de doctrine **extrait**.

**Sans sous-bloc assigné :**

6. ⛔ **Le hub** — il n'existe pas, son contenu dépend de ④, et **⑥ ne le livre
   pas** (spec §6, §9).
7. ⛔ **Aucune primitive « lien »** ; **aucune ancre** dans aucune entrée.
8. ⛔ **`galerie-primitives.ts` et `galerie.ts` sans test.**
9. ⛔ **Le legs n°9 de S2** : `lireBlocsDeTheme` ne nomme que trois blocs.
10. ⛔ **Le plafond de poids CSS n'est calibré par rien.**
11. ⛔ **Le sens « toute classe déclarée est employée »** de §7.9 : il exigerait
    une seconde liste d'attente. **C'est `primitives.html` et l'œil qui le
    tiennent** — et l'œil n'est pas passé.

---

## 12. La revue transverse de fin de branche

Le détail des affirmations devenues fausses, avec leur `fichier:ligne` et leur
sort, vit dans la section **« Sous-projet ⑥ Design system — sous-bloc S3 »** de
`CLAUDE.md`, §⑦. Les deux journaux qui l'étayent sont
`journaux-design-s3/revue-transverse-enumeration.log` (l'énumération **avant**
édition) et `journaux-design-s3/longueurs-hors-echelle.log` (le compte de six,
avec sa règle énoncée avant de compter).
