# desk — le bureau distant Windows, en package Nivuus

Ce répertoire est un **package Nivuus** (`nivuus.dev/v1`) : le moteur
d'installation le découvre, l'offre dans l'assistant, et l'installe par les
trois mêmes phases que n'importe quel package tiers. Il n'a rien de
spécial — c'est le point : si l'API ne suffisait pas pour lui, elle ne
suffirait pour personne.

| Phase | Ce qu'elle fait |
|---|---|
| `resolve` | Lecture seule, et **avant `partition()`** — le disque cible n'existe pas encore. Dérive l'adresse d'écoute, le port, les deux adresses TURN et le proxy de confiance ; valide `node` contre `engines.node` ; **refuse**, avec une raison, ce qui ne peut pas être dérivé. Émet un événement `refuse` et sort en code 0 — jamais une exception non rattrapée (voir `hooks/resolve.py`). ⚠️ Il ne dérive **aucune origine CORS** : `PLATEFORME_ORIGINE_CLIENT` n'est écrite par aucun hook, son absence étant le cas nominal derrière un proxy. Cette ligne l'affirmait ; c'était faux, relevé par la revue finale de branche. |
| `install` | **Un pré-vol d'abord** : ce qui manque à la source (`client/dist`, `plateforme/node_modules/.bin/tsx`, `proto/ts/`, un runtime Node à déposer) est dit **avant qu'un secret soit tiré**. Pose ensuite la plateforme, le client bâti et `proto/ts/` sous `/opt/nivuus/desk/`, **le runtime Node sous `/opt/nivuus/node/`**, l'unité systemd `desk-plateforme.service`, `desk.env` (secrets tirés au sort, jamais demandés au wizard), `turnserver.conf`. N'a **aucun** canal `refuse` : une erreur ici sort en code non nul, avec une phrase (voir `hooks/install.py`). |
| `activate` | **C'est ici que la VM Windows est éprouvée**, par un échange WinRM réel — la seule phase où elle peut exister (voir plus bas). Arme le lien systemd que `install` n'a fait que poser, pose ProjFS dans la VM, crée le compte administrateur initial, enrôle l'agent, **lui attribue la VM** (`hooks/administration.py`), puis **compile `agent.exe` par compilation croisée et le dépose là où `console` le cherche** (`scripts/build-agent-croise.sh`, `hooks/agent_payload.py`). |

**Dépendance dure** : `desk` ne peut pas s'installer sans `console`
(`nivuus-package.yaml::requires.packages`) — c'est `console` qui provisionne
la VM Windows, y déploie l'agent et l'arme en session 1. Sans elle, `desk`
installerait un service qui n'a rien à piloter.

🔴 **C'est cette dépendance, et elle seule, qui garantit la VM — jamais un
contrôle de `resolve`.** Le moteur refuse un pré-requis manquant
(`missing_dependencies`) **avant** le premier hook `resolve` et **avant**
`partition()`. `resolve` a porté, jusqu'au 30 août 2026, une porte
`hw["vm_windows"]` : une clé qu'aucun producteur du moteur ne pose
(`common/hardware.py::detect_all()` en rend huit), éprouvée à une phase où la
VM ne peut pas encore exister. Elle refusait donc toujours, et le moteur
traduit un refus en arrêt de l'installation **entière**. La porte vit
désormais dans `activate`, où elle est une mesure ;
`tests/test_desk_contrat_hw.py` fige le contrat de `hw` en lisant les clés du
**producteur** au lieu de les inventer.

## Le wizard

Quatre questions, et pas une de plus — le reste est **dérivé** (les deux
adresses TURN, le port) ou **tiré au sort** (le secret de jeton) :

| Clé | Type | Ce qu'elle pilote |
|---|---|---|
| `admin_email` | texte | le courriel du premier compte |
| `admin_password` | secret | son mot de passe |
| `auth_mode` | choix (`motdepasse` / `pomerium`) | comment les utilisateurs s'authentifient — un **mode**, jamais un armement : une valeur inconnue lève côté service |
| `vb_audio` | bool, défaut `false` | poser VB-Audio dans la VM (licence **personnelle seulement** — une option que l'opérateur arme, jamais un défaut) |

## `agent.exe` ne s'invente plus : il se recompile

`console/guest/fetch_payload.py` déclarait l'agent « extrait de la VM avant
son effacement » et « jamais récupérable autrement » — un binaire que
personne ne savait refabriquer. `scripts/build-agent-croise.sh` le
refabrique par compilation croisée (`x86_64-pc-windows-gnu`, sans la VM),
et `activate` le dépose à l'emplacement fixe où `console` le cherche
(`<NIVUUS_PACKAGES_DIR>/console/guest/payload/agent/agent.exe`) — **jamais**
sous `<drivers_dir>/agent/agent.exe`, qui dépend d'une réponse de wizard de
`console` que ce hook ne reçoit pas.

⚠️ **Ce que ce dépôt n'établit pas** : que le binaire produit par
compilation croisée FONCTIONNE. Il se lie ; c'est un produit `mingw` là où
l'ancien était bâti sur la VM en MSVC, et il n'a jamais tourné. Le juge est
une exécution sur la VM — voir la spec du chantier `package-nivuus`, § 4.3.

## 🔴 Ce que `make test` éprouve, et ce qu'il n'éprouve PAS

**Les suites de `tests/` ne remplacent `scripts/verify-all.sh` sous aucun
prétexte — elles n'éprouvent pas la même chose, et les confondre ferait
croire qu'un `make test` vert vaut recette du produit.**

| | Ce qu'il éprouve | Ce qu'il ne touche jamais |
|---|---|---|
| `make test` (ce package) | le **packaging** : le manifeste et le wizard (`nivuus-package.yaml`, `wizard.yaml`), les trois hooks (`resolve`/`install`/`activate`), le contrat de `hw` avec le moteur voisin, et le script de compilation croisée qui dépose `agent.exe` | l'agent Rust réel, la plateforme, le client, le protocole partagé — aucun d'eux ne tourne ici |
| `env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh` | le **produit** : `cargo test`/`cargo clippy` du workspace agent+proto, les suites et le typecheck du client, `design:verifier` (les huit contrôles de token/contraste/poids CSS), les suites et le typecheck de `proto/ts/`, les suites et le typecheck de la plateforme (SQLite et Postgres) | le packaging — aucune de ses dix étapes ne lit `nivuus-package.yaml`, un hook ou un wizard |

🔴 **`make test` N'EST PAS HERMÉTIQUE, ET CETTE PAGE A AFFIRMÉ LE CONTRAIRE.**
Elle disait « contre un moteur et un système de fichiers **factices** » ;
c'est faux, relevé par la revue finale de branche. Les installations 1, 2, 3,
5 et 7 de `tests/test_desk_install.py` lisent le **vrai dépôt** (elles ne
posent pas `DESK_SOURCE_RACINE`) et copient le vrai
`plateforme/node_modules` et le vrai `client/dist` ; l'installation 7 dépose
le **vrai runtime Node** de cette machine ; `tests/test_desk_manifeste.py` et
`tests/test_desk_contrat_hw.py` lisent le **vrai dépôt voisin**
`../installer`. Sur un clone frais, `make test` échoue. Ce qu'il exige :

```bash
cd plateforme && npm install     # node_modules/.bin/tsx
cd client && npm run build       # client/dist
# un node conforme a engines.node de plateforme/package.json dans le PATH
# le depot voisin installer/ a cote de desk/ (ou DESK_INSTALLER_RACINE)
```

Un `make test` vert ne dit donc **rien** de l'agent, de la plateforme, du
client ou du protocole ; un `verify-all.sh` vert ne dit **rien** du
manifeste, des hooks ou du dépôt d'`agent.exe`. Les deux sont nécessaires,
aucun ne couvre l'autre.

### La règle de découverte de `make test`

`make test` **découvre** les suites, il ne les énumère pas à la main : toute
suite de `tests/` porte le préfixe `test_`, et rien d'autre dans ce
répertoire ne le porte. `tests/desk_activate_fixtures.py` et
`tests/desk_install_fixtures.py` — les deux modules de fixtures partagées —
s'appellent délibérément SANS ce préfixe précisément pour ne pas être
découverts : ce ne sont pas des suites, les exécuter directement ne fait rien
d'utile (voir leurs propres en-têtes). Une suite ajoutée demain n'a donc rien à câbler
dans le `Makefile` : il suffit qu'elle s'appelle `test_desk_*.py`.

## Commandes

```bash
make test      # les suites de packaging de ce package (voir ci-dessus)
make help      # liste les cibles
```

Pour recetter le produit — jamais remplacé par ce qui précède :

```bash
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

Voir `CLAUDE.md` (§ Commandes) pour tout le reste de l'outillage : la VM
Windows, `scripts/build-agent.sh`, les recettes navigateur.
