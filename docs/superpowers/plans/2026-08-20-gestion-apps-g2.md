# Sous-bloc G2 — les icônes 256, et la preuve que c'en est : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal :** donner à chaque application du catalogue de G1 une icône **256×256
avec son canal alpha**, et — c'est tout l'objet du sous-bloc — **la preuve que
c'en est une, et non un agrandissement**. Cette preuve ne peut pas venir de
l'image rendue : elle vient de la **ressource** (le `GRPICONDIR` d'un module PE,
l'`ICONDIR` d'un `.ico`), et elle admet une troisième valeur, `NonMesuree`, que
rien n'a le droit de confondre avec 256. Les octets sont **adressés par leur
contenu** : la plateforme ne redemande jamais une icône qu'elle possède déjà.

**Architecture :** quatre étages, et la coupure pur / `#[cfg(windows)]` est
décidée par la spec §6, pas à l'implémentation.

1. **Deux modules PURS dans l'agent.** La lecture d'un `GRPICONDIR` / `ICONDIR`
   depuis un `&[u8]` (spec §6 : « le point le plus important de cette liste »),
   et le magasin en mémoire adressé par contenu. Tous deux se testent sur
   l'hôte Linux, contre **deux fichiers `.ico` témoins versés en `testdata`**.
2. **Trois modules `#[cfg(windows)]`.** L'extraction par
   `IShellItemImageFactory::GetImage`, l'encodage PNG **par WIC** (qui préserve
   l'alpha, là où `Image::FromHbitmap` le perd), et l'obtention des octets de
   ressource par `LoadLibraryExW` / `FindResourceW`.
3. **Le canal `/agent` gagne un aller-retour d'inventaire** —
   `IconesManquantes`, poussé par la plateforme après chaque `Catalogue` — et
   **les octets ne l'empruntent jamais** : ils passent par HTTP, comme les
   installeurs de G3 (spec D7).
4. **La plateforme gagne un magasin de disque adressé par contenu**, deux
   colonnes sur `application`, et deux routes HTTP.

**Tech Stack :** inchangée. 🔴 **G2 N'AJOUTE AUCUNE DÉPENDANCE de production**,
ni en TypeScript ni en Rust. Le témoin de cette propriété est
`plateforme/src/base/pilote.test.ts:48-49` (`expect(deps).toEqual(['pg','ws'])`,
relu) et il doit rester **vert**. Côté Rust, la seule addition à
`agent/Cargo.toml` est **une fonctionnalité du crate `windows` déjà présent** —
même figure que `Win32_System_Environment` pour G1 et `Win32_System_DataExchange`
pour P1 (presse-papier).

**Spec :** `docs/superpowers/specs/2026-08-19-gestion-apps-design.md`
(commit `dded5b5`), §3.1, §3.2, §4 (D5, D10), §5 « G2 », §6, §7, §8, §11.
**Amont qui fait autorité sur l'état du code :**
`docs/superpowers/plans/2026-08-19-gestion-apps-g1.md`, son document de
résultats, la section G1 de `CLAUDE.md`, **et le code lui-même** — l'arbre a
bougé depuis la clôture de G1, et deux de ses trois legs 🔴 sont fermés (voir
E5 et E6).

**Ce plan ne couvre QUE G2.** Rien du téléversement d'installeur ni de son
exécution (G3) : aucune route `/televersement`, aucun `Installer`,
`Progression` ni `Termine`, aucune table `installation`, aucune sonde UAC.
Rien de `ReadDirectoryChangesW` ni de l'anti-rebond (G4). Rien du manifeste
PWA par application, des `file_handlers` ni des types installeur du hub (G5).
**Aucune page de hub** — comme G1 (sa décision D12), la recette exerce les
routes par `curl`, ce qui les éprouve davantage qu'une page.

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **20 août 2026**, sur cette
machine, en lançant réellement la commande. **Aucune n'est recopiée d'un
document.** Elles **DÉRIVERONT** : la seule source de vérité est la commande.

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>450'
```

| Fichier | Lignes | Ce que G2 en fait |
| --- | --- | --- |
| `agent/src/encode.rs` | **1536** | dette gelée, non touchée |
| `agent/src/windows_source.rs` | **630** | dette gelée, non touchée |
| 🔴 `proto/src/plateforme/tests.rs` | **561** | **G2 LA RÉSORBE** — voir D2 |
| 🔴 `proto/ts/plateforme.test.ts` | **512** | **G2 LA RÉSORBE** — voir D2 |
| `plateforme/src/http/routes-applications.test.ts` | **480** (marge **20**) | 🔴 **extraction AVANT addition** — voir E10 |
| `proto/src/plateforme.rs` | **433** (marge 67) | +1 enum, +1 variante, +2 champs |
| `agent/src/plateforme.rs` | **453** (marge 47) | +1 branche descendante |
| `proto/ts/plateforme.ts` | **429** (marge 71) | le miroir |
| `plateforme/src/agents/canal.ts` | **423** (marge 77) | +1 branche |
| `plateforme/src/http/serveur.ts` | **371** | +1 routeur chaîné |
| `plateforme/src/http/routes-applications.ts` | **315** | +2 champs rendus |
| `agent/src/apps/lecture.rs` | **281** | inchangé |
| `agent/src/apps/boucle.rs` | **213** | le câblage, et le legs n°7 de G1 |
| `plateforme/src/depot/application.ts` | **186** | +2 colonnes |
| `plateforme/src/config.ts` | **183** | `PLATEFORME_ICONES` |
| `agent/src/apps/sha256.rs` | **168** | **réemployé tel quel**, aucune addition |
| `agent/src/apps/raccourci.rs` | **159** | inchangé |
| `agent/src/apps.rs` | **156** | +1 `mod` |
| `plateforme/src/apps/catalogue.ts` | **127** | +2 champs |
| `scripts/run-agent.sh` | **145** | 🔴 **tâche dédiée** — voir D14 |

Autres relevés par la commande, le même jour :

- `windows` est verrouillé à **0.62.2** dans `Cargo.lock` (l. 1498-1500), et
  c'est **cette version-là** dont les symboles sont cités plus bas. *(La leçon
  est celle d'un agent qui a déclaré fausses trois citations exactes parce
  qu'il lisait deux modules homonymes d'une autre version : le chemin complet
  du crate lu est
  `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/windows-0.62.2/`.)*
- `agent/Cargo.toml` porte **38 fonctionnalités** `Win32_*`. **`Win32_System_LibraryLoader`
  y est** (l. 115). **`Win32_UI_Shell` n'y figure pas nommément** et n'en a pas
  besoin : `Win32_UI_Shell_PropertiesSystem` (l. 139) l'entraîne, et c'est ce
  qui fait déjà compiler `agent/src/apps/lecture.rs`.
  **`Win32_Graphics_Imaging` est ABSENTE** — c'est l'unique addition de G2 à ce
  fichier. ⚠️ **Candidate, à CONFIRMER par la compilation, pas à affirmer** :
  voir E11.
- `plateforme/src/base/migrations/` porte **quatre** migrations
  (`0001-socle`, `0002-identite`, `0003-agents`, `0004-applications`), et
  `plateforme/src/base/pilotes.test.ts:33` fige
  `expect(suivi.map((l) => Number(l.version))).toEqual([1, 2, 3, 4])`.
- `scripts/verify-all.sh` appelle `etape` **dix** fois (l. 67, 70, 82, 85, 95,
  98, 101, 104, 107, 110). ⚠️ **Une exécution complète en affiche DIX-SEPT** —
  les sept de plus viennent de l'intérieur de `client : npm run design:verifier`.
  **Dire lequel on compte** (piège déjà payé en P3 et P4).

### 🔴 Les mesures prises SUR LA VM pour ce plan, transcrites VERBATIM

**Méthode** : trois `.ps1` écrits sur `/media/vm/dev/` et invoqués par
`powershell -ExecutionPolicy Bypass -File C:\dev\<nom>.ps1`, **jamais en ligne
de commande** — `nodejs-winrm` enveloppe la commande dans
`powershell -Command "& { … }"` et un guillemet interne y entre en collision
(piège D3). Chaque sonde écrit dans un fichier UTF-8 relu depuis l'hôte : **le
pipeline d'encodage PowerShell n'est jamais sur le chemin de la mesure.**

⚠️ **UNE EXÉCUTION CHACUNE, sur une seule VM, sur un seul corpus, le 20 août
2026. AUCUN TAUX N'EST REVENDIQUÉ.** Les sondes vivent dans `C:\dev\`,
c'est-à-dire nulle part de durable : **ce document est leur seule trace**, comme
le §3 de la spec l'est des siennes.

#### M1 — 🔴 Le corpus réel porte les DEUX valeurs, et de très loin

Sonde `C:\dev\g2plan-icones.ps1` : les quatre racines, les trois règles de
filtrage de D3, le triplet d'identité, puis pour chaque source d'icône
distincte `LoadLibraryExW(LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE)`,
`EnumResourceNamesW(RT_GROUP_ICON)`, et lecture du `GRPICONDIR` du **premier**
groupe — `bWidth == 0` valant 256. Verbatim :

```
total_lnk=218 retenus=167 cles=153
iconlocation_sans_chemin=92 sources_distinctes=110
lecture_grpicondir_ms=3012 pour 110 sources
--- histogramme source_max_px, pondere par application ---
  128 : 2
  16 : 16
  256 : 61
  32 : 7
  48 : 27
  64 : 2
  96 : 1
  inconnue : 35
  source-absente : 2
```

**61 applications à 256, 55 en dessous, 37 hors de portée d'une lecture de
ressource.** Le risque nommé au §11 de la spec — « le corpus réel ne porte pas
les deux valeurs, le critère ② ne peut pas être exercé contre le produit » —
**est LEVÉ par la mesure**, et le critère ③ **est mesurable** au lieu d'être
`NON MESURABLE`.

⚠️ **Portée exacte, et elle est étroite.** Cette sonde **n'est pas le produit** :
elle prend la source d'icône du `.lnk` (`IconLocation`, ou la cible à défaut),
là où le produit interrogera le Shell ; elle ne lit que le **premier** groupe
d'icônes énuméré, là où le produit devra choisir celui que le Shell choisit ; et
`inconnue` y recouvre indistinctement les `.ico` autonomes, les chemins
d'installeur MSI et les modules qu'`EnumResourceNamesW` n'a rendus vides.
**Les nombres ci-dessus ne sont donc PAS le résultat attendu du produit** : ils
établissent une chose, une seule, et elle suffit — **le corpus n'est pas
uniforme**.

⚠️ **`cles=153`, LÀ OÙ LA SPEC ET LA RECETTE DE G1 RENDENT `154`.** Les deux
autres comptes concordent à l'unité (218 lus, 167 retenus). **L'écart d'une
unité n'est PAS expliqué**, et deux causes sont candidates sans qu'aucune ne
soit vérifiée : (a) l'état de la VM a bougé depuis le 20 août au matin — la
recette de G1 y a créé puis retiré des `.lnk` ; (b) la clé de cette sonde n'est
pas celle du produit — elle replie la casse sans `trim` ni retrait de barre
finale, et surtout elle emploie `WScript.Shell.TargetPath`, qui **résout**, là
où le produit emploie `IShellLinkW::GetPath(SLGP_RAWPATH)`, qui ne résout pas.
**C'est exactement la réserve que le legs n°6 de G1 laisse ouverte** — « les
champs par entrée du corpus restent ceux de `WScript.Shell` » — et cette mesure
en est un indice de plus, pas une réfutation. **Aucune décision de ce plan ne
dépend de 153 plutôt que de 154.**

#### M2 — 🔴 Le témoin de la spec §3.2, REPRODUIT sur des fichiers fabriqués depuis l'hôte

Deux `.ico` construits par un script Python de trente lignes **sur l'hôte
Linux** — l'un ne contenant qu'une entrée 48×48, l'autre qu'une entrée 256×256 —
puis interrogés sur la VM par `IShellItemImageFactory::GetImage(256,…)`, par
`PrivateExtractIconsW`, et par une lecture de leur `ICONDIR`. Verbatim :

```
=== C:\dev\g2plan-temoin-48.ico
   ICONDIR (la RESSOURCE)          -> entrees=1 tailles=48
   ShellImageFactory 256 ICONONLY  -> 256x256 32bpp
   ShellImageFactory 256 +BIGGEROK -> 256x256 32bpp
   PrivateExtractIcons idx0 256    -> 256x256 32bpp
   PrivateExtractIcons idx0 48     -> 48x48 32bpp
=== C:\dev\g2plan-temoin-256.ico
   ICONDIR (la RESSOURCE)          -> entrees=1 tailles=256
   ShellImageFactory 256 ICONONLY  -> 256x256 32bpp
   ShellImageFactory 256 +BIGGEROK -> 256x256 32bpp
   PrivateExtractIcons idx0 256    -> 256x256 32bpp
   PrivateExtractIcons idx0 48     -> 48x48 32bpp
```

🔴 **LES QUATRE LIGNES DE RENDU SONT IDENTIQUES ENTRE LES DEUX FICHIERS. SEULE
LA LIGNE `ICONDIR` DIFFÈRE.** Un fichier qui ne contient QUE du 48×48,
interrogé à 256, rend 256×256 32bpp — par les deux API, sans
`SIIGBF_SCALEUP`, et **y compris avec `SIIGBF_BIGGERSIZEOK`**, c'est-à-dire en
disant explicitement au Shell qu'une taille plus grande serait acceptée.

**C'est le fait qui gouverne tout le sous-bloc, et il est vérifié deux fois** :
par la spec le 19 août, et ici le 20 août sur des témoins **fabriqués par le
dépôt lui-même**, donc reproductibles sans la VM d'origine. **Un critère de
réception qui compare la taille rendue à 256 NE PEUT PAS ÉCHOUER.**

#### M3 — 🔴 L'extraction réelle : 153/153, alpha, déduplication et coût

Sonde `C:\dev\g2plan-dedup2.ps1` : sur les 153 raccourcis retenus,
`SHCreateItemFromParsingName` sur **le `.lnk`**,
`IShellItemImageFactory::GetImage({256,256}, SIIGBF_ICONONLY)`, puis **lecture
directe des bits du DIB** (et non `Image::FromHbitmap`, qui perd l'alpha),
encodage PNG, SHA-256. Verbatim :

```
applications=153
extraction+png_ms=2298 echecs=0
png_distincts=99 octets_sans_dedup=4576398
alpha_non_trivial=149
--- dimensions rendues ---
  256x256 : 153
--- empreintes partagees ---
  05CC39B59275E9AD x7
  148B2C6DAEDF8508 x4
  1C27DB62377F92D0 x2
  5A080EC53C0A01C5 x4
  6E54F5C10B49429A x27
  73638076EA34DD5F x2
  77852FCF6AC08A80 x3
  A5E30757ABC07B66 x2
  BDACA164D6243DB4 x2
  E1F3E34FA2E7E700 x2
  F3AD4E5D5ABF6C17 x4
  FB8D89B730BEC520 x7
empreintes_partagees=12 televersements_evites=54
```

Ce que cela établit, et **rien de plus** :

| Fait | Valeur | Ce qu'il impose au plan |
| --- | --- | --- |
| échecs d'extraction | **0 sur 153** | l'étage `IShellItemImageFactory` n'est pas une inconnue : il est mesuré |
| dimensions rendues | **256×256, 153 fois sur 153** | le critère ① ne vaut **rien** sur la taille (M2) |
| **déduplication par contenu** | **99 PNG distincts pour 153 applications**, soit **54 téléversements évités (35,3 %)** | 🔵 **c'est la mesure que la spec réclamait** — l'adressage par contenu paie, sur le corpus réel |
| la plus grosse collision | **×27** — un même exécutable visé par vingt-sept raccourcis d'arguments différents (`runcmdu.exe` de `smartmontools`, §3.1 de la spec) | D4 de la spec sépare bien ces 27 **applications** ; elles partagent **une** icône |
| 🔴 alpha non trivial | **149 sur 153** — donc **4 N'EN ONT PAS** | 🔴 **le critère ① NE PEUT PAS exiger l'alpha de TOUTES les icônes** : cet état n'est pas atteignable, il est mesuré faux |
| poids total non dédupliqué | **4 576 398 octets** (29 911 o/icône) | proche des ≈ 5,2 Mo estimés par la spec §4 D5, et cette fois **avec** l'alpha |
| coût | **2 298 ms pour 153 icônes**, soit **15,0 ms/icône** | à opposer aux 28,7 ms/icône de la spec §3.1, dont le chemin perdait l'alpha |

⚠️ **Le poids DÉDUPLIQUÉ n'a pas été mesuré** : la sonde somme les 153 PNG, pas
les 99 distincts. Ne pas le déduire d'une règle de trois — les icônes n'ont pas
la même taille.

⚠️ **Cette sonde emploie GDI+ (.NET), pas WIC**, qui est ce que l'agent
emploiera (D3). Les 99 empreintes distinctes prouvent que **GDI+ est
déterministe à l'intérieur d'une exécution** — sans quoi les douze collisions
n'auraient pas eu lieu. **Elles ne disent rien de WIC, ni d'une exécution à
l'autre.** C'est l'objet de la tâche 10.

⚠️ **Une première rédaction de cette sonde a rendu `echecs=153`,
`png_distincts=0` et `octets=0` — TROIS ZÉROS SUR UNE MACHINE SAINE.** La cause
n'était pas le produit : l'interface COM traversait PowerShell en
`System.__ComObject`, et l'appel `GetImage` n'existait pas sur cet objet. **Le
verdict a été refusé et l'exception relevée**, ce qui a désigné l'instrument.
C'est le piège que ce plan écrit en tête de sa section de méthode : **un verdict
négatif exige que la chose mesurée soit ABSENTE, jamais seulement NULLE.**

### La règle des 500 lignes : TROIS extractions, toutes AVANT leur addition

**Aucune compression, sans exception** — la doctrine du dépôt est écrite en tête
de `CLAUDE.md` et a été repayée en D9 (`sommeil.rs` ramené à 499 par compression,
puis extrait sur exigence de revue).

| # | Fichier | Lignes | Pourquoi G2 doit l'extraire |
| --- | --- | --- | --- |
| 1 | `proto/src/plateforme/tests.rs` | **561** | 🔴 **DETTE INSCRITE**, née de G1. G2 y ajoute les cas de `SourceMax`, des deux champs et d'`IconesManquantes` |
| 2 | `proto/ts/plateforme.test.ts` | **512** | 🔴 idem, côté miroir |
| 3 | `plateforme/src/http/routes-applications.test.ts` | **480**, marge **20** | G2 y ajoute une famille entière (la route d'icône) : il franchira |

🔴 **LES DEUX PREMIÈRES SONT LA DETTE QUE `CLAUDE.md` INSCRIT SANS POINT DE
CHUTE**, et la règle qui décide est écrite au même endroit : *« le découpage
rétroactif des fichiers se fait au moment où l'on travaille dedans, pas en
chantier séparé »*. **G2 travaille dedans.** Le sous-bloc P1 du presse-papier
les a écrites en déclarant explicitement n'y pas toucher — « ils appartiennent à
un autre chantier » —, et **ce chantier est celui-ci**. Leur point de chute est
nommé en D2.

⚠️ **Les trois extractions ne portent AUCUNE ligne de comportement neuve**, et
chacune se vérifie de la même façon : le compte de tests avant et après est
**identique**, annoncé avant d'être mesuré. *(La leçon est de D10 : un
implémenteur a écrasé un fichier de tests et supprimé un test antérieur — il
l'a vu parce que le compte est sorti à 452 au lieu des 453 annoncés d'avance.)*

### Les règles de méthode, héritées et non négociables

1. **Aucune pièce fabriquée.** Toute sortie de commande citée dans un rapport
   est **relancée**, jamais reconstituée de mémoire. D10 a trouvé deux pièces
   fabriquées dont les **faits étaient vrais** : c'est le mode de défaillance à
   surveiller, parce qu'il produit une conclusion juste **sans preuve**, donc
   invérifiable par le suivant.
2. **Un contrôle qu'on n'a jamais vu ROUGE n'est pas un contrôle.** Pour chaque
   contrôle prescrit ci-dessous, la colonne « ce qui le rend ROUGE » nomme
   l'état, **et cet état est atteignable** — quand il ne l'est pas, c'est dit.
3. 🔴 **Un verdict NÉGATIF exige que la chose mesurée soit ABSENTE, pas
   seulement nulle.** Trois zéros sur une machine saine ne sont pas un verdict
   (voir M3). Toute sonde de ce plan doit d'abord établir qu'elle **sait
   observer** ce qu'elle cherche.
4. **`fichier:ligne` relu après avoir été écrit, et l'ENTRÉE nommée**, pas
   seulement la ligne.
5. **Deux exécutions par critère, jamais une.** Règle du sous-projet ⑤ et du
   chantier D. **Aucun taux ne sera revendiqué.**
6. **Toute variable neuve de l'agent passe par `scripts/run-agent.sh`, dans une
   TÂCHE DÉDIÉE.** Piège payé **cinq fois** : `SUPERVISEUR` (D1),
   `MULTIFENETRE_REPRISE` (D2), `AUDIO` (D7), et évité par une tâche dédiée en
   D6 (`BUDGET_BPS`) et en G1 (`APPS`).
7. **Nommer les fichiers dans `git add`, jamais `git add -A`** : l'arbre est
   partagé avec quatre chantiers actifs.
8. 🔴 **`git checkout` NE RESTAURE PAS un fichier non suivi et EFFACE un fichier
   suivi non commité.** Un agent y a perdu une implémentation entière. Toute
   mutation jouée pour voir une rouge se défait par une **copie de sauvegarde
   nommée**, jamais par `git checkout`.
9. **`git commit` valide TOUT L'INDEX, `-F message` compris.** Pathspec
   explicite obligatoire, et `git show --stat` après coup. **Jamais
   `--amend`.**
10. ⚠️ **`scripts/verify-all.sh` n'est PAS hermétique** (legs n°5 de G1) : avec
    `TURN_URL`/`TURN_SECRET` dans l'environnement — c'est-à-dire après le
    `set -a && source .env` que tout travail sur la VM exige — **six** tests de
    `src/signaling/server.test.ts` échouent pour une raison étrangère.
    **Le lancer depuis un shell propre, ou `env -u TURN_URL -u TURN_SECRET`.**
11. ⚠️ **`docker compose config` imprime les secrets en clair**, et `${VAR:?}`
    engage **toutes** les sous-commandes. Aucune tâche de ce plan n'en a
    besoin ; c'est écrit pour qu'aucune ne l'invente.

### Périmètre concurrent — à lire avant de toucher `proto/`, `agent/` ou la VM

Relevé par `git log --oneline -12 -- proto/` le 20 août 2026 : **quatre
chantiers ont commité dans l'arbre le même jour** (presse-papier P1/P2,
pont fichiers F1/F2, micro E2, design S4).

| Fichier | Qui d'autre | Ce que G2 fait |
| --- | --- | --- |
| `proto/src/control.rs`, `proto/src/fichiers.rs` et leurs miroirs | presse-papier, pont fichiers | 🔵 **G2 N'Y TOUCHE PAS** — `plateforme.rs` est le module de ④, et il est à lui |
| `agent/src/main.rs` | tous | 🔵 **G2 N'Y TOUCHE PAS** : `ICONES` est lue par `OnceLock` dans `apps/icone.rs`, comme `APPS` l'est dans `apps.rs`. Aucun appel neuf n'est ajouté à `main.rs` |
| `plateforme/src/http/serveur.ts` | P5 (en-têtes), P4 (routes VM) | **une ligne de chaînage**, sur le modèle exact de `servirApplications` (l. 223) |
| `plateforme/src/config.ts` | P5 | **un champ**, `PLATEFORME_ICONES` |
| `scripts/run-agent.sh` | micro E2, presse-papier | **une ligne**, dans une tâche dédiée |
| 🔴 **la VM Windows** | **un agent concurrent y joue une porte éliminatoire** | **une seule tâche de G2 l'emploie** (la recette), et elle vérifie `Get-Process agent` avant et après |

⚠️ **La VM était ÉTEINTE au moment d'écrire ce plan** (`virsh list --all` →
`fermé`) ; elle a été démarrée pour prendre M1, M2 et M3, et les trois sondes
sont **en lecture seule** — elles n'écrivent que dans `C:\dev\`, ne créent aucun
`.lnk`, ne touchent ni au registre ni au mode d'affichage, et ne lancent aucun
agent. **Les cinq fichiers déposés (`g2plan-*.ps1`, `g2plan-*.txt`,
`g2plan-temoin-*.ico`) sont inertes et y restent.**

---

## Décisions tranchées

### D1 — 🔴 `PLATEFORME_VERSION` passe à **3**, et G2 IMPOSE un redéploiement conjoint

**La question est posée par le brief, et la réponse se prouve en regardant qui
vérifie quoi, des deux côtés.**

G2 change le fil de trois façons : `Application` gagne deux champs obligatoires,
`DepuisLaPlateforme` gagne la variante `IconesManquantes`, et `VersLaPlateforme`
n'en gagne aucune (les octets passent par HTTP, D8). **Chacune de ces trois
choses est une rupture**, et voici pourquoi :

| Qui vérifie | Où | Ce qu'il fait d'un message de G2 reçu par un pair de G1 |
| --- | --- | --- |
| `verifie_version` | `proto/src/plateforme.rs:103-114` | rejette tout `v != PLATEFORME_VERSION`. **C'est la seule barrière qui rende le refus TYPÉ** |
| `#[serde(deny_unknown_fields)]` sur `Application` | `proto/src/plateforme.rs:196` | un `icone` de trop est **refusé** ; un `icone` manquant l'est aussi (aucun `default`) |
| `#[serde(tag = "type", …, deny_unknown_fields)]` sur les deux enums | `proto/src/plateforme.rs:258` et `:337` | `{"type":"icones-manquantes"}` est une variante **inconnue** |
| côté TypeScript | `proto/ts/plateforme.ts:300` et `:425` | `if (parsed.v !== PLATEFORME_VERSION) return { ok: false, motif: 'version' }` |

🔴 **Et voici ce que coûterait de NE PAS monter la version.** Sur un couple
mal apparié, la désérialisation échouerait — champ inconnu, champ manquant, ou
variante inconnue — et le refus émis porterait le motif **`forme`**, pas
`version`. Or `agent/src/plateforme.rs::sur_refus` (l. 408-443, relu) rend
`Fin::Definitive` **pour le seul `MotifCanal::Version`** (l. 410-419) et
`Fin::Reprenable` **pour tous les autres** (l. 420-423). **Une incompatibilité
de format se déguiserait donc en boucle de reconnexion sans terme** — très
exactement le mode de panne que la recette de G1 a mesuré (10 reprises jusqu'au
palier de 30 s), et que le protocole nomme lui-même « le plus coûteux à
diagnostiquer ».

**Décision : `PLATEFORME_VERSION = 3`, et agent et plateforme se déploient AU
MÊME COMMIT.** C'est la reconduite explicite de D5 de P3 et de D10 de G1 ; la
reprendre sans le dire serait la subir.

✅ **Une chose a changé depuis G1, et elle est en notre faveur** : le refus est
désormais **hors versionnement** (correction du 20 août 2026, commit `457a7f8`,
trois clauses en tête de `proto/src/plateforme.rs`, l. 49-70). Un agent v2 face
à une plateforme v3 **lira** son refus, journalisera
`la plateforme REFUSE la version du canal /agent : aucune reprise, il faut
rebâtir l'agent ou la plateforme` avec `version_emise=2 version_recue=3`, et
**renoncera**. Au moment de G1 il aurait bouclé. **La rupture reste une
rupture ; elle est seulement devenue DIAGNOSTICABLE**, et le bump de G2 est le
premier à en bénéficier — donc le premier à pouvoir le PROUVER (critère ⑧).

### D2 — 🔴 Les deux dettes de `proto/` sont RÉSORBÉES par G2, et leur point de chute est nommé

`CLAUDE.md` inscrit `proto/src/plateforme/tests.rs` (**561**) et
`proto/ts/plateforme.test.ts` (**512**) au tableau de dette **sans point de
chute**, avec cette phrase : « c'est au chantier qui les rouvrira de le
choisir ». **G2 les rouvre.**

**Le point de chute suit la coupure que le protocole porte déjà** — le cycle de
vie d'un côté, la gestion d'apps de l'autre :

| Source | Devient | Ce qu'il porte |
| --- | --- | --- |
| `proto/src/plateforme/tests.rs` (561) | `proto/src/plateforme/tests.rs` | version, refus, enrôlement, battement — le **cycle de vie** |
| | `proto/src/plateforme/tests_apps.rs` (neuf) | `Application`, `Catalogue`, `Lancer`/`Lancee`, et **tout ce que G2 ajoute** |
| `proto/ts/plateforme.test.ts` (512) | `proto/ts/plateforme.test.ts` | idem |
| | `proto/ts/plateforme-apps.test.ts` (neuf) | idem |

⚠️ **La déclaration se fait par `#[path]` DEPUIS `plateforme.rs`**, exactement
comme le `#[cfg(test)] #[path = "plateforme/tests.rs"] mod tests;` existant
(`proto/src/plateforme.rs:431-433`). Ce n'est **pas** la « Convention de module
enfant » de `CLAUDE.md`, qui vise les modules extraits d'un parent
`#[cfg(windows)]` : c'est le même mécanisme Rust employé pour l'autre raison —
la règle des 500 lignes —, et `CLAUDE.md` le nomme explicitement hors de portée
de cette convention (`superviseur/table.rs` en a deux précédents).

### D3 — L'extraction vient du Shell sur le `.lnk`, et le PNG est encodé par **WIC**

**Extraction** : `SHCreateItemFromParsingName` sur le **`.lnk` lui-même**, puis
`IShellItemImageFactory::GetImage({256,256}, SIIGBF_ICONONLY)` — spec D5, et
mesuré **153 fois sur 153 sans échec** (M3). Sur le `.lnk` et non sur la cible,
parce que **92 des 153 raccourcis retenus portent un `IconLocation` sans
chemin** (M1 ; la spec relève 135 sur 218) : leur icône est celle de la cible, et
les autres portent une icône **propre au raccourci**. Seul le Shell connaît
toute cette chaîne.

🔴 **L'ENCODAGE PNG PASSE PAR WIC, ET SÛREMENT PAS PAR UNE CONVERSION QUI PERD
L'ALPHA.** La spec §3.1 relève que sa propre sonde de coût passait par
`Image::FromHbitmap`, **qui perd le canal alpha** — et c'est précisément ce que
le critère ① existe pour attraper. Les symboles employés, **relus dans
`windows-0.62.2`** :

| Symbole | Où (windows-0.62.2) |
| --- | --- |
| `SHCreateItemFromParsingName<P0, P1, T> -> Result<T>` | `src/Windows/Win32/UI/Shell/mod.rs:2678` |
| `IShellItemImageFactory::GetImage(&self, size: SIZE, flags: SIIGBF) -> Result<HBITMAP>` | `src/Windows/Win32/UI/Shell/mod.rs:38666` |
| `SIIGBF_ICONONLY = SIIGBF(4i32)` | `src/Windows/Win32/UI/Shell/mod.rs:55214` |
| `SIIGBF_BIGGERSIZEOK = SIIGBF(1i32)` | `:55211` |
| `SIIGBF_SCALEUP = SIIGBF(256i32)` | `:55218` |
| `CLSID_WICImagingFactory` | `src/Windows/Win32/Graphics/Imaging/mod.rs:143` |
| `IWICImagingFactory` | `:3458` |
| 🔵 `IWICImagingFactory::CreateBitmapFromHBITMAP(hbitmap, hpalette, options: WICBitmapAlphaChannelOption)` | `:3584` |
| `WICBitmapUseAlpha = WICBitmapAlphaChannelOption(0i32)` | `:6265` |
| `IWICImagingFactory::CreateEncoder(guidcontainerformat, pguidvendor)` | `:3498` |
| `IWICImagingFactory::CreateStream()` | `:3535` |
| `GUID_ContainerFormatPng` | `:214` |
| garde de fonctionnalité `Win32_Graphics_Imaging` | `src/Windows/Win32/Graphics/mod.rs:39` |

🔵 **`CreateBitmapFromHBITMAP` prend un `WICBitmapAlphaChannelOption`, et c'est
là que tout se joue** : `WICBitmapUseAlpha` conserve le canal, `WICBitmapIgnoreAlpha`
(`:6190`) le jette. La ligne qui décide du critère ① est donc **une constante
nommée**, ce qui rend sa mutation triviale à jouer.

⚠️ **`SIIGBF_SCALEUP` n'est PAS employé, et `SIIGBF_BIGGERSIZEOK` non plus.** Ni
l'un ni l'autre ne change quoi que ce soit (M2 : le témoin 48 rend 256×256 dans
les deux cas), et les employer suggérerait à tort que le drapeau protège de
l'agrandissement. **Le seul drapeau est `SIIGBF_ICONONLY`**, comme la spec
l'écrit.

### D4 — 🔴 La preuve vient de la RESSOURCE, et le module qui la lit est PUR

**C'est le point que la spec §6 désigne comme « le plus important de cette
liste », et ce plan le reprend sans l'affaiblir.**

`agent/src/apps/icone/ressource.rs` prend un `&[u8]` et rend une liste de
tailles. Il ne connaît ni Windows, ni COM, ni le système de fichiers. Il lit
deux formats **de même encodage** :

| Format | Où on l'obtient | Structure |
| --- | --- | --- |
| `GRPICONDIR` (module PE) | `LoadLibraryExW` + `FindResourceW(RT_GROUP_ICON)` + `LockResource` | `WORD reserved, WORD type, WORD count`, puis `count` × 14 octets |
| `ICONDIR` (`.ico` autonome) | les six premiers octets du fichier | idem, puis `count` × **16** octets |

🔴 **`bWidth == 0` VAUT 256, et c'est le seul piège de l'analyse** : le champ
fait un octet, et 256 n'y tient pas. Un lecteur qui rendrait `0` ferait dire à
une icône 256 qu'elle est de taille nulle — et un `max()` la classerait sous
n'importe quelle autre entrée.

⚠️ **Les deux structures diffèrent par la TAILLE DE LEUR ENTRÉE — 14 octets pour
un `GRPICONDIR`, 16 pour un `ICONDIR`** — parce que la première finit par un
`WORD nID` (l'identifiant de la ressource `RT_ICON`) et la seconde par un
`DWORD dwImageOffset`. **Les six premiers octets sont identiques**, ce qui rend
les deux formats indiscernables sans le savoir. Le module expose donc **deux
fonctions distinctes**, jamais une avec un drapeau.

**Ce qui reste `#[cfg(windows)]` est l'OBTENTION de ces octets**, et elle seule :
`LoadLibraryExW(LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE)`,
`EnumResourceNamesW`, `FindResourceW`, `SizeofResource`, `LoadResource`,
`LockResource` — **tous dans `Win32_System_LibraryLoader`, déjà présente**
(relus dans `windows-0.62.2/src/Windows/Win32/System/LibraryLoader/mod.rs`,
respectivement l. 259, 103, 160, 340, et `LockResource` l. 300).

**Sans cette coupure, le critère ② n'aurait aucun test d'hôte, et il serait dans
la situation exacte de F1 de D7 : un argument de flot de contrôle en guise de
preuve.**

### D5 — 🔴 `SourceMax` est un enum à DEUX variantes, dont une à DEUX MOTS

**Le type, décidé ici :**

```rust
/// D'où vient l'image : la plus grande entrée réellement PRÉSENTE dans le
/// répertoire d'icônes de la source.
///
/// 🔴 CE N'EST PAS LA TAILLE RENDUE, ET LES DEUX NE DOIVENT JAMAIS ÊTRE
/// CONFONDUES. Mesuré le 20 août 2026 sur deux témoins fabriqués : un `.ico`
/// ne contenant QU'UNE entrée 48×48, interrogé à 256, rend 256×256 32bpp — par
/// `IShellItemImageFactory` comme par `PrivateExtractIconsW`, sans
/// `SIIGBF_SCALEUP` et MÊME avec `SIIGBF_BIGGERSIZEOK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceMax {
    /// La plus grande entrée du répertoire d'icônes, en pixels. `bWidth == 0`
    /// vaut 256.
    Pixels(u16),
    /// 🔴 UNE VALEUR DISTINCTE DE 256, ET IL EST INTERDIT DE LES CONFONDRE.
    /// La provenance n'est ni un module PE ni un `.ico` lisible : association
    /// de type, espace de noms Shell, ou ressource illisible. **Mesuré : 37
    /// des 153 applications de cette VM.**
    NonMesuree,
}
```

Sur le fil, serde en fait `{"pixels":256}` pour la première et `"non-mesuree"`
pour la seconde. **`NonMesuree` ne peut structurellement pas être un nombre**,
et c'est ce que le critère ④ demande.

🔵 **`NonMesuree` S'ÉCRIT EN DEUX MOTS, ET CE N'EST PAS UN HASARD.** Le legs
n°9 de G1 est une **lacune de couverture inscrite dans le code** :
`IssueLancement` porte `rename_all = "kebab-case"` sur quatre variantes d'**un
seul mot**, si bien que passer en `snake_case` laisse `cargo test -p proto` à
**75 passed, 0 failed** — *« aucun test ne peut donc rougir si la convention de
nommage change ici »*. Une variante à deux mots rend la convention
**observable** : `non-mesuree` contre `non_mesuree`. **G2 referme donc la lacune
POUR L'ENUM QU'IL AJOUTE** ; ⚠️ **elle reste OUVERTE pour `IssueLancement`**,
qu'aucune tâche de ce plan ne touche, et le commentaire qui l'inscrit
(`proto/src/plateforme.rs:223-239`) reste vrai mot pour mot.

**`Application` gagne deux champs, tous deux OBLIGATOIRES :**

```rust
    /// L'empreinte SHA-256 du PNG, en hexadécimal minuscule — ou `None` quand
    /// l'extraction a échoué.
    ///
    /// ⚠️ UNE APPLICATION SANS ICÔNE VAUT MIEUX QU'UNE APPLICATION ABSENTE
    /// (spec §7). `None` n'est pas une erreur.
    pub icone: Option<String>,
    /// Toujours présent. Vaut `NonMesuree` quand `icone` est `None`.
    pub source_max: SourceMax,
```

⚠️ **Aucun `#[serde(default)]`, aucun `skip_serializing_if`.** C'est la règle
que ce module s'impose déjà pour le champ `v`
(`proto/src/plateforme.rs:99-102`) : un champ absent doit être **rejeté**, pas
silencieusement complété. `Option<String>` s'émet donc en `"icone":null`.

### D6 — L'adresse est le SHA-256 du **PNG**, et la non-détermination coûte des octets, jamais la justesse

**Décision : l'empreinte est celle des octets PNG**, et non celle des pixels.

La raison est le maillon suivant : **la plateforme RECALCULE l'empreinte de ce
qu'elle reçoit** (D10, doctrine des trois vérifications de la spec D7). Adresser
par les **pixels** l'obligerait à **décoder** le PNG pour vérifier, c'est-à-dire
à embarquer un décodeur PNG en TypeScript — **une dépendance de production
neuve**, ce que ce sous-bloc refuse. Adresser par les octets PNG rend la
vérification exacte et gratuite : `sha256(corps) === :sha256`.

🔵 **Le SHA-256 est DÉJÀ ÉCRIT ET TESTÉ dans l'agent** :
`agent/src/apps/sha256.rs` (**168** lignes, pur, éprouvé sur les vecteurs de
réponse connue de FIPS 180-4). **G2 le réemploie tel quel et n'y ajoute rien.**
Côté plateforme, `node:crypto` est dans la bibliothèque standard.

⚠️ **LE PRIX DE CE CHOIX EST LA DÉTERMINATION DE L'ENCODEUR, ET IL EST NOMMÉ.**
Si WIC n'écrivait pas deux fois les mêmes octets pour la même image — un chunk
`tIME`, un `tEXt` de logiciel —, l'empreinte changerait à chaque réconciliation
et **l'agent retéléverserait tout, indéfiniment**.

🔴 **CE N'EST PAS UNE PORTE ÉLIMINATOIRE, ET JE REFUSE DE LA RÉDIGER COMME
TELLE.** Un verdict négatif ne rendrait pas G2 faux : il le rendrait **coûteux**.
Le catalogue resterait juste, les icônes resteraient servies, le critère ⑤
tomberait, et le remède serait à portée — **retirer les chunks auxiliaires du
PNG avant de l'empreindre**, une opération de quinze lignes sur un format dont
le découpage en chunks est trivial, et **qui vit dans le module PUR**. Déguiser
cela en porte éliminatoire serait exactement le geste que ce dépôt vient de
payer : *un faux verdict éliminatoire rendu par une sonde sur une machine
saine.*

**Ce qui EST mesuré à ce jour, et ce qui ne l'est pas** : M3 établit que
**GDI+ est déterministe à l'intérieur d'une exécution** — 12 empreintes
partagées, dont une ×27, n'auraient pas collisionné sinon. **Rien n'est mesuré
de WIC, ni d'une exécution à l'autre.** C'est l'objet de la tâche 10, dont le
verdict négatif exige de relever **deux empreintes présentes et différentes** —
jamais une absence.

### D7 — 🔴 Le magasin est sur DISQUE, adressé par contenu ; la base ne porte que l'empreinte

**Décision : les octets vivent dans un répertoire, un fichier par empreinte, et
la table `application` ne porte que la chaîne hexadécimale.**

**Trois raisons, dans l'ordre de leur poids :**

1. 🔴 **Un BLOB ne traverse pas la double passe sans mentir.** PostgreSQL n'a
   **pas** de type `BLOB` (il a `bytea`) ; SQLite, lui, accepte **n'importe
   quel nom de type par affinité** — le lint statique
   `plateforme/src/base/sous-ensemble.test.ts:3-10` le documente pour `SERIAL`
   avec la mesure à l'appui. Écrire `BYTEA` passerait donc les deux passes en
   signifiant deux choses différentes : **c'est le piège `SERIAL` à l'envers**,
   et aucun des deux gardes du dépôt ne l'attrape.
2. **La doctrine est déjà écrite, par la spec elle-même.** Le §4 D7 tranche,
   pour la reprise de téléversement, en faveur d'« un **listage de répertoire**,
   jamais une table de comptabilité qui pourrait diverger du disque ». G2 pose
   la même frontière, un sous-bloc plus tôt, et G3 la trouvera posée.
3. **Le volume.** **4 576 398 octets mesurés** pour ce seul catalogue (M3),
   dans un fichier SQLite qui porte par ailleurs des sessions et des jetons.

**Où** : `PLATEFORME_ICONES`, **facultative**, défaut `donnees/icones`,
**journalisée au démarrage**.

⚠️ **Ce défaut est une ASYMÉTRIE ASSUMÉE avec `PLATEFORME_HOTE`**, qui n'en a
aucun, et il faut dire pourquoi. Le commentaire de tête de
`plateforme/src/config.ts` (l. 1-11, relu) fonde l'absence de défaut sur le fait
qu'un mauvais défaut **exposerait le service**. Ici, un mauvais répertoire coûte
**un retéléversement**, borné et automatique (D9) : le magasin **se reconstruit
tout seul**. Une rupture bruyante ne serait pas proportionnée — mais un silence
non plus, d'où la ligne de journal.

🔵 **ET C'EST CETTE PROPRIÉTÉ D'AUTO-RECONSTRUCTION QUI REND LE DISQUE
ACCEPTABLE**, pas une commodité. Elle mérite donc d'être **éprouvée** plutôt que
supposée : c'est le critère ⑦.

### D8 — Les octets d'icône passent par HTTP ; le canal `/agent` ne transporte que l'inventaire

**Décision : le canal ne transporte jamais un octet d'image.** C'est la
transposition littérale de la spec D7 (« le canal `/agent` ne transporte jamais
un installeur »), et les raisons sont les mêmes : le canal est en JSON, il porte
le battement de cœur, et **4,4 Mo en base64 y coûteraient +33 % et bloqueraient
le battement**.

**Ce qui passe par le canal** : `IconesManquantes { empreintes: Vec<String> }`,
descendante. **Ce qui passe par HTTP** : `PUT /icone/:sha256`,
`application/octet-stream`, jeton d'agent en en-tête `Authorization`.

⚠️ **Le plafond de corps de `routes-auth.ts` (4 Kio) NE DOIT PAS ÊTRE RELEVÉ** —
la spec D7 l'écrit déjà. La route d'icône a **son propre** plafond,
`ICONE_MAX_OCTETS`, posé à **1 Mio** : la plus grosse icône mesurée pèse moins
de 30 Kio en moyenne et le corpus entier 4,4 Mo pour 153, mais **aucune taille
individuelle n'a été relevée** — la valeur est donc **majorante à vue et NON
CALIBRÉE**, et le dire vaut mieux que de la présenter comme réglée.

### D9 — `IconesManquantes` est poussée après chaque `Catalogue`, et l'agent garde les octets du catalogue COURANT

**Le cycle :**

1. l'agent réconcilie, extrait les icônes des applications **dont la clé est
   neuve ou dont le `.lnk` a changé**, et retient
   `BTreeMap<empreinte, Vec<u8>>` **pour le catalogue courant seulement** ;
2. il pousse `Catalogue`, chaque `Application` portant son `icone` et son
   `source_max` ;
3. la plateforme fusionne, puis **demande au magasin lesquelles des empreintes
   annoncées lui manquent**, et pousse `IconesManquantes` — **et rien si
   l'ensemble est vide** ;
4. l'agent téléverse celles-là, une par une, par HTTP.

🔴 **L'ÉTAPE 3 NE SE DÉDUIT PAS DE LA BASE, ELLE INTERROGE LE DISQUE.** Une
table de comptabilité divergerait du magasin le jour où un fichier serait perdu
— et c'est précisément le jour où l'on a besoin de le savoir (D7, raison n°2).

⚠️ **Le coût mémoire est nommé et mesuré** : **4 576 398 octets** pour ce
corpus, non dédupliqué (M3). Dédupliqué en mémoire — un seul tampon par
empreinte, donc **99** pour ce corpus — il est **plus petit, mais N'A PAS ÉTÉ
MESURÉ**, et l'on ne le déduira pas d'une règle de trois : les icônes n'ont pas
la même taille. Le tampon est **remplacé à chaque réconciliation**, jamais
accumulé.

⚠️ **Un `IconesManquantes` perdu ne casse rien** : le canal est un `push` sans
garantie de livraison, et la réconciliation suivante rejoue l'annonce. **C'est
le même filet que `complet = true` à chaque réenrôlement** (décision D3 de G1),
et la recette de G1 l'a vu fonctionner sur le chemin réel.

### D10 — La plateforme RECALCULE l'empreinte de ce qu'elle reçoit

`PUT /icone/:sha256` **ne fait jamais confiance au chemin**. Elle lit le corps,
recalcule son SHA-256 par `node:crypto`, et **refuse en `400 { refus:
'empreinte' }`** s'il diffère — le fichier partiel n'est jamais écrit.

C'est la troisième des trois vérifications de la spec D7, transposée : *« aucun
saut ne fait confiance au précédent »*. **Sans elle, l'adressage par contenu
n'en serait pas un** : un agent fautif empoisonnerait le magasin d'un fichier
qui ne correspond pas à son nom, et **le `Cache-Control: immutable` de D11
rendrait l'empoisonnement permanent dans les caches**.

⚠️ **L'écriture est ATOMIQUE** : fichier temporaire puis `rename`. Un `PUT`
interrompu laisserait sinon un fichier tronqué **sous un nom qui promet son
contenu**, et le maillon suivant le servirait sans jamais le relire.

### D11 — 🔴 `GET /application/:id/icone?e=<empreinte>` — et c'est le `?e=` qui rend `immutable` HONNÊTE

La spec §5 écrit : « `GET /application/:id/icone` avec `Cache-Control` immuable
clé sur l'empreinte ». **Ces deux moitiés se contredisent telles quelles**, et
c'est la divergence E1 : sur une URL qui **ne porte pas** l'empreinte,
`immutable` est un mensonge — le jour où l'icône change, tous les caches
servent l'ancienne, pour un an.

**Décision : l'empreinte entre dans l'URL, en paramètre de requête**, et la
route **refuse en `404`** si `e` ne vaut pas l'empreinte courante de
l'application. L'URL devient alors véritablement adressée par contenu, et
`Cache-Control: private, max-age=31536000, immutable` **dit vrai**.

- **`private`, pas `public`** : la réponse est authentifiée par le porteur, et un
  cache partagé n'a rien à faire d'une icône servie sous un jeton.
- **le `404` sur `e` périmé n'est pas une commodité** : sans lui, une vieille
  URL servirait l'icône **courante** sous un en-tête immuable, ce qui
  empoisonnerait le cache pour un an avec une image qui n'est pas celle que
  l'URL nomme.

🔴 **L'AUTHENTIFICATION PASSE PAR `http/porteur.ts`, JAMAIS PAR UNE COPIE**, et
l'autorisation par la **même fonction `acces`** que les deux routes de G1
(`plateforme/src/http/routes-applications.ts:131-148`). Un refus rend
`404 { refus: 'vm-inconnue' }` — **le refus indistinguable**, comme le
propriétaire du dépôt l'a tranché (E5).

⚠️ **CONSÉQUENCE NOMMÉE, ET ELLE APPARTIENT À G5 : un `<img src>` ne porte pas
d'en-tête `Authorization`.** Une page qui afficherait ces icônes devra les
chercher par `fetch()` puis `URL.createObjectURL`, et **un manifeste PWA — dont
le navigateur va chercher les icônes tout seul, sans en-tête — ne pourra PAS
pointer cette route en l'état**. C'est une contrainte réelle pour G5, elle est
**écrite ici plutôt que découverte là-bas**, et G2 ne la tranche pas : le
trancher demanderait de décider si une icône peut être servie sans jeton, ce qui
est une décision de sécurité.

### D12 — `GET /applications` gagne `icone` et `source_max`, et rien d'autre

La route rend aujourd'hui `{ id, nom }` et **tait délibérément** `cible`,
`arguments`, `repertoire` et `chemin` — « des chemins du DISQUE DE LA VM, dont
le navigateur n'a aucun usage » (`routes-applications.ts:250-255`). **Ce
raisonnement ne s'oppose pas aux deux champs neufs** : une empreinte n'est le
chemin de rien, et `source_max` est une propriété de l'image.

Elle rend donc `{ id, nom, icone, source_max }`. C'est aussi **par elle que le
critère ③ se juge** : les deux valeurs doivent s'y lire sur le corpus réel.

### D13 — 🔴 `0005-icones.sql` : les deux colonnes sont NULLABLES, parce que `application` est désormais PEUPLÉE

**Mesure de G1, reportée dans `0004-applications.sql:4-12` et non re-supposée**
(SQLite 3.50.4, 19 août 2026) : `ALTER TABLE … ADD COLUMN … NOT NULL` **sans
défaut** est **refusé dès que la table porte une seule ligne** —
`Cannot add a NOT NULL column with default value NULL`. Or `application` porte
**154 lignes** sur cette VM depuis la recette de G1. **Les deux colonnes de G2
sont donc NULLABLES, et il n'y a pas d'alternative.**

```sql
ALTER TABLE application ADD COLUMN icone TEXT NULL;
ALTER TABLE application ADD COLUMN source_max_px INTEGER NULL;
```

🔴 **ET C'EST POURQUOI `NonMesuree` EST REPRÉSENTÉE PAR `NULL`, JAMAIS PAR `0`
NI PAR `256`.** La colonne est `INTEGER` : elle **ne peut pas** porter le mot
`non-mesuree`, donc elle porte `NULL`, et la reconstruction est sans ambiguïté
parce que **l'invariant est écrit** :

| `icone` | `source_max_px` | Ce que cela veut dire | Sur le fil |
| --- | --- | --- | --- |
| `NULL` | `NULL` | aucune icône : l'extraction a échoué | `icone: null`, `source_max: "non-mesuree"` |
| non nul | `NULL` | l'icône existe, **sa provenance n'est pas lisible** | `icone: "<hex>"`, `source_max: "non-mesuree"` |
| non nul | `n` | l'icône existe, et sa source portait une entrée de `n` px | `icone: "<hex>"`, `source_max: {"pixels": n}` |

⚠️ **La quatrième combinaison — `icone` nul et `source_max_px` non nul — est
INTERDITE et n'est écrite par aucun chemin.** Un test la nomme pour qu'elle ne
naisse pas d'une inattention.

⚠️ **Aucune clé étrangère, aucun `ADD COLUMN … UNIQUE`** : le premier parce que
SQLite ne sait pas ajouter de contrainte par `ALTER TABLE` (legs n°2 de P1), le
second parce qu'il est **refusé** par SQLite et accepté par Postgres —
divergence E7 de G1, mesurée. Aucun index n'est ajouté : on ne cherche jamais
une application **par** son icône.

⚠️ **`ADD COLUMN` et non `DROP`/`CREATE`**, pour la raison que `0004` écrit
(l. 14-19) : les deux supposent la même chose, et l'un **crie** quand la
supposition est fausse quand l'autre détruirait un catalogue en silence.

### D14 — `ICONES=0` désarme l'extraction, et `scripts/run-agent.sh` a sa TÂCHE DÉDIÉE

**Convention identique à `APPS`, `PLEIN_ECRAN`, `AUDIO`, `PART_SONDAGE` :
`=0` DÉSARME, une simple présence n'active pas.** Tester `is_ok()` activerait
le mécanisme en écrivant `ICONES=0` **pour le couper**.

**Trois raisons de l'ajouter, et elles ne sont pas décoratives :**

1. **L'extraction est la première chose de ce projet qui fasse durer une
   réconciliation en SECONDES** : **2 298 ms pour 153 icônes** au premier tour
   (M3), sur le fil COM dédié. Toute recette d'un autre chantier qui lance
   l'agent en mode `SUPERVISEUR` la paiera. `APPS=0` la coupe déjà, **mais en
   coupant tout le catalogue** : `ICONES=0` sépare les deux.
2. **Elle donne au critère ③ un témoin qui ne demande AUCUN rebâtissage du
   binaire.** Les rouges ⑤ et ⑥ de G1 ont exigé de muter la source, de
   recompiler et de redéployer sur la VM ; ici, deux exécutions du **même**
   binaire suffisent.
3. **Elle rend le désarmement OBSERVABLE** — trace
   `extraction d'icones DESARMEE (ICONES=0)`.

🔴 **`scripts/run-agent.sh` NE TRANSMET PAS UNE VARIABLE NEUVE**, et ce piège a
été payé **cinq fois**. La ligne à ajouter est
`${ICONES:+\$env:ICONES = '$ICONES'}`, entre `APPS` (l. 38) et `MICRO_MESURE`
(l. 39). **Tâche dédiée, qui ne fait rien d'autre.**

⚠️ **Le dernier saut — PowerShell → `agent.exe` — ne peut se vérifier que sur la
VM.** G1 l'a appris : `APPS` a vécu douze tâches déclarée « non vérifiée faute
de VM ». **La recette de G2 le vérifie explicitement**, et ce n'est pas un
supplément : c'est ce qui fait du témoin du point 2 un témoin.

### D15 — La prédicat de désarmement est PUR, comme celui d'`APPS`

`apps::desarme(valeur: Option<&str>) -> bool` (`agent/src/apps.rs:45-47`) existe
déjà, avec son test et sa rouge nommée. **G2 le RÉEMPLOIE tel quel** et n'écrit
pas de jumeau : la lecture d'environnement, elle, vit dans `apps/icone.rs`
derrière un `OnceLock`, `#[cfg(windows)]`, comme celle d'`APPS` vit dans
`apps::demarrer`.

⚠️ **C'est délibérément une réutilisation et non une copie** : deux prédicats
identiques divergeraient le jour où l'un accepterait `"false"`, et le dépôt a
déjà refusé une copie de décision de sécurité pour la même raison
(`routes-applications.ts:9-20`).

---

## Divergences relevées entre la spec, ce que G1 a livré, et l'état RÉEL de l'arbre

**Chacune a été relue dans le code le 20 août 2026.** L'arbre a bougé depuis la
clôture de G1 : **deux de ses trois legs 🔴 sont fermés**, et les documents qui
les décrivent — le document de résultats de G1 et la section G1 de `CLAUDE.md` —
**ne décrivent plus le dépôt sur ces deux points**.

### E1 — La spec demande un `Cache-Control` immuable sur une URL qui ne porte pas l'empreinte

Spec §5, G2, ligne « Livre » : « `GET /application/:id/icone` avec
`Cache-Control` immuable **clé sur l'empreinte** ». **Les deux moitiés se
contredisent** : `immutable` promet que la ressource à **cette URL** ne changera
jamais, et l'URL proposée est stable quand l'icône, elle, peut changer.
**Tranché en D11** : l'empreinte entre dans l'URL (`?e=<empreinte>`), et un `e`
périmé rend `404`. **Corrigé, pas contourné.**

### E2 — ✅ Le risque n°3 du §11 de la spec est LEVÉ : le corpus porte les deux valeurs

La spec §11 nomme comme risque « le corpus réel ne porte pas les deux valeurs de
`source_max_px` », avec pour mitigation que « le critère ③ existe pour le dire,
et il rend `NON MESURABLE` plutôt que `tenu` ». **Mesuré (M1) : 61 applications
à 256, 55 en dessous, 37 hors de portée.** Le critère ③ **est mesurable**, et il
n'a plus à se replier. ⚠️ **Une exécution, une VM, un corpus**, et la sonde n'est
pas le produit — voir la portée de M1.

### E3 — 🔴 Le critère ① de la spec ne peut PAS exiger l'alpha de TOUTES les icônes

Spec §5, critère ① : « Le PNG rendu fait 256×256 **avec un canal alpha non
trivial** ». **Mesuré (M3) : 149 sur 153 en portent un ; QUATRE N'EN PORTENT
PAS.** Un critère écrit « toutes » serait donc **faux par construction sur ce
corpus**, et le jouer rendrait rouge un produit correct.

**Tranché** : le critère porte sur **le mécanisme**, pas sur une universalité —
« il existe au moins une application dont le PNG porte un alpha partiel, et le
compte de celles qui en portent est du même ordre que les 149/153 mesurés ». **Sa
rouge reste entière et réelle** : elle se joue en remplaçant `WICBitmapUseAlpha`
par `WICBitmapIgnoreAlpha` (une constante nommée, D3), ce qui fait tomber le
compte à **zéro**.

### E4 — ⚠️ Ma sonde rend `cles=153` là où la spec et G1 rendent `154`

Voir M1. **Non expliqué**, deux causes candidates nommées, aucune vérifiée.
**Aucune décision de ce plan n'en dépend** ; c'est signalé plutôt que dissimulé,
et cela **renforce** le legs n°6 de G1 (« les champs par entrée du corpus
restent ceux de `WScript.Shell` ») au lieu de le réfuter.

### E5 — ✅ Le legs n°3 de G1 (la divergence `403`/`404`) est TRANCHÉ, et G2 en hérite

Le document de résultats de G1 (§10) et la section G1 de `CLAUDE.md` déclarent
cette divergence « VIVANTE dans le produit et NON TRANCHÉE ». **Elle l'est
depuis le commit `1976f2f`** (« une VM d'autrui repond comme une VM inconnue, et
le journal seul distingue ») : `plateforme/src/http/routes-applications.ts`
**a cédé**, ses deux routes rendent le **même** `404 { refus: 'vm-inconnue' }`
(l. 246 et l. 278), le motif de fil `vm-etrangere` **n'existe plus**, et le
verdict interne `'etrangere'` survit **uniquement** pour alimenter la ligne de
journal de `journaliserLeRefus` (l. 169-181).

**Conséquence directe pour G2** : la route d'icône **n'a aucune décision à
prendre** — elle appelle `acces` et rend le refus commun. ⚠️ **Un implémenteur
qui suivrait le document de résultats de G1 réintroduirait l'oracle
d'énumération que le propriétaire du dépôt vient de retirer.**

### E6 — ✅ Le legs n°2 de G1 (un refus de version illisible) est FERMÉ, et c'est ce qui rend D1 diagnosticable

Commit `457a7f8`. Le refus est désormais une **enveloppe minimale hors
versionnement**, en trois clauses écrites en tête de `proto/src/plateforme.rs`
(l. 49-70) : son `v` est **toléré** (`version_toleree`, l. 124-129) mais reste
obligatoire et entier ; son `motif` est un **mot libre** sur le fil (l. 374) que
`MotifCanal::depuis_mot` (l. 182-184) interprète quand elle le peut ; et **sa
forme est GELÉE** — `type`, `v`, `motif`, et rien d'autre, jamais.

🔴 **G2 EST LE PREMIER BUMP QUI PEUT PROUVER QUE CELA MARCHE**, parce qu'il est
le premier depuis la correction. C'est le critère ⑧, et sa rouge est **gratuite** :
le binaire d'avant G2 est un agent v2.

⚠️ **La clause 3 est une contrainte SUR G2 :** la variante `Refus` ne doit gagner
aucun champ. Aucune tâche de ce plan n'y touche, et c'est écrit pour qu'aucune
ne l'invente.

### E7 — ✅ Le legs n°1 de G1 (le pont s'évince avec son père) est FERMÉ

`agent/src/main.rs:222-238` (relu) : les enfants et le pont reçoivent
`AGENT_JETON` du superviseur, **n'ouvrent aucun canal**, et journalisent
`identité héritée du superviseur (AGENT_JETON) : ce processus n'ouvre …`.
`agent/src/apps.rs:60-66` en tire la conséquence mesurée : **6 réconciliations
en 64 s avant correction contre 3 après**.

**Conséquence pour G2, et elle est heureuse** : la recette de G1 a été conduite
**sous** ce défaut, et toutes ses mesures de catalogue en portaient la
condition. **Celle de G2 ne la portera pas** — et le coût de l'extraction
d'icônes ne sera donc pas payé deux fois par deux processus.

### E8 — `pilotes.test.ts:33` fige la liste des migrations : `0005` la fait ROUGIR

`expect(suivi.map((l) => Number(l.version))).toEqual([1, 2, 3, 4])`. **Rouge
gratuite et obligatoire**, comme la divergence E9 de G1 pour `0004`. La mettre à
jour fait partie de la tâche qui ajoute la migration, jamais d'une autre.

### E9 — `estApplication` exige que TOUS les champs déclarés soient des `string`

`proto/ts/plateforme.ts:264` :
`return estObjetJson(valeur) && CHAMPS_APPLICATION.every((champ) => estChaine(valeur[champ]));`
avec `CHAMPS_APPLICATION` écrit à la main l. 45-47. **Les deux champs neufs ne
sont pas des chaînes** — l'un est `string | null`, l'autre un objet ou une
chaîne. Le validateur doit donc les traiter **séparément**, et le compteur
`CHAMPS_APPLICATION` ne peut plus les couvrir tous.

⚠️ **C'est une rouge gratuite ET un piège** : ajouter naïvement `'icone'` à
`CHAMPS_APPLICATION` ferait **refuser tout catalogue** dont une application n'a
pas d'icône, en silence, avec un motif `forme`. **Le test qui l'attrape est celui
d'une `Application` à `icone: null`.**

⚠️ **Asymétrie relevée, et NON corrigée par G2** : côté Rust,
`deny_unknown_fields` **refuse** un champ inconnu ; côté TypeScript,
`estApplication` **ignore** ce qu'il ne déclare pas. Les deux bouts ne sont donc
pas également stricts — mais la vérification de version passe **avant** (l. 300),
si bien que le cas ne se présente que pour deux pairs de **même** version.
**Signalé, pas tranché** : le trancher demanderait de décider si un miroir doit
reproduire `deny_unknown_fields`, ce qui dépasse G2.

### E10 — `routes-applications.test.ts` est à 480 lignes, marge 20

Il porte **dix-sept** tests (annoncé et mesuré par G1). G2 y ajoute la famille de
la route d'icône. **Il franchira 500** : extraction **AVANT** l'addition, jamais
compression. Point de chute : `plateforme/src/http/routes-icone.test.ts`, qui
suit le découpage du code (une route, un fichier de test).

### E11 — ⚠️ `Win32_Graphics_Imaging` est une CANDIDATE, pas une certitude

Elle est absente d'`agent/Cargo.toml` et le module WIC est gardé derrière elle
(`windows-0.62.2/src/Windows/Win32/Graphics/mod.rs:39`, relu). **Mais G1 a payé
exactement cette affirmation** : sa divergence E10 annonçait `Win32_UI_Shell`
seule pour `ShellExecuteExW`, et la compilation l'a **réfutée** — il fallait
aussi `Win32_System_Registry`, parce que `SHELLEXECUTEINFOW` porte un champ
`HKEY`.

**Règle de ce plan : la liste de fonctionnalités est une HYPOTHÈSE, et la tâche
qui l'écrit la confirme par `cargo check --target x86_64-pc-windows-gnu` AVANT
de déclarer quoi que ce soit.** Si une seconde fonctionnalité s'avère
nécessaire, elle s'ajoute avec sa raison — et **elle reste une fonctionnalité
d'un crate déjà présent**, donc l'invariant « aucune dépendance neuve » tient.

### E12 — `PLATEFORME_ICONES` est une variable de configuration NEUVE

`plateforme/src/config.ts` lit l'environnement **ici et nulle part ailleurs**
(l. 35-37, relu). G2 y ajoute un champ. ⚠️ **Ce n'est pas une variable de
l'agent** : `scripts/run-agent.sh` n'a rien à voir avec elle, et les confondre
ferait chercher un piège là où il n'est pas. Le piège des cinq variables
oubliées porte sur `ICONES`, pas sur celle-ci.

### E13 — Le legs n°7 de G1 est à portée, et G2 travaille dans le fichier

`agent/src/apps/boucle.rs:109` émet `retenus = lancables.len()` — **une table
indexée par CLÉ**, donc toujours égale à `cles`. Les 167 raccourcis retenus **ne
sont émis nulle part**, et le champ ment sur son nom. G2 câble l'extraction
d'icônes **dans ce fichier même**.

**Décision : G2 le ferme**, par un compteur distinct incrémenté dans la branche
`Ok(())` de `raccourci::retenir`. La règle qui l'autorise est celle du
découpage rétroactif — *« au moment où l'on travaille dedans »* — et le coût est
de deux lignes. **Sa rouge** : sur ce corpus, `retenus` doit valoir **167** et
`cles` **154** ; un `retenus` égal à `cles` est l'état d'avant, et il est
observable.

### E14 — Contrôle qui PASSE : le SHA-256 de l'agent est réutilisable tel quel

`agent/src/apps/sha256.rs` expose `condenser(&[u8]) -> [u8; 32]` (l. 79) et
`hex` (employé par `raccourci::cle`, l. 137). Il est **pur**, sans `cfg`, et
éprouvé sur les vecteurs de FIPS 180-4. **Vérifié : G2 n'a besoin d'aucune
addition à ce module.** Son en-tête (l. 13-17) prévient qu'il n'est pas une
primitive de sécurité — **et l'usage de G2 ne l'en fait pas une** : une
empreinte de contenu qui servirait à décider d'un droit devrait céder la place
à une implémentation auditée, et **aucun droit ne dépend d'elle ici** (D11
autorise par `acces`, jamais par l'empreinte).

### E15 — Contrôle qui PASSE : `IconesManquantes` ne traverse aucun `match` catch-all

Le bras `Ok(autre) => return` de `agent/src/capteur/pont_media.rs` **tue le fil
en silence**, et ce dépôt l'a payé **quatre fois** (D5, D6, D7, D8). G1 a
vérifié par la commande qu'aucun de ses messages n'y passe (E18). **G2 doit le
re-vérifier plutôt que le supposer** : `IconesManquantes` voyage sur
`DepuisLaPlateforme`, c'est-à-dire sur le canal `/agent`, dont le point d'entrée
est `agent/src/plateforme.rs` — pas le canal capteur↔enfant. ⚠️ **Le contrôle est
une TÂCHE de la revue transverse, pas une supposition de ce paragraphe.**

⚠️ **En revanche, `agent/src/transport/controle.rs` porte un `match` EXHAUSTIF
sur `AgentControl`** — celui-là se signale tout seul, et G2 n'y ajoute rien
puisqu'il ne touche pas à `control.rs`.

---

## Structure des fichiers

```
proto/
  src/plateforme.rs                  MODIFIÉ — SourceMax, 2 champs, IconesManquantes, VERSION = 3
  src/plateforme/tests.rs            🔴 EXTRAIT (561 -> cycle de vie seul)
  src/plateforme/tests_apps.rs       NEUF — extraction, PUIS les cas de G2
  ts/plateforme.ts                   MODIFIÉ — le miroir
  ts/plateforme.test.ts              🔴 EXTRAIT (512 -> cycle de vie seul)
  ts/plateforme-apps.test.ts         NEUF — extraction, PUIS les cas de G2
  plateforme-vectors.json            MODIFIÉ — "version": 3, un cas par forme neuve

agent/
  Cargo.toml                         MODIFIÉ — Win32_Graphics_Imaging (HYPOTHÈSE, E11)
  src/apps.rs                        MODIFIÉ — `pub mod icone;`
  src/apps/icone.rs                  NEUF — #[cfg(windows)] : IShellItemImageFactory + WIC, ICONES=0
  src/apps/icone/ressource.rs        NEUF — 🔴 PUR : GRPICONDIR et ICONDIR depuis un &[u8]
  src/apps/icone/ressource/tests.rs  NEUF — PUR, sur les deux témoins versés
  src/apps/icone/magasin.rs          NEUF — 🔴 PUR : l'adressage par contenu et la déduplication
  src/apps/icone/lecture_pe.rs       NEUF — #[cfg(windows)] : LoadLibraryExW / FindResourceW
  src/apps/icone/source.rs           NEUF — PUR : d'où vient l'icône d'un raccourci
  src/apps/icone/televersement.rs    NEUF — le PUT HTTP vers la plateforme
  src/apps/boucle.rs                 MODIFIÉ — le câblage, et le legs n°7 de G1
  src/plateforme.rs                  MODIFIÉ — la branche descendante IconesManquantes
  testdata/g2-temoin-48.ico          NEUF — 🔴 le témoin qui ne contient QUE du 48×48
  testdata/g2-temoin-256.ico         NEUF — 🔴 le témoin qui contient du 256×256
  testdata/fabriquer-temoins-ico.py  NEUF — les fabrique, sur l'hôte, sans Windows

plateforme/
  src/base/migrations/0005-icones.sql  NEUF — deux colonnes NULLABLES (D13)
  src/base/pilotes.test.ts             MODIFIÉ — [1,2,3,4] -> [1,2,3,4,5] (E8)
  src/apps/icones.ts                   NEUF — le magasin de disque adressé par contenu
  src/depot/application.ts             MODIFIÉ — les deux colonnes
  src/apps/catalogue.ts                MODIFIÉ — les deux champs traversent la fusion
  src/agents/canal.ts                  MODIFIÉ — la branche qui pousse IconesManquantes
  src/http/routes-icone.ts             NEUF — PUT /icone/:sha256 et GET /application/:id/icone
  src/http/routes-icone.test.ts        NEUF
  src/http/routes-applications.ts      MODIFIÉ — deux champs de plus dans la réponse
  src/http/routes-applications.test.ts 🔴 EXTRAIT (480, marge 20 — E10)
  src/http/serveur.ts                  MODIFIÉ — une ligne de chaînage
  src/http/entetes-routeurs.test.ts    MODIFIÉ — un `it()` pour le routeur neuf
  src/config.ts                        MODIFIÉ — PLATEFORME_ICONES

scripts/
  run-agent.sh                       🔴 MODIFIÉ — une ligne, TÂCHE DÉDIÉE (D14)
```

🔴 **`agent/src/apps/icone/ressource.rs` et `magasin.rs` sont PURS, et c'est le
point le plus important de cette liste** — la spec §6 l'écrit déjà pour le
premier. `apps` est déclaré **sans `cfg`** dans `main.rs` depuis G1
(`agent/src/apps.rs:3-7`), et ce sont ses enfants Windows qui portent le leur :
l'arbre `apps::icone::ressource` **existe donc sur l'hôte Linux**, où ses tests
courent. **La « Convention de module enfant » de `CLAUDE.md` n'est PAS
mobilisée** — aucun module ne franchit ici de frontière `#[cfg(windows)]` —,
et elle est nommée pour dire qu'elle a été considérée.

⚠️ **`apps/icone.rs` est le parent, et il est `#[cfg(windows)]`.** Ses enfants
purs (`ressource`, `magasin`, `source`) doivent donc être déclarés **ailleurs
que dans lui** — dans `apps.rs`, comme `raccourci` et `reconciliation` le sont
déjà. C'est la même figure exactement, et elle évite le `#[path]`.

---

## Interfaces partagées

**Ce que G2 ajoute à `proto/src/plateforme.rs` :**

```rust
pub const PLATEFORME_VERSION: u8 = 3;   // v3 (G2) : icônes et leur provenance

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceMax { Pixels(u16), NonMesuree }

pub struct Application {
    // … les six champs de G1, inchangés …
    pub icone: Option<String>,     // SHA-256 hex du PNG, ou None
    pub source_max: SourceMax,     // toujours présent
}

pub enum DepuisLaPlateforme {
    // … les quatre variantes de G1, inchangées …
    /// Les empreintes que la plateforme n'a PAS, parmi celles que le dernier
    /// `Catalogue` a annoncées.
    ///
    /// 🔴 ELLE N'EST PAS ÉMISE QUAND L'ENSEMBLE EST VIDE : une liste vide
    /// coûterait un message par réconciliation sur un disque au repos, ce que
    /// le diff de G1 existe précisément pour éviter.
    IconesManquantes {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        empreintes: Vec<String>,
    },
}
```

**Formes de fil, à figer dans `proto/plateforme-vectors.json` :**

| Forme | JSON |
| --- | --- |
| `SourceMax::Pixels(256)` | `{"pixels":256}` |
| `SourceMax::NonMesuree` | `"non-mesuree"` |
| une `Application` avec icône | `…,"icone":"a1b2…","source_max":{"pixels":256}` |
| une `Application` sans icône | `…,"icone":null,"source_max":"non-mesuree"` |
| `IconesManquantes` | `{"type":"icones-manquantes","v":3,"empreintes":["a1b2…"]}` |

**Ce que la plateforme expose en HTTP :**

| Route | Qui | Corps | Réponses |
| --- | --- | --- | --- |
| `PUT /icone/:sha256` | l'**agent**, jeton d'agent | `application/octet-stream`, ≤ `ICONE_MAX_OCTETS` | `204` ; `400 {refus:'empreinte'}` ; `413 {refus:'taille'}` ; `403 {refus:'jeton-utilisateur'}` |
| `GET /application/:id/icone?e=<empreinte>` | l'**utilisateur**, jeton porteur | — | `200 image/png` ; `400 {refus:'empreinte-absente'}` ; `404 {refus:'vm-inconnue'}` ; `404 {refus:'application-inconnue'}` ; `404 {refus:'icone-inconnue'}` |
| `GET /applications?vm=…` | inchangée | — | `200 { applications: [{ id, nom, icone, source_max }] }` |

⚠️ **Le `403 { refus: 'jeton-utilisateur' }` du `PUT` est le SYMÉTRIQUE du
`403 { refus: 'jeton-agent' }` de `porteur.ts`**, et il est argumenté de la même
façon : le jeton est **valide**, il n'est simplement pas celui d'un agent, et un
`401` inviterait à se reconnecter pour rien.

---

## Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable | Paquets touchés |
| --- | --- | --- | --- | --- |
| 0 — dette et protocole | 1, 2, 3, 4, 5 | 1 ← rien ; 2 ← rien ; 3 ← 1 ; 4 ← 2, 3 ; 5 ← 4 | **1 et 2 entre elles seulement** (D1) | 🔴 `proto/` |
| 1 — l'agent, **pur** | 6, 7, 8 | 6 ← rien ; 7 ← 6 ; 8 ← rien | 6 et 8 avec tout | 🔴 `agent/` |
| 2 — l'agent, Windows | 9, 10, 11, 12, 13 | 9 ← 3 ; 10 ← 9 ; 11 ← 6, 7, 8, 9, 10 ; 12 ← 5, 11 ; 13 ← 9 | 13 avec tout (paquet `scripts/` seul) | 🔴 `agent/`, `scripts/` |
| 3 — la plateforme, persistance | 14, 15, 16, 17 | 14 ← rien ; 15 ← rien ; 16 ← rien ; 17 ← 14, 5 | 14, 15 et 16 avec tout | `plateforme/` |
| 4 — la plateforme, HTTP et canal | 18, 19, 20, 21 | 18 ← rien ; 19 ← 15, 16, 17, 18 ; 20 ← 19 ; 21 ← 15, 17, 5 | 18 avec tout | `plateforme/` |
| 5 — recette et clôture | 22, 23, 24, 25 | 22 ← tout ; 23, 24, 25 ← 22 | 23 et 24 entre elles | — |

**Chemin critique** : 1 → 3 → 4 → 5 → 11 → 12 → 22. ⚠️ **Un second brin
de MÊME longueur passe par la mesure de déterminisme** — 1 → 3 → 9 → 10 → 11 —
et c'est lui qui décide de la date à laquelle la VM est nécessaire.

**SEPT tâches purement isolées peuvent démarrer ensemble au premier tour** :
**1** et **2** (`proto/`, les deux extractions de dette), **6** et **8**
(`agent/`, purs), **14**, **15** et **16** (`plateforme/`). Trois paquets
distincts, donc aucune ne bloque une autre sur un fichier.
⚠️ **Le compte a été vérifié contre la liste, pas estimé** : une première
rédaction annonçait « cinq » pour six noms.

🔴 **Les tâches 3, 4 et 5 ne se parallélisent avec RIEN dans `proto/`** : elles
portent le bump de version, et le vert ne se lit qu'à la fin de la 5 (D1). C'est
la reconduite du groupe indivisible de G1.

🔴 **Les tâches 1 et 2 (extractions) précèdent la 3 et la 4 sans exception**, et
n'ajoutent **aucune** ligne de comportement.

🔴 **La tâche 18 (extraction de `routes-applications.test.ts`) précède la 19**,
même règle.

🔴 **La tâche 13 (`scripts/run-agent.sh`) est dédiée et ne fait rien d'autre.**

⚠️ **La tâche 10 est la SEULE tâche de développement qui emploie la VM**, et la
tâche **22** est la seule recette. Toutes deux vérifient `Get-Process agent`
avant et après — **y compris après une tentative échouée** (piège de D8, payé
trois fois sur trois).

---

# Famille 0 — la dette de `proto/`, puis le protocole

### Task 1 : 🔴 EXTRACTION de `proto/src/plateforme/tests.rs`, AVANT toute addition

- [ ] Découper `proto/src/plateforme/tests.rs` (**561** lignes, relevé par la
      commande) en deux : le **cycle de vie** reste (version, refus,
      enrôlement, battement), la **gestion d'apps** part vers
      `proto/src/plateforme/tests_apps.rs` (`Application`, `Catalogue`,
      `Lancer`, `Lancee`, `IssueLancement`, la conformité aux vecteurs de ces
      formes-là).
- [ ] Déclarer le module neuf **chez le parent**, à côté de l'existant :
      `#[cfg(test)] #[path = "plateforme/tests_apps.rs"] mod tests_apps;`
      (`proto/src/plateforme.rs:431-433` porte déjà la forme).
- [ ] **Transposition VERBATIM. Aucune ligne de comportement n'est ajoutée,
      retirée ni reformulée.**

**Test :** `cargo test -p proto`.
**Ce qui le rend ROUGE :** un test perdu au découpage. 🔴 **Le contrôle est le
COMPTE, et il s'annonce AVANT d'être mesuré** — relever `cargo test -p proto`
avant l'extraction, écrire le nombre attendu dans le rapport, puis mesurer.
*(D10 : un implémenteur a écrasé un fichier de tests et supprimé un test
antérieur ; il l'a vu parce que le compte est sorti à 452 au lieu des 453
annoncés d'avance.)*
**Contrôle de taille :** `wc -l` sur les **deux** fichiers ; aucun au-dessus de
500. ⚠️ **Ils sont dans la table de dette de `CLAUDE.md` : la tâche 24 doit les
en RETIRER**, sans quoi la dette resterait écrite après avoir été purgée.

### Task 2 : 🔴 EXTRACTION de `proto/ts/plateforme.test.ts`, AVANT toute addition

- [ ] Même découpage, même frontière : le cycle de vie reste,
      `proto/ts/plateforme-apps.test.ts` accueille la gestion d'apps.
- [ ] Aucune déclaration à ajouter : Vitest découvre les `*.test.ts`.
- [ ] **Transposition VERBATIM.**

**Test :** `cd proto && npm test`.
**Ce qui le rend ROUGE :** identique à la tâche 1 — **le compte annoncé
d'avance**. ⚠️ Vitest rend un compte de **fichiers** et un compte de **tests** :
**dire lequel on annonce** (piège des « dix » et « dix-sept » de P3).

### Task 3 : `SourceMax`, les deux champs, `IconesManquantes`, et `PLATEFORME_VERSION = 3`

- [ ] `proto/src/plateforme.rs` : ajouter `SourceMax` (D5), les deux champs
      d'`Application` (D5), la variante `DepuisLaPlateforme::IconesManquantes`
      et son constructeur, et porter `PLATEFORME_VERSION` à **3**.
- [ ] Compléter la documentation de `PLATEFORME_VERSION` : `v3 (sous-bloc G2) :
      icônes 256, leur provenance, et l'inventaire des manquantes`.
- [ ] 🔴 **Reprendre l'encadré `❌ « LA VM SE TAIT SANS BOUCLER » EST FAUX,
      MESURÉ`** (l. 93-96) : il décrit le comportement d'**avant** la correction
      du 20 août 2026, et E6 le réfute. **L'annoter, ne pas l'effacer** — c'est
      un relevé daté, et le dépôt annote ses relevés datés plutôt que de les
      réécrire.
- [ ] Écrire les cas dans `tests_apps.rs` (né tâche 1) : la ronde de
      sérialisation des deux formes de `SourceMax`, une `Application` avec
      icône et une **sans**, `IconesManquantes`, et le refus d'un `v` de 2.
- [ ] 🔴 **Un test qui NOMME la lacune refermée** : muter `rename_all` de
      `SourceMax` en `snake_case` doit faire **ÉCHOUER** un test. C'est la
      démonstration que `NonMesuree` — deux mots — rend la convention
      observable, là où `IssueLancement` ne le permet pas (legs n°9 de G1).

**Test :** `cargo test -p proto`.
**Ce qui le rend ROUGE — quatre états, tous atteignables :**
1. `SourceMax::NonMesuree` sérialisé en nombre (par ex. `0`) : le test de forme
   compare la chaîne JSON exacte et tombe. **C'est le critère ④.**
2. `#[serde(default)]` posé sur `icone` : le test « une `Application` sans champ
   `icone` est REFUSÉE » tombe.
3. `PLATEFORME_VERSION` laissée à 2 : le test qui compare la constante à 3 tombe.
4. `rename_all` mutée en `snake_case` : voir ci-dessus.

### Task 4 : le miroir TypeScript

- [ ] `proto/ts/plateforme.ts` : `PLATEFORME_VERSION = 3`, le type
      `SourceMax = { pixels: number } | 'non-mesuree'`, les deux champs
      d'`Application`, `IconesManquantesMessage`, `encodeIconesManquantes`, et
      l'entrée dans l'union `DepuisLaPlateforme` **et** dans `typesDepuis()`.
- [ ] 🔴 **Corriger `estApplication` (l. 264) SANS ÉLARGIR `CHAMPS_APPLICATION`**
      (E9) : les six champs `string` restent validés par la liste, et les deux
      neufs le sont **séparément**, par leurs propres gardes —
      `icone === null || typeof icone === 'string'`, et une garde de `SourceMax`
      qui accepte `'non-mesuree'` **ou** un objet à `pixels` entier.
- [ ] Écrire les cas dans `plateforme-apps.test.ts` (né tâche 2).

**Test :** `cd proto && npm test && npm run typecheck`.
**Ce qui le rend ROUGE — et le premier est le piège de E9 :**
1. `'icone'` ajouté à `CHAMPS_APPLICATION` : le test « un catalogue dont une
   application n'a pas d'icône est ACCEPTÉ » tombe. **Cette rouge est celle qui
   compte**, parce que son état est **silencieux** en production : un catalogue
   entier refusé pour `forme`.
2. la garde de `SourceMax` acceptant n'importe quel objet : le test qui refuse
   `{"pixels":"gros"}` tombe.
3. `typesDepuis()` non mis à jour : le test qui compare l'union à la liste
   tombe. *(C'est le jumeau du `TYPES_AGENT` DÉRIVÉ de P1 : une liste écrite à
   la main est une liste qu'on oublie.)*

### Task 5 : `proto/plateforme-vectors.json` — `"version": 3`, vérifiée des DEUX côtés

- [ ] Porter `"version"` à **3**, et **réécrire les `json` de TOUS les cas
      existants**, dont le `v` passe de 2 à 3. ⚠️ **Sauf ceux de
      `refus_lisibles`**, qui portent délibérément d'autres versions : ils sont
      la preuve de la clause 1 de E6, et les toucher détruirait ce qu'ils
      gardent.
- [ ] Ajouter un cas par forme neuve (tableau des « Formes de fil » ci-dessus),
      **dont une `Application` sans icône**.
- [ ] Les deux consommateurs — `tests_apps.rs` et `plateforme-apps.test.ts` —
      lisent la clé `version` et la comparent à leur constante.

**Test :** `cargo test -p proto` **et** `cd proto && npm test`. Les deux.
**Ce qui le rend ROUGE :** `"version": 2` laissé dans le fichier — les deux
côtés tombent, et c'est la propriété que ce fichier existe pour tenir. ⚠️ **La
lacune de `vectors.json`, dont seul le consommateur TypeScript contrôle la
version, ne doit pas être rouverte ici** (elle est déjà fermée pour ce
fichier-ci, en-tête du JSON, relu).

**🔴 Le groupe 3-4-5 est INDIVISIBLE.** Entre la tâche 3 et la tâche 5, l'arbre
est **rouge par construction** : le Rust dit 3 et le TypeScript dit encore 2.
**Ne pas commiter un vert intermédiaire, ne pas chercher à en fabriquer un.**
C'est la reconduite littérale de D10 de G1.

---

# Famille 1 — l'agent, PUR

### Task 6 : 🔴 `apps/icone/ressource.rs` — lire un `GRPICONDIR` et un `ICONDIR`, et les deux témoins VERSÉS

**C'est la tâche qui porte la preuve du sous-bloc**, et elle n'a besoin d'aucune
VM.

- [ ] `agent/testdata/fabriquer-temoins-ico.py` : un script **hôte**, sans
      Windows, qui écrit les deux `.ico`. **Il est versé avec eux**, pour que
      personne n'ait à croire à leur contenu — le §11 de la spec relève que les
      témoins d'origine vivent dans `C:\dev\`, « c'est-à-dire nulle part de
      durable ».
- [ ] `agent/testdata/g2-temoin-48.ico` — **une seule entrée, 48×48, 32 bpp**.
- [ ] `agent/testdata/g2-temoin-256.ico` — **une seule entrée, 256×256, 32 bpp**,
      donc `bWidth == 0` dans son `ICONDIR`.
- [ ] `agent/src/apps/icone/ressource.rs`, **PUR, aucun `cfg`** :
      - `pub fn tailles_icondir(octets: &[u8]) -> Option<Vec<u16>>` — entrées de
        **16** octets, en-tête `reserved == 0 && type == 1` ;
      - `pub fn tailles_grpicondir(octets: &[u8]) -> Option<Vec<u16>>` — entrées
        de **14** octets, même en-tête ;
      - `pub fn maximum(tailles: &[u16]) -> SourceMax` — `NonMesuree` sur une
        liste vide, `Pixels(max)` sinon.
      **Deux fonctions distinctes, jamais une avec un drapeau** (D4).
- [ ] Déclarer `pub mod icone;` dans `agent/src/apps.rs` **sans `cfg`**, et
      `pub mod ressource;` **dans `apps.rs` également** — pas dans `icone.rs`,
      qui est `#[cfg(windows)]`.

**Test :** `agent/src/apps/icone/ressource/tests.rs`, sur l'hôte, contre les deux
témoins lus par `include_bytes!`.
**Ce qui le rend ROUGE — cinq états, tous atteignables :**
1. 🔴 **`bWidth == 0` rendu comme `0` au lieu de `256`** : le témoin 256 rend
   `Pixels(0)`, et `maximum` le classe **sous** n'importe quelle autre entrée.
   **C'est la rouge principale**, et la seule qui soit invisible sans le témoin.
2. le pas d'entrée de 14 employé pour un `ICONDIR` (ou 16 pour un
   `GRPICONDIR`) : les tailles lues sont du bruit, et le témoin à une seule
   entrée le montre sans ambiguïté.
3. une liste vide rendant `Pixels(0)` au lieu de `NonMesuree`.
4. un en-tête non vérifié : quatre octets arbitraires passent pour un `ICONDIR`.
5. un tampon tronqué qui déborde au lieu de rendre `None`.

**Contrôle qu'aucun de ces cinq états n'est vacueux :** les deux témoins sont
**structurellement différents sur le seul octet qui compte** — `48` contre
`0` — et ils sont fabriqués par un script versé, donc relisibles.

### Task 7 : `apps/icone/magasin.rs` — l'adressage par contenu et la déduplication, PURS

- [ ] `agent/src/apps/icone/magasin.rs`, **PUR** :
      - `pub fn empreinte(png: &[u8]) -> String` — délègue à
        `super::super::sha256::hex` (E14), **aucune ligne de cryptographie
        neuve** ;
      - `pub struct Magasin` : `BTreeMap<String, Vec<u8>>`, avec `ajouter`
        (idempotent), `contient`, `octets`, `empreintes`, et `remplacer` qui
        **jette tout le contenu précédent** ;
      - `pub fn manquantes(annoncees: &[String], connues: &BTreeSet<String>)
        -> Vec<String>` — la règle que la plateforme applique aussi, écrite une
        fois du côté où elle est **pure**.
- [ ] Déclarer `pub mod magasin;` dans `apps.rs`, sans `cfg`.

**Test :** tests d'hôte dans le module.
**Ce qui le rend ROUGE — quatre états, tous atteignables :**
1. `ajouter` qui **accumule** au lieu de dédupliquer : deux ajouts de la même
   empreinte donnent deux entrées, et le test compte.
   🔴 **Ce test s'appuie sur un chiffre MESURÉ** : sur le corpus de cette VM,
   **153 applications rendent 99 PNG distincts** (M3), soit **54 téléversements
   évités**. Le test unitaire n'a pas les 153 icônes, mais il exerce la même
   règle sur un cas ×3 tiré de la mesure (l'empreinte `77852FCF…`).
2. `remplacer` qui fusionne au lieu de jeter : le magasin croît sans terme d'une
   réconciliation à l'autre. **Le test pose deux catalogues disjoints et
   vérifie que le premier a DISPARU.**
3. `manquantes` qui rend tout : le second passage retéléverse. **C'est la règle
   du critère ⑤.**
4. `manquantes` qui rend l'ensemble vide quand les connues sont vides : plus
   rien n'est jamais téléversé, en silence.

### Task 8 : `apps/icone/source.rs` — d'où vient l'icône d'un raccourci, PUR

- [ ] `pub enum Provenance { Module(String), Ico(String), Aucune }`.
- [ ] `pub fn provenance(icon_location: &str, cible: &str) -> Provenance` :
      découpe `"<chemin>,<index>"` sur la **dernière** virgule ; **chemin vide
      ⇒ c'est la CIBLE qui porte l'icône** ; extension `.ico` ⇒ `Ico`, `.exe` /
      `.dll` / `.mun` ⇒ `Module`, le reste ⇒ `Aucune`.
- [ ] `pub fn index(icon_location: &str) -> i32` — l'index, `0` à défaut.

🔴 **La règle « chemin vide ⇒ la cible » n'est pas un détail** : **92 des 153
raccourcis retenus de cette VM sont dans ce cas** (M1 ; la spec en relève 135
sur 218 avant filtrage). C'est exactement là que les icônes du produit
historique se perdaient — `convertToLinuxPath('')` rend la chaîne vide
(`src/lnkParser.js:172`, cité par la spec §4 D5).

**Test :** tests d'hôte, avec des cas **tirés du corpus versé**
(`agent/testdata/gapps-corpus-vm.json`) et des lignes verbatim de M1 —
`,0` (chemin vide), `C:\Program Files\GSmartControl\gsmartcontrol.ico`,
`%windir%\system32\notepad.exe`, `C:\Windows\Installer\{1BEA…}\ProductIcon`
(sans extension ⇒ `Aucune`).
**Ce qui le rend ROUGE :** un découpage sur la **première** virgule (un chemin
peut en porter une) ; un chemin vide traité comme `Aucune` au lieu de renvoyer
à la cible — **et ce dernier état ferait perdre son icône à 92 applications sur
153, en silence**.

---

# Famille 2 — l'agent, Windows

### Task 9 : `apps/icone.rs` — l'extraction Shell, l'encodage WIC, et `ICONES=0`

- [ ] `agent/Cargo.toml` : ajouter `"Win32_Graphics_Imaging"` **avec son
      commentaire de raison**, sur le modèle de `Win32_System_Environment`
      (G1) et de `Win32_System_DataExchange` (P1). ⚠️ **HYPOTHÈSE (E11)** :
      confirmer par la compilation croisée, et **ajouter ce qu'elle réclame
      avec sa raison** si elle en réclame.
- [ ] `agent/src/apps/icone.rs`, `#[cfg(windows)]` :
      - `pub fn extraire(lnk: &Path) -> Result<Vec<u8>>` —
        `SHCreateItemFromParsingName` sur le `.lnk`, puis
        `GetImage({256,256}, SIIGBF_ICONONLY)` ;
      - 🔴 l'encodage passe par **WIC** :
        `CreateBitmapFromHBITMAP(hbm, HPALETTE(0), WICBitmapUseAlpha)`, puis
        `CreateStream` + `InitializeFromMemory`, `CreateEncoder(&GUID_ContainerFormatPng, null)`,
        une frame, `Commit`. **Jamais `Image::FromHbitmap` ni son équivalent :
        il perd l'alpha** (spec §3.1) ;
      - `DeleteObject(hbm)` sur **tous** les chemins de sortie, y compris
        d'erreur ;
      - `pub fn armee() -> bool` — `OnceLock`, `!apps::desarme(std::env::var("ICONES").ok().as_deref())`
        (D15), et un `warn!("extraction d'icones DESARMEE (ICONES=0)")` **au
        premier appel seulement**.
- [ ] `apps/icone/lecture_pe.rs`, `#[cfg(windows)]` : rendre les octets bruts
      d'un `GRPICONDIR` — `LoadLibraryExW(LOAD_LIBRARY_AS_DATAFILE |
      LOAD_LIBRARY_AS_IMAGE_RESOURCE)`, `EnumResourceNamesW(RT_GROUP_ICON)`,
      `FindResourceW`, `SizeofResource`, `LoadResource`, `LockResource`,
      `FreeLibrary`. **Il ne DÉCIDE rien** : il rend un `Vec<u8>` que la tâche 6
      analyse.

**Test :** `cargo check --target x86_64-pc-windows-gnu` (types, emprunts,
visibilités, durées de vie — **et PAS l'édition de liens**, acquis de D3), puis
`cargo clippy --workspace`.
**Ce qui le rend ROUGE :** l'`unresolved import` de la fonctionnalité absente —
c'est **exactement** la forme sous laquelle G1 a découvert `Win32_System_Registry`
(E10 de G1). ⚠️ **Aucun test d'hôte ne peut couvrir ce module**, et c'est dit
plutôt que dissimulé : sa seule preuve de fonctionnement est la tâche 10 puis la
recette.

⚠️ **`LOAD_LIBRARY_AS_IMAGE_RESOURCE` s'ajoute à `AS_DATAFILE` et n'est pas
facultatif** : sans lui, `FindResourceW` sur un module chargé en pur fichier de
données ne trouve pas toujours ses ressources. **La sonde M1 les a employés tous
les deux**, et elle a rendu 110 sources lues sur 110 tentées.

### Task 10 : 🔴 LA MESURE DE DÉTERMINISME — et ce n'est PAS une porte éliminatoire

**Objet :** deux extractions successives de la **même** icône rendent-elles le
**même** SHA-256, sur WIC, dans **deux processus distincts** ?

- [ ] Un binaire de sonde — ou `ICONES` plus une trace, au choix de
      l'implémenteur — qui extrait **dix** icônes du corpus **deux fois, dans
      deux exécutions séparées de l'agent**, et journalise
      `icone extraite chemin=… empreinte=…`.
- [ ] Comparer les deux journaux, empreinte par empreinte.

🔴 **RÉDACTION DU VERDICT, ET ELLE EST CONTRAINTE.** *Le dépôt vient de payer un
faux verdict éliminatoire — une sonde a rendu trois zéros sur une machine saine
parce que rien n'avait encore eu lieu.* Donc :

- **VERDICT POSITIF** : les dix couples d'empreintes sont **présents** dans les
  deux journaux **et égaux deux à deux**. Un journal incomplet n'est pas un
  verdict positif.
- **VERDICT NÉGATIF** : au moins un couple est **présent des deux côtés ET
  DIFFÉRENT**. 🔴 **Une empreinte absente, un journal vide, une exécution qui
  n'a pas démarré ne sont PAS un verdict négatif** — ce sont des mesures non
  prises, et elles se déclarent comme telles.
- **NON MESURABLE** : tout le reste.

🔵 **UN VERDICT NÉGATIF NE BLOQUE PAS G2, ET C'EST POURQUOI CE N'EST PAS UNE
PORTE.** Il coûterait des retéléversements — le catalogue resterait juste, les
icônes resteraient servies, et **seul le critère ⑤ tomberait**. Le remède est
nommé d'avance et vit dans le module **PUR** de la tâche 6 : **ne prendre
l'empreinte que des chunks `IHDR`/`PLTE`/`IDAT`/`IEND`**, en écartant les
auxiliaires (`tIME`, `tEXt`, `pHYs`). Le découpage d'un PNG en chunks est
trivial et se teste sur l'hôte.

⚠️ **Ce que M3 établit déjà, et ce qu'il n'établit pas** : **GDI+ est
déterministe à l'intérieur d'une exécution** — 12 empreintes partagées, dont une
×27, n'auraient pas collisionné sinon. **Rien n'est mesuré de WIC, ni d'un
processus à l'autre.**

⚠️ **G2 N'A AUCUNE PORTE ÉLIMINATOIRE, ET C'EST UN FAIT, PAS UN OUBLI.** Les
trois inconnues qu'il aurait pu avoir sont fermées : le rendu du Shell est
mesuré (153/153, M3), la discrimination par la ressource est mesurée (M2), et le
corpus porte les deux valeurs (M1). **G3, lui, en a une** — la sonde UAC de la
spec §5 — et la contraste vaut d'être écrit : **fabriquer une porte là où il n'y
en a pas est le même défaut que d'en manquer une.**

### Task 11 : le câblage dans `boucle.rs`, et le legs n°7 de G1

- [ ] Dans `agent/src/apps/boucle.rs::reconcilier` : après avoir retenu une
      application, extraire son icône **si sa clé est neuve ou si son `chemin`
      a changé** — jamais à chaque tour. Sur `Ok`, poser
      `icone = Some(empreinte)` et `source_max` ; sur `Err`, poser
      `icone = None`, `source_max = NonMesuree`, **et une trace `warn!`** — une
      application sans icône vaut mieux qu'une application absente (spec §7).
- [ ] `source_max` vient de la **ressource** (tâches 6, 8, 9), jamais du PNG.
- [ ] Remplacer le magasin à chaque réconciliation (`Magasin::remplacer`).
- [ ] 🔴 **Fermer le legs n°7 de G1** : `retenus` (l. 109) vaut aujourd'hui
      `lancables.len()`, donc toujours `cles`. Compter les retenus dans la
      branche `Ok(())` de `raccourci::retenir` et émettre **ce** compteur.
- [ ] Ajouter au `tracing::info!("catalogue reconcilie")` :
      `icones` (extraites ce tour) et `icones_distinctes` (taille du magasin).

**Test :** les modules purs sont déjà couverts ; ici,
`cargo check --target x86_64-pc-windows-gnu`.
**Ce qui le rend ROUGE, et c'est mesurable en RECETTE :**
1. `retenus == cles` : l'état d'avant. Sur ce corpus, `retenus` doit valoir
   **167** et `cles` **154** (ou 153 — voir E4). **Deux nombres différents, donc
   un contrôle qui peut échouer.**
2. une extraction à **chaque** tour : `icones` reste à 153 toutes les trente
   secondes au lieu de retomber à 0. **C'est la rouge du critère ⑤ côté agent.**

### Task 12 : `apps/icone/televersement.rs` — le `PUT` HTTP, et la branche descendante

- [ ] `agent/src/plateforme.rs` : router `DepuisLaPlateforme::IconesManquantes`
      vers la boucle d'apps, sur le chemin exact de `Lancer` (déjà en place
      depuis G1).
- [ ] `agent/src/apps/icone/televersement.rs` : pour chaque empreinte demandée,
      `PUT <base>/icone/<empreinte>` avec les octets du magasin,
      `Authorization: Bearer <jeton d'agent>`, `Content-Type: application/octet-stream`.
- [ ] ⚠️ **Une empreinte demandée mais absente du magasin est SAUTÉE AVEC SA
      TRACE**, jamais une erreur : la réconciliation a pu changer entre
      l'annonce et la demande. C'est le même raisonnement que « une clé inconnue
      de `disparues` est ignorée » (`plateforme/src/apps/catalogue.ts:83-86`).
- [ ] ⚠️ **Le téléversement ne bloque pas la boucle de réconciliation** : il
      court sur le fil d'apps entre deux tours, comme les ordres de lancement.

**Test :** `cargo check --target x86_64-pc-windows-gnu`.
**Ce qui le rend ROUGE :** un `IconesManquantes` qui **abat** le fil au lieu
d'être traité — c'est le mode de défaillance du bras catch-all que ce dépôt a
payé quatre fois. ⚠️ **Vérifier, pas supposer** : la tâche 23 relance le
contrôle sur `capteur/pont_media.rs` (E15).

### Task 13 : 🔴 TÂCHE DÉDIÉE — `scripts/run-agent.sh` transmet `ICONES`

- [ ] Ajouter `${ICONES:+\$env:ICONES = '$ICONES'}` entre `APPS` (l. 38) et
      `MICRO_MESURE` (l. 39).
- [ ] **Rien d'autre. Aucune autre ligne de ce fichier n'est touchée.**

**Test :** lancer le script avec `ICONES=0` et **relire le `run-agent.ps1`
généré** sur `/media/vm/dev/`.
**Ce qui le rend ROUGE :** la ligne absente — le fichier généré ne porte pas
`$env:ICONES`. 🔴 **Le dernier saut, PowerShell → `agent.exe`, ne se vérifie
QUE sur la VM** : c'est la tâche 22 qui le fait, et G1 a mis douze tâches à
l'apprendre.

---

# Famille 3 — la plateforme, persistance

### Task 14 : `0005-icones.sql`, et la liste figée des migrations

- [ ] `plateforme/src/base/migrations/0005-icones.sql` : les deux colonnes
      **NULLABLES** de D13, avec l'en-tête qui **cite la mesure** (SQLite 3.50.4,
      `Cannot add a NOT NULL column with default value NULL` dès la première
      ligne) et l'invariant à trois cas.
- [ ] `plateforme/src/base/pilotes.test.ts:33` : `[1, 2, 3, 4]` → `[1, 2, 3, 4, 5]`.
- [ ] Ajouter au test de magnitude de `pilotes.test.ts` une ligne
      `application` portant `source_max_px = 256` **et** une portant `NULL`.

**Test :** `cd plateforme && npm run test:sqlite && npm run test:postgres`.
🔴 **LES DEUX PASSES, ET UN SAUT EST UN ÉCHEC** (règle de P1 §7.1) : si Postgres
manque, la passe échoue, elle ne se saute pas.
**Ce qui le rend ROUGE :**
1. la liste laissée à `[1,2,3,4]` : **rouge gratuite** (E8), à jouer.
2. `NOT NULL` sans défaut sur une base **peuplée** : SQLite refuse. ⚠️ **Sur une
   base NEUVE, la table est vide et cela PASSE** — c'est exactement le piège de
   la divergence E8 de G1. **Le test doit donc INSÉRER une application AVANT
   d'appliquer `0005`** pour que la rouge soit atteignable.
3. `ADD COLUMN … UNIQUE` : SQLite refuse (`Cannot add a UNIQUE column`),
   Postgres accepte. **Un seul des deux moteurs rougit** : c'est ce que la
   double passe existe pour attraper.

### Task 15 : `plateforme/src/apps/icones.ts` — le magasin de disque adressé par contenu

- [ ] `ouvrirMagasin(repertoire: string)` : crée le répertoire s'il manque,
      **journalise le chemin retenu** (D7).
- [ ] `possede(empreinte)` — une existence de fichier, **jamais une table**.
- [ ] `manquantes(annoncees: string[])` — le complément, dans l'ordre d'annonce.
- [ ] `ecrire(empreinte, octets)` — 🔴 **recalcule le SHA-256 par `node:crypto`
      et refuse s'il diffère** (D10) ; écriture **atomique** (temporaire puis
      `rename`).
- [ ] `lire(empreinte)` — les octets, ou `undefined`.
- [ ] 🔴 `empreinteValide(s: string)` — **exactement 64 caractères
      hexadécimaux minuscules**, et rien d'autre. **Sans elle, `:sha256` est un
      composant de chemin fourni par le réseau**, et `..` y est significatif.
      *(Piège nommé en D3 : « ne jamais interpoler une valeur d'environnement
      brute dans un composant de chemin » — ici c'est pire, elle vient d'un
      pair.)*

**Test :** `plateforme/src/apps/icones.test.ts`, sur un répertoire temporaire.
**Ce qui le rend ROUGE — quatre états, tous atteignables :**
1. 🔴 `ecrire` qui **ne recalcule pas** : le test qui dépose des octets sous une
   empreinte fausse **réussit** au lieu d'être refusé. **C'est la troisième des
   trois vérifications de la spec D7, et c'est ce qui rend l'adressage par
   contenu véritable.**
2. `empreinteValide` absente : le test qui demande `../../../etc/passwd` — ou
   `..%2f..` — **atteint** un fichier hors du magasin.
3. écriture non atomique : le test qui interrompt entre deux écritures laisse un
   fichier tronqué **sous un nom qui promet son contenu**.
4. `manquantes` déduit d'une liste en mémoire au lieu du disque : le test qui
   **supprime un fichier sous les pieds du magasin** ne le voit pas revenir dans
   les manquantes. **C'est la propriété du critère ⑦.**

### Task 16 : `PLATEFORME_ICONES` dans `config.ts`

- [ ] Un champ `repertoireIcones: string`, lu **ici et nulle part ailleurs**
      (commentaire de `config.ts`, l. 35-37).
- [ ] **Facultative, défaut `donnees/icones`**, avec le commentaire qui écrit
      **l'asymétrie assumée avec `PLATEFORME_HOTE`** (D7) : un mauvais
      répertoire coûte un retéléversement borné et automatique, là où un
      mauvais hôte exposerait le service.

**Test :** `plateforme/src/config.test.ts`.
**Ce qui le rend ROUGE :** la variable posée et ignorée — le test lit une valeur
explicite et la retrouve ; et le test du défaut lit `donnees/icones` sur un
environnement vide.

### Task 17 : les deux colonnes traversent le dépôt et la fusion

- [ ] `plateforme/src/depot/application.ts` : deux champs sur
      `LigneApplication`, **deux noms de plus dans `COLONNES`** (l. 66-67), et
      les deux dans l'`INSERT` **et** dans le `SET` de l'`UPDATE`. ⚠️ **Dans le
      `SET`, contrairement à `cle` et `apparue_a`** : une icône change quand
      l'application se met à jour, et c'est le cas nominal.
- [ ] 🔴 **Le nombre de `?` de l'`INSERT` doit suivre** : il en porte douze
      aujourd'hui (l. 133). **Deux de plus, et le test l'attrape parce qu'un
      `INSERT` mal compté LÈVE sur les deux moteurs.**
- [ ] `plateforme/src/apps/catalogue.ts` : les deux champs voyagent dans
      `aInserer` et `aMettreAJour` **sans que la règle change** — la fusion
      apparie sur la clé, et l'icône n'est pas une identité.
- [ ] La reconstruction `NULL` ⇄ `NonMesuree` est écrite **une seule fois**, au
      dépôt, avec l'invariant à trois cas de D13.

**Test :** `application.test.ts` et `catalogue.test.ts`, **double passe**.
**Ce qui le rend ROUGE :**
1. `source_max_px` reconstruit en `Pixels(0)` sur `NULL` : le test lit le cas
   `NonMesuree`. **C'est le critère ④ jusqu'au bout de la chaîne.**
2. la quatrième combinaison interdite (`icone` nul, `source_max_px` non nul)
   écrite par un chemin : le test la nomme.
3. les colonnes absentes du `SET` : une icône qui change n'atteint jamais la
   base, et **le test le voit en relisant après une seconde fusion**.

---

# Famille 4 — la plateforme, HTTP et canal

### Task 18 : 🔴 EXTRACTION de `routes-applications.test.ts`, AVANT l'addition

- [ ] `plateforme/src/http/routes-applications.test.ts` est à **480** lignes,
      marge **20** (E10). En extraire le harnais commun — base neuve, jeton,
      serveur — vers `plateforme/src/http/routes-harnais.ts`, sur le modèle de
      `plateforme/src/base/harnais.ts` et de `agents/canal-harnais.ts`, tous
      deux existants.
- [ ] **Aucune ligne de comportement. Le compte de tests est annoncé AVANT
      d'être mesuré**, et il doit être **identique** — **dix-sept**, chiffre que
      G1 a annoncé puis mesuré.

**Test :** `npm run test:sqlite`.
**Ce qui le rend ROUGE :** un test perdu — le compte annoncé.

### Task 19 : `routes-icone.ts` — les deux routes

- [ ] `PUT /icone/:sha256` : jeton **d'agent** ; `413` au-delà d'`ICONE_MAX_OCTETS`
      (1 Mio, **NON CALIBRÉE**, D8) ; `ecrire` recalcule ; `204` au succès.
- [ ] `GET /application/:id/icone?e=<empreinte>` : jeton **porteur**, `acces`
      (E5), `404 { refus: 'vm-inconnue' }` sur un refus, `400` sur un `e`
      absent, `404 { refus: 'icone-inconnue' }` sur un `e` qui ne correspond pas
      à l'empreinte courante ou sur un magasin qui ne l'a pas ;
      `200 image/png` avec
      `Cache-Control: private, max-age=31536000, immutable` (D11).
- [ ] 🔴 **Le chemin est découpé PAR SEGMENTS, jamais par `startsWith`.**
      `lancementDe` (`routes-applications.ts:96-102`) est le modèle exact, avec
      son commentaire : « un préfixe ouvrirait une famille entière de chemins
      que personne n'a décidés ».
- [ ] Servir `OPTIONS` (les deux routes exigent `Authorization`, donc la requête
      est NON SIMPLE ; un 404 sur l'`OPTIONS` ferait abandonner le navigateur
      avant la vraie requête).
- [ ] Étaler `ENTETES_SECURITE` **avant** `cors`, sur **toute** réponse, y
      compris les refus.

**Test :** `plateforme/src/http/routes-icone.test.ts`, double passe.
🔴 **LE CONTRÔLE DE CHEMIN COMPARE LE CORPS, JAMAIS LE SEUL STATUT**, et ce
n'est pas une précaution : G1 a mesuré qu'un `startsWith('/application')`
**laissait ses DIX-SEPT tests VERTS** — la route mangeait toute la famille et
rendait **son PROPRE 404 typé**, indiscernable du 404 générique tant qu'on ne
lisait que le statut (commit `a97f902`, message relu). Le test compare donc le
corps à `introuvable` (celui de `serveur.ts`), **sur au moins quatre chemins
déclinés**, dont deux qui n'existent pas dans la première rédaction :
`/iconedetournee`, `/icone/`, `/application/x/icone/y`, `/application/x/icone`
sans `?e=`.

**Ce qui le rend ROUGE — sept mutations, toutes à jouer APRÈS le vert :**
1. `startsWith` : ⚠️ **c'est celle qui a SURVÉCU en G1.** Si elle survit encore,
   **le test est faux, pas la mutation** — le réécrire, puis la rejouer.
2. `immutable` posé **sans** exiger `?e=` : le test qui demande une icône avec
   un `e` périmé reçoit `200` au lieu de `404`. **C'est la rouge de D11.**
3. `ecrire` sans recalcul : couvert tâche 15, **rejoué ici de bout en bout**.
4. la garde `empreinteValide` retirée : `PUT /icone/..%2f..%2fx` écrit hors du
   magasin.
5. `acces` non appelé sur le `GET` : **l'icône d'une VM d'autrui est servie**.
6. `403` au lieu de `404` sur une VM étrangère : **l'oracle d'énumération que le
   propriétaire du dépôt vient de retirer** (E5) — le test compare le corps,
   pas seulement le code.
7. le jeton d'agent accepté sur le `GET`, ou le jeton porteur sur le `PUT` : les
   deux tests symétriques tombent.

### Task 20 : chaîner le routeur, et l'`it()` de sécurité

- [ ] `plateforme/src/http/serveur.ts` : une ligne, après
      `servirApplications` (l. 223).
- [ ] 🔴 `plateforme/src/http/entetes-routeurs.test.ts` : **un `it()` pour le
      routeur neuf.** Son en-tête l'exige en toutes lettres (l. 3-18, relu) :
      « UN `it()` PAR ROUTEUR, ET JAMAIS UN TEST GLOBAL », et il nomme le
      précédent — **« G1 vient d'ajouter un routeur sans que personne ne s'en
      aperçoive côté P5 : c'est précisément le mode de défaillance que ce
      fichier rend visible ».** ⚠️ **G2 ajoute le sixième. Ne pas rejouer le
      défaut que ce fichier existe pour empêcher.**

**Test :** `npm run test:sqlite`, `npm run test:postgres`, `npm run typecheck`.
**Ce qui le rend ROUGE :** retirer l'étalement d'`ENTETES_SECURITE` du **seul**
routeur neuf — **son** `it()` tombe, les cinq autres restent verts. C'est la
rouge que ce fichier prescrit.

### Task 21 : `agents/canal.ts` — pousser `IconesManquantes`

- [ ] Dans la branche `catalogue` (l. 214-255), après la fusion : demander au
      magasin les manquantes parmi les empreintes annoncées, et pousser
      `encodeIconesManquantes` **si et seulement si l'ensemble n'est pas vide**
      (D9).
- [ ] ⚠️ **`void … .catch(…)`, comme `marquerVu` et le catalogue lui-même**
      (l. 204-206, l. 248-254) : un `await` ferait qu'une base ou un disque
      momentanément indisponible **ABATTRAIT LA CONNEXION** d'un agent qui va
      très bien, et une promesse rejetée sans `catch` abattrait tout le process.
- [ ] ⚠️ **Aucun enrôlement, aucune icône** : la garde `vmId === undefined`
      existante (l. 215-224) couvre déjà le cas, et il ne faut pas la
      contourner.

**Test :** `plateforme/src/agents/canal-apps.test.ts` (existant), double passe.
**Ce qui le rend ROUGE :**
1. le message poussé **même quand l'ensemble est vide** : le test compte les
   messages sortants sur un second catalogue identique et en trouve un de trop.
   **C'est le critère ⑤ côté plateforme.**
2. un `await` au lieu du `void … .catch` : le test qui fait échouer le magasin
   voit **le socket se fermer**.
3. les manquantes déduites de la base au lieu du disque : le test qui vide le
   répertoire ne voit rien redemandé. **Critère ⑦.**

---

# Famille 5 — recette, revue, et clôture

### Task 22 : 🔴 LA RECETTE, sur la VM Windows

**C'est la SEULE tâche de recette de G2.** Elle emploie la VM, et
**l'agent concurrent y joue une porte éliminatoire** : vérifier
`virsh list --all` puis `Get-Process agent` **avant ET après chaque tentative,
y compris échouée** (piège de D8, rencontré trois fois sur trois : un
superviseur laissé vivant empêche le `StreamWriter` d'ouvrir `agent.log`, et
**la copie relue est celle, périmée, de la tentative précédente**).

**Préparation, dans cet ordre :**

- [ ] `set -a && source .env && set +a`, sans quoi `build-agent.sh` **s'arrête
      en silence** après « sources synchronisées » et l'on mesure le binaire
      précédent.
- [ ] Tuer tout agent **avant** de rebâtir : le binaire ne peut pas être réécrit
      tant qu'un agent tourne (`Accès refusé (os error 5)`).
- [ ] Relever la **taille** du binaire bâti. ⚠️ **Une compilation de 0,13 s est
      un aveu** : après un aller-retour de sources, seul
      `cargo clean --release -p proto -p agent` débloque.
- [ ] Copier les deux `.ico` témoins sur la VM et y **créer deux raccourcis
      Bureau** qui les portent en `IconLocation` — c'est ce qui fait entrer les
      témoins dans le corpus du produit, et donc dans le critère ②.
- [ ] Supprimer `agent.log` **depuis Windows**, jamais depuis l'hôte : un `rm`
      qui échoue laisse lire un journal périmé mélangé au neuf.
- [ ] Vérifier que le registre d'affichage ne bloque pas les ouvertures de
      fenêtre (legs n°4 de D9) — sans objet pour G2, qui n'ouvre aucune fenêtre,
      **et écrit ici pour qu'on ne le cherche pas ailleurs**.

**Les critères. DEUX EXÉCUTIONS CHACUN, jamais une. AUCUN TAUX.**

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE, **et cet état est-il atteignable ?** |
| --- | --- | --- | --- |
| ① | Le PNG rendu fait **256×256** et porte un **canal alpha non trivial** sur la très grande majorité des applications | dimensions relues depuis les octets servis par `GET …/icone` ; alpha = au moins un octet A ni 0 ni 255 | 🔴 **CE CRITÈRE NE VAUT RIEN SUR LA TAILLE** — M2 le prouve deux fois. Il ne vaut que pour l'**alpha**, et sa rouge est réelle : `WICBitmapUseAlpha` → `WICBitmapIgnoreAlpha`, une constante nommée, qui fait tomber le compte à **zéro**. ⚠️ **NE PAS EXIGER « TOUTES »** : mesuré **149 sur 153** (M3), donc quatre n'en ont pas et cet état-là **n'est pas atteignable** (E3) |
| ② | `source_max` **distingue** un vrai 256 d'un agrandissement | les deux raccourcis témoins : celui du `.ico` 48 rend `{"pixels":48}` **avec un PNG de 256×256** ; celui du 256 rend `{"pixels":256}` | 🔴 **ROUGE GRATUITE, ET DÉJÀ JOUÉE — M2, transcrite verbatim plus haut.** Tout code qui déduirait `source_max` de la taille rendue donnerait `256` **aux deux**. ⚠️ **« Déjà mesuré » ne dispense PAS de jouer la rouge du TEST** : ce qui est mesuré est le fait du monde, ce qui reste à voir rouge est que le contrôle le dénonce |
| ③ | Le corpus réel porte **les deux valeurs** | `GET /applications` : au moins une à 256, au moins une en dessous, au moins une `non-mesuree` | 🔵 **MESURABLE, et mesuré d'avance (M1) : 61 / 55 / 37.** Le risque n°3 du §11 de la spec est levé. ⚠️ **Ne pas exiger que les nombres du produit soient ceux de M1** : la sonde n'est pas le produit (portée de M1) |
| ④ | `NonMesuree` n'est **jamais** rendue comme un nombre | la charge JSON de `GET /applications` porte `"non-mesuree"` ; la colonne porte `NULL` | représenter `NonMesuree` par `0` ou par `256`. **Atteignable** : c'est une mutation d'une ligne, et la chaîne est éprouvée à trois étages (proto, dépôt, route) |
| ⑤ | Une icône déjà connue n'est **pas retéléversée** | deux réconciliations consécutives ; la seconde ne transporte **aucun** octet d'icône | omettre `IconesManquantes`, ou la pousser vide. 🔴 **Le témoin est un COMPTE D'OCTETS SUR LE FIL, pas une absence de ligne de journal** — G1 a mesuré la taille d'un `Catalogue` par `tshark` sur `internalBridge`, et c'est le même instrument |
| ⑥ | **La déduplication paie sur le corpus réel** | le compte d'empreintes distinctes contre le compte d'applications | 🔵 **MESURÉ D'AVANCE : 99 distinctes pour 153 applications, 54 téléversements évités (M3).** La rouge est une empreinte tirée du chemin du `.lnk` au lieu du contenu : le compte remonte à 153 |
| ⑦ | Le magasin **se reconstruit tout seul** après une perte | vider `PLATEFORME_ICONES`, attendre une réconciliation, relire | 🔴 **C'est la propriété qui rend le disque acceptable (D7), et elle doit être ÉPROUVÉE, pas supposée.** La rouge est un inventaire déduit de la base : rien n'est redemandé, et les icônes sont perdues **pour toujours** |
| ⑧ | Un agent **v2** face à une plateforme **v3** RENONCE, et le dit | le binaire d'avant G2, conservé, contre la plateforme de G2 | 🔴 **ROUGE GRATUITE**, et c'est la **première** occasion de prouver la correction du 20 août (E6). Attendu : **au moins une** ligne `la plateforme REFUSE la version du canal /agent : aucune reprise` portant `version_emise=2 version_recue=3`, et **zéro** `reprise du canal /agent` après elle. ⚠️ **Au moment de G1, ce même montage rendait 0 et 10** — c'est le contraste qui fait la preuve |

**Relevés annexes à prendre, et à verser :**

- [ ] la **durée** d'une réconciliation **avec** icônes, au premier tour et à
      chaud, à opposer aux **84 / 92 ms** de G1 et aux **2 298 ms** de M3 ;
- [ ] le **dernier saut d'`ICONES`** : `run-agent.ps1` généré porte-t-il
      `$env:ICONES` ? l'agent journalise-t-il son désarmement ? **combien de
      lignes `icone extraite` sur deux périodes ?** (piège payé cinq fois) ;
- [ ] le legs n°7 de G1 : `retenus` vaut-il **167** quand `cles` vaut **154** ?
- [ ] le **poids dédupliqué** du magasin sur le disque de la plateforme — **il
      n'a été mesuré nulle part** (M3 ne somme que les 153 non dédupliqués) ;
- [ ] la **taille du `Catalogue` sur le fil** avec les deux champs neufs, à
      opposer aux **56 145 octets** que G1 a mesurés par `tshark` ;
- [ ] `Get-Process agent` et `virsh list --all` **après** la dernière exécution.

**Journaux :** versés sous
`docs/superpowers/plans/journaux-gestion-apps-g2/`, **UTF-8, séquences ANSI
retirées** (un `-plat.log` par journal brut), et **le contrôle de la tâche 25
vérifie qu'ils sont suivis par git** — la spec §11 relève que les sondes vivent
dans `C:\dev\`, « c'est-à-dire nulle part de durable ».

⚠️ **Pièges d'outillage à relire AVANT de commencer** : le jeton d'accès expire
en quelques minutes (le redemander à **chaque** `curl`) ; `AGENT_VM` attend
l'**identifiant**, pas le nom ; `POST /auth/connexion` attend `motdepasse` en un
mot ; `unset -f chpwd` avant tout relevé ; et la VM **s'hiberne toute seule** —
vérifier `virsh list --all` après toute séquence longue.

### Task 23 : 🔴 LA REVUE TRANSVERSE DE FIN DE BRANCHE — obligatoire

**Barème des sous-blocs antérieurs, pour dimensionner l'effort** : **douze** en
D10, **treize** en S3, **vingt-sept** en S4, **huit** en G1 (sur seize places),
**dix-sept** dans le presse-papier P1, **neuf** en P5, **onze** en F1.

**Sa cible propre est nommée d'avance** : les affirmations de code et de
document que **G2 lui-même** rend fausses, et celles que **l'arbre a rendues
fausses depuis G1**. Une revue par tâche ne peut structurellement pas les voir —
la tâche qui écrit la phrase et celle qui la réfute ne se relisent jamais l'une
l'autre.

- [ ] **Les affirmations que G2 rend fausses.** Balayer au moins :
      `proto/src/plateforme.rs` (l'encadré « LA VM SE TAIT SANS BOUCLER », déjà
      annoté tâche 3 — vérifier qu'il l'est) ; le commentaire de
      `IssueLancement` sur la lacune de nommage (**il reste VRAI** — le
      vérifier plutôt que le supposer) ; `apps/boucle.rs` et son `retenus` ;
      `0004-applications.sql` (« toutes les colonnes NOT NULL naissent ici » —
      **G2 en ajoute deux, NULLABLES : la phrase reste vraie, le vérifier**).
- [ ] 🔴 **Les affirmations que l'ARBRE a rendues fausses depuis G1**, et elles
      sont **déjà relevées** : E5 (`403`/`404`, tranché par `1976f2f`), E6 (le
      refus hors versionnement, `457a7f8`), E7 (le pont, `AGENT_JETON`).
      **Le document de résultats de G1 (§10, §11 legs 1/2/3) et la section G1 de
      `CLAUDE.md` les déclarent toujours ouverts.** ⚠️ **Les ANNOTER, pas les
      réécrire** : ce sont des relevés datés, et les barrer les rendrait faux
      comme histoire.
- [ ] 🔴 **Balayer par le SENS, pas par la formule.** Une négation se dit de
      plusieurs façons, et **c'est celle qu'on n'a pas listée qui survit** —
      « non tranchée / pas tranchée / jamais / reste ouverte / n'est fermée par
      rien ». Le naufrage du « 487 » s'est rejoué **neuf fois** dans ce dépôt.
- [ ] 🔴 **Corriger une affirmation exige de la CHERCHER, pas de la corriger là
      où on nous l'a montrée.** Énumérer les places par
      `grep -n '<la chose>' <fichier>` **AVANT** d'écrire, **et relire place par
      place APRÈS l'édition** : une substitution qui ne dit pas combien
      d'occurrences elle a touchées est une affirmation de complétude non
      vérifiée.
- [ ] Re-jouer le contrôle E15 : `IconesManquantes` ne traverse **aucun** bras
      catch-all. Le vérifier par la commande, pas le supposer.
- [ ] Vérifier par la commande qu'aucun fichier touché par les commits `(g2)`
      n'est sous `agent/src/capteur/`, `agent/src/superviseur/`, `src/`, `web/`
      ni `client/`.

### Task 24 : `CLAUDE.md` — la section G2, et la dette qui DISPARAÎT

- [ ] Écrire la section « Sous-projet ④ Gestion d'apps — sous-bloc G2 », sur le
      modèle de celle de G1 : le verdict, les critères **avec leur nombre
      d'exécutions**, la variable `ICONES`, les relevés annexes, les pièges
      neufs, ce que G2 n'établit pas, et ses legs.
- [ ] 🔴 **RETIRER `proto/src/plateforme/tests.rs` et `proto/ts/plateforme.test.ts`
      DU TABLEAU DE DETTE**, et dire par quoi : G2 les a résorbés (D2). ⚠️ **Une
      dette purgée qui reste écrite est aussi trompeuse qu'une dette non
      écrite** — c'est la symétrie exacte du legs n°6 de P1 (« le legs n'est pas
      la dette, c'est que personne ne l'avait vue »).
- [ ] 🔴 **Les tailles sont RELEVÉES PAR LA COMMANDE, APRÈS la dernière édition
      de la ronde — revue transverse comprise.** Une table relevée en début de
      ronde serait fausse à la fin de la même ronde ; **D8 a commis cette erreur
      en croyant bien faire.**
- [ ] 🔴 **Toucher une ligne d'un tableau de comptes OBLIGE à remesurer son
      compte**, même quand ce n'est pas l'objet de l'édition. C'est la doctrine
      que D10 a ajoutée après ses septième et neuvième naufrages.

### Task 25 : le document de résultats

- [ ] `docs/superpowers/plans/2026-08-20-gestion-apps-g2-resultats.md`, sur le
      modèle de celui de G1 : le verdict en faits qui ne se simplifient dans
      aucun sens, les critères avec leur nombre d'exécutions, les rouges jouées
      **avec leur forme exacte**, les relevés annexes, ce que G2 n'établit pas,
      et ses legs.
- [ ] 🔴 **Le contrôle qu'aucune preuve ne vit hors de git** :
      `git ls-files docs/superpowers/plans/journaux-gestion-apps-g2 | wc -l`, et
      la liste des journaux avec ce que chacun porte. **La leçon est de D10** :
      l'espace de travail de D9 était gitignoré, il a disparu, et **six constats
      de revue sont définitivement perdus**.
- [ ] ⚠️ **Ne renvoyer vers AUCUN rapport de tâche** : ils vivent dans
      `.superpowers/sdd/`, qui est gitignoré. Ce que G2 veut garder, il l'écrit
      **ici**.

---

## Ce que G2 n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, **une
  seule** pour chacune des trois mesures M1, M2 et M3 de ce plan.
- **Aucun jugement visuel.** `source_max` dit **d'où vient l'image**, jamais si
  elle est bonne. La spec §3.2 relève un 256 authentique de **3 451 octets** et
  un de **72 024** : les deux sont vrais. C'est la lacune exacte que `BPP_MIN`
  traîne depuis le chantier C volet 1, et que `FACTEUR_FOCUS`,
  `PART_DORMANTE_BPS`, `TAILLE_MAX_SORTIE` et toutes les autres traînent après
  lui.
- **Aucune constante calibrée** : `ICONE_MAX_OCTETS` (majorante à vue),
  le défaut de `PLATEFORME_ICONES`, et la règle « extraire quand la clé est
  neuve ou que le `chemin` a changé » — dont **le taux de réextraction inutile
  n'est mesuré par rien**.
- **Rien d'une icône qui CHANGE sans que le raccourci change.** Une mise à jour
  d'application qui réécrit son `.exe` en place, même chemin, même icône
  déclarée mais image différente, **ne sera pas revue** : la règle
  d'extraction ne relit pas le disque. **C'est un trou nommé, pas un oubli** —
  le fermer demanderait un horodatage ou une empreinte de la source, donc un
  accès disque par application et par tour.
- **Rien du HiDPI ni d'un client réel** : la recette reste `curl` et, au mieux,
  un Chromium sans interface. **Aucune icône n'est REGARDÉE.**
- **Rien d'une icône servie sans jeton**, et donc **rien du manifeste PWA** :
  D11 nomme la contrainte, G5 la tranchera.
- **Rien de la charge** : ni le nombre de téléversements d'icônes simultanés,
  ni le comportement du magasin à plusieurs VMs. **Une seule VM, un seul
  catalogue de 218 raccourcis.**
- **Rien de la latence de bout en bout**, qu'aucun sous-bloc du chantier D ni du
  sous-projet ⑤ n'a jamais mesurée.
- **Rien d'un antivirus** : l'état de celui de la VM n'a **pas** été relevé, ni
  par la spec, ni par G1, ni ici.
- **Rien des applications UWP/MSIX**, ni des sept raccourcis de l'espace de
  noms Shell : hors périmètre v1 (spec §10).
- **Le nettoyage du magasin n'existe pas.** Une icône dont plus aucune
  application ne porte l'empreinte **reste sur le disque, pour toujours**. Le
  ramasse-miettes demanderait de balayer toutes les VMs ; **il est nommé, pas
  écrit.**
- **L'écart `153` / `154` n'est pas expliqué** (E4).
- **Le magasin n'est pas partagé entre plateformes**, et il ne survit pas à un
  conteneur sans volume — **il se reconstruit** (critère ⑦), ce qui est une
  propriété différente et plus faible.

---

## Risques, et ce qui rendrait G2 NON LIVRABLE

| Risque | Ce qu'il coûte | Mitigation, ou constat |
| --- | --- | --- |
| ⚠️ **`Win32_Graphics_Imaging` ne suffit pas** (E11) | une ronde de compilation | **précédent EXACT en G1** : `Win32_UI_Shell` seule était annoncée, il fallait aussi `Win32_System_Registry`. **La liste est une hypothèse, la compilation tranche**, et une fonctionnalité de plus reste une fonctionnalité d'un crate déjà présent |
| ⚠️ **WIC n'est pas déterministe** (D6, tâche 10) | le critère ⑤ tombe, et l'agent retéléverse à chaque tour | 🔵 **NON ÉLIMINATOIRE, et c'est établi plutôt qu'espéré** : le catalogue reste juste, les icônes restent servies. **Remède nommé d'avance**, dans le module PUR : n'empreindre que `IHDR`/`PLTE`/`IDAT`/`IEND` |
| 🔴 **Un `<img>` ne porte pas d'`Authorization`** (D11) | **G5 ne peut pas pointer cette route depuis un manifeste PWA** | **nommé ici plutôt que découvert là-bas.** G2 ne le tranche pas : trancher demanderait de décider si une icône se sert sans jeton, ce qui est une décision de sécurité et appartient au propriétaire du dépôt |
| ⚠️ **Le magasin de disque disparaît** (conteneur sans volume) | tout est retéléversé | **c'est le critère ⑦**, et c'est ce qui rend le choix du disque acceptable. ⚠️ **La reconstruction est bornée par la réconciliation, pas immédiate** |
| ⚠️ **Le corpus de recette n'est plus celui de M1** | les critères ③ et ⑥ mesurent autre chose | **les nombres du produit ne sont PAS exigés égaux à ceux de M1** (portée de M1). Ce qui est exigé est la **forme** : deux valeurs présentes, et moins d'empreintes que d'applications |
| ⚠️ **La VM est occupée par un chantier concurrent** | la recette attend | **une seule tâche de G2 l'emploie**, et elle vérifie `Get-Process agent` avant et après **chaque** tentative |
| 🔴 **`PLATEFORME_VERSION = 3` casse tout agent déployé** | la VM ne s'établit plus | **ASSUMÉ, comme P3 D5 et G1 D10.** ✅ **Et depuis le 20 août, la panne est DIAGNOSTICABLE** (E6) : l'agent lit le refus, le journalise avec les deux versions, et **renonce** au lieu de boucler. **Le critère ⑧ le prouve, et sa rouge est gratuite** |
| ⚠️ **Une icône de plus de 1 Mio est refusée** | l'application entre au catalogue **sans** icône | `ICONE_MAX_OCTETS` est **majorante à vue et NON CALIBRÉE** — **aucune taille individuelle n'a été relevée**, seulement une moyenne de 29 911 octets sur 153. Le refus est **typé et journalisé**, jamais silencieux |
| ⚠️ **Trois extractions de dette dans un arbre partagé** | un conflit | **`git add` NOMINATIF**, jamais `-A`. Les extractions ne touchent que `proto/` et `plateforme/src/http/`, et **aucun chantier concurrent n'y travaille** (relevé par `git log -- proto/`) |

🔵 **Ce qui rendrait G2 NON LIVRABLE, en une phrase : rien de connu.** Les trois
inconnues qu'il aurait pu porter sont **fermées par la mesure** — le Shell rend
153 icônes sur 153 (M3), la ressource discrimine (M2), et le corpus porte les
deux valeurs (M1). **G2 n'a donc aucune porte éliminatoire, et le dire est plus
honnête que d'en fabriquer une.** Le seul risque qui pourrait imposer un repli
est la détermination de WIC, et son repli est écrit, borné, et vit dans le
module pur.

---

## Contrôle final, avant de déclarer G2 clos

```bash
# 🔴 DEPUIS UN SHELL PROPRE, ou `env -u TURN_URL -u TURN_SECRET` :
# verify-all.sh N'EST PAS HERMÉTIQUE (legs n°5 de G1).
./scripts/verify-all.sh

# La règle des 500 lignes, relancée APRÈS la dernière édition de la ronde.
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

**Attendu, et à VÉRIFIER plutôt qu'à supposer :** le tableau de dette **retombe
à DEUX lignes** — `agent/src/encode.rs` et `agent/src/windows_source.rs` —, les
deux fichiers de `proto/` en étant **sortis** (D2). ⚠️ **Si un troisième
apparaît, il s'écrit** : une dette non écrite est une dette qu'on découvre.
