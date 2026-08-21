# Sous-projet ④ — Gestion d'apps

**Date** : 19 août 2026
**Cadrage parent** : `2026-07-27-refonte-produit-design.md`, §5 ④ (« Gestion
d'apps »), son amendement du 28/07/2026 sur les file handlers, §6 (« Installation
d'app »), §7 (remontée du code de sortie et des journaux), §9.3 (ordre de
construction)
**Amont** : `2026-08-19-plateforme-design.md` §3.3 (le canal plateforme ↔ agent),
§5 (la table `application`)
**Statut** : conception, plan à écrire
**Portée** : téléversement d'installeur, découverte des applications, extraction
d'icônes haute résolution, et le lancement qui rend le catalogue utile

---

## 1. Objet, et ce que ce document tranche

Le cadrage décrit ④ en quatre puces (§5 ④). Elles portent **trois inconnues
techniques dont aucune n'avait été mesurée** avant ce document :

1. par quel mécanisme Windows surveille-t-on le Bureau et le menu Démarrer, et
   **que se passe-t-il quand une notification est perdue** ;
2. quelle API Shell remplace le parseur `.lnk` maison, et **rend-elle vraiment
   les cinq champs dont on a besoin** ;
3. par quelle API obtient-on une icône 256×256, et — la vraie question —
   **comment sait-on que c'en est une, et non un agrandissement**.

Les trois ont été mesurées sur la VM le 19 août 2026, en lecture seule, avant
d'écrire une ligne de décision. Les relevés sont **transcrits verbatim au §3**,
pas résumés : le sous-bloc D10 a établi que la preuve d'une affirmation de ce
dépôt ne doit jamais vivre dans un fichier gitignoré, et les sondes de ce
document vivent dans `C:\dev\` sur la VM, c'est-à-dire nulle part de durable.
**Ce document EST leur seule trace.**

**Ce document ne modifie aucun code.** Chaque affirmation sur l'existant porte
son `fichier:ligne`, relu après avoir été écrit.

⚠️ **Deux fichiers cités ici étaient en cours de modification par un chantier
concurrent pendant la rédaction** : `agent/src/superviseur/protocole.rs` (un
`git status` le donnait modifié) et l'ensemble d'`agent/src/plateforme*`,
apparu en cours de rédaction au commit `09d7adf`. **Pour ces deux-là je cite
des SYMBOLES, jamais des numéros de ligne** — et j'ai déjà payé la leçon
**deux fois** dans cette session :

- le §3.4 de la spec plateforme donne `SESSION_DE_CONTROLE` à
  `protocole.rs:17` ; une première lecture me l'a rendu à `:26` ; et à la
  relecture cinq minutes plus tard la ligne 26 était un `///` et la constante
  s'appelait `NOM_SESSION_DE_CONTROLE` à la ligne 32 ;
- `agent/src/plateforme.rs` a été écrit **282** lignes au §3.3 et au §6 de ce
  document, et il en faisait **277** au moment de committer. **Corrigé aux deux
  places, énumérées par `grep -n` AVANT d'écrire** — c'est le geste que
  `CLAUDE.md` exige après le naufrage du « 487 », rejoué neuf fois dans ce
  dépôt.

**Les comptes de lignes de ce document sont donc datés du 19 août 2026 et
DÉRIVERONT. La seule source de vérité est la commande de `CLAUDE.md`.**

---

## 2. Ce qu'on remplace, et ce que ça a coûté

L'ancien système est encore là et fonctionne (cadrage §11). Il est lu ici pour
ses **modes de défaillance constatés** et pour la **liste des données réellement
nécessaires** — jamais comme modèle d'architecture.

| Ce qu'il fait | Où | Ce que ça coûte |
| --- | --- | --- |
| Découverte par **balayage horaire** | `src/app.js:152` — `setInterval(update, 1000 * 60 * 60)` | une application installée est invisible pendant **jusqu'à une heure**. Le cadrage exige des notifications de changement |
| Découverte du **Bureau seul** | `src/app.js:54` — `` `/media/vm/Users/${…}/Desktop` `` | le menu Démarrer est ignoré. Sur cette VM il porte **203 des 218** raccourcis (§3.1) |
| Lecture du `.lnk` par un **parseur maison** | `src/lnkParser.js` (200 l.) | ses commentaires nomment eux-mêmes ses trous : « LinkInfo sometimes only returns "C:\\" which is incomplete » (`src/lnkParser.js:90`), d'où un repli sur `RELATIVE_PATH` (`:91-95`), et une table de substitution d'environnement écrite à la main, **quatre variables et pas une de plus** (`:191-194`) |
| Chemins convertis en **chemins Linux** via CIFS | `src/lnkParser.js:171` `convertToLinuxPath` | tout le pipeline dépend du montage `/media/vm`. Le nouveau produit n'en a pas : l'agent lit **dans** la VM |
| Icônes par **cinq méthodes en cascade** | `src/iconExtractor.js:41-131` | quatre échouent en pratique (CLAUDE.md § « Extraction d'icônes ») ; la cinquième, retenue, est `[System.Drawing.Icon]::ExtractAssociatedIcon` (`src/iconExtractor.js:168`), **qui ne rend que du 32×32 ou 48×48**, agrandi ensuite en 512 par `sharp` (`:138-145`) |
| Identifiant d'app dérivé du **nom** | `src/app.js:67` — `name.replace(/\s*[0-9\.]/g, '')…` | les chiffres sont **supprimés** : « Nsight 2020.3 » et « Nsight 2024.6 » produisent le même `short`. Ce n'est pas un identifiant |
| `file_handlers` dynamiques par app | `src/asset.js:82-98` | 🔵 **la seule pièce à garder**, et le cadrage le dit déjà (amendement du 28/07) : le mécanisme existe, seule reste la déclaration des types installeur |

⚠️ **`src/app.js:106` appelle `getAppFileAssociations` PAR APPLICATION**, et
chaque appel est un aller-retour WinRM qui balaie tout `HKCU\…\FileExts`
(`src/app.js:19-23`). C'est la partie la plus lente du balayage horaire, et elle
disparaît avec WinRM.

---

## 3. Trois relevés sur la VM, pris le 19 août 2026, en lecture seule

**Méthode** : chaque sonde est un `.ps1` écrit sur `/media/vm/dev/` et invoqué
par `powershell -ExecutionPolicy Bypass -File C:\dev\<nom>.ps1` — jamais en
ligne de commande, parce que `nodejs-winrm` enveloppe la commande dans
`powershell -Command "& { … }"` et qu'un guillemet interne y entre en collision
(piège documenté dans `CLAUDE.md`, sous-bloc D3). Chaque sonde écrit son
résultat dans un fichier UTF-8 relu depuis l'hôte : **le pipeline d'encodage
PowerShell n'est jamais sur le chemin de la mesure.**

### 3.1 Ce que rend l'API Shell sur les `.lnk` réels de cette VM

Sonde `C:\dev\gapps-sonde-lnk.ps1` — `WScript.Shell.CreateShortcut`, qui est
l'enveloppe COM d'`IShellLinkW`, sur les quatre racines. **Une exécution.**

```
=== DOSSIER C:\Users\Administrateur\Desktop
   nombre=7
=== DOSSIER C:\Users\Public\Desktop
   nombre=8
=== DOSSIER C:\Users\Administrateur\AppData\Roaming\Microsoft\Windows\Start Menu
   nombre=26
=== DOSSIER C:\ProgramData\Microsoft\Windows\Start Menu
   nombre=177
```

**218 raccourcis**, dont **203 dans le menu Démarrer** que l'ancien système
n'ouvre jamais. Extrait verbatim de deux entrées :

```
  --- C:\Users\Administrateur\Desktop\Steam.lnk
      TargetPath=C:\Program Files (x86)\Steam\steam.exe
      Arguments=
      WorkingDirectory=C:\Program Files (x86)\Steam
      IconLocation=,0
      WindowStyle=1
  --- C:\Users\Administrateur\Desktop\Paramètres Windows.lnk
      TargetPath=
      Arguments=
      WorkingDirectory=
      IconLocation=C:\Windows\ImmersiveControlPanel\SystemSettings.exe,0
      WindowStyle=1
```

Dépouillement des 218 entrées (script Python sur le fichier de sonde) :

| Fait | Valeur | Ce qu'il impose |
| --- | --- | --- |
| cibles par extension | `.exe` **170**, `.msc` 15, **vide 7**, `.url` 7, `.html` 5, `.pdf` 4, `.chm` 3, `.txt` 3, `.bat` 1, `.msi` 1, `.2` 1 | un raccourci n'est pas une application : il faut une règle de filtrage, et elle doit être **écrite**, pas devinée |
| `TargetPath` **vide** | **7** — `Paramètres Windows`, `computer`, `Control Panel`, `File Explorer`, `Run`, et deux pages web de `smartmontools` | ce sont des cibles de l'espace de noms Shell (PIDL), sans chemin de fichier. **Une cible vide n'est pas une erreur** |
| `IconLocation` **sans chemin** (commence par `,`) | **135 sur 218** | l'icône vient de la **cible**, pas du raccourci. `convertToLinuxPath('')` du legacy rend `''` : c'est là que ses icônes se perdent |
| cibles `.exe` **absentes du disque** | **3** | la troisième règle de filtrage de D3 n'est pas théorique |
| raccourcis **retenus** par les trois règles de D3 | **167** sur 218 | c'est la population sur laquelle portent les deux lignes suivantes |
| clés `(cible, arguments, répertoire)` distinctes | **154** | c'est le compte d'applications attendu sur cette VM |
| clés `(cible seule)` distinctes | **104** | 🔴 **l'écart de 50 entre 104 et 154 EST la décision d'identité** : 26 raccourcis de `smartmontools` visent tous `runcmdu.exe` avec des **arguments différents**. Une identité par cible seule les fondrait en une |
| désinstalleurs (nom ou cible) | **15** | ils ressemblent à des applications et n'en sont pas |
| cibles sous `C:\Windows` | **63** | dont Bloc-notes et Paint : **un filtre par chemin système serait faux** |

**Quatre lignes de ce tableau — « absentes du disque », « retenus », et les deux
comptes de clés — ne viennent PAS du dépouillement Python** mais de deux sondes
distinctes qui appliquent les **trois** règles de filtrage de D3 avant de
compter. Verbatim, `C:\dev\gapps-sonde-existence.ps1` puis
`C:\dev\gapps-sonde-cle2.ps1` :

```
lnk total = 218
cibles .exe non vides = 170
dont cible ABSENTE du disque = 3
cles distinctes apres les TROIS regles de filtrage = 154
```

```
raccourcis RETENUS par les trois regles = 167
cles (cible, arguments, repertoire) = 154
cles (cible seule)                   = 104
```

⚠️ **170 − 3 = 167 : les deux sondes concordent**, et c'est le seul contrôle
croisé de ce paragraphe.

⚠️ **Une première rédaction de ce document donnait 157 et 106**, chiffres
calculés sur les 170 raccourcis `.exe` **sans** la règle « le fichier cible
existe ». Ils étaient justes pour cette population-là et faux pour celle du
produit. **Corrigés en relançant la mesure, pas en recalculant de tête.**

**Coût de l'énumération**, sonde `C:\dev\gapps-sonde-cout.ps1`, verbatim :

```
enumeration recursive des 4 racines : 218 .lnk en 31 ms
resolution COM de 218 raccourcis : 82 ms (0.38 ms/raccourci)
extraction 256x256 + encodage PNG de 20 icones : 573 ms, 678833 octets cumules (33942 o/icone)
```

⚠️ **Une exécution, sur une VM au repos, avec le cache de fichiers chaud.**
Les 33 942 octets par icône passent par `Image::FromHbitmap`, **qui perd le
canal alpha** : c'est un ordre de grandeur, pas le poids que produira l'agent.

### 3.2 🔴 Le relevé qui gouverne toute la question des icônes

**Une taille rendue de 256×256 ne prouve RIEN.** Sonde
`C:\dev\gapps-sonde-temoin.ps1`, sur deux fichiers `.ico` **fabriqués pour
cela** — l'un ne contenant qu'une entrée 48×48, l'autre qu'une entrée 256×256 :

```
C:\dev\gapps-temoin-48.ico | ShellImageFactory 256 ICONONLY -> 256x256 32bpp
C:\dev\gapps-temoin-48.ico | ShellImageFactory 256 ICONONLY|BIGGERSIZEOK -> 256x256 32bpp
C:\dev\gapps-temoin-48.ico | PrivateExtractIcons idx0 256 -> 256x256 32bpp
C:\dev\gapps-temoin-48.ico | PrivateExtractIcons idx0 48 -> 48x48 32bpp
C:\dev\gapps-temoin-256.ico | ShellImageFactory 256 ICONONLY -> 256x256 32bpp
C:\dev\gapps-temoin-256.ico | ShellImageFactory 256 ICONONLY|BIGGERSIZEOK -> 256x256 32bpp
C:\dev\gapps-temoin-256.ico | PrivateExtractIcons idx0 256 -> 256x256 32bpp
C:\dev\gapps-temoin-256.ico | PrivateExtractIcons idx0 48 -> 48x48 32bpp
```

**Les quatre premières lignes et les quatre suivantes sont IDENTIQUES.** Un
fichier qui ne contient que du 48×48, interrogé à 256, rend 256×256 32bpp — et
il le rend par les **deux** API candidates, `IShellItemImageFactory::GetImage`
et `PrivateExtractIconsW`, y compris sans `SIIGBF_SCALEUP` et y compris avec
`SIIGBF_BIGGERSIZEOK`. **Un critère de réception qui compare la taille rendue à
256 ne peut pas échouer** : c'est exactement le patron de contrôle vacueux que
ce dépôt a payé en D7 (F1), en D8 (la sonde P1) et en D9 (le confondeur de
cible).

**Le témoin est construit sans aucune hypothèse**, et c'est ce qui le rend
décisif : je n'ai pas à supposer que tel index de `shell32.dll` désigne tel
groupe d'icônes — je fabrique un fichier dont je connais le contenu.

**Ce qui discrimine, c'est la RESSOURCE, jamais le rendu.** Sonde
`C:\dev\gapps-sonde-icones.ps1` : `LoadLibraryExW(LOAD_LIBRARY_AS_DATAFILE)`,
`EnumResourceNamesW(RT_GROUP_ICON)`, puis lecture du `GRPICONDIR` — dont chaque
entrée porte un `bWidth` sur un octet, où **`0` signifie 256**. Extrait
verbatim :

```
=== C:\Program Files (x86)\Steam\steam.exe
   groupe[0] nom=#101 entrees=13 : … 256x32bpp/18630o 64x32bpp/16936o 48x32bpp/9640o …
=== C:\Windows\system32\notepad.exe
   groupe[0] nom=#2 entrees=13 : … 256x32bpp/72024o 48x32bpp/9640o 32x32bpp/4264o …
=== C:\Windows\system32\shell32.dll
   groupe[0] nom=#1 entrees=8 : 256x32bpp/3451o 64x32bpp/16936o …
   groupe[2] nom=#3 entrees=8 : 32x4bpp/744o 16x4bpp/296o 48x8bpp/3752o 32x8bpp/2216o 16x8bpp/1384o 48x32bpp/9640o 32x32bpp/4264o 16x32bpp/1128o
=== C:\Program Files\Mozilla Firefox\firefox.exe
   groupe[0] nom=#1 entrees=4 : 16x32bpp/1320o 32x32bpp/5160o 48x32bpp/11560o 256x32bpp/50218o
```

`shell32.dll` porte **327 groupes d'icônes, dont 154 sans aucune entrée 256** —
le groupe `#3` ci-dessus plafonne à 48. `explorer.exe` porte un groupe `#20101`
qui plafonne à 128. **La ressource distingue ; le rendu ne distingue pas.**

⚠️ **Ce que ce relevé ne dit pas** : rien de la QUALITÉ visuelle. Un `256×32bpp`
de 3 451 octets (`shell32.dll` groupe `#1`) et un de 72 024 octets
(`notepad.exe`) sont tous deux des 256 authentiques ; le premier est simplement
très compressible. **Aucun jugement visuel n'a été porté**, comme pour
`BPP_MIN`, `FACTEUR_FOCUS` et toutes les autres constantes non calibrées de ce
dépôt.

### 3.3 Ce que le canal plateforme ↔ agent porte aujourd'hui

Livré par le sous-bloc P3, lu le 19 août 2026 :

- `proto/src/plateforme.rs:36` — `pub const PLATEFORME_VERSION: u8 = 1;` ;
- deux messages montants seulement (`VersLaPlateforme::Enroler`,
  `::Battement`) et trois descendants (`DepuisLaPlateforme::Enrole`,
  `::BattementRecu`, `::Refus`) ;
- `#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]` sur
  les deux énumérations : **un champ de trop est refusé**, et la version est
  vérifiée **variante par variante** ;
- côté plateforme, la boucle vit dans `plateforme/src/agents/canal.ts` (198 l.)
  et la montée est routée sur `plateforme/src/http/serveur.ts:38`
  (`const CHEMIN_AGENT = '/agent';`) ;
- côté agent, `agent/src/plateforme.rs` (**277** l., relu au commit `a74c2c8`) porte le
  client **avec sa reprise à repli exponentiel** et un battement de
  `PERIODE_BATTEMENT = 30 s`, à opposer au `SEUIL_INJOIGNABLE_MS = 90_000` de
  `plateforme/src/agents/fraicheur.ts:47` ⚠️ *(le numéro a dérivé — il était 37 ; la VALEUR, elle, est inchangée. Relevé par le sous-bloc G3, le 21 août 2026.)*.

La table cible existe déjà, vide et assumée telle :
`plateforme/src/base/migrations/0003-agents.sql:52-58` —
`application(id, vm_id, nom, chemin, vue_a)`, avec son commentaire (`:43-44`)
« son chemin d'ecriture est le sous-projet ④, qui empruntera le canal /agent
pour la remplir ». **④ est nommément le propriétaire de ce chemin.**

Mesuré sur le SQLite embarqué (3.50.4) le 19 août 2026, parce que ④ devra
ajouter des colonnes à cette table :

```
ADD COLUMN nullable : OK
ADD COLUMN NOT NULL DEFAULT : OK
ADD COLUMN avec REFERENCES : OK

<!-- ANNOTATION G1 (20 août 2026) — RELEVÉ DATÉ, CONSERVÉ, COMPLÉTÉ.
Ce relevé est exact ET INCOMPLET : il n'a mesuré ni `ADD COLUMN … UNIQUE`, ni
`ADD COLUMN … NOT NULL` SANS DÉFAUT sur une table NON VIDE. Le plan de G1 a
mesuré les deux, le 19 août 2026, une exécution chacune sur base neuve :
  - `ADD COLUMN TEXT UNIQUE` : 🔴 REFUSÉ par SQLite 3.50.4 (`Cannot add a
    UNIQUE column`), accepté par PostgreSQL 16.15 — divergence E7. Remède
    retenu : `CREATE UNIQUE INDEX`, OK des deux côtés.
  - `ADD COLUMN … NOT NULL` sans DÉFAUT : OK sur table VIDE, 🔴 REFUSÉ dès
    qu'elle porte une ligne (`Cannot add a NOT NULL column with default value
    NULL`) — divergence E8. Conséquence qui dépasse G1 : toute colonne NOT NULL
    dont un sous-bloc ultérieur aura besoin sur `application` doit naître tant
    que la table est vide.
Rien de ce qui est écrit ci-dessus n'est réfuté ; c'est la portée qui était
plus étroite qu'il n'y paraissait. -->
version SQLite = 3.50.4
```

⚠️ **`ADD COLUMN avec REFERENCES` est accepté SYNTAXIQUEMENT ; je n'ai pas
mesuré que la contrainte soit ENSUITE APPLIQUÉE.** Le leg n°2 de P1 —
« toute contrainte doit naître avec sa table » — n'est donc **pas réfuté** par
ce relevé, et ④ le respecte : les colonnes ajoutées sont **sans clé
étrangère**, et toute table neuve naît avec les siennes.

---

## 4. Décisions actées

### D1 — Découverte : la RÉCONCILIATION est la source de vérité, la notification n'est qu'un déclencheur

Le cadrage écrit « notifications de changement, plus de polling horaire ». Un
mécanisme **purement** événementiel dérive en silence, et le brief le dit :
c'est aussi ce que dit l'API.

**Mécanisme retenu : `ReadDirectoryChangesW`**, sur les quatre racines relevées
au §3.1, `bWatchSubtree = TRUE`, filtres `FILE_NOTIFY_CHANGE_FILE_NAME |
FILE_NOTIFY_CHANGE_DIR_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE`.

**Pourquoi pas `SHChangeNotifyRegister`** : il exige un `HWND` et une pompe de
messages, il délivre des PIDL qu'il faut re-résoudre en chemins, et il vise
l'espace de noms Shell alors que les quatre racines sont de **vrais répertoires
de fichiers** — le §3.1 le montre, `Get-ChildItem -Filter *.lnk` y suffit. Son
seul avantage — voir les changements de l'espace de noms virtuel — ne sert que
les 7 raccourcis à cible vide, que D3 exclut de la v1. Le coût, une pompe de
messages de plus dans un processus qui en a déjà une pour le hook de fenêtres
(`agent/src/superviseur/hook.rs`), n'est pas payé.

**Ce que coûte `ReadDirectoryChangesW`** : quatre handles de répertoire ouverts
en permanence (`FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED`), quatre
tampons, et un fil dédié ou un port de complétion.

🔴 **Ce qui se passe quand une notification est perdue, et le remède.** L'appel
rend **`lpBytesReturned == 0`** quand son tampon interne déborde : les
notifications sont perdues et **rien ne dit lesquelles**. Trois autres façons de
les perdre : l'agent est arrêté au moment du changement, le répertoire surveillé
est renommé ou supprimé, le handle est invalidé. **Aucune reprise ne peut
reconstruire ce qui a été perdu.**

**Décision : la réconciliation périodique complète est la source de vérité, et
elle n'est jamais désarmée.** La notification ne fait qu'**avancer** la
prochaine réconciliation ; son absence coûte de la latence, jamais une
application manquante.

**Le chiffre qui autorise cette décision est mesuré** (§3.1) : énumération
récursive des quatre racines **31 ms**, résolution COM des 218 raccourcis
**82 ms**, soit **113 ms** pour une réconciliation complète, **sans les
icônes**. Une réconciliation toutes les `PERIODE_RECONCILIATION` (30 s proposé,
**NON CALIBRÉE**) coûte donc de l'ordre de 0,4 % d'un cœur. Le balayage horaire
du legacy n'était pas cher parce qu'il balayait : il était cher parce qu'il
traversait WinRM et CIFS et extrayait toutes les icônes à chaque tour.

**Anti-rebond** : un installeur écrit des dizaines de fichiers. Une
réconciliation par notification en déclencherait autant. `DELAI_ANTI_REBOND`
(750 ms proposé, **NON CALIBRÉE**) : toute notification repousse l'échéance,
bornée par `DELAI_ANTI_REBOND_MAX` (5 s) pour qu'un flux continu de
notifications ne l'ajourne pas indéfiniment.

**Ce que la réconciliation compare** : un `Catalogue` d'aujourd'hui contre celui
d'hier, et elle produit un **diff** — `apparues`, `disparues`, `modifiees`.
C'est une **fonction pure**, testée sur l'hôte, et c'est le cœur du sous-bloc.

⚠️ **Une disparition n'est pas une suppression.** Un raccourci absent d'un tour
peut revenir au suivant (un installeur qui remplace son `.lnk`). Décision :
`application.disparue_a` est **posé** et l'application quitte le catalogue
affiché, mais **la ligne n'est jamais supprimée** — sans quoi une mise à jour
d'application ferait perdre son identifiant, donc sa PWA installée côté
navigateur.

### D2 — Lecture du raccourci : `IShellLinkW`, et JAMAIS `Resolve`

**Décision : `CoCreateInstance(CLSID_ShellLink)` + `IPersistFile::Load` en
lecture seule**, puis `GetPath(SLGP_RAWPATH | SLGP_UNCPRIORITY)`,
`GetArguments`, `GetWorkingDirectory`, `GetIconLocation`, `GetShowCmd`.

**Le §3.1 répond à la question « rend-elle tout ce dont on a besoin » : oui, pour
211 des 218 raccourcis**, les cinq champs sortis d'un seul objet. Les 7 restants
n'ont pas de cible parce qu'ils n'en ont pas — voir D3.

**Les cas retors du legacy disparaissent, un par un** :

| Cas retors, `src/lnkParser.js` | Ce qu'en fait `IShellLinkW` |
| --- | --- |
| `LinkInfo` ne rend que `"C:\"` (`:90`) | le Shell résout l'`IDList`, qui est la source normative ; le `LinkInfo` n'est qu'un cache. Le relevé §3.1 ne montre **aucune** cible tronquée sur 218 |
| chemins relatifs `..\..\AppData\…` (`:176-186`) | résolus par le Shell relativement au `.lnk` |
| substitution d'environnement écrite à la main (`:190-195`) | `SLGP_RAWPATH` rend la forme brute ; **on appelle `ExpandEnvironmentStringsW`**, qui connaît toutes les variables et pas seulement quatre |

🔴 **`IShellLink::Resolve` n'est JAMAIS appelé.** Il peut interroger le réseau,
parcourir le disque à la recherche d'une cible déplacée, **et déclencher
l'installation à la demande d'un raccourci MSI publié** — c'est-à-dire lancer un
installeur pendant une réconciliation de routine. Le coût de ce refus est nommé :
un raccourci dont la cible a bougé garde son ancien chemin, et D6 le rattrape au
lancement.

### D3 — La règle de filtrage est ÉCRITE, et elle n'est pas « intelligente »

218 raccourcis ne sont pas 218 applications. **Décision, v1 :**

1. **cible non vide** — exclut les 7 raccourcis de l'espace de noms Shell ;
2. **extension de la cible ∈ {`.exe`}** — exclut `.msc`, `.url`, `.html`,
   `.pdf`, `.chm`, `.txt`, `.bat`, `.msi` ;
3. **le fichier cible existe** ;
4. **aucun filtre par nom, aucun filtre par chemin.**

**Attendu sur cette VM, MESURÉ en appliquant les trois règles (§3.1) :
218 raccourcis, 167 retenus, 154 applications distinctes.**

**Pourquoi aucun filtre par nom** : un motif « Uninstall » est dépendant de la
langue (cette VM est en français, et l'un de ses désinstalleurs s'appelle
`maintenancetool.exe`) et **il écarterait en silence des applications
légitimes**. Le dépôt refuse les replis muets. Les 15 désinstalleurs relevés
entrent donc au catalogue, **et l'utilisateur les masque** :
`application.masquee_a`, un geste explicite, réversible, journalisé.

**Pourquoi aucun filtre par chemin** : 63 des cibles vivent sous `C:\Windows`,
Bloc-notes et Paint compris. Un filtre système les perdrait.

**Pourquoi `.exe` seul** : les 15 `.msc` et les 7 `.url` sont lançables, mais
`.msc` ouvre une console MMC dont la fenêtre est celle de `mmc.exe` — le
rattachement de fenêtre du chantier D n'a jamais été éprouvé dessus — et `.url`
ouvre le navigateur par défaut, c'est-à-dire une application qui n'est pas
celle qu'on croit lancer. **Les deux sont nommés hors périmètre v1, pas
oubliés.**

### D4 — L'identité d'une application est `(cible, arguments, répertoire)`, jamais son nom ni son chemin de raccourci

**Le chiffre décide : 104 clés par cible seule contre 154 par le triplet**,
sur la même population de 167 raccourcis retenus (§3.1). Les 26 raccourcis de `smartmontools` visent le même `runcmdu.exe` avec
des arguments différents ; les fondre serait perdre 25 applications, et l'écart total est de **50**.

`application.cle` = empreinte SHA-256 de la concaténation de la cible
**normalisée** (chemin absolu, casse repliée — le système de fichiers Windows
est insensible à la casse), des arguments **bruts** (un argument est sensible à
la casse), et du répertoire de travail **normalisé**.

**Ce n'est pas le chemin du `.lnk`** : un raccourci qui se déplace du Bureau vers
le menu Démarrer resterait la même application. **Ce n'est pas le nom** : le
legacy le montre (`src/app.js:67` retire les chiffres, donc confond deux
versions d'un même outil).

⚠️ **Le coût est réel** : une mise à jour qui change le répertoire
d'installation — un dossier versionné — produit une application **neuve**, et
l'ancienne disparaît. C'est le mauvais côté de ce choix, et il est assumé plutôt
que corrigé par une heuristique de rapprochement qui, elle, se tromperait en
silence.

### D5 — Icônes : le rendu vient du Shell, la PREUVE vient de la ressource

**Extraction** : `SHCreateItemFromParsingName` sur le **`.lnk` lui-même**, puis
`IShellItemImageFactory::GetImage({256,256}, SIIGBF_ICONONLY)`.

**Sur le `.lnk`, pas sur la cible**, et c'est le §3.1 qui l'impose : **135
raccourcis sur 218 portent un `IconLocation` sans chemin**, donc leur icône est
celle de la cible ; et les autres portent une icône **propre au raccourci**
(`shell32.dll,21` pour le Panneau de configuration). Seul le Shell connaît toute
cette chaîne. C'est aussi ce qui explique les icônes perdues du legacy :
`convertToLinuxPath('')` rend la chaîne vide (`src/lnkParser.js:172`).

🔴 **Et la taille rendue n'est jamais le critère** (§3.2). L'agent produit, à
côté du PNG, un champ **`source_max_px`** : la plus grande entrée réellement
présente dans le répertoire d'icônes de la source.

| Source | Comment `source_max_px` est obtenu |
| --- | --- |
| module PE (`.exe`, `.dll`) | `LoadLibraryExW(LOAD_LIBRARY_AS_DATAFILE \| LOAD_LIBRARY_AS_IMAGE_RESOURCE)`, `FindResourceW(RT_GROUP_ICON, …)`, lecture du `GRPICONDIR` ; **`bWidth == 0` vaut 256** |
| fichier `.ico` autonome | le `ICONDIR` en tête de fichier, même encodage de `bWidth` |
| tout le reste (association de type, espace de noms) | **`Inconnu`** |

🔴 **`Inconnu` est une valeur DISTINCTE de 256, et il est interdit de les
confondre.** Un catalogue qui afficherait « 256 » pour une icône dont on ignore
la provenance mentirait exactement comme le rendu.

**Ce que ce champ NE dit pas** : la qualité. Le §3.2 relève un 256 authentique de
3 451 octets. `source_max_px` répond à « d'où vient l'image », jamais à
« est-elle belle ».

**Stockage adressé par le contenu** : le PNG est nommé par son SHA-256 ; la ligne
`application` ne porte que l'empreinte. Le canal ne transporte donc **jamais**
une icône déjà connue de la plateforme : l'agent annonce les empreintes, la
plateforme répond avec celles qui lui manquent, et l'agent ne téléverse que
celles-là. À 33 942 octets par icône (§3.1, ordre de grandeur) et 154
applications, un catalogue complet pèse **≈ 5,2 Mo** : le renvoyer à chaque
réconciliation serait absurde.

### D6 — Le lancement fait partie de ④, parce qu'un catalogue qu'on ne peut pas lancer n'est pas un livrable

La spec plateforme relève que « **rien ne permet de demander le lancement d'une
application** » (§2.4) : `DepuisLaShell` n'a qu'une variante, `Viewport`. Le
cadrage place pourtant le lancement dans son flux principal (§6). Aucun des cinq
sous-blocs P1 à P5 ne le livre.

**Décision : ④ le livre, dès son premier sous-bloc.** Sans lui, la réception de
G1 serait la comparaison de deux fichiers JSON — un critère qui n'exerce pas le
produit.

**Le lancement se fait par `ShellExecuteExW` sur le chemin du `.lnk`**, jamais
en reconstruisant une ligne de commande depuis la cible et les arguments. C'est
ce que fait un double-clic : il honore le répertoire de travail, le verbe par
défaut, le `nShow` de `GetShowCmd`, et la publication MSI. Reconstruire, c'est
réintroduire un analyseur de ligne de commande maison, c'est-à-dire le défaut
qu'on retire à `lnkParser.js`.

**Repli** : si le `.lnk` a disparu depuis la réconciliation, l'agent tente la
cible enregistrée. **Les deux tentatives sont journalisées** ; l'échec des deux
est un refus typé, jamais un silence.

⚠️ **Le processus lancé n'entre PAS dans le job object du superviseur.** Voir
D8 : la raison est la même que pour l'installeur, et elle a la même
contrepartie.

### D7 — Téléversement : les octets passent par HTTP, les ordres par le canal

**Décision : le canal `/agent` ne transporte jamais un installeur.** Il est en
JSON (`proto/src/plateforme.rs`, en-tête), il porte le battement de cœur, et une
tranche de 8 Mio en base64 y coûterait +33 % et bloquerait le battement. Le
plafond de corps HTTP de la plateforme est aujourd'hui de 4 Kio
(`plateforme/src/http/routes-auth.ts:44`) : ④ ouvre des routes qui ont **leur
propre** plafond, et ne relève surtout pas celui-là.

**Le chemin, et les trois vérifications d'empreinte** :

| # | Qui → qui | Quoi |
| --- | --- | --- |
| 1 | navigateur → plateforme | `POST /televersement` avec `{nom, taille, sha256}` — le navigateur a calculé l'empreinte par `SubtleCrypto`. Rend `{id, taille_tranche}` |
| 2 | navigateur → plateforme | `PUT /televersement/:id/tranche/:n`, `application/octet-stream`. **Idempotent** : redéposer `n` l'écrase |
| 3 | navigateur → plateforme | `POST /televersement/:id/sceller` — 🔴 **la plateforme recalcule le SHA-256** et refuse s'il diffère. ⚠️ *« du fichier **réassemblé** » est le SEUL mot de cette spec que le sous-bloc G3 contredit, et il le contredit PAR UNE ÉQUIVALENCE : il ne réassemble jamais (sa décision D7), le scellement étant une passe de FLUX sur les tranches dans l'ordre. Même valeur, même refus, et le doublement du disque en moins — 1,6 Go économisés pour un installeur de 800 Mo.* |
| 4 | plateforme → agent, sur `/agent` | `Installer { installation, url, sha256, nom }` |
| 5 | agent → plateforme, en HTTP | `GET <url>`, jeton d'agent en en-tête. 🔴 **L'agent recalcule l'empreinte** après écriture |
| 6 | agent → plateforme, sur `/agent` | `Progression`, puis `Termine` |

**Trois vérifications, une seule valeur, et aucun saut ne fait confiance au
précédent.** Le navigateur peut mentir, le disque de la plateforme peut se
corrompre, le transfert vers la VM peut tronquer : chacun est attrapé par le
maillon suivant.

**Reprise** : `GET /televersement/:id` rend **la liste des tranches présentes**,
obtenue par un **listage de répertoire**, jamais par une table de comptabilité
qui pourrait diverger du disque. Le navigateur renvoie les manquantes.

**Où le fichier atterrit dans la VM** :
`%ProgramData%\Guacamole\installeurs\<id>\<nom>`.

- **pas `%TEMP%`** : Windows le purge, y compris pendant une installation ;
- **pas le profil utilisateur** : un installeur élevé s'exécute sous un autre
  jeton et peut ne pas voir le chemin ;
- **`%ProgramData%`** est lisible par tous les comptes de la machine et survit
  aux redémarrages.

**Il est supprimé après que le code de sortie a été rapporté ET que la
réconciliation suivante a tourné**, jamais avant : sans quoi un installeur qui
relit son propre fichier — les archives auto-extractibles le font — échouerait.
Une purge d'âge (`EXPIRATION_INSTALLEUR`, 24 h proposé, **NON CALIBRÉE**)
rattrape les cas où la séquence s'interrompt, sans quoi la VM se remplit.

🔴 **Si la session tombe en cours de transfert — trois chutes distinctes, qu'il
ne faut pas confondre.**

1. **Le navigateur tombe.** Rien n'est perdu : les tranches sont sur la
   plateforme. Le hub liste les téléversements inachevés de l'utilisateur
   depuis la base ; il reprend avec le même `id`. Au-delà
   d'`EXPIRATION_TELEVERSEMENT` (24 h proposé), un balayage les supprime.
2. **Le canal `/agent` tombe avant que l'agent n'ait commencé à télécharger.**
   L'ordre est perdu — un `push` WebSocket n'a aucune garantie de livraison.
   **L'ordre n'est donc pas « tiré et oublié »** : la ligne `installation` vit
   en base à l'état `en_attente`, et **la plateforme la réémet à chaque
   enrôlement**. L'agent **déduplique par `installation.id`** : sans cela, une
   reprise de canal installerait deux fois.
3. **Le canal tombe pendant que l'installeur tourne.** L'installeur n'est pas
   dans le job object (D8) : il continue. À la reprise, l'agent réémet l'état
   de l'installation qu'il détient. **Si l'AGENT est mort**, l'installeur
   continue et son code de sortie est perdu à jamais : l'installation est
   rapportée `issue_inconnue`, **jamais `reussie`**. Et c'est précisément
   pourquoi D9 fait juger la réussite par la réconciliation.

### D8 — Exécuter un binaire arbitraire fourni par l'utilisateur : ce que ça veut dire, et ce qu'on fait quand ça casse

**Le cadrage l'assume en toutes lettres** (§3, ligne « Isolation ») :
« Indispensable : exécution d'installeurs arbitraires uploadés », et la
mitigation est **la VM dédiée par utilisateur**. ④ ne l'atténue pas davantage et
ne prétend pas le faire.

**Ce que cela implique, nommément** :

- **un installeur peut casser la VM** : installer un pilote, corrompre le
  registre, remplir le disque, désactiver le réseau ;
- **un installeur peut désinstaller ou remplacer l'agent lui-même** — rien ne
  l'en empêche, il tourne avec les mêmes privilèges ;
- **un installeur peut changer le périphérique audio par défaut, et
  CELA S'EST PRODUIT LE 19 AOÛT 2026.** L'installation de VB-Cable a fait
  basculer le rendu par défaut de Windows sur un câble virtuel que rien
  n'alimente ; le loopback du chantier A s'est mis à capter du silence **sans
  qu'aucune ligne de journal ne dise pourquoi** (CLAUDE.md, chantier E,
  § « ④ La correction A-bis »). **C'est le cas nominal, pas un cas limite** :
  tout installeur audio le rejouera.

**Ce que le produit fait alors** :

- **le remède du périphérique existe déjà et ④ n'en invente pas d'autre** :
  `AUDIO_PERIPHERIQUE` (règle pure dans `agent/src/wasapi/peripherique.rs`)
  désigne l'endpoint explicitement au lieu de subir le défaut ;
- **④ ajoute un devoir de trace, et c'est tout ce qu'il ajoute** : l'agent
  journalise le périphérique de rendu par défaut **avant et après** chaque
  installation. La prochaine occurrence devient attribuable par un `grep`, au
  lieu d'une campagne ;
- **④ n'ajoute AUCUN garde-fou de restauration.** Remettre d'autorité le
  périphérique d'avant serait décider à la place de l'utilisateur qui vient
  d'installer un périphérique audio exprès.

🔴 **L'installeur n'est PAS assigné au job object du superviseur.**
`agent/src/superviseur/lanceur.rs:98` pose
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, dont l'en-tête (`:19-28`) explique la
raison : la mort du superviseur ne doit pas laisser N agents derrière lui. **Ce
raisonnement ne se transpose pas à un installeur** : un redémarrage d'agent le
tuerait au milieu d'une écriture de registre et laisserait la machine à
moitié installée — un état dont rien ne sait sortir. **Le coût du choix inverse
est nommé** : un agent tué laisse l'installeur orphelin, et son code de sortie
est perdu (voir D7, chute n°3).

**Aucun mode silencieux n'est imposé.** `.exe` est exécuté tel quel ; `.msi`
passe par `msiexec /i <fichier>`. Il n'existe pas de drapeau silencieux
universel, et la silenciosité de l'installation de VB-Cable est explicitement
**inconnue** dans ce dépôt (CLAUDE.md, chantier E §⑤). Ce n'est pas un pis-aller :
**depuis D1, toute fenêtre principale Windows ouvre une fenêtre navigateur** —
l'interface de l'installeur est donc déjà diffusée et cliquable. C'est le seul
sous-projet où le multi-fenêtres paie directement.

⚠️ **La seule exception connue, et c'est le risque n°1 de G3 : une élévation
UAC ouvre une boîte de dialogue sur le BUREAU SÉCURISÉ**, que Desktop
Duplication ne capture pas. **Ce comportement est documenté par Windows ; il
n'est pas mesuré ici.** Une installation qui l'exige se présenterait à
l'utilisateur comme un écran figé. La sonde qui tranche est nommée en G3.

**Extensions acceptées à l'exécution, v1 : `.exe` et `.msi`.** `.bat` est
**refusé avec son motif** : c'est un script, dont l'interprète, le répertoire de
travail et la politique d'exécution appellent leurs propres décisions. Il reste
déclarable côté navigateur comme type de fichier (l'amendement du 28/07 le
nomme) — **déclarer un type et l'exécuter sont deux choses différentes**, et
un fichier refusé l'est avec un message, jamais en silence.

### D9 — Le verdict d'installation se lit dans la RÉCONCILIATION, pas dans le code de sortie

`msiexec` rend **3010** pour « réussi, redémarrage requis » ; beaucoup
d'installeurs rendent **0** après que l'utilisateur a annulé.

**Décision : le code de sortie est RAPPORTÉ, jamais INTERPRÉTÉ.** Une
installation est `reussie` quand la réconciliation qui la suit rapporte au moins
une application apparue ; `sans_effet` quand elle n'en rapporte aucune ;
`issue_inconnue` quand le code de sortie n'a pas pu être recueilli.

C'est la doctrine que le sous-bloc D8 a payée sur un autre terrain — **juger sur
la relecture, jamais sur le code de retour** : `ChangeDisplaySettingsExW` y
rendait `0` sur une sortie qui n'avait pas bougé d'un pixel (CLAUDE.md,
sous-bloc D8, « refus déguisé en succès »).

**Progression et journaux** (cadrage §7) :

- `Progression { installation, phase, octets_faits, octets_total }`, phases
  `transfert`, `execution`, `reconciliation` ;
- 🔴 **la phase `execution` ne porte AUCUN pourcentage.** Un installeur n'en
  publie pas ; en inventer un serait mentir. Elle porte le temps écoulé, et
  l'interface affiche un état indéterminé ;
- stdout et stderr sont redirigés vers un fichier du répertoire d'installation,
  dont la **queue bornée** (64 Kio) part avec `Termine` ;
- ⚠️ **un journal vide est le cas NORMAL** — la plupart des installeurs Windows
  sont graphiques et n'écrivent rien sur les flux standard. L'interface ne doit
  pas le présenter comme un échec.

### D10 — Le protocole monte à `PLATEFORME_VERSION = 2`, et c'est une rupture assumée

Les messages de ④ (`Catalogue`, `Installer`, `Progression`, `Termine`,
`Lancer`, `Lancee`, `IconesManquantes`) sont des variantes neuves des deux
énumérations de `proto/src/plateforme.rs`. Or ce module vérifie la version
**variante par variante** et pose `deny_unknown_fields` : **un agent v1 et une
plateforme v2 ne se parlent pas**, et le refus `version` **ne se réessaie pas**
(`proto/src/plateforme.rs`, en-tête, l. 24-28).

**Décision : `PLATEFORME_VERSION` passe à 2 dans le premier sous-bloc, et agent
et plateforme se déploient ensemble.** C'est exactement la décision D5 de P3
(« OUI, P3 CASSE l'agent déjà déployé, et c'est une décision ») ; la reprendre
sans le dire serait la subir.

**Les vecteurs partagés suivent** : `proto/plateforme-vectors.json` gagne ses cas
et sa clé `version`, **vérifiée des deux côtés** — la lacune d'`input.rs` que P3
a corrigée pour ce fichier-là et qu'il ne faut pas rouvrir.

---

## 5. Découpage en sous-blocs

| # | Objet | Dépend de |
| --- | --- | --- |
| **G1** | **Le catalogue naît, et on peut lancer ce qu'il contient** | P3 |
| **G2** | Les icônes 256, et la preuve que c'en est | G1 |
| **G3** | Le téléversement, l'exécution, et son issue | G1 |
| **G4** | La surveillance, qui n'est qu'une accélération | G1 |
| **G5** | La PWA par application, et les types installeur du hub | G2, G3 |

**Deux exécutions par critère, jamais une** — règle héritée de D9, D10 et de
tout le sous-projet ⑤. **Aucun taux ne sera revendiqué.**

⚠️ **G4 vient APRÈS G3, et l'ordre est une conséquence de D1**, pas une
commodité : la réconciliation étant la source de vérité, la surveillance
n'améliore que la latence. La construire en dernier garantit qu'aucun critère
antérieur ne repose sur elle, donc qu'aucune régression de la surveillance ne
peut créer un trou de correction.

### G1 — Le catalogue naît, et on peut lancer ce qu'il contient

**Livre** : la lecture d'un raccourci (`IShellLinkW`, D2) ; la règle de filtrage
(D3) et la clé d'identité (D4), **toutes deux pures** ; le diff de
réconciliation, **pur** ; la réconciliation périodique ; `PLATEFORME_VERSION = 2`
avec les variantes `Catalogue`, `Lancer`, `Lancee` et leurs vecteurs ; les
colonnes ajoutées à `application` ; `GET /applications` ; `POST
/application/:id/lancer` ; le lancement par `ShellExecuteExW` (D6).

**Tranche verticale démontrable** : un raccourci créé sur le Bureau de la VM
apparaît dans la réponse de `GET /applications`, et un `POST … /lancer` ouvre sa
fenêtre.

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | Le catalogue de cette VM compte **154 applications pour 218 raccourcis** | `GET /applications` après une réconciliation, comparé au dépouillement du §3.1 | **l'écart a été MESURÉ sur la VM avant l'écriture de ce document** (§3.1) : une identité par cible seule rend **104**, écart de 50. La ROUGE se joue en changeant la clé — elle n'est pas supposée, son résultat est déjà connu |
| ② | Un raccourci **neuf** apparaît sans redémarrer l'agent | créer un `.lnk` sur le Bureau, attendre au plus 2 × `PERIODE_RECONCILIATION`, relire | figer le catalogue au démarrage : le compte ne bouge pas. **À exercer** |
| ③ | Un raccourci **retiré** quitte le catalogue **sans que sa ligne disparaisse** | `disparue_a` non nul, ligne toujours présente en base | supprimer la ligne : la PWA installée côté navigateur perd son identifiant. Le test lit **les deux** |
| ④ | Les 7 raccourcis sans cible sont **exclus et journalisés** | 7 lignes de trace nommant chacune son fichier | les exclure en silence : le test compte les lignes de trace, pas seulement l'absence des entrées |
<!-- ANNOTATION G1 — ④ N'EST PAS APPLICABLE TEL QUEL (divergence E12). Sous une
réconciliation périodique de 30 s, « 7 lignes de trace » vaudrait 7 lignes
TOUTES LES 30 SECONDES dans un journal partagé. Décision D13 : la trace est
émise AU CHANGEMENT, et le critère se mesure sur la PREMIÈRE réconciliation,
BORNÉE TEMPORELLEMENT — jamais sur un total de fichier. MESURÉ en recette, deux
exécutions : 7 `motif="cible-vide"` sur la première réconciliation, aux deux. -->
| ⑤ | Un lancement ouvre **la bonne fenêtre** | `POST … /lancer` sur Bloc-notes, puis une fenêtre `notepad.exe` est détectée par le superviseur | lancer par la cible reconstruite au lieu du `.lnk` **passerait ce critère-ci** : c'est pourquoi ⑥ existe |
<!-- ANNOTATION G1 — ⑤ EST DEVENU DÉCIDABLE (divergence E13). La colonne de
droite dit elle-même que ce critère ne discrimine pas ; la décision D4 le
répare en typant l'issue du lancement — `IssueLancement::Raccourci` contre
`::Cible` —, ce qui donne DEUX contrôles indépendants au lieu d'un seul plus un
contournement. ⑥ reste. MESURÉ en recette : la route rend
`{"issue":"raccourci"}` (3/3), et la rouge jouée sur la VM — tentative par le
raccourci détournée vers un fichier inexistant — rend `{"issue":"cible"}` (3/3)
avec le fichier témoin de ⑥ qui n'atterrit alors nulle part. -->
| ⑥ | Le lancement honore le **répertoire de travail** du raccourci | un raccourci vers `cmd.exe /c cd > sortie.txt` avec un `WorkingDirectory` posé ; le fichier atterrit là | reconstruire la ligne de commande sans le répertoire : `sortie.txt` atterrit ailleurs. **À exercer** |

### G2 — Les icônes 256, et la preuve que c'en est

**Livre** : l'extraction par `IShellItemImageFactory` (D5) ; `source_max_px` par
lecture du `GRPICONDIR` et de l'`ICONDIR` ; l'encodage PNG avec alpha ; le
stockage adressé par contenu ; `IconesManquantes` sur le canal ; le téléversement
d'icônes ; `GET /application/:id/icone` avec `Cache-Control` immuable clé sur
l'empreinte.

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | Le PNG rendu fait **256×256 avec un canal alpha non trivial** | dimensions **et** présence d'au moins un pixel partiellement transparent | 🔴 **ce critère seul NE VAUT RIEN sur la taille** — le §3.2 le prouve — et il n'est là que pour l'**alpha**, dont la ROUGE est réelle : la sonde de coût du §3.1 est passée par `Image::FromHbitmap`, **qui perd le canal alpha**. Un chemin d'extraction naïf rend une icône opaque, et ce critère le voit |
| ② | `source_max_px` **distingue** un vrai 256 d'un agrandissement | le témoin `gapps-temoin-48.ico` rend `source_max_px = 48` **avec un PNG de 256×256** ; `gapps-temoin-256.ico` rend `256` | **la ROUGE est GRATUITE** : tout code qui déduirait `source_max_px` de la taille rendue donnerait `256` aux deux. Les deux fichiers témoins sont **versés avec le sous-bloc**, pas laissés sur la VM |
| ③ | Le corpus réel porte **les deux valeurs** | sur les 154 applications, au moins une à 256 et au moins une en dessous ou `Inconnu` | 🔴 **un corpus où toutes les valeurs sont égales ne peut pas faire échouer ②.** Si le corpus réel n'en fournit pas, le critère est **NON MESURABLE** et doit le dire, pas se déclarer tenu |
| ④ | `Inconnu` n'est jamais rendu comme un nombre | le type de la colonne et de la charge JSON admet trois cas, et le test lit le cas `Inconnu` | représenter `Inconnu` par `0` ou par `256` : le test le voit |
| ⑤ | Une icône déjà connue n'est **pas retéléversée** | deux réconciliations consécutives ; la seconde ne transporte aucun octet d'icône | omettre `IconesManquantes` : la seconde transporte ≈ 5,2 Mo. **À exercer** |

### G3 — Le téléversement, l'exécution, et son issue

**Livre** : les quatre routes de téléversement (D7) ; les trois vérifications
d'empreinte ; la reprise par listage ; l'ordre `Installer` réémis à
l'enrôlement et dédupliqué par l'agent ; l'exécution hors job object (D8) ; la
trace du périphérique de rendu avant/après ; `Progression` et `Termine` ; le
verdict par réconciliation (D9).

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | Un installeur réel de plusieurs centaines de Mo traverse et s'installe | l'application apparaît au catalogue après la réconciliation | **la ROUGE est GRATUITE** : sur le binaire de G1 aucune route de téléversement n'existe et le `POST` rend `404`. À jouer, pas à supposer |
| ② | Une **empreinte fausse est refusée aux trois étages** | trois tests : altérer une tranche ; altérer le fichier scellé sur la plateforme ; altérer le transfert vers l'agent | retirer une des trois vérifications : **le test correspondant doit rougir, les deux autres rester verts** — sans quoi on ne saurait pas laquelle protège |
| ③ | Une coupure en cours de téléversement **reprend là où elle en était** | couper après *k* tranches ; le second passage ne renvoie que les manquantes | recommencer à zéro passerait un test qui ne regarde que le résultat : **le test compte les octets du second passage** |
| ④ | Un ordre d'installation **survit à une chute du canal** | tuer le canal avant que l'agent ne télécharge ; la réémission à l'enrôlement le rejoue | le mode « tiré et oublié » de P3 le perd. **La ROUGE est disponible sur le binaire de G1** |
| ⑤ | Une réémission n'installe **pas deux fois** | même `installation.id` réémis ; un seul processus lancé | retirer la déduplication : deux processus. **À exercer** |
| ⑥ | Un code de sortie **0 sans effet** est rapporté `sans_effet`, jamais `reussie` | un `.exe` témoin qui rend 0 et n'installe rien | interpréter le code de sortie : il rend `reussie`. **La ROUGE est le témoin lui-même** |
| ⑦ | Tuer l'agent pendant l'installation **ne tue pas l'installeur** | le processus survit ; l'issue est `issue_inconnue` | assigner l'installeur au job object : il meurt avec l'agent. **À exercer, et c'est la ROUGE du choix D8** |

> 🔴 **LE CRITÈRE ⑦ EST VACUEUX TANT QU'ON N'A PAS RELEVÉ LE JOB, et c'est le
> sous-bloc G3 qui l'a établi** (sa divergence E7). `superviseur/lanceur.rs`
> n'appelle `AssignProcessToJobObject` que sur ses ENFANTS : **le superviseur ne
> s'assigne pas lui-même**, donc un processus qu'il crée n'hérite d'aucun job,
> et « tuer l'agent ne tue pas l'installeur » est **vrai par construction**. Un
> vert ne prouverait rien.
>
> Ce qui le rend décidable : `apps::installation::execution::dans_un_job`
> journalise `IsProcessInJob` **à chaque installation**, que le refus ait lieu
> ou non, et c'est cette ligne que la recette lit. La seule ROUGE disponible
> reste la **mutation** que ce tableau nomme.
>
> ⚠️ **ET LE PONT, LUI, EST DANS LE JOB** : sous le leg n°1 de G1, l'ordre
> pouvait lui échoir. G3 s'en protège par un refus typé — un GARDE, pas le
> remède.

> ⚠️ **G3 A QUATRE ISSUES, PAS TROIS** (sa divergence E4). `reussie` /
> `sans_effet` / `issue_inconnue` ne couvrent pas les cas où l'installation
> **n'a jamais démarré** — empreinte fausse, élévation requise, extension
> refusée, job object, disque plein. Les y ranger ferait dire « on ne sait
> pas » **là où l'on sait très bien** : `refusee` est ajoutée, avec un `motif`
> typé.
| ⑧ | Le périphérique de rendu par défaut est tracé **avant et après** | deux lignes par installation, avec le nom de l'endpoint | ne tracer qu'après : un changement n'est plus attribuable |

⚠️ **Sonde préalable à G3, et elle est ÉLIMINATOIRE pour la moitié
« interactive » du sous-bloc** : lancer un exécutable qui déclenche une
élévation UAC, et relever si sa boîte de dialogue **apparaît dans le flux**.
Si elle n'apparaît pas — ce que le bureau sécurisé de Windows laisse
attendre —, alors **tout installeur exigeant une élévation est inutilisable en
v1**, et il faut le dire en toutes lettres plutôt que le découvrir en
production. Le repli, s'il faut en prendre un : lancer l'agent élevé, ce qui
est une décision de sécurité qui n'appartient pas à ce document.

### G4 — La surveillance, qui n'est qu'une accélération

**Livre** : `ReadDirectoryChangesW` sur les quatre racines ; l'anti-rebond
(D1) ; la détection du débordement de tampon ; le rétablissement d'une
surveillance perdue.

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | Un raccourci créé apparaît **en moins de 5 s** | horodatage de création contre horodatage de la ligne | la réconciliation seule met jusqu'à `PERIODE_RECONCILIATION`. **La ROUGE est le binaire de G1** |
| ② | Un débordement de tampon est **détecté et journalisé** | provoquer une rafale (création de plusieurs milliers de fichiers) ; une ligne `notifications perdues` apparaît | 🔴 **si la rafale ne provoque aucun débordement, le critère est NON MESURABLE**, et il doit le dire. Un critère qu'on n'a pas vu se déclencher n'est pas un critère |
| ③ | Un débordement **ne perd aucune application** | après la rafale, le catalogue est complet | c'est la propriété que D1 achète. La ROUGE est un agent purement événementiel — **elle se joue en désarmant la réconciliation périodique** |
| ④ | L'anti-rebond **réduit** le nombre de réconciliations | installer une application réelle ; compter les réconciliations | sans anti-rebond, une par notification. **À exercer** |

> ⚠️ **ANNOTÉ PAR LE SOUS-BLOC G4 (21 août 2026), ET NON RÉÉCRIT : ce tableau
> reste un relevé daté, et il est vrai comme histoire.** Quatre de ses clauses
> ont été réfutées ou déplacées AVANT toute mesure, par lecture du code :
>
> - 🔴 **la ROUGE de ① — « le binaire de G1 » — N'EST PAS JOUABLE.**
>   `PLATEFORME_VERSION` vaut **4** ; le binaire de G1 parle **1**, celui
>   d'avant G2 parle **2**. Un agent v1 ou v2 face à la plateforme
>   d'aujourd'hui **est refusé et boucle sans terme**, sans même pouvoir LIRE
>   le refus — G1 l'a mesuré. **Tranché** : la ROUGE de ① est
>   `APPS_SURVEILLANCE=0` sur le binaire de G4, qui reproduit exactement le
>   comportement de G1 sans en reproduire le protocole. 🔵 Elle est même
>   MEILLEURE : même binaire, même corpus, même machine, **une seule variable
>   de différence**.
>
> - 🔴 **la ROUGE de ③ est très probablement NON DISCRIMINANTE, et c'est écrit
>   AVANT de la jouer.** Trois faits, raisonnés sur le code : une
>   réconciliation, **quel que soit son déclencheur**, relit le disque ENTIER ;
>   un débordement est **lui-même une complétion**, donc un déclencheur ; et le
>   tout premier tour est `complet` par construction, donc un changement
>   survenu agent ARRÊTÉ est rattrapé au démarrage — pas par la période. **Ce
>   qui achète l'absence de perte n'est donc pas la réconciliation
>   périodique : c'est le fait que toute réconciliation relise tout.**
>   Ce que la période achète RÉELLEMENT est le seul cas que D1 nomme et
>   qu'aucun événement ne peut signaler : **une surveillance qui cesse de
>   délivrer SANS ERREUR**. 🔵 Le montage qui, lui, PEUT être rouge est
>   `APPS_FAUTE=muette:<n>` — une complétion avalée.
>
> - ⚠️ **le pas de temps de ④ n'est pas « anti-rebond contre RIEN »** : le
>   sondage de `apps/boucle.rs` a une granularité de **200 ms**, qui est déjà
>   un anti-rebond faible. Le ROUGE mesure « anti-rebond contre 200 ms », et
>   **si les deux bras rendent le même compte, ④ est NON MESURABLE**.
>
> - ⚠️ **la borne haute de l'anti-rebond vaut 4 s et non les 5 s proposées
>   plus bas** : elle est **DÉRIVÉE** du critère ① et de deux coûts mesurés
>   (granularité du sondage 200 ms, coût d'une réconciliation ≈ 70 ms, 10 ms
>   par icône neuve). À 5 s le pire cas rend **5 280 ms**, au-dessus de ce que
>   ① exige. Voir `agent/src/apps/surveillance/rebond.rs`, qui porte le calcul.

### G5 — La PWA par application, et les types installeur du hub

**Livre** : le manifeste dynamique par application (icône 256, couleur
d'accent) ; les `file_handlers` alimentés par les **vraies** associations lues
par l'agent, sur le modèle de `src/asset.js:82-98` ; et l'ajout des types
installeur au manifeste du **hub**, conformément à l'amendement du 28/07/2026.

⚠️ **L'amendement est une décision déjà prise et ce document ne la rouvre pas** :
« tenter l'enregistrement, avec repli silencieux sur le glisser-déposer », et
**aucune fonctionnalité ne doit en dépendre**. Le glisser-déposer et
`showOpenFilePicker` sont le chemin nominal de G3 et doivent fonctionner seuls.

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | Une application découverte est **installable** en PWA | son manifeste valide, son icône 256 servie, le navigateur propose l'installation | retirer l'icône du manifeste, ou la servir en dessous de 192 px : le navigateur refuse l'installation. **À exercer** — un manifeste que personne n'a jamais vu refusé n'est pas éprouvé |
| ② | Le glisser-déposer d'un installeur **fonctionne sans aucun file handler** | tester dans un navigateur où l'enregistrement a échoué ou n'existe pas | 🔴 **c'est le critère qui garde l'amendement** : si le dépôt de fichier ne marche que par le file handler, la décision est violée |
| ③ | Le **test empirique** de l'amendement est joué et son résultat écrit | Chromium accepte-t-il d'enregistrer un handler pour `.msi` / `.exe` ? | **le résultat n'a pas à être positif** ; ne pas jouer le test, ou n'en écrire que la moitié, est l'échec |

⚠️ **ChromeOS est le meilleur candidat de ③ selon l'amendement, et il n'est pas
disponible ici.** Si le test ne se joue que sous Chromium desktop, **la portée
du relevé le dit**, et la question ChromeOS reste ouverte.

---

## 6. Ce qui est PUR, ce qui est `#[cfg(windows)]`, et l'arborescence

La séparation est décidée **ici**, pas à l'implémentation : le pur se teste sur
l'hôte Linux, le reste ne se vérifie que par
`cargo check --target x86_64-pc-windows-gnu`.

```
proto/
  src/plateforme.rs              MODIFIÉ — PLATEFORME_VERSION -> 2, sept variantes neuves
  ts/plateforme.ts               MODIFIÉ — le miroir
  plateforme-vectors.json        MODIFIÉ — un cas par variante, version vérifiée des deux côtés

agent/
  src/apps.rs                    NEUF — declare SANS cfg ; sa fonction d'assemblage porte le #[cfg(windows)]
  src/apps/raccourci.rs          NEUF — PUR : normalisation, clé d'identité, règle de filtrage
  src/apps/lecture.rs            NEUF — #[cfg(windows)] : IShellLinkW, ExpandEnvironmentStringsW
  src/apps/reconciliation.rs     NEUF — PUR : le diff (apparues / disparues / modifiees)
  src/apps/surveillance.rs       NEUF — #[cfg(windows)] : ReadDirectoryChangesW (G4)
  src/apps/rebond.rs             NEUF — PUR : l'échéance d'anti-rebond, horloge en paramètre (G4)
  ⚠️ CES DEUX LIGNES SONT ANNOTÉES PAR G4, PAS RÉÉCRITES. Ce qui a été livré :
     `apps/surveillance.rs` est **SANS `cfg`** — parent de `mode.rs`,
     `rebond.rs`, `faute.rs`, `partage.rs` (purs ou sans `cfg`) et de `fil.rs`,
     `racine.rs` (`#[cfg(windows)]`). C'est l'idiome que ④ a DÉJÀ établi deux
     fois, `apps/icone.rs` et `apps/installation.rs`, et dont `apps/icone.rs`
     porte la raison en tête : « on ne peut pas déclarer un PETIT-fils depuis le
     grand-parent sans `#[path]` ». Ranger `rebond.rs` en FRÈRE, comme cette
     ligne le prescrit, obligerait au `#[path]` que ④ évite depuis G2.
  src/apps/icone.rs              NEUF — #[cfg(windows)] : IShellItemImageFactory
  src/apps/icone/ressource.rs    NEUF — PUR : lecture d'un GRPICONDIR / ICONDIR depuis un &[u8]
  src/apps/lancement.rs          NEUF — #[cfg(windows)] : ShellExecuteExW
  src/apps/installation.rs       NEUF — SANS cfg ; SEUL `execution` porte le sien (G3)
  ⚠️ Cette ligne portait « src/installation.rs NEUF — #[cfg(windows)] :
     téléchargement, exécution hors job ». **Le téléchargement n'a AUCUNE raison
     d'être Windows** : `tokio::net::TcpStream`, un analyseur d'en-têtes et une
     écriture de fichier compilent et tournent sur l'hôte. G3 l'a donc rendu
     PORTABLE, et ce n'est pas un détail de rangement — c'est une amélioration
     de COUVERTURE : la troisième vérification d'empreinte, la reprise par
     `Range`, le refus du `chunked` et celui de `https` sont éprouvés sur
     l'hôte, CONTRE UN VRAI `TcpListener`, au lieu de dépendre d'une recette VM.
     Le chemin a changé aussi (`apps/installation`, déclaré depuis `apps.rs`) :
     `main.rs` ne bouge pas, ce qui tient l'engagement de périmètre pris envers
     quatre chantiers concurrents.
  src/installation/verdict.rs    NEUF — PUR : l'issue depuis (code de sortie, diff) — D9
  src/plateforme.rs              MODIFIÉ — les nouvelles variantes du canal

plateforme/
  src/base/migrations/0004-applications.sql   NEUF — colonnes ajoutées + tables televersement/installation
  src/depot/application.ts       NEUF
  src/depot/televersement.ts     NEUF (G3)
  src/http/routes-applications.ts NEUF (G1)
  src/http/routes-televersement.ts NEUF (G3)
  src/apps/catalogue.ts          NEUF — PUR : la fusion d'un catalogue reçu dans l'état connu
  src/apps/icones.ts             NEUF — le magasin adressé par contenu (G2)
  src/agents/canal.ts            MODIFIÉ — les branches neuves

client/
  src/hub/…                      NEUF — catalogue, dépôt d'installeur, progression (G1, G3)
  ⚠️ `src/hub/tranches.ts` — **G3 l'a mis dans `proto/ts/`, avec `sha256.ts`.**
     Ce sont des règles PARTAGÉES navigateur ↔ plateforme, et les y mettre est
     ce qui empêche deux arithmétiques de tranches de diverger — le symptôme
     serait un scellement qui refuse sans qu'on sache lequel des deux bouts a
     tort. C'est aussi ce qui rend la recette exécutable SANS navigateur.
     `client/src/hub/` ne garde que l'orchestration.
```

🔴 **`agent/src/apps/icone/ressource.rs` est PUR, et c'est le point le plus
important de cette liste.** La lecture d'un `GRPICONDIR` est de l'analyse
d'octets : elle prend un `&[u8]` et rend une liste de tailles. **Elle se teste
donc sur l'hôte Linux**, avec les deux fichiers témoins du §3.2 versés comme
`testdata`. Le `#[cfg(windows)]` ne couvre que l'**obtention** de ces octets
(`LoadLibraryExW` + `FindResourceW`). Sans cette coupure, le critère ② de G2
n'aurait aucun test d'hôte, et il serait dans la même situation que F1 de D7 —
un argument de flot de contrôle en guise de preuve.

**Le plafond de 500 lignes est budgété d'avance.** Relevé par la commande de
`CLAUDE.md` le 19 août 2026 : deux fichiers seulement le dépassent
(`agent/src/encode.rs` 1536, `agent/src/windows_source.rs` 630). Les tailles
des fichiers voisins de ④ : `agent/src/superviseur/lanceur.rs` (367),
`agent/src/plateforme.rs` (**277**), `proto/src/plateforme.rs` (410, marge 90) et
`plateforme/src/agents/canal.ts` (198).

⚠️ **`proto/src/plateforme.rs` est le fichier à surveiller** : 410 lignes,
marge 90, et ④ y ajoute **sept variantes avec leurs constructeurs, leurs tests
de version et leurs cas de vecteurs** — le module en compte cinq aujourd'hui
pour 410 lignes. **Il franchira le plafond, et l'extraction précède
l'addition, sans exception** : la doctrine de ce dépôt (D9, `serveur/instances.rs`)
est qu'une marge traitée **avant** l'addition est rendue, et qu'une marge
traitée après est compressée. Point de chute nommé : `proto/src/plateforme/`,
en séparant les messages du **cycle de vie** (enrôlement, battement, déjà là) de
ceux de la **gestion d'apps**.

⚠️ **`plateforme/` doit entrer au § « Portée » de `CLAUDE.md`** — la spec ⑤ §5 le
relève déjà, et `client/src/hub/` relève de `client/`, déjà nommé.

<!-- ANNOTATION G1 — DEUX POINTS.
(E1) `plateforme/` EST DÉJÀ au § « Portée » de `CLAUDE.md` (ligne 20, relue) :
c'était fait avant G1, et il n'y avait rien à faire.
(E14) `client/src/hub/…` est rangé « (G1, G3) » par cette spec, mais AUCUN des
six critères de G1 ne juge une page : sa tranche verticale est décrite en
`GET /applications` et `POST … /lancer`, c'est-à-dire en HTTP. Décision D12 :
G1 ne touche pas `client/`, et la recette exerce les deux routes par `curl`,
ce qui les éprouve DAVANTAGE qu'une page. Le hub arrive avec G3. -->

**La « Convention de module enfant » de `CLAUDE.md` s'applique-t-elle ?** Elle
vise les modules extraits d'un parent `#[cfg(windows)]` pour compiler sur
l'hôte. `apps/icone/ressource.rs` est exactement ce cas — mais il reste
**enfant d'`apps`, et `apps` n'est PAS gaté** : l'arbre `apps::icone::ressource`
doit exister sur l'hôte pour que ses tests y courent. **Décision : `apps` est
déclaré `mod apps;` sans `cfg` dans `main.rs`, et ce sont ses fonctions
Windows qui portent le `cfg`** — le fichier n'a donc jamais besoin de
franchir une frontière `#[cfg(windows)]`, et la convention `#[path]` n'est pas
mobilisée. Elle est nommée pour dire qu'elle a été considérée.

---

## 7. Gestion des erreurs

| Panne | Comportement |
| --- | --- |
| Un `.lnk` illisible (corrompu, verrouillé) | il est **sauté avec sa trace**, la réconciliation continue. Une réconciliation qui échouerait en entier sur un fichier ferait disparaître tout le catalogue |
| Cible d'un raccourci absente du disque | exclu par D3, journalisé. **Pas d'erreur** : c'est un état courant après une désinstallation |
| `IShellItemImageFactory` échoue sur une application | l'application entre au catalogue **sans icône** ; `icone_sha256` est nul. Une application sans icône vaut mieux qu'une application absente |
| `source_max_px` indéterminable | **`Inconnu`**, jamais `0`, jamais `256` (D5) |
| Empreinte fausse à un des trois étages | refus **typé**, avec l'étage nommé ; le fichier partiel est supprimé ; le téléversement reste reprenable |
| Canal `/agent` coupé pendant un téléversement | l'agent poursuit le `GET` HTTP, qui n'en dépend pas ; les `Progression` sont perdus et **la barre gèle** — l'interface le dit (« progression indisponible »), jamais un pourcentage figé passé pour vrai |
| Agent tué pendant l'exécution | l'installeur survit (D8) ; l'issue est `issue_inconnue` ; la réconciliation suivante dit ce qui a réellement été installé |
| Installeur qui ne rend jamais la main | `EXPIRATION_EXECUTION` (proposé 2 h, **NON CALIBRÉE**) ; l'agent **ne le tue pas** — tuer un installeur au milieu est pire — il rapporte `issue_inconnue` et cesse d'attendre |
| Installeur qui désinstalle l'agent | l'agent meurt ; la plateforme le voit par `vu_a` (`SEUIL_INJOIGNABLE_MS = 90_000`, `plateforme/src/agents/fraicheur.ts:47` ⚠️ *(le numéro a dérivé — il était 37 ; la VALEUR, elle, est inchangée. Relevé par le sous-bloc G3, le 21 août 2026.)*) et annonce la VM `injoignable`. **Rien ne le répare automatiquement** ; le cadrage §7 prévoit que le hub le dise |
| Disque plein dans la VM | le `GET` échoue à l'écriture, refus typé avec la cause, fichiers partiels supprimés |
| Notifications perdues | latence, jamais perte : la réconciliation périodique rattrape (D1) |
| Version de protocole divergente | refus `version`, socket fermé, **aucun réessai** — comportement de P3, inchangé |

---

## 8. Stratégie de test

**Tout ce qui peut être pur l'est**, et se teste sur l'hôte Linux sans VM, sans
COM et sans socket : la normalisation et la clé d'identité, la règle de
filtrage, le diff de réconciliation, l'échéance d'anti-rebond (**horloge en
paramètre**, comme `plateforme/src/agents/fraicheur.ts` et `identite/jeton.ts`
le font déjà), la lecture d'un `GRPICONDIR`, le verdict d'installation, le
découpage en tranches côté navigateur.

**Le reste** : `cargo check --target x86_64-pc-windows-gnu` pour tout le
`#[cfg(windows)]` — **qui couvre types, emprunts, visibilités et durées de vie,
et PAS l'édition de liens** (acquis du sous-bloc D3, dont la portée est écrite
là-bas) ; les routes et les dépôts contre une base réelle et de vrais sockets,
avec la **double passe SQLite et Postgres** qu'exige `scripts/verify-all.sh`
(étapes `plateforme : npm run test:sqlite` et `test:postgres`, l. 82-86) ; et la
VM pour ce qui ne peut vivre que là.

⚠️ **Un saut est un échec** : si Postgres manque, la passe échoue, elle ne se
saute pas. C'est la règle de P1 §7.1, et ④ n'y déroge pas.

**Ce que la recette exige de la VM.** Les critères de G1, G3 et G4 sont
**tous** des critères de VM : ils portent sur le corpus réel de raccourcis, sur
un lancement réel, sur une installation réelle. **Seuls G2 ② et G2 ④ se jouent
entièrement sur l'hôte**, avec les deux fichiers témoins du §3.2 versés en
`testdata` — et c'est précisément le critère qui décide de tout le §3.2 qui n'a
pas besoin de la VM. Les **modules purs**, eux, sont couverts par des tests
d'hôte indépendamment des critères : c'est la couverture de base, pas la
recette.

🔴 **Quatre ROUGES sont GRATUITES et doivent être JOUÉES, pas supposées** — le
dépôt a payé trois fois pour un contrôle qu'il n'a jamais vu rouge (F1 de D7, la
sonde P1 de D8, le confondeur de D9) :

- **G1 ①** : la clé par cible seule rend 104 au lieu de 154 — **écart déjà
  mesuré sur la VM** (§3.1) ;
- **G2 ②** : `gapps-temoin-48.ico` rend un PNG 256×256 — **déjà mesuré** (§3.2) ;
- **G3 ①** : sur le binaire de G1, la route de téléversement n'existe pas ;
- **G3 ④** : le binaire de G1 perd un ordre d'installation à la chute du canal.

⚠️ **« Déjà mesuré » ne dispense PAS de jouer la rouge du TEST** : ce qui est
mesuré, c'est le fait du monde ; ce qui reste à voir rouge, c'est que le
contrôle le dénonce.

---

## 9. Ce que ④ n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux.
- **Aucune latence de bout en bout** — qu'aucun sous-bloc du chantier D n'a
  jamais mesurée, et que ⑤ ne mesure pas davantage.
- **Rien de la charge** : le nombre de téléversements simultanés qu'une
  plateforme soutient n'est pas mesuré, ni le débit du transfert vers la VM.
- **Rien d'un antivirus.** ⚠️ **L'état de l'antivirus de cette VM n'a PAS été
  relevé** — je ne l'affirme donc dans aucun sens. L'analyse d'un installeur de
  plusieurs centaines de mégaoctets peut ajouter un délai, voire une mise en
  quarantaine qui ferait échouer l'installation avec un code de sortie
  trompeur : **ni mesuré, ni prédit**.
- **Rien de l'élévation UAC** tant que la sonde de G3 n'est pas jouée. Si elle
  révèle que le bureau sécurisé n'est pas capturé, **une part des installeurs
  réels est hors de portée de la v1**, et ce sera écrit, pas contourné.
- **Aucune qualité visuelle d'icône n'est jugée** : `source_max_px` dit d'où
  vient l'image, jamais si elle est bonne. Même lacune que `BPP_MIN` traîne
  depuis le chantier C volet 1.
- **Aucune constante n'est calibrée** : `PERIODE_RECONCILIATION`,
  `DELAI_ANTI_REBOND`, `DELAI_ANTI_REBOND_MAX`, `EXPIRATION_TELEVERSEMENT`,
  `EXPIRATION_INSTALLEUR`, `EXPIRATION_EXECUTION`, la taille de tranche. Elles
  rejoignent la liste déjà longue de ce dépôt.
- **Rien d'un raccourci de l'espace de noms Shell** (les 7 à cible vide) : ni
  découverts, ni lancés.
- **Rien d'une application sans raccourci** — une application installée qui ne
  pose ni icône de bureau ni entrée de menu Démarrer est **invisible**, et ce
  document ne prévoit aucune autre source. C'est le trou structurel de la voie
  retenue, et il est nommé plutôt que découvert.
- **Rien des applications du Microsoft Store (UWP/MSIX)** : elles n'ont pas de
  `.lnk` au sens de D2 et vivent dans `shell:AppsFolder`. Hors périmètre v1.
- **Aucune restauration après un installeur destructeur** : l'isolation est la
  VM (cadrage §3), et ④ n'ajoute ni instantané, ni retour arrière — le backend
  d'orchestration v1 de ⑤ **refuse explicitement** `instantane` (spec ⑤ §3.6).
- **Aucun audit de sécurité** : le modèle de menace reste celui de ⑤, borné à
  « un pair anonyme n'obtient rien ». L'exécution d'un binaire arbitraire **dans
  la VM de son propre auteur** est, elle, une décision de produit déjà prise.
- **Rien d'un client réel, rien du HiDPI** : la recette reste un Chromium sans
  interface, limite héritée de D5 qu'aucun sous-bloc n'a levée.

---

## 10. Hors périmètre v1 (explicitement)

- **`.msc`, `.url`, `.bat`, `.msi` comme applications lançables** — seuls `.exe`
  entrent au catalogue (D3) ; `.msi` est **installable**, pas **lançable**.
- **Les raccourcis de l'espace de noms Shell** et l'identité par PIDL.
- **Les applications UWP/MSIX** et `shell:AppsFolder`.
- **Le rapprochement d'une application avec sa version antérieure** quand son
  répertoire d'installation change (D4).
- **Le mode silencieux d'installation** : aucun drapeau n'est deviné (D8).
- **La désinstallation depuis le hub** : les désinstalleurs sont au catalogue et
  se lancent comme tout le reste, mais ④ ne pilote aucun cycle de
  désinstallation.
- **La mise à jour des applications**, et toute notion de version.
- **Le partage d'un catalogue entre VMs** : `application.vm_id` est requis, une
  application appartient à une VM.
- **Toute modification du produit historique** (`src/`, `web/`, `index.js`) —
  cadrage §11.
- **La direction visuelle du hub** : ④ livre un hub **fonctionnel** ; ⑥ l'habille.
- **Le classement, la recherche, les catégories** du catalogue.

---

## 11. Risques, et ce qui rendrait ④ non livrable

| Risque | Ce qu'il coûte | Mitigation, ou constat |
| --- | --- | --- |
| 🔴 **L'élévation UAC n'est pas capturée** | tout installeur qui l'exige devient inutilisable : l'utilisateur voit un écran figé | **sonde préalable de G3**, éliminatoire pour cette moitié. Repli connu mais non arbitré : agent élevé — décision de sécurité, hors de ce document |
| 🔴 **Un installeur casse la VM ou l'agent** | la VM devient injoignable et **rien ne la répare** : le backend d'orchestration v1 refuse `instantane` et `demarrer` | **assumé par le cadrage** (§3). ④ ajoute la trace du périphérique audio (D8), rien d'autre |
| **Le corpus réel ne porte pas les deux valeurs de `source_max_px`** | le critère ② de G2 ne peut pas être exercé contre le produit, seulement contre les témoins | **le critère ③ existe pour le dire**, et il rend `NON MESURABLE` plutôt que `tenu` |
| **`ReadDirectoryChangesW` ne déborde jamais dans la recette** | le critère ② de G4 devient non mesurable | **déclaré tel quel.** La propriété qui compte — aucune perte d'application — est tenue par ③, qui, lui, se joue en désarmant la réconciliation |
| **`PLATEFORME_VERSION = 2` casse un agent déployé** | l'agent refuse le canal et **ne réessaie pas** : la VM ne s'établit plus du tout | **assumé, comme P3 D5**. Agent et plateforme se déploient au même commit |
| **`proto/src/plateforme.rs` franchit 500 lignes** | dette de taille sur un fichier neuf | **extraction AVANT addition**, tâche dédiée du premier sous-bloc |
| **Un `GET` de plusieurs centaines de Mo sature le canal de la VM** | l'établissement de session concurrent se dégrade | non mesuré. Le débit du pont est connu ≥ 1,44 Gb/s (D6), donc **probablement sans effet** — *probablement* est le mot juste, et il n'est pas remplacé par un chiffre |
| **La sonde d'icône n'a été jouée que sur UNE VM** | rien ne dit qu'un autre parc porte des 256 | **une exécution, une machine, un corpus**, et c'est écrit partout dans ce document |
| **Les fichiers de sonde vivent sur `C:\dev\` de la VM** | ils disparaîtront avec elle | **c'est pourquoi leurs sorties sont transcrites verbatim au §3** ; les deux témoins `.ico` sont **versés** avec G2 |
