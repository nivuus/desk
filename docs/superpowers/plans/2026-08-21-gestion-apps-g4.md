# Sous-bloc G4 — la surveillance, qui n'est qu'une accélération : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal :** qu'un raccourci créé dans la VM apparaisse au catalogue **en moins de
cinq secondes** au lieu de trente, **sans qu'aucun chemin de perte ne s'ouvre**
— et que le sous-bloc dise, mesure à l'appui, **ce qui achète réellement cette
absence de perte**.

🔴 **G4 N'AJOUTE AUCUNE FONCTIONNALITÉ. C'EST UNE ACCÉLÉRATION.** La
réconciliation périodique existe depuis G1, elle fonctionne, et **elle n'est
jamais désarmée** (spec D1). Tout ce que G4 pose est un **déclencheur**. La
propriété qui compte n'est donc pas « la surveillance marche » — elle marchera —
mais **« la surveillance ne peut RIEN perdre »**, et c'est la seule chose que ce
plan cherche à rendre falsifiable.

**Architecture :** quatre étages, et la coupure pur / sans-`cfg` /
`#[cfg(windows)]` est décidée **ici**, pas à l'implémentation.

1. **Un fil dédié tient les quatre racines** par `ReadDirectoryChangesW`
   recouvert, et **ne lit JAMAIS le contenu de son tampon** (D3). Il n'incrémente
   que deux compteurs monotones : *quelque chose a bougé*, et *quelque chose a
   été perdu*.
2. **L'anti-rebond est PUR**, horloge en paramètre, testé sur l'hôte (D5). Sa
   borne haute n'est **pas recopiée de la spec** : elle est **dérivée du critère
   ① et de deux coûts mesurés** (D6).
3. **La boucle de découverte sonde ces compteurs** dans l'attente qu'elle a déjà,
   à côté du drapeau que G3 y a posé. Elle ne gagne **aucun** fil.
4. **Rien ne traverse le fil.** G4 ne touche ni `proto/`, ni `plateforme/`, ni
   `client/`, et **ne monte PAS `PLATEFORME_VERSION`** (D1). Ses quatre critères
   se jugent tous dans le **journal de l'agent**.

**Tech Stack :** inchangée. 🔴 **G4 N'AJOUTE AUCUNE DÉPENDANCE ET AUCUNE
FONCTIONNALITÉ DE CRATE** — relevé **dans les bindings**, pas supposé (M4). Le
témoin de l'invariant côté plateforme (`plateforme/src/base/pilote.test.ts`,
`expect(deps).toEqual(['pg','ws'])`) n'est même pas concerné : G4 ne touche pas
`plateforme/`.

**Spec :** `docs/superpowers/specs/2026-08-19-gestion-apps-design.md`, §4 (D1),
§5 « G4 », §6 (l'arborescence), §7, §9.
**Amont qui fait autorité sur l'état du code :** les sections **G1**, **G2** et
**G3** de `CLAUDE.md`, les trois documents de résultats correspondants, les plans
de G2 et de G3 — **et le code lui-même**, qui a bougé depuis la clôture de G3.

**Ce plan ne couvre QUE G4.** Rien du manifeste PWA par application, des
`file_handlers`, ni des types installeur du hub (G5). Rien des icônes (G2) ni du
téléversement (G3), dont G4 se contente de ne rien casser.

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **21 août 2026**, sur cette
machine, en lançant réellement la commande. **Aucune n'est recopiée d'un
document.** Elles **DÉRIVERONT** : la seule source de vérité est la commande.

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>380'
```

| Fichier | Lignes | Ce que G4 en fait |
| --- | --- | --- |
| `agent/src/encode.rs` | **1536** | dette gelée, non touchée |
| `agent/src/windows_source.rs` | **630** | dette gelée, non touchée |
| `plateforme/src/http/routes-installation.ts` | **500** (marge **0**) | **non touché** — c'est la marge nulle que G3 lègue |
| `agent/src/encode/arret.rs` | **500** (marge 0) | non touché |
| `client/verify-webrtc.mjs` | **494** | non touché |
| `agent/src/superviseur/lanceur.rs` | **488** | **LU, non modifié** |
| `agent/src/apps/installation/telechargement/tests.rs` | **473** | non touché |
| `proto/ts/plateforme.ts` | **468** | **non touché** — G4 ne monte pas la version |
| `proto/ts/plateforme.test.ts` | **462** | non touché |
| `plateforme/src/agents/canal.ts` | **463** | non touché |

Et, hors de ce seuil, **les fichiers que G4 touche réellement** :

| Fichier | Lignes | Ce que G4 en fait |
| --- | --- | --- |
| 🔴 `agent/src/apps/boucle.rs` | **385** | **EXTRACTION AVANT ADDITION** (D9) : le sondage, le déclencheur, le mode, et la correction de la course de `reconciliee` (E5) |
| `proto/src/plateforme.rs` | **407** | **non touché** |
| `agent/src/apps/lecture.rs` | **313** | **LU** (`racines()`), non modifié |
| `agent/src/main.rs` | **320** | **non touché** — `let _apps = apps::brancher(…)` ne bouge pas (engagement de périmètre de G3) |
| `agent/src/plateforme.rs` | **308** | **non touché** |
| `agent/src/apps.rs` | **242** | +`pub mod surveillance;`, +le fil dans `Poignees` |
| `agent/src/apps/raccourci.rs` | **170** | non touché |
| `scripts/run-agent.sh` | **160** | 🔴 **TÂCHE DÉDIÉE** (D8) — deux lignes |
| `agent/src/apps/reconciliation.rs` | **83** | non touché |
| `agent/src/apps/installation/partage.rs` | **47** | **LU**, non modifié — G4 pose un SECOND état partagé, il ne touche pas celui-ci |

**Comptes de tests de départ, à opposer aux comptes d'arrivée** (relevés le
21 août 2026, sur l'arbre partagé — ⚠️ **un compte n'est attribuable qu'assorti
de son arbre**, leçon de D11 et de G2) :

| Commande | Départ |
| --- | --- |
| `cargo test -p agent` | **916 passed, 0 failed** (doc-tests : 0) |
| `cargo test -p proto` | **109 passed, 0 failed** (doc-tests : 0) |
| `cd proto && npx vitest run` | **296 passed, 9 fichiers** |
| `cd plateforme && npm run test:sqlite` | **564 passed, 58 fichiers** |
| `cd agent && cargo check --target x86_64-pc-windows-gnu` | **sortie 0, 22 avertissements**, **tous** de la famille `dead_code` (« never used » / « never read ») — vérifié par filtrage, pas supposé |
| `grep -c '^ *etape "' scripts/verify-all.sh` | **10** |

⚠️ **Dire lequel on annonce** — le piège des « dix » et « dix-sept » de P3, et le
« dix-huit » de S4. Le compte retenu par ce plan est celui des appels `etape`
dans le script : **DIX**. Le nombre d'en-têtes `==>` affichés à l'exécution est
**plus grand**, il dérive avec `design:verifier`, et **il n'est pas prédit ici**.

**Autres relevés par la commande, le même jour :**

- `proto/src/plateforme.rs:122` — `pub const PLATEFORME_VERSION: u8 = 4;`.
  **G4 ne le touche pas** (D1).
- `agent/src/apps.rs:36` — `PERIODE_RECONCILIATION: Duration = Duration::from_secs(30)`.
- `agent/src/plateforme.rs:59` — `PERIODE_BATTEMENT: Duration = Duration::from_secs(30)`.
  ⚠️ **Les deux valent trente secondes PAR COÏNCIDENCE, pas par dérivation** :
  ce sont deux constantes indépendantes, et elles se recalibreraient séparément.
  C'est cette coïncidence qui produit le fait mesuré n°2 ci-dessous.
- `agent/src/apps/boucle.rs:382` — `std::thread::sleep(reste.min(Duration::from_millis(200)))` :
  **la granularité du sondage est de 200 ms**, et c'est elle qui borne par le bas
  la résolution de tout anti-rebond (D6).
- `agent/src/apps/boucle.rs:325-327` — le drapeau `partage.reconcilier` est
  **abaissé APRÈS** la réconciliation. 🔴 Voir E5 : **le commentaire posé juste
  au-dessus nomme exactement le défaut que ce code produit.**
- `agent/src/apps/lecture.rs:197-219` — `racines()` rend les quatre dossiers
  connus, **sans déduplication** et **sans garantie de cardinal** : une racine
  absente est sautée avec un `warn!`.
- `agent/src/apps/lecture.rs:221+` — `lnk_sous` ne retient qu'une extension,
  `.lnk`, insensible à la casse. **C'est ce qui rend la rafale du critère ②
  inoffensive pour le corpus** (D10).
- `agent/Cargo.toml` — `windows = "0.62"` porte déjà
  `Win32_Storage_FileSystem`, `Win32_System_IO`, `Win32_System_Threading`,
  `Win32_Security`, `Win32_Foundation`. **Aucune fonctionnalité à ajouter** (M4).
- `grep -rn "catalogue reconcilie" --include='*.mjs' --include='*.js'
  --include='*.ts' --include='*.sh' --include='*.ps1'` sur tout le dépôt hors
  `node_modules` rend **AUCUNE ligne** : ✅ **aucun instrument versé ne lit
  cette trace**, et lui ajouter un champ ne casse rien. *Vérifié, à ne pas
  re-vérifier.*
- ⚠️ **`CLAUDE.md` publie QUATRE lignes de dette et le dépôt n'en a que DEUX.**
  `proto/src/plateforme/tests.rs` y figure à **561** et vaut **340** ;
  `proto/ts/plateforme.test.ts` y figure à **512** et vaut **462**. Les deux ont
  été résorbées par G2 (`727e6e5`), et la table de tête n'a pas suivi.
  **CONSIGNÉ, NON CORRIGÉ** : corriger l'index n'est pas le livrable d'un plan.

### 🔴 Les mesures prises POUR ce plan, transcrites VERBATIM

⚠️ **AUCUNE MESURE DE CE PLAN N'A PRIS LA VM.** Elles sont toutes des **lectures
en seul accès** — un montage CIFS déjà présent, et le journal d'agent qu'un
chantier voisin était en train d'écrire. **Aucun `build-agent.sh`, aucun
`run-agent.sh`, aucun WinRM.**

#### M1 — Le corpus vaut **220 raccourcis**, et non 218

```
$ for d in .../Administrateur/Desktop .../Public/Desktop \
           .../Administrateur/AppData/Roaming/Microsoft/Windows/Start\ Menu \
           /media/vm/ProgramData/Microsoft/Windows/Start\ Menu ; do
    find "$d" -iname '*.lnk' | wc -l ; done
9
8
26
177
```

**9 + 8 + 26 + 177 = 220.** ✅ C'est **218 + les deux témoins de G2**,
`G2 Temoin 48.lnk` et `G2 Temoin 256.lnk`, relevés présents sur le Bureau
(`ls -la`, horodatés du 21 août 00:12). **Ne pas les supprimer** : ils rendent le
critère ② de G2 rejouable, et **ils portent le corpus de 154 à 156 clés**.

#### M2 — 🔴 Le catalogue part **COMPLET à chaque période**, et la trace qui le dit MENT sur son nom

Relevé sur `/media/vm/dev/agent.log`, mis à plat, au moment de la rédaction —
c'est-à-dire **le journal d'un agent que le chantier voisin faisait tourner** :

```
$ grep -ac 'catalogue reconcilie' agent-plat.log      →  12
$ grep -ac 'reenrolement observe' agent-plat.log      →  11
```

```
2026-08-21T06:18:55.722897Z  INFO agent::apps::boucle: catalogue reconcilie total=220 retenus=169 cles=156 icones=156 icones_echouees=0 icones_distinctes=98 apparues=156 modifiees=0 disparues=0 duree_ms=1576
2026-08-21T06:21:54.156536Z  INFO agent::apps::boucle: reenrolement observe : le prochain catalogue sera COMPLET
2026-08-21T06:21:56.147112Z  INFO agent::apps::boucle: catalogue reconcilie total=220 retenus=169 cles=156 icones=0 icones_echouees=0 icones_distinctes=98 apparues=0 modifiees=0 disparues=0 duree_ms=68
2026-08-21T06:22:24.229741Z  INFO agent::apps::boucle: reenrolement observe : le prochain catalogue sera COMPLET
2026-08-21T06:22:26.213197Z  INFO agent::apps::boucle: catalogue reconcilie total=220 retenus=169 cles=156 icones=0 icones_echouees=0 icones_distinctes=98 apparues=0 modifiees=0 disparues=0 duree_ms=66
```

**Onze `reenrolement observe` pour douze réconciliations** : le drapeau `complet`
est levé à **chaque** tour sauf le premier, qui l'est déjà par construction.
**Aucun réenrôlement n'a eu lieu** — c'est le **rafraîchissement de jeton** du
battement (`plateforme/session.rs:123`, `BattementRecu` → `tx.send(Some(Identite
{…}))`) qui fait bouger la `watch`, et `PERIODE_BATTEMENT` vaut exactement
`PERIODE_RECONCILIATION`.

**Trois conséquences, et elles gouvernent trois décisions de ce plan :**

1. **La trace ment sur son nom** — troisième occurrence dans ④ après `retenus`
   (leg n°7 de G1) et `icones_echouees` (défaut ② de G2). **G4 la corrige** (D7).
2. **Le catalogue complet part sur le fil toutes les trente secondes**, pour
   toujours. G1 a mesuré un `Catalogue` complet à **56 145 octets** de charge
   utile TCP — pour **154** applications et **sans** les champs `icone` et
   `source_max` que G2 a ajoutés. **Celui d'aujourd'hui est donc PLUS GROS, et je
   ne l'ai PAS mesuré.** *Legué, non corrigé* (D7).
3. 🔴 **Le critère ③ ne peut donc PAS se juger sur la plateforme** : elle reçoit
   un catalogue complet toutes les trente secondes quoi qu'il arrive, et
   « le catalogue est complet » y serait vrai **par un mécanisme étranger à
   D1**. **Il se juge sur le champ `cles=` du journal de l'agent** (D11).

#### M3 — La réconciliation coûte **66 ms au repos**, et **1 576 ms au premier tour**

Même journal. Douze valeurs de `duree_ms` : **1576** au premier tour (celui qui
extrait les 156 icônes), puis **65, 66, 66, 66, 68, 68, 72, 72, 78, 79, 84, 85,
86, 96** — la médiane des tours suivants est **≈ 70 ms**, et le mode **66 ms**.

⚠️ **Le premier tour coûte 24 fois plus que les suivants, et l'écart est
entièrement l'extraction d'icônes** : `icones=156` contre `icones=0`, soit
**≈ 10 ms par icône neuve**. C'est ce chiffre qui borne le pire cas du critère ①
(D6) : une réconciliation déclenchée par l'apparition de *k* applications neuves
coûte `70 ms + k × 10 ms`.

⚠️ **Ces valeurs sont celles d'un agent qui partageait sa VM avec la recette d'un
autre chantier.** Elles sont un **ordre de grandeur**, pas une caractérisation.

#### M4 — Les cinq appels Win32 dont G4 a besoin sont **DÉJÀ** derrière des fonctionnalités activées

Relevé **dans les bindings de `windows-0.62.2`**, fichier par fichier, pas
supposé :

| Appel / constante | Fichier des bindings | Fonctionnalité qui le garde | Déjà dans `Cargo.toml` ? |
| --- | --- | --- | --- |
| `ReadDirectoryChangesW` | `Win32/Storage/FileSystem/mod.rs:2103` | `Win32_System_IO` (le `#[cfg]` posé l. 2100) | ✅ |
| `FILE_NOTIFY_CHANGE_{FILE_NAME,DIR_NAME,LAST_WRITE}` | `Win32/Storage/FileSystem/mod.rs:4420,4419,4417` | **aucune** (le module lui-même est gaté par `Win32_Storage_FileSystem`) | ✅ |
| `GetOverlappedResult`, `CancelIoEx` | `Win32/System/IO/mod.rs:33, 12` | **aucune** (module gaté par `Win32_System_IO`) | ✅ |
| `CreateEventW` | `Win32/System/Threading/mod.rs:251` | `Win32_Security` | ✅ |
| `WaitForMultipleObjects` | `Win32/System/Threading/mod.rs:1919` | **aucune** (module gaté par `Win32_System_Threading`) | ✅ |
| `ERROR_NOTIFY_ENUM_DIR` (1022), `ERROR_OPERATION_ABORTED` (995), `WAIT_OBJECT_0`, `WAIT_TIMEOUT`, `WAIT_FAILED` | `Win32/Foundation/mod.rs:3452, 3632, 10486, 10487, 10484` | **aucune** | ✅ |

⚠️ **CECI RESTE UNE HYPOTHÈSE JUSQU'À `cargo check --target
x86_64-pc-windows-gnu`, et ce dépôt a payé cette leçon deux fois** : G1 avait
annoncé `Win32_UI_Shell` seule pour `ShellExecuteExW` — **réfuté par la
compilation** —, G2 avait annoncé `Win32_Graphics_Imaging` seule — **confirmé**.
La différence entre les deux est qu'on ne le sait qu'après. **La tâche qui écrit
`fil.rs` doit compiler avant de déclarer quoi que ce soit.**

### La règle des 500 lignes : UNE extraction, AVANT son addition, et deux portes armées

**Porte de ce sous-bloc : 450 lignes.** Tout fichier qu'une addition porterait
au-delà déclenche une **extraction**, et **l'extraction précède l'addition** —
la forme forte que D9 (tâche 6) a inventée et que D10 a jouée trois fois, jamais
la forme « on franchit puis on rattrape », que ce dépôt a payée cinq fois.

| Fichier | Départ | Ce qui l'y porte | Décision |
| --- | --- | --- | --- |
| 🔴 `agent/src/apps/boucle.rs` | **385** | le sondage, le déclencheur, le mode, la correction E5, **et leur documentation** | **EXTRACTION AVANT ADDITION**, tâche 4. Point de chute nommé : `Memoire`, `reconcilier` et `mesurer` (l. 18-257) partent **VERBATIM** vers `agent/src/apps/boucle/memoire.rs`, déclaré par un `mod` ordinaire à l'intérieur de `boucle.rs` — pas de `#[path]`, les deux étant `#[cfg(windows)]` |
| `agent/src/apps.rs` | **242** | `pub mod surveillance;`, le fil dans `Poignees`, son câblage | budget ≤ 330. **Porte armée à 450** ; si elle se déclenche, le point de chute est `demarrer_surveillance` → `apps/surveillance.rs` lui-même |
| `agent/src/apps/surveillance/fil.rs` | **NEUF** | la boucle d'attente, les quatre racines, l'arrêt | budget ≤ 380. **Porte armée à 450** ; point de chute : la construction et la réouverture d'une racine sont **déjà** dans `racine.rs`, et c'est l'ouverture des *quatre* qui en sortirait, vers `surveillance/racines.rs` |
| `scripts/run-agent.sh` | **160** | deux lignes | aucune porte |

⚠️ **Les cinq franchissements récents de ce dépôt ont tous été vus par un
VOISIN, jamais par le chantier qui les commettait** (G2 : trois plafonds vus par
la clôture de E2). **La tâche de clôture relance la commande APRÈS la dernière
édition de la ronde, revue transverse comprise** — une table mesurée en début de
ronde est fausse à la fin de la même ronde, erreur de D8.

### Les règles de méthode, héritées et non négociables

- **DEUX exécutions par critère, jamais une, et AUCUN TAUX n'est revendiqué.**
  Deux exécutions établissent la reproductibilité ; elles ne mesurent pas une
  fréquence. La question « combien de fois sur combien » **ne se pose pas ici**
  et **ne s'emprunte pas** à une campagne qui, elle, la posait.
- **Chaque garde doit être VU ROUGE.** Un contrôle qu'on n'a jamais vu rouge
  n'est pas un contrôle. Et **une rouge doit rougir POUR SA RAISON** : S3 a dû
  refaire quatre rouges sur seize qui rougissaient sur une autre clause.
- 🔴 **LE HARNAIS DE ROUGE, ET IL EST OBLIGATOIRE.** Un chantier récent a
  découvert que son contrôle « la mutation a-t-elle changé quelque chose ? »
  était **vacueux**, `git diff` étant non vide quoi qu'on mute — l'arbre porte
  du travail d'autrui. Donc, pour **chaque** rouge :
  1. `cp <fichier> /tmp/g4-rouge-<n>.avant` **et** `sha256sum` ;
  2. la mutation, **par numéro de ligne ou par un motif ancré sur la syntaxe** —
     ⚠️ **jamais par une sous-chaîne nue** : dans un dépôt qui commente ses
     invariants, la première occurrence d'une chaîne est **dans le commentaire
     qui la justifie**, et une rouge y est restée VERTE (P4) ; et **une ligne à
     muter peut exister deux fois**, où un `replace(…, 1)` a frappé la mauvaise ;
  3. `diff /tmp/g4-rouge-<n>.avant <fichier>` — **une sortie vide est un ÉCHEC de
     la rouge**, jamais un succès du produit ;
  4. le contrôle, et **lire QUELLE assertion tombe**, jamais le seul code de
     sortie ;
  5. `cp /tmp/g4-rouge-<n>.avant <fichier>` — 🔴 **restaurer depuis la COPIE, et
     jamais par `git checkout --`**, qui restaure HEAD et a effacé du travail non
     commité deux fois dans ce dépôt ;
  6. `sha256sum` identique, et `git status --porcelain <fichier>` vide.
- **Un ZÉRO se qualifie avant de se rapporter.** Un zéro rendu par une trace
  qu'on n'a pas allumée n'est pas une mesure (G2). **Vérifier que la trace
  cherchée PEUT sortir, et avec quel `RUST_LOG`, avant de lire son compte.**
- **`grep -a` toujours** sur un journal de pilote : sans lui, un fichier à
  octets NUL est classé « binaire » et `grep` rend une **sortie vide**,
  indiscernable d'un zéro (D10).
- **Compter les APPLICATIONS, jamais les raccourcis.** Le corpus est de **220
  raccourcis pour 156 clés** : le rapport n'est pas de 1, et il ne l'a jamais été.
- **`git add` nominatif**, `git commit -F <fichier> -- <chemins>`, puis
  `git show --name-only`. **Jamais `git add -A`, jamais `--amend`.**
- **`unset -f chpwd`** avant tout relevé : le hook du shell de l'hôte injecte un
  `ls` dans toute sortie dès qu'un `cd` court dans un sous-shell (S2 a dû
  reprendre une passe entière de journaux).
- 🔴 **L'hôte porte un défaut non identifié** : `/dev/null` y a été trouvé
  remplacé par un fichier ordinaire, ce qui empêchait toute VM de démarrer.
  Réparé, **cause inconnue, peut se reproduire**. Contrôle en une ligne, à jouer
  avant toute séquence qui dépend de la VM : `stat -c '%F' /dev/null` doit rendre
  `character special file`. *Relevé conforme au moment d'écrire ce plan.*
- **Une différence entre deux commits n'est pas une attribution.** Mesurer au
  parent de son propre premier commit ne dit que ce qu'on a trouvé en arrivant :
  un voisin peut avoir écrit entre les deux. G3 a attribué à lui-même une purge
  de dette qui était de G2, et `git merge-base --is-ancestor` le tranche en une
  ligne. **Toute attribution passe par `git log --numstat -- <chemin>`.**

### 🔴 Périmètre concurrent — et la VM est un préalable EXTERNE, pas une dépendance de tâche

**Deux chantiers travaillent dans le même arbre au moment où ce plan est écrit**,
relevé par `git log --since` et par `virsh list --all` :

- **le presse-papier P3**, qui **clôt** (commit `a74e6d1`, 08:24, « les QUATRE
  critères TENUS ») et qui tient `agent/src/presse_papier*`,
  `agent/src/capteur/sommeil/`, `proto/src/control.*`, `client/src/presse-papier*` ;
- **le pont fichiers F4**, dont le **plan vient d'être commité** (`6608005`,
  08:18) et qui revendique `agent/src/pont/` — *et lui seul dans `agent/`* —,
  `proto/src/fichiers*`, `client/src/fichiers/`, **et une ligne de
  `scripts/run-agent.sh`**.

**Ce qui est à G4** : `agent/src/apps/` (et lui seul dans `agent/`),
`scripts/run-agent.sh` (**deux lignes**), et
`docs/superpowers/plans/journaux-gestion-apps-g4/`.

**Ce qui n'est à personne de G4** : `proto/` (ni `plateforme.*`, ni `control.*`,
ni `fichiers.*`), `plateforme/`, `client/`, `agent/src/main.rs`,
`agent/src/capteur/`, `agent/src/superviseur/`, `agent/src/pont/`, `src/`,
`web/` — `CLAUDE.md` excepté à la tâche de clôture.

⚠️ **`scripts/run-agent.sh` est réclamé par F4 ET par G4.** Les deux additions
sont des lignes distinctes et ne se contredisent pas, mais **le fichier est
partagé** : la tâche dédiée de G4 (D8) le relit avant d'écrire, et **ne
réordonne rien**.

🔴 **LA VM EST UN PRÉALABLE EXTERNE, ET C'EST LA FORMULATION QUI COMPTE.** Elle
n'est **pas** une dépendance de tâche que l'on attend : c'est une ressource
**exclusive** dont l'indisponibilité **arrête proprement** le sous-bloc au lieu
d'écraser le binaire d'un voisin. Trois raisons concrètes :

- `scripts/build-agent.sh` **rsynchronise l'arbre ENTIER**, donc le travail non
  commité et non compilant d'un chantier voisin (G2 a dû bâtir depuis un
  `git worktree`, **avec `node_modules` lié**, sans quoi le script s'arrête en
  silence après « sources synchronisées ») ;
- `scripts/run-agent.sh` **tue et relance** l'agent de qui que ce soit ;
- l'écriture de `C:\dev\agent.log` est **exclusive** : un superviseur laissé
  vivant fait lire à la tentative suivante **le journal de la précédente** (piège
  payé trois fois sur trois en D8).

**Conduite prescrite, dans cet ordre :**

1. `stat -c '%F' /dev/null`, puis `virsh list --all` — **ne pas croire un
   rapport qui dit la VM démarrée** (G2 en a trouvé une éteinte alors qu'un
   rapport la disait allumée depuis 1 j 11 h) ;
2. relever si un agent tourne : `ls -la /media/vm/dev/agent.log` — un `mtime` de
   moins d'une minute **est** un agent vivant ;
3. **si la VM est prise, G4 s'arrête et le DIT.** Les familles 0 à 3 de ce plan
   (les modules purs, l'extraction, la surveillance, le câblage) **ne demandent
   pas la VM** et se jouent entièrement sur l'hôte, `cargo check --target
   x86_64-pc-windows-gnu` compris. **Seules la sonde S1 et la recette
   l'exigent.**

---

## 🔴 LA PORTE — une rafale peut-elle faire déborder un tampon, et à quel régime ?

Le critère ② de la spec porte **sa propre condition de non-mesurabilité, et elle
est écrite** : *« si la rafale ne provoque aucun débordement, le critère est NON
MESURABLE, et il doit le dire. Un critère qu'on n'a pas vu se déclencher n'est
pas un critère. »*

**Cette porte se joue AVANT toute ligne de produit**, et **sans une ligne de
produit** : elle emploie le `FileSystemWatcher` de .NET, qui est
`ReadDirectoryChangesW` sous le capot et dont la propriété
`InternalBufferSize` est passée telle quelle à l'appel. Un débordement y lève
`InternalBufferOverflowException` sur l'événement `Error`.

### Pourquoi la question est réelle, et pas rhétorique

Le tampon retenu (D2) vaut **65 536 octets**. Un enregistrement
`FILE_NOTIFY_INFORMATION` pèse 12 octets plus le nom en UTF-16, aligné sur 4 :
pour un nom de 19 caractères, **52 octets**, soit **≈ 1 260 enregistrements** par
tampon. **Le tampon ne déborde que si plus de 1 260 événements arrivent entre
deux réarmements** — c'est-à-dire dans les microsecondes qui séparent une
complétion du `ReadDirectoryChangesW` suivant.

🔴 **Une boucle PowerShell qui crée cinq mille fichiers un par un ne débordera
JAMAIS.** À ~166 créations par seconde, le lecteur draine chaque événement en
microsecondes. **Il faut un régime parallèle** — `robocopy /MT:32` depuis une
préparation hors des racines surveillées — et **rien ne garantit qu'il suffise.**

### La sonde S1 — ce qu'elle relève, et pourquoi chaque ligne compte

Un `.ps1` **écrit en UTF-8 avec BOM ou en pur ASCII** (🔴 voir le premier piège
de G3 : un `.ps1` sans BOM contenant un seul caractère non-ASCII ne s'analyse
pas, **et l'erreur désigne une autre ligne que la vraie cause** ; contrôle en une
ligne, `LC_ALL=C grep -c '[^ -~]' fichier.ps1` doit rendre **0**), lancé par
**tâche planifiée `/it`** — un relevé WinRM est celui de la session 0, jamais de
la session interactive.

| # | Ce que la sonde relève | Pourquoi |
| --- | --- | --- |
| a | l'état du corpus AVANT : `(Get-ChildItem -Recurse -Filter *.lnk).Count` sur les quatre racines | **220 attendu.** Sans ce chiffre, aucun contrôle de sortie ne peut échouer |
| b | la création de `…\Start Menu\Programs\g4-rafale\` et de *N* fichiers `.tmp` par `robocopy /MT:32` depuis `C:\dev\g4-preparation\` | le régime le plus agressif que la machine permette |
| c | le nombre d'`Error` / `InternalBufferOverflowException` levés, et le nombre de `Created` reçus | **c'est le verdict** |
| d | la durée réelle de la rafale, et le débit en événements par seconde | dimensionne la rafale de la recette, et **borne le pire cas du critère ①** |
| e | 🔴 le corpus APRÈS le nettoyage : `Remove-Item -Recurse -Force` puis re-comptage | **220 exigé.** C'est la seule preuve que l'instrument ne détruit pas ce qu'il mesure |
| f | l'état de la racine si le nettoyage ÉCHOUE : le répertoire `g4-rafale` est-il encore là, et avec combien de fichiers | **écrit d'avance** parce qu'un nettoyage à mi-course est le cas qu'on ne veut pas découvrir |

### Ce que chaque verdict COMMANDE — écrit d'avance, pour que personne n'arbitre sous le coup du résultat

| Verdict de S1 | Ce qu'il commande |
| --- | --- |
| 🟢 **DÉBORDE**, à *N* fichiers et *D* événements/s | le critère ② est **mesurable**. La recette rejoue la même rafale sur le produit, et ② se juge sur la ligne `notifications perdues`. ⚠️ **Cela ne PROUVE pas que NOTRE surveillance débordera** — voir la réserve ci-dessous |
| 🔴 **NE DÉBORDE PAS**, au régime maximal atteignable | **le critère ② sera NON MESURABLE, et on le sait d'avance.** ⚠️ **Il est INTERDIT de rétrécir `TAMPON_NOTIFICATIONS` pour faire passer ②** : ce serait régler le produit sur son test. La rafale est **jouée quand même** sur le produit (elle est gratuite), et son verdict est écrit tel quel. **L'injection `APPS_FAUTE=debordement` devient alors le SEUL moyen d'exercer le chemin de code**, et le §« ce que l'injection n'établit pas » ci-dessous s'applique mot pour mot |
| ⚪ **NON MESURABLE** — .NET absent, droits refusés, tâche planifiée qui n'aboutit pas | **aucune conclusion.** La sonde journalise pourquoi, et le sous-bloc **se déroule tel quel** : S1 dimensionne la rafale, elle ne conditionne aucun livrable |
| 🔴 **LE NETTOYAGE ÉCHOUE** (relevé e ≠ 220) | 🔴 **G4 NE JOUE PAS LA RAFALE SUR LE PRODUIT**, et le critère ② est NON MESURABLE par décision. Le corpus de cette VM est un actif partagé par ④ tout entier : **le mettre en risque pour un critère coûte plus que le critère** |
| 🔴 **LA VM EST PRISE** | G4 s'arrête à la fin de la famille 3, et le dit. **Aucune famille de produit n'en dépend.** |

🔴 **LA RÉSERVE QUI SURVIT À TOUS LES VERDICTS, ET ELLE EST DE MOI :** le
`FileSystemWatcher` de .NET a **sa propre file gérée** et **son propre fil de
distribution** entre `ReadDirectoryChangesW` et l'appelant. Notre surveillance
réarme dans une boucle serrée, sans marshalling. **Elle peut déborder plus
facilement, ou moins.** S1 **borne la question, elle ne la répond pas pour notre
code** — et la seule mesure qui la réponde est la rafale rejouée sur le produit.

---

## Décisions tranchées

### D1 — 🔴 G4 NE TOUCHE NI `proto/`, NI `plateforme/`, NI `client/`, et NE MONTE PAS `PLATEFORME_VERSION`

**C'est la décision la plus structurante du sous-bloc, et elle se dérive d'un
relevé** : les quatre critères de la spec se jugent tous sur des grandeurs qui
existent déjà — un horodatage de ligne de journal (①), une ligne `warn!` (②), le
champ `cles=` d'une ligne existante (③), le nombre de lignes `catalogue
reconcilie` (④). **Aucun n'a besoin d'un message neuf.**

**Ce que cela achète, et c'est considérable :**

- **le binaire déployé et la plateforme restent compatibles.** G1 a payé ce
  point : un agent v1 face à une plateforme v2 **boucle sans terme** et **ne peut
  même pas LIRE le refus**, `verifie_version` étant posé sur le champ `v` de
  *tout* message, refus compris ;
- **la surface de collision avec les chantiers voisins tombe à `agent/src/apps/`
  et deux lignes de script** ;
- **aucune migration, aucun vecteur partagé, aucun miroir TypeScript.**

⚠️ **Corollaire, et il faut le dire** : `PLATEFORME_VERSION` reste à **4**, et
**G4 ne consomme pas de numéro**. Le prochain chantier qui en aura besoin le
prend. **Deux chantiers ne peuvent pas monter la même version** — G1 l'a mesuré
en trouvant une VM muette pendant toute une tâche.

### D2 — `ReadDirectoryChangesW` RECOUVERT, un fil, quatre événements, et un tampon de 64 Kio

**Retenu** : un **seul** fil dédié, quatre handles ouverts en
`FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED`, quatre `OVERLAPPED` portant
chacun son **événement à réarmement manuel**, et un `WaitForMultipleObjects` sur
les quatre plus un délai.

**Écarté, avec sa raison** :

- **quatre fils bloquants** (`ReadDirectoryChangesW` synchrone) : quatre fils
  pour attendre, et **aucun moyen d'arrêter proprement** — un fil bloqué dans un
  appel synchrone ne voit pas un drapeau d'arrêt ;
- **un port de complétion** : plus de machinerie que quatre handles n'en
  justifient, et une seconde façon d'attendre dans un processus qui en a déjà
  quatre ;
- **`SHChangeNotifyRegister`** : écarté par la spec D1 elle-même, et pour de
  bonnes raisons (un `HWND`, une pompe de messages, des PIDL à re-résoudre).

**`TAMPON_NOTIFICATIONS = 65 536` octets par racine**, soit **256 Kio** de pool
non paginé pour les quatre. ⚠️ **Choisi SUR SON MÉRITE** : c'est le plafond que la
documentation impose pour un chemin réseau et celui qu'elle recommande de ne pas
dépasser, le tampon étant verrouillé en mémoire. 🔴 **Il n'est PAS choisi pour
rendre le critère ② mesurable, et il ne doit jamais l'être** — voir la porte.
**NON CALIBRÉ.**

**Filtres** : `FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME |
FILE_NOTIFY_CHANGE_LAST_WRITE`, `bWatchSubtree = TRUE` — la spec D1, sans
changement.

### D3 — 🔴 LE CONTENU DU TAMPON N'EST JAMAIS LU, ET C'EST LA SIMPLIFICATION CENTRALE

La spec D1 le dit sans le nommer : *« la notification ne fait qu'AVANCER la
prochaine réconciliation »*. Une réconciliation **relit le disque entier**. Il
n'y a donc **rien** à tirer du nom du fichier qui a bougé.

**Décision : la surveillance n'analyse jamais un `FILE_NOTIFY_INFORMATION`.**
Elle n'incrémente que deux compteurs. Ce que cela retire du produit :

- aucune chaîne UTF-16 à décoder, aucune chaîne de `NextEntryOffset` à suivre,
  **aucun aliasing de tampon** — trois familles de défaut qui n'existeront pas ;
- **aucune tentation de filtrer sur `.lnk`**, laquelle serait de toute façon
  **impossible à tenir** : un débordement **jette le tampon entier**, donc le
  chemin « je ne sais pas ce qui a changé » doit exister quoi qu'il arrive.
  Écrire un filtre qui ne couvre pas ce cas serait écrire deux chemins pour en
  servir un.

⚠️ **Le coût, nommé** : n'importe quelle écriture dans les quatre arborescences
déclenche une réconciliation, y compris un fichier temporaire qui n'a rien à voir
avec un raccourci. **C'est exactement ce que l'anti-rebond borne**, et c'est
aussi ce qui rend le critère ④ mesurable au lieu de théorique.

⚠️ **Conséquence qu'il faut MESURER et non supposer** : si l'une des quatre
racines est bruyante au repos — le Bureau d'un utilisateur l'est souvent —, la
surveillance ferait passer la réconciliation de **une par trente secondes** à
**une par `DELAI_ANTI_REBOND_MAX`**, soit **7,5 fois plus**. D'où le **témoin de
repos** (relevé ⑥), qui **peut échouer**.

### D4 — 🔴 UNE SEULE VARIABLE, QUATRE ÉTATS, ET AUCUN N'EST UNE PRÉSENCE

`APPS_SURVEILLANCE`, **variable de BANC pour trois de ses quatre états**.

| Valeur | Effet | Sert |
| --- | --- | --- |
| **absente** | surveillance armée, anti-rebond armé, réconciliation périodique armée | **le produit livré** |
| `0` | **surveillance désarmée** — le comportement de G1, exactement | la ROUGE du critère ① |
| `sans-rebond` | surveillance armée, **anti-rebond neutralisé** : toute notification rompt l'attente | la ROUGE du critère ④ |
| `seule` | surveillance armée, **réconciliation périodique DÉSARMÉE** | la ROUGE du critère ③ |

**Pourquoi une seule variable et non trois** :

1. **le piège de `run-agent.sh` a été payé cinq fois** (`SUPERVISEUR` en D1,
   `MULTIFENETRE_REPRISE` en D2, `AUDIO` en D7…). Une ligne à transmettre au
   lieu de trois, c'est un tiers du risque ;
2. **les quatre états sont mutuellement exclusifs par construction.** Trois
   booléens autoriseraient « ni surveillance ni réconciliation périodique »,
   c'est-à-dire un agent qui ne réconcilie jamais — un état qui n'a aucun sens et
   qu'aucune recette ne veut ;
3. `agent/src/apps.rs` l'écrit déjà de lui-même : *« une variable de plus qui ne
   servirait à personne est une variable qu'on oubliera de transmettre »*.

🔴 **Chaque état est une ÉGALITÉ EXACTE, jamais un `is_ok()` ni un `is_some()`.**
`0` réemploie `apps::desarme`, **réutilisé et non recopié** — G2 a posé ce
précédent pour `ICONES`. La convention de `PLEIN_ECRAN`, `AUDIO`, `APPS`,
`ICONES`, `PART_SONDAGE` et `PRESSE_PAPIER` est donc intégralement préservée :
**`=0` désarme, une simple présence n'active pas.**

🔴 **UNE VALEUR INCONNUE EST UN `warn!` QUI LA NOMME, ET LE COMPORTEMENT LIVRÉ EST
RETENU.** Sans quoi une coquille dans une rouge (`seul` pour `seule`) ferait
tourner le comportement VERT sous le nom du ROUGE, et la recette lirait un
verdict faux — le patron « un contrôle qui ne peut pas échouer », sous une forme
neuve.

🔵 **ET LE REMÈDE NE S'ARRÊTE PAS AU `warn!` : un `info!` est émis À CHAQUE
DÉMARRAGE**, quel que soit le mode, `mode de surveillance retenu mode=…`. Aucune
exécution ne peut alors être mal attribuée. ⚠️ **La trace prouve que la variable a
atteint le processus ; elle ne prouve pas que le mécanisme est coupé** — leçon
de P1 sur `PRESSE_PAPIER=0`. **Ce qui discrimine est le COMPTE**, jamais la
trace.

### D5 — L'anti-rebond est PUR, et son horloge est un PARAMÈTRE

`agent/src/apps/surveillance/rebond.rs`, **aucun `cfg`, aucune horloge propre,
aucune entrée-sortie**. La spec §6 le range à `src/apps/rebond.rs` ; ce plan le
range sous `surveillance/` (divergence E1).

```
DELAI_ANTI_REBOND      = 750 ms   — toute notification repousse l'échéance
DELAI_ANTI_REBOND_MAX  = 4 s      — bornée à `première + MAX` (voir D6)
```

État : l'instant de la **première** notification du train courant, et celui de la
**dernière**. Échéance : `min(derniere + DELAI_ANTI_REBOND, premiere +
DELAI_ANTI_REBOND_MAX)`.

**Pourquoi la borne haute existe** : sans elle, un flux continu de notifications
ajourne la réconciliation **sans terme** — un installeur qui écrit pendant deux
minutes ne produirait aucun catalogue avant sa fin, et une racine bruyante en
produirait **jamais**. C'est le même raisonnement, et le même précédent, que
`REPLI_MAX_MS` dans `agent/src/plateforme/repli.rs` : *« ce plafond n'est pas un
confort, c'est le garde-fou du critère »*.

**Les tests d'hôte, et leurs rouges :**

| Test | Ce qu'il fixe | Sa ROUGE |
| --- | --- | --- |
| une notification isolée fait échoir à `+ DELAI_ANTI_REBOND` | le cas nominal | rendre l'échéance immédiate |
| une seconde notification à `+500 ms` **repousse** l'échéance | c'est *l'anti-rebond*, pas un simple délai | ne pas repousser : l'échéance resterait à la première |
| 🔴 un train de notifications toutes les 100 ms **échoit quand même** à `premiere + MAX` | **le test qui compte** : sans lui, la borne haute pourrait être absente et tous les autres passeraient | retirer le `min(…, premiere + MAX)` — l'échéance fuit indéfiniment |
| `consommer()` remet le train à zéro, et la notification suivante repart de `premiere` | sans quoi la borne haute mordrait pour toujours après le premier train | ne pas remettre `premiere` à `None` |
| aucune notification ⇒ **aucune échéance** | un anti-rebond qui échoit sans notification déclencherait des réconciliations fantômes | rendre `Some(maintenant)` au repos |

⚠️ **`Instant` ne se fabrique pas, il s'offsette.** Les tests prennent un
`Instant::now()` de base et lui ajoutent des `Duration` : **aucun test ne dort**.

### D6 — 🔴 `DELAI_ANTI_REBOND_MAX` vaut 4 s, et c'est DÉRIVÉ, pas recopié

La spec propose **5 s**. Ce plan retient **4 s**, et le calcul est écrit :

```
pire cas du critère ① = DELAI_ANTI_REBOND_MAX
                      + granularité du sondage   (200 ms, RELEVÉ dans boucle.rs:382)
                      + coût de la réconciliation (≈ 70 ms au repos, MESURÉ — M3)
                      + 10 ms par icône neuve      (MESURÉ — M3)
```

Avec **5 s** : `5 000 + 200 + 70 + 10 = 5 280 ms` — **au-dessus des cinq
secondes que le critère ① exige**, dans le cas où le raccourci naît *à
l'intérieur* d'un flux continu. Avec **4 s** : `4 000 + 200 + 70 + 10 = 4 280 ms`,
et il reste **720 ms** de marge, soit **soixante-douze icônes neuves** avant que
le critère ne soit menacé.

🔵 **Ce n'est pas une constante calibrée — c'est une constante DÉRIVÉE d'un
critère et de deux coûts mesurés.** Les deux ne sont pas la même chose, et la
différence est écrite : personne n'a jugé que 4 s « se sent bien ». **Elle
rejoint néanmoins la liste des non calibrées** — `BPP_MIN`, `FACTEUR_FOCUS`,
`PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`, `SEUIL_INJOIGNABLE_MS`,
`PERIODE_RECONCILIATION`, `DELAI_LANCEMENT_MS` — parce qu'aucun jugement d'usage
n'a été porté sur l'expérience qu'elle produit.

⚠️ **Le pire cas reste ouvert à un endroit** : si *k* applications apparaissent
d'un coup, le terme d'icônes vaut `k × 10 ms` et le critère ① tombe à
`k > 72`. **Nommé, non borné**, et c'est le prix de la voie « une seule
réconciliation pour tout un train ».

### D7 — La trace `reenrolement observe` est CORRIGÉE ; le COMPORTEMENT ne l'est PAS

**Le fait, mesuré (M2)** : `identite.has_changed()` rend vrai à **chaque**
battement, donc à chaque période, donc **le catalogue complet part toutes les
trente secondes** — pour toujours, sur un disque au repos.

**La trace ment sur son nom** : aucun réenrôlement n'a lieu. C'est la **troisième
occurrence** de ce défaut dans ④ — `retenus` (leg n°7 de G1, fermé par G2),
`icones_echouees` sous `ICONES=0` (défaut ② de G2), et celle-ci. **G4 la
corrige** : elle doit dire ce qu'elle observe, `identité changée : le prochain
catalogue sera COMPLET (rafraîchissement de jeton OU réenrôlement — cette boucle
ne les distingue pas)`.

🔴 **LE COMPORTEMENT N'EST PAS CHANGÉ, ET LA RAISON EST L'INVERSE DE CE QU'ON
CROIRAIT.** La boucle *pourrait* distinguer les deux — un réenrôlement change le
`prefixe`, un battement ne change que le `jeton` — et ne lever `complet` que sur
le premier. **Mais ce serait retirer une réparation réelle** : le commentaire de
`boucle.rs:298-302` promet qu'*« un `Catalogue` perdu pendant une coupure
laisserait sinon la plateforme divergente SANS TERME »*, et **c'est précisément
l'envoi complet périodique qui tient cette promesse**. Ce qui a l'air d'un défaut
est, en fait, **la seule implémentation de la garantie que son propre commentaire
annonce.**

**Ce que G4 fait donc** : corrige la trace, **inscrit dans le commentaire que
l'envoi complet est périodique et pourquoi c'est un filet**, et **lègue** :

⛔ **Legs** — arbitrer entre le filet et son coût demande de **mesurer la trame
sur le fil**, ce que G4 ne fait pas. Le seul chiffre disponible est celui de G1 :
**56 145 octets** pour **154** applications **sans** `icone` ni `source_max`.
Celui d'aujourd'hui porte **156** applications **avec** les deux champs : **il
est plus gros, et il n'est mesuré par rien.**

### D8 — Deux lignes dans `scripts/run-agent.sh`, par une TÂCHE DÉDIÉE qui ne fait que cela

```
${APPS_SURVEILLANCE:+\$env:APPS_SURVEILLANCE = '$APPS_SURVEILLANCE'}
${APPS_FAUTE:+\$env:APPS_FAUTE = '$APPS_FAUTE'}
```

🔴 **Piège payé cinq fois par ce dépôt, et évité par G2 et G3 exactement de cette
façon.** En D7, l'implémenteur **et** le relecteur avaient vérifié la propriété
**en traçant le code** : le tracé était juste, et la valeur ne pouvait simplement
pas atteindre le processus.

**Le contrôle qui vaut n'est pas la lecture du script** : c'est de relever
`$env:APPS_SURVEILLANCE` dans le `C:\dev\run-agent.ps1` **GÉNÉRÉ**, puis la ligne
`mode de surveillance retenu mode=…` dans le journal de l'agent. G2 a fermé le
sien ainsi pour `ICONES`.

### D9 — L'extraction de `boucle.rs` précède l'addition, et elle est VERBATIM

`agent/src/apps/boucle.rs` vaut **385** lignes ; les additions de G4 et leur
documentation le porteraient au-delà de la porte de 450. **La tâche 4 extrait
AVANT que la tâche 8 n'ajoute quoi que ce soit.**

`Memoire`, `reconcilier` et `mesurer` (l. 18-257) partent **caractère pour
caractère** vers `agent/src/apps/boucle/memoire.rs`, déclaré par un `mod
memoire;` **ordinaire** à l'intérieur de `boucle.rs`. **Aucun `#[path]`** : les
deux sont `#[cfg(windows)]`, et la « Convention de module enfant » de `CLAUDE.md`
écrit noir sur blanc qu'un module gaté qui n'a pas besoin d'exister sur l'hôte
reste un enfant normal.

⚠️ **Le contrôle de l'extraction n'est pas « ça compile »** : c'est
`git show <commit>:agent/src/apps/boucle.rs | sed -n '18,257p'` opposé au
nouveau fichier, **et le compte de tests inchangé**.

### D10 — La rafale est INVISIBLE AU PRODUIT PAR CONSTRUCTION

**C'est la réponse à « l'instrument peut détruire ce qu'il mesure », et elle est
structurelle, pas procédurale.**

- **Où** : `%APPDATA%\Microsoft\Windows\Start Menu\Programs\g4-rafale\`, un
  sous-répertoire **neuf** d'une racine surveillée.
  ⚠️ **Ni le Bureau ni le Bureau public** : le premier est capturé par le
  superviseur et visible, le second **apparaît sur le bureau de tous les
  utilisateurs**.
- **Quoi** : des fichiers `g4-rafale-NNNNN.tmp`. 🔴 **L'extension est le point :**
  `lecture::lnk_sous` ne retient que `.lnk` (relevé), donc **le catalogue ne peut
  pas les voir**. Un nettoyage qui échouerait à mi-course laisse des fichiers que
  le produit ignore : le corpus reste à **220**, et le dommage se borne à de
  l'espace disque et à un répertoire au nom sans ambiguïté.
- **Nettoyage** : `Remove-Item -Recurse -Force` du répertoire entier, dans le
  `finally` du script, **et** un contrôle de sortie qui recompte les `.lnk` des
  quatre racines et exige **220**.
- **Préparation** : les *N* fichiers sont fabriqués dans `C:\dev\g4-preparation\`,
  **hors des racines surveillées**, puis copiés par `robocopy /MT:32`. Fabriquer
  sur place mesurerait la vitesse de PowerShell, pas celle du système de
  fichiers.

⚠️ **Risque nommé et non éliminé** : `%APPDATA%\…\Start Menu` est indexé par le
Shell, qui peut reconstruire sa base de tuiles. **Cinq mille fichiers y sont un
geste réel**, et S1 le joue **avant** le produit précisément pour que ce soit une
sonde qui le découvre, pas une recette.

### D11 — 🔴 Le critère ③ se juge sur `cles=` DANS LE JOURNAL DE L'AGENT, jamais sur la plateforme

Corollaire direct de M2 : la plateforme reçoit un catalogue **complet** toutes
les trente secondes, quoi qu'il arrive. **`GET /applications` serait donc complet
même sur un agent dont la surveillance perd tout.** Le juger là serait un
contrôle qui ne peut pas échouer.

**Le chiffre-juge est le champ `cles=` de la ligne `catalogue reconcilie`**, et
la corroboration par `GET /applications` est **une corroboration**, pas le
critère. *La distinction est écrite ici pour qu'aucune tâche ne l'inverse.*

### D12 — 🔴 CE QUE L'INJECTION ÉTABLIT, ET CE QU'ELLE N'ÉTABLIT PAS

`APPS_FAUTE`, **variable de BANC, jamais une configuration livrée**. Trois
familles, **budget GLOBAL AU PROCESSUS** — `OnceLock` + `AtomicU32` +
`fetch_update`, sur le patron exact de
`agent/src/transport/piste_audio/injection.rs` :

| Valeur | Effet | Ce qu'elle rend atteignable |
| --- | --- | --- |
| `debordement:<n>` | les *n* prochaines complétions sont traitées comme des débordements : comptées, journalisées, **et déclenchantes** | le chemin de code du critère ②, quand la rafale réelle ne déborde pas |
| 🔵 `muette:<n>` | les *n* prochaines complétions sont **AVALÉES** : ni comptées, ni journalisées, ni déclenchantes | **le SEUL montage qui rende le critère ③ DISCRIMINANT** — voir E4 |
| `perte:<n>` | les *n* prochaines complétions rendent une erreur fatale de handle | le rétablissement d'une surveillance perdue (relevé ⑤) |

⚠️ **Convention `absente = désarmée`**, celle d'`AUDIO_FAUTE_LECTURE`,
d'`AUDIO_FAUTE_RECONSTRUCTION` et d'`INSTALLATION_FAUTE` — **jamais** celle de
`PLEIN_ECRAN`. Trace, **émise seulement si armée** :
`faute de surveillance ARMEE (APPS_FAUTE=…) : banc, jamais une configuration
livrée` (`warn!`).

🔴 **BUDGET GLOBAL, ET LA RAISON EST MESURÉE AILLEURS** : D10 a payé un budget
relu **par fil** sur `AUDIO_FAUTE_LECTURE` — chaque capture reconstruite recevait
un budget neuf, et **le chiffre-juge était structurellement incapable de quitter
zéro, sur un produit pourtant corrigé**. Ici la surveillance **se rouvre** après
une perte : un budget relu à la réouverture se réarmerait à l'identique.

🔴 **ET VOICI CE QUE L'INJECTION N'ÉTABLIT PAS, écrit d'avance :
elle établit que le REMÈDE fonctionne, jamais qu'une CAUSE existe.** C'est la
phrase que D11 a écrite d'`AUDIO_FAUTE_RECONSTRUCTION`, et elle vaut mot pour
mot. Une ligne `notifications perdues` produite par injection **ne dit rien** de
la probabilité qu'un débordement réel se produise sur cette machine ; seule la
rafale le dit, et son verdict peut être NON MESURABLE.

### D13 — La surveillance déduplique ses racines, et un `warn!` par TRANSITION

- **Déduplication** : `racines()` ne déduplique pas, et rien ne garantit que les
  quatre dossiers connus soient distincts sur une configuration inhabituelle.
  Ouvrir deux fois le même répertoire gaspillerait 64 Kio de pool non paginé pour
  compter chaque événement deux fois. **La surveillance déduplique**, par un
  `BTreeSet<PathBuf>`. ⚠️ **`racines()` n'est PAS touchée** : elle est partagée
  avec la réconciliation, où les doublons sont déjà inoffensifs
  (`vus.insert(app.cle)`).
- **Journal** : une ligne quand une racine **entre** en échec, une ligne quand
  elle **en sort**. **Jamais une ligne par tentative.** C'est le patron de
  l'ensemble `ecartes` de `boucle.rs`, dont le commentaire chiffre ce qu'il
  évite : *« sans cet ensemble, les sept écarts de cette VM feraient 20 160
  lignes par jour »*, dans un journal que le superviseur, le capteur et tous les
  enfants partagent depuis D4.
- **Repli** : `delai_de_repli` de `agent/src/plateforme/repli.rs` est
  **RÉUTILISÉ, pas recopié** — doublant de 500 ms à 30 s, déjà testé, déjà
  protégé contre le débordement de décalage.

⚠️ **Trou nommé, non fermé** : une racine **absente au démarrage** n'a pas de
handle, donc pas d'erreur, donc pas de tentative de réouverture. Si elle
apparaît plus tard, **elle n'est jamais surveillée** et seule la réconciliation
périodique la voit. C'est de la latence, jamais une perte (spec D1), et le fermer
demanderait un minuteur de re-résolution que rien ne justifie aujourd'hui.

### D14 — Le `warn!` de débordement n'est PAS limité en débit, et c'est raisonné

Une ligne `warn!` par débordement, avec sa racine et le compte cumulé. **Pas de
limitation de débit**, contrairement au patron `ecartes` de D13, et pour une
raison qui s'écrit : **un débordement exige plus de 1 260 événements entre deux
réarmements** (D2), c'est-à-dire dans les microsecondes qui les séparent — il est
**rare par construction**. Limiter son débit **cacherait exactement le cas
pathologique qu'on voudrait voir**.

⚠️ **Si la recette relève un déluge, c'est un RÉSULTAT** : le sous-bloc le
consigne et lègue la limitation, il ne l'ajoute pas en catastrophe.

### D15 — G4 rend la fenêtre de verdict de G3 PLUS juste, pas moins — et corrige la course qui la menace

**Vérifié en lisant les deux chemins**, et il faut le dire parce que l'inverse
serait facile à craindre :

- `Fenetres::ajouter` est appelée à **chaque** réconciliation, y compris celles
  que G4 déclenche. Une fenêtre ouverte pendant une installation compte donc les
  apparitions **plus près de l'instant où elles ont lieu**. ✅ **Plus juste.**
- La poignée de main `reconcilier` / `reconciliee` reste intacte : le drapeau est
  levé à la sortie de l'installeur, l'attente rompue, la réconciliation faite, le
  drapeau abaissé. ✅
- 🔴 **Mais la course de E5 s'aggrave**, et G4 la corrige — voir E5.

---

## Divergences relevées entre la spec, ce que G1-G3 ont livré, et l'état RÉEL de l'arbre

### E1 — L'arborescence de la spec range `rebond.rs` à côté de `surveillance.rs` ; ce plan le range DESSOUS

Spec §6 : `src/apps/surveillance.rs` et `src/apps/rebond.rs`, frères.
**Ce plan** : `apps/surveillance.rs` **sans `cfg`**, parent de `mode.rs`,
`rebond.rs`, `faute.rs`, `partage.rs` (tous purs ou sans `cfg`) et de `fil.rs`,
`racine.rs` (`#[cfg(windows)]`).

**Pourquoi** : c'est l'idiome que ④ a **déjà** établi deux fois —
`apps/icone.rs` (53 lignes, sans `cfg`, parent de purs et de Windows) et
`apps/installation.rs` (31 lignes, idem). Et `apps/icone.rs` porte en tête la
raison exacte : *« on ne peut pas déclarer un PETIT-fils depuis le grand-parent
sans `#[path]` »*. **Suivre la spec à la lettre obligerait au `#[path]` que ④
évite depuis G2.**

### E2 — La ROUGE du critère ① que la spec nomme n'est PAS JOUABLE

Spec : *« la ROUGE est le binaire de G1 »*. **Elle n'est pas jouable, et c'est
mesuré** : `PLATEFORME_VERSION` vaut **4** ; le binaire de G1 parle **1**, celui
d'avant G2 parle **2** (`C:\dev\agent-v2-avant-g2.exe`, conservé). Un agent v1 ou
v2 face à la plateforme d'aujourd'hui **est refusé et boucle sans terme** — G1 l'a
mesuré, sept reprises et un palier à 30 s, **sans jamais pouvoir lire le refus**.

**Tranché** : la ROUGE de ① est **`APPS_SURVEILLANCE=0`** sur le binaire de G4,
qui reproduit exactement le comportement de G1 sans en reproduire le protocole.
🔵 **Elle est même MEILLEURE que celle de la spec** : même binaire, même corpus,
même machine, une seule variable de différence.

### E3 — 🔴 LE CRITÈRE ② PORTE UNE RÉSERVE QUE LA SPEC NE NOMME PAS : notre débordement est LUI-MÊME un événement

Le tampon déborde, `lpBytesReturned` vaut **0**, et **la complétion se produit
quand même**. Notre surveillance la voit, l'appelle « quelque chose a bougé »,
et **déclenche une réconciliation**. C'est-à-dire : **dans notre conception, un
débordement se répare tout seul, sans que la réconciliation périodique ait rien
à faire.**

⚠️ **Ce n'est pas une objection au design — c'est le design.** Mais cela réfute la
lecture naïve du critère ③ (voir E4).

### E4 — 🔴 LE CRITÈRE ③ DE LA SPEC EST TRÈS PROBABLEMENT NON DISCRIMINANT, ET IL FAUT LE DIRE AVANT DE LE JOUER

Spec : *« ③ Un débordement ne perd aucune application ; la ROUGE est un agent
purement événementiel, elle se joue en désarmant la réconciliation
périodique. »*

**Raisonné sur le code, avant toute mesure :**

1. une réconciliation, **quel que soit son déclencheur**, relit le disque
   **entier** ;
2. un débordement est **lui-même une complétion**, donc un déclencheur (E3) ;
3. la fin d'une rafale produit de toute façon des notifications **non perdues**,
   qui déclenchent ;
4. le tout premier tour est `complet` **par construction**
   (`let mut complet = true`), donc un changement survenu **agent arrêté** est
   rattrapé au démarrage — **pas par la réconciliation périodique**.

🔴 **Conclusion : sur les trois cas que le protocole sait construire — rafale,
débordement, agent arrêté — la ROUGE de ③ sera VERTE.** Ce qui achète l'absence
de perte n'est pas la réconciliation périodique : c'est le fait que toute
réconciliation relit tout.

**Ce que la réconciliation périodique achète RÉELLEMENT** est le seul cas que
D1 nomme et qu'aucun événement ne peut signaler : **une surveillance qui cesse de
délivrer SANS ERREUR** — un handle invalidé en silence, un volume remonté, un
filtre qui rate un changement… **et un défaut de notre propre surveillance.**

**Décisions, écrites d'avance :**

| Ce que la mesure rend | Ce qu'il faut écrire |
| --- | --- |
| la ROUGE de la spec est **ROUGE** (le catalogue reste incomplet) | la propriété est bien achetée par la réconciliation périodique, **comme la spec le dit**, et mon raisonnement était faux. **Le dire.** |
| la ROUGE de la spec est **VERTE** | ③ est **TENU** — le catalogue est complet — **et NON DISCRIMINANT par ce montage**. Écrire les deux moitiés, et **nommer le mécanisme réel** (E3) |

🔵 **ET LE MONTAGE QUI, LUI, PEUT ÊTRE ROUGE** : `APPS_FAUTE=muette:<n>`
(D12). Une complétion avalée est **exactement** la panne que D1 décrit et
qu'aucun événement ne signale.

- **VERT** (périodique armée) : la réconciliation suivante rattrape,
  `cles` passe à 157.
- **ROUGE** (`APPS_SURVEILLANCE=seule`) : rien ne rattrape ; l'attente ne
  s'achève jamais, `cles` **reste à 156**, et l'application n'apparaît **jamais**.

⚠️ **Vérifié que ce rouge est propre** : en mode `seule`, ni un changement
d'identité (qui ne fait que lever `complet`) ni un ordre `Lancer` ne rompent
l'attente ; seul `partage.reconcilier` le ferait, et **aucune installation ne
court dans ce montage**.

### E5 — 🔴 UN COMMENTAIRE DE `boucle.rs` NOMME EXACTEMENT LE DÉFAUT QUE SON PROPRE CODE PRODUIT

`agent/src/apps/boucle.rs:322-327`, verbatim :

```rust
// ⚠️ LE DRAPEAU SE BAISSE APRÈS LA RÉCONCILIATION, PAS AVANT : le fil
// d'installation attend `reconciliee`, et le lever trop tôt lui ferait
// lire un compte pris avant que l'installeur n'ait fini d'écrire.
if partage.reconcilier.swap(false, SeqCst) {
    partage.reconciliee.store(true, SeqCst);
}
```

**Le chemin nominal est correct** : le drapeau est levé à la sortie de
l'installeur, l'attente est rompue, la réconciliation qui suit a **commencé
après** la sortie.

🔴 **Le chemin de course ne l'est pas.** Si une réconciliation **périodique est
déjà en cours** quand l'installeur sort et lève le drapeau, le `swap` posé
**après** la voit vrai et déclare `reconciliee` — **pour une réconciliation qui a
commencé AVANT que l'installeur n'ait fini d'écrire**. `forcer_une_reconciliation`
(`installation/fil.rs:311-315`) rend alors la main immédiatement, et le verdict
se lit sur une fenêtre qui **n'a pas vu les derniers fichiers écrits** : un
`sans-effet` **faux**, c'est-à-dire exactement ce que le commentaire dit vouloir
empêcher.

**Le remède tient en un déplacement de deux lignes** — lire et abaisser le
drapeau **AVANT** d'appeler `reconcilier` :

```rust
let demandee = partage.reconcilier.swap(false, SeqCst);
let (diff, catalogue) = reconcilier(&mut memoire, declencheur);
…
if demandee { partage.reconciliee.store(true, SeqCst); }
```

Vérifié sur les trois chemins : nominal ✅, course ✅ (le drapeau survit au tour et
la réconciliation *suivante* l'honore, une période plus tard mais **juste**),
et **aucun** chemin ne perd la demande.

**Pourquoi c'est de G4 :** la fenêtre de course vaut aujourd'hui `durée de la
réconciliation / période`, soit **≈ 0,2 %** des sorties d'installeur. **G4 rend
les réconciliations bien plus fréquentes pendant une installation** — c'est tout
son objet — et **élargit donc cette fenêtre d'un ordre de grandeur.**

⚠️ **SA SEULE PREUVE EST UN ARGUMENT DE FLOT DE CONTRÔLE.** `boucle.rs` est
`#[cfg(windows)]` : aucun test d'hôte ne peut l'atteindre, et la recette de G4 ne
lance aucune installation. C'est la situation exacte que le défaut F1 du
sous-bloc D7 a payée. **Le correctif est appliqué parce qu'il est strictement
plus sûr ; la mesure est LÉGUÉE, et déclarée manquante.**

### E6 — La spec §6 n'a pas prévu où vit l'état partagé, et `installation/partage.rs` a déjà répondu

`installation/partage.rs` porte en tête : *« CE MODULE N'A AUCUN `cfg`, ET C'EST
LE POINT. Le fil d'installation est Windows ; ce qu'il partage avec la découverte
ne l'est pas […]. C'est aussi ce qui rend ces deux mécanismes observables sur
l'hôte. »*

**G4 réemploie la forme, pas le fichier** : `apps/surveillance/partage.rs`, sans
`cfg`, porte les deux compteurs monotones et le drapeau d'arrêt.
⚠️ **Deux états partagés distincts, et c'est voulu** : la demande d'installation
et la notification de fichier n'ont ni la même cause, ni la même sémantique, ni
le même consommateur d'écriture. Les fusionner ferait un objet qui ment sur les
deux.

### E7 — Contrôle qui PASSE : G4 n'a **aucun** message qui traverse un `match` catch-all

Le bras `Ok(autre) => return` de `agent/src/capteur/pont_media.rs` **tue le fil en
silence**, et ce dépôt l'a payé **cinq** fois — D5 (`Sommeil`), D6 (`Part`), D7
(`Audio`), D8 (`PleinEcran`), P1 (`PressePapier`).

✅ **G4 n'ajoute aucun message, ni au canal `/agent` (D1), ni au tube du capteur.**
Le contrôle est donc **sans objet**, et il est écrit ici plutôt que supposé :
la tâche de clôture vérifie que `git diff --name-only` ne porte **aucun** fichier
sous `agent/src/capteur/` ni `proto/`.

### E8 — La divergence `403 vm-etrangere` / `404 vm-inconnue` — SIGNALÉE, NON TRANCHÉE

Elle survit dans **un seul fichier**, `plateforme/src/http/routes-applications.ts`
(G1), contre P4, G2 et G3 alignés sur le refus **indistinguable**. Le `403` est
un **oracle d'énumération** : un utilisateur apprend par tâtonnement quelles VMs
existent.

🔴 **G4 ne la rencontre pas** — il ne touche pas `plateforme/`. **Elle est
rappelée ici pour qu'aucune tâche ne la « corrige en passant » : c'est une
décision de sécurité, et elle appartient au propriétaire du dépôt.**

### E9 — La lacune de nommage d'`IssueLancement` reste OUVERTE, et G4 ne la ferme pas

Un `rename_all` est **inobservable** sur un enum dont toutes les variantes tiennent
en un mot (G1). G4 ajoute bien un état à deux mots — `Mode::SansRebond` — mais
**il ne voyage sur aucun fil et n'est sérialisé nulle part** : il ne referme donc
rien. **Dit, plutôt que compté à tort.**

---

## Structure des fichiers

```
agent/src/apps.rs                            MODIFIÉ  +pub mod surveillance; +le fil dans Poignees
agent/src/apps/surveillance.rs               NEUF     SANS cfg — le parent, `demarrer`, les constantes
agent/src/apps/surveillance/mode.rs          NEUF     PUR — les quatre états d'APPS_SURVEILLANCE
agent/src/apps/surveillance/rebond.rs        NEUF     PUR — l'anti-rebond, horloge en paramètre
agent/src/apps/surveillance/faute.rs         NEUF     PUR + le budget GLOBAL au processus
agent/src/apps/surveillance/partage.rs       NEUF     SANS cfg — deux compteurs monotones, un arrêt
agent/src/apps/surveillance/fil.rs           NEUF     #[cfg(windows)] — l'attente, les N racines
agent/src/apps/surveillance/racine.rs        NEUF     #[cfg(windows)] — ouvrir, armer, compléter, rouvrir
agent/src/apps/boucle/memoire.rs             NEUF     #[cfg(windows)] — EXTRACTION VERBATIM (D9)
agent/src/apps/boucle.rs                     MODIFIÉ  le sondage, le déclencheur, le mode, E5, D7
scripts/run-agent.sh                         MODIFIÉ  DEUX lignes, tâche DÉDIÉE (D8)
docs/superpowers/plans/journaux-gestion-apps-g4/      NEUF — journaux, sondes, rouges
```

**Ce que G4 NE crée PAS, et il faut le dire** : aucune migration, aucun vecteur
partagé, aucun fichier sous `proto/`, `plateforme/` ou `client/`.

---

## Interfaces partagées

```rust
// apps/surveillance/partage.rs — SANS cfg
#[derive(Clone, Default)]
pub struct Veille {
    /// MONOTONE, jamais remis à zéro. La boucle compare à la valeur qu'elle a
    /// retenue : un incrément survenu PENDANT une réconciliation est donc vu au
    /// sondage suivant, ce qu'un booléen échangé perdrait.
    notifications: Arc<AtomicU64>,
    /// MONOTONE aussi. C'est lui qui rend le critère ② lisible sur la ligne
    /// `catalogue reconcilie`, en plus du `warn!`.
    debordements: Arc<AtomicU64>,
    arret: Arc<AtomicBool>,
}
impl Veille {
    pub fn notifications(&self) -> u64;
    pub fn debordements(&self) -> u64;
    pub fn signaler(&self);            // +1 notifications
    pub fn signaler_debordement(&self); // +1 débordements ET +1 notifications
    pub fn arreter(&self);
    pub fn arretee(&self) -> bool;
}

// apps/surveillance/mode.rs — PUR
pub enum Mode { Armee, Desarmee, SansRebond, Seule }
impl Mode {
    /// Rend le mode ET, le cas échéant, la valeur inconnue à journaliser.
    /// 🔴 Jamais un `is_some()` : chaque état est une ÉGALITÉ EXACTE, et `0`
    /// réemploie `apps::desarme`.
    pub fn lire(valeur: Option<&str>) -> (Mode, Option<String>);
    pub fn surveille(&self) -> bool;    // false pour Desarmee
    pub fn rebond(&self) -> bool;       // false pour SansRebond
    pub fn periodique(&self) -> bool;   // false pour Seule
}

// apps/surveillance/rebond.rs — PUR
pub const DELAI_ANTI_REBOND: Duration = Duration::from_millis(750);
pub const DELAI_ANTI_REBOND_MAX: Duration = Duration::from_secs(4); // D6
#[derive(Default)]
pub struct Rebond { premiere: Option<Instant>, derniere: Option<Instant> }
impl Rebond {
    pub fn notifier(&mut self, maintenant: Instant);
    pub fn echeance(&self) -> Option<Instant>;
    pub fn du(&self, maintenant: Instant) -> bool;
    pub fn consommer(&mut self);
}

// apps/surveillance/faute.rs — PUR (+ un static)
pub enum Famille { Debordement, Muette, Perte }
pub fn lire(valeur: Option<&str>) -> Option<(Famille, u32)>;  // PURE, testée
pub fn consommer(famille: Famille) -> bool;                   // budget GLOBAL

// apps/surveillance.rs — SANS cfg
pub const TAMPON_NOTIFICATIONS: usize = 65_536;   // D2, NON CALIBRÉ
pub fn demarrer(mode: Mode) -> (Veille, Option<JoinHandle<()>>);
// #[cfg(not(windows))] : rend une `Veille` inerte et `None`, SANS journaliser —
// même raison qu'`apps::demarrer` : un agent Linux n'a pas à se plaindre.

// apps/boucle.rs — le déclencheur, sur la ligne `catalogue reconcilie`
enum Declencheur { Demarrage, Periode, Notification, Installation }
```

⚠️ **`signaler_debordement` incrémente les DEUX compteurs**, et c'est délibéré :
un débordement **est** un événement (E3), et le compter comme une notification
est ce qui déclenche la réconciliation qui répare. Ne l'incrémenter que dans
`debordements` **ouvrirait** le chemin de perte que D1 existe pour fermer.

---

## Ordre et parallélisme

```
Famille U (VM, PRÉALABLE EXTERNE)      Tâche 1        — la porte S1
      │ (ne bloque AUCUNE des familles 0 à 3)
Famille 0 (hôte, PUR)                  Tâches 2, 3    — parallélisables
      ▼
Famille 1 (hôte, extraction)           Tâche 4        — AVANT toute addition
      ▼
Famille 2 (Windows, la surveillance)   Tâches 5, 6, 7
      ▼
Famille 3 (le câblage)                 Tâches 8, 9, 10
      ▼
Famille 4 (VM, la recette)             Tâches 11, 12
      ▼
Famille 5 (clôture)                    Tâches 13, 14, 15
```

🔴 **La tâche 1 ne bloque rien.** Son verdict change ce que la recette **joue**,
jamais ce que le produit **livre**. Si la VM est prise, les familles 0 à 3 se
jouent quand même et G4 s'arrête proprement à la fin de la tâche 10.

**Parallélisation possible** : les tâches 2 et 3 (deux modules purs sans
dépendance mutuelle) ; les tâches 6 et 7 après la 5.

---

# Famille U — la porte, avant toute ligne de produit

### Task 1 : 🔴 LA PORTE — une rafale peut-elle faire déborder un tampon de 64 Kio ?

- [ ] **Préalables, dans cet ordre, et un échec ARRÊTE la tâche** :
      `stat -c '%F' /dev/null` → `character special file` ;
      `virsh list --all` → `en cours d'exécution` ;
      `ls -la /media/vm/dev/agent.log` → si le `mtime` a moins d'une minute, **un
      agent d'un voisin tourne : NE PAS le tuer, s'arrêter et le dire.**
- [ ] Relever le corpus AVANT : les `.lnk` des **quatre** racines, par
      `find … -iname '*.lnk' | wc -l` depuis l'hôte. **220 attendu** (M1).
- [ ] Écrire `docs/superpowers/plans/journaux-gestion-apps-g4/g4-rafale.ps1`,
      **en pur ASCII** — contrôle `LC_ALL=C grep -c '[^ -~]' g4-rafale.ps1` doit
      rendre **0** — et le déposer sur le partage.
- [ ] Le script : fabrique *N* fichiers dans `C:\dev\g4-preparation\` ; pose un
      `FileSystemWatcher` sur `%APPDATA%\Microsoft\Windows\Start Menu` avec
      `IncludeSubdirectories = $true`, `InternalBufferSize = 65536`, et des
      gestionnaires `Created` **et** `Error` ; lance
      `robocopy C:\dev\g4-preparation "…\Start Menu\Programs\g4-rafale" /MT:32 /NFL /NDL` ;
      relève les six points (a) à (f) du §« LA PORTE ».
- [ ] Jouer *N* ∈ {1 000, 5 000, 20 000} — **et s'arrêter au premier
      débordement**, qui suffit à rendre le verdict 🟢.
- [ ] Lancer **par tâche planifiée `/it`**, jamais par WinRM direct.
- [ ] 🔴 **Nettoyer, puis RECOMPTER : 220 exigé.** Si le compte diffère,
      **le verdict est 🔴 NETTOYAGE ÉCHOUE**, et la rafale ne sera PAS jouée sur
      le produit.
- [ ] **Deux exécutions.** Verser les journaux, et écrire le verdict **dans les
      termes de la table du §« LA PORTE »**, sans en inventer un cinquième.
- [ ] ⚠️ **Écrire la réserve** : ce que S1 mesure est le watcher de .NET, pas le
      nôtre. **Elle borne la question, elle ne la répond pas.**

---

# Famille 0 — les modules PURS, sur l'hôte

### Task 2 : `apps/surveillance/mode.rs` — les quatre états, et aucun n'est une présence

- [ ] Créer `apps/surveillance.rs` (SANS `cfg`) et y déclarer `pub mod mode;`.
      Le déclarer dans `apps.rs` par `pub mod surveillance;`, **sans `cfg`**.
- [ ] `Mode::lire(Option<&str>)` : `None` → `Armee` ; `Some("0")` → `Desarmee`
      **via `crate::apps::desarme`, réutilisé et non recopié** ;
      `Some("sans-rebond")` → `SansRebond` ; `Some("seule")` → `Seule` ; **toute
      autre valeur** → `(Armee, Some(valeur))`.
- [ ] Tests d'hôte, **avec leurs rouges** :
      - les quatre états exacts ;
      - 🔴 la rouge de la convention : remplacer l'égalité par `valeur.is_some()`
        doit faire **tomber** le test — c'est la rouge que `apps.rs` porte déjà
        pour `APPS`, et elle vaut ici pour la même raison ;
      - `Some("seul")`, `Some("SEULE")`, `Some("")`, `Some("00")` → `Armee` **et**
        une valeur inconnue rendue ;
      - 🔴 `Some("0 ")` ne désarme pas (le test d'`apps.rs` le fixe déjà pour
        `APPS` ; le réemploi de `desarme` doit le préserver).
- [ ] **Annoncer le compte de tests AVANT de le mesurer**, puis le mesurer.

### Task 3 : `apps/surveillance/rebond.rs` et `faute.rs` — l'anti-rebond et l'injection, PURS

- [ ] `rebond.rs` : les deux constantes de D5/D6, la structure, les quatre
      méthodes. **Le commentaire de `DELAI_ANTI_REBOND_MAX` porte le calcul de D6
      en toutes lettres**, avec ses trois termes et lequel est mesuré.
- [ ] Les **cinq** tests de la table de D5, **et la rouge de chacun**. ⚠️ **Aucun
      test ne dort** : `Instant::now()` de base, plus des `Duration`.
- [ ] `faute.rs` : `Famille`, `lire` (PURE), `budget` (`OnceLock` +
      `AtomicU32`), `consommer` (`fetch_update` + `checked_sub(1)`), sur le
      patron **verbatim** de `transport/piste_audio/injection.rs`.
- [ ] Tests d'hôte de `lire` : `None` → `None` ; `"debordement:3"` →
      `(Debordement, 3)` ; `"muette:1"`, `"perte:2"` ; `"debordement"` (sans
      compte) → `None` ; `"debordement:0"` → `None` **ou** `(_, 0)` — **trancher
      dans le test, et écrire lequel** ; une famille inconnue → `None` **avec un
      `warn!`**, jamais un silence.
- [ ] 🔴 **Un test qui fixe que le budget est GLOBAL** : deux `consommer`
      successifs sur un budget de 1 rendent `true` puis `false`. Sa rouge est un
      budget relu à chaque appel — la panne que D10 a payée.

---

# Famille 1 — l'extraction, AVANT toute addition

### Task 4 : 🔴 EXTRACTION de `agent/src/apps/boucle.rs`, et rien d'autre

- [ ] **Ne rien ajouter dans ce commit.** C'est la forme forte de D9 (tâche 6)
      et de D10 (tâches 1 à 3), et elle existe précisément pour que la marge soit
      rendue **avant** d'être consommée.
- [ ] Déplacer `Memoire`, `reconcilier` et `mesurer` (l. 18-257) **caractère pour
      caractère** vers `agent/src/apps/boucle/memoire.rs`.
- [ ] Le déclarer par `mod memoire;` **ordinaire** dans `boucle.rs`. **Aucun
      `#[path]`** (D9).
- [ ] Contrôle : `git show HEAD:agent/src/apps/boucle.rs | sed -n '18,257p'`
      opposé au corps du fichier neuf — **la seule différence admise est
      l'en-tête de module et les `use`**.
- [ ] `cargo check --target x86_64-pc-windows-gnu` → sortie 0, **et le nombre
      d'avertissements de la famille `dead_code` inchangé ou expliqué**.
- [ ] Relever les deux tailles par la commande, et les écrire dans le message de
      commit.

---

# Famille 2 — la surveillance, `#[cfg(windows)]`

### Task 5 : `apps/surveillance/partage.rs` — les deux compteurs, SANS `cfg`

- [ ] La structure `Veille` et ses sept méthodes (§Interfaces).
- [ ] L'en-tête cite `installation/partage.rs` et **dit pourquoi ce sont DEUX
      états et non un** (E6).
- [ ] 🔴 Le commentaire de `signaler_debordement` écrit que **les deux compteurs
      montent**, et **pourquoi** : un débordement est un événement (E3), et ne
      pas le compter comme tel ouvrirait le chemin de perte.
- [ ] Tests d'hôte : monotonie, `signaler_debordement` monte les deux, `arreter`
      / `arretee`. Rouge de chacun.

### Task 6 : `apps/surveillance/racine.rs` — une racine, ouvrir / armer / compléter / rouvrir

- [ ] `#[cfg(windows)]`. `CreateFileW` en `FILE_LIST_DIRECTORY`,
      `FILE_SHARE_READ | WRITE | DELETE`, `OPEN_EXISTING`,
      `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED`.
- [ ] Un `OVERLAPPED` portant un `CreateEventW` **à réarmement manuel**, un
      tampon de `TAMPON_NOTIFICATIONS` octets **aligné sur 4** (`Vec<u32>` puis
      cast, jamais un `Vec<u8>` nu — l'alignement est un contrat de l'appel).
- [ ] `armer()` : `ReadDirectoryChangesW` avec les trois filtres de D2 et
      `bWatchSubtree = TRUE`.
- [ ] `completer()` : `GetOverlappedResult(bWait = false)`, puis :
      - `octets == 0` **ou** `ERROR_NOTIFY_ENUM_DIR` → **débordement** ;
      - succès → **notification** ;
      - toute autre erreur → **perte**, la racine passe en échec.
      🔴 **Le contenu du tampon n'est JAMAIS lu** (D3), et l'en-tête le dit.
- [ ] `rouvrir()` : fermer, **re-résoudre le chemin** (D13), rouvrir, réarmer.
- [ ] Le repli emploie `crate::plateforme::repli::delai_de_repli`, **réutilisé et
      non recopié** (D13).
- [ ] Le point d'injection : `completer()` consulte
      `faute::consommer(Famille::Debordement | Muette | Perte)` **avant** de
      classer. ⚠️ **`Muette` ne compte rien, ne journalise rien, ne déclenche
      rien** — c'est tout son objet (D12).
- [ ] **Aucun test d'hôte n'est possible** : le dire en tête du module, comme
      `lecture.rs` le fait. La seule vérification est
      `cargo check --target x86_64-pc-windows-gnu`, qui couvre types, emprunts,
      visibilités et durées de vie — **et pas l'édition de liens**.

### Task 7 : `apps/surveillance/fil.rs` — l'attente, l'arrêt, et un `warn!` par TRANSITION

- [ ] `#[cfg(windows)]`. Déduplique `lecture::racines()` par un
      `BTreeSet<PathBuf>` (D13) ; ouvre une `Racine` par entrée.
- [ ] `WaitForMultipleObjects(&evenements, false, 1_000)` — le délai de 1 s
      existe **pour que le drapeau d'arrêt soit vu**, jamais pour sonder.
- [ ] Sur `WAIT_OBJECT_0 + i` : compléter la racine *i*, la réarmer, et
      `veille.signaler…`.
- [ ] Sur `WAIT_TIMEOUT` : tenter la réouverture des racines **en échec dont le
      repli est échu**, et rien d'autre.
- [ ] Sur `WAIT_FAILED` : `error!` **une fois**, puis quitter — un fil qui boucle
      sur un échec d'attente est un fil qui brûle un cœur en silence.
- [ ] 🔴 **Une ligne par TRANSITION d'état d'une racine** (perdue → rétablie), et
      **jamais une par tentative** (D13).
- [ ] À l'arrêt : `CancelIoEx` sur chaque handle, puis fermeture.
- [ ] Un `warn!` de débordement par occurrence, portant la racine et le cumul
      (D14).
- [ ] `cargo check --target x86_64-pc-windows-gnu` → **sortie 0**. 🔴 **C'est ici
      que M4 cesse d'être une hypothèse.** Si une fonctionnalité manque, la
      déclarer et l'ajouter **en la nommant dans le commentaire de
      `Cargo.toml`**, comme G1 et G2 l'ont fait.

---

# Famille 3 — le câblage

### Task 8 : `apps/boucle.rs` — le sondage, le déclencheur, le mode, et les DEUX corrections

- [ ] `tourner` prend la `Veille` et le `Mode`.
- [ ] Le corps de l'attente sonde `veille.notifications()` contre la valeur
      retenue ; tout écart appelle `rebond.notifier(Instant::now())` **ou**, en
      `SansRebond`, rompt l'attente immédiatement.
- [ ] `rebond.du(maintenant)` rompt l'attente et **`consommer()`**.
- [ ] En `Mode::Seule`, **l'échéance de période n'existe pas** : `let echeance =
      mode.periodique().then(|| Instant::now() + periode)`, et la rupture sur
      `reste.is_zero()` ne court que si `Some`.
- [ ] `reconcilier` prend un `Declencheur` et le porte sur la ligne `catalogue
      reconcilie`, avec `notifications` et `debordements` **cumulés**.
      ✅ **Aucun instrument versé ne lit cette trace** — vérifié par la commande,
      §« relevé ».
- [ ] 🔴 **E5** : lire et abaisser `partage.reconcilier` **AVANT** d'appeler
      `reconcilier`, et ne poser `reconciliee` qu'ensuite. Le commentaire est
      **réécrit** : il nommait le défaut que le code produisait.
- [ ] 🔴 **D7** : la trace `reenrolement observe` dit désormais ce qu'elle
      observe, et le commentaire de `complet` écrit que **l'envoi complet est
      périodique en pratique**, avec le relevé de M2 et le legs de son coût.
- [ ] ⚠️ **Ne pas toucher `Fenetres::ajouter`** : G4 le rend plus juste sans le
      modifier (D15).

### Task 9 : `apps.rs` — le fil de surveillance dans `Poignees`

- [ ] `Mode::lire(std::env::var("APPS_SURVEILLANCE").ok().as_deref())`, l'`info!`
      **inconditionnel** `mode de surveillance retenu mode=…`, et le `warn!` si
      la valeur est inconnue (D4).
- [ ] `surveillance::demarrer(mode)` rend `(Veille, Option<JoinHandle>)` ; la
      poignée entre dans `Poignees`, **conservée sans être attendue** — la
      lâcher terminerait le fil, exactement comme les deux autres.
- [ ] `APPS=0` désarme **aussi** la surveillance : `demarrer` retourne avant
      tout. **Déclaré, pas découvert** — c'est la formulation que G3 a employée
      pour l'installation.
- [ ] La variante `#[cfg(not(windows))]` rend une `Veille` inerte et **ne
      journalise rien** (même raison qu'`apps::demarrer`).
- [ ] Relever la taille du fichier après édition. **Porte à 450.**

### Task 10 : 🔴 TÂCHE DÉDIÉE — `scripts/run-agent.sh` transmet les DEUX variables

- [ ] **Cette tâche ne fait QUE cela.** Deux lignes, sur le patron exact des
      voisines (D8).
- [ ] ⚠️ **Relire le fichier avant d'écrire** : F4 y ajoute aussi une ligne. **Ne
      rien réordonner.**
- [ ] Le commentaire dit pourquoi la tâche est dédiée, et cite les cinq
      précédents.
- [ ] **Le contrôle est différé à la recette** : le `run-agent.ps1` **généré**
      doit porter `$env:APPS_SURVEILLANCE`, et le journal la ligne `mode de
      surveillance retenu`.

---

# Famille 4 — la recette, sur la VM

### Task 11 : 🔴 LA RECETTE — les quatre critères, DEUX exécutions chacun

- [ ] **Préalables de la tâche 1**, rejoués. Puis : `ss -ltn` avant de choisir un
      port ; une **seconde instance** de plateforme sur un port libre — 🔴 **ne
      JAMAIS redémarrer celle d'un voisin**, cela invalide ses jetons (P3, G3) ;
      `npm run admin:agent` (⚠️ `AGENT_VM` attend **l'IDENTIFIANT**, pas le nom —
      piège de G1 rencontré à nouveau par G3 ; et `--adresse` n'a **aucun
      défaut**).
- [ ] `scripts/build-agent.sh` depuis un **`git worktree` au commit de G4**, avec
      `node_modules` lié. ⚠️ **`cargo clean --release -p proto -p agent`** —
      G4 ne modifie pas `proto`, mais l'horloge de la VM avance sur celle de
      l'hôte et purger un seul crate ne suffit pas (leçon du chantier E).
      **Vérifier la TAILLE du binaire** : une compilation de 0,13 s est un aveu.
- [ ] Poser `RUST_LOG` de sorte que **toutes** les traces lues puissent sortir.
      🔴 **Un zéro rendu par une trace qu'on n'a pas allumée n'est pas une
      mesure** — G2 l'a payé sur `apps::icone`.
- [ ] **Le raccourci témoin** : `G4 Temoin.lnk`, avec un **argument distinct**
      (`--g4-temoin`). ⚠️ **Sans argument distinct, sa clé serait celle d'un
      raccourci existant et le catalogue n'en garderait qu'un** — G2 a perdu un
      témoin sur deux pour cette raison exacte.
- [ ] 🔴 **L'horodatage de création se prend SUR LA VM**, par le script qui crée
      le fichier (`Get-Date -Format o` avant et après), **jamais sur l'hôte** :
      les deux horloges divergent, et le journal d'agent est en heure VM.

**Critère ① — un raccourci créé apparaît en moins de 5 s**

- [ ] VERT (variable absente) : créer `G4 Temoin.lnk`, relever `T0` sur la VM,
      chercher la première ligne `catalogue reconcilie … apparues>=1` après `T0`,
      calculer `T1 − T0`. **< 5 s exigé.** Relever aussi `declencheur=notification`.
- [ ] ROUGE : **`APPS_SURVEILLANCE=0`** (E2). Le même geste doit mettre **jusqu'à
      `PERIODE_RECONCILIATION`**. ⚠️ **Le rouge n'est rouge que si le délai
      dépasse 5 s** : un raccourci créé une seconde avant une réconciliation
      périodique passerait. **Créer juste APRÈS une ligne `catalogue reconcilie`**,
      et le dire dans le protocole.
- [ ] **Deux exécutions par bras.** Retirer le témoin entre les bras, et
      **recompter 220**.

**Critère ② — un débordement est détecté et journalisé**

- [ ] **Si et seulement si S1 a rendu 🟢 et que son nettoyage a rendu 220** :
      rejouer la rafale, avec le *N* et le régime que S1 a dimensionnés.
- [ ] Chercher `notifications perdues`. **Deux exécutions.**
- [ ] 🔴 **Si aucun débordement : écrire `② NON MESURABLE`, avec le régime
      atteint et le compte de notifications reçues.** Ne pas rétrécir le tampon.
      Ne pas conclure.
- [ ] **Dans les DEUX cas**, jouer `APPS_FAUTE=debordement:3` et relever que la
      ligne sort **trois fois** et que `debordements=3` apparaît sur la ligne
      `catalogue reconcilie`. ⚠️ **Étiqueter ce relevé `②bis (injection)`** et
      écrire, mot pour mot, que **l'injection établit que le remède fonctionne,
      jamais qu'une cause existe** (D12).

**Critère ③ — un débordement ne perd aucune application**

- [ ] **Montage A, celui de la spec** : rafale, création du témoin **pendant**,
      puis arrêt de la rafale ; attendre ≥ 2 × `PERIODE_RECONCILIATION` ; lire
      `cles=`.
      - VERT (variable absente) : **157** attendu.
      - ROUGE (`APPS_SURVEILLANCE=seule`) : **la spec attend 156.**
      🔴 **Écrire d'avance, et ne pas arbitrer sous le coup du résultat** : si le
      ROUGE rend **157**, ③ est **TENU et NON DISCRIMINANT par ce montage**, et
      le mécanisme réel est celui d'E3. Si le ROUGE rend **156**, mon
      raisonnement d'E4 était faux, et **il faut le dire**.
- [ ] 🔵 **Montage B, celui qui PEUT être rouge** : `APPS_FAUTE=muette:1`.
      Créer le témoin, et vérifier que la complétion correspondante est avalée
      (compteur `notifications` inchangé).
      - VERT : la réconciliation périodique rattrape → `cles=157`.
      - ROUGE (`seule` + `muette:1`) : **`cles` reste à 156**, indéfiniment.
- [ ] **Deux exécutions par montage et par bras.**

**Critère ④ — l'anti-rebond réduit le nombre de réconciliations**

- [ ] VERT (variable absente) et ROUGE (`APPS_SURVEILLANCE=sans-rebond`),
      **même binaire, même corpus, même machine**, sur la **même** rafale
      (à défaut, sur une série de 200 créations `.tmp` étalées sur 3 s).
- [ ] Compter les lignes `catalogue reconcilie` dans la fenêtre de la rafale,
      **bornée explicitement par deux horodatages**, jamais sur un total de
      fichier — c'est la conséquence que `boucle.rs` écrit lui-même de l'ensemble
      `ecartes`, et que D2 a payée sur ses « 44 avant / 44 après ».
- [ ] 🔴 **AUCUN RATIO N'EST PRÉDIT ICI.** Ce qui est écrit d'avance est
      seulement ceci : **le ROUGE ne mesure pas « anti-rebond contre rien », il
      mesure « anti-rebond contre le sondage de 200 ms »**, qui est déjà un
      anti-rebond faible. **Si les deux bras rendent le même compte, ④ est NON
      MESURABLE et doit le dire.**
- [ ] **Deux exécutions par bras.**

**Relevé ⑤ — le rétablissement d'une surveillance perdue** *(hors critères de la
spec, ajouté par ce plan et déclaré comme tel)*

- [ ] `APPS_FAUTE=perte:1` : relever la ligne de transition « racine perdue »,
      puis « racine rétablie », et vérifier qu'une notification postérieure
      déclenche encore. **Une exécution.**

**Relevé ⑥ — le témoin de repos, et il PEUT échouer** *(idem)*

- [ ] Agent armé, disque au repos, **≥ 3 périodes** : compter les lignes
      `catalogue reconcilie` et les `notifications`.
- [ ] **Attendu : une réconciliation par période, `declencheur=periode`, et zéro
      notification.** 🔴 **Si les racines sont bruyantes au repos, la surveillance
      multiplie le travail par `PERIODE_RECONCILIATION / DELAI_ANTI_REBOND_MAX`
      = 7,5, et c'est un RÉSULTAT à écrire** (D3), pas un incident.

### Task 12 : nettoyage et contrôle de sortie

- [ ] Retirer `G4 Temoin.lnk`, `…\g4-rafale\`, `C:\dev\g4-preparation\`.
- [ ] **Recompter les `.lnk` des quatre racines : 220 exigé.**
- [ ] Relever ce que G4 **laisse** sur la VM, et pourquoi — sur le modèle du
      legs n°9 de G2, qui a évité qu'un chantier suivant balaie ses témoins.
- [ ] ⚠️ **Copier `agent.log` APRÈS la fin réelle de l'exécution**, jamais à la
      fin du pilote (D4).

---

# Famille 5 — clôture

### Task 13 : 🔴 LA REVUE TRANSVERSE DE FIN DE BRANCHE — obligatoire

Barème : **5** en D7, **3** en D8, **6** en D9, **douze** en D10, **sept** en
D11, **huit** en P1, **dix** en P2 (sur **vingt-trois places**), **cinq** en S1,
**neuf** sur le chantier E, **douze** en P3, **douze** en S2, **treize** en S3,
**huit** en P4, **huit** en G1, **onze** en F1, **vingt-sept** en S4.

- [ ] Cible propre : **les affirmations devenues fausses DANS LA BRANCHE
      elle-même**. Elles franchissent toutes une frontière de tâche, et **aucune
      revue par tâche ne peut structurellement les voir**.
- [ ] Candidats connus d'avance, à vérifier un par un :
      - `agent/src/apps.rs:32-35` — *« la notification par `ReadDirectoryChangesW`
        d'un sous-bloc ultérieur ne sera qu'un ACCÉLÉRATEUR »* : **le futur est
        passé** ;
      - `agent/src/apps/installation/partage.rs:30-33` — *« C'EST CE QUE G4
        RENDRA IMMÉDIAT »* : idem ;
      - `agent/src/apps/boucle.rs:322-327` — le commentaire d'E5, **réécrit par la
        tâche 8**, à relire **après** ;
      - `agent/src/apps/boucle.rs:298-302` — le commentaire de `complet`, que M2
        complète ;
      - la spec §5 « G4 » — sa ROUGE de ① n'est pas jouable (E2), et sa ROUGE de
        ③ très probablement non discriminante (E4). **Annoter, jamais réécrire :
        un relevé daté reste vrai comme histoire** ;
      - `CLAUDE.md` — le legs n°6 de G3, *« G4 et G5 n'ont pas de plan »*.
- [ ] 🔴 **Énumérer les places par `grep -n` AVANT d'écrire, et les RELIRE place
      par place APRÈS.** *Une substitution qui ne dit pas combien d'occurrences
      elle a touchées est une affirmation de complétude non vérifiée.*
- [ ] ⚠️ **Trier avant de corriger** : S4 a trouvé **17** occurrences d'un compte,
      dont **trois** étaient des citations justes et **neuf** nommaient un autre
      compte. **Une substitution globale les aurait abîmées.**
- [ ] ⚠️ **Relire une citation `fichier:ligne` APRÈS l'exécution de ce qui la
      déplace** : P3 a vu la sienne rendue fausse par sa propre branche. **Et
      dans `CLAUDE.md`, ne jamais citer un numéro de ligne** — on écrit toujours
      au-dessus.
- [ ] ⚠️ **La revue transverse est elle-même une source de croissance** : S2 y a
      perdu 13 lignes de marge, S3 y a ajouté **+54 lignes**. **Le relevé de
      tailles qui fait foi est celui d'APRÈS.**

### Task 14 : `CLAUDE.md` — la section G4, et les tailles PAR LA COMMANDE

- [ ] La section, sur le modèle de G1/G2/G3 : le fait qui gouverne, les critères
      **avec leur nombre d'exécutions**, la variable neuve, ce que G4 n'établit
      pas, les pièges neufs, les legs.
- [ ] **Les deux variables** au tableau des variables d'environnement, avec leur
      convention **et** la raison de la convention.
- [ ] Le tableau des familles de lecture des journaux, **relevé par la commande**
      (`file`, `grep -lP '\x1b\['`, un balayage d'octets NUL).
- [ ] 🔴 **Relancer la commande des tailles APRÈS la dernière édition de la
      ronde**, revue transverse comprise, et **dire à quel commit** : l'arbre est
      partagé, et deux chantiers y écrivent.
- [ ] Consigner, **sans le corriger**, que le tableau de dette de `CLAUDE.md`
      publie **quatre** lignes pour **deux** réelles (§« relevé »).

### Task 15 : le document de résultats

- [ ] `docs/superpowers/plans/2026-08-21-gestion-apps-g4-resultats.md`.
- [ ] 🔴 **Chaque énoncé porte son nombre d'exécutions.** Aucun taux.
- [ ] 🔴 **Aucune pièce fabriquée.** Ce dépôt a vu deux fois un fait **vrai**
      présenté avec une preuve **inventée** — une transcription `cargo`
      assemblée à la main, et une sortie de commande inscrite dans `CLAUDE.md`
      sans avoir été lancée. **C'est le mode de défaillance qu'il juge le plus
      grave.** Si une commande n'a pas été lancée, l'écrire.
- [ ] Les journaux **versés** sous
      `docs/superpowers/plans/journaux-gestion-apps-g4/`.

---

## Ce que G4 n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux ; **une** pour
  les relevés ⑤ et ⑥, et **une** pour la sonde S1 par valeur de *N*.
- 🔴 **Que le critère ③ soit discriminant.** E4 raisonne qu'il ne le sera pas par
  le montage de la spec, et le montage B (injection) **prouve le remède, jamais
  qu'une cause existe** (D12).
- 🔴 **Qu'un débordement réel se produise jamais sur cette machine.** Si S1 et la
  rafale ne débordent pas, tout ce qui est mesuré du chemin de débordement l'est
  **sous injection**.
- **Que le rétablissement d'une surveillance perdue serve un jour** : aucune
  cause naturelle de perte de handle n'a été observée, ni ici ni ailleurs dans ce
  dépôt.
- **La latence de bout en bout** — de la création du raccourci à son apparition
  **dans une page** —, qu'aucun sous-bloc du chantier D ni de ④ n'a jamais
  mesurée. G4 mesure la latence **jusqu'à une ligne de journal**.
- **Le coût de l'envoi complet périodique sur le fil** (D7) : le seul chiffre
  disponible est celui de G1, pour un catalogue plus petit et sans les champs
  d'icône.
- **La correction d'E5** : son unique preuve est un **argument de flot de
  contrôle**, et aucune tâche ne l'exerce.
- **Le comportement sous une racine réellement bruyante** : le relevé ⑥ mesure le
  repos de **cette** VM, pas celui d'un poste de travail utilisé.
- **Aucune constante calibrée** : `TAMPON_NOTIFICATIONS`, `DELAI_ANTI_REBOND`,
  `DELAI_ANTI_REBOND_MAX` (⚠️ **dérivée, ce qui n'est pas calibrée** — D6), le
  délai de 1 s de l'attente, `PERIODE_RECONCILIATION` que G4 **ne change pas**.
  Elles rejoignent `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`, `SEUIL_INJOIGNABLE_MS`,
  `DELAI_LANCEMENT_MS`, `FILE_EMISSION`.
- **Rien de la charge** : une VM, un corpus de 220 raccourcis, une racine
  bruyante à la fois.
- **Rien d'un client réel, aucune page de hub** : la recette est un journal
  d'agent et, au mieux, un `curl`.
- **Le leg n°1 de G1 n'est pas refermé** — et ⚠️ **il est aujourd'hui SANS EFFET
  sur G4** : depuis la correction du 20 août, le pont et les enfants n'ouvrent
  aucun canal, donc `apps::brancher` ne branche rien chez eux, donc **une seule
  surveillance existe par VM**. *Vérifié en lisant `apps.rs:64-82`, pas supposé.*
- **La divergence `403`/`404`** (E8) : signalée, non tranchée, **et hors du
  périmètre de G4**.
- **Aucune isolation entre utilisateurs sur les VMs non attribuées** : G4 hérite
  de cet état, ne l'aggrave pas et ne le répare pas.

---

## Risques, et ce qui rendrait G4 NON LIVRABLE

| Risque | Ce qu'il coûte | Mitigation, ou constat |
| --- | --- | --- |
| 🔴 **La rafale ne fait jamais déborder** | ② est NON MESURABLE | **écrit d'avance** (« LA PORTE »), et **il est interdit de rétrécir le tampon** pour le faire passer. G4 reste livrable : ②bis exerce le chemin de code, et le §« ce que l'injection n'établit pas » borne ce qu'on en tire |
| 🔴 **La ROUGE de ③ est VERTE** | le critère est tenu **et** non discriminant | **écrit d'avance** (E4), avec le mécanisme réel (E3) **et** le montage B qui, lui, peut être rouge |
| 🔴 **Le nettoyage de la rafale échoue** | le corpus de la VM — un actif de ④ tout entier — est abîmé | **la rafale n'est faite que de `.tmp`**, que le produit ignore par construction (D10). Et S1 **mesure le nettoyage avant** que le produit n'existe |
| 🔴 **Une racine est bruyante au repos** | la réconciliation passe de 1/30 s à 1/4 s : **7,5× le travail, 7,5× le trafic** | **le relevé ⑥ le mesure et PEUT échouer.** Si c'est le cas, c'est un résultat, et le remède — filtrer, ou allonger `DELAI_ANTI_REBOND_MAX` — est une décision qui se prend **sur la mesure** |
| 🔴 **La VM est prise par F4 ou par P3** | aucune recette | **la VM est un PRÉALABLE EXTERNE** : les familles 0 à 3 se jouent sans elle, et G4 s'arrête proprement à la tâche 10 |
| 🔴 **Une fonctionnalité de crate manque** | `cargo check` échoue en `unresolved import` | **M4 est une hypothèse relevée dans les bindings**, et G1 a payé exactement cette affirmation. **La tâche 7 compile avant de déclarer** |
| **`boucle.rs` franchit 450** | dette de taille sur un fichier déjà chargé | **extraction AVANT addition**, tâche 4, **jamais une compression** |
| **`build-agent.sh` emporte le travail d'un voisin** | un binaire qui ne compile pas, ou qui n'est pas le nôtre | **bâtir depuis un `git worktree`** au commit de G4, `node_modules` lié, et **vérifier la TAILLE du binaire** |
| **Deux chantiers montent `PLATEFORME_VERSION`** | une VM muette pendant toute une tâche | **sans objet : G4 ne la monte pas** (D1) |
| **La recette tourne sous un `RUST_LOG` trop étroit** | des zéros qui ne veulent rien dire | **vérifier que chaque trace lue PEUT sortir**, avant de lire son compte (G2) |

🔴 **Ce qui rendrait G4 NON LIVRABLE — et rien d'autre :**

1. **`cargo check --target x86_64-pc-windows-gnu` ne passe pas.** Le reste du
   sous-bloc n'a alors aucune valeur : rien ne tournera sur la VM.
2. **La surveillance fait perdre une application**, c'est-à-dire : un montage où
   le catalogue est **moins** complet avec la surveillance que sans. **C'est la
   seule chose que D1 interdit absolument**, et c'est le seul résultat qui
   obligerait à désarmer par défaut.
3. **Le relevé ⑥ montre que la surveillance multiplie le travail au repos**, sans
   qu'aucun réglage de `DELAI_ANTI_REBOND_MAX` ne le ramène à un coût acceptable.

**Tout le reste est mesurable, et son verdict s'écrit.** Un critère NON MESURABLE
n'est pas un échec du sous-bloc : c'est une mesure non prise, et elle se dit.

---

## Contrôle final, avant de déclarer G4 clos

🔴 **DEPUIS UN SHELL PROPRE, ou `env -u TURN_URL -u TURN_SECRET`** — sans quoi
six tests de signaling échouent pour une raison étrangère à G4 (leg n°5 de G1).
⚠️ **JE N'AI PAS REJOUÉ CE LEG** : mes deux passes de `plateforme` ont été
lancées avec `env -u TURN_URL -u TURN_SECRET`, donc **dans les conditions où le
défaut ne se manifeste pas**. Sa persistance est **reprise de `CLAUDE.md`, non
mesurée par ce plan** — et la précaution est prise de toute façon, parce qu'elle
ne coûte rien.

```bash
unset -f chpwd
scripts/verify-all.sh            # DIX appels `etape` — dire lequel on compte
cd agent && cargo check --target x86_64-pc-windows-gnu
```

Puis, **et seulement ensuite** :

- [ ] `cargo test --workspace` — le compte **annoncé avant d'être mesuré**, et
      **assorti de son arbre** (départ : agent **916**, proto **109**) ;
- [ ] `cargo clippy --workspace` — ⚠️ **vérifier la NATURE des avertissements,
      jamais leur NOMBRE**, qui dérive avec la fraîcheur du build ;
- [ ] `cd proto && npx vitest run && npm run typecheck` — **296 attendu,
      inchangé** : G4 ne touche pas `proto/` ;
- [ ] `cd plateforme && npm run test:sqlite && npm run test:postgres && npm run typecheck`
      — **564 attendu, inchangé** ; ⚠️ **un saut est un échec** ;
- [ ] `cd client && npm test && npm run typecheck` — **inchangé** ;
      ⚠️ **le compte de DÉPART n'a PAS été relevé par ce plan** : `client/` n'est
      touché par aucune de ses tâches, et je n'ai pas lancé la commande. **Le
      relever À L'ENTRÉE de la recette**, sans quoi « inchangé » n'est comparable
      à rien ;
- [ ] `cargo check --target x86_64-pc-windows-gnu` — **sortie 0**, et **tous** les
      avertissements de la famille `dead_code`, vérifié **par filtrage** ;
- [ ] la commande des tailles de `CLAUDE.md`, **relancée APRÈS la dernière
      édition de la ronde**, et **le commit nommé** ;
- [ ] `git diff --name-only <base>..HEAD` — **aucun fichier sous `proto/`,
      `plateforme/`, `client/`, `agent/src/main.rs`, `agent/src/capteur/`,
      `agent/src/superviseur/`, `agent/src/pont/`, `src/`, `web/`** ;
- [ ] `git log --numstat -- scripts/run-agent.sh` — **vérifier que les lignes de
      G4 sont bien de G4**, et non de F4 ;
- [ ] les journaux de recette **versés**, avec leur tableau de familles de
      lecture **relevé par la commande** ;
- [ ] `git show --stat` après **chaque** commit — **pathspec explicite, jamais
      `git add -A`, jamais `--amend`.**
