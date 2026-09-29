# CLAUDE.md

> ⚠️ **CE FICHIER EST UN INDEX, PAS UNE ARCHIVE — et il l'a oublié une fois.**
> Il a atteint **1 111 970 octets / 17 199 lignes**, soit de l'ordre de
> **280 000 tokens payés à chaque session**, dont **87,5 % de journal de
> chantiers**. Ce journal a été déplacé **verbatim** vers
> [`docs/JOURNAL.md`](docs/JOURNAL.md) le 21 août 2026 (partition vérifiée à
> l'octet près : **rien n'a été réécrit, résumé ni supprimé**).
>
> **La règle qui en sort, et qui vaut pour toute addition future :** ce qui se
> lit **AVANT d'agir** vit ici ; ce qui se lit **quand on rouvre un chantier
> précis** vit dans `docs/JOURNAL.md` et dans `docs/superpowers/`.
> **Un relevé daté n'est pas une consigne** — il vieillit, et sa place est le
> journal.

## Où vit quoi

| Ce que tu cherches | Où |
| --- | --- |
| Une règle, une commande, une variable, un piège | **ici** |
| Le récit d'un sous-bloc, ses mesures, ses réfutations | [`docs/JOURNAL.md`](docs/JOURNAL.md) |
| Le détail d'un chantier : plan, conception, recette, journaux bruts | `docs/superpowers/{plans,specs}/` — **129 documents au premier niveau** (100 plans, 29 specs), **149 avec les journaux bruts** des sous-répertoires `journaux-*`, **tous suivis par git** — relevé par `find docs/superpowers -name '*.md' | wc -l`, jamais recopié |
| Le produit legacy, **retiré de l'arbre** le 21 août 2026 | `docs/legacy/`, et la Partie II du journal |

## Le produit

**Ce dépôt livre un bureau distant Windows accessible au navigateur, en
WebRTC.** Il ne reste rien du chemin Guacamole d'origine sur le chemin chaud :
voir « Le legacy » ci-dessous.

| Composant | Rôle |
| --- | --- |
| `agent/` (Rust, sur la VM Windows) | capture, encode, diffuse, injecte les entrées. **Un seul binaire, QUATRE modes de processus** |
| `plateforme/` (TypeScript, Node) | identité, signaling, orchestration, catalogue d'applications, HTTP |
| `client/` (TypeScript, Vite) | la page de session, la page-shell, l'écran de connexion, le hub |
| `proto/` (Rust **et** TypeScript) | le protocole partagé, épinglé des deux côtés par des fichiers de vecteurs |

**Les quatre modes d'`agent.exe`**, aiguillés dans `agent/src/main.rs` :

| Mode | Variable | Ce qu'il fait |
| --- | --- | --- |
| **superviseur** | `SUPERVISEUR` | détecte les fenêtres, leur donne une sortie virtuelle, lance et relance les autres |
| **capteur** | `CAPTEUR` | tient **toutes** les duplications DXGI et **tous** les encodeurs, sert chaque enfant par un tube nommé |
| **pont** | `PONT` | tient la racine ProjFS des fichiers, sur sa propre `PeerConnection` |
| **enfant** | *(aucune)* | une fenêtre = un processus = une `PeerConnection` : WebRTC, entrées, audio |

🔴 **Le fait d'architecture qui gouverne tout le reste : il y a N SESSIONS, pas
N pistes dans une session.** Chaque fenêtre est un processus avec sa propre
`PeerConnection`, donc son propre estimateur de bande passante. Toute lecture
qui suppose une session unique est fausse depuis le sous-bloc D1.

### Le legacy — ~~arrêté, son retrait engagé~~ **RETIRÉ**

✅ **RETIRÉ LE 21 AOÛT 2026, sur décision du propriétaire du dépôt.** Il ne
reste **aucun fichier** de l'ancien produit dans l'arbre : `index.js`, `src/`,
`web/`, `assets/`, `dist/`, `Dockerfile`, `docker-compose.yml` et
`test_winrm_*.js` sont supprimés, le conteneur `guacamole-web-1` et l'image
`guacamole-web` n'existent plus.

🔴 **CE QUI RESTE RÉCUPÉRABLE, ET PAR QUEL CHEMIN — la distinction est vitale,
parce que « l'historique git reste » était FAUX pour 890 de ces lignes :**

| Ce qui a disparu | Où le retrouver |
| --- | --- |
| Les fichiers **suivis par git** (2 280 lignes) | l'historique, jusqu'au commit de retrait |
| `web/index.js` (804 l.) et `index.js` (61 l.) — **jamais commités** | `docs/legacy/web-index.js` et `docs/legacy/racine-index.js`, **versionnés** |
| `docker-compose.yml` (25 l.) — **jamais commité, et porteur de mots de passe en clair** | sa **structure sans une valeur** dans `docs/legacy/README.md` §3 ; le fichier lui-même **hors du dépôt**, en `~/.guacamole-legacy/docker-compose.yml.legacy` (mode 600) |

⚠️ **`docs/legacy/` ne doit JAMAIS recevoir le fichier réel** : `docs/` est
versionné, et l'y déposer committerait les identifiants Windows.

⚠️ **Ce que le retrait n'a pas attendu** : sur les dix verrous de
`docs/superpowers/specs/2026-08-20-retrait-legacy-design.md`, **deux seulement
étaient satisfaits** (relevé du 21 août 2026,
`docs/superpowers/plans/2026-08-21-retrait-legacy-etat-des-verrous.md`). Les
fonctions que personne ne reprend sont nommées au §4.1 de la spec et au § « Ce
que le retrait emporte » du journal — **ce sont des régressions assumées, pas
des oublis**.

## 📏 Conventions de code

### Taille maximale d'un fichier : 500 lignes

**Un fichier de code source ne doit pas dépasser 500 lignes.** Au-delà, le
fichier porte plus d'une responsabilité : il faut le découper avant d'y ajouter
quoi que ce soit.

**Portée** — la règle s'applique au code source écrit à la main :
`agent/src/`, `client/src/`, `plateforme/`, `proto/`, `scripts/`.
*(`src/` et `web/` en sont retirés le 21 août 2026 : ces répertoires étaient
ceux du legacy, et ils n'existent plus.)*

**Exemptions explicites** :

- `docs/` — les plans, specs et recettes sont des journaux d'exécution, longs
  par nature et non maintenus comme du code.
- Fichiers générés ou vendorisés : `dist/`, `target/`, `node_modules/`,
  `*-lock.json`, `Cargo.lock`, `testdata/`.

**Règle d'application** : geler la dette, pas la purger. Aucun **nouveau**
fichier ne naît au-dessus de 500 lignes, et un fichier déjà au-dessus ne doit
pas grossir davantage — toute addition substantielle s'accompagne d'une
extraction. Le découpage rétroactif des fichiers ci-dessous se fait au moment
où l'on travaille dedans, pas en chantier séparé.

**Dette existante** (code source uniquement) :

| Fichier | Lignes | Pourquoi elle reste |
| --- | --- | --- |
| ✅ ~~`agent/src/encode.rs`~~ **SORTI DE LA DETTE** | ~~1536~~ → **427** (30 août 2026, lot 31) | 🔵 **Il ne franchit plus le plafond** — vérifié par la commande ci-dessus, qui ne rend plus que `windows_source.rs`. L'extraction du lot 31 l'a scindé en `encode/mft.rs` (301) + `encode/mft/convertisseur.rs` (337) + `encode/mft/encodeur.rs` (287) pour le dos Media Foundation, et `encode/natif.rs` (162) + le sous-arbre `encode/nvenc/` pour le dos NVENC. ⚠️ **Ces quatre tailles sont un RELEVÉ DATÉ du 30 août 2026, pas une source de vérité** — celle d'`encode/natif.rs` a dérivé TROIS fois dans le seul lot 31, à chaque fois qu'un commentaire d'en-tête était corrigé. **La source de vérité est la commande ci-dessus, relancée.** ⚠️ **Jouée dans des tâches DÉDIÉES et AVANT l'addition qu'elle préparait**, jamais par compression. ⚠️ `encode/arret.rs` reste à **500 lignes EXACTES**, donc à sa porte : toute addition dedans exige sa propre extraction |
| `agent/src/windows_source.rs` | ~~638~~ ~~628~~ **630** (7 août 2026, D10) | `#[cfg(windows)]`, aucun test |


> ⚠️ **LES RELEVÉS DE TAILLES DATÉS, SOUS-BLOC PAR SOUS-BLOC, SONT DANS LA
> PARTIE I DE [`docs/JOURNAL.md`](docs/JOURNAL.md)** — ils y pesaient
> **88 682 octets**, et **aucun n'est une consigne** : ce sont des instantanés
> qui vieillissent, et plusieurs se réfutent les uns les autres.
>
> 🔴 **LA SEULE SOURCE DE VÉRITÉ SUR LA TAILLE D'UN FICHIER EST LA COMMANDE
> CI-DESSOUS, RELANCÉE.** Ne jamais s'y fier pour décider si un fichier peut
> encore grossir — et **corriger le tableau de dette dans le même mouvement**.
> Ce dépôt a payé **neuf fois** le naufrage du « 487 » : un nombre recopié
> survit à la réalité qu'il décrivait.


**Vérifier l'état** :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

## 🗂️ Index des fichiers déplacés — à charger à la demande

Le détail de ce fichier vit dans `docs/claude/`. **Ne pas l'importer par `@`** : chaque fichier se lit (Read) quand la tâche le demande, sinon le contexte n'est pas réduit. Chaque fichier est déplacé **verbatim** ; rien n'a été réécrit ni résumé.

- [`docs/claude/conventions-modules.md`](docs/claude/conventions-modules.md) — Convention de module enfant `#[path]`/racine nue, et ses cas limites : à lire avant de créer ou d'extraire un module Rust.
- [`docs/claude/vm-cycle-de-vie.md`](docs/claude/vm-cycle-de-vie.md) — Démarrer, attendre, arrêter la VM Windows ; WinRM, montage /media/vm : avant toute commande qui touche la VM.
- [`docs/claude/commandes.md`](docs/claude/commandes.md) — Commandes de build, de test, de déploiement de l'agent, des services et des recettes navigateur.
- [`docs/claude/variables-environnement.md`](docs/claude/variables-environnement.md) — Variables du serveur (`plateforme/`) et de l'agent (banc et produit) : avant d'en ajouter, d'en lire ou d'en désarmer une.
- [`docs/claude/pieges-methode-de-mesure.md`](docs/claude/pieges-methode-de-mesure.md) — Comment mesurer sans se mentir (témoin négatif, rouge jamais vue, non-reproduction) : avant d'écrire ou de croire une mesure.
- [`docs/claude/pieges-documentation-taille-vm.md`](docs/claude/pieges-documentation-taille-vm.md) — Le naufrage du « 487 », la dérive des tailles copiées, l'outillage VM/Windows : avant de recopier un nombre ou de scripter la VM.
- [`docs/claude/pieges-shell-tests-recette.md`](docs/claude/pieges-shell-tests-recette.md) — Pièges du shell de l'hôte, des tests et des types, et de ce que le montage de recette ne voit pas : avant d'écrire un test ou une recette.
- [`docs/claude/index-chantiers.md`](docs/claude/index-chantiers.md) — Une ligne par chantier clos ou en cours, avec son document de résultats : pour retrouver le récit d'un chantier.
- [`docs/claude/legs-decisions-auth-pomerium.md`](docs/claude/legs-decisions-auth-pomerium.md) — Décisions réservées au propriétaire du dépôt et ce que `auth-pomerium` laisse dû : avant de toucher identité, proxy, jeton.
- [`docs/claude/legs-package-nivuus.md`](docs/claude/legs-package-nivuus.md) — Ce que le chantier `package-nivuus` laisse dû (packaging, installateur, déploiement) : avant de toucher au package Nivuus.
- [`docs/claude/legs-non-mesures.md`](docs/claude/legs-non-mesures.md) — Ce qu'aucun chantier n'a mesuré, les trois couches inconnues du chantier D, le tableau du dû par sous-projet : pour savoir ce qui reste ouvert.

## 📌 Tenir ce fichier

**Ce fichier a déjà dérivé une fois, jusqu'à peser 87,5 % d'archive.** Trois
gestes suffisent à l'en empêcher :

1. **Un chantier qui se clôt ajoute UNE LIGNE à l'index** et son récit à
   `docs/JOURNAL.md` — jamais l'inverse.
2. **Un relevé daté n'a pas sa place ici.** S'il porte une date, il va au
   journal ; s'il porte une règle, il reste.
3. **Un piège payé DEUX FOIS remonte ici**, condensé en une ligne, depuis le
   § « Pièges neufs » de son chantier.

⚠️ **`CLAUDE.md` n'est PAS exempté de la règle des 500 lignes** : il a été
découpé le 29 septembre 2026 (1667 → 165 lignes) et le détail vit dans
`docs/claude/`. Quand il approche de 500, on déplace vers `docs/claude/` ou vers
le journal, on ne compresse pas.
