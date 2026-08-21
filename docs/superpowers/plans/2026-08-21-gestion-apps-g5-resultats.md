# Sous-projet ④ — Gestion d'apps, sous-bloc **G5** : résultats

**Date** : 21 août 2026.
**Plan** : `docs/superpowers/plans/2026-08-21-gestion-apps-g5.md` (`06eafc1`).
**Conception** : `docs/superpowers/specs/2026-08-19-gestion-apps-design.md`, §G5,
et l'**amendement du 28/07/2026** au cadrage produit.
**Journaux** : `docs/superpowers/plans/journaux-gestion-apps-g5/`.

> ⚠️ **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les
> contrôles d'hôte de ce dépôt sont déterministes ; la question « combien de
> fois sur combien » **ne se pose pas ici** et n'est empruntée à aucune campagne
> qui, elle, la posait. **Aucun taux n'est revendiqué nulle part.**

---

## 0. Les familles de lecture des journaux

**UNE SEULE**, et c'est **mesuré, pas supposé** : tous les fichiers de
`journaux-gestion-apps-g5/` sont en UTF-8 ou ASCII, **sans séquence ANSI, sans
`\r`, sans octet NUL**. Ils se `grep`ent à plat, **sans `sed`, sans `grep -a`**.

**La raison est structurelle** : ce sont des sorties `npm`/`node`/`bash` sur
l'**hôte**, jamais du PowerShell distant. Le défaut à deux réglages de
`build-agent.sh` / `run-agent.sh` — toujours non corrigé — ne peut pas les
atteindre. ⛔ **AUCUNE TÂCHE DE G5 N'A EMPLOYÉ LA VM WINDOWS** (décision D7).

---

## 1. 🔵 LA PORTE P0 : **V1 REÇUE** — et le juge du plan n'était pas le bon

Le verdict complet, avec son tableau à six sondes, vit dans
`journaux-gestion-apps-g5/P0-VERDICT.md`. **Six sondes, deux exécutions
chacune.** Ce qu'il faut en retenir :

**Un manifeste que la page authentifiée construit elle-même, publie en `blob:`,
et dont les icônes sont des `data:`, est chargé, analysé et jugé INSTALLABLE par
Chromium.** Le témoin servi par HTTP ordinaire rend **exactement le même
relevé** : ce qui diffère entre `blob:` et HTTP est **rien**.

> 🔵 **AUCUNE ROUTE NE CHANGE, AUCUN CONTRÔLE DE PORTEUR NE SAUTE, ET AUCUNE
> DÉCISION DE SÉCURITÉ N'EST DEMANDÉE.** La voie V2 — ouvrir la route d'icône —
> et la voie V3 — une capacité signée dans l'URL — restent **présentées et non
> prises** : G2 avait écrit que ce choix « appartient au propriétaire du
> dépôt », et un sous-bloc suivant qui le prendrait en silence **le rendrait
> invisible**.

### 🔴 Le défaut du plan, trouvé par la mesure

Le plan juge le critère ① sur `Page.getAppManifest().errors`. **Mesuré : cette
liste reste VIDE sous P0-b** (icône 128, sous le seuil), aux deux exécutions.
Une icône trop petite **n'est pas une erreur d'ANALYSE de manifeste**. Le juge
est **`Page.getInstallabilityErrors`**, qui se remplit 2/2.

**Pris à la lettre, le plan aurait rendu le critère ① NON MESURABLE** — sa
propre ligne « `errors` vide aux deux ». Les deux listes sont conservées, et
**P0-f établit que `errors` se remplit bel et bien, pour autre chose**.

### Trois pronostics du plan réfutés ou dépassés

| Pronostic | Mesure |
| --- | --- |
| « P0-e ne se déclenche pas — attendu » | 🔵 **`beforeinstallprompt` SE DÉCLENCHE** en `--headless=new`, et **seulement quand l'application est installable** : il est présent aux dix exécutions vertes et absent aux quatre rouges. **Second discriminant, indépendant du premier** |
| P0-d pourrait exiger un service worker | 🔵 **AUCUN service worker n'est exigé** : `getRegistrations().length` vaut **0** et l'application est installable. La décision 2.3 reste un **choix**, pas une dépendance |
| ROUGE de ① « en dessous de **192 px** » (conception) | ⚠️ Le seuil est **144**, et Chromium le NOMME : `minimum-icon-size-in-pixels: 144`. **Sans conséquence** sur la ROUGE, dont l'icône témoin fait 128 — sous les deux seuils — **mais un successeur qui poserait 160 en se croyant sous le seuil lirait sa rouge ratée comme un produit correct** |

### 🔴 Deux contraintes de V1 que rien n'annonçait

1. **SOUS UN MANIFESTE `blob:`, LES URL DOIVENT ÊTRE ABSOLUES.** Le premier jet
   de l'instrument écrivait `start_url: '/p0.html…'`, `scope: '/'` — la forme de
   n'importe quel manifeste servi par HTTP. Chromium refuse :
   `property 'start_url' ignored, URL is invalid.` plus `start-url-not-valid`,
   et `beforeinstallprompt` ne se déclenche pas. **Le journal du défaut est
   versé** (`p0-0-instrument-defectueux-url-relatives.json`) et le cas est
   **rejouable** — c'est la sonde `f`, conservée pour cela. C'est la différence
   entre un manifeste installable et un manifeste refusé.
2. **Le seuil de 144** ci-dessus.

---

## 2. Les trois critères, et le legs de S4

**Montage** : base SQLite neuve, ensemencée **par les modules du dépôt**
(`depot/application.ts::appliquer`, `apps/icones.ts::ouvrirMagasin`), plateforme
vivante, client bâti et servi en statique **sur un autre port** — donc en
**origine croisée**, ce qui met la politique CORS du navigateur dans le chemin.
Chromium 151, `--headless=new`. **Deux exécutions, relevés identiques.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Une application découverte est installable en PWA | **TENU** | **2** + 2 rouges |
| ② | Le glisser-déposer fonctionne **sans aucun file handler** | **TENU** | **2** + 2 rouges |
| ③ | Le test empirique de l'amendement est joué, et son résultat écrit | **TENU** | **2** + 1 sonde |
| — | Le legs WCO de S4 | ⚠️ **DEMI-MESURE** — voir §5 | **2** |

### ① — l'installabilité, et **trois issues distinctes**

Le relevé, identique aux deux exécutions :

| Application | icône réelle | `icons` déclaré | `errors` | `getInstallabilityErrors` |
| --- | --- | --- | --- | --- |
| **Bloc-notes** | PNG 256 | `256x256` | 0 | 🔵 **`[]` — INSTALLABLE** |
| **Temoin-128** | PNG 128 | `128x128` | 0 | `manifest-missing-suitable-icon`, `no-acceptable-icon` |
| **Sans-icone** | aucune | `[]` | 0 | les deux mêmes |

Les trois manifestes sont chargés depuis une URL `blob:` (`estBlob: true`), et
les **trois icônes** des applications qui en ont une sont bien lues par `fetch`
authentifié puis publiées en objet (`iconesChargees: 3`).

🔴 **LA ROUGE DE ① N'EST PAS UNE MUTATION, C'EST UNE DONNÉE** — et c'est plus
fort : `Temoin-128` et `Sans-icone` sont des applications **réelles de la base**,
qui traversent **tout le chemin de production**. Une mutation de code aurait
éprouvé un binaire modifié ; celles-ci éprouvent le produit livré.

### ② — le glisser-déposer, et **deux rouges dont une d'atteignabilité**

Le fichier est déposé par un `DragEvent('drop')` portant un vrai `DataTransfer`
et un vrai `File`, dispatché **sur le véritable écouteur du produit** — pas sur
une fonction exposée pour l'occasion.

| Bras | Ce qui change | Bandeau après le dépôt |
| --- | --- | --- |
| **VERT** | rien | `temoin-g5.msi a été téléversé et scellé (1 tranche(s) déposée(s)).` |
| **ROUGE 3** | `if ('launchQueue' in window)` → `if (false)` | **le même** — 🔵 **le dépôt aboutit SANS `launchQueue`** |
| **ROUGE 3bis** | l'écouteur `drop` est vidé | **inchangé** (`4 application(s) sur g5.`) |

🔴 **LA ROUGE 3bis EST CE QUI EMPÊCHE LA ROUGE 3 D'ÊTRE VACUEUSE.** Sans elle,
un dépôt qui **ne fonctionnerait pas du tout** passerait la ROUGE 3 exactement
comme un dépôt indépendant. C'est le patron que ce dépôt a payé six fois.

⚠️ **`launchQueue` EXISTE en `--headless=new`** (`'launchQueue' in window` vaut
**`true`**, mesuré) : la ROUGE 3 retire donc **du code vivant**, et non du code
mort. Son consommateur ne se déclenche jamais, faute de PWA installée.

**L'amendement du 28/07/2026 est GARDÉ.**

⚠️ **Un `DataTransfer` construit dans la page n'est pas un geste humain**, et
c'est écrit ici plutôt que laissé croire. Ce qu'il exerce est le **vrai
écouteur**, avec de vrais `dataTransfer.files`. `showOpenFilePicker` reste hors
d'atteinte d'un Chromium sans interface (mesuré par F1).

### ③ — le test empirique, **et il tranche**

Le manifeste du hub est chargé depuis `/hub.webmanifest`, `errors` est **vide**,
et son `data` porte les trois types :

```
application/x-msi                                  .msi
application/vnd.microsoft.portable-executable      .exe
application/x-bat                                  .bat
```

⚠️ **Mais le manifeste PARSÉ que le CDP rend n'expose AUCUN `fileHandlers`.**
Deux lectures s'opposaient — ou Chromium les rejette, ou le CDP ne les sérialise
pas —, et **un `errors` vide ne départage pas** : il serait vide aussi si
Chromium ignorait le membre.

🔵 **UNE SONDE TRANCHE, ET C'EST LE RÉSULTAT LE PLUS UTILE DE CE CRITÈRE.** Une
`action` posée **hors du `scope`** fait rendre à Chromium, verbatim :

```
property 'action' ignored, should be within scope of the manifest.
FileHandler ignored. Property 'action' is invalid.
```

**Chromium ANALYSE donc bel et bien `file_handlers`**, et l'`errors` vide du
bras vert est une **acceptation**, pas une indifférence. Les trois types
survivent à l'analyse.

🔴 **CE QUI N'EST PAS MESURÉ, ET QUI EST LA MOITIÉ DU PROBLÈME.** L'amendement
nomme **quatre** obstacles ; ce montage n'en éprouve **qu'un** :

| Obstacle | Éprouvé ? |
| --- | --- |
| Chromium accepte-t-il un handler pour un type exécutable ? | ✅ **OUI, à l'analyse du manifeste** — jamais à l'enregistrement d'une PWA installée |
| Sur Windows, `.exe` n'est pas une association ; `.msi`/`.bat` sont protégés par le hash `UserChoice` | ❌ **non éprouvé** — il faudrait un poste Windows client |
| Sur Linux/macOS, `.exe` n'a aucune association | ❌ **non éprouvé** |
| ChromeOS, « le meilleur candidat » | ❌ **indisponible** |

**Le critère est TENU parce que le test est joué et que son résultat, y compris
sa portée, est écrit.**

---

## 3. 🔴 LE DÉFAUT QUE LA RECETTE A TROUVÉ : le manifeste mentait sur son icône

Un premier jet de `batirManifeste` prenait un `coteIcone` **optionnel** valant
**256** par défaut — la seule taille que le magasin connaisse
(`agent/src/apps/icone/extraction.rs:45`). **`page.ts` ne le passait pas.**
Résultat mesuré sur le chemin réel : `Temoin-128`, dont l'icône fait 128,
publiait un manifeste annonçant **`256x256`**.

**Chromium l'a attrapé** — il décode l'image et rend `no-acceptable-icon` —
**mais un manifeste qui ment sur ce qu'il porte est un défaut même quand le
navigateur le rattrape** : sur une icône de 200 px annoncée 256, il aurait
**accepté**, et le système aurait mis à l'échelle une image qu'il croyait plus
grande.

⚠️ **AUCUN TEST D'HÔTE NE POUVAIT LE VOIR** : le défaut était dans l'**appelant**,
qui omettait un paramètre **optionnel**. Les 22 tests de `manifeste.ts` étaient
verts.

**Le remède RETIRE le paramètre au lieu de le rendre obligatoire** : la taille
est **lue dans les octets** (`cotePng`, l'en-tête IHDR, pur, 6 tests). Un
appelant ne peut plus se tromper puisqu'il n'a plus rien à dire. Et **une icône
dont on ne sait pas lire la taille n'est plus déclarée du tout** — poser
`256x256` par défaut serait affirmer ce qu'on ne sait pas.

---

## 4. Les rouges — huit jouées, et **le harnais en refuse une**

Toutes au harnais en huit étapes : `sha256` → **copie nommée** → l'ancre existe
**exactement une fois** (un compte, jamais une relecture) → mutation → **preuve
que le diff CONTRE LA COPIE est non vide** → contrôle → restauration **depuis la
copie** → `sha256` **égal**.

🔴 **JAMAIS `git diff`**, qui compare à **HEAD** et reste non vide tant qu'un
travail non commité vit dans le fichier — **même quand la mutation est nulle**.
🔴 **JAMAIS `git checkout --`**, qui restaure à HEAD et a **effacé du travail non
commité deux fois** dans ce dépôt.

| # | Ce qu'elle mute | Ce qui tombe |
| --- | --- | --- |
| **0** | **rien** | ⛔ **le harnais REFUSE** : `DIFF VIDE — la mutation n'a rien muté, ce n'est pas une rouge` |
| **3** | `launchQueue` retiré | 🔵 **rien** — le dépôt aboutit. **C'est le critère ②** |
| **3bis** | l'écouteur `drop` vidé | le dépôt **cesse** — l'atteignabilité de ② |
| **4** | le `<link rel="manifest">` du hub | `url` **vide**, `data` **nul**, `no-manifest`. ⚠️ **`errors` reste `[]`** — le piège du plan, confirmé : **c'est l'URL qui tranche, jamais `errors`** |
| **5** | l'`id` du manifeste devient constant | **3 tests sur 22**, dont « deux applications ont des `id` DISTINCTS » |
| **6** | `env(titlebar-area-height, 8px)` dans `style.css` | **DEUX contrôles indépendants** : l'assertion ① du garde WCO de S4, **et** §7.10 (`8px` hors token) |
| **7** | une `@media (display-mode: window-controls-overlay)` dans `hub.css` | **UNE seule** des six assertions — l'assertion ② du garde de S4. **Elle rougit pour la BONNE raison**, et prouve que le garde de S4 **s'étend à une feuille née après lui** |
| **8** | `var(--accent-fenetre, var(--accent))` dans `hub.css` | §7.6 : `NON DÉCLARÉ --accent-fenetre employé par client/src/hub/hub.css` |

> 🔵 **LA ROUGE 8 EST CE QUI FONDE LA DÉCISION D3 SUR UNE MESURE, ET ELLE
> RÉFUTE UNE CLAUSE D'A1.** Elle a été jouée **avec le repli** —
> `var(--accent-fenetre, var(--accent))` — et §7.6 rougit **quand même**. La
> phrase d'A1 « toute référence future doit porter un repli **ou** déclarer le
> token » laisse croire que le repli suffit : **il ne suffit pas**. Le plan
> l'avait prédit en lisant la regex de `tokensReferences` ; la rouge le prouve
> sur le contrôle réel.

⚠️ **Les ROUGES 1 et 2 du plan n'ont pas été jouées comme MUTATIONS**, et c'est
mieux : elles sont **des données** — `Temoin-128` et `Sans-icone` traversent le
chemin de production à chaque exécution verte (§2 ①).

---

## 5. ⚠️ Le legs de S4 — le Window Controls Overlay : **DEMI-MESURE**

**Ce qui EST mesuré, et c'est neuf pour ce dépôt** : Chromium **accepte et
retient** la déclaration. Son manifeste parsé porte
`displayOverrides: ["kWindowControlsOverlay", "kStandalone"]`, aux deux
exécutions, pour le hub **comme** pour chaque application.

**Ce qui NE l'est PAS** :

- `matchMedia('(display-mode: window-controls-overlay)').matches` vaut
  **`false`** — aucune fenêtre à barre superposée n'est atteignable ;
- `env(titlebar-area-height)` est **supporté** par le moteur, mais ses variables
  **ne sont définies que dans une fenêtre de PWA installée**, et un Chromium
  sans interface n'en installe aucune.

> 🔴 **LE VERDICT EST CELUI QUE LE PLAN ÉCRIVAIT D'AVANCE : `NON MESURABLE PAR
> CE MONTAGE`, et le legs de S4 RESTE ENTIER.** G5 apporte **la déclaration, et
> il ne prétend pas à plus**. S4 l'avait dit en livrant le WCO sans critère :
> « un critère vacueux est pire qu'un critère absent : il se lit comme une
> preuve. »

⚠️ **Le garde de forme de S4 reste VERT, et `client/src/style.css` n'a pas été
touché** — la tâche **pose la condition** qui rend la règle atteignable, et
**relève**. Les rouges 6 et 7 montrent qu'il mord toujours.

---

## 6. 🔴 Le hub n'est PAS installable, et ses `file_handlers` ne peuvent donc pas être honorés

Relevé aux deux exécutions : `installabilityErrors` du hub vaut
`manifest-missing-suitable-icon` + `no-acceptable-icon`. **Le manifeste du hub
n'a aucune icône.**

**Conséquence, écrite plutôt que tue** : `file_handlers` n'a d'effet que pour une
PWA **installée**. Un hub qu'on ne peut pas installer ne peut **jamais** voir ses
handlers honorés par le système.

⚠️ **G5 NE FABRIQUE PAS D'ICÔNE POUR AUTANT**, et c'est une décision : ⑥ est
clos, sa spec §10 dit « ④ livre un hub **fonctionnel** ; ⑥ l'habille », et le
§2.3 du plan interdit à G5 d'inventer une direction visuelle. Poser un aplat de
couleur ferait **paraître** le critère ③ meilleur sans rien mesurer de plus — les
trois obstacles non éprouvés du §2 ③ le resteraient. **Legs nommé.**

🔵 **Les manifestes PAR APPLICATION, eux, ont leur icône et SONT installables** —
et c'est ce que le critère ① de la conception demande.

---

## 7. La tranche F — **JOUÉE**, et mesurée sur la VM

Relevé complet : `journaux-gestion-apps-g5/13-porte-tranche-F.log`,
`15-tranche-F-vm-manifeste.log`, `16-tranche-F-vm-base.log`.

### 7.1 La porte a été évaluée DEUX FOIS, et elle a changé d'avis

⚠️ **Le premier relevé reste VRAI À SON HEURE, et il n'est pas effacé.** À
l'heure prescrite par le plan — avant la tâche 17 — **G4 n'était pas clos**
(cinq fichiers non commités sous `agent/src/apps/surveillance/`), **A1 non plus**
(tâches 13 et 14 non faites), et **la VM était prise**. La table du plan donne
alors un seul verdict : **RETIRÉE**, et c'est ce qui a été écrit.

Une heure plus tard, mesuré par la commande : **G4 clos**, **A1 clos et ayant
rendu la VM**, `git status --porcelain proto/ agent/ plateforme/` **vide**. La
même table donne l'autre verdict : **JOUÉE**.

> 🔵 **CE QUE CE RENVERSEMENT ENSEIGNE, ET QUI VAUT AU-DELÀ DE G5 : une porte
> qui lit l'état des voisins n'a de verdict qu'ASSORTI DE SON HEURE.** Le premier
> relevé était juste, sa conclusion l'était aussi, et les deux ont cessé de
> l'être en une heure — **sans qu'aucune ligne du plan ne bouge**. C'est la
> leçon que D11 a payée sur un compte de tests, transposée à une **décision**.

### 7.2 Ce qu'elle livre — et les deux clauses de la spec qu'elle ferme

| Champ | Ce qu'il ferme | Où il aboutit |
| --- | --- | --- |
| `accent` | la clause « couleur d'accent » du §G5 — qui n'existait **nulle part** dans ④ (divergence **E1**) | le `theme_color` du manifeste par application |
| `associations` | la clause « `file_handlers` alimentés par les **vraies** associations » (divergence **E2**), dont le verbe supposait un tuyau qui n'avait **jamais** existé | les `file_handlers` par application |

**En UN SEUL COMMIT** (D16) : `PLATEFORME_VERSION` 4 → 5, les deux champs, le
miroir TypeScript, les gardes, les vecteurs partagés, l'agent, la migration
`0007`, le dépôt, la route et le client. `Application` porte
`#[serde(deny_unknown_fields)]` : **tout champ neuf est cassant**, et aucun des
deux ordres possibles ne compilerait — c'est exactement ce qu'A1 vient de payer.

### 7.3 🔵 Ce que la VM a rendu, et une corroboration que personne n'a conçue

Binaire rebâti après `cargo clean --release -p proto -p agent`, et **vérifié
mien par TROIS chaînes** : une chaîne **neuve** de G5 (**1**), un **témoin
négatif** (**0**), et une chaîne **préexistante** (**1**).

> 🔴 **C'EST LA TROISIÈME QUI PORTE.** Sans elle, un `0` d'absence et un `0` de
> mauvais chemin se liraient **exactement pareil** — le zéro qui a menti à A1
> quelques heures plus tôt, parce qu'il interrogeait `/media/vm/dev/agent.exe`,
> qui n'existe pas.

Réconciliation réelle : `total=220 retenus=169 cles=156 icones=156
icones_echouees=0`.

| Grandeur | Relevé |
| --- | --- |
| applications en base | **156** |
| avec un accent | **149** |
| accents **distincts** | **87** |
| **sans accent MAIS avec une icône** | **7** |
| lignes d'association | **221**, sur **21** applications |

🔵 **LES 7 SANS ACCENT SONT LE RELEVÉ LE PLUS UTILE DE CETTE MESURE** : elles
**ont toutes une icône**. Leur `null` ne vient donc pas d'une extraction ratée
mais de la **clause 5** de `dominante` — trop pâle, trop sombre, trop
transparent. **La clause est ATTEIGNABLE sur des données réelles**, ce qui la
distingue d'un ornement.

🔵 **ET LES ACCENTS SE CORROBORENT EUX-MÊMES, SANS QUE PERSONNE NE L'AIT
CONÇU** : Photoshop rend `#30a7fe`, Illustrator `#fe9900`, Creative Cloud
`#f90d06`, InDesign `#fe3265`. **Photoshop EST bleu, Illustrator EST orange,
Creative Cloud EST rouge**, et aucune de ces valeurs n'a été choisie par nous.
⚠️ **C'est une CORROBORATION, jamais une preuve** : elle ne vaut que parce que
ces icônes sont **fortement colorées**, et elle ne dit rien du sens des canaux
sur une icône terne. **C'est le test d'hôte de `bgra_en_rgba` qui tient ce
sens-là** — le même arbitrage qu'A1 a rendu sur sa propre conversion.

### 7.4 Le manifeste par application, dans un vrai navigateur

Sur `Adobe Photoshop 2024`, catalogue réel de la VM, manifeste `blob:` :

```
manifeste blob:      : true
errors               : []
installabilityErrors : []          ← INSTALLABLE
theme_color          : #30a7fe     ← le bleu de SON icône
icons                : ["256x256"]
file_handlers        : action http://…/hub.html?app=<uuid>, 42 extensions
```

⚠️ **La plupart des extensions tombent sur `application/octet-stream`** — ce
sont des formats bruts d'appareil photo que la carte MIME ne connaît pas. C'est
le comportement **déclaré** (« le type honnête pour des octets dont on ne sait
rien »), et **Chromium l'accepte** : `errors` reste vide.

### 7.5 🔴 Un défaut de MONTAGE qui a failli se lire comme un défaut de produit

La première tentative a rendu **`applications au hub : 0`**. La cause n'était ni
le hub ni la route : **`PLATEFORME_ORIGINE_CLIENT` n'avait jamais atteint le
processus qui écoute**, parce qu'un `kill %1` avait visé un job du shell au lieu
du PID. Le préflight rendait bien `204`, **sans un seul en-tête
`Access-Control-Allow-Origin`** — donc le navigateur refusait, en silence.

**Diagnostiqué par la méthode que ce dépôt tient depuis le chantier TURN** :
lire l'environnement du **processus qui écoute réellement**, par
`/proc/<pid>/environ`, et non de celui qu'on croit avoir lancé. C'est la classe
« ce qu'un navigateur exige et qu'un test serveur ne voit pas », que P4 a
rencontrée sur CORS et qui **n'a toujours aucun garde automatique**.

## 8. Ce que G5 n'établit PAS

- **Aucun taux, nulle part.**
- 🔴 **Rien du WCO EN FONCTIONNEMENT** (§5). Le legs de S4 reste entier.
- 🔴 **Rien de l'invite d'installation** : l'hôte n'a ni `DISPLAY`, ni `Xvfb`, ni
  `xdotool`. `beforeinstallprompt` dit que Chromium **tient l'application pour
  installable** ; il ne dit **rien** de l'installation.
- 🔴 **Rien d'une PWA réellement installée** : ni son cycle de vie, ni la
  re-recherche de son manifeste **après la mort de l'onglet qui a créé le
  `blob:`**. **C'est le coût déclaré de V1, et il reste entier.**
- 🔴 **Trois des quatre obstacles de l'amendement restent non éprouvés** (§2 ③).
- **Rien de deux applications installées côte à côte** : que des `id` distincts
  sous un `scope` partagé donnent deux applications distinctes n'est **pas
  mesurable ici**.
- **Rien du lancement de bout en bout depuis une PWA** : la lecture de `?app=`
  par `shell-page.ts` est **livrée, et exercée par aucun critère**.
- **Aucun agent réel n'a produit le catalogue DES TROIS CRITÈRES** (D7) : leur
  base est ensemencée par les modules du dépôt. ⚠️ **La tranche F, elle, a bien
  été mesurée sur un catalogue produit par l'agent réel** (§7.3) — mais elle ne
  sert **aucun** des trois critères, et les deux mesures ne se recouvrent pas.
- 🔴 **Rien du chemin `file_handlers` par application AU-DELÀ DE L'ANALYSE** :
  Chromium accepte le membre, et **aucune PWA n'a été installée** pour que le
  système d'exploitation l'honore. Les trois obstacles non éprouvés du §2 ③
  valent pour les manifestes par application exactement comme pour celui du hub.
- ⚠️ **La carte extension → MIME n'est pas calibrée** : sur le corpus réel, la
  plupart des extensions tombent sur `application/octet-stream`.
- **Rien d'un navigateur autre que Chromium** — ni Firefox, ni Safari. La File
  System Access API n'y existe pas : limite du **produit**, pas de la recette.
- **Rien du HiDPI** : `deviceScaleFactor = 1`, limite héritée de D5.
- 🔴 **AUCUN JUGEMENT VISUEL, sur aucune page.** Personne n'a jamais ouvert une
  page de ⑥ dans un navigateur, de S1 à S4, et G5 ne l'a pas fait davantage. Les
  **vingt-cinq** jugements humains de ⑥ restent entiers, et G5 en ajoute — voir
  le §10.
- **Aucun service worker**, et donc rien du verrou C5 du retrait du legacy.
- **`--accent-fenetre` reste sans déclaration et sans appelant** (D3).
- **Aucune constante calibrée** : les trois types MIME de D11, et le plafond de
  poids CSS que G5 rapproche sans le calibrer. Elles rejoignent la liste que ce
  dépôt tient depuis `BPP_MIN`.
- **Aucun audit de sécurité** : le modèle de menace reste celui de ⑤.
- **Rien de la latence**, qu'aucun sous-bloc n'a jamais mesurée depuis D1.

---

## 9. Pièges — à connaître avant de toucher à ce terrain

- 🔴 **`Page.getAppManifest().errors` N'EST PAS UN VERDICT D'INSTALLABILITÉ.**
  C'est une liste d'erreurs d'**analyse**. Une icône trop petite la laisse
  **vide**. Le verdict est `Page.getInstallabilityErrors`. **Les deux sont
  nécessaires**, et confondre l'une pour l'autre rend un critère non mesurable
  en le faisant paraître vert.
- 🔴 **`getAppManifest` rend un objet MÊME QUAND LA PAGE N'A PAS DE MANIFESTE.**
  Mesuré par la ROUGE 4 : `errors` vaut `[]`, `data` est nul, et **c'est l'`url`
  vide qui le dit**.
- 🔴 **SOUS `blob:`, LES URL D'UN MANIFESTE DOIVENT ÊTRE ABSOLUES** (§1).
- 🔴 **UN `**` SUIVI D'UN `/*` FERME UN COMMENTAIRE CSS**, et le build échoue sur
  « Unknown word » **à une ligne sans rapport**. C'est le piège que S1 a payé
  dans `vite.config.ts` — où `src/**` suivi de `/*.ts` fermait un commentaire
  JS —, rejoué dans une autre syntaxe. **Ne pas écrire de glob dans un
  commentaire.**
- 🔴 **UN IMPORT SANS SON EXTENSION `.ts` DANS `vite.config.ts` LAISSE LE BUILD
  VERT ET CASSE DEUX CONTRÔLES DE ⑥.** `outils/tokens-orphelins.mjs` et
  `outils/classes-employees.mjs` **importent ce fichier** pour en dériver leur
  périmètre, et ils sont chargés par **Node**, dont le résolveur exige
  l'extension là où Vite s'en passe. Symptôme : `ERR_MODULE_NOT_FOUND` sur §7.6
  et §7.9, **build parfaitement vert**.
- 🔴 **§7.2 BALAIE LES `.ts` AUTANT QUE LES `.css`.** Une couleur écrite dans un
  module TypeScript est relevée. **Le remède est de retirer la couleur, jamais
  d'élargir l'exclusion** — ce serait satisfaire un contrôle en le vidant.
- ⚠️ **`Uint8Array` N'EST PAS UN `BlobPart`** : il vaut désormais
  `Uint8Array<ArrayBufferLike>`, possiblement adossé à un `SharedArrayBuffer`.
  Trouvé par `npm run typecheck`, **jamais par Vitest**, qui transpile sans
  vérifier les types. Même piège que `televersement.ts` a documenté sur
  `InitHttp.body`.
- ⚠️ **UNE PAGE SERVIE SUR UN AUTRE PORT QUE LA PLATEFORME EXIGE
  `?plateforme=`** : `adressePlateforme` retombe sur l'origine de la **page**,
  ce qui est le cas **nominal** derrière le proxy inverse de P5. Le symptôme est
  un hub qui ne peuple jamais sa liste, sans erreur de page ni ligne de console.
- ⚠️ **LE CORPS DE `POST /auth/connexion` ATTEND `motdepasse`, EN UN MOT** — pas
  `motDePasse`. Un `400 {"refus":"forme"}` est la seule indication (G1).
- ⚠️ **UN PORT « LIBRE PAR CONVENTION » NE L'EST PAS** : le montage les relève
  par `ss -ltn`. G3 et P4 ont chacun perdu une exécution ainsi.
- ⚠️ **UNE MUTATION DU CLIENT EXIGE UN REBUILD AVANT LA RECETTE**, sans quoi la
  recette sert le build d'**avant** et se croit verte en mesurant le produit
  intact. Le contrôle des rouges de client est donc `rebuild && recette`.

---

## 10. Les jugements humains que G5 ajoute

**Énumérés, non comptés de mémoire** — S4 en prévoyait sept et en a produit dix :

1. que la **grille de cartes** soit la bonne forme pour une liste
   d'applications ;
2. qu'une **icône de 64 px** (`--e-8`) soit la bonne taille dans cette grille ;
3. que la **zone de dépôt en pointillés** soit lisible comme telle ;
4. que **« Lancer » en principal et « Installer » en discret** soit le bon
   partage d'importance ;
5. que le **titre du hub** en `--t-2xl` soit au bon cran.

**Aucun ne deviendra une mesure**, et **aucun n'a été porté** : personne n'a
ouvert `dist/hub.html` dans un navigateur (§8).

---

## 11. Ce que G5 lègue

1. 🔴 **Le hub n'est pas installable, faute d'icône** (§6) — donc ses
   `file_handlers` ne peuvent **jamais** être honorés. Destinataire : le
   propriétaire du dépôt, ou ⑥ s'il rouvrait.
2. 🔴 **Le WCO n'a jamais été rendu** (§5). Le legs de S4 est **transmis avec sa
   raison**, pas éteint.
3. 🔴 **Une URL `blob:` morte** : le manifeste n'existe que dans l'onglet qui l'a
   construit. Le comportement de Chromium quand une PWA installée re-cherche son
   manifeste **n'est mesuré par rien**.
4. ⛔ **Trois des quatre obstacles de l'amendement** restent non éprouvés.
5. ✅ **La tranche F est JOUÉE** (§7) — ce legs n'existe plus. ⛔ **Ce qu'elle
   laisse, en revanche** : le hub n'a **aucune icône** (§6), et le portage MIME
   de la carte d'extensions n'est **pas calibré** — la plupart des extensions
   réelles de la VM tombent sur `application/octet-stream`.
6. ⛔ **`--accent-fenetre` reste sans déclaration et sans appelant**, et **la
   scission de `client/src/design/tokens.css`** — 300/300, **neuf** lecteurs —
   reste due au premier chantier qui devra déclarer un token. La note vit dans
   `journaux-gestion-apps-g5/20-scission-tokens.md`.
7. ⛔ **Le service worker** — destinataire nommé : le chantier de retrait du
   legacy, verrou **C5**. **P0-d a établi qu'il n'est PAS une dépendance de
   l'installabilité.**
8. ⛔ **`hub.html` n'est pas dans `SURFACES_PRODUIT` de §7.9 ② A** (D6).
9. ⛔ **Le lancement depuis une PWA (`?app=`) est livré et exercé par aucun
   critère.**
10. ⛔ **Aucun jugement visuel**, et cinq de plus attendent un œil.
