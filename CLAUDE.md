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
- `CLAUDE.md` — ce fichier est un index de connaissances, pas du code.
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
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

### Convention de module enfant : `#[path]` chez le parent, ou racine nue

**Tranché le 7 août 2026 (tâche 17, sous-bloc D10), après que le sous-bloc
précédent a relevé `survie_verdict.rs` comme une « déviation » puis s'est
lui-même trompé en la corrigeant (voir le leg n°9 de D9, plus bas). Ce dépôt
avait deux conventions pour un module hors du `#[cfg(windows)]` de son parent
logique, sans jamais avoir écrit la règle qui les départage.

**Portée de la règle** : elle ne s'applique QU'aux modules qu'on extrait d'un
fichier `#[cfg(windows)]` (ou autrement non portable) pour que leur logique
*pure* compile et se teste sur l'hôte Linux, et qui doivent de ce fait devenir
des **frères de premier niveau** de ce parent, déclarés dans `main.rs`. Un
module `#[cfg(windows)]` ordinaire qui n'a pas besoin d'exister sur l'hôte
reste un enfant normal, déclaré par un simple `mod` **à l'intérieur** de son
parent gaté (`capture.rs::mod enumeration;`, `capture.rs::mod types;`,
`windows_source.rs::mod redimensionnement;`) : il ne se pose jamais la
question ci-dessous, faute d'avoir jamais besoin de sortir de l'arbre de son
parent. Est également hors de portée l'usage de `#[path]` pour scinder un
module de *tests* trop long À L'INTÉRIEUR d'un fichier par ailleurs portable
(`superviseur/table.rs` déclare ainsi `#[path = "table/tests.rs"] mod tests;`
et `#[path = "table/tests_relance.rs"] mod tests_relance;`, tous deux
`#[cfg(test)]`, tous deux internes à `table.rs`, sans rapport avec une
frontière `#[cfg(windows)]`) : c'est le même mécanisme Rust, employé pour une
raison différente (la règle des 500 lignes), et il ne suit pas la convention
ci-dessous.

**La règle, pour les modules dans cette portée : le NOM du module tranche.**

- **Le nom du module s'écrit `<parent>_<enfant>`**, où `<parent>` nomme un
  module de premier niveau existant (déclaré dans `main.rs`) : le fichier
  reste physiquement chez ce parent (`src/<parent>/<enfant>.rs`), et se
  déclare dans `main.rs` par
  `#[path = "<parent>/<enfant>.rs"] mod <parent>_<enfant>;` — c'est la
  déclaration qui franchit le `#[cfg(windows)]` du parent, le fichier
  physique, lui, n'a pas bougé de sous son parent. Exemples :
  `capture_reprise` (`capture/reprise.rs`), `windows_source_sortie`
  (`windows_source/sortie.rs`), `windows_source_telemetrie`
  (`windows_source/telemetrie.rs`).
  ⚠️ **Si plusieurs modules de premier niveau sont chacun un préfixe valide du
  nom** (cas non encore rencontré, mais qui existe dès aujourd'hui :
  `windows_source_sortie` est LUI-MÊME un module de premier niveau depuis D1,
  donc un futur `windows_source_sortie_conversion` préfixerait à la fois
  `windows_source` et `windows_source_sortie`), **c'est le préfixe le PLUS
  LONG — le plus spécifique — qui l'emporte.** Le fichier physique suit :
  `windows_source_sortie_conversion` se rangerait sous
  `windows_source/sortie/conversion.rs` (enfant de `windows_source_sortie`,
  lui-même à `windows_source/sortie.rs`), pas sous
  `windows_source/sortie_conversion.rs`. Cette clause ne change le
  classement d'aucun des six cas relevés ci-dessous : aucun n'a de second
  préfixe candidat plus court.
  **Une égalité de longueur entre deux préfixes candidats ne peut pas se
  produire** : un préfixe valide s'arrête toujours sur une frontière de
  tiret bas (`<parent>_`). Si deux noms de module de premier niveau
  DIFFÉRENTS étaient chacun un préfixe de la MÊME longueur du nom à ranger,
  ils seraient la même sous-chaîne — donc le même nom. L'égalité est
  structurellement exclue, pas seulement absente des cas rencontrés à ce
  jour ; il n'y a donc rien à trancher au-delà de « le plus long l'emporte ».
- **Le nom du module se comprend SANS référence à un parent** — il ne porte
  le préfixe d'aucun module de premier niveau existant (`geometry`,
  `sortie_dxgi`, `survie_verdict`) : il vit à la racine nue, `mod <nom>;`
  ordinaire dans `main.rs`, fichier `src/<nom>.rs`. **La profondeur du module
  dont on l'extrait ne change rien** : `survie_verdict` vient de
  `diagnostics::multifenetre::mode_sortie::persistance`, quatre niveaux plus
  bas que `main.rs`, et n'a pourtant aucun nom de parent court et unique à
  préfixer — la règle le range à la racine comme `geometry` et `sortie_dxgi`,
  extraits pour la même raison (compiler sur l'hôte) d'un parent tout aussi
  gaté (`capture.rs`, `window.rs`).

**Cas du parent qui n'existe pas encore quand on écrit le module** : la règle
se résout mécaniquement vers la racine nue — un nom ne peut préfixer un
module de premier niveau qui n'est pas encore déclaré dans `main.rs`. **Effet
de bord non traité** : si un module homonyme du préfixe apparaît plus tard
(un futur `mod windows_source_sortie_conversion` créé avant que
`windows_source_sortie` existe, par exemple), rien ne force à re-hisser le
premier sous le second après coup — aucune règle de re-hissage n'est posée
ici, à écrire le jour où le cas se présente réellement.

**Vérifiée sur les six cas existants au 7 août 2026**
(`grep -rn '#\[path' agent/src/`, `ls agent/src/*.rs`, depuis `agent/`) : les
trois noms préfixés sont TOUS déclarés par `#[path]` chez leur parent, les
trois noms autonomes sont TOUS à la racine nue. **Aucune exception**, y
compris sous la clause du préfixe le plus long ci-dessus.
`survie_verdict.rs` s'y conforme déjà — il n'a jamais eu besoin de bouger.

**Preuve, portée ici plutôt que dans un rapport de tâche gitignoré, que la
clause du préfixe le plus long ne change le classement d'aucun des trois
noms préfixés** : pour chacun, aucun AUTRE module de premier niveau n'en est
un préfixe plus court et valide.
- `capture_reprise` : seul `capture` le préfixe.
- `windows_source_sortie` : seul `windows_source` le préfixe.
- `windows_source_telemetrie` : seul `windows_source` le préfixe —
  `windows_source_sortie` n'en est PAS un préfixe : le nom continue par
  `_telemetrie`, pas par `_sortie`.

## 🖥️ Cycle de vie de la VM Windows

> 🔴 **CE QUI SUIT DÉCRIT LA VM DE DÉVELOPPEMENT D'AVANT LE CHANTIER
> `package-nivuus` (29 août 2026) — trois faits que ce fichier ne disait
> nulle part avant ce chantier, et qui ont coûté une demi-journée à
> retrouver :**
>
> 1. **La VM cible est désormais une APPLIANCE**, provisionnée par le
>    package voisin `packages/installer` (dépôt `console`) : `C:\dev`, la
>    chaîne Rust et le montage CIFS décrits juste en dessous **n'existent
>    plus, retirés délibérément** — vérifié le 29 août 2026 (`/media/vm` est
>    aujourd'hui un répertoire vide, non monté).
> 2. **WinRM n'accepte plus Basic** : `scripts/winrm.js` (compte
>    `Administrateur`, transport Basic, cité plus bas) **ne fonctionne
>    plus** (401 mesuré le 22 août 2026). Le chemin qui répond aujourd'hui
>    est `installer/console/guest/winrm_exec.py`, en **NTLM**, mot de passe
>    lu depuis `/root/.config/nivuus/windows-admin.pass` (jamais sur
>    l'argv).
> 3. **Le lot 3 du chantier `package-nivuus` (une campagne de douze items de
>    mesure visant cette VM) est SUSPENDU** pour cette raison — spec et plan
>    restent valides pour le jour où la VM sera rééquipée pour les recevoir.
>
> Voir
> [`docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md`](docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md)
> pour le détail.
>
> ⚠️ **Le sort de `scripts/winrm.js`, de `/media/vm` et de
> `scripts/build-agent.sh` ci-dessous — qui visent tous cette VM de
> développement qui n'existe plus sous cette forme — N'EST PAS TRANCHÉ.**
> Les corriger, les retirer ou les garder est une décision du propriétaire
> du dépôt, pas de ce chantier. Ce qui suit reste donc écrit tel quel,
> **à lire désormais comme un relevé historique**, jusqu'à cette décision.

**La VM cible est une machine libvirt/QEMU nommée `Windows`, et elle n'est pas
démarrée automatiquement.** Tout travail touchant l'agent Rust, la capture, la
recette WebRTC ou WinRM exige qu'elle tourne. Symptômes d'une VM éteinte :
`192.168.3.2` injoignable (« Aucun chemin d'accès pour atteindre l'hôte
cible ») et `/media/vm/` vide.

```bash
# État
virsh list --all            # « fermé » = éteinte, « en cours d'exécution » = démarrée

# Démarrer, puis attendre que WinRM réponde (~5 à 60 s)
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done

# Puis attendre que le partage de fichiers soit monté : /media/vm se monte
# après que WinRM répond, pas en même temps — un `scripts/build-agent.sh`
# lancé dès que le port 5985 répond échoue avec « erreur : /media/vm n'est
# pas monté » (`scripts/sync-agent.sh`, qui teste le montage, pas le port).
# `mountpoint -q` ne suffit pas : /media/vm est un montage CIFS
# (//192.168.3.2/c) dont l'entrée persiste dans la table de montage même VM
# éteinte et connexion morte — il faut éprouver un ACCÈS réel, pas la seule
# présence de l'entrée.
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done

# Arrêter proprement
virsh shutdown Windows
```

Une fois la VM démarrée, l'outillage habituel redevient disponible :

```bash
set -a && source .env && set +a      # charge les identifiants (fichier gitignoré)
node scripts/winrm.js '<commande PowerShell>'
scripts/build-agent.sh               # synchronise puis compile SUR la VM
scripts/run-agent.sh                 # lance l'agent en session interactive
```

**Ne jamais supposer la VM allumée.** Vérifier `virsh list --all` avant toute
séquence qui en dépend, et la démarrer si besoin — c'est une opération sûre et
idempotente.

⚠️ **`scripts/build-agent.sh` lancé sans avoir sourcé `.env` s'arrête EN
SILENCE**, après sa ligne « sources synchronisées », sans message ni statut
d'erreur : son `set -euo pipefail` avorte sur l'affectation du quota WinRM, dont
le `2>/dev/null` mange la cause. **Le symptôme se lit exactement comme une
compilation réussie et muette** — on mesure alors le binaire précédent sans
qu'aucune trace ne le dise. Relevé le 2 août 2026, revue finale du sous-bloc D2.
Sourcer `.env` d'abord, et se méfier d'un build qui ne dit rien.

---


## 🔨 Commandes

**Tout commence par l'environnement.** `.env` est **gitignoré** et porte les
identifiants Windows et TURN :

```bash
set -a && source .env && set +a
```

⚠️ **`scripts/build-agent.sh` lancé sans avoir sourcé `.env` s'arrête EN
SILENCE**, après sa ligne « sources synchronisées », **sans message ni statut
d'erreur** — et le symptôme se lit exactement comme une compilation réussie et
muette. Voir les pièges.

⚠️ **`.env` ne porte AUCUNE variable `PLATEFORME_*`** : la plateforme ne
démarre donc pas après un simple `source .env`, et elle dit pourquoi.

### L'agent, sur la VM

⚠️ **Ce tableau documente le workflow de développement D'AVANT le 29 août
2026 — voir la réserve en tête de « Cycle de vie de la VM Windows » ci-dessus :
`C:\dev`, le montage CIFS et `scripts/winrm.js` (transport Basic) visent une
machine qui n'existe plus sous cette forme ; leur sort n'est pas tranché.**

| Commande | Ce qu'elle fait |
| --- | --- |
| `scripts/sync-agent.sh` | synchronise les sources Rust vers `C:\dev` (via `/media/vm`) |
| `scripts/build-agent.sh` | synchronise **puis compile sur la VM** |
| `scripts/run-agent.sh` | génère `C:\dev\run-agent.ps1` et lance l'agent en **session interactive** (tâche planifiée `/it`) |
| `scripts/check-session.sh` | 🔴 vérifie que l'agent tourne en **session 1** et non en session 0 — un agent en session 0 ne peut ni capturer une fenêtre ni injecter d'entrées |
| `scripts/stop-agent.sh` | arrête l'agent |
| `scripts/sonde-multifenetre.sh` | enchaîne les sondes du chantier D, **une par exécution du binaire** (ces API échouent par plantage de processus) |
| `node scripts/winrm.js '<PowerShell>'` | exécute une commande sur la VM — ⚠️ **en session 0** |

⚠️ **Ce piège aussi vise la compilation SUR la VM de développement d'avant le
29 août 2026 — même réserve qu'en tête de section : le sort de ce chemin
n'est pas tranché.** Trouvé par une recherche par le SENS (« l'horloge de la
VM », « le rlib ») après qu'une première recherche par motifs littéraux
l'avait manqué — voir
[`docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md`](docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md).

🔴 **Avant toute compilation qui touche `proto/`** — l'horloge de la VM avance
sur celle de l'hôte, et cargo saute alors le rlib de `proto` :

```bash
cargo clean --release -p proto -p agent   # les DEUX crates, jamais -p agent seul
```

### Les tests

```bash
cargo test --workspace                              # agent + proto
cargo check --target x86_64-pc-windows-gnu          # vérifie le code #[cfg(windows)] SUR L'HÔTE
cd client && npx vitest run                         # client/src/ SEUL
cd client && npx vitest run --dir ../proto          # 🔴 proto/ts/ — DEUX commandes, jamais une
cd plateforme && npm run test:sqlite                # et test:postgres — la double passe
./scripts/verify-all.sh                             # les 10 étapes
```

🔴 **`./scripts/verify-all.sh` N'EST PAS HERMÉTIQUE** : avec `TURN_URL` et
`TURN_SECRET` dans l'environnement — c'est-à-dire **après le `source .env` que
tout travail sur la VM exige** — six tests de signaling échouent. Le lancer
depuis un shell propre :

```bash
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

⚠️ **Le script compte DIX étapes et l'exécution affiche DIX-HUIT en-têtes
`==>`** (les huit de plus viennent de `client : npm run design:verifier`).
**Les deux sont vrais de choses différentes : dire lequel on annonce.**

⚠️ **`cargo check` : vérifier la NATURE des avertissements, jamais leur
nombre** — il dérive avec la fraîcheur du build. La formule « tous
`dead_code` » **n'est plus vraie du dépôt** (un `unused_variables` dans
`micro.rs`, quatre `clippy` hors famille). ⚠️ **Ne jamais compter par
`grep -c '^warning'`** : cela compte aussi la ligne de résumé.

### Les services

```bash
cd plateforme && npm start                    # exige PLATEFORME_HOTE et PLATEFORME_SECRET_JETON, sans défaut
docker compose -f docker-compose.plateforme.yml up -d          # Postgres de test
docker compose -f docker-compose.yml -f docker-compose.coturn.yml up -d coturn
scripts/netem.sh <lan|adsl|4g|congestionné|effondrement|off>   # dégradation réseau, TOUJOURS reposer `off`
```

**Administration de la plateforme** (`cd plateforme`) :
`npm run admin:utilisateur` (mot de passe **sur stdin seul**, jamais en `argv`),
`npm run admin:agent` (⚠️ `--vm` attend l'**identifiant**, pas le nom, et
`--adresse` n'a aucun défaut), `npm run admin:attribuer`.

### Les recettes navigateur

| Instrument | Ce qu'il établit |
| --- | --- |
| `client/verify-webrtc.mjs` | que le média traverse — **jamais par quel chemin** |
| `client/recette/paire-candidats.mjs` | la paire de candidats réellement employée, le RTT, le débit. `FORCER_RELAIS=1` impose `iceTransportPolicy: 'relay'` |

⚠️ **Toute mesure de plus de 5 minutes** exige `--disable-background-timer-throttling`,
`--disable-backgrounding-occluded-windows` et `--disable-renderer-backgrounding` :
sans eux, Chrome gèle une page jamais mise au premier plan, et la session tombe
vers 331–340 s.

---

## 🎛️ Variables d'environnement

**C'est le seul inventaire de ces variables dans tout le dépôt** — il agrège
celles du produit et celles des bancs, tous chantiers confondus.

🔴 **DEUX CONVENTIONS COEXISTENT, ET LES CONFONDRE INVERSE LE COMPORTEMENT :**

| Convention | Sens | Pourquoi |
| --- | --- | --- |
| **`=0` DÉSARME**, une simple présence n'active pas | ce qui est **LIVRÉ** | tester `is_ok()` **armerait** le mécanisme chez qui écrit `X=0` pour le **couper** |
| **absente = DÉSARMÉE**, présente = armée | ce qui **NE L'EST PAS** (bancs, injections de faute) | on arme sur `=1` ce qu'on ne livre pas |

⚠️ **Une variable de banc n'est JAMAIS une configuration livrée**, et chacune le
dit dans sa propre trace.

🔴 **Toute variable neuve doit être ajoutée à `scripts/run-agent.sh` par une
TÂCHE DÉDIÉE**, et le contrôle qui vaut est de **lire la ligne dans le
`run-agent.ps1` GÉNÉRÉ** — jamais de tracer le code. Voir les pièges.

### Variables du SERVEUR (`plateforme/`)

**Relevées dans `plateforme/src/config.ts`**, pas recopiées. ⚠️ **`.env` n'en
porte AUCUNE** : `cd plateforme && npm start` après un simple `source .env` ne
démarre donc pas — et il dit pourquoi.

| Variable | Effet |
| --- | --- |
| `PLATEFORME_HOTE` | l'adresse d'écoute. 🔴 **AUCUN DÉFAUT — le service REFUSE de démarrer sans elle.** C'est délibéré : un défaut, même `127.0.0.1`, ferait passer le critère « l'écoute est bornée » **sans rien garantir**, là où l'ancêtre écoutait sur toutes les interfaces et délivrait des identifiants TURN de 86 400 s à quiconque. 🔴 **ET DEPUIS `auth-pomerium` (21 août 2026), UNE SECONDE GARDE REFUSE LE DÉMARRAGE : en mode `pomerium` — LE DÉFAUT —, les quatre écoutes UNIVERSELLES `0.0.0.0`, `::`, `[::]` et `*` LÈVENT.** ⚠️ **Cela mord sur la valeur avec laquelle un service tournait la veille** : un montage à `0.0.0.0` ne démarre plus du tout, et le message nomme la variable et la raison. **Pourquoi liée au mode** : en `motdepasse` le service s'authentifie lui-même ; en `pomerium` l'identité arrive dans un en-tête EN CLAIR (`X-Pomerium-Claim-Email`, aucune signature vérifiée), qu'une écoute universelle offre à quiconque atteint la machine. ⚠️ **Ce que la garde NE promet PAS**, et son propre commentaire le dit : elle refuse l'écoute **universelle**, elle ne garantit pas que « seul le proxy atteint le port » — cela reste à la charge de l'exploitant |
| `PLATEFORME_SECRET_JETON` | le secret HMAC des jetons. 🔴 **AUCUN DÉFAUT**, et **refusée si plus courte que 32 caractères** — un défaut aléatoire invaliderait toutes les sessions à chaque redémarrage, un secret court n'authentifie personne |
| `PLATEFORME_PORT` | défaut **8080**. Un port non entier **lève**, jamais ne retombe sur le défaut |
| `PLATEFORME_BASE` | `sqlite` (défaut) ou `postgres`. **Une valeur inconnue LÈVE**, à deux endroits |
| `PLATEFORME_BASE_URL` | chemin SQLite (défaut `:memory:`) ou URL `pg`. ⚠️ **`pg` prend `:memory:` pour un nom d'hôte** |
| `PLATEFORME_AUTH` | `pomerium` (défaut) ou `motdepasse`. 🔴 **UN CHOIX DE MODE, PAS UN ARMEMENT** — la convention `=0 désarme` de `agent/` ne s'applique pas ici, précédent `PLATEFORME_BASE` deux lignes plus haut, et pour la même raison : un repli silencieux ferait tourner un mode sous le nom de l'autre, et l'un des deux sens est une **ouverture**. **Une valeur inconnue LÈVE.** En mode `pomerium`, `GET /auth/moi` échange l'en-tête `X-Pomerium-Claim-Email` posé par le proxy contre le MÊME jeton interne que le mot de passe, et `POST /auth/connexion`/`/auth/rafraichir` rendent le 404 générique (route retirée). 🔴 **ELLE NE CHOISIT PAS QUE DES ROUTES : ELLE ARME UNE GARDE D'ÉCOUTE**, et ce tableau ne l'a pas dit pendant toute la branche. En mode `pomerium`, `PLATEFORME_HOTE` **refuse le démarrage** sur `0.0.0.0`, `::`, `[::]` et `*` (voir sa ligne, cinq plus haut) — un refus de démarrer se lit AVANT d'agir, pas après. 🔴 **ET LE PROFIL DE DÉPLOIEMENT LA POSE À `motdepasse`, AVEC UNE DIRECTIVE NGINX QUI VA AVEC** : `docker-compose.plateforme.yml` écrit `PLATEFORME_AUTH: motdepasse` et `deploiement/nginx.conf` efface l'en-tête entrant par `proxy_set_header X-Pomerium-Claim-Email "";` — **les deux ensemble, et elles s'inversent ensemble** ; n'en appliquer qu'une moitié donne soit un contournement complet de l'authentification, soit un service que personne ne peut atteindre. Invariant ⑤ de `deploiement/README.md`. Chantier `auth-pomerium`, tâches 1 à 3 — voir l'index des chantiers |
| `PLATEFORME_ORIGINE_CLIENT` | l'origine CORS. **FACULTATIVE, et son défaut est le REFUS** : absente, aucun en-tête CORS n'est émis. **Jamais `*`, sous aucune condition.** ⚠️ Son absence est le cas **nominal** derrière le proxy, où page et API partagent l'origine |
| `PLATEFORME_PROXY_DE_CONFIANCE` | 🔴 **~~FACULTATIVE, SANS CONDITION~~ — FAUX DEPUIS `auth-pomerium` (21 août 2026) : facultative en mode `motdepasse`, OBLIGATOIRE en mode `pomerium`, où `lireConfig` REFUSE DE DÉMARRER sans elle.** Absente en `motdepasse`, `X-Forwarded-For` **n'est pas cru du tout** — le défaut sûr. **Mal posée** (trop large, ou une valeur qui n'est pas celle du proxy), **le frein par adresse dégénère en frein GLOBAL** et le premier attaquant bloque tout le monde ; le seul endroit où cela se voit est la ligne de journal du frein, qui **nomme l'adresse retenue**. 🔴 **ELLE PORTE DÉSORMAIS DEUX RÔLES, PAS UN** : ① le crédit de `X-Forwarded-For` (ci-dessus, inchangé) ; ② **l'autorisation de poser l'en-tête d'identité** — `routes-identite.ts::servirIdentite` n'accepte `X-Pomerium-Claim-Email` que d'un pair dont `req.socket.remoteAddress` figure dans cet ensemble (`pairDeConfiance`, `http/adresse-source.ts`), sinon `401 pair-non-de-confiance` **avant même de lire l'en-tête**. C'est ce second rôle qui la rend obligatoire en mode `pomerium` : sans lui, l'identité arrive dans un en-tête EN CLAIR qu'aucune signature ne vérifie, et quiconque atteint le port obtient un jeton pour l'identité de son choix — voir le legs `/auth/moi`, ci-dessous, **FERMÉ** par cette garde. 🔴 **ET L'ENSEMBLE RETENU EST ANNONCÉ AU DÉMARRAGE depuis le 22 août 2026** : `proxys de confiance retenus=<entrées> nombre=<n>`, ou `retenus=aucun`. C'est ce qui rend visible **un nom d'hôte** — qui ne correspond à AUCUNE `remoteAddress` et fait donc refuser TOUT LE MONDE, service répondant, sans qu'aucune requête ne le trace. ⚠️ **Le refus par requête reste non tracé, à dessein** : une trace par requête rendrait le service amplificateur. `plateforme/src/http/annonces.ts` |
| `PLATEFORME_ICONES` | **FACULTATIVE**, défaut `donnees/icones`. ⚠️ Une valeur **vide** retombe sur le défaut, à dessein |
| `PLATEFORME_TELEVERSEMENTS` | **FACULTATIVE**, défaut `donnees/televersements`. Même raison : une valeur vide ferait de la racine le magasin |
| `PLATEFORME_PAGE` | **Correction, 22 août 2026 — la plateforme SERT DÉSORMAIS LA PAGE BÂTIE**, ce qu'elle ne faisait pas avant ce lot (`plateforme/src/http/page/`). **FACULTATIVE, et AUCUN DÉFAUT** — à la différence de `PLATEFORME_ICONES` et `PLATEFORME_TELEVERSEMENTS` juste au-dessus. 🔴 **ABSENTE OU VIDE ⇒ AUCUN SERVANT, et `GET /` rend le `404 introuvable` D'HIER À L'OCTET PRÈS** : c'est ce qui rend l'ajout strictement additif. ⚠️ **UN DÉFAUT SERAIT UN DÉFAUT DE SÉCURITÉ, PAS UNE COMMODITÉ** : dans le montage nginx (docker-compose, mode `motdepasse`), la plateforme ne doit RIEN servir — nginx sert déjà `client/dist` — et un défaut la ferait publier ce que son répertoire courant contient. Valuée vers la racine `client/dist` **bâtie** (`npm run build`). Lue dans `plateforme/src/config.ts`, servie par `plateforme/src/http/page/routes-page.ts` (garde contre les liens symboliques par `realpath`, voir son commentaire). 🔴 **LE DÉMARRAGE L'ANNONCE, DEPUIS LE 22 AOÛT 2026 — et il ne le faisait pas quand ce lot a été livré** : `page servie racine=<chemin RÉSOLU> lisible=oui`, ou `racine=aucune` quand elle n'est pas posée, ou un **`console.error`** portant `lisible=non` quand elle l'est et que le disque refuse. **Sans cette ligne, une racine inexistante rendait `404` sur toute page, STRICTEMENT indiscernable de la variable absente.** `plateforme/src/http/annonces.ts` |

**Le relais TURN** lit `TURN_URL` et `TURN_SECRET` (sans `TURN_URL`, il n'annonce
aucun relais **et le journalise**), et `docker-compose.coturn.yml` exige
`TURN_LISTENING_IP` **et** `TURN_RELAY_IP`, **tous deux obligatoires** — borner
la seule écoute laisserait les allocations de relais sur toutes les interfaces.

🔴 **Vérifier l'environnement du processus QUI ÉCOUTE, jamais de celui qu'on
croit avoir lancé** — un signaling a tourné 36 h sans ses variables TURN, sans
que rien ne le signale côté client :

```bash
P=$(ss -ltnp | grep ':8080 ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1)
tr '\0' '\n' < /proc/$P/environ | grep ^PLATEFORME_
```

### Variables de l'AGENT — ~~du banc et des sondes~~ du banc **ET DU PRODUIT**

> ⚠️ **Le titre d'origine disait « du banc et des sondes ». Il est devenu FAUX** :
> la table a accueilli depuis des variables de **produit** — `AUDIO_PERIPHERIQUE`,
> `MICRO`, `PLEIN_ECRAN`, `PRESSE_PAPIER`, `PONT_MUTATION`, `ACCENT`… — et chaque
> ligne dit laquelle elle est. **Barré plutôt qu'effacé**, comme ce dépôt le fait
> partout ailleurs.

Un processus par voie (ces API échouent par **plantage du processus**, pas par
code d'erreur).

| Variable | Effet |
| --- | --- |
| `MULTIFENETRE_DXGI=1` | Relève la topologie DXGI et sort — le contrôle d'état depuis un processus neuf |
| `MULTIFENETRE_WGC=1` | Sonde `Windows.Graphics.Capture` (voie éliminée) |
| `MULTIFENETRE_REPLIS=1` | Sonde les voies de repli |
| `MULTIFENETRE_BANC=duplication\|printwindow` + `MULTIFENETRE_N=1..8` | Le banc de cadence |
| `MULTIFENETRE_SORTIE=<\\.\DISPLAYn>` | Force la sortie DXGI capturée par le banc. **Un NOM, plus un couple d'index** depuis D2 : `DesktopCapture::sur_sortie` résout par nom (les index sont positionnels). Un `0:1` récolte « aucune sortie DXGI nommée 0:1 » |
| `MULTIFENETRE_CONTRAT=1` | Éprouve le contrat IOCTL du pilote (deux tampons simples, sans effet de bord) |
| `MULTIFENETRE_VDD=1` | **Mesure ①** — montée en N de sorties virtuelles jusqu'au refus |
| `MULTIFENETRE_VDD_VEILLE=<secondes>` | Épreuve du chien de garde : une sortie, aucun ping, relevé à 1 Hz |
| `MULTIFENETRE_VDD_PURGE=1` | **Purge autonome** des sorties orphelines |
| `MULTIFENETRE_VDD_CAPTURE=1` | **Mesure ③** — crée une sortie virtuelle et y lance le banc |
| `MULTIFENETRE_NVENC=partage\|separe` | **Mesure ②** — plafond d'encodeurs, périphérique D3D11 partagé ou un par encodeur |
| `MULTIFENETRE_NVENC_CYCLES=<k>` | **Sous-bloc D5, la mesure pivot** — monte jusqu'au refus dans l'arrangement de PRODUCTION (un périphérique D3D11 par encodeur), puis répète *k* fois « détruire un, en construire un ». **Le cycle répété est le cœur** : un seul recyclage ne distingue pas un plafond de concurrence d'un plafond de créations cumulées. Branché **avant** `MULTIFENETRE_NVENC`, les deux variables ayant un préfixe commun |
| `MULTIFENETRE_VDD_PARALLELE=<1..8>` | **Chantier des duplications parallèles** — N sorties virtuelles × 1 fenêtre × 1 duplication DXGI × 1 encodeur, trois passes (témoin, capture, capture+encodage), contrôle d'image **en rotation**, chien de garde pingué à 1 Hz |
| `MULTIFENETRE_EPREUVE_FILE_MS=<ms>` | Bouche la file de travail sérialisée imposée à la MFT pendant la passe — c'est l'épreuve qui montre que la barrière n'est pas un placebo |
| `MULTIFENETRE_REPRISE=<k>` | **Sous-bloc D2** — *k* sorties virtuelles, *k* duplications, puis **une sortie de plus** créée en cours de capture : éprouve que les *k* duplications reprennent et rendent encore des images justes. Sonde post-mortem sur les voies mortes |
| `MULTIFENETRE_PLAFOND=<P>x<D>` | **Sous-bloc D3** — le **porteur** : crée K = P×D sorties virtuelles, bat le chien de garde, **ne duplique rien lui-même**, lance P processus **sondes** en escalier (la *i+1* attend que la *i* soit prête ou en échec), puis détruit ses sorties et compare la topologie **par ensemble de noms** |
| `MULTIFENETRE_PLAFOND_SONDE=<noms,séparés,par,virgules>` | **Sous-bloc D3** — la **sonde**, posée par le porteur et **jamais à la main** (avec `MULTIFENETRE_PLAFOND_RANG=<i>`). Ouvre une duplication par nom, les tient jusqu'au signal d'arrêt, journalise le `HRESULT` exact. **Branche en tête de tout l'aiguillage** : sans quoi un `MULTIFENETRE_VDD_PURGE=1` résiduel, hérité par l'enfant, détruirait les sorties vivantes du porteur en pleine mesure |
| `CAPTEUR=1` | **Sous-bloc D4** — lance l'agent en **capteur** de capture mutualisée (serveur du tube `\\.\pipe\agent-capteur`). Posée par le superviseur lui-même (`lancer_capteur`), pas à la main ; transmise par `scripts/run-agent.sh`. Un capteur qui hériterait de `SUPERVISEUR` se prendrait pour un superviseur |
| `AGENT_TRACE_EXCEPTIONS=1` | Arme le filtre d'exception (pile symbolisable de la faute, journal séparé d'`agent.log`). Inerte sans la variable. `…_FICHIER` en change la destination ; `…_AUTOTEST=1` **tue délibérément le processus** pour éprouver l'instrument |
| `BUDGET_BPS=<bps>` | **Sous-bloc D6** — le **budget de débit de toute la session**, lu par le **capteur** seul (`capteur/sommeil/parts.rs`), qui le découpe en parts et les pousse aux enfants. Défaut **12 000 000**. Transmise par `scripts/run-agent.sh` (tâche 9). C'est une variable de **produit**, pas de banc. Trace de contrôle : `budget de debit de la session budget_bps=<valeur>` — **comparer la VALEUR, jamais la seule présence de la ligne**, et **pas avant la première fenêtre** : elle vient d'un `OnceLock` initialisé au premier calcul de parts |
| `SOURCE_TRACE=1` | ⚠️ **MORT DES DEUX CÔTÉS en multi-fenêtres** (constaté le 3 août 2026, sous-bloc D6). Les écrivains vivent dans `windows_source.rs` — `PRODUCED` **313**, `TICKS` **518**, `CAPTURED` **544** (ordre non positionnel : ne pas apparier à la liste `TICKS`/`CAPTURED`/`PRODUCED`) —, donc dans le **capteur** depuis D4 ; le lecteur unique est `demarrage.rs:127-129` (déplacé depuis `142-144` par les
remaniements de D7), donc dans
l'**enfant** ; et `main.rs:268` rend la main à `capteur::executer` avant que `demarrage::executer` ne soit atteint. La variable n'affiche donc que des **zéros** côté enfant, et les compteurs du capteur ne sont lus par **personne**. **Elle ne décrit plus que le chemin mono-fenêtre**, sans `CAPTEUR` ni `SUPERVISEUR`. ✅ **Les cinq numéros de ligne de cette case ont été REVÉRIFIÉS le 5 août 2026 (D8, tâche 12) et sont tous EXACTS** — `313`, `518`, `544`, `demarrage.rs:127-129`, `main.rs:268` : la mention « valeur non revérifiée pour le reste » est donc levée. ✅ **LE REMÈDE A ÉTÉ APPLIQUÉ le 6 août 2026 (D9, tâche 11), et TOUT CE QUI PRÉCÈDE EST DEVENU DE L'HISTOIRE — y compris les cinq numéros de ligne, qui ne désignent plus rien.** Les trois statiques ont disparu : elles sont un champ `telemetrie` **par session** de `WindowsSource`, dans **`agent/src/windows_source/telemetrie.rs`** (**72** lignes, **pur, aucun `cfg`**, deux tests d'hôte). Le lecteur mort de `demarrage.rs` a été retiré ; **le lecteur est désormais `agent/src/capteur/fenetre/trace.rs`** (**43**), qui vit dans le CAPTEUR, là où les compteurs sont écrits, et qui trace **sous le span `fenetre{session=…}` posé par D7** — donc attribuable sans champ supplémentaire. **La convention de la variable est INCHANGÉE : la simple PRÉSENCE active** (à l'inverse de `PLEIN_ECRAN`/`AUDIO`/`SUPERVISEUR`/`CAPTEUR`, où `=0` désarme). Trace : `compteurs de capture ticks=… capturees=… produites=…`. Toujours transmise par `scripts/run-agent.sh`. ⚠️ **Ce qui reste dans `demarrage.rs`, côté enfant, est la partie ENCODEUR** (tentatives et accumulation de capture, entrées/sorties du convertisseur et de l'encodeur) : elle, n'a pas bougé et reste lue là. **Effet de bord recherché et obtenu** : `windows_source.rs` retombe de 638 à **628** |
| `MULTIFENETRE_MODE_SORTIE=<L>x<H>` | **Sous-bloc D8** — la sonde **P1** (`agent/src/diagnostics/multifenetre.rs:169`) : crée une sortie virtuelle, relit sa taille **courante** par DXGI, choisit une cible parmi les modes annoncés **en excluant cette taille courante**, tente `ChangeDisplaySettingsExW`, et **juge sur le MOUVEMENT relu par DXGI, jamais sur une égalité** — la première version rendait `P1 RECU` sans que rien n'ait bougé, son critère ne pouvant pas échouer. Rend `P1 NON MESURABLE` si aucun mode ne diffère de la taille courante. Transmise par `scripts/run-agent.sh` *(les numéros de ligne publiés ici — `:77` puis `:78` — ont dérivé DEUX fois : `PLEIN_ECRAN_MODE_SORTIE` en avait ajouté un, D9 l'a retiré et a ajouté `PART_SONDAGE`. **Ne plus recopier de numéro de ligne pour ce script : `grep -n` avant de s'y fier.**)*. ⚠️ **ÉTENDUE PAR LA PHASE P DE D9** : elle porte désormais la duplication DXGI ouverte et tenue pendant la tentative (l'écart banc/produit que D8 laissait béant), un **témoin sans duplication** dans la même exécution, une **quatrième combinaison de drapeaux** (`flags = 0`, sélectionnable par `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`), le compte des **pertes d'accès infligées à deux voisines**, et une **épreuve de PERSISTANCE** (une sortie de plus est créée après le changement, et la sortie sous test est relue). Voir la section D9. ⚠️ **C'est AUSSI le remède opérationnel au blocage produit par pollution du registre** : une sortie naît à la dernière taille laissée au registre, et un registre resté à 2560×1440 empêche toute fenêtre de s'attacher — `MULTIFENETRE_MODE_SORTIE=1280x720` le rétablit. ⚠️ **Un lancement à elle seule** : l'aiguillage retourne après la première sonde reconnue, un `MULTIFENETRE_VDD_PURGE=1` dans le même lancement l'annulerait en silence |
| `PLEIN_ECRAN=0` | **Sous-bloc D8** — **variable de PRODUIT**, pas de banc. **Désarme le mécanisme ENTIER** : ni relecture du style (`capteur/fenetre.rs`), ni changement de mode de sortie (`windows_source/redimensionnement.rs`). ⚠️ **`=0` désactive ; une simple PRÉSENCE n'active pas** — même convention qu'`AUDIO`, `SUPERVISEUR` et `CAPTEUR`, et pour la même raison : tester `is_ok()` activerait le plein écran en écrivant `PLEIN_ECRAN=0` pour le couper. Lue dans le **capteur** seul (`capteur/plein_ecran.rs:78`), par `OnceLock` — l'enfant ne fait que relayer. Transmise par `scripts/run-agent.sh:34`. Traces de contrôle : `plein ecran DESARME (PLEIN_ECRAN=0) : …` au démarrage du capteur, et `redimensionnement ignoré : PLEIN_ECRAN=0 …` à chaque `resize`. ⚠️ ~~**C'est la SEULE parade actuelle au défaut HiDPI ouvert**~~ — **plus vrai depuis la revue finale de branche de D8** : le changement de mode étant désormais désarmé PAR DÉFAUT (ligne suivante), le défaut HiDPI est inatteignable en configuration livrée, et `PLEIN_ECRAN=0` est devenu la parade du mécanisme **entier**, plus la seule parade d'un défaut |
| ❌ ~~`PLEIN_ECRAN_MODE_SORTIE=1`~~ **CETTE VARIABLE N'EXISTE PLUS** (retirée le 6 août 2026, D9 tâche 3 — ni dans le code, ni dans `scripts/run-agent.sh` ; les legs 12 et 13 **cessent d'exister** au lieu d'être différés. Voir la section D9). Tout ce qui suit est le relevé de D8, conservé pour son diagnostic. | ~~**Sous-bloc D8, revue finale de branche (5 août 2026)** — **variable de PRODUIT**. **ARME** le changement de mode de la sortie virtuelle (`windows_source/redimensionnement/mode_sortie.rs`), **DÉSARMÉ PAR DÉFAUT**. ⚠️ **Convention INVERSE de `PLEIN_ECRAN`, à dessein** : on désarme sur `=0` ce qui est livré, on **arme sur `=1`** ce qui ne l'est pas — et ici la simple présence ne suffit pas non plus, il faut la valeur `1`. **Ce qui reste actif sans elle** : détection du style, annonce `PleinEcran`/`Fullscreen`, armement client. **Pourquoi** : critère ② **JAMAIS EXERCÉ** (`mode_sortie_demande=0` aux deux exécutions) et **deux Critiques ouvertes** — C1, la pollution du registre qui bloque les ouvertures de fenêtre ultérieures (portée inconnue : **cinq GUID SudoVDA distincts** au journal de recette) ; C2, la reprise D2 court-circuitée par une `Err` sur un échec **transitoire** de réouverture. **Les deux sont délibérément NON CORRIGÉES**, le désarmement les rendant inatteignables. Garde et raisons : `agent/src/capteur/plein_ecran.rs::changement_de_mode_arme`. Transmise par `scripts/run-agent.sh:35`. Traces : `changement de mode de sortie ARME (…)` au premier appel si armée, `redimensionnement ignoré : changement de mode de sortie DÉSARMÉ (…)` à chaque `resize` sinon.~~ **Ces deux traces n'existent plus** ; `resize` en mode `SortieEntiere` journalise désormais `redimensionnement ignoré : la source capture une sortie DXGI entière (voir le constat de mesure de capteur::plein_ecran, sous-bloc D9)` |
| `PART_SONDAGE=0` | **Sous-bloc D9, tâche 12** — **variable de BANC, jamais une configuration livrée**. Neutralise l'appel `set_desired_bitrate` de `agent/src/transport/part.rs` : c'est le bras « désarmé » de l'A/B différentiel que D6 laissait dû (son leg n°4). ⚠️ **Convention de `PLEIN_ECRAN` — `=0` DÉSARME, une simple présence n'active pas** ; l'appel est armé par défaut. Lue dans l'**enfant**. Transmise par `scripts/run-agent.sh`. Trace, émise **seulement si désarmé** : `objectif de sondage DESARME (PART_SONDAGE=0) : bras A/B, jamais une configuration livrée` (`warn!`). ⚠️ **L'A/B qu'elle sert a été joué et N'ÉTABLIT RIEN** : 2 exécutions par bras, écart de trafic cumulé +23,2 % dans le sens attendu, mais **variance intra-bras +83,1 %** — plus grande que l'écart mesuré. Voir la section D9 |
| `SORTIE_DESIGNEE=0` | **Lot 32** (30 août 2026) — **variable de BANC, jamais une configuration livrée**, même statut que `PART_SONDAGE`, `PONT_ECRITURE`, `PONT_CACHE` et `PRESSE_PAPIER_GARDE`. Désarme le chemin ① d'appariement — **DÉSIGNER la sortie qu'on vient de créer par le couple `(adaptateur, identifiant de cible)` que le pilote a rendu**, changé en nom GDI par l'API CCD (`moniteurs_virtuels/config_affichage.rs`). Désarmée, elle rend **EXACTEMENT le produit d'avant le lot 32** : l'appariement par différence d'ensembles, qui refuse toute fenêtre quand la première sortie virtuelle **REMPLACE une cible forcée** sur la même source (défaut mesuré par le lot 30, sans le VGA QEMU). ⚠️ **`=0` DÉSARME ; une simple PRÉSENCE n'active pas** — convention de `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`, `PRESSE_PAPIER`, `APPS`, `PONT_ECRITURE` et `PONT_MUTATION`, et pour la même raison : tester `is_ok()` armerait le mécanisme chez qui écrit `SORTIE_DESIGNEE=0` pour le couper. **Le prédicat est RÉUTILISÉ, pas recopié** : `crate::apps::desarme`, précédent d'`ICONES` (G2) et d'`APPS_SURVEILLANCE` (G4). Lue dans le **superviseur** (`superviseur/designation.rs::armee`), par `OnceLock`, **forcée au DÉMARRAGE de `boucle::tourner` et non au premier appariement** — sinon la trace ne sortirait qu'à la première fenêtre, donc APRÈS les premiers gestes d'une recette courte (leçon de `PONT_MESURE`, F4). Transmise par `scripts/run-agent.sh`, **par une tâche DÉDIÉE qui ne fait que cette ligne** — le piège payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`) et D7 (`AUDIO`). Trace, **émise seulement si désarmé** : `designation de sortie DESARMEE (SORTIE_DESIGNEE=0) : bras de banc, jamais une configuration livree` (`warn!`). ✅ **La ligne a été LUE DANS LE `run-agent.ps1` GÉNÉRÉ**, jamais dans le script hôte (30 août 2026) : bras désarmé `$env:SORTIE_DESIGNEE = '0'`, bras armé **0 ligne** (`${SORTIE_DESIGNEE:+…}` ne rend rien quand la variable n'est pas posée — le chemin ① est armé PAR DÉFAUT), témoin négatif `SORTIE_DESIGNEE_INEXISTANTE` **0**, témoin positif `PLEIN_ECRAN=0` **1**. 🔴 **LA TRACE PROUVE QUE LA VARIABLE A ATTEINT LE PROCESSUS, JAMAIS QUE LE MÉCANISME EST COUPÉ** (leçon de P1 sur `PRESSE_PAPIER=0`) : **ce qui discrimine est le champ `designee` VIDE du refus `aucune sortie candidate ne peut servir ce viewport`, et la fenêtre non servie**. ✅ **LES TROIS BRAS ONT ÉTÉ JOUÉS SUR LA VM le 30 août 2026**, VGA retiré sur autorisation du propriétaire : bras désarmé **8 sorties créées, 8 refus, 0 fenêtre tenue** ; bras armé **1 créée, 0 refus, 1 tenue**, `chemin ① = 1` et `chemin ② = 0`, `nom_designe="\\.\DISPLAY5"` — le nom que portait la cible forcée (`statusFlags=0x11`, relevé en session 1). 🔴 **LA PREMIÈRE TENTATIVE DU BRAS ROUGE ÉTAIT VACUEUSE, ET SEULE LA TRACE L'A DIT** : la ligne était bien dans `C:\nivuus\agent\run-agent.ps1` mais **APRÈS** l'invocation de l'agent, donc jamais exécutée — `TRACE DE DESARMEMENT : 0`. Le piège de D1/D2/D7, revécu une quatrième fois. |
| `AUDIO_FAUTE_LECTURE=<n>` | **Sous-bloc D10, tâche 3** — **variable de BANC, jamais une configuration livrée**. Fait échouer les *n* prochaines **lectures** WASAPI (`agent/src/windows_audio/fil.rs`) ; au-delà de `LECTURES_ECHOUEES_MAX = 10` la capture se déclare morte, ce qui déclenche la reconstruction. **ABSENTE = DÉSARMÉE.** Budget **global au processus** depuis D10 — ⚠️ il était **relu par fil** au premier jet, et chaque capture reconstruite recevait alors un budget neuf : **le chiffre-juge ne pouvait pas quitter zéro**, sur un produit pourtant corrigé. Trace : `injection de fautes de lecture audio ARMEE (banc)`. Transmise par `scripts/run-agent.sh:42`. 🔵 **Le compte de fautes CONSOMMÉES est un témoin d'armement INDÉPENDANT du spectre** : le garde `if !emettait` précède l'injection, donc **une source muette ne peut pas consommer de faute**. ⚠️ **Cette ligne manquait à ce tableau depuis D10** ; ajoutée par la revue transverse de D11 |
| `AUDIO_FAUTE_RECONSTRUCTION=<n>` | **Sous-bloc D11, tâche 4** — **variable de BANC, jamais une configuration livrée**. Fait échouer les *n* prochaines **reconstructions** de capture audio (`agent/src/transport/piste_audio/injection.rs`), et c'est ainsi que le critère ④ de D10 — « une capture irrécupérable retombe sur la promotion d'une voisine », que D10 déclarait *non démontrable par le protocole prescrit* — devient atteignable. ⚠️ **Convention INVERSE de `PLEIN_ECRAN` : ABSENTE = DÉSARMÉE**, présente et non nulle = armée (jamais `is_ok()`). ⚠️ **Budget GLOBAL AU PROCESSUS** (`OnceLock` + `AtomicU32`, `fetch_update`), **jamais par appel** — c'est la leçon que D10 a payée sur `AUDIO_FAUTE_LECTURE` : un budget relu par fil se réarme à chaque reconstruction, et le chiffre-juge qu'il sert devient **structurellement incapable de quitter zéro**. Lue dans l'**enfant**. Transmise par `scripts/run-agent.sh:44`. Trace, **seulement si armée** : `injection de fautes de RECONSTRUCTION audio ARMEE : banc, jamais une configuration livrée` (`warn!`). La faute emprunte le `warn!` du leg 6, d'où `erreur="faute injectée (AUDIO_FAUTE_RECONSTRUCTION)"` au journal. ⚠️ **Le compte à poser n'est PAS `> RECONSTRUCTIONS_MAX`** : le réarmement de D9 réapprovisionne le budget, et le compte juste est **`(REARMEMENTS_MAX + 1) × RECONSTRUCTIONS_MAX` = 18** — mesuré tel quel |
| `AUDIO_FAUTE_LECTURE_MS=<ms>` | **Sous-bloc D11, tâche 5** — **variable de BANC**. Borne **dans le temps** l'armement de `AUDIO_FAUTE_LECTURE` (prédicat pur `crate::audio::injection_encore_armee`, testé sur l'hôte ; lue dans `windows_audio/fil.rs`). **ABSENTE = ILLIMITÉ**, donc le comportement de D10 est strictement préservé et ses recettes restent reproductibles. Sans elle, la voisine qu'on veut voir promue meurt **à l'instant même de sa promotion** et le critère reste non démontrable. Transmise par `scripts/run-agent.sh:43`. La trace est celle de `AUDIO_FAUTE_LECTURE`, **enrichie du champ `fenetre_ms`** — un seul `warn!`, à dessein : deux traces au même instant se compteraient comme deux événements (piège maison de D6). ⚠️ **L'ORIGINE DU BUDGET EST LE DÉMARRAGE DU FIL, PAS L'ÉLECTION** : `3000` rend le critère **inatteignable** (le premier arbitrage du capteur arrive ~2,7 s après le démarrage du fil, et la fenêtre se referme 6 ms avant l'élection de la porteuse), **`5000` est la valeur dérivée de la mesure**. ⚠️ **Non calibrée** : c'est une valeur de banc, pas une constante de produit |
| `AUDIO_PERIPHERIQUE=<nom ou identifiant>` | **Correction « A-bis », 19 août 2026** — **variable de PRODUIT**, pas de banc. Désigne le point de terminaison de **rendu** que le loopback de session doit capter, au lieu de subir le rendu **par défaut** de Windows. ⚠️ **Convention VALUÉE** — celle de `MULTIFENETRE_SORTIE` et `BUDGET_BPS`, **pas** celle de `PLEIN_ECRAN` : **absente ou vide, le comportement est EXACTEMENT celui d'avant** (le défaut de Windows). Trois critères, dans cet ordre : **identifiant d'endpoint** exact (`IMMDevice::GetId`, forme `{0.0.0.00000000}.{guid}` — stable, opaque), **nom convivial** exact (`PKEY_Device_FriendlyName`), puis **sous-chaîne insensible à la casse**. 🔵 **Une sous-chaîne AMBIGUË refuse de trancher** (`Choix::Ambigu`) au lieu de prendre le premier : prendre le premier serait retomber sur un **rang d'énumération** par la porte de derrière — la leçon des index DXGI de D1, payée une fois, appliquée ici d'avance. Règle **PURE** dans `agent/src/wasapi/peripherique.rs` (aucun `cfg`, éprouvée sur l'hôte), moitié COM dans `agent/src/wasapi/rendu.rs`. Lue par `LoopbackCapture::open`, donc **le mode MONO-FENÊTRE et la sonde `AUDIO_PROBE` seulement** — `pour_processus` (multi-fenêtres, D7+) **ne résout aucun endpoint** et n'est pas concerné. Transmise par `scripts/run-agent.sh`. **Le périphérique réellement retenu est JOURNALISÉ à chaque ouverture**, et tout repli l'est aussi : jamais silencieux |
| `MICRO_MESURE=1` | **Chantier E, bloc E1** — **variable de BANC, jamais une configuration livrée**. Arme le **puits de mesure du micro** (`agent/src/demarrage/micro.rs`) : un consommateur qui joue le rôle du futur fil WASAPI d'E2, retire du tampon à la cadence réelle et journalise ce qu'il obtient. ⚠️ **Convention `=1` qui ARME** — et non `=0` qui désarmerait : le puits n'est **pas** livré, donc c'est sa présence qu'il faut déclarer, pas son absence. Trace de contrôle, dont **l'absence prouve que la variable n'a pas atteint le processus** : `micro de mesure ARME (MICRO_MESURE=1) : instrument de banc, jamais une configuration livree`. Trace périodique : `micro mesuré`, portant `crete` et `frequence_hz` **côte à côte** (une crête sans fréquence est du bruit), `plc` et `plc_plafonnees` **côte à côte** (le second est **disjoint** du premier — c'est ce qui rend le plafond de dissimulation observable), plus `deposees`, `famines`, `occupation_ms` et `occupation_max_ms`. Transmise par `scripts/run-agent.sh` |
| `PONT_MESURE=1` | **Sous-projet ③ Pont fichiers, sous-bloc F4** (21 août 2026) — **variable de BANC, jamais une configuration livrée**. Arme l'**ÉMISSION** de la ligne de recensement des traversées du pont (`agent/src/pont/latence.rs`). 🔴 **CONVENTION INVERSE DES TROIS AUTRES `PONT_*`, ET C'EST DÉLIBÉRÉ : `=1` ARME, l'ABSENCE désarme** — c'est celle de `MICRO_MESURE`, parce que la règle du dépôt est *on désarme sur `=0` ce qui est LIVRÉ, on arme sur `=1` ce qui ne l'est pas*. Le pont, l'écriture et les mutations sont livrés ; la ligne de latence est un instrument. 🔴 **CE QU'ELLE ARME EST L'ÉMISSION, PAS LA COLLECTE** : l'histogramme est alimenté TOUJOURS, parce qu'*un mécanisme qui n'est armé que pendant sa propre mesure est un mécanisme que le produit n'exerce jamais, donc qu'on ne verra jamais rouge*. Lue dans `agent/src/pont/service/recensement.rs`, par `OnceLock`, **forcée au démarrage du fil et non au premier recensement** — sinon la trace ne sortirait qu'après `PERIODE_RECENSEMENT` (10 s), donc APRÈS les premiers gestes d'une recette courte. Transmise par `scripts/run-agent.sh`, **par une tâche dédiée**. Trace, **émise seulement si armée** : `banc de latence du pont ARME (PONT_MESURE=1) : instrument de banc, jamais une configuration livree` (`warn!`). ⚠️ **Les compteurs sont CUMULATIFS** : une mesure se lit par DIFFÉRENCE entre deux recensements, jamais sur une ligne isolée. 🔴 **LE CONTRÔLE QUI VAUT N'EST PAS QUE LA LIGNE SORTE, c'est qu'elle COMPTE ce qu'elle dit** — et le bras « armé, aucun geste », qui rend `n:0` partout, est ce qui rend le bras « armé, un geste » discriminant |
| `PRESSE_PAPIER=0` | **Sous-projet ① Divers — presse-papier, sous-bloc P1** (20 août 2026) — **variable de PRODUIT**, pas de banc. Désarme le mécanisme ENTIER : `Sondeur::tour` teste le garde **AVANT toute lecture**, donc `PRESSE_PAPIER=0` empêche jusqu'à la lecture du compteur de séquence, pas seulement l'envoi. ⚠️ **`=0` DÉSACTIVE ; une simple PRÉSENCE n'active pas** — convention d'`AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR`, `CAPTEUR` et `PART_SONDAGE`, et pour la même raison : tester `is_ok()` armerait le mécanisme en écrivant `PRESSE_PAPIER=0` pour le couper. Lue dans le **capteur** (le propriétaire, D1), par `OnceLock`. Transmise par `scripts/run-agent.sh:37`. Trace, **émise seulement si désarmé** : `presse-papier DESARME (PRESSE_PAPIER=0) : le contenu copie dans la VM n'est plus pousse au navigateur` (`warn!`). 🔴 **LE CONTRÔLE QUI VAUT EST LE ZÉRO DE MESSAGES, PAS LA TRACE** : la trace prouve que la variable a atteint le processus, elle ne prouve pas que le mécanisme est coupé — et un zéro seul serait rendu par un produit entièrement en panne. C'est le bras SANS la variable, avec ses 4 messages, qui rend le zéro discriminant (2 exécutions par bras, recette P1) |
| `PRESSE_PAPIER_GARDE=0` | **Sous-projet ① Divers — presse-papier, sous-bloc P2** (21 août 2026) — **variable de BANC, jamais une configuration livrée**, même statut que `PART_SONDAGE`. Neutralise `Sondeur::apres_notre_ecriture` **EN ENTIER**. ⚠️ **`=0` DÉSARME ; une simple présence n'arme pas** — convention de `PRESSE_PAPIER` deux lignes plus haut, et **inverse de `PRESSE_PAPIER_SONDE`** juste en dessous : les trois sont écrites côte à côte pour qu'on ne les confonde pas. Lue par `OnceLock` dans le **propriétaire** (le capteur), via `crate::presse_papier::gardes_armes`. Transmise par `scripts/run-agent.sh`. Trace, **émise seulement si désarmé** : `garde anti-echo du presse-papier DESARME (PRESSE_PAPIER_GARDE=0) : bras de banc, jamais une configuration livree` (`warn!`). 🔴 **ELLE DÉSARME LES DEUX GARDES DE D5, PAS LE SEUL N°1, ET C'EST LE POINT.** La spec prescrivait de désarmer le n°1 et d'attendre un compte « qui croît sans borne » ; **il reste à UN**, et la spec avait prévu ce cas. Sans armement du n°2, le `Sondeur` relit notre texte, l'annonce **une** fois, puis pose lui-même `dernier_emis` et `reference` — au tour suivant `observer` sort dès sa première ligne. Et **rien ne relance** : le client n'émet vers l'agent que sur un `paste`, donc sur un GESTE HUMAIN. Désarmer le seul n°1 rendrait donc **zéro message aussi**, et la rouge du critère ④ serait vacueuse une seconde fois. 🔵 **Conséquence de conception, qui contredit une phrase de D5** : dans l'architecture livrée, **aucune oscillation auto-entretenue n'est possible** — ce que les gardes suppriment est **un aller-retour PAR COLLAGE**, pas une divergence. 🔵 **MESURÉE des deux côtés (recette P2, 2 exécutions par bras)** : bras armé **0, 0, 0, 0** messages sur 4 collages ; bras désarmé **1, 2, 3, 4**, chacun portant exactement le texte qu'on venait de coller. ⚠️ **La trace ne prouve que l'arrivée de la variable au processus** (elle vaut 1 dans les deux journaux désarmés et 0 dans les deux armés) ; c'est le compte de messages qui prouve l'effet. ⚠️ **DEPUIS LE SOUS-BLOC P3 (21 août 2026), ELLE DÉSARME AUSSI UNE TROISIÈME PRISE** : `Sondeur::ecarter_notre_ecriture`, la seconde prise de D-P3-6, qui écarte l'annonce que NOTRE PROPRE seconde écriture vient de produire. Il le faut : une prise qui mordrait quand même viderait ce bras de banc de son sens, la rouge du critère ④ comptant des messages revenant vers la fenêtre après un collage. Un test d'hôte le tient |
| `PRESSE_PAPIER_SONDE=<secondes>` | **Sous-projet ① Divers — presse-papier, sous-bloc P1, sonde P0** — **variable de BANC, jamais une configuration livrée**. ⚠️ **Convention INVERSE de la ligne ci-dessus, et les deux sont écrites côte à côte pour qu'on ne les confonde pas : ABSENTE = DÉSARMÉE**, présente = armée (la valeur est une durée, pas un interrupteur). Mesure les cinq questions de la porte éliminatoire sur `GetClipboardSequenceNumber` (`agent/src/diagnostics/presse_papier.rs`). Transmise par `scripts/run-agent.sh:89`. ⚠️ **Elle ÉCRIT le presse-papier de la VM** en phases C et D, et le détruit donc ; le produit, lui, ne l'écrit jamais en P1. 🔴 **Ne jamais la poser en même temps que `SUPERVISEUR`** : `main()` appelle `diagnostics::aiguiller()` en `main.rs:172`, AVANT la branche `CAPTEUR` (`:180`) et avant `PONT` (`:279`) — **quel que soit le mode demandé**, un agent qui la porte exécute la sonde et s'arrête. ⚠️ **La menace que la divergence E10 du plan lui prêtait est FAUSSE** : elle annonçait un capteur exécutant la sonde pendant qu'un superviseur vivant le relance en boucle, ce qui supposerait que l'enfant porte la variable et pas son père — or `Command` hérite de l'environnement, donc le père se serait arrêté le premier. La consigne ne change pas, sa raison si |
| `INSTALLATION_FAUTE=empreinte` | **Sous-projet ④ Gestion d'apps, sous-bloc G3** — **variable de BANC, jamais une configuration livrée**. L'agent altère **un octet** de l'installeur téléchargé, ce qui fait échouer la vérification d'empreinte **côté agent** — le TROISIÈME des trois étages, les deux autres étant le dépôt d'une tranche et le scellement côté plateforme. ⚠️ **Convention `absente = DÉSARMÉE`**, celle d'`AUDIO_FAUTE_LECTURE` et d'`AUDIO_FAUTE_RECONSTRUCTION` — **jamais** celle de `PLEIN_ECRAN`, où `=0` désarme. Lue dans l'**enfant** qui installe. Trace, **émise seulement si armée** : `faute d'installation ARMEE (INSTALLATION_FAUTE=…) : banc, jamais une configuration livrée` (`warn!`). Transmise par `scripts/run-agent.sh`, **par une tâche DÉDIÉE qui ne fait que cette ligne** — le piège payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`) et D7 (`AUDIO`). Détail et mesures : section « Sous-bloc G3 » en pied de fichier |
| `APPS_SURVEILLANCE=<0\|sans-rebond\|seule>` | **Sous-projet ④ Gestion d'apps, sous-bloc G4** — **UNE SEULE VARIABLE, QUATRE ÉTATS, et aucun n'est une présence.** **Absente** : le produit livré — surveillance armée, anti-rebond armé, réconciliation périodique armée. `0` : **surveillance désarmée**, c'est-à-dire le comportement de G1 exactement, et c'est la ROUGE du critère ①. `sans-rebond` : anti-rebond neutralisé, ROUGE de ④. `seule` : **réconciliation périodique DÉSARMÉE**, ROUGE de ③ — **variable de BANC pour ces trois états**, jamais une configuration livrée. ⚠️ **`=0` désarme ; une simple PRÉSENCE n'active pas**, et **chaque état est une ÉGALITÉ EXACTE** — convention de `PLEIN_ECRAN`, `AUDIO`, `APPS`, `ICONES`, `PART_SONDAGE`, `PRESSE_PAPIER`. `apps::desarme` est **RÉUTILISÉ, pas recopié** (précédent de G2 pour `ICONES`), ce qui préserve gratuitement le cas `"0 "`. 🔴 **UNE VALEUR INCONNUE EST NOMMÉE PAR UN `warn!` ET LE COMPORTEMENT LIVRÉ EST RETENU** : sans cela une coquille (`seul` pour `seule`) ferait tourner le VERT sous le nom du ROUGE, et la recette lirait un verdict faux. **UNE SEULE VARIABLE ET NON TROIS BOOLÉENS**, pour deux raisons — le piège de `run-agent.sh` payé cinq fois, et le fait que trois booléens autoriseraient « ni surveillance ni période », c'est-à-dire un agent qui ne réconcilie **jamais**. Lue dans `agent/src/apps.rs`, `Mode` **PUR** dans `apps/surveillance/mode.rs`. Trace **INCONDITIONNELLE** : `mode de surveillance retenu mode=…`. 🔴 **ELLE PROUVE QUE LA VARIABLE A ATTEINT LE PROCESSUS, JAMAIS QUE LE MÉCANISME EST COUPÉ** (leçon de P1 sur `PRESSE_PAPIER=0`) : **ce qui discrimine est l'ABSENCE de toute ligne `racine surveillée`**, mesurée. Transmise par `scripts/run-agent.sh`, **par une tâche DÉDIÉE** |
| `APPS_FAUTE=<debordement\|muette\|perte>:<n>` | **Sous-bloc G4** — **variable de BANC, jamais une configuration livrée**. `debordement:<n>` : les *n* prochaines complétions sont traitées comme des **débordements** — comptées, journalisées **et déclenchantes**. 🔵 `muette:<n>` : elles sont **AVALÉES** — ni comptées, ni journalisées, ni déclenchantes. **C'EST LE SEUL MONTAGE QUI RENDE LE CRITÈRE ③ DISCRIMINANT**, parce que dans cette conception un débordement est *lui-même* une complétion, donc un déclencheur, et se répare tout seul : la seule panne que la réconciliation périodique achète réellement est **une surveillance qui cesse de délivrer SANS ERREUR**. `perte:<n>` : erreur fatale de handle, pour observer le rétablissement. ⚠️ **Convention `absente = DÉSARMÉE`**, celle d'`AUDIO_FAUTE_LECTURE`, d'`AUDIO_FAUTE_RECONSTRUCTION` et d'`INSTALLATION_FAUTE` — **jamais** celle de `PLEIN_ECRAN`, et les deux lignes sont écrites côte à côte ici pour qu'on ne les confonde pas. ⚠️ **`debordement:0` vaut l'absence** : un budget nul est une injection qui ne tirera jamais, et la déclarer « ARMÉE » ferait lire un armement à qui n'en a aucun. 🔴 **BUDGET GLOBAL AU PROCESSUS** (`OnceLock` + `AtomicU8`/`AtomicU32`, `fetch_update`), jamais par fil : la surveillance **se rouvre** après une perte, et un budget relu à la réouverture se réarmerait — c'est la panne de mesure que D10 a payée sur `AUDIO_FAUTE_LECTURE`, où le chiffre-juge était structurellement incapable de quitter zéro. Trace, **seulement si armée** : `faute de surveillance ARMEE (APPS_FAUTE) : banc, jamais une configuration livrée`. 🔴 **CE QU'ELLE ÉTABLIT ET CE QU'ELLE N'ÉTABLIT PAS** : que le **REMÈDE** fonctionne, **jamais qu'une CAUSE existe** — et sur cette VM, aucune cause naturelle de débordement n'existe (voir G4). Transmise par `scripts/run-agent.sh`, **par une tâche DÉDIÉE** |
| `PONT_ECRITURE=0` | **Sous-projet ③ Pont fichiers, sous-bloc F2** (21 août 2026) — **variable de BANC, jamais une configuration livrée**. Désarme la **POUSSÉE** d'écriture : le pont continue de détecter, de journaliser et de compter les écritures dues, **et n'en pousse aucune**. ⚠️ **`=0` DÉSARME ; une simple PRÉSENCE n'active pas** — convention d'`AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`, `PRESSE_PAPIER`, `APPS` et `PONT`. 🔴 **Le PLAN de F2 se contredisait en une phrase à son sujet** : il écrivait « `=0` désarme » ET prescrivait `matches!(…, Ok(v) if v != "0")`, **qui rend `false` en l'ABSENCE de la variable** — pris à la lettre, il aurait livré un pont **MUET PAR DÉFAUT**, sans un `ERROR`. Lue dans le **pont** (`agent/src/pont.rs`). Transmise par `scripts/run-agent.sh`, **par une tâche dédiée**. Trace, **émise seulement si désarmé** : `poussee d'ecriture DESARMEE (PONT_ECRITURE=0) : bras de banc, jamais une configuration livree` (`warn!`). 🔴 **LE CONTRÔLE QUI VAUT EST LE ZÉRO DE POUSSÉES, PAS LA TRACE** : un zéro seul serait rendu par un produit entièrement en panne, et c'est le bras SANS la variable, avec ses six acquittements, qui le rend discriminant |
| `PONT_MUTATION=0` | **Sous-projet ③ Pont fichiers, sous-bloc F3** (21 août 2026) — **variable de PRODUIT**, à la différence de `PONT_ECRITURE` juste au-dessus. Désarme le renommage ET la suppression : `notifications::decider` les refuse **au PRE_**, donc **rien n'est poussé** au poste local et le geste ÉCHOUE côté VM. ⚠️ **`=0` DÉSARME ; une simple PRÉSENCE n'active pas** — convention d'`AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`, `PRESSE_PAPIER`, `APPS`, `PONT` et `PONT_ECRITURE`. Lue dans le **pont** (`agent/src/pont.rs`), par `OnceLock`. Transmise par `scripts/run-agent.sh`, **par une tâche dédiée qui ne fait que cela**. Trace, **émise seulement si désarmé** : `mutations DESARMEES (PONT_MUTATION=0) : renommage et suppression refuses au PRE_, rien n'est pousse` (`warn!`). 🔴 **LE CONTRÔLE QUI VAUT EST QUE LA SOURCE RESTE PRÉSENTE, PAS LA TRACE** : la rouge relève `protege-en-ecriture=9`, les trois gestes en ÉCHEC, la source **PRÉSENTE** et la cible **ABSENTE** — mécanisme présent, résultat absent. ⚠️ **NE PAS EMPLOYER `PONT_ECRITURE=0` À SA PLACE** : ce drapeau pose `inscriptible=false`, ce qui fait refuser `PRE_RENAME`/`PRE_DELETE` **pour une autre raison**, et une rouge de F3 y a été DISQUALIFIÉE |
| `PONT_CACHE=0` | **Sous-projet ③ Pont fichiers, sous-bloc F5** (21 août 2026) — **variable de BANC, jamais une configuration livrée**. Désarme le **cache d'énumération** (`agent/src/pont/cache.rs`, `TTL_ENUMERATION = 30 s`) : chaque listage repaie son aller-retour, c'est-à-dire **exactement le produit d'avant F5**. ⚠️ **`=0` DÉSARME ; une simple PRÉSENCE n'active pas** — convention d'`AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`, `PRESSE_PAPIER`, `APPS`, `PONT`, `PONT_ECRITURE` et `PONT_MUTATION`. Lue dans le **pont** (`agent/src/pont.rs`), par `OnceLock`. Transmise par `scripts/run-agent.sh`, **par une tâche dédiée**. Trace, **émise seulement si désarmé** : `cache d'enumeration DESARME (PONT_CACHE=0) : bras de banc, jamais une configuration livree` (`warn!`). 🔴 **C'EST LE BRAS ROUGE DU CRITÈRE ① SUR LE PRODUIT LUI-MÊME** — désarmé, un fichier ajouté côté poste local apparaît **sans** `Rafraichir` (2 exécutions), là où le bras armé ne le montre qu'après (3 exécutions) : un rouge du MÉCANISME, présent et sans effet, jamais vacueux. 🔵 **Il sert aussi d'A/B d'ATTRIBUTION** : c'est lui qui a établi que la disparition d'un répertoire frère après un renommage est **préexistante** et que le cache ne fait que la **prolonger** |
| `MICRO=0` | **Chantier E, bloc E2** — **variable de PRODUIT**. Désarme le microphone **entier** : aucun puits n'est posé, `micro_disponible()` reste faux, `ready` porte `mic: false`, et le bouton du navigateur ne paraît pas. ⚠️ **`=0` DÉSARME ; une simple présence n'arme pas** — convention d'`AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR`, `CAPTEUR`, `PRESSE_PAPIER` et `PART_SONDAGE`, et pour la même raison : tester `is_ok()` allumerait le micro chez qui écrit `MICRO=0` pour le couper. Un test garde le prédicat (`demarrage/micro.rs::arme_micro`). Trace, **émise au branchement** donc avant toute session : `micro DESARME (MICRO=0)`. **Mesurée** (recette E2) : bouton caché sur une session `ice=connected`, **0** ligne `windows_micro`, juge à `AMPLITUDE=0,000000` |
| `MICRO_PERIPHERIQUE=<nom ou identifiant>` | **Chantier E, bloc E2** — **variable de PRODUIT**. Désigne le point de terminaison de **rendu** sur lequel le micro écrit. Convention **VALUÉE**, celle d'`AUDIO_PERIPHERIQUE` et de `MULTIFENETRE_SORTIE`. 🔴 **DEUX DIFFÉRENCES DÉLIBÉRÉES AVEC `AUDIO_PERIPHERIQUE`** : ① **absente, elle ne vaut PAS le défaut de Windows mais la désignation INTÉGRÉE `"VB-Audio"`** — retomber sur `GetDefaultAudioEndpoint` ferait sortir la voix de l'utilisateur **par les haut-parleurs de la VM** sur une machine où le défaut est la carte son ; ② **il n'y a AUCUN repli** — `Choix::Introuvable` et `Choix::Ambigu` valent **échec**, pas de fil de rendu, `mic: false`, un `warn!` avec l'inventaire. A-bis se replie parce que « du son, peut-être le mauvais » vaut mieux que rien ; ici l'arbitrage s'**inverse** : « la voix de l'utilisateur, peut-être dans le mauvais tuyau » n'est pas un moindre mal, c'est une **fuite**. Règle de sélection : `wasapi_peripherique::choisir`. Trace : `cable de rendu retenu pour l'ecriture du micro … integree=true … critere="nom partiel"` — **comparer la valeur RETENUE, jamais la seule présence de la ligne** |
| `MICRO_FAUTE_ECRITURE=<n>` | **Chantier E, bloc E2** — **variable de BANC, jamais une configuration livrée**. Fait échouer les *n* prochaines **écritures** WASAPI sur le câble. ⚠️ **ABSENTE = DÉSARMÉE**, et **budget GLOBAL AU PROCESSUS** (`OnceLock`) — c'est la leçon que D10 a payée sur `AUDIO_FAUTE_LECTURE` : un budget relu par fil se réarme à chaque reconstruction, et le chiffre-juge qu'il sert devient **structurellement incapable de quitter zéro**. Transmise par `scripts/run-agent.sh`. 🔴 **JAMAIS ARMÉE À CE JOUR** : le chemin d'échec d'écriture WASAPI **n'a jamais couru** (recette E2, legs n°9) |
| `ACCENT=0` | **Sous-projet ① Divers — la couleur d'accent, sous-bloc A1** (21 août 2026) — **variable de PRODUIT**, pas de banc. Désarme le mécanisme ENTIER : `capteur/fenetre/accent.rs::tour` teste `accent::actif()` **AVANT toute lecture Win32**, donc `ACCENT=0` empêche jusqu'au `SendMessageTimeout` vers l'application, pas seulement l'envoi. ⚠️ **`=0` DÉSACTIVE ; une simple PRÉSENCE n'active pas** — convention de `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`, `PRESSE_PAPIER` et `APPS`, et pour la même raison : tester `is_ok()` armerait le mécanisme en écrivant `ACCENT=0` pour le couper. Lue dans le **capteur**, par `OnceLock`. Transmise par `scripts/run-agent.sh` (ligne posée par une **tâche DÉDIÉE**, quatre fois payée : `SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2, `AUDIO` en D7, `APPS` évité en G1). Trace, **émise seulement si désarmé** : `accent de fenetre DESARME (ACCENT=0) : la couleur de l'icone n'est plus poussee au navigateur` (`warn!`). 🔴 **LE CONTRÔLE QUI VAUT EST LE ZÉRO D'ANNONCES `accent de la fenetre Windows`, PAS LA TRACE** — même patron que `PRESSE_PAPIER=0` ci-dessus : la trace prouve que la variable a atteint le processus, elle ne prouve pas que le mécanisme est coupé, et un zéro seul serait rendu par un produit entièrement en panne. ✅ **CE CONTRÔLE A ÉTÉ JOUÉ, et la ligne a été LUE DANS LE `run-agent.ps1` GÉNÉRÉ SUR LA VM** — jamais dans le script hôte (21 août 2026, **2 exécutions**) : `ps1_accent` rend `["$env:ACCENT = '0'"]`, et le témoin négatif `ACCENT_INEXISTANT` rend `[]`. ⚠️ **Sur le bras ARMÉ la liste est VIDE, et c'est CORRECT** : `${ACCENT:+…}` ne rend rien quand la variable n'est pas posée, l'accent étant armé PAR DÉFAUT — c'est le bras désarmé qui qualifie ce vide. **Relevé du bras désarmé** : **0** annonce `accent de la fenetre Windows`, **1** trace de désarmement (`OnceLock`), **16** lignes d'attache au capteur, **0** `ERROR`, et le token **jamais posé** (chaîne vide) sur les deux pages. 🔴 **Le zéro est discriminant parce que le MÊME montage en rend DEUX sur le bras armé**, et parce que les 16 attaches prouvent qu'il ne s'agit pas d'un produit en panne. |


---

## 🪤 Pièges transverses — ceux que ce dépôt a payés PLUSIEURS fois

⚠️ **Les pièges propres à un chantier sont restés dans son § « Pièges neufs »
du journal.** Ceux d'ici ont été payés dans au moins deux chantiers
indépendants : ce sont eux qui coûtent.

### Méthode de mesure

- 🔴 **UN CONTRÔLE QU'ON N'A JAMAIS VU ROUGE N'EST PAS UN CONTRÔLE.** Payé une
  dizaine de fois. **La règle complète est en deux temps :** exécuter le
  contrôle, **PUIS provoquer délibérément l'état qu'il doit dénoncer et
  vérifier qu'il le dénonce**. Un contrôle peut être exécuté, rendre un
  diagnostic juste, et rester **structurellement incapable de rendre l'autre
  valeur** — c'est ainsi qu'un `grep` de recette écrit pour révéler une panne
  muette a couru sans pouvoir échouer. **Un plan n'immunise pas contre ce
  patron : il en est une source.**
  🔴 **SA FORME LA PLUS TRAÎTRE : UN ATTENDU DÉRIVÉ DE LA MESURE ELLE-MÊME.**
  Payé au lot 32R, en citant la règle dans le même document. Trois points de
  curseur mesurés, le rectangle de mappage **déduit de ces trois points**, puis
  chaque point comparé **à ce rectangle-là** : « écart nul au pixel » ne disait
  rien d'autre que « les trois points sont alignés », et **tout mapping affine
  passait**. L'erreur réelle était de **432 px** au bord droit, et un humain
  l'a vue le lendemain. **L'attendu doit venir du PRODUIT — une dimension
  relevée dans son journal, une constante de sa configuration —, jamais d'un
  calcul sur les points qu'on juge.** ⚠️ Le symptôme est un contrôle qui
  « passe parfaitement » : plus l'accord est bon, plus il faut se demander
  d'où vient l'attendu.
- 🔴 **UNE ROUGE QUI ROUGIT POUR LA MAUVAISE RAISON NE PROUVE RIEN**, et elle
  est indiscernable d'une bonne si l'on ne lit que son code de sortie. **Lire
  QUELLE assertion a rougi.** Corollaire : **une rouge restée VERTE se
  DIAGNOSTIQUE, elle ne se classe pas** — deux fois, le diagnostic a révélé un
  trou de couverture réel.
- 🔴 **UN ZÉRO N'EST INTERPRÉTABLE QU'AVEC UN TÉMOIN NÉGATIF.** Un zéro rendu
  par une trace qu'on n'a pas allumée, par un chemin qui n'existe pas, ou par
  une chaîne que le produit n'émet nulle part, n'est pas une mesure. Vérifier
  dans le même relevé qu'une chose **connue pour exister** rend non-zéro.
- 🔴 **UNE SORTIE VIDE N'EST PAS UN ZÉRO** : `grep` sans `-a` classe « binaire »
  un journal portant un seul octet NUL et rend une sortie **vide**.
- 🔴 **JUGER SUR LA RELECTURE, JAMAIS SUR LE CODE DE RETOUR.** Une API peut
  rendre `0` sur une sortie qui n'a pas bougé d'un pixel ; trois appels
  `waveOut` peuvent tous réussir et ne rien jouer.
- 🔴 **ON JUGE UN SON À SA FRÉQUENCE DOMINANTE**, jamais à un compte d'octets
  ni à une crête : `bytesReceived` croît sur un spectre à −1000 dB, et une
  crête franche se lit sur un silence numérique (VB-Cable rejoue son tampon).
- ⚠️ **UN PALIER DE MESURE DOIT ÊTRE PLUSIEURS FOIS PLUS LONG QUE LA
  TEMPORISATION DU MÉCANISME QU'IL OBSERVE.** Lire les constantes de temps du
  code **avant** de dimensionner un palier. Et **attendre le FAIT, jamais une
  durée**.
- ⚠️ **ÉNONCER LA RÈGLE DE SÉLECTION AVANT DE COMPTER.** Un sous-ensemble sans
  règle énoncée est un sous-ensemble **choisi**, même quand on ne l'a pas
  choisi.
- ⚠️ **UN COMPTEUR DE JOURNAL PEUT COMPTER DES LIGNES ET NON DES ÉVÉNEMENTS**,
  et une « latence » lue dans un journal de pilote peut être une **période
  d'échantillonnage** (facteur 86 relevé une fois). **Prendre la latence du
  côté qui la SUBIT.**
- ⚠️ **UN BUDGET D'INJECTION DE FAUTE DOIT ÊTRE GLOBAL AU PROCESSUS**, jamais
  relu par fil : un budget qui se réarme rend le chiffre-juge **structurellement
  incapable de quitter zéro**, sur un produit pourtant corrigé.
- ⚠️ **DISCULPER UN MAILLON NE DÉSIGNE PAS LE COUPABLE SUIVANT** : il faut une
  mesure **par maillon**.

### Documentation et dérive — « le naufrage du 487 »

- 🔴 **CORRIGER UNE AFFIRMATION EXIGE DE LA CHERCHER, PAS DE LA CORRIGER LÀ OÙ
  ON NOUS L'A MONTRÉE.** Payé **neuf fois**, dont une **à l'intérieur du commit
  qui le dénonçait**. « Corrigé à sa place » est une affirmation de
  **complétude** : énumérer les places par `grep -n` **AVANT** d'écrire, **et
  les relire une par une APRÈS**. Une substitution qui ne dit pas combien
  d'occurrences elle a touchées est une affirmation non vérifiée.
- 🔴 **CHERCHER PAR LE SENS, PAS PAR LA FORMULE** : une négation se dit de
  plusieurs façons, et c'est celle qu'on n'a pas listée qui survit.
- 🔴 **TOUCHER UNE LIGNE D'UN TABLEAU DE COMPTES OBLIGE À REMESURER SON
  COMPTE**, même quand ce n'est pas l'objet de l'édition — deux fois, la main
  était **sur la ligne même** qui portait le chiffre faux. **Mesurer chaque
  ligne d'une table au moment où on l'écrit**, jamais de mémoire.
- 🔴 **RELEVER LES TAILLES APRÈS LA DERNIÈRE ÉDITION DE LA RONDE**, revue
  transverse comprise : une table mesurée en début de ronde est fausse à la fin
  de la même ronde.
- 🔴 **LA DURÉE DE VIE D'UN « CELA RESTE VRAI » EST D'UN SOUS-BLOC.** Mesuré :
  le même en-tête a été corrigé **trois fois, une par sous-bloc**, chaque
  correction laissant une « moitié qui reste vraie » que la suivante a dû
  reprendre.
- 🔴 **UN NUMÉRO DE LIGNE DANS `CLAUDE.md` EST FAUX DÈS QU'ON ÉCRIT AU-DESSUS —
  et on écrit toujours au-dessus. NOMMER LA CHOSE, jamais compter les lignes
  qui l'en séparent.** Idem dans le code quand le chantier **déplace** la ligne
  citée : relire la citation **après** avoir exécuté ce qui la déplace.
- 🔴 **UN MESSAGE DE COMMIT EST UNE PIÈCE DU DÉPÔT, ET PERSONNE NE LE RELIT.**
  Sept écarts sur neuf n'existaient que là. **Relire les journaux contre le
  message, jamais l'inverse.**
- 🔴 **UNE REVUE PAR TÂCHE NE PEUT PAS VOIR UN DÉFAUT QUI FRANCHIT UNE
  FRONTIÈRE DE TÂCHE** — chacun est correct des deux côtés pris séparément.
  Ce n'est pas un défaut de rigueur, c'est une propriété du découpage, et c'est
  ce qui justifie la **revue transverse de fin de branche** (elle a trouvé
  jusqu'à **vingt-sept** affirmations devenues fausses en une branche).
- ⚠️ **CORRIGER UNE AFFIRMATION FAUSSE PEUT EN PRODUIRE UNE AUTRE** : inscrire
  dans le commentaire les commandes qui l'établissent, pour que le prochain
  lecteur refasse le contrôle sans croire personne.
- 🔴 **UNE PREUVE NE DOIT JAMAIS VIVRE DANS UN RAPPORT GITIGNORÉ.** L'espace de
  travail d'un sous-bloc a disparu avec sa session, emportant **six constats de
  revue définitivement perdus**.
- 🔴 **NE JAMAIS FABRIQUER UNE PIÈCE.** Deux transcriptions ont été assemblées à
  la main et présentées comme des relevés : les faits rapportés étaient
  **vrais**, les preuves ne l'étaient pas — donc invérifiables par le suivant.
  Le mécanisme nommé : **réutiliser la sortie d'une commande pour répondre à la
  question d'une AUTRE, sans la relancer.**

### Taille des fichiers

- 🔴 **EXTRAIRE, JAMAIS COMPRIMER.** Le plafond a été franchi une douzaine de
  fois ; il n'a été rattrapé par une compression que **deux** fois, et ce
  fichier l'interdit désormais nommément. **La forme forte est l'extraction
  jouée dans une tâche DÉDIÉE, AVANT celle qui ajoute.**
- 🔴 **LA MARGE REGAGNÉE PAR UNE EXTRACTION SE REPERD SI ON LA TRAITE COMME
  ACQUISE** — payé **six fois**, dont une le jour même dans la branche qui
  l'avait gagnée.
- 🔴 **UNE ADDITION DE COMMENTAIRE PEUT ANNULER UNE EXTRACTION**, et **la revue
  transverse est elle-même une source de croissance** (jusqu'à +54 lignes).
- ⚠️ **UNE EXTRACTION N'EST JAMAIS RIGOUREUSEMENT VERBATIM** : elle laisse ses
  imports derrière elle (d'une famille **autre** que `dead_code` — un `TS6133`
  est un **échec** de `tsc`), déplace les visibilités (un `pub(super)` sans son
  type rend `private_interfaces`), casse les déictiques (« plus bas dans ce
  fichier »), et **déplace ce qu'un garde d'absence surveille**.

### Outillage VM et Windows

- 🔴 **LA VM S'ÉTEINT SEULE, PAR DEUX MÉCANISMES DISTINCTS — les confondre fait
  chercher du mauvais côté.** ① Une hibernation initiée **DANS l'invité**
  (`Kernel-Power` 187/42, QEMU se terminant ~5 s après) ; ② l'**HÔTE** qui tue
  QEMU — `terminating on signal 15 from pid <N>`, ce PID étant
  `libvirtd --timeout 120`, qui s'arrête sur inactivité et **emporte le
  domaine**. **Vérifier `virsh list --all` après toute séquence longue**, et le
  compteur d'extinctions avant/après.
- 🔴 **LA TAILLE DU BINAIRE NE PROUVE RIEN, DANS LES DEUX SENS** : deux
  compilations de la **même** source rendent deux tailles, et un binaire neuf
  peut peser **exactement** autant que celui qu'il remplace — voire **moins**
  en ajoutant du code. Ce qui tranche est **une chaîne qu'on a posée soi-même**,
  cherchée sur le chemin que `run-agent.sh` lance, **avec son témoin négatif et
  une chaîne préexistante**. ⚠️ Une chaîne **courte** peut être inlinée et
  coupée aux frontières de mot ; une constante **sans appelant** est éliminée.
- 🔴 **UN AGENT SURVIVANT TIENT `agent.log`**, et l'on relit alors le journal de
  la tentative **précédente** en croyant lire le sien. **`Get-Process agent`
  avant CHAQUE tentative, y compris échouée** — c'est aussi ce qui fait échouer
  `link.exe` en 1104 / `os error 5`.
  🔴 **ET IL EN EXISTE PLUSIEURS : le journal le plus récent n'est pas
  forcément celui qu'on cherche.** Un lot qui monte sa propre recette écrit
  ailleurs (`C:\nivuus\lot31\agent-lot31.log`), et la dernière occurrence
  d'une erreur dans `agent.log` peut être **la sienne propre**, vieille d'une
  heure. **Trancher sur une DONNÉE du relevé** — le viewport demandé, le
  numéro de session —, jamais sur la seule date.
  🔴 **UN MARQUEUR ÉCRIT PAR `Add-Content` DANS UN JOURNAL TENU PAR UN AGENT
  VIVANT EST SILENCIEUSEMENT PERDU** : l'agent le tient par un `StreamWriter`
  qui écrit à SA position et recouvre l'ajout, et `Add-Content` rend la main
  sans erreur. Poser un marqueur **quand l'agent est ARRÊTÉ** ; s'il tourne,
  **segmenter par HORODATAGE**. Le symptôme est un « bras » qui compte tout le
  journal.
- 🔴 **UN RELEVÉ WinRM EST CELUI DE LA SESSION 0**, jamais de la session
  interactive : identité, intégrité, presse-papier, audio et périphériques y
  diffèrent. **Tout ce qui dépend de la session passe par une tâche planifiée
  `/it`**, dont la sonde doit **imprimer sa session** plutôt que la supposer.
- 🔴 **`build-agent.sh` RSYNCHRONISE L'ARBRE ENTIER**, travail non commité des
  voisins compris : il a échoué sur un module déclaré dont le fichier n'était
  pas suivi. **Vérifier `git status --porcelain agent/ proto/` avant CHAQUE
  build, pas seulement le premier**, ou bâtir depuis un `git worktree` isolé —
  ⚠️ avec `node_modules` lié, sans quoi le script s'arrête en silence.
- 🔴 **UNE VARIABLE NEUVE DOIT ÊTRE AJOUTÉE À `scripts/run-agent.sh` PAR UNE
  TÂCHE DÉDIÉE** — payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`),
  D7 (`AUDIO`) et **lot 32 (`SORTIE_DESIGNEE`)**, où implémenteur **et**
  relecteur avaient vérifié la propriété **en traçant le code** : le tracé
  était juste, la valeur ne pouvait pas atteindre le processus.
  🔴 **LIRE LA LIGNE DANS LE SCRIPT GÉNÉRÉ NE SUFFIT PAS — LE LOT 32 L'A
  MESURÉ.** Une ligne peut être présente dans le `.ps1` **et n'être jamais
  exécutée** : posée APRÈS l'invocation de l'agent, elle n'atteint rien, et le
  fichier semble parfaitement bon. **Ce qui compte est sa POSITION RELATIVE À
  L'INVOCATION** — et le seul contrôle qui ne peut pas mentir est **la trace
  émise par le processus lui-même** (un `warn!` de désarmement, un champ de
  journal), parce qu'elle n'existe que si la valeur est arrivée. ⚠️ **Une
  rouge de banc dont la variable n'atteint pas le processus est VACUEUSE et
  se lit exactement comme une bonne** : elle rougit, pour la mauvaise raison.
- ⚠️ **`nodejs-winrm` ENVELOPPE TOUJOURS LA COMMANDE** dans
  `powershell -Command "& { … }"` : un script en ligne portant guillemets ou
  parenthèses entre en collision, **et le symptôme est un script qui ne tourne
  jamais**. Écrire le script sur le partage et l'invoquer par `-File`.
  ⚠️ Il **rend la main dès la première ligne et tue le processus distant** :
  rediriger tous les flux vers un fichier, et le lire depuis l'hôte.
- ⚠️ **UN `.ps1` SANS BOM CONTENANT UN SEUL CARACTÈRE NON-ASCII NE S'ANALYSE
  PAS**, et l'erreur désigne une **autre** ligne. Contrôle en une ligne :
  `LC_ALL=C grep -c '[^ -~]' fichier.ps1` doit rendre **0**.
- ⚠️ **LE DÉFAUT À DEUX RÉGLAGES, TOUJOURS NON CORRIGÉ** dans
  `build-agent.sh` / `run-agent.sh` : un journal PowerShell lisible demande
  `[Console]::OutputEncoding` (**lecture**) **ET** un `StreamWriter` UTF-8
  (**écriture**). Symptôme : un `grep` sur un mot accentué rend 0 quand le même
  `grep` sur sa partie ASCII rend 1.
- ⚠️ **UNE SORTIE VIRTUELLE ET UNE RACINE ProjFS SURVIVENT À UN ARRÊT BRUTAL** :
  le `Drop` ne court pas sur un `TerminateProcess`. **Purger
  (`MULTIFENETRE_VDD_PURGE=1`) entre deux exécutions.**
- ⚠️ **COMPTER LES FENÊTRES, JAMAIS LES LANCEMENTS** : Paint en ouvre deux,
  Bloc-notes fait avancer le compteur de sessions de deux, et Chrome rejoint
  son instance existante sans `--user-data-dir` distinct.

### Shell de l'hôte

- 🔴 **UN SERVICE LAISSÉ PAR UNE EXÉCUTION PRÉCÉDENTE RÉPOND À LA PLACE DU
  VÔTRE, ET LE SYMPTÔME EST UN `404` QU'ON LIT COMME UN DÉPÔT RATÉ.** Payé
  **deux fois** sur `python3 -m http.server` (lots 32Q et 32T). Cause prouvée
  par PID et ligne de commande : `(cd D && python3 … & echo $!)` met le
  `cd && python3` **entier** en arrière-plan, donc `$!` désigne le
  **sous-shell** — le `kill` le tue, le serveur survit, le suivant ne peut plus
  se lier au port, et c'est **l'ancien** qui répond, depuis l'autre répertoire.
  **Ne jamais mettre un `cd` dans la commande qu'on met en arrière-plan**
  (`--directory`, `--chdir`, un chemin absolu), **libérer le port par PID
  relevé** avant de servir, et **juger sur ce qui est arrivé à destination** —
  ici la comparaison des deux sha256, qui a attrapé le défaut les deux fois là
  où aucun code de retour ne le pouvait.
- 🔴 **`pkill -f <motif>` DEPUIS UN SHELL DONT LA LIGNE DE COMMANDE CONTIENT LE
  MOTIF TUE LE SHELL** (exit 144, la suite de la chaîne ne s'exécute pas).
  Payé **au moins quatre fois**. **Tuer par PID relevé**, jamais par motif — et
  jamais par une heuristique de rang : l'ordre est superviseur, capteur, pont,
  **puis** les enfants.
- 🔴 **ZSH** : `"$var:suffixe"` applique un modificateur d'historique et **mange
  la valeur** (`$P:e2-v1` rend `2-v1`) ; zsh **ne découpe pas** les variables en
  mots, donc `git add $FICHIERS` passe la liste entière comme **un seul
  chemin**. **Toujours `${var}`, et des arguments littéraux.**
- 🔴 **DES BACKTICKS DANS UN `echo` DE JOURNAL EXÉCUTENT UNE COMMANDE**, et un
  `git commit -m` portant des accents graves **mutile les phrases**. **Quotes
  simples pour toute prose ; message long par un fichier (`-F`).**
- 🔴 **`git checkout -- <fichier>` RESTAURE HEAD, PAS L'ÉTAT D'AVANT LA
  MUTATION** : il a effacé du travail non commité. **Une rouge se restaure
  depuis une COPIE NOMMÉE**, et son garde « la mutation a-t-elle changé quelque
  chose ? » doit comparer **à cette copie**, jamais à HEAD (sinon il ne peut
  pas échouer).
- 🔴 **MUTER PAR NUMÉRO DE LIGNE OU PAR UN MOTIF ANCRÉ SUR LA SYNTAXE, JAMAIS
  PAR UNE SOUS-CHAÎNE** : dans un dépôt qui commente ses invariants, la
  substitution frappe **le commentaire avant le code** — et une ligne peut
  exister cinq fois. `assert texte.count(ancre) == 1`.
- ⚠️ **LE HOOK `chpwd` DU SHELL HÔTE INJECTE UN `ls` DANS CHAQUE JOURNAL** dès
  qu'un `cd` court dans un sous-shell : `unset -f chpwd` avant toute collecte.
- ⚠️ **`grep -qa $'\000'` CHERCHE LA CHAÎNE VIDE ET MATCHE TOUT** : un contrôle
  d'octets NUL écrit ainsi ne peut pas échouer.
- 🔴 **`sed 's/[^ -~]//g'` SANS `LC_ALL=C` MANGE DES LETTRES ASCII** — la plage
  ` -~` est dépendante de la **collation** de la locale, pas des octets. Payé
  au lot 32O : un `.ps1` « nettoyé » de son unique caractère non-ASCII est
  ressorti avec `[System.Diagnostics.Process]::GetCurrentProcess()` réduit à
  `[..]::()`, et la sonde a échoué à l'analyse — **sans que rien ne dise que
  le fichier avait été mutilé**. ⚠️ Le contrôle `LC_ALL=C grep -c '[^ -~]'`,
  lui, est juste : c'est le `sed` qui doit porter `LC_ALL=C`, pas seulement le
  `grep` qui le vérifie. **Et le symptôme se lit comme une erreur de syntaxe
  du script**, jamais comme une corruption — c'est en LISANT la sortie plutôt
  qu'en la supposant qu'on le voit.
- ⚠️ **UN FICHIER DE CONTRÔLE PEUT SE POLLUER LUI-MÊME** : `echo` interprète
  `\x1b` et `\r`, si bien qu'un fichier qui **décrit** ses motifs les
  **contient**. Heredoc **cité**.

### Tests et types

- 🔴 **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`** — la racine
  Vitest est `client/`. **Deux commandes, jamais une**, et un test posé hors de
  `client/src/` ne tournerait pas sans que personne ne le voie.
- 🔴 **VITEST TRANSPILE SANS VÉRIFIER LES TYPES** (esbuild) : `tsc --noEmit` est
  une étape **distincte et obligatoire**. Elle seule attrape `Buffer` sans
  `@types/node`, ou un `Uint8Array` passé comme `BlobPart`.
- 🔴 **`expect(x).toBe(y, 'message')` EST SILENCIEUSEMENT IGNORÉ** par Vitest —
  attrapé par `tsc`, et par lui seul.
- 🔴 **TROIS ASSERTIONS DANS UN TEST NE PROUVENT QUE LA PREMIÈRE** : `expect`
  s'arrête au premier échec. Une assertion de **perte de données** placée en
  seconde position n'était éprouvée par **rien**.
- 🔴 **UNE SOURCE FACTICE QUI IMPLÉMENTE UN EFFET DE BORD EN NO-OP REND UNE
  FAMILLE ENTIÈRE DE DÉFAUTS INVISIBLE** : 456 tests verts sur un produit muet.
  De même, un test qui **normalise** ce qu'il éprouve (un `Number(...)`
  défensif) n'éprouve plus rien.
- 🔴 **UN `match` CATCH-ALL TUE UN FIL EN SILENCE.** `capteur/pont_media.rs`
  porte un `Ok(autre) => return` qui **tue `lire_le_media` sans panne
  apparente** : payé **SIX fois** (`Sommeil`, `Part`, `Audio`, `PleinEcran`,
  `PressePapier`, `Accent`). **À vérifier pour tout message neuf du capteur.**
  ⚠️ Son jumeau `transport/controle.rs` porte un `match` **exhaustif** : il se
  signale au compilateur, mais **le bras part dans le MÊME commit que la
  variante**, sinon l'arbre ne compile pas — un voisin qui bâtit depuis l'arbre
  partagé y perd sa mesure.
- 🔴 **UN COMPTE DE TESTS N'EST ATTRIBUABLE QU'ASSORTI DE SON COMMIT** quand
  plusieurs chantiers partagent l'arbre — mesuré : 107 → 120 en vingt-deux
  minutes, sans qu'aucune ligne du chantier ne bouge. **Mesurer avec
  `git show HEAD:` ou depuis un `git worktree`, et dater tout compte.**
- ⚠️ **NE JAMAIS `git add -A` DANS UN ARBRE PARTAGÉ** : un `git add -A agent/src`
  a emporté le travail concurrent d'une autre tâche, sans sa déclaration de
  module. **Nommer les fichiers** — et ⚠️ un pathspec de **répertoire** ne prend
  pas le module **homonyme** (`agent/src/pont` laisse `agent/src/pont.rs`).
- ⚠️ **UN `||` DE REPLI TRANSFORME « FICHIER ABSENT » EN « CONTRÔLE VERT ».**
- ⚠️ **LE NOM D'UN `grep` DE RECETTE SE VÉRIFIE CONTRE LE CODE, JAMAIS CONTRE LA
  SPEC** — et **sur le VERT**, avant de conclure quoi que ce soit du rouge.

### Ce que le montage de recette ne peut pas voir

- 🔴 **CE QU'UN NAVIGATEUR EXIGE, AUCUN TEST DE NODE NE LE VOIT** : deux défauts
  CORS rendaient deux routes inatteignables avec une suite entièrement verte.
  **Cette classe n'a AUCUN garde automatique dans ce dépôt.**
- 🔴 **UN CHROME SANS INTERFACE RAPPORTE `document.hidden = true` POUR TOUTE
  FENÊTRE D'ARRIÈRE-PLAN**, n'entre **pas** réellement en plein écran, n'expose
  **pas** `navigator.keyboard`, et n'a **aucun chemin acoustique**. **La
  visibilité et le focus sont donc IMPOSÉS par le pilote de recette, page par
  page** — limite héritée de D5, **qu'aucun sous-bloc n'a levée**.
  ⚠️ `window.open(url, nom)` ouvre un **ONGLET** ; avec une chaîne de
  caractéristiques, une **POPUP** — et cela change le comportement du focus.
  **Une sonde qui croit reproduire un geste doit le RELIRE, pas s'en souvenir.**
- 🔴 **`Page.addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page ouverte
  par `window.open`** : poser l'amorce explicitement, page par page.
- 🔴 **UNE BOUCLE D'ATTENTE QUI NE DIT PAS CE QU'ELLE A VU REND UN ÉCHEC
  INDISCERNABLE D'UN PRODUIT EN PANNE.** Une boucle « aucune page de session »
  a coûté **deux créneaux d'interruption du propriétaire** avant qu'on ne lui
  fasse journaliser les cibles CDP **et l'état de la page** — laquelle portait
  la réponse en toutes lettres : *« Bureau refusé : un client est déjà connecté
  à la session … »*. **Faire dire à l'attente ce qu'elle observe**, pas
  seulement qu'elle a renoncé.
- 🔴 **LE RÔLE `client` EST EXCLUSIF PAR SESSION (lot 22) : AUCUNE RECETTE
  NAVIGATEUR NE PEUT ÊTRE JOUÉE PENDANT QUE LE PROPRIÉTAIRE EST CONNECTÉ**, et
  réciproquement un pilote qui se connecte **lui prend sa place**. Toute
  campagne visant cette VM doit donc être **annoncée et bornée dans le temps**,
  et son échec le plus probable n'est pas technique : c'est un humain déjà là.
  🔵 **LE REMÈDE, trouvé au lot 32C : `Target.setAutoAttach` avec
  `waitForDebuggerOnStart` au niveau NAVIGATEUR.** Chaque cible neuve pause
  avant son premier script ; on y pose l'amorce, puis `Runtime.
  runIfWaitingForDebugger` la relâche. ⚠️ **L'URL portée par
  `Target.attachedToTarget` est celle d'AVANT navigation (`about:blank`)** :
  apparier dessus ne marche jamais — **demander** son adresse à chaque session.
  🔴 **ET SURTOUT : NE PAS FERMER LA POP-UP POUR LA ROUVRIR.** L'agent voit
  alors partir son pair, démonte la session, et la page rouverte reste à
  `ice=new` — mesuré. L'amorce se pose **sans rien fermer**.
- 🔴 **TOUTE ÉVALUATION CDP SUR UNE PAGE PORTANT UN FLUX WebRTC ACTIF DOIT ÊTRE
  BORNÉE** : `Page.captureScreenshot` peut ne **jamais** rendre. Et
  `execFileSync` **bloque la boucle d'événements de Node**, donc le pilote cesse
  de lire son WebSocket CDP — **l'instrument détruit ce qu'il mesure**.
- 🔴 **UNE MIRE IMMOBILE NE PRODUIT AUCUNE IMAGE** : Desktop Duplication n'émet
  qu'au changement du bureau. **Animer la source, à une cadence CONNUE et
  affichée par la source elle-même.**
- ⚠️ **NE JAMAIS TRACER PAR PAQUET DANS LA BOUCLE DE TRANSPORT** : 18 619 lignes
  en quelques secondes sur un partage CIFS ont empêché une session de
  s'établir. **Compter ou échantillonner.**

---

## 🧭 Index des chantiers

**Chaque ligne renvoie à son document de résultats — le seul qui fasse foi.**
Le récit complet, avec ses mesures et ses réfutations annotées, est dans
[`docs/JOURNAL.md`](docs/JOURNAL.md), sous le titre exact repris ici.

⚠️ **Aucun chiffre de ces documents ne se recopie sans relancer sa commande.**
Ils sont **datés**, et plusieurs se réfutent les uns les autres à dessein.

### Mesures fondatrices — réseau, NAT, capture

- **Chantier C volet 1 — asservissement réseau (fusionné le 29 juillet 2026)** — [résultats](docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md)
- **Chantier C volet 2 — traversée NAT (30 juillet 2026)** — [résultats](docs/superpowers/plans/2026-07-29-traversee-nat-resultats.md)
- **Sonde de capture multi-fenêtres (30 juillet 2026)** — [résultats](docs/superpowers/plans/2026-07-30-sonde-capture-multifenetre-resultats.md)

### Chantier D — multi-fenêtres (D1 → D11)

- **Mesures préalables au chantier D (31 juillet 2026)** — [résultats](docs/superpowers/plans/2026-07-31-mesures-prealables-chantier-d-resultats.md)
- **N duplications DXGI de front sur N sorties virtuelles (31 juillet 2026)** — [résultats](docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md)
- **Sous-bloc D1 — tranche verticale multi-fenêtres (1ᵉʳ août 2026)** — [résultats](docs/superpowers/plans/2026-08-01-multifenetres-tranche-verticale-resultats.md)
- **Sous-bloc D2 — arrangement multi-fenêtres dynamique (1ᵉʳ août 2026)** — [résultats](docs/superpowers/plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md)
- **Sous-bloc D3 — retenir les sorties, et caractériser le plafond de concurrence (2 août 2026)** — [résultats](docs/superpowers/plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md)
- **Sous-bloc D4 — capture mutualisée : huit fenêtres diffusent (2 août 2026)** — [résultats](docs/superpowers/plans/2026-08-02-multifenetres-capture-mutualisee-resultats.md)
- **Sous-bloc D5 — le vivier d'encodeurs, et la mise en sommeil (3 août 2026)** — [résultats](docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs-resultats.md)
- **Sous-bloc D6 — le budget de session, et la réfutation de sa propre prémisse (3 août 2026)** — [résultats](docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md)
- **Sous-bloc D7 — l'audio par fenêtre (3 août 2026)** — [résultats](docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md)
- **Sous-bloc D8 — le plein écran par fenêtre (5 août 2026)** — [résultats](docs/superpowers/plans/2026-08-04-multifenetres-plein-ecran-resultats.md)
- **Sous-bloc D9 — solder la dette, et ce qu'on retire au lieu de le réparer (6 août 2026)** — [résultats](docs/superpowers/plans/2026-08-06-multifenetres-solder-la-dette-resultats.md)
- **Sous-bloc D10 — solder la branche : la sortie cesse d'être la fenêtre (7 août 2026)** — [résultats](docs/superpowers/plans/2026-08-07-multifenetres-solder-la-branche-resultats.md)
- **Sous-bloc D11 — solder les legs : le son revient au cas majoritaire (19 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-multifenetres-solder-les-legs-resultats.md)

### Chantier E — microphone (E1 → E3)

- **Chantier E — Microphone, bloc E1 : le sens montant (19 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-micro-resultats.md)
- **Chantier E — Microphone, bloc E2 : une application Windows ENTEND (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-20-micro-e2-resultats.md)
- **Chantier E — Microphone, bloc E3 : le micro en MULTI-FENÊTRES (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-micro-e3-resultats.md)

### Sous-projet ③ — pont fichiers (F0 → F5, CLOS)

- **Sous-projet ③ Pont fichiers — sous-blocs F0 et F1 : un lecteur en lecture seule (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-pont-fichiers-f1-resultats.md)
- **Sous-projet ③ Pont fichiers — sous-bloc F2 : la VM enregistre, le poste local reçoit (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-20-pont-fichiers-f2-resultats.md)
- **Sous-projet ③ Pont fichiers — sous-bloc F3 : le renommage, la suppression, et la casse en lecture (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-20-pont-fichiers-f3-resultats.md)
- **Sous-projet ③ Pont fichiers — sous-bloc F4 : la mesure que le cadrage réclamait, et deux murs (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-pont-fichiers-f4-resultats.md)
- **Sous-projet ③ Pont fichiers — sous-bloc F5 : la vie longue, et la CLÔTURE de ③ (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-pont-fichiers-f5-resultats.md)

### Sous-projet ④ — gestion d'apps (G1 → G5)

- **Sous-projet ④ Gestion d'apps — sous-bloc G1 : le catalogue naît, et on peut lancer ce qu'il contient (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-gestion-apps-g1-resultats.md)
- **Sous-projet ④ Gestion d'apps — sous-bloc G2 : les icônes 256, et la preuve que c'en est (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-20-gestion-apps-g2-resultats.md)
- **Sous-projet ④ Gestion d'apps — sous-bloc G3 : le téléversement, l'exécution, et son issue (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-20-gestion-apps-g3-resultats.md)
- **Sous-projet ④ Gestion d'apps — sous-bloc G4 : la surveillance, qui n'est qu'une accélération (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-gestion-apps-g4-resultats.md)
- **Sous-projet ④ Gestion d'apps — sous-bloc G5 : la PWA par application (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-gestion-apps-g5-resultats.md)

### Sous-projet ⑤ — plateforme (P1 → P5, CLOS)

- **Sous-projet ⑤ Plateforme — sous-bloc P1 : le service naît, absorbe le signaling, et persiste (19 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md)
- **Sous-projet ⑤ Plateforme — sous-bloc P2 : l'identité des humains, et la moitié du trou que la garde ferme (19 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-plateforme-p2-resultats.md)
- **Sous-projet ⑤ Plateforme — sous-bloc P3 : l'identité des agents, et le canal plateforme ↔ agent (19 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-plateforme-p3-resultats.md)
- **Sous-projet ⑤ Plateforme — sous-bloc P4 : l'orchestration, et la première VM qui appartient à quelqu'un (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-plateforme-p4-resultats.md)
- **Sous-projet ⑤ Plateforme — sous-bloc P5 : le durcissement, le déploiement, et la CLÔTURE de ⑤ (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-plateforme-p5-resultats.md)

### Sous-projet ⑥ — design system (S1 → S4, CLOS)

- **Sous-projet ⑥ Design system — sous-bloc S1 : le socle, et les sept contrôles (19 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-design-system-s1-resultats.md)
- **Sous-projet ⑥ Design system — sous-bloc S2 : les primitives (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-design-system-s2-resultats.md)
- **Sous-projet ⑥ Design system — sous-bloc S3 : les deux surfaces habillées (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-design-system-s3-resultats.md)
- **Sous-projet ⑥ Design system — sous-bloc S4 : la fenêtre de session (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-20-design-system-s4-resultats.md)

### Sous-projet ① — divers : presse-papier, couleur d'accent

- **Sous-projet ① Divers — presse-papier, sous-bloc P1 : la VM copie, le navigateur colle (20 août 2026)** — [résultats](docs/superpowers/plans/2026-08-19-presse-papier-p1-resultats.md)
- **Sous-projet ① Divers — presse-papier, sous-bloc **P2** : le navigateur colle dans la VM (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-20-presse-papier-p2-resultats.md)
- **Sous-projet ① Divers — presse-papier, sous-bloc P3 : les N fenêtres (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-presse-papier-p3-resultats.md)
- **Sous-projet ① Divers — la couleur d'accent, sous-bloc A1 (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-accent-a1-resultats.md)

### Chantier auth-pomerium — l'identité vient du proxy (CLOS)

- **auth-pomerium : l'identité vient du proxy, le jeton interne RESTE (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-auth-pomerium-resultats.md)
  — ⚠️ **RÉSERVE : le critère ⑦ (la page, dans un navigateur, derrière Pomerium) n'a TOUJOURS PAS été joué**, mais ~~un de ses deux blocages n'est pas une limite de recette mais un défaut de conception déjà appliqué au `config.yaml` réel~~ **LE BLOCAGE ① (LA PLATEFORME NE SERVAIT AUCUN FICHIER STATIQUE) EST LEVÉ le 22 août 2026** : elle sert désormais la page bâtie (`PLATEFORME_PAGE`, voir le tableau des variables ci-dessus), donc la route nue de la spec §7.2 vise un backend qui répond. **Le blocage ② DEMEURE** : le flux OAuth Google exige un humain, qu'aucun Chrome sans interface ne peut fournir — le critère ⑦ reste **NON JOUÉ**, pour cette seule raison désormais. Voir les legs ci-dessous et la spec `auth-pomerium` § 7.
- **page-derriere-pomerium : la plateforme sert la page bâtie, `/auth/moi` ne croit que le pair déclaré (22 août 2026)** — [résultats](docs/superpowers/plans/2026-08-22-page-derriere-pomerium-resultats.md) — les huit critères joués, chaque rouge comprise ; le blocage ① ci-dessus est ce que ce lot lève, le blocage ② (OAuth, un humain requis) reste dû, et ce document distingue nommément ce « critère ⑦ » (les deux bras de la garde d'identité) de celui d'`auth-pomerium` (la page dans un navigateur réel).

### Le retrait du legacy (CLOS)

- **Retrait du legacy — état des verrous : DEUX satisfaits sur dix (21 août 2026)** — [relevé](docs/superpowers/plans/2026-08-21-retrait-legacy-etat-des-verrous.md)
- **Retrait du legacy — EXÉCUTÉ, sur décision du propriétaire (21 août 2026)** — [résultats](docs/superpowers/plans/2026-08-21-retrait-legacy-resultats.md)

### Chantier legs-sans-vm (CLOS)

- **legs-sans-vm : huit legs fermés sans la VM, et la revue transverse de fin de lot (26 août 2026)** — [résultats](docs/superpowers/plans/2026-08-26-legs-sans-vm-resultats.md) — canal `Message` du capteur borné, `GET /vm`/`POST /session`/`/signal` freinés par un budget de volume, le contrat de `SIGNALING_URL` figé par un test, les deux magasins évincés par âge, `tokens.css` extrait à marge nulle, `--accent-fenetre` déclaré et peint, douze pilotes de recette réparés vers `/signal`, les galeries dotées d'un test de liste de cas. Le **WCO reste ouvert**, écarté par décision (hub non installable) ; aucun pilote n'est rejoué, aucun jugement visuel n'est porté.

### Chantier package-nivuus — `desk` devient un package Nivuus

- **package-nivuus : `desk` devient un package Nivuus, et l'agent redevient refabricable (29 août 2026)** — [résultats](docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md) — `scripts/build-agent-croise.sh` refabrique `agent.exe` en croisé (mingw, jamais exécuté sur la VM) là où l'appliance le déclarait « never fetchable » ; manifeste et wizard (`requires.packages: [console]`) ; trois hooks (`resolve`/`install`/`activate`) qui refusent avant que le disque ne soit touché, posent le service systemd (jamais armé par `install`) et l'arment par un lien ; `hooks/vm.py` pose ProjFS et VB-Audio dans la VM par le chemin WinRM de `console` ; `agent.exe` déposé là où `console` va le chercher ; `Makefile`/`README-package.md`. **N'établit ni que l'agent croisé fonctionne, ni qu'une installation a été jouée de bout en bout sur une machine neuve, ni aucun des douze items du lot 3 (suspendu)** — voir son § « Ce que ce lot n'établit PAS ». 🔴 **Défaut transverse trouvé, non corrigé** : rien ne pousse `AGENT_VM`/`AGENT_SECRET` dans l'environnement de l'agent qui tourne réellement dans la VM — sans ce couple, aucune session ne peut s'établir, quelle que soit la qualité de l'installation.
- **lot 17 — un pair qui arrive tard voit les fenêtres (30 août 2026)** — [résultats](docs/superpowers/plans/2026-08-30-lot17-reannonce-fenetres-resultats.md) — le relais gagne `pair-present`, **le jumeau symétrique de `peer-gone`**, et le superviseur redit ses fenêtres à une page-shell qui arrive après lui. 🔴 **Le défaut n'était PAS que `DELAI_ATTENTE_VIEWPORT_MAX` soit trop court** : les annonces partaient vers un socket inexistant et `send(peer, …)` les laissait tomber sans une trace — la borne ne faisait que rendre la perte visible trente secondes plus tard. Elle est **intacte**, et court désormais depuis l'arrivée de la shell. Mesuré sur la VM : **0** fenêtre au bras rouge, **4** et **5** aux deux bras verts. 🔴 **Legs distinct trouvé et NON corrigé : la connexion de contrôle du superviseur ne se reconnecte JAMAIS** — voir « Ce qu'aucun chantier n'a jamais mesuré ».
- **package-nivuus, lots 10A à 13 : la MISE EN SERVICE réelle, et quatre défauts que seul un navigateur voyait (30 août 2026)** — [résultats](docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md) § 10 — un service `desk` **tourne en production** sur cette machine (`192.168.3.1:3445`, derrière `https://app.allanic.me`, mode **`pomerium`** sur décision du propriétaire), servi par une copie déployée sous `/opt/nivuus/desk`. 🔴 **Le bug le plus grave n'était pas dans le package** : `PLATEFORME_HOTE` était confondu avec l'adresse TURN dérivée de la route par défaut — **l'adresse PUBLIQUE de cet hôte** —, et la garde du produit ne pouvait pas l'attraper (`ECOUTES_UNIVERSELLES` ne connaît que quatre littéraux, jamais « une adresse routable ordinaire »). Aussi : la CSP interdisait l'amorce anti-FOUC de la plateforme (sortie du HTML plutôt qu'un hash, parce que `deploiement/nginx.conf` porte une copie STATIQUE de la même CSP) ; le hub était **vide** faute d'attribution de VM, alors que `routes-applications.ts` l'écrivait déjà en toutes lettres. ⚠️ **La racine `/` sert la PAGE DE SESSION, qui se rabat sur la session `demo` sans jeton** : décision non prise, offerte au propriétaire.
- **lots 32M à 32T — la souris, le cadrage, et une dérive qui n'était pas un décalage (31 août 2026)** — [diagnostic et suites](docs/superpowers/plans/2026-08-30-souris-et-cadrage-diagnostic.md) — la référence des entrées était **la zone client de la fenêtre** alors que la capture est **un recadrage de la sortie DXGI** : erreur à deux termes, origine (+1288 px, fermée au lot 32Q) puis échelle (**+432 px au bord droit**, fermée au lot 32T). 🔴 **Le défaut d'échelle est SYSTÉMATIQUE sur cette machine** : les trois sorties virtuelles mesurent **1860×1080** alors qu'elles sont toutes créées à **1428×1080** (relevé en session 1). 🔴 **La taille de l'image n'est pas recalculée dans l'enfant, elle est PARTAGÉE** (`entrees::TailleImage`, un seul `AtomicU64`, seul stockage de la taille de `SourceDistante`) — deux descriptions du même rectangle sont le mécanisme des deux défauts. ⚠️ **La mesure du curseur reste DUE, rouge comme verte** : le rôle `client` est exclusif et le propriétaire était connecté ; E1 n'est établie que par l'arithmétique et les tests d'hôte. 🔵 **Le lot voisin (NVENC / porte Apollo) est DISCULPÉ.**
- **package-nivuus, revue finale de branche : une Critique et six Importantes (30 août 2026)** — [résultats](docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md) § 12 — 🔴 **le package NE POUVAIT PAS S'INSTALLER** : `hooks/resolve.py` refusait sur `hw["vm_windows"]`, une clé qu'**aucun producteur du moteur ne pose**, à une phase (avant `partition()`) où la VM ne peut pas exister ; le refus devient un `StepError` qui arrête l'installation **entière**. **Huit suites et neuf revues ne pouvaient pas le voir : elles FABRIQUAIENT la clé dont elles vérifiaient la consommation.** La porte a migré dans `activate`, où elle est une mesure, et `tests/test_desk_contrat_hw.py` fige le contrat en lisant le **producteur**. Aussi fermés : le runtime Node **déposé** au lieu d'être supposé (il l'était par un geste manuel consigné dans un rapport gitignoré), un refus qui arrive **avant** le premier secret écrit, et les `facts` validés au lieu d'être crus sur parole.
- **lot 31 — la porte d'Apollo, et `desk` qui la prend (30 août 2026)** — [résultats](docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md) — 🔴 **La MFT `NVIDIA H.264 Encoder MFT` s'active en session 0 et rend `0x8000FFFF` en SESSION 1**, sur les QUATRE arrangements que Media Foundation permet — avec **deux témoins verts dans la même exécution** (encodeur H.264 *logiciel* et processeur vidéo *logiciel* s'activent, eux) : la machinerie n'est pas en cause, seule la MFT matérielle refuse. Apollo, même machine et même session, fabrique six encodeurs par l'**API NVENC native** et ne charge **aucun** module Media Foundation. **Trois remèdes bon marché sont RÉFUTÉS PAR LA MESURE** — `MFT_ENUM_ADAPTER_LUID`, un périphérique D3D11 NVIDIA vivant, et lier l'affichage virtuel au GPU NVIDIA (déjà vrai chez nous). ✅ **`desk` emprunte désormais la même porte** : NVENC natif quand un adaptateur NVIDIA est présent, **la MFT restant le dos GÉNÉRIQUE** (Intel Quick Sync, AMD VCE, session 0) — la retirer priverait une machine sans NVIDIA de TOUT encodeur matériel, et trois tests d'hôte figent cette régression. Mesuré : **1195 unités d'accès en 10 s** là où la MFT rendait *aucune*. 🔵 L'ABI est **dérivée en compilant l'en-tête réel** (`FFmpeg/nv-codec-headers` `n12.2.72.0`, notice MIT/NVIDIA reproduite, frontière d'attribution en UN sous-arbre) et vérifiée **SUR LA CIBLE** par 29 `_Static_assert` compilées par mingw — sans exécution, donc rejouables. Quatre pièges qu'aucune erreur lisible n'aurait signalés : les **deux empaquetages de version** (`(majeure<<4)|mineure` du pilote ≠ `majeure|(mineure<<24)` de l'API), **`ARGB` et non `ABGR`** (intervertir rouge et bleu SANS erreur), `PIC_STRUCT_FRAME` qui vaut **1 et non 0**, et le membre de queue oublié qui ferait écrire NVENC **32 octets au-delà de l'allocation**. 🔵 **`encode.rs` SORT DE LA DETTE** (1536 → 427). ⚠️ **CE QUE CE LOT N'ÉTABLIT PAS** : le **plafond d'encodeurs à N fenêtres** n'est pas mesuré — son banc n'en ouvre qu'un, protocole écrit et **NON JOUÉ** — et **personne n'a regardé une image** : `verdicts_faux=0` dit qu'aucun verdict n'est faux, **pas qu'une image est juste**. 🔴 **Le chiffre-juge — `framesDecoded` — a été relevé par le LOT 32, pas par celui-ci**, et avec DEUX remèdes en place : voir sa ligne.
- **lot 32 — la première sortie virtuelle et la cible forcée (30 août 2026)** — [résultats](docs/superpowers/plans/2026-08-30-premiere-sortie-cible-forcee-resultats.md) — l'appariement cesse de DEVINER la sortie par une différence d'ensembles et la DÉSIGNE par le couple `(adaptateur, identifiant de cible)` que le pilote rend déjà (API CCD, `moniteurs_virtuels/config_affichage.rs`) ; le repli d'hier reste en place mot pour mot. 🔴 **La cause première était une affirmation FAUSSE de `placement.rs` écrite en D1 — « aucune correspondance n'est exposée » — corrigée avec les commandes qui l'établissent.** Mesuré sur la VM, VGA retiré : bras désarmé **8 sorties créées, 8 refus, 0 fenêtre tenue** ; bras armé **`chemin ① = 1`, `nom_designe="\\.\DISPLAY5"`** — le nom que portait la cible forcée (`statusFlags=0x11`, relevé AVANT de conclure). L'hypothèse que `sudovda.rs` déclarait « non confirmée » est **établie** : id pilote = cible CCD = UID du moniteur = 257.
- **lot 32C — Apollo assoupli, et le JUGE : une image de la VM arrive au navigateur (30 août 2026)** — [résultats](docs/superpowers/plans/2026-08-30-juge-image-au-navigateur-resultats.md) — 🔴 **le chiffre-juge est TOMBÉ, VERT, deux fois** : `framesDecoded` **+494** et **+484** sur 25 s (≈19 i/s), `bytesVideo` **+1,29 Mo**, ICE `connected` — avec son **témoin négatif mesuré par le même instrument** (source STATIQUE : `framesDecoded` **0** et `bytesVideo` **0**, alors que `bytesAudio` coule et qu'ICE est connecté). Les deux remèdes courent ensemble : la **désignation** du lot 32 et l'**encodeur NVENC natif** du lot 31. L'agent bâti en croisé est **DÉPLOYÉ** (`hooks/agent_payload.py::deposer_agent_console`, session 1 attestée par l'appliance), et `AGENT_VM`/`AGENT_SECRET` **atteignent le processus vivant** (trace d'enrôlement présente, avertissement d'absence à 0). ✅ **Moonlight fonctionne** — établi par le PROPRIÉTAIRE avec son client, pas par ce lot. 🔴 **MAIS `ensure_active` NE SUFFIT PAS, et c'est MESURÉ** : Apollo `Running` → **7 refus, 0 fenêtre tenue** ; Apollo `Stopped` → **0 refus, 4 tenues**, même binaire à deux minutes d'intervalle, **et AUCUN client connecté** (`Get-NetTCPConnection` = 0). Apollo **relance sa sonde d'encodeur à chaque changement de topologie**, à la cadence de 5 s de `LIMITE_RATTACHEMENT`. **La coexistence n'est PAS acquise, et le diagnostic qui imputait tout à `ensure_only_display` était INCOMPLET.** **Suite (lot 32E) : `desk` gagne une REPRISE bornée** (`superviseur/reprise.rs`, 3 tours sur la MÊME sortie — recréer redéclencherait la sonde d'Apollo —, 6 tests d'hôte, pire cas 5 s → **17 s de boucle bloquée**, hors portée de `DELAI_ATTENTE_VIEWPORT_MAX` **vérifié**). 🔴 **ELLE N'A JAMAIS TIRÉ** : `on RÉESSAIE` = **0** dans les trois bras, y compris celui où Apollo sondait pour de bon — la condition qui échouait ne s'est pas reproduite. **Non-régression seulement ; le remède n'est PAS démontré.** 🔵 Et le refus DOMINANT est désormais un autre défaut, mesuré : **`plus aucune sortie virtuelle disponible`**, 27 à 33 fenêtres annoncées pour un vivier de dix.
- **lot 32E→32I — le vivier épuisé : trois réfutations, puis la règle d'APPARTENANCE (30 août 2026)** — [résultats](docs/superpowers/plans/2026-08-30-vivier-epuise-diagnostic.md) — 🔴 **trois cadrages successifs réfutés par la mesure, le mien inclus** : ni un critère de fenêtre incomplet (il porte déjà l'occultation DWM), ni une évaluation trop précoce (les six fenêtres relevées à 15 ms ne cessent JAMAIS de passer), ni une fuite de sorties (10 créées / 10 servies, **zéro** fuite en régime — les 146 non détruites du journal sont le sillage d'arrêts brutaux). **La cause est le NOMBRE de fenêtres adoptées** : dix, pour un vivier de dix. ✅ **Règle d'appartenance livrée** (`crate::appartenance`, job object **sans aucune limite**) : armé **18 écartées, 5 sorties, 0 refus** ; désarmé (`APPARTENANCE=0`) **0 écartée, 10 sorties, 7 refus** — et le **témoin** dans le même relevé, les applications du catalogue servies. 🔵 Aussi : le verdict de `purge.rs` cesse de crier à tort (`verdict_purge.rs`, trois cas), et `apps/lancement.rs` est corrigé — **le superviseur EST dans un job**, celui du Planificateur, et ses lancements en héritent.


---

## 🔴 Legs ouverts

**Consolidé au 21 août 2026, depuis les § « Ce que … lègue » et « Ce que … n'établit PAS ».**
Le détail et les pièces de chacun sont dans [`docs/JOURNAL.md`](docs/JOURNAL.md).

### Décisions qui appartiennent au propriétaire du dépôt

Aucune n'est une correction, et aucun chantier ne doit les prendre en douce.

- ✅ ~~🔴 **`403 vm-etrangere` contre `404 vm-inconnue`** — le même service rend
  deux réponses différentes pour la même situation.~~ **TRANCHÉ le 21 août 2026
  en faveur du `404` — ET LE CODE L'APPLIQUAIT DÉJÀ : CE LEGS ÉTAIT PÉRIMÉ, PAS
  OUVERT.** Relevé le jour de la décision : `vm-etrangere` **n'est émis nulle
  part** (`grep -rn 'vm-etrangere' plateforme/src --include='*.ts'` ne rend que
  des commentaires historiques), il **n'est pas un membre du type `Motif`**
  (`plateforme/src/orchestration/refus.ts`), et `CODE_HTTP` mappe
  `'vm-inconnue' → 404`. Les `403` qui subsistent portent sur `jeton-agent` —
  un jeton d'agent employé sur une route d'utilisateur — ce qui est **une autre
  question, et une réponse juste**. ⚠️ **Aucune ligne de code n'a été modifiée
  par cette décision** : elle n'a fait que constater. 🔴 **La leçon vaut plus
  que le legs : un § « Legs ouverts » consolidé À LA MAIN vieillit comme
  n'importe quel relevé daté, et celui-ci affirmait une contradiction que le
  produit avait déjà résolue.**
- 🔴 **L'écho acoustique**, trois voies : ① rassembler la restitution (**défait
  D7**) ; ② une AEC côté agent (**que la spec exclut nommément**) ; ③ le casque,
  **dit au bon moment** — la seule dont le défaut mesuré ait encore besoin, et
  la seule livrable par un mécanisme déjà construit.
- ✅ ~~**Le retrait du legacy** — arrêté, à moitié archivé, jamais retiré.~~
  **TRANCHÉ ET EXÉCUTÉ le 21 août 2026** : voir « Le legacy » plus haut. Ce
  qu'il emporte — les fonctions que personne ne reprend — n'est PAS un legs
  ouvert mais une **régression assumée**, et elle est nommée là-bas.
- ⚠️ **La portée de la règle des 500 lignes** : le § « Portée » ne liste que
  `client/src/`, la commande attrape `client/verify-webrtc.mjs`. **Signalé
  depuis D10, jamais tranché.**
- ⚠️ **`FilterAdministratorToken=1` sur la VM** — sans lui, la porte d'élévation
  de G3 reste **non mesurable**, et un installeur qui exige une élévation
  s'exécute **sans aucune boîte de dialogue**.

### Ce que le chantier `auth-pomerium` laisse dû (21 août 2026)

🔴 **INSCRIT ICI ET NON DANS LE SEUL DOCUMENT DE RÉSULTATS, PARCE QUE CE DÉPÔT
VIENT DE CONSTATER QU'UN LEGS QUI NE VIT QUE DANS UN RELEVÉ DATÉ EST UN LEGS
PERDU** — c'est la leçon du legs `403/404`, déclaré « ouvert » alors que le
produit l'avait résolu, et de six constats de revue disparus avec un rapport
gitignoré.

- 🔴 **LE CRITÈRE ⑦ RESTE NON JOUÉ** — la page, dans un navigateur réel,
  derrière Pomerium — ~~**mais ses deux blocages ne sont PAS de même nature,
  et les confondre est l'erreur à éviter**~~ **CE N'EST PLUS VRAI QUE D'UN
  SEUL DES DEUX, DEPUIS LE 22 AOÛT 2026 :**
  - ① ~~**un défaut de CONCEPTION, déjà appliqué au `config.yaml` réel** : la
    route nue de Pomerium (spec §7.2, commentée « La page et l'API ») vise
    `http://192.168.3.1:8080`, c'est-à-dire la plateforme — **qui ne sert
    aucun fichier statique**, `GET /` y rendant `404 introuvable` (mesuré).
    C'est nginx qui sert la page, et ce bloc le saute. Il reste à CONCEVOIR,
    pas à mesurer : soit router Pomerium vers nginx (…), soit doter la
    plateforme d'un servant statique. Aucune des deux n'est tranchée.~~
    **LEVÉ.** La plateforme sert désormais la page bâtie
    (`plateforme/src/http/page/`, variable `PLATEFORME_PAGE` — voir le
    tableau des variables serveur), donc la route nue de la spec §7.2, qui
    vise `http://192.168.3.1:8080`, vise un backend qui répond
    (`GET /` y rend la page si `PLATEFORME_PAGE` est posée vers `client/dist`
    bâti, ou le `404` d'hier si elle ne l'est pas). Voir spec `auth-pomerium`
    § 7, dont l'annotation du 21 août 2026 est levée à son tour.
  - ② **DEMEURE, seul désormais** : une limite de recette, celle-là
    ordinaire — le flux OAuth Google exige **un humain**, et aucun Chrome
    sans interface ne le franchit. ⚠️ **Le critère ⑦ reste donc NON JOUÉ**,
    et écrire « critère ⑦ levé » serait faux : lever un blocage de
    conception ne joue pas le critère à sa place.
- ~~🔴 **AUCUN FREIN SUR `/auth/moi`, ET C'EST DÉSORMAIS LA SEULE SURFACE
  D'AUTHENTIFICATION** en mode `pomerium`. `securite/frein.ts` n'est consulté
  que par `servirAuth` (`grep -ln 'deps\.frein' plateforme/src/http/routes-*.ts`
  ne rend que `routes-auth.ts`), or ce routeur **se retire** dans ce mode. La
  route **crée une ligne `utilisateur` par courriel distinct**, sans borne :
  qui atteint le port `8080` — dont la VM Windows — fait grossir la table à
  volonté, l'en-tête n'étant vérifié par aucune signature. Non mesuré, et ce
  n'est pas une raison de l'écrire moins fort : c'est une lecture de code,
  elle est dite comme telle.~~
  🔴 **REQUALIFIÉ ET FERMÉ, 22 août 2026.** Ce legs se lisait comme un frein
  manquant, appelant une borne de cadence — **ce n'en était pas un**. C'était
  un **contournement COMPLET de l'authentification** : quiconque atteignait le
  port `8080` — dont la VM Windows — obtenait, par un simple en-tête
  `X-Pomerium-Claim-Email` qu'AUCUNE signature ne vérifiait, un jeton interne
  valide pour l'identité de son choix. Un frein n'aurait borné que la
  **cadence** d'un contournement qui n'a besoin d'aboutir **qu'une fois**. La
  garde qui ferme ce trou est `PLATEFORME_PROXY_DE_CONFIANCE`, désormais
  **obligatoire en mode `pomerium`** (`plateforme/src/config.ts::lireConfig`,
  qui refuse de démarrer sans elle) : `routes-identite.ts::servirIdentite`
  n'accepte l'en-tête que d'un pair dont l'adresse socket figure dans cette
  liste (`pairDeConfiance`), et rend `401 pair-non-de-confiance` **avant même
  de la lire** sinon. Voir sa ligne dans le tableau des variables serveur.
- ~~🔴 **ONZE PILOTES DE RECETTE VISENT UNE RACINE QUE CE CHANTIER A FERMÉE.** Le
  relais a quitté `/` pour `/signal`, et `?signaling=` reste **explicite** —
  il ne reçoit pas le suffixe. Or les pilotes de
  `docs/superpowers/plans/journaux-*/instrument/` posent tous une URL **sans
  chemin** : `accent-a1`, `micro-e3` (le pilote et son `injection-e3.js`),
  `pont-fichiers` f1 à f5, `presse-papier` p1 à p3 — **plus un douzième hors de
  ce répertoire**, `journaux-micro-e2/pilote-recette-e2.mjs`. **Aucun n'a été
  réparé** : c'est un chantier à part.~~ **FERMÉ (lot `legs-sans-vm`)** : onze
  des douze pilotes réparés vers `/signal` (le douzième,
  `injection-e3.js`, n'avait rien à changer — sa valeur vient déjà corrigée de
  son pilote appelant). ⚠️ **Aucun pilote n'est rejoué** : la VM Windows est
  hors périmètre de ce lot, le contrôle joué est `node --check` sur chaque
  fichier modifié. ⚠️ **Le commentaire qui affirmait qu'« aucune recette n'en
  pose » a été corrigé** dans `client/src/adresse-plateforme.ts` : son `grep`
  ne couvrait pas `docs/superpowers/`, où vivent TOUS les pilotes — patron du
  « naufrage du 487 ».
- ~~🔴 **LE CONTRAT DE `SIGNALING_URL` N'EST FIGÉ PAR AUCUN TEST.** La variable
  est **la BASE du service**, jamais l'URL du relais : `url_du_relais` y ajoute
  `/signal`, `url_du_canal` y ajoute `/agent`. **Y écrire `/signal` casserait
  l'enrôlement** (`ws://h:8080/signal/agent`) **sans qu'aucun test ne
  bronche** — le test `le_canal_agent_n_est_pas_affecte` d'`agent/src/
  signaling.rs` passe une base PROPRE, donc n'éprouve pas ce cas, alors que son
  commentaire prétendait le fermer. Le commentaire est corrigé ; **le test
  manquant, lui, reste dû.**~~ **FERMÉ (lot `legs-sans-vm`)** : le test
  `une_base_portant_deja_signal_casse_le_canal_agent` (`agent/src/
  signaling.rs`) joue désormais ce cas et fige le contrat — vérifié VERT sur
  le produit d'aujourd'hui, puis rougi par mutation ciblée d'`url_du_canal`,
  restaurée depuis une copie nommée.

### Ce que le chantier `package-nivuus` laisse dû (30 août 2026)

🔴 **INSCRIT ICI ET NON DANS LE SEUL DOCUMENT DE RÉSULTATS, POUR LA MÊME
RAISON QUE LE CHANTIER `auth-pomerium` — et parce que ce chantier vient de
payer DEUX FOIS le patron « une preuve ne doit jamais vivre dans un rapport
gitignoré » : la revue de sa tâche 9 l'a fait corriger, et quatre lots plus
tard il était rouvert en plus grand.**

- 🔴 **UNE FENÊTRE OUVERTE PLUS DE 30 SECONDES AVANT LA CONNEXION DU
  NAVIGATEUR EST PERDUE, DÉFINITIVEMENT, ET RIEN NE LA REPROPOSE.** C'est un
  défaut du **PRODUIT**, pas du package, établi par capture réseau et
  messages du protocole décodés (lot 10D). `agent/src/superviseur/table/
  orphelines.rs::relancer_les_orphelines`, second garde-fou : toute entrée en
  `AttendLeViewport` depuis plus de `DELAI_ATTENTE_VIEWPORT_MAX`
  (`agent/src/superviseur/table.rs`, **30 s**) est retirée et refusée
  (« la page-shell n'a jamais répondu après la relance »). Le commentaire
  d'`orphelines.rs` le dit lui-même : **« une entrée abandonnée n'est JAMAIS
  reproposée, le hook ne réémettant rien pour une fenêtre déjà ouverte »** —
  la seule façon de la revoir est de fermer puis rouvrir la fenêtre Windows.
  Et **rien ne bufferise côté plateforme** : `plateforme/src/signaling/
  appariement.ts` ne conserve aucun état pour `fenetre-ouverte`/`refus`
  (seule l'offre SDP l'est), un message dont le pair n'est pas connecté est
  simplement **non relayé, sans mise en file**. 🔴 **C'est exactement le mode
  d'usage réel derrière Pomerium** — l'OAuth prend du temps — et c'est le
  symptôme que le propriétaire a rapporté (« aucune fenêtre disponible »).
  **NON CORRIGÉ, à dessein** : le corriger est un changement de conception
  (rejeu à la connexion, ou file côté plateforme, ou relance du hook) qui
  mérite sa propre tâche. Détail et mesures : document de résultats, § 11.
- 🔴 **UNE APPLICATION DU WINDOWS STORE NE SERAIT PAS ADOPTÉE** (lot 32I,
  30 août 2026). Depuis la règle d'appartenance, `desk` n'adopte que les
  fenêtres des processus qu'il a lancés, reconnus par un **job object sans
  aucune limite** (`crate::appartenance`). Or une application du Store paraît
  sous `ApplicationFrameHost.exe`, **qui ne descend pas de nous** — mesuré :
  `ApplicationFrameHost.exe ← svchost ← services ← wininit`.
  🔵 **Ce N'EST PAS une panne muette, et c'est ce qui rend ce legs
  acceptable** : `superviseur::hook::refusee_pour_appartenance` journalise
  chaque refus en nommant **la fenêtre, son processus et la raison**, et dit
  comment désarmer (`APPARTENANCE=0`).
  ⚠️ **Aucune application du catalogue n'est dans ce cas aujourd'hui** : ses
  41 entrées sont des raccourcis Win32, et `Calculator` est servi par
  `win32calc.exe`, la version héritée, enfant direct de l'agent. **Le risque
  est repoussé, pas supprimé.**
  ⚠️ **La contrepartie, assumée et non une régression** : une fenêtre **déjà
  ouverte avant `desk`** n'est plus reprise — `cmd.exe` et `Forza Horizon 6`
  quittent le hub. Décision du propriétaire, prise en connaissance de cause.
- 🔴 **`AGENT_VM`/`AGENT_SECRET` NE SONT POUSSÉS PAR RIEN DANS LA VM.**
  ⚠️ **REQUALIFIÉ le 30 août 2026 (lot 32C), PAS FERMÉ.** Sur la machine de
  production, le couple **atteint bien l'agent** — mesuré sur le processus
  vivant, trace d'enrôlement présente et avertissement d'absence à zéro : il y
  a été posé **à la main** dans `C:\nivuus\agent\run-agent.ps1`. **L'asset
  livré par `console`** (`console/guest/provision/assets/run-agent.ps1`) **ne
  les pose toujours pas** : une installation NEUVE retombe dans le legs. Ce
  qu'il faudrait y écrire : `$env:AGENT_VM` et `$env:AGENT_SECRET`, alimentés
  par ce que `desk activate` écrit déjà dans `desk.env`, **AVANT** la ligne
  `& 'C:\nivuus\agent\agent.exe'` — l'ordre est ce qui compte.
  ⚠️ **Et `main.rs` ne REFUSE pas** : `SourceIdentite::Aucune` émet un `warn!`
  et continue sans canal. L'agent démarre quand même ; seul le journal le dit.
  `agent/src/configuration.rs` les lit, `main.rs` refuse explicitement sans
  eux, et `run-agent.ps1` de `console` ne pose que `SIGNALING_URL`,
  `LOCAL_IP` et `RUST_LOG`. Le hook `activate` les écrit dans `desk.env`
  **côté hôte**. Sans ce couple, **aucune session ne peut s'établir**, quelle
  que soit la qualité de l'installation. Vérifié encore le 30 août 2026.
- ⚠️ **UN SERVICE `desk` TOURNE EN PRODUCTION SUR CETTE MACHINE**
  (`192.168.3.1:3445`, derrière `https://app.allanic.me`, mode `pomerium`,
  copie déployée sous `/opt/nivuus/desk`) — **ce n'est pas le dépôt qui le
  sert**. Tout redéploiement par `rsync -a` doit refaire un
  `chmod -R a+rX` : `DynamicUser=yes` fait tourner le service sous un UID
  éphémère, et des droits trop restrictifs lui rendent la page illisible
  (incident réel, page blanche d'une minute).
- ⚠️ **LA RACINE `/` SERT LA PAGE DE SESSION**, qui se rabat sur la session
  `demo` sans jeton : un utilisateur qui tape l'adresse du service tombe
  mécaniquement sur la seule page qui ne peut pas marcher. **Décision non
  prise** (servir le hub, ou rediriger) — elle appartient au propriétaire.
- ⚠️ **`coturn` EST POSÉ, JAMAIS ARMÉ** : `/etc/turnserver.conf` est écrit,
  `/etc/default/coturn` ne l'est pas, et **rien n'écoute sur le port 3478**
  alors que le service annonce `TURN_URL=turn:90.87.35.18:3478` à ses
  clients.
- 🔴 **UN REDÉMARRAGE DE L'AGENT ORPHELINE TOUTES LES FENÊTRES OUVERTES, ET
  RIEN NE LES RÉCUPÈRE.** Conséquence d'exploitation de la règle
  d'appartenance du lot 32I, mesurée deux fois le 31 août 2026 : après toute
  relance, chaque fenêtre préexistante est refusée (`fenêtre ÉCARTÉE : desk ne
  l'a pas lancée`), le hub est vide, et le propriétaire ne retrouve rien tant
  qu'il n'a pas **relancé** ses applications depuis le hub. Le job
  d'appartenance ne survit pas au processus qui le crée, et **rien ne persiste
  l'ensemble des PID adoptés**. ⚠️ **NON CORRIGÉ, à dessein** : persister les
  PID, ré-adopter au démarrage ou adopter par ascendance sont un **changement
  de conception**, qui appartient au propriétaire. Se cumule avec le legs
  « une fenêtre ouverte plus de 30 secondes avant la connexion du navigateur
  est perdue », ci-dessus.
- 🔴 **LA MESURE DU CURSEUR DU LOT 32T RESTE DUE, ROUGE COMME VERTE.** Le
  remède E1 est écrit, éprouvé sur l'hôte, **vu rouge** (432 px à l'assertion
  attendue) et **déployé** (`12040BEAAE905B3D…`, session 1 attestée) — mais
  **aucune session ne l'a encore exercé** : sa trace n'apparaît **0** fois dans
  `agent.log`, `InputInjector::new` n'étant atteint qu'à l'établissement d'une
  session WebRTC réelle. Le pilote a été refusé par le produit lui-même
  (« Bureau refusé : un client est déjà connecté »), le rôle `client` étant
  exclusif par session. **Il faut un créneau où le propriétaire est
  déconnecté** — ou son propre jugement, la correction étant en place.
- 🔴 **AUCUNE INSTALLATION N'A JAMAIS ÉTÉ JOUÉE PAR LE MOTEUR RÉEL.** La
  Critique de la revue finale — un `resolve` qui refusait toujours — a
  survécu à huit suites vertes et neuf revues **pour cette seule raison**.
  Tant que `run.py` n'a pas appelé ces hooks pour de vrai, la même classe de
  défaut reste possible.
- 🔴 **`SORTIE_DESIGNEE` N'ATTEINT PAS L'AGENT DE L'APPLIANCE** (lot 32,
  30 août 2026). La variable est posée par `scripts/run-agent.sh`, ce
  qu'exige la règle du dépôt — mais ce script vise la VM de développement
  **qui n'existe plus sous cette forme**. L'agent réel est lancé par la tâche
  planifiée `guacamole-agent`, qui exécute **`C:\nivuus\agent\run-agent.ps1`**,
  un fichier du package **`console`** ne posant que `SIGNALING_URL`,
  `LOCAL_IP`, `RUST_LOG`, `AGENT_VM`, `AGENT_SECRET` et `SUPERVISEUR`.
  🔴 **CE QUE CELA EMPÊCHE : le bras ROUGE du remède du lot 32 n'est PAS
  rejouable sur l'agent réel de l'appliance** — seulement sur celui que
  `scripts/run-agent.sh` lance, ou par une injection à la main dans le `.ps1`
  de `console` (ce que la recette du lot 32 a dû faire). ⚠️ **Corriger cela
  touche un AUTRE package** : hors périmètre du lot 32, la décision
  appartient au propriétaire. **Consigné ici et non dans le seul relevé daté,
  parce que ce dépôt a constaté deux fois qu'un legs qui ne vit que là est un
  legs perdu.**

### Ce qu'aucun chantier n'a jamais mesuré

- 🔴 **LA LATENCE DE BOUT EN BOUT — jamais, par aucun sous-bloc, depuis D1.**
- 🔴 **AUCUNE CONSTANTE N'EST CALIBRÉE, ET AUCUN JUGEMENT D'USAGE N'A ÉTÉ
  PORTÉ** — ni visuel, ni d'écoute. La liste est longue et ouverte : `BPP_MIN`
  et le `fps` de `Config` (qui se recalibrent **ensemble**), `FACTEUR_FOCUS`,
  `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
  `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`, les cinq du micro, les quatre du
  pont, le plafond de poids CSS, les paramètres `scrypt`, `SEUIL_INJOIGNABLE_MS`
  et `PERIODE_BATTEMENT` (⚠️ qui se recalibrent ensemble **et vivent dans deux
  dépôts distincts**).
- 🔴 **AUCUN JUGEMENT VISUEL N'A ÉTÉ PORTÉ SUR ⑥, D'UN BOUT À L'AUTRE** : aucune
  page n'a été ouverte dans un navigateur par un humain, de S1 à S4. **Vingt-cinq
  jugements humains attendent un œil**, et la galerie existe pour cela.
- 🔴 **LA CONNEXION DE CONTRÔLE DU SUPERVISEUR NE SE RECONNECTE JAMAIS**
  (trouvé le 30 août 2026 en mesurant le lot 17, **non corrigé**). Après un
  redémarrage du service `desk-plateforme`, le superviseur journalise
  `émission vers la shell échouée erreur=Trying to work with closed connection`
  et **reste ainsi** : le pont fichiers se relance
  (`boucle/surveillance_pont.rs`), le superviseur non
  (`superviseur/signalisation.rs` se contente de journaliser « connexion de
  contrôle au signaling perdue »). ⚠️ **Tant que ce socket est mort, AUCUNE
  fenêtre ne peut être annoncée ni réannoncée, et la correction du lot 17 est
  inopérante** — le seul remède connu est de relancer l'agent. Panne muette
  d'exploitation, à traiter dans son propre lot.
- 🔴 **LE CHEMIN D'EXTINCTION PROPRE DU SUPERVISEUR N'A JAMAIS ÉTÉ EXERCÉ**,
  depuis D1 — chaque recette se termine par un `Stop-Process -Force`. C'est ce
  qui laisse des sorties virtuelles et des racines ProjFS orphelines.
- 🔴 **`showDirectoryPicker()` N'EST JAMAIS APPELÉ**, ni le modèle de
  permission, ni le mode `readwrite` : l'instrument est OPFS, et **aucune
  commande CDP n'existe pour ACCEPTER un sélecteur de fichiers**. La parade
  nommée est `Xvfb` + `xdotool`, **dont le consentement a été donné en D8 et
  jamais suivi d'effet** — ⚠️ et les mesures qui en sortiraient ne se
  compareraient à aucune campagne antérieure.

### Les trois couches inconnues du chantier D

Contournées, jamais expliquées : **le plafond de 8 encodeurs** (et rien au-delà
de 720p), **le plafond de 4 processus** tenant une duplication, et **le
mécanisme de l'abandon du mutex DXGI**. ⚠️ **Rien ne nettoie le registre**, dont
la pollution fait naître une sortie à la mauvaise taille — **par GUID**, tranché
par D11, et **toujours le quatrième**, inexpliqué.

### Par sous-projet

| Sous-projet | Ce qui reste dû |
| --- | --- |
| **D** multi-fenêtres | l'A/B sur `set_desired_bitrate` (**écarté par décision**, condition de réouverture : charge d'hôte **contrôlée**, ≥ 8 paires) ; le maillon fautif du `Resize` **non identifié** ; **six constats de revue PERDUS** avec un rapport gitignoré ; aucune **cause naturelle** de mort de capture audio |
| **E** microphone | 🔴 **personne n'a écouté** — le critère de fin n'est atteint que par un juge logiciel ; le son d'une **autre application** n'est pas annulé (−13 dB, donc **amplifié**) et rien ne le dit à l'utilisateur ; le rééchantillonnage 48 → 44,1 kHz du câble, **remède inapplicable** ; deux replis livrés et **jamais courus** ; la licence VB-Audio est **personnelle seulement** |
| **③** pont fichiers | 🔴 **~33 Kio/s**, et **aucun fichier de plus de 128 Kio n'est lisible** ; 🔴 **aucun listage de plus de ~3 150 entrées n'aboutit** (taille d'un message SCTP) ; 🔴 **l'idiome « temporaire + renommage » n'a jamais été exercé sur un éditeur réel** — *le seul chemin par lequel une sauvegarde peut se perdre en silence* ; un renommage fait **disparaître un répertoire frère** ; aucune éviction, le disque grossit |
| **④** gestion d'apps | ~~🔴 **un `<img src>` ne porte pas d'`Authorization`**~~ **FERMÉ (lot 16, 30 août 2026)** : sur décision du propriétaire du dépôt, `GET /application/:id/icone` s'atteint désormais par une **URL SIGNÉE** — sous-clé HKDF dérivée du secret de jeton, signature couvrant l'application, la VM et l'expiration, durée de 5 à 6 minutes, frappée par le seul `GET /applications` (donc sous jeton porteur, après le contrôle d'appartenance de la VM), et **l'ancien chemin à `Authorization` est RETIRÉ** — `plateforme/src/apps/url-icone.ts`. 🔴 **CE QUI RESTE OUVERT, ET CE N'EST PAS LA MÊME CHOSE : le hub n'est toujours PAS ÉTABLI installable**, pour deux raisons distinctes — ① aucun navigateur ne l'a jugé (il faut un humain, ou un Chromium piloté) ; ② `/hub.webmanifest` ne déclare **aucun** `icons`, et **une URL signée ne peut pas la lui donner** : ce manifeste est statique, engendré au build, et une URL qui expire en cinq minutes n'y a pas sa place — il appelle une icône **statique**, donc un choix d'image qui appartient au propriétaire. ⚠️ Et une URL signée placée dans un `icons[].src` **ne serait pas atteignable derrière Pomerium** : un manifeste est allé chercher SANS cookie, et cette URL ne finit ni par `.png`, ni par `.ico`, ni par `manifest.json` — les trois seuls suffixes que la politique laisse passer. Ses `file_handlers` restent donc inertes ; 71 applications restent `NonMesuree` ; une icône qui change **sans que le raccourci change** n'est jamais revue ; ~~le magasin n'est **jamais nettoyé**~~ **FERMÉ (lot `legs-sans-vm`)** : `apps/nettoyage.ts` câble une éviction par âge sur les deux magasins (icônes, tranches), avec un plancher de référence contre la corruption, câblée sur la base réelle au démarrage du service — voir le document de résultats du lot pour les legs qu'elle laisse (course stat→rm, fenêtre d'obsolescence de l'instantané d'icônes) |
| **⑤** plateforme | 🔴 **la scalabilité horizontale est IMPOSSIBLE** — **quatre** états de routage vivent en mémoire, et `--scale plateforme=2` donne une panne **muette** que rien n'empêche ; 🔴 **TURNS sur 443 n'est pas livré**, donc **la cible « réseaux restrictifs » n'est pas couverte** ; ~~`GET /vm` et `POST /session` ne sont **pas freinées** ; le relais de signaling — `/signal` depuis le chantier `auth-pomerium` (il vivait à la racine `/` avant) — non plus~~ **FERMÉ (lot `legs-sans-vm`)** : un budget de VOLUME (`securite/frein.ts::BUDGET_REQUETES`), distinct du budget d'échecs, protège désormais `GET /vm`, `POST /session` et la connexion `/signal` ; le jeton vit dans `localStorage` (**aucun cookie livré**) ; le secret d'enrôlement est **en clair sur la VM** (rotation possible, retrait non) |
| **⑥** design system | le **WCO** n'a jamais été rendu — **écarté du lot `legs-sans-vm` par décision : inatteignable tant que le hub n'est pas installable**, une décision de sécurité qui appartient au propriétaire ; ~~`--accent-fenetre` **n'est déclaré nulle part et peint par rien**~~ **FERMÉ (lot `legs-sans-vm`)** : déclaré dans `tokens/couleurs.css`, peint sur `#remote` par `style.css`, et le garde des orphelins (§7.6) reconnaît désormais `poserToken(...)` en plus d'un `var(--…)` CSS ; `client/src/design/tokens.css` **n'est plus à sa porte** — scindé en `tokens/couleurs.css` et `tokens/echelles.css` le 25 août 2026, chacun avec sa propre porte armée et son découpage suivant nommé (voir leur en-tête). 🔴 **AUCUNE TAILLE N'EST RECOPIÉE ICI, ET C'EST DÉLIBÉRÉ** : la première rédaction de cette ligne portait trois nombres, et le round qui les a écrits les a rendus faux **dans le même round** — 45/191/149 annoncés, 64/201/167 mesurés. `wc -l` sur les trois fichiers — avec **treize** lecteurs qui le nommaient par son chemin AVANT ce découpage (« onze » et « 33 » étaient tous deux vrais de choses différentes, faute de règle énoncée ; chaque lecteur explique désormais son choix dans son propre en-tête) ; ~~les galeries n'ont **aucun test**~~ **FERMÉ (lot `legs-sans-vm`)** : `galerie.test.ts` et `galerie-primitives.test.ts` figent la LISTE des cas (tokens, primitives balisées) — **aucun jugement visuel**, ce que ces deux fichiers disent eux-mêmes ne pas établir |
| **①** divers | le **propriétaire mono-fenêtre n'existe pas** (ni presse-papier, ni accent) ; ~~le canal `Message` du registre est **non borné**, et trois sous-blocs l'ont aggravé~~ **FERMÉ (lot `legs-sans-vm`)** : une file bornée par coalescence par variante (`capteur/sommeil/file.rs`), branchée sur les cinq points d'appel de production, avec trace au palier ; ce lot n'a rien exercé en charge réelle — voir le document de résultats ; le **niveau 2** du presse-papier (qu'un humain puisse coller) reste **non mesurable** |

---

## 📌 Tenir ce fichier

**Ce fichier a déjà dérivé une fois, jusqu'à peser 87,5 % d'archive.** Trois
gestes suffisent à l'en empêcher :

1. **Un chantier qui se clôt ajoute UNE LIGNE à l'index** et son récit à
   `docs/JOURNAL.md` — jamais l'inverse.
2. **Un relevé daté n'a pas sa place ici.** S'il porte une date, il va au
   journal ; s'il porte une règle, il reste.
3. **Un piège payé DEUX FOIS remonte ici**, condensé en une ligne, depuis le
   § « Pièges neufs » de son chantier.

⚠️ **`CLAUDE.md` est explicitement exempté de la règle des 500 lignes** — c'est
un index de connaissances, pas du code. **Cette exemption n'est pas un permis
d'archiver.**
