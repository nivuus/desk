# desk — le bureau distant Windows, en package Nivuus

Ce répertoire est un **package Nivuus** (`nivuus.dev/v1`) : le moteur
d'installation le découvre, l'offre dans l'assistant, et l'installe par les
trois mêmes phases que n'importe quel package tiers. Il n'a rien de
spécial — c'est le point : si l'API ne suffisait pas pour lui, elle ne
suffirait pour personne.

| Phase | Ce qu'elle fait |
|---|---|
| `resolve` | Lecture seule. Dérive l'adresse d'écoute, le port et l'origine CORS de la machine cible ; **refuse**, avec une raison, une machine où ces éléments ne peuvent pas être dérivés. Émet un événement `refuse` et sort en code 0 — jamais une exception non rattrapée (voir `hooks/resolve.py`). |
| `install` | Pose les fichiers sur la cible : la plateforme (`plateforme/`) et le client bâti (`client/dist`) sous `/opt/nivuus/desk/`, l'unité systemd `desk-plateforme.service`, `desk.env` (secrets tirés au sort, jamais demandés au wizard), `turnserver.conf`. N'a **aucun** canal `refuse` : une erreur ici est une anomalie, pas une décision à motiver (voir `hooks/install.py`). |
| `activate` | Arme le lien systemd que `install` n'a fait que poser, crée le compte administrateur initial et enrôle l'agent auprès de la plateforme (`hooks/administration.py`), puis **compile `agent.exe` par compilation croisée et le dépose là où `console` le cherche** (`scripts/build-agent-croise.sh`, `hooks/activate.py::deposer_agent_console`) — sans jamais toucher la VM Windows. |

**Dépendance dure** : `desk` ne peut pas s'installer sans `console`
(`nivuus-package.yaml::requires.packages`) — c'est `console` qui provisionne
la VM Windows, y déploie l'agent et l'arme en session 1. Sans elle, `desk`
installerait un service qui n'a rien à piloter.

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
| `make test` (ce package) | le **packaging** : le manifeste et le wizard (`nivuus-package.yaml`, `wizard.yaml`), les trois hooks (`resolve`/`install`/`activate`) contre un moteur et un système de fichiers **factices**, et le script de compilation croisée qui dépose `agent.exe` | l'agent Rust réel, la plateforme, le client, le protocole partagé — aucun d'eux ne tourne ici |
| `env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh` | le **produit** : `cargo test`/`cargo clippy` du workspace agent+proto, les suites et le typecheck du client, `design:verifier` (les huit contrôles de token/contraste/poids CSS), les suites et le typecheck de `proto/ts/`, les suites et le typecheck de la plateforme (SQLite et Postgres) | le packaging — aucune de ses dix étapes ne lit `nivuus-package.yaml`, un hook ou un wizard |

Un `make test` vert ne dit donc **rien** de l'agent, de la plateforme, du
client ou du protocole ; un `verify-all.sh` vert ne dit **rien** du
manifeste, des hooks ou du dépôt d'`agent.exe`. Les deux sont nécessaires,
aucun ne couvre l'autre.

### La règle de découverte de `make test`

`make test` **découvre** les suites, il ne les énumère pas à la main : toute
suite de `tests/` porte le préfixe `test_`, et rien d'autre dans ce
répertoire ne le porte. `tests/desk_activate_fixtures.py` — le module de
fixtures partagées par `test_desk_activate.py` — s'appelle délibérément
SANS ce préfixe précisément pour ne pas être découvert : ce n'est pas une
suite, `python3 tests/desk_activate_fixtures.py` ne fait rien d'utile
(voir son propre en-tête). Une suite ajoutée demain n'a donc rien à câbler
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
