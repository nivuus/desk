# `desk` devient un package Nivuus — résultats

> 🔴 **CE DOCUMENT A DÉCRIT, JUSQU'AU 30 AOÛT 2026, UNE BRANCHE QUI S'ÉTAIT
> ARRÊTÉE À LA TÂCHE 9 — alors qu'elle portait déjà six commits de travail
> réel de plus.** La revue finale de branche a relevé **douze affirmations
> devenues fausses** dans les pages qui suivent, et le fait que toute la
> preuve des lots 10A à 13 ne vivait que dans des rapports **gitignorés** —
> le patron exact que ce dépôt nomme « une preuve ne doit jamais vivre dans
> un rapport gitignoré », et que **la revue de la tâche 9 de cette branche
> même avait déjà fait corriger une fois**. Les douze affirmations sont
> corrigées sur place, chacune remesurée ; les lots 10A à 13 et la revue
> finale ont leurs sections **§10, §11 et §12**, en pied de document.
>
> ⚠️ **Chaque chiffre de ce document a été REMESURÉ le 30 août 2026, à
> `ae8a1fb`.** Un chiffre daté vieillit : relancer la commande, ne jamais le
> recopier.

**29 août 2026, complété le 30 août 2026.** Chantier `package-nivuus`.
Tâches 1 à 9 : commits `2cc9949` (exclu, c'est le plan) à `ed0e53f` inclus —
**dix-sept commits**, mesuré `git rev-list --count 2cc9949..ed0e53f`.
Lots 10A à 13 et revue finale : §10 à §12. **La branche entière** porte
**42 commits** depuis `084bc6c` (mesuré le 30 août 2026 à `ae8a1fb` ;
elle en portait 35 quand la revue finale a commencé). Spec :
`docs/superpowers/specs/2026-08-29-package-nivuus-design.md`. Journal de
bord complet, avec chaque ronde de revue et chaque ruling du contrôleur :
`.superpowers/sdd/2026-08-29-package-nivuus/progress.md`.

Ce document est la revue **transverse** de fin de lot : ce qu'aucune revue
par tâche ne pouvait voir, parce que chaque tâche était correcte prise
séparément (huit tâches, huit revues, une ronde de correction chacune sauf
la tâche 2 et la tâche 7, revues propres du premier coup).

---

## 1. Ce que le lot livre

Le trou qu'il ferme, nommé dans la spec : `console/guest/payload.py`
déclarait l'agent Windows « extracted before the wipe » et
`fetch_payload.py` écrivait « Not fetched, and never fetchable » — la VM
cible était devenue une **appliance** provisionnée par `packages/installer`
(retrait délibéré de `C:\dev`, de Rust et du montage CIFS), et plus personne
ne savait refabriquer le binaire autour duquel elle se reconstruit.

| Tâche | Ce qu'elle ajoute | Commits |
| --- | --- | --- |
| 1 | `scripts/build-agent-croise.sh` — compile l'agent en croisé (`x86_64-pc-windows-gnu`), sans VM ni CIFS, et le dépose à une destination donnée | `1dc6117`, `b63ea1e` |
| 2 | `nivuus-package.yaml` (tier userspace, `requires.packages: [console]`) et `wizard.yaml` (quatre questions) | `9be0d58` |
| 3 | `hooks/resolve.py` — refuse AVANT que le disque soit touché ; émet les faits | `fa5dc03`, `175ae64`, `e1d07a7` |

⚠️ **Deux affirmations de la ligne « tâche 3 » ci-dessus ont été retirées
plutôt que laissées vieillir** (revue finale, 30 août 2026) : ① le refus de
`pomerium` « sans garde de confiance possible » est **levé** depuis le lot
10A — le proxy de confiance est désormais dérivé, pas demandé ; ② le refus
« VM injoignable » **n'existe plus du tout** — c'était la Critique de la
revue finale, voir §12.1. Les faits émis ne sont pas cinq mais **sept**
(`vm_repond`, `node_version`, `turn_ecoute`, `turn_relais`, `hote`,
`proxy_confiance`, `port`) : `hote` et `proxy_confiance` sont entrés au lot
10A. Le compte se relit dans le code, il ne se recopie pas —
`tests/test_desk_contrat_hw.py` le mesure à chaque exécution.
| 4 | `hooks/install.py` — pose `desk.env` (600), `/opt/nivuus/desk/{plateforme,client/dist}`, l'unité systemd (posée, pas armée), `turnserver.conf` (posé, pas armé) | `b6a0c92`, `92cacfb` |
| 5 | `hooks/activate.py` — arme l'unité par un LIEN, crée le compte admin et enrôle l'agent (mot de passe et secret jamais sur l'argv), idempotent | `911c910`, `cc1da83` |
| 6 | `hooks/vm.py` — pose ProjFS et VB-Audio dans la VM par le chemin WinRM de `console` ; `vb_audio: true` refusé dans `resolve` avant tout octet écrit | `a7cffb4`, `ca03468` |
| 7 | dépose `agent.exe` là où `console` va le chercher (`<NIVUUS_PACKAGES_DIR>/console/guest/payload/agent/agent.exe`) ; deux extractions préalables (`administration.py`, `env_fichier.py`) pour rester sous 500 lignes | `c774d02`, `b1d27da`, `4fa540c` |
| 8 | `Makefile` (`make test`, découverte par préfixe), `README-package.md` (packaging ≠ produit) | `f1bbaf8`, `ed0e53f` |

État final, rejoué le 30 août 2026 à `ae8a1fb` : `make test` — **10 suites**,
exit 0 (`ls tests/test_desk_*.py | wc -l` → 10). ⚠️ **Ce chiffre a été faux
deux fois** : « 7 » ici alors que le lot 10A en comptait déjà 8, et la
correction du lot 10A ne vivait que dans un rapport gitignoré. Le naufrage
du « 487 », exactement.
`env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh` — **10/10 étapes
vertes**, 65 fichiers de test plateforme, **717 tests**, tous passés
(rejoué par moi indépendamment pour cette revue, même compte exact que le
commit `ed0e53f` l'annonce).

---

## 2. Relecture de chaque message de commit contre son diff

**Dix-sept commits relus, un par un**, `git show --stat` et le diff complet
en regard du message. Méthode : pour chaque affirmation vérifiable du
message (nombre de lignes, chemin de fichier, numéro de ligne cité dans un
dépôt voisin, comportement d'une fonction), la commande qui l'établit a été
rejouée — jamais recopiée d'un rapport de tâche.

**Vérifications indépendantes effectuées** (au-delà du simple diff) :

- `1dc6117`, `9be0d58`, `fa5dc03`, `175ae64`, `e1d07a7`, `b6a0c92`, `92cacfb`,
  `911c910`, `cc1da83`, `a7cffb4`, `ca03468`, `c774d02`, `b1d27da`, `4fa540c`,
  `f1bbaf8`, `ed0e53f` : stat + message relus. `b63ea1e` idem.
- `c774d02`/`b1d27da`/`4fa540c` : les transitions de taille annoncées pour
  `hooks/activate.py` (479→420, 420→374, 374→453) **rejouées par
  `git show <commit>^:hooks/activate.py | wc -l` et `git show
  <commit>:hooks/activate.py | wc -l`** — les six nombres correspondent
  exactement.
- `fa5dc03` : la citation de `plateforme/src/config.ts::lireConfig` («
  refuse de démarrer sans `PLATEFORME_PROXY_DE_CONFIANCE` » en mode
  `pomerium`) et la borne `node_version` « RELUE dans
  `plateforme/package.json::engines.node` » — les deux confirmées dans les
  fichiers cités (`config.ts:302-308`, `package.json:6-8`,
  `">=24.0.0 <25.0.0"`).
- `4fa540c` : les trois citations de fichiers du dépôt voisin `installer`
  (`fetch_payload.py:61` → `PACKAGED_AGENT_EXE`, `discovery.py:22` →
  `PACKAGES_DIR = os.environ.get("NIVUUS_PACKAGES_DIR", …)`,
  `activate_cli.py:107-108` → `run_activate(match[0], detected,
  state[name].get("answers") or {}, …)`) — **les trois lues au numéro de
  ligne exact cité**, dans `installer/console/guest/fetch_payload.py` et
  `installer/installer/packages/{discovery,activate_cli}.py`.
- `a7cffb4` : la citation `guest-ready-watch.py:113` (`WINRM_EXEC =
  "/opt/nivuus-packages/console/guest/winrm_exec.py"`) — confirmée au
  numéro de ligne exact.
- `b6a0c92`/`92cacfb` : la garantie `run_install(manifest, hw, answers,
  root)` **sans** `facts`, et `run_activate(…, facts=facts)` qui les
  fusionne via `merge_into_hw` — confirmée en lisant
  `installer/installer/packages/runner.py:315-338` intégralement.
- `92cacfb` : `DynamicUser=yes` / `StateDirectory=nivuus-desk` /
  `PLATEFORME_ICONES`+`PLATEFORME_TELEVERSEMENTS` vers
  `/var/lib/nivuus-desk/{icones,televersements}` — confirmés dans
  `hooks/assets/desk-plateforme.service` et `hooks/install.py` (nommé,
  jamais numéroté).
  `os.open(…, 0o600)` atomique pour `desk.env` et `turnserver.conf` —
  confirmé, deux occurrences (`os.open(..., 0o600)` dans
  `hooks/fichiers_installes.py::ecrire_env` et `::ecrire_turnserver_conf`).
- `ca03468`/`ed0e53f` : la doctrine « refuse dans `resolve`, avant tout
  octet écrit » pour `vb_audio: true` — confirmée
  (`hooks/resolve.py::valider_vb_audio`, et son appel dans `resoudre()`).
- **`ed0e53f` : l'empreinte `fc66b5649cf2782e` recalculée indépendamment**,
  en rejouant l'algorithme exact du détecteur (regex d'affectation, extraction
  de valeur par `valeurApres`, SHA-256 tronqué à 16 hex) sur la ligne 59 de
  `tests/desk_activate_fixtures.py` telle qu'elle vit sur le disque — **même
  résultat, à l'octet près** (script Node ad hoc, non versionné). L'annonce
  finale du commit (« 65 fichiers, 717 tests, tous passés ») a été **rejouée
  en entier** pour cette revue, pas relue seulement : même compte exact.
- `92cacfb` : l'extraction de `hooks/commun.py` (`interface_de_route_par_defaut`,
  `adresse_ipv4_de`, `PORT_DEFAUT`) — le fichier existe, 64 lignes, importé
  par les deux hooks, aucune duplication résiduelle trouvée par grep.

**Écart trouvé** : **un seul**, mineur, purement numérique.

> `b6a0c92` affirme « `hooks/install.py` (314 lignes) » dans son propre
> corps de message. Mesuré à ce commit
> (`git show b6a0c92:hooks/install.py | wc -l`) : **312 lignes**, exactement
> ce que `git show --stat` annonçait déjà (`312 +++`). Écart de 2 lignes.
> Ce même écart avait déjà été repéré par le contrôleur pendant la ronde de
> revue de la tâche 4, mais **attribué au rapport de tâche** (« minor
> deferred: rapport inexact (314 l./750 annonces, 312/0755 mesures) — à
> corriger dans le rapport, pas dans le code ») — l'écart existe
> **identiquement dans le message de commit lui-même**, une pièce du dépôt
> que personne ne relit ensuite. Aucune conséquence fonctionnelle : la
> phrase suivante du plan ne s'appuie sur aucun de ces deux nombres. Nommé
> ici plutôt que corrigé — corriger un message de commit après coup en
> réécrirait l'historique, ce que ce chantier ne fait jamais.

Aucun autre écart trouvé sur les dix-sept commits. Les affirmations
comportementales non directement rejouables (par ex. « 9 échecs » de
`e1d07a7`, « 8 assertions » de `4fa540c`, les mutations ciblées « restaurées
depuis une copie nommée ») sont cohérentes avec la doctrine TDD que ce
chantier applique partout et avec les tests actuellement présents dans le
dépôt ; elles décrivent des exécutions ponctuelles d'hier, non rejouables à
l'identique (l'état du hook a changé depuis), et n'ont pas été mises en
doute faute de pouvoir les rejouer sans réintroduire le bug corrigé.

---

## 3. Tailles des fichiers, après la dernière édition de la ronde

Commande de `CLAUDE.md`, relancée le 30 août 2026 à `ae8a1fb` :

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Résultat :

```
   1536 agent/src/encode.rs
    630 agent/src/windows_source.rs
```

**Aucun fichier neuf de ce lot ne franchit 500 lignes.** Les deux seules
lignes qui dépassent sont la dette déjà gelée et documentée dans le tableau
de dette de `CLAUDE.md` (`encode.rs`, `windows_source.rs`), inchangée par ce
chantier.

Tailles des fichiers produits par ce lot — **remesurées une par une le
30 août 2026 à `ae8a1fb`, après la dernière édition**, jamais recopiées.

> 🔴 **ONZE DES VINGT-ET-UNE LIGNES DE LA TABLE PRÉCÉDENTE ÉTAIENT FAUSSES,
> ET TROIS FICHIERS Y MANQUAIENT** (revue finale de branche). Exemples
> mesurés ce jour-là : `install.py` annoncé 308, mesuré 454 ;
> `test_desk_install.py` annoncé 233, mesuré 465 ; `commun.py` annoncé 64,
> mesuré 240. La table ci-dessous compte **29 fichiers**, et chaque nombre
> vient d'un `wc -l` joué à l'instant où la ligne a été écrite.

| Fichier | Lignes |
| --- | --- |
| `scripts/build-agent-croise.sh` | 48 |
| `nivuus-package.yaml` | 32 |
| `wizard.yaml` | 32 |
| `hooks/resolve.py` | 437 |
| `hooks/install.py` | 441 |
| `hooks/activate.py` | 470 |
| `hooks/vm.py` | 210 |
| `hooks/commun.py` | 380 |
| `hooks/administration.py` | 130 |
| `hooks/env_fichier.py` | 59 |
| `hooks/depot_arbre.py` | 98 |
| `hooks/agent_payload.py` | 99 |
| `hooks/fichiers_installes.py` | 116 |
| `hooks/depot_node.py` | 142 |
| `hooks/assets/desk-plateforme.service` | 71 |
| `tests/test_desk_build_croise.py` | 89 |
| `tests/test_desk_manifeste.py` | 64 |
| `tests/test_desk_resolve.py` | 270 |
| `tests/test_desk_install.py` | 364 |
| `tests/test_desk_install_gardes.py` | 206 |
| `tests/test_desk_activate.py` | 478 |
| `tests/test_desk_vm.py` | 266 |
| `tests/test_desk_payload.py` | 262 |
| `tests/test_desk_administration.py` | 106 |
| `tests/test_desk_contrat_hw.py` | 190 |
| `tests/desk_activate_fixtures.py` | 251 |
| `tests/desk_install_fixtures.py` | 152 |
| `Makefile` | 37 |
| `README-package.md` | 118 |

**Aucun fichier de ce lot ne franchit 500 lignes**, relevé par la commande de
`CLAUDE.md` ci-dessus au même instant : seules `agent/src/encode.rs` (1536)
et `agent/src/windows_source.rs` (630) dépassent, la dette gelée du produit,
inchangée par ce chantier.

⚠️ **Marges à surveiller** : `tests/test_desk_activate.py` 478/500,
`hooks/activate.py` 470/500. Les deux ont motivé quatre extractions dédiées
au fil de ce chantier ; la prochaine addition dans l'un d'eux en demandera
une cinquième, **avant** l'ajout et jamais après.

---

> ⚠️ **TOUS LES NUMÉROS DE LIGNE DE CE DOCUMENT ONT ÉTÉ REMPLACÉS PAR DES
> NOMS le 30 août 2026.** La revue finale en a relevé **six** devenus faux —
> dont `hooks/activate.py:444`, cité trois fois, dans un fichier qui en
> comptait alors 440. `CLAUDE.md` l'interdit nommément : « NOMMER LA CHOSE,
> jamais compter les lignes qui l'en séparent ».

## 4. Ce que ce lot n'établit PAS

### 4.1 Que l'agent bâti en croisé FONCTIONNE

`scripts/build-agent-croise.sh` compile et lie `agent.exe` — la compilation
prouve que l'agent **se lie**, pas qu'il **tourne**. Le binaire produit est
un produit **mingw** (`x86_64-pc-windows-gnu`, éditeur de liens
`x86_64-w64-mingw32-gcc`), là où l'ancien binaire de production était bâti
**sur la VM, en MSVC** (chaîne `cargo build` invoquée directement dans
Windows, avec `link.exe` de Visual Studio). **Ce binaire n'a jamais tourné
sur la VM Windows.** Le script lui-même le dit dans son propre en-tête
(`scripts/build-agent-croise.sh:9-11`), la tâche 7 le répète dans son
rapport (réserve 3), et la spec (§4.3) nomme le juge qui reste dû : une
exécution réelle sur la VM. Rien dans ce lot n'a changé cet état.

### 4.2 Que l'installation a été jouée de bout en bout, sur une machine neuve

Elle ne l'a pas été. Les rouges de la tâche 4 (« démarrer le service ») et
de la tâche 5 (l'armement par lien, l'idempotence) se jouent **sous une
racine temporaire** (`--root=/tmp/…` ou équivalent, `TMPDIR=/var/tmp` sur
cette machine) et sur des **noms d'unité factices** — jamais sur
`/etc/systemd/system` réel, jamais avec `systemctl daemon-reload` ni
`systemctl start` sur le système hôte. C'est une décision du chantier
(ruling du contrôleur, `progress.md`, avant la tâche 4) : le système de
l'hôte ne devait se toucher qu'à un **lot 10**, séparé, pas couvert par ce
plan à neuf tâches. Conséquence directe, nommée par les deux rapports de
tâche concernés : le chemin `--root=/` (armement réel de l'unité,
`daemon-reload`, démarrage réel du service `npm start`) **n'a aucune
couverture automatisée**, par construction.

🔴 **CORRECTION, 30 AOÛT 2026 — LA SECONDE MOITIÉ DE CE PARAGRAPHE EST
DEVENUE FAUSSE.** Il disait « personne n'a encore observé le package
s'installer, de `resolve` à un service qui répond sur le port 3445 ». C'est
faux depuis le **lot 10A** (29 août 2026) : `install` et `activate` ont été
joués sur `--root /`, sur cette machine, et le service **tourne et sert** —
mesuré, `LISTEN 192.168.3.1:3445`, `200` sur la racine. Voir §10.1.
**Ce qui reste vrai, et c'est la moitié qui compte** : cela n'a **jamais**
été joué sur une machine où rien n'existait avant, ni par le moteur réel
(`run.py`), ni depuis un ISO. La différence n'est pas rhétorique — c'est
précisément parce que le moteur réel n'avait jamais appelé `resolve` que la
Critique du §12.1 a survécu à huit suites vertes et à neuf revues.

### 4.3 Qu'aucun des douze items du lot 3 n'est mesuré

Le lot 3 (la campagne de mesure sur la VM — voir la spec, §1 et §2) est
**suspendu**, pas retiré : il a buté sur une VM qui refusait les
identifiants du dépôt, et ce chantier existe précisément pour lever ce
blocage (produire et déposer `agent.exe`, adapter `desk` à une VM devenue
appliance). Il **rend** la VM atteignable ; il ne **mesure** rien lui-même.
Les douze items de ce lot 3 restent à zéro mesure après ce chantier — la
spec et le plan qui les décrivent restent valides pour le jour où la VM
sera équipée pour les recevoir.

### 4.4 L'ordre d'activation : `console` avant `desk`

`desk` déclare `requires.packages: [console]` (`nivuus-package.yaml`), et
le moteur trie les packages sélectionnés par dépendance avant de les
appliquer (tri de Kahn, `installer/installer/packages/dependencies.py:59-…`,
vérifié en lisant le code) : `console` s'active donc **avant** `desk`, dans
le sens de la dépendance. Conséquence directement nommée par la tâche 7
(réserve 2, reprise ici sur décision du contrôleur) : le dépôt d'`agent.exe`
que la tâche 7 met en place **ne bénéficie qu'aux reconstructions futures**
de la VM (un opérateur qui relance `fetch_payload.py` après coup) — **jamais
au tout premier provisionnement**, qui consomme la source vendorisée déjà
committée dans `console` au moment où l'image d'installation a été bâtie.
Résoudre cet ordre (faire porter le dépôt par `console` lui-même, ou changer
l'ordre de dépendance) est une décision de conception hors du périmètre de
ce plan.

### 4.5 VB-Audio : la question existe, la charge n'existe pas

Le wizard pose la question `vb_audio` (défaut désarmé, licence VB-Audio
personnelle seulement). Depuis la ronde de correction de la tâche 6,
`hooks/resolve.py::valider_vb_audio` **refuse** `vb_audio: true` — avant
qu'un octet touche le disque — en nommant la vraie raison (aucune charge
livrable, licence personnelle). La levée de `hooks/vm.py::poser_vb_audio`
reste en place en défense en profondeur, mais elle n'est plus le premier
rempart. La question elle-même n'a **pas** été retirée du wizard : elle
documente une intention réelle de produit, et son défaut (`false`) ne bloque
rien. **C'est une dette assumée**, nommée dans le ruling du contrôleur
(`progress.md`) et vérifiée ici dans le code
(`hooks/resolve.py::valider_vb_audio`) —
pas un oubli, et pas quelque chose que ce lot corrige plus avant : il faudrait
une charge VB-Audio livrable pour que la question cesse d'être un refus
garanti.

### 4.6 Node sur la cible

> 🔴 **CETTE SECTION EST DEVENUE FAUSSE DEUX FOIS, ET ELLE EST RÉÉCRITE
> PLUTÔT QUE RAPIÉCÉE.** Elle citait `ExecStart=/usr/bin/npm start` comme le
> code courant : ce n'est plus vrai depuis le lot 10A (le fichier porte un
> gabarit `__NODE_BIN__`). Elle concluait qu'« aucune tâche de ce plan ne
> referme ce trou » : ce n'est plus vrai depuis le 30 août 2026.

**Le diagnostic, qui reste entièrement valide** (vérifié dans sa forme la
plus dure le 29 août 2026 : `ls /usr/bin/node /usr/bin/npm
/usr/local/bin/node`, `dpkg -l | grep nodejs`, `type node npm`) : **il
n'existe AUCUN Node à l'échelle du système sur cette machine**, ni pour root
ni pour personne — pas seulement « sous `/root/.nvm`, en mode 700 ». `node`
et `npm` n'y sont que des **fonctions shell** qui sourcent `nvm`. Un
`ExecStart=/usr/bin/npm start` ne pouvait donc pas démarrer, `DynamicUser`
ou non.

**Ce que le lot 10A a fait** : remplacé `/usr/bin/npm` par un gabarit
`__NODE_BIN__` (dans `ExecStart=` **et** dans `Environment=PATH=`, parce que
`npm` est lui-même un script `#!/usr/bin/env node`), substitué à
l'installation vers `commun.py::NODE_BIN_DEFAUT` = `/opt/nivuus/node/bin`.
**Et déposé l'arbre à la main**, par une commande consignée dans un rapport
gitignoré.

**Ce que la revue finale a relevé** : *aucun hook ne déposait quoi que ce
soit à cet emplacement*, et aucun garde ne le disait — là où `install.py`
refuse avec une phrase quand `node_modules/.bin/tsx` manque et quand
`proto/ts/plateforme.ts` manque. Le garde était posé sur deux des trois
conditions de démarrage.

**Ce qui a été tranché le 30 août 2026 : le hook POSE Node.** Refuser
seulement aurait reproduit la Critique du §12.1 — une porte qu'aucune
installation ne peut franchir, puisque rien ne provisionne Node sur la
cible. Poser est possible **sans rien deviner** : `resolve` a déjà validé le
`node` de la machine qui exécute les hooks contre `engines.node` de
`plateforme/package.json`, et c'est celui-là que `install` copie
(`hooks/depot_node.py`) — `bin/node`, `bin/npm`, `bin/npx` et
`lib/node_modules/npm`, la forme exacte relevée sur l'arbre posé à la main,
sans les 237 Mio de paquets globaux sans rapport avec ce dépôt. Si ce
runtime ne peut pas être localisé, le pré-vol **refuse en le nommant**,
avant qu'un secret soit tiré.

🔵 **Effet de bord recherché, qui ferme la mineure #6** : la version que
`resolve` valide devient la version que le service exécute. Les deux
machines n'étaient pas la même — voir §6, mineure #6, dont la justification
était **fausse**.

⚠️ **Ce que cela n'établit toujours pas** : que le service démarre sur une
machine neuve. Le dépôt est éprouvé par `tests/test_desk_install.py`
(installation 7, avec le **vrai** runtime de cette machine : `bin/npm` reste
un lien, `bin/node` est exécutable par autrui, l'unité pointe sur le chemin
réellement déposé) — jamais par un `systemctl start` sur une cible fraîche.

### 4.7 coturn : posée, jamais armée

`hooks/install.py::ecrire_turnserver_conf` écrit `/etc/turnserver.conf`,
mais **ne touche jamais** `/etc/default/coturn` (`TURNSERVER_ENABLED`) : le
fichier de configuration existe, le service ne démarre pas pour autant. Le
contenu de ce fichier est de surcroît **extrapolé** depuis les options déjà
vérifiées de `docker-compose.coturn.yml` (`--listening-ip`, `--relay-ip`,
`--static-auth-secret`, etc.), **jamais vérifié contre un vrai binaire
`coturn`** : `dpkg -l coturn` ne rend rien sur la machine où ce lot a été
écrit. Le code le dit lui-même, au mot près : « POSÉE, PAS ARMÉE » et
« FORMAT NON VÉRIFIÉ SUR CETTE MACHINE »
(`hooks/fichiers_installes.py::ecrire_turnserver_conf` — **nommé, jamais
numéroté** : ce document citait `hooks/install.py:149-166`, et cette
fonction a depuis changé de fichier).
`nivuus-package.yaml` déclare `apt: [coturn]`, donc le paquet Debian sera
installé par le moteur — mais installer le paquet, poser sa configuration et
armer son service sont trois gestes distincts, et seuls les deux premiers
sont couverts.

---

## 5. Défaut transverse trouvé par cette revue — nommé, non corrigé

**🔴 Rien, dans ce lot ni dans le dépôt voisin `console` (tel qu'observé),
ne dépose `AGENT_VM`/`AGENT_SECRET` dans l'environnement du processus
`agent.exe` qui tourne réellement dans la VM Windows.**

C'est le genre de défaut qu'aucune revue par tâche ne pouvait voir, parce
que les deux moitiés sont chacune correctes prises séparément :

- **Côté `desk`** (tâche 5, `hooks/activate.py::main`) : une fois le compte
  admin créé et l'agent enrôlé (`npm run admin:agent`), le hook écrit
  `AGENT_VM` et `AGENT_SECRET` dans **`desk.env`** — le fichier
  d'environnement du **service `plateforme` sur l'hôte Linux**. C'est
  correct et suffisant pour que la plateforme **reconnaisse** un agent qui
  se présenterait avec ce couple.
- **Côté VM Windows** (dépôt voisin `console`, hors périmètre de ce lot,
  observé par la reconnaissance en lecture seule du 29 août 2026,
  `.superpowers/sdd/2026-08-29-package-nivuus/recon-appliance.md`, Q2) : le
  lanceur que `console` dépose et arme dans la VM
  (`console/guest/provision/assets/run-agent.ps1`, copié par
  `console/guest/provision/40-agent.ps1`) ne pose **que trois** variables :
  `SIGNALING_URL`, `LOCAL_IP`, `RUST_LOG`. **Ni `AGENT_VM` ni
  `AGENT_SECRET`.**
- **Côté agent Rust** (`agent/src/plateforme/identite.rs:44-49`) : sans
  `AGENT_JETON` hérité ET sans le couple `AGENT_VM`/`AGENT_SECRET`, la
  source d'identité résout à `SourceIdentite::Aucune`, dont le commentaire
  du code dit, au mot près : *« Ni l'un ni l'autre : aucun jeton, donc
  aucune session ne s'établira. Ce n'est pas un mode de repli, c'est une
  panne annoncée. »*

**Conséquence** : même si `resolve`/`install`/`activate` réussissent
intégralement sur l'hôte Linux (ce que ce lot établit), et même si la VM
Windows est une appliance saine avec `agent.exe` à jour (ce que la tâche 7
établit), **l'agent qui démarre dans la VM ne présentera jamais** le secret
que `desk` vient de créer pour lui — parce que rien ne le lui a transmis. Le
compte admin existe, la ligne `vm` existe dans la base de la plateforme,
mais **aucune session WebRTC ne s'établira jamais** sans une intervention
manuelle qui n'est décrite nulle part dans ce lot (ni dans `console`, pour
autant que cette reconnaissance en lecture seule ait pu l'établir).

**Ce que j'ai vérifié, pas seulement supposé** :
`grep -rn "AGENT_VM\|AGENT_SECRET" hooks/` ne montre que l'écriture dans
`desk.env` (`hooks/activate.py::main`) et sa documentation — aucune fonction
de `hooks/vm.py` (qui, lui, pose ProjFS et VB-Audio dans la VM par le
même chemin WinRM) ne s'occupe de ce couple. Aucun rapport de tâche (1 à 8)
ne nomme ce trou. ⚠️ **Corrigé après une première rédaction de ce
paragraphe** : le contrôleur a lui-même consigné le même constat dans
`progress.md` (« FAIT B », section « Deux faits établis par moi avant le
lot 10 »), en parallèle de cette revue et sans connaissance de son
avancement — trouvaille indépendante, pas reprise l'une de l'autre,
convergente sur les mêmes trois fichiers (`hooks/activate.py`,
`console/guest/provision/assets/run-agent.ps1`,
`agent/src/plateforme/identite.rs` — le contrôleur cite en outre
`configuration.rs:200-201` et `main.rs:270`, lus et confirmés ici aussi).

**Je ne l'ai pas corrigé** (règle du plan pour cette tâche 9 : aucune
modification de code). C'est, à mon jugement, le défaut le plus important
que cette revue transverse ait trouvé — plus important qu'aucune des
mineures différées ci-dessous — parce qu'il touche directement la mission
du propriétaire pour la suite (« rendre le service utilisable ») : sans un
mécanisme qui pousse `AGENT_VM`/`AGENT_SECRET` (ou un jeton équivalent) dans
l'environnement du processus superviseur de la VM, aucune installation de ce
package, aussi correcte soit-elle par ailleurs, ne peut aboutir à une session
qui fonctionne. Le remède le plus direct serait sans doute que `hooks/vm.py`
(tâche 6), qui possède déjà le chemin WinRM vers la VM, y dépose aussi ces
deux variables une fois l'enrôlement réussi — mais choisir entre ce remède,
un autre porté par `console`, ou un mécanisme distinct, est une décision de
conception qui appartient au propriétaire du dépôt.

---

## 6. Tri des mineures différées

Seize mineures ont été différées au fil des huit tâches (`progress.md`).
Pour chacune : correction avant fusion, ou reste due — et pourquoi. Aucune
n'a été corrigée par cette tâche 9 (règle : ne pas corriger par réflexe).

| # | Mineure (tâche d'origine) | Verdict | Pourquoi |
| --- | --- | --- | --- |
| 1 | `rustup target list \| grep` sous `2>/dev/null` attribuerait à « cible absente » une panne de `rustup` lui-même (T1) | **Reste due** | Cosmétique : si `rustup` lui-même est cassé, l'étape suivante (`cargo build`) échouera de toute façon avec un message qui le révèle. Aucun chemin ne rend un succès faux. |
| 2 | `mkdir -p` rend un message générique si la destination existe et n'est pas un répertoire (T1) | **Reste due** | Cas limite qui suppose un appelant se trompant sur son propre argument `<destination>` ; `mkdir -p` échoue déjà (code non nul), seul le message est moins parlant. Coût de correction disproportionné à la fréquence du cas. |
| 3 | `auth_mode` porte `required: true` ET `default: motdepasse` ; `validate_answers` lève avant de consulter le défaut (T2) | **Reste due** | Hérité du plan verbatim, pas un écart de l'implémenteur ; le défaut ne sert qu'au pré-remplissage du portail, jamais à un repli silencieux — comportement voulu ailleurs dans ce dépôt (« un défaut de mode est une ouverture, pas un confort »). |
| 4 | Aucun test versionné n'exerce le garde **générique** de `resolve.py` sur un cas vraiment imprévu — seul un `RecursionError` joué à la main par le relecteur l'a fait rougir (T3) | ✅ **CORRIGÉE le 30 août 2026** | C'est l'invariant CENTRAL de la tâche 3 (« refuse, ne lève jamais »), et ce dépôt a déjà érigé en règle transverse « un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle » — appliquée à ce MÊME chantier pour le garde éditeur-de-liens de la tâche 1 (élevé en Importante pour la même raison, `b63ea1e`). Fermée par `tests/test_desk_resolve.py`, § « Le garde GÉNÉRIQUE ». ⚠️ **Le remède évident ne mordait pas** : un JSON de profondeur excessive — le `RecursionError` que le relecteur avait joué à la main — est aujourd'hui AVALÉ par `json.load` (mesuré à 4 × `sys.getrecursionlimit()`), et l'entrée retombe dans un cas NOMMÉ ; le test aurait été vert sans jamais atteindre le garde. Les deux entrées retenues lèvent bien hors de tout `except` nommé : des octets non décodables en UTF-8 (`UnicodeDecodeError`) et un entier littéral de 5 000 chiffres (limite CPython). Vues rouges à 6 échecs en retirant le `except Exception`, restauration vérifiée par `sha256sum`. |
| 5 | Le test ne vérifie pas la forme complète de `facts` (T3) | **Reste due** | Repris verbatim du plan, pas un écart de l'implémenteur ; les champs individuels sont vérifiés ailleurs dans la suite. |
| 6 | Le contrôle de version Node interroge la machine qui exécute le hook, jamais une cible pas encore installée (T3) | ✅ **CORRIGÉE le 30 août 2026 — ET SA JUSTIFICATION ÉTAIT FAUSSE** | 🔴 **La raison écrite ici était démentie par le hook lui-même.** Elle affirmait que « `resolve` s'exécute réellement SUR la machine cible (c'est le contrat du moteur) ». Le docstring de `hooks/resolve.py` dit le contraire en toutes lettres — « CE HOOK COURT AVANT `partition()` : à l'instant où le moteur l'appelle, le disque cible n'existe pas encore » — et `installer/installer/packages/runner.py` aussi (« a resolve hook writing to the LIVE INSTALLER filesystem »). Le contrôle interrogeait donc le `node` de **l'ISO d'installation**, jamais celui que le service exécuterait. Deux affirmations versionnées de la même branche se contredisaient, et c'est **la fausse** qui avait servi à classer la mineure « pas un défaut ». **Le remède ne corrige pas le contrôle, il supprime l'écart** : `install` DÉPOSE désormais sur la cible le runtime que `resolve` a validé (`hooks/depot_node.py`, §4.6). Les deux `node` sont le même. |
| 7 | Pas de garde défensif sur `facts["turn_ecoute"]` (T4) | ✅ **CORRIGÉE le 30 août 2026 — ET SA RAISON AVAIT DÉJÀ ÉTÉ RÉFUTÉE PAR CETTE BRANCHE MÊME** | 🔴 La raison écrite ici — « `facts` a une forme fixe, produite par `resolve.py` du même package et jamais par un tiers » — a été **démentie sur la clé voisine** dix jours plus tôt : le lot 10A a démontré que `facts["hote"]="0.0.0.0"` traversait `install.py` sans jamais rencontrer la garde des écoutes universelles, rendait code 0, et écrivait `PLATEFORME_HOTE=0.0.0.0` — il a fallu une Importante pour le fermer. La même raison est pourtant restée écrite pour `turn_ecoute`, `turn_relais`, `proxy_confiance` et `port`. Les trois adresses passent désormais par le même validateur que la valeur dérivée ; `port` n'est plus un `int()` nu (qui levait une `ValueError` non rattrapée), et un booléen y est refusé nommément — `int(True)` vaut 1, un port valide et absurde. Vu rouge à **17 échecs** en neutralisant les trois validations, témoin négatif joué. ⚠️ **Ce que ces gardes ne promettent PAS** : que l'adresse soit privée — le ruling du lot 10A a refusé une liste blanche RFC1918, et cette réserve tient (§10.1). |
| 8 | `sous()` avalerait un chemin absolu (`pathlib.Path.__truediv__` avec un opérande absolu écrase le préfixe) (T4) | **Reste due, vérifié inoffensif aujourd'hui** | Vérifié par cette revue (`grep -n 'sous(' hooks/install.py`) : les cinq appels passent tous un littéral relatif codé en dur (`"etc/nivuus/desk.env"`, etc.), jamais une valeur dérivée d'`answers`/`facts`. Le risque est réel EN PRINCIPE mais inatteignable EN PRATIQUE dans le code actuel. |
| 9 | Un `install` rejoué reforge le secret en silence (T4) | **Reste due, mais à surveiller** | Vérifié : `ecrire_secret()` (`secrets.token_hex(32)`) est appelé inconditionnellement à chaque exécution d'`install.py` (`ecrire_secret()`, appelé deux fois dans `main()`), sans lire un `desk.env` préexistant. Une réinstallation/réparation regénère silencieusement `PLATEFORME_SECRET_JETON` ET `TURN_SECRET`, invalidant toute session en cours — opérationnellement surprenant, pas une faille de sécurité. `install` n'est normalement joué qu'une fois ; nommé pour le jour où quelqu'un le rejoue en réparation. |
| 10 | Rapport de tâche 4 inexact (« 314 lignes » / « 750 » annoncés contre 312/0755 mesurés) (T4) | **Non pertinent à corriger dans le code** ; **le même écart existe dans le message de commit `b6a0c92`, voir §2** | Le rapport de tâche n'est pas une pièce versionnée dans le même sens qu'un commit ; l'écart réel et actionnable est celui du message de commit, déjà nommé ci-dessus. |
| 11 | `activate.py` en 644 là où ses frères sont en 755 (T5) | **Sans objet, confirmé** | Le moteur lance `[sys.executable, hook, "--phase", phase]` (`installer/installer/packages/runner.py::_run_hook`) : le bit exécutable n'entre jamais en jeu. Vérifié à nouveau par cette revue, même lecture que le ruling du contrôleur pendant la tâche 5. |
| 12 | Rapport de tâche 5 annonce 394 lignes de test pour 379 (T5) | **Reste due, cosmétique** | Écart de rapport, pas de code ni de commit ; `git show 911c910:tests/test_desk_activate.py \| wc -l` confirme 379, cohérent avec le message de commit lui-même qui ne cite aucun total. |
| 13 | Aucune garde d'idempotence sur le dépôt d'`agent.exe` : chaque `activate` rejoué redéclenche une compilation croisée complète (~40 s) (T7) | **Reste due** | Assumé explicitement par l'implémenteur et le contrôleur : « sans corruption possible », coût de performance/UX seulement, pas de correction. Non commenté dans le code lui-même — pourrait l'être à peu de frais, mais ce n'est pas un défaut fonctionnel. |
| 14 | Le toucher de `tests/desk_activate_fixtures.py` au-delà du périmètre littéral de la tâche 7 (T7) | **Non pertinent : déjà jugé légitime par la revue de tâche 7** | Sans ce toucher, les scénarios existants auraient soit refusé (« console absent »), soit déclenché une compilation réelle de 40 s à chaque appel de test — la revue de tâche 7 l'a explicitement approuvé, avec la raison écrite dans son propre rapport. |
| 15 | Le glob de découverte de suites est `test_*.py`, pas `test_desk_*.py` (T8) | **Reste due** | Fonctionne aujourd'hui parce que toutes les suites de `tests/` portent le préfixe `desk_` implicitement (elles ne s'appellent QUE `test_desk_*.py`) ; un futur `test_foo.py` non préfixé serait pris pour une suite de ce package. Improbable tant que `tests/` reste privé à `desk`, et le `Makefile` documente déjà la règle de sélection énoncée — respecte la doctrine « énoncer la règle de sélection avant de compter ». |
| 16 | Le README ne restitue pas lui-même la distinction « 10 étapes contre 18 en-têtes » de `verify-all.sh`, il défère à `CLAUDE.md` (T8) | **Reste due, choix délibéré** | Éviter la duplication d'une même information à deux endroits qui pourraient diverger — cohérent avec la doctrine anti-« naufrage du 487 » de ce dépôt (une affirmation vérifiable ne devrait vivre qu'à un seul endroit). |

**Résumé du tri, réécrit le 30 août 2026** : la version précédente concluait
que **sur seize mineures, une seule** (#4) méritait correction avant fusion.
La revue finale de branche a **renversé ce tri sur deux points de plus**, et
les trois sont désormais **corrigées** : #4 (le garde générique sans test),
#6 (dont la justification était fausse) et #7 (dont la raison avait déjà été
réfutée par cette branche, sur la clé voisine). Les treize autres restent
dues, avec leur raison propre — et le §12.4 dit lesquelles la revue finale a
**confirmées** comme pouvant rester dues, plutôt que de les recopier.

🔵 **La leçon transverse de ces deux renversements** : dans les deux cas, ce
qui était faux n'était pas le verdict mais **sa raison**, et personne ne
relit une raison. Une mineure classée « pas un défaut » sur un motif que le
code contredit est plus dangereuse qu'une mineure ouverte : elle est
**fermée**.

---

## 7. Ce que le lot 10 devait trancher — et ce qui a RÉELLEMENT été tranché

> 🔴 **DEUX LIGNES DE CETTE SECTION ANNONÇAIENT DES DÉCISIONS QUE LE LOT 10 A
> PRISES EN SENS INVERSE.** Elles sont corrigées ici, à leur place, et le
> détail de ce qui a été fait est au **§10**.

- **Le défaut transverse du §5** (`AGENT_VM`/`AGENT_SECRET` jamais poussés
  dans la VM) : **TOUJOURS OUVERT**, vérifié le 30 août 2026 — `hooks/vm.py`
  ne pousse rien dans la VM, et `console/guest/provision/assets/run-agent.ps1`
  n'a pas été corrigé. C'est la découverte la plus importante de la tâche 9,
  et elle reste due.
- Port et hôte de la plateforme : **3445** / **192.168.3.1**. ✅ **Appliqués
  et mesurés** (§10.1).
- ~~« La route Pomerium devra être recadrée de `127.0.0.1:3445` vers
  `192.168.3.1:3445` »~~ — 🔴 **CETTE PHRASE VISAIT LE MAUVAIS FICHIER.** Le
  `config.yaml` réellement chargé par Pomerium est
  **`/opt/nivuus/Pomerium/config.yaml`**, jamais `/etc/pomerium/config.yaml`
  que la reconnaissance avait lu — ce dernier reste sur le disque comme
  **leurre**, périmé. Le recadrage a été fait dans le bon fichier au lot 10B.
- `SIGNALING_URL` de la VM : passée à `ws://192.168.3.1:3445/signal`
  (§10.2) — **avec** le suffixe que `auth-pomerium` a introduit.
- ~~« `PLATEFORME_AUTH=motdepasse` retenu plutôt que `pomerium`, pour ne pas
  ajouter `pass_identity_headers` à une route Pomerium qui sert six autres
  hôtes »~~ — 🔴 **LE PROPRIÉTAIRE DU DÉPÔT A TRANCHÉ L'INVERSE** (29 août
  2026, question posée et répondue : « DÉLÉGUER À POMERIUM »). Le service
  tourne en **`pomerium`**, mesuré le 30 août 2026 sur le processus qui
  écoute : `PLATEFORME_AUTH=pomerium`, `PLATEFORME_PROXY_DE_CONFIANCE=192.168.3.1`,
  `PLATEFORME_HOTE=192.168.3.1`, `PLATEFORME_PORT=3445`,
  `PLATEFORME_PAGE=/opt/nivuus/desk/client/dist`. Le choix du propriétaire
  prime sur la recommandation du contrôleur, et rend
  `PLATEFORME_PROXY_DE_CONFIANCE` obligatoire, `pass_identity_headers`
  nécessaire sur la route, et l'écoute non universelle **NÉCESSAIRE** et non
  plus seulement préférable.
- `coturn` (§4.7) reste à équiper : **rien n'écoute sur le port 3478 de cette
  machine**, alors que le service annonce aujourd'hui
  `TURN_URL=turn:90.87.35.18:3478` à ses clients. Node sur la cible (§4.6)
  est, lui, **fermé**.

---

## 8. Réserves de cette revue elle-même

- Les affirmations comportementales ponctuelles des messages de commit
  (comptes d'échecs exacts sur un état de code révolu, ex. « 9 échecs » de
  `e1d07a7`) n'ont pas été rejouées à l'identique — l'état du hook qui les a
  produites n'existe plus tel quel. Elles ont été jugées cohérentes avec la
  doctrine TDD de ce chantier et avec la suite de tests actuelle, pas
  revérifiées bit à bit.
- Cette revue n'a pas cherché à établir la provenance exacte de
  `guest_workdir` ni le comportement réel de Pomerium sans
  `pass_identity_headers` explicite (§4.4 de `recon-appliance.md`, « Ce que
  je n'ai pas pu établir ») — hors périmètre de la tâche 9, déjà nommé par
  la reconnaissance elle-même.
- Le défaut transverse du §5 a été établi par lecture de code des trois
  dépôts concernés (`desk`, `console` via `installer/`, et `agent/src`),
  jamais par une exécution réelle sur la VM (qui n'a pas été touchée par
  cette revue, conformément à son périmètre en lecture seule).

---

## 9. Correction de `CLAUDE.md` — méthode, compte, ventilation, et ce qui a été laissé

> **Ronde de correction 1 (relue par le contrôleur)** : la première version
> de ce document ne portait cette méthode, ce compte et cette ventilation
> nulle part — ils ne vivaient que dans un rapport de tâche **gitignoré**
> (`task-9-report.md`). Le contrôleur a nommément rappelé la règle que ce
> dépôt dit avoir payée : *une preuve ne doit jamais vivre dans un rapport
> gitignoré*. Cette section la déplace ici, dans le document **versionné**
> qui fait foi.

### Méthode et premier passage

Recherche par le sens, pas par une seule formule, sur les quatre familles
que le brief de la tâche 9 nomme explicitement : montage CIFS, `C:\dev`,
`scripts/winrm.js`, compilation sur la VM.

Premier passage (motifs littéraux) :

```
grep -c "sync-agent\|build-agent.sh\|run-agent.sh\|C:\\dev\|/media/vm\|winrm.js\|montage CIFS\|scripts/check-session\|scripts/stop-agent\|scripts/sonde-multifenetre" CLAUDE.md
```

Résultat mesuré **avant** les éditions de cette tâche : **48**. Ventilation
par famille à ce moment-là :

- `CIFS` / `/media/vm` : 7 lignes.
- `C:\dev` : 2 lignes.
- `scripts/winrm.js` : 2 lignes.
- `build-agent.sh`/`sync-agent.sh` (« compile SUR la VM ») : 9 lignes.

Après élimination des recoupements entre familles, ces occurrences se
regroupaient en **deux blocs contigus** portant l'affirmation périmée
(« la VM est une machine de développement sur laquelle on compile ») :
la section « 🖥️ Cycle de vie de la VM Windows » et le tableau
« ### L'agent, sur la VM » — chacun corrigé par un bandeau nommant les
trois faits neufs, sans réécrire le contenu qui suit (dont le sort n'est
pas ma décision).

⚠️ **Ce premier passage a MANQUÉ une affirmation contraire réelle** — relevé
par la revue de la ronde 1 : `CLAUDE.md` (juste après le tableau
« L'agent, sur la VM ») portait encore, sans aucun des dix termes du grep
ci-dessus :

> 🔴 **Avant toute compilation qui touche `proto/`** — l'horloge de la VM
> avance sur celle de l'hôte, et cargo saute alors le rlib de `proto` […]

Ce paragraphe affirme, lui aussi, qu'on compile Rust **sur la VM** — exactement
le mode d'échec contre lequel la tâche 9 avait été mise en garde d'avance
(« chercher par la formule au lieu du sens »). **Corrigé** : il reçoit
désormais le même bandeau que le tableau au-dessus de lui, nommant qu'il vise
la VM de développement d'avant le 29 août 2026 et que son sort n'est pas
tranché, avec un renvoi à cette section.

### Second passage, par concepts (ronde de correction 1)

Relancé, cette fois sur des **concepts** plutôt que des chaînes : « compiler
sur la machine distante », « l'horloge de la VM », « le rlib », « la tâche
planifiée », « le partage », plus une vérification directe
(`cargo build`, `link.exe`, `MSVC`, `rustc`, `target\release`, `\dev\`).

**Aucune nouvelle affirmation contraire** trouvée au-delà de celle déjà
traitée ci-dessus. Les autres occurrences de ces concepts vivent toutes dans
la sous-section « ### Outillage VM et Windows » de « 🪤 Pièges transverses »,
et s'en distinguent d'une façon qui justifie de les LAISSER plutôt que de les
encadrer : cette section est explicitement rétrospective (son propre titre :
« ceux que ce dépôt a payés PLUSIEURS fois »), donc déjà lue comme un relevé
historique, jamais comme une instruction à exécuter aujourd'hui — à la
différence du tableau et du paragraphe `proto/`, qui vivent dans
« 🔨 Commandes » et se lisent comme des gestes à faire maintenant. Les cinq
pièges concernés, tous dans cette même sous-section, et pourquoi chacun reste
en l'état :

- « `build-agent.sh` RSYNCHRONISE L'ARBRE ENTIER » — leçon sur un script dont
  le sort n'est pas tranché (déjà repérée au premier passage).
- « LE DÉFAUT À DEUX RÉGLAGES… dans `build-agent.sh` / `run-agent.sh` » —
  même raison (déjà repérée au premier passage).
- « UN AGENT SURVIVANT TIENT `agent.log` […] `link.exe` en 1104 » — leçon
  méthodologique sur la lecture d'un journal après un échec de build,
  indépendante de la nature dev-VM/appliance de la machine.
- « UN RELEVÉ WinRM EST CELUI DE LA SESSION 0 […] une tâche planifiée `/it` »
  — leçon générale sur WinRM et la session 0, qui reste vraie quel que soit
  le chemin WinRM employé (`scripts/winrm.js` ou `winrm_exec.py`) : elle ne
  décrit pas une capacité de la VM, mais une propriété de WinRM lui-même.
- « `nodejs-winrm` ENVELOPPE TOUJOURS LA COMMANDE […] Écrire le script sur le
  partage et l'invoquer par `-File` » — piège spécifique à la bibliothèque
  `nodejs-winrm` qu'utilise `scripts/winrm.js` ; son sort suit celui de ce
  script, déjà nommé comme non tranché.

Une sixième occurrence, hors sujet et laissée sans y toucher : « … 18 619
lignes en quelques secondes sur un partage CIFS ont empêché une session de
s'établir » (section « Ce que le montage de recette ne peut pas voir ») — un
piège de traçage par paquet dans la boucle de transport WebRTC, sans rapport
avec la nature de la VM cible.

### Compte final, et pourquoi il diffère du premier

**Rechercher à nouveau la même commande après TOUTES les éditions de cette
tâche (y compris celle de la ronde de correction 1) rend 55, pas 48.**
L'écart n'est pas une erreur : les bandeaux correctifs eux-mêmes citent
`C:\dev`, `/media/vm`, `scripts/winrm.js` et `scripts/build-agent.sh` pour
les nommer, ce qui fait mécaniquement remonter le compte du grep qui les
cherche. **Le 48 mesure l'état AVANT correction ; le 55 mesure l'état
APRÈS.** Un lecteur qui relance la commande sur `HEAD` doit s'attendre à 55,
pas à 48 — les deux nombres sont vrais, de deux instants différents, et
c'est la raison qu'il fallait écrire plutôt que recopier un seul chiffre.

### Bilan complet, en un tableau

| Catégorie | Compte | Traitement |
| --- | --- | --- |
| Blocs corrigés par un bandeau | 3 (section VM, tableau « L'agent, sur la VM », paragraphe `proto/`) | Bandeau nommant les trois faits neufs + sort non tranché, sans réécrire le contenu qui suit |
| Pièges historiques laissés tels quels, avec raison | 5 (`build-agent.sh` rsync, encodage PowerShell, `agent.log`/`link.exe`, session 0/`tâche planifiée`, `nodejs-winrm`/partage) | Laissés : rétrospectifs par construction (section « Pièges transverses »), liés au sort non tranché des mêmes scripts |
| Mention hors sujet, non touchée | 1 (partage CIFS générique dans le traçage WebRTC) | Aucun rapport avec la nature de la VM — écartée |

### Une mineure différée, nommée par le contrôleur, non corrigée ici

Le chemin exact cité pour `guest-ready-watch.py` (§2 ci-dessus, et dans
`hooks/vm.py`) ne porte jamais son répertoire parent : le fichier vit sous
`console/host/guest-ready-watch.py`, **pas** sous `console/guest/` — vérifié
(`find … -iname guest-ready-watch.py`). Aucun document de ce lot n'affirme
le mauvais chemin, mais aucun ne dit non plus le bon. Classé mineure et
**délibérément non corrigé** par cette ronde — instruction du contrôleur,
qui l'a lui-même différée plutôt que demandé de la traiter.

---

## 10. Les lots 10A à 13 — la mise en service réelle

> 🔴 **CETTE SECTION EXISTE PARCE QUE TOUTE LA PREUVE DE CES LOTS NE VIVAIT
> QUE DANS DES RAPPORTS GITIGNORÉS** (`.superpowers/sdd/…/lot-10*.md`,
> `lot-1{1,2,3}-*.md`, `progress.md`), voués à disparaître avec la session
> qui les a écrits. C'est le patron que `CLAUDE.md` nomme « UNE PREUVE NE
> DOIT JAMAIS VIVRE DANS UN RAPPORT GITIGNORÉ » — et **la revue de la tâche
> 9 de cette branche même l'avait déjà fait corriger une fois** (c'est ce
> que le §9 ci-dessus est). Quatre lots plus tard, le même défaut avait été
> rouvert, en plus grand. Six commits de travail réel (`ae98f88`, `bf5a2b3`,
> `6668865`, `07ed919`, `b9a4679`, `a63d543`) étaient postérieurs au dernier
> document versionné.

**Le lot 10 n'est pas dans le plan à neuf tâches** : il a été ajouté par le
contrôleur après la tâche 9, sur la mission du propriétaire du dépôt —
« finir le projet et le rendre UTILISABLE VIA `https://app.allanic.me` ».
Le plan produit le package ; il ne produit pas le service en marche.

⚠️ **La mise en service a d'abord été BLOQUÉE par le classificateur de
permissions, à juste titre** : elle écrit dans `/etc`, pose une unité
systemd, installe un runtime, touche un proxy partagé et l'intérieur de la
VM. Le contrôleur a arrêté et demandé. Réponses du propriétaire (29 août
2026) : ampleur « **TOUT, y compris Pomerium et la VM** » ;
authentification « **DÉLÉGUER À POMERIUM** » — cette seconde réponse
**renverse** la décision antérieure du contrôleur (voir §7).

### 10.1 Lot 10A — l'hôte : Node, le build, `install`+`activate` réels

Le service **tourne et sert**, vérifié indépendamment par la revue du lot :
`200` sur la racine, `404` sur un chemin absent, `ss -ltn` montrant
`LISTEN 192.168.3.1:3445` — **borné à cette adresse**, jamais universel.

🔴 **QUATRE DÉFAUTS RÉELS DU PACKAGE TROUVÉS EN MARCHANT, dont un GRAVE** —
et aucune des huit suites de tests ne pouvait les voir :

1. 🔴 **`PLATEFORME_HOTE` était confondu avec l'adresse TURN**, dérivée de la
   route par défaut — qui sur cet hôte est **l'adresse PUBLIQUE**
   (`ppp0`, `90.87.35.18`). Une installation réelle aurait **exposé le
   bureau distant sur l'internet, sans Pomerium devant**. 🔴 **Et la garde
   du produit ne l'aurait PAS vu** : `config.ts::ECOUTES_UNIVERSELLES` ne
   connaît que `0.0.0.0`, `::`, `[::]` et `*` — jamais « une adresse
   routable ordinaire ». Corrigé en séparant les deux dérivations
   (`commun.py::HOTE_DEFAUT` porte le diagnostic complet, dans le code).
2. Réinstallation cassée par les liens symboliques de `node_modules/.bin`
   (`dirs_exist_ok=True` couvre les répertoires, jamais les liens qu'ils
   contiennent) — jamais vu parce qu'aucun test ne rejouait `install` deux
   fois sur la même racine.
3. Permissions `DynamicUser` bloquant le `CHDIR` sous l'umask de root :
   `200/CHDIR`, avant la première ligne de JavaScript.
4. **`proto/ts/` n'était pas copié du tout** : `plateforme/src` l'importe par
   un chemin relatif, `tsx` transpile à la volée, `npm start` échouait en
   `ERR_MODULE_NOT_FOUND`.

**Une Importante en revue, corrigée (ronde 1)** : le garde d'écoute
universelle était **contourné** quand `facts["hote"]` était fourni — démontré,
code 0 et `PLATEFORME_HOTE=0.0.0.0` écrit sans un mot. Ruling retenu : *un
défaut de sécurité fermé sur un seul de ses deux chemins n'est pas fermé*.

⚠️ **Réserve du lot 10A, laissée NOMMÉE et toujours ouverte** : le canal
`facts` accepte encore une **adresse publique routable** sans refus. Trois
raisons de ne pas poser de liste blanche RFC1918 : ① le chemin est
inatteignable aujourd'hui (le moteur ne passe aucun `facts` à `install`) ;
② une liste blanche interdirait des déploiements légitimes ; ③ la notion
d'« adresse sûre » appartient à la garde du **produit**, et la durcir dans le
seul package créerait deux notions divergentes de la même chose. Atténuation :
le démarrage **annonce** l'adresse retenue.

### 10.2 Lot 10B — Pomerium reciblé, l'agent de la VM armé, ProjFS rejoué

⚠️ **Le fichier `config.yaml` réellement chargé par Pomerium est
`/opt/nivuus/Pomerium/config.yaml`**, jamais `/etc/pomerium/config.yaml` que
la reconnaissance avait lu et cité — ce dernier reste sur le disque, périmé,
comme **leurre**. La route `app.allanic.me` a été recadrée vers
`192.168.3.1:3445` avec `pass_identity_headers`, dans le bon fichier.

🔴 **Legs du lot 10B, TOUJOURS OUVERT** : `AGENT_VM`/`AGENT_SECRET` ne sont
toujours poussés par rien dans la VM (voir §5) — sans ce couple, aucune
session ne peut s'établir, quelle que soit la qualité de l'installation.

### 10.3 Ce que le NAVIGATEUR du propriétaire a trouvé, et qu'aucun test n'avait vu

Le propriétaire a joué le produit **pour de vrai**. Quatre défauts en sont
sortis, tous de la classe que `CLAUDE.md` nomme « ce qu'un navigateur exige,
aucun test de Node ne le voit » — **et qui n'a aucun garde automatique dans
ce dépôt**.

1. Un `400` de Pomerium — **pas une régression du chantier** : un service
   worker du spike du 28 juillet survivait dans son profil et rejouait une
   URL de connexion en cache dont la signature avait expiré.
2. 🔴 **La CSP de la plateforme interdisait son PROPRE script inline** :
   `entetes-page.ts` pose `script-src 'self'`, et `vite.config` injectait une
   amorce anti-FOUC **verbatim** dans chaque page. Corrigé (`6668865`) en
   **SORTANT l'amorce du HTML** plutôt qu'en ajoutant un hash — parce que
   `deploiement/nginx.conf` porte une copie **statique** de la même CSP et
   que nginx ne sait pas calculer un hash : un hash aurait dû être recopié à
   la main d'un fichier à l'autre. Le garde qui en sort,
   `client/src/design/amorce-theme.csp.test.ts`, est un test de `client/`
   qui **importe** un fichier de `plateforme/` — précisément parce que le
   défaut vivait dans l'espace entre deux paquets qui ont chacun leur suite.
3. ⚠️ **La racine `/` sert `index.html`, qui est la PAGE DE SESSION**, laquelle
   se rabat sur `?? 'demo'` sans jeton. Un utilisateur qui tape l'adresse du
   service tombe donc mécaniquement sur **la seule page qui ne peut pas
   marcher** (« poignée de main refusée : poignée de main sans jeton sur la
   session demo »). 🔴 **NON CORRIGÉ, décision offerte au propriétaire** :
   servir le hub à la racine, ou rediriger.
4. 🔴 **LE HUB ÉTAIT VIDE — le défaut le plus grave du lot.** `GET /vm`
   rendait `{"vms":[]}` parce que `vm.utilisateur_id` était `NULL` : le hook
   `activate` créait le compte, enrôlait l'agent, et **s'arrêtait là**. 🔴 **Et
   le code l'avait écrit** : `routes-applications.ts` porte, depuis avant ce
   correctif, « `vm.utilisateur_id` est NULL après `npm run admin:agent` —
   `enrolerLaVm` ne passe jamais d'utilisateur ». **Trois revues du hook
   `activate` ont lu ce fichier sans rapprocher les deux** : un défaut qui
   franchit une frontière de tâche, chaque moitié correcte prise seule.
   Réparé à la main sur l'instance, puis **fermé dans le package par le lot
   13**.

⚠️ **Incident d'exploitation, consigné** : un `rsync -a` de redéploiement a
recopié des droits trop restrictifs et le service, sous `DynamicUser`, n'a
plus pu lire la page — page blanche pour le propriétaire pendant une minute.
Réparé par `chmod -R a+rX`. **Tout redéploiement doit refaire ce geste.**

### 10.4 Lots 11, 12 et 13

- **Lot 11** (`6668865`) : la CSP, ci-dessus.
- **Lot 12** (`07ed919`) : le lien du manifeste PWA part avec les cookies,
  Pomerium n'a plus à rediriger.
- **Lot 13** (`b9a4679` + `a63d543`) : le hook `activate` **attribue la VM**
  au compte qu'il vient de créer — le trou du §10.3 ④, fermé dans le
  package. Placement soigné, avec ses deux pièges écartés et leur raison :
  ni dans le court-circuit d'idempotence (jamais atteint au passage normal),
  ni hors de toute garde (`vm-deja-attribuee` ferait échouer toute
  réactivation). Une extraction dédiée (`agent_payload.py`) a précédé
  l'ajout, `activate.py` frôlant 500.

---

## 11. Un défaut du PRODUIT, établi par le lot 10D — la fenêtre perdue au bout de 30 s

> 🔴 **CE RÉSULTAT EST LE PLUS IMPORTANT DE TOUTE LA BRANCHE, ET IL N'A
> JAMAIS ÉTÉ ÉCRIT NULLE PART DE VERSIONNÉ.** Il est porté ici, et dans les
> « Legs ouverts » de `CLAUDE.md`, sur consigne de la revue finale.
> **Ce n'est PAS un défaut du package** : c'est un défaut du produit, et
> **il n'est pas corrigé** — le corriger est un changement de conception qui
> mérite sa propre tâche.

**Le symptôme rapporté par le propriétaire** : le hub s'ouvre, mais aucune
fenêtre n'est disponible ; le client dit que l'agent n'a pas répondu.

**L'hypothèse de départ — « les fenêtres sont en session 0 » — est
RÉFUTÉE**, par capture réseau et messages du protocole décodés (`tcpdump`
sur l'interface libvirt de la VM, `tshark` sur le WebSocket `ws://`, non
chiffré) :

```
t=8,83 s   /agent  → enrôlement
t=8,97 s   /signal → déclaration agent, session « …:bureau »
t=12,00 s  fenetre-ouverte  « xemu | v0.8.136 »
t=12,04 s  fenetre-ouverte  « Steam Big Picture Mode »
t=12,04 s  fenetre-ouverte  « Untitled - Notepad »
t=43,14 s  refus … « la page-shell n'a jamais répondu après la relance »  ×3
```

L'agent **voit** les trois fenêtres, les **annonce**, puis les **retire
lui-même** 31 secondes plus tard. Ce n'est pas un défaut d'énumération
inter-session : c'est un mécanisme du produit qui fait exactement ce qu'il
dit.

**Le mécanisme, nommé** :
`agent/src/superviseur/table/orphelines.rs::relancer_les_orphelines`,
second garde-fou — toute entrée en `AttendLeViewport` depuis plus de
`DELAI_ATTENTE_VIEWPORT_MAX` (`agent/src/superviseur/table.rs`, **30 s**) est
retirée et refusée. Le commentaire de la constante le dit lui-même :
« une fenêtre préexistante au démarrage est abandonnée si la page-shell ne
s'est pas connectée dans ce délai ».

🔴 **ET ELLE N'EST JAMAIS REPROPOSÉE.** Le commentaire d'`orphelines.rs` est
explicite — « une entrée abandonnée n'est JAMAIS reproposée, le hook ne
réémettant rien pour une fenêtre déjà ouverte ». La seule façon de la revoir
est que la fenêtre Windows soit **fermée puis rouverte**.

🔴 **ET RIEN NE BUFFERISE CÔTÉ PLATEFORME.**
`plateforme/src/signaling/appariement.ts` ne conserve aucun état pour
`fenetre-ouverte`/`refus` — seule l'offre SDP est mémorisée, dans un champ
dédié. Un message dont le pair n'est pas connecté est **simplement non
relayé, sans mise en file**. Un navigateur qui arrive après n'a **aucun
moyen** de savoir qu'une fenêtre avait existé.

**Pourquoi c'est exactement le mode d'usage réel** : derrière Pomerium,
l'OAuth Google prend du temps. À l'instant où le navigateur se connecte
enfin, toute fenêtre annoncée plus de 30 s plus tôt a déjà été refusée.

⚠️ **Ce que le lot 10D n'a PAS établi** : ① le texte exact que le client
affiche (le lot s'est arrêté au signaling) ; ② qu'un navigateur **déjà
connecté** au moment de l'ouverture fonctionnerait — probable d'après le
code lu (`viewport_recu` déclenche `CreerSortie`), **non mesuré**, faute de
navigateur réel derrière l'OAuth ; ③ `RELANCES_MAX`, le **premier**
garde-fou, n'a pas été exercé.

⚠️ **Aparté d'outillage, à connaître avant de perdre une heure** :
`scripts/winrm.js` échoue systématiquement sur cette VM, y compris pour
`Get-Date` — la panne est **côté client Node**, la VM répond
(`curl --ntlm` rend `401 WWW-Authenticate: Negotiate`). L'outil qui marche
est `console/guest/winrm_exec.py`. C'est le même constat que la réserve en
tête de `CLAUDE.md` § « Cycle de vie de la VM Windows ».

---

## 12. La revue finale de branche, et ce qu'elle a fait corriger

**30 août 2026, `084bc6c` → `a63d543`, verdict : NON FUSIONNABLE EN L'ÉTAT.**
Une Critique, six Importantes. Corrections jouées le même jour
(`e6c3389` → `ae8a1fb`).

### 12.1 La Critique — `resolve` refusait TOUJOURS, donc le package ne s'installait pas

`hooks/resolve.py` refusait sur `hw.get("vm_windows")`. **Aucun producteur de
cette clé n'existe** : `installer/installer/common/hardware.py::detect_all()`
rend **huit** clés (`disks`, `ethernet`, `wifi`, `gpus`, `cpu`, `iommu`,
`memory_mib`, `passthrough_candidates`), et `install-engine/run.py` passe ce
dict **verbatim** à `plan_packages` puis à `run_resolve` — vérifié, `hw`
n'est enrichi nulle part entre les deux. Le hook refusait donc toujours, et
`steps/packages.py` traduit un refus en `StepError` : **l'installation
entière s'arrêtait**, pas seulement `desk`.

🔴 **POURQUOI HUIT SUITES ET NEUF REVUES NE POUVAIENT PAS LE VOIR : elles
FABRIQUAIENT la clé dont elles vérifiaient la consommation.** **Sept** occurrences dans
`tests/test_desk_resolve.py` (six `True`, une `False` — celle qui éprouvait
le refus) et **une** dans `tests/test_desk_install.py`, recomptées le
30 août 2026 par `git show e6c3389:… | grep -c` ; le lot 10A l'a écrite à la main dans son contexte
de test. C'est la forme la plus pure du « contrôle qu'on n'a jamais vu
rouge » : **un test qui invente son entrée ne peut pas découvrir que
personne ne la produit.**

**Aggravant, et il gouverne le remède** : `plan_packages()` appelle `resolve`
**AVANT `partition()`** — le docstring de `installer/installer/packages/
runner.py` le dit. À cet instant, le disque cible n'existe pas, donc le
système que `console` installe n'existe pas, donc la VM n'existe pas. La
condition était **insatisfiable par construction**.

**Ce qui a été tranché, et pourquoi** :

- **la porte est RETIRÉE de `resolve`** — parce que la garantie existe déjà,
  **plus tôt et plus forte, et qu'elle n'est pas la nôtre** :
  `nivuus-package.yaml` déclare `requires: packages: [console]`, et
  `plan_packages()` refuse par `missing_dependencies` **avant** le premier
  hook `resolve` et **avant** `partition()` ;
- **elle MIGRE dans `activate`** — la seule phase où la VM peut exister
  (après le redémarrage, sur le système installé, avec le réseau, `console`
  activé avant `desk`). Elle y est une **MESURE** : le premier échange WinRM
  réel. Son message nomme la VM, `console`, et le fait que `resolve` ne
  pouvait pas l'éprouver ;
- **une sonde séparée en lecture seule a été écartée**, avec sa raison
  écrite : un second aller-retour WinRM qui ne dirait rien de plus, et dont
  le critère de succès (une sortie non vide) n'est pas fiable —
  `winrm_exec.py` rend légitimement une sortie vide pour une commande qui
  n'imprime rien. *Un contrôle qui ne peut pas distinguer ses deux valeurs
  n'est pas un contrôle.*

**Le contrat est désormais FIGÉ par une suite dédiée** :
`tests/test_desk_contrat_hw.py` (neuvième suite) lit les clés du
**producteur** — par analyse syntaxique de `detect_all()`, jamais exécutée —
et rougit si un hook lit dans `hw` une clé absente de cet ensemble (élargi
aux `facts` pour `activate`, que le moteur y fusionne). Il rejoue aussi
`resolve` sur le contexte que le **moteur** enverrait.

**Vue ROUGE sur le `resolve.py` d'avant le correctif**, copie nommée
d'abord :

```
FAIL (3)
  - resolve ne lit dans hw aucune clé que detect_all() ne produit pas: got ['vm_windows'], want []
  - contexte du MOTEUR : AUCUN refus: got ["aucune VM Windows détectée ou répondante : …"], want []
  - contexte du MOTEUR : un événement facts est émis: got 0, want 1
```

**Vue VERTE après**, restauration vérifiée par `sha256sum` :

```
OK - contrat de hw : 8 cles produites par detect_all(), 7 facts emis par resolve
```

⚠️ **La ROUGE reproduit mot pour mot le refus que l'opérateur aurait lu** sur
une machine où c'est `console`, plus tard dans la même installation, qui doit
créer cette VM.

### 12.2 Les Importantes, et ce qu'elles sont devenues

| # | Importante | État |
| --- | --- | --- |
| a | **`/opt/nivuus/node/bin` n'était posé par AUCUN hook** — un geste manuel du lot 10A, consigné dans un rapport gitignoré | ✅ **Fermée** : `install` dépose le runtime que `resolve` a validé, et refuse en le nommant s'il est introuvable (§4.6) |
| b | **`install.py` échouait par TRACE PYTHON sur un dépôt frais**, après avoir écrit `desk.env` et ses deux secrets | ✅ **Fermée** : un pré-vol énumère **tout** ce qui manque et refuse **avant** le premier `ecrire_secret()`. Vu rouge à 8 échecs, dont « aucune trace Python: got True » et « desk.env n'est PAS cree: got True » |
| c | **Douze affirmations FAUSSES** dans ce document | ✅ **Corrigées sur place**, chacune remesurée — §1, §3, §4.2, §4.6, §4.7, §6, §7, et six numéros de ligne remplacés par des noms |
| d | **Les lots 10A à 13 n'avaient rien de versionné** | ✅ **§10** |
| e | **Le diagnostic du lot 10D ne vivait que dans un rapport gitignoré** | ✅ **§11**, et dans les « Legs ouverts » de `CLAUDE.md`. **Non corrigé dans le produit**, à dessein |
| f | Les mineures **#4, #6, #7** deviennent bloquantes, et deux verdicts sont renversés | ✅ **Les trois corrigées**, §6 |

### 12.3 Ce que ces corrections n'établissent PAS

- **Qu'une installation ait été jouée par le MOTEUR RÉEL.** La Critique a été
  établie par lecture de code des quatre fichiers du moteur et par l'absence
  totale de `vm_windows` dans le dépôt voisin ; le correctif est éprouvé par
  une suite qui **lit** le producteur, jamais par un `run.py` exécuté. C'est
  dit comme tel.
- **Que le service démarre sur une machine neuve** avec le runtime déposé :
  la forme du dépôt est éprouvée (le vrai runtime de cette machine, ses vrais
  liens), jamais un `systemctl start` sur une cible fraîche.
- **Que l'agent croisé fonctionne** (§4.1, inchangé), ni **le critère ⑦**
  d'`auth-pomerium` (la page derrière l'OAuth exige un humain), ni **aucun
  des douze items du lot 3** (suspendu).
- **Que les rouges des lots 10A→13 aient été rejouées** : la revue s'est
  bornée à vérifier que le code qu'elles décrivent existe et dit ce que les
  rapports prétendent.

### 12.4 Ce que la revue finale a CONFIRMÉ comme pouvant rester dû

Sans les recopier : les mineures **#1, #2, #3, #5, #8, #9, #10 à #16** du §6,
chacune avec la raison qui y est écrite ; la **réserve du lot 10A** sur
l'adresse publique routable dans `facts` (§10.1) ; le **legs
`AGENT_VM`/`AGENT_SECRET`** (§5), qui reste la découverte la plus importante
de la tâche 9 ; **coturn posé mais jamais armé** (§4.7) ; et le choix, laissé
au propriétaire, de **ce que sert la racine `/`** (§10.3 ③).

⚠️ **Deux traces hors du dépôt à purger par le propriétaire**, relevées par la
revue : `/var/tmp/desk-admin-password.txt` porte le mot de passe
administrateur **en clair**, et `/var/tmp/lot10*`, `/var/tmp/lot10d/`,
`/var/tmp/desk-contexte.json` subsistent. Hors du dépôt, donc non bloquant
pour la fusion — mais ce document est le seul endroit versionné qui le dit.
