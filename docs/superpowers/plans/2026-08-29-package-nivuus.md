# `desk` devient un package Nivuus : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que `desk` s'installe comme un package Nivuus sur une machine où `console` a provisionné la VM Windows, et que l'`agent.exe` dont l'appliance dépend cesse d'être un binaire que personne ne sait refabriquer.

**Architecture:** Un manifeste `userspace` déclarant `requires: packages: [console]`, un wizard de quatre questions, trois hooks qui parlent le protocole jsonl du moteur, et une compilation croisée qui produit l'agent sur l'hôte Linux. Le package porte ses propres tests, sur le modèle du `Makefile` de `console`.

**Tech Stack:** Python 3.11 (stdlib + PyYAML) pour les hooks et les tests — ni pytest, ni dépendance neuve ; Rust pour la compilation croisée (`x86_64-pc-windows-gnu`) ; Node ≥ 24 pour la plateforme ; systemd.

**Spec:** [`docs/superpowers/specs/2026-08-29-package-nivuus-design.md`](../specs/2026-08-29-package-nivuus-design.md)

> ⚠️ **UN ÉCART DE CE PLAN AVEC SA PROPRE MÉTHODE, DIT PLUTÔT QUE MASQUÉ.**
> Les tâches 1 à 3 portent le code complet — tests **et** implémentation. Les
> tâches 4 à 7 portent **leurs tests et leurs contraintes d'implémentation,
> pas leur code ligne à ligne** : les tests y définissent le comportement
> attendu (le secret tiré au sort, l'armement par lien, le redémarrage dit et
> non pris, le nom du binaire déposé), et chaque étape nomme la contrainte qui
> gouverne l'écriture. **C'est un écart assumé, pas un oubli** : écrire quatre
> hooks entiers dans un plan revient à écrire le code deux fois, et la seconde
> version vieillit sans que personne ne la relance. Un implémenteur qui trouve
> une de ces tâches sous-spécifiée doit le **dire** et demander, jamais
> deviner.

## Global Constraints

- **Français partout** : commentaires, messages d'erreur destinés à l'opérateur, messages de commit. Les identifiants techniques gardent leur forme.
- **Le nom du package est `desk`**, et il déclare `requires: packages: [console]` — tranché par le propriétaire du dépôt le 29 août 2026. ⚠️ Un nom de package devient un **répertoire** sous `/opt/nivuus-packages` et un **nom d'instance systemd** : il valide `^[a-z][a-z0-9-]{0,31}$` et ne se rattrape pas.
- 🔴 **CE PLAN NE TOUCHE PAS AU DÉPÔT `installer`.** `requires.packages` y est **déjà spécifié et planifié** (`installer/docs/superpowers/plans/2026-08-28-requires-packages-dependances.md`, 787 lignes). Ce plan s'y **conforme**. Toute tentation de « corriger au passage » un fichier d'`installer` est hors périmètre et doit remonter au propriétaire.
- ⚠️ **ORDONNANCEMENT ENTRE DEUX CHANTIERS** : tant que le plan d'`installer` n'est pas exécuté, le manifeste de `desk` est **rejeté** par le moteur — le parseur refuse ce qu'il ne comprend pas. Le package s'écrit et se teste dès maintenant ; il ne s'installe qu'après. **Aucune tâche de ce plan n'a le droit d'attendre l'autre chantier pour être jouée.**
- **500 lignes maximum** par fichier de code source. 🔴 **Relever par `wc -l`, JAMAIS recopier.**
- **Les hooks parlent jsonl sur stdout**, un objet JSON par ligne, et reçoivent `{"hw": …, "answers": …}` sur stdin. Événements : `progress`, `platform`, `facts`, `refuse` (resolve seulement), `done`. 🔴 **UN REFUS EST UNE DONNÉE, JAMAIS UNE EXCEPTION** : tout chemin qui peut échouer dans `resolve` finit par un événement `refuse` portant une phrase — une trace d'exception donne à l'opérateur un code non nul sur lequel il ne peut rien.
- **Les tests sont des scripts autonomes** : `python3 tests/<nom>.py`, sans framework, sur le modèle de `console/tests/test_console_resolve.py` — une liste `failures`, une fonction `check(label, got, want)`, `sys.exit(1)` si non vide, sinon une ligne `OK - …`.
- 🔴 **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle** : chaque garde se mute et se voit rouge. **Copie nommée** (`cp`) avant de muter, restauration **depuis elle** — jamais `git checkout --`, qui restaure HEAD et non l'état d'avant.
- **Jamais `git add -A`** — nommer les fichiers. ⚠️ Un pathspec de **répertoire** ne prend pas le module **homonyme**.
- Commencer chaque commande par `unset -f chpwd 2>/dev/null;`.
- Message de commit long **par un fichier** (`git commit -F`), jamais `-m` avec des accents graves ou des backticks.
- ⚠️ **Un `.ps1` sans BOM portant un seul caractère non-ASCII ne s'analyse pas.** Contrôle : `LC_ALL=C grep -c '[^ -~]' fichier.ps1` doit rendre **0**.

---

### Task 1: La production d'`agent.exe` — ce qui ferme le trou de l'appliance

🔴 **C'est la tâche qui justifie tout le chantier.** `console/guest/payload.py` déclare l'agent `"Guacamole agent, extracted before the wipe"`, et `fetch_payload.py` écrit `"Not fetched, and never fetchable"` : **l'appliance se reconstruit autour d'un binaire que personne ne sait refabriquer.**

**Files:**
- Create: `scripts/build-agent-croise.sh`
- Create: `tests/test_desk_build_croise.py`

**Interfaces:**
- Consumes: rien.
- Produces: `scripts/build-agent-croise.sh <destination>` — produit `agent.exe` et le copie à la destination donnée ; code 0 et fichier présent, ou message d'erreur nommant ce qui manque et code non nul. Consommé par les tâches 5 et 7.

- [ ] **Step 1: Écrire le test qui échoue**

```python
#!/usr/bin/env python3
"""Tests de la compilation croisée de l'agent.

Ce que ces tests éprouvent : que le script REFUSE proprement quand son
outillage manque, et qu'il nomme ce qui manque. Ils n'éprouvent PAS que le
binaire fonctionne — voir la tâche 9 et la spec §4.3 : la compilation prouve
que l'agent se lie, jamais qu'il tourne.

Run: python3 tests/test_desk_build_croise.py
"""
import os
import pathlib
import subprocess
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = RACINE / "scripts" / "build-agent-croise.sh"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


check("le script existe", SCRIPT.is_file(), True)
check("le script est exécutable", os.access(SCRIPT, os.X_OK), True)

# Sans destination, il doit refuser et le DIRE — jamais écrire quelque part
# par défaut : un défaut ferait déposer un binaire de 20 Mio à un endroit que
# personne n'a demandé.
r = subprocess.run(["bash", str(SCRIPT)], capture_output=True, text=True)
check("sans destination : code non nul", r.returncode != 0, True)
check("sans destination : la raison est dite",
      "destination" in (r.stdout + r.stderr).lower(), True)

# Une cible rustup absente doit être nommée, pas laissée à cargo qui rendrait
# une erreur de compilation illisible.
with tempfile.TemporaryDirectory() as tmp:
    env = dict(os.environ, CIBLE_RUST="cible-qui-n-existe-pas")
    r = subprocess.run(["bash", str(SCRIPT), tmp],
                       capture_output=True, text=True, env=env)
    check("cible inconnue : code non nul", r.returncode != 0, True)
    check("cible inconnue : elle est nommée",
          "cible-qui-n-existe-pas" in (r.stdout + r.stderr), True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests de la compilation croisée passés")
```

- [ ] **Step 2: Le voir échouer**

Run: `unset -f chpwd 2>/dev/null; python3 tests/test_desk_build_croise.py`
Expected: FAIL, « le script existe: got False, want True ».

- [ ] **Step 3: Écrire le script**

```bash
#!/usr/bin/env bash
# Produit agent.exe SANS la VM, et le dépose à la destination donnée.
#
# 🔴 POURQUOI CE SCRIPT EXISTE. `console/guest/payload.py` déclare l'agent
# « extracted before the wipe » et `fetch_payload.py` écrit « Not fetched, and
# never fetchable » : l'appliance se reconstruit aujourd'hui autour d'un
# binaire que personne ne sait refabriquer. Celui-ci le refabrique.
#
# ⚠️ CE QU'IL N'ÉTABLIT PAS : que le binaire FONCTIONNE. Il se lie ; c'est un
# produit mingw là où l'ancien était bâti sur la VM en MSVC, et il n'a jamais
# tourné. Voir la spec § 4.3 : le juge est une exécution sur la VM.
set -euo pipefail
unset -f chpwd 2>/dev/null || true

CIBLE_RUST="${CIBLE_RUST:-x86_64-pc-windows-gnu}"
DESTINATION="${1:-}"

if [ -z "${DESTINATION}" ]; then
    echo "usage : $0 <destination>   (le répertoire où déposer agent.exe)" >&2
    echo "aucune destination par défaut : un défaut déposerait 20 Mio à un" >&2
    echo "endroit que personne n'a demandé." >&2
    exit 2
fi

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${RACINE}"

if ! rustup target list --installed 2>/dev/null | grep -qx "${CIBLE_RUST}"; then
    echo "🔴 cible rustup absente : ${CIBLE_RUST}" >&2
    echo "   l'installer : rustup target add ${CIBLE_RUST}" >&2
    exit 1
fi

if [ "${CIBLE_RUST}" = "x86_64-pc-windows-gnu" ] \
   && ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    echo "🔴 éditeur de liens absent : x86_64-w64-mingw32-gcc" >&2
    echo "   l'installer : apt install gcc-mingw-w64-x86-64" >&2
    exit 1
fi

cargo build --release --target "${CIBLE_RUST}" -p agent

BINAIRE="target/${CIBLE_RUST}/release/agent.exe"
[ -f "${BINAIRE}" ] || { echo "🔴 cargo a réussi mais ${BINAIRE} est absent" >&2; exit 1; }

mkdir -p "${DESTINATION}"
cp "${BINAIRE}" "${DESTINATION}/agent.exe"
echo "agent.exe déposé : ${DESTINATION}/agent.exe ($(stat -c%s "${BINAIRE}") octets)"
```

- [ ] **Step 4: Le voir passer**

Run: `unset -f chpwd 2>/dev/null; chmod +x scripts/build-agent-croise.sh && python3 tests/test_desk_build_croise.py`
Expected: `OK - tests de la compilation croisée passés`

- [ ] **Step 5: Produire le binaire pour de vrai, une fois**

Run: `unset -f chpwd 2>/dev/null; scripts/build-agent-croise.sh /tmp/desk-agent`
Expected: une ligne `agent.exe déposé : …` avec une taille de l'ordre de 20 Mio. **Relever la taille réelle, ne pas recopier celle-ci.**

⚠️ **La taille ne prouve rien, dans les deux sens** : deux compilations de la même source rendent deux tailles, et un binaire neuf peut peser exactement autant que celui qu'il remplace. Ce qui vaut ici est **que le fichier existe et que le script l'ait dit**.

- [ ] **Step 6: Commit**

```bash
git add scripts/build-agent-croise.sh tests/test_desk_build_croise.py
git commit -F /tmp/msg-t1.txt
```

Message : `agent(croise) : le binaire que l'appliance declarait never fetchable se refabrique sur l'hote`.

---

### Task 2: Le manifeste et le wizard

**Files:**
- Create: `nivuus-package.yaml`
- Create: `wizard.yaml`
- Create: `tests/test_desk_manifeste.py`

**Interfaces:**
- Consumes: rien.
- Produces: le manifeste que le moteur lit, et quatre questions dont les clés sont consommées par les tâches 3 à 6 : `admin_email` (texte), `admin_password` (secret), `auth_mode` (choix `motdepasse`/`pomerium`), `vb_audio` (bool, défaut `false`).

- [ ] **Step 1: Écrire le test qui échoue**

```python
#!/usr/bin/env python3
"""Tests du manifeste et du wizard du package desk.

Run: python3 tests/test_desk_manifeste.py
"""
import pathlib
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
INSTALLER = RACINE.parent / "installer"
sys.path.insert(0, str(INSTALLER / "installer"))

from packages.manifest import load_manifest          # noqa: E402
from packages.wizard import load_questions           # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


m = load_manifest(str(RACINE / "nivuus-package.yaml"))

check("le nom est desk", m.name, "desk")
check("le tier est userspace", m.tier, "userspace")

# 🔴 Le tier n'est pas cosmétique : `userspace` INTERDIT de déclarer
# kernel-cmdline, modules et hugepages. console a déjà pris VFIO, le GPU et le
# NVMe ; desk n'ajoute qu'un service. La garantie est vérifiée par le moteur,
# pas promise par nous.
check("aucun module noyau", m.platform.modules, ())
check("aucune ligne de commande noyau", m.platform.kernel_cmdline, ())

# La dépendance dure : desk ne doit pas pouvoir s'installer sans console.
check("console est un pré-requis", m.packages, ("console",))

questions = load_questions(str(RACINE / "wizard.yaml"))
par_cle = {q.key: q for q in questions}

check("quatre questions, pas une de plus", len(questions), 4)
check("le courriel du compte initial", par_cle["admin_email"].type, "texte")
check("le mot de passe est un secret", par_cle["admin_password"].type, "secret")
check("le mode d'authentification est un choix", par_cle["auth_mode"].type, "choix")
check("ses deux valeurs", tuple(sorted(par_cle["auth_mode"].choices)),
      ("motdepasse", "pomerium"))

# 🔴 VB-Audio a une licence PERSONNELLE seulement : sa pose est une option
# que l'opérateur arme, jamais un défaut.
check("vb_audio est un booléen", par_cle["vb_audio"].type, "bool")
check("vb_audio est DÉSARMÉ par défaut", par_cle["vb_audio"].default, False)

# Le secret de jeton n'est PAS demandé : il se tire au sort à l'installation.
# Le demander, c'est le faire choisir court.
check("aucune question ne demande le secret de jeton",
      [q.key for q in questions if "jeton" in q.key or "token" in q.key], [])

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du manifeste et du wizard passés")
```

- [ ] **Step 2: Le voir échouer**

Run: `unset -f chpwd 2>/dev/null; python3 tests/test_desk_manifeste.py`
Expected: FAIL — le manifeste n'existe pas encore.

⚠️ **Si l'échec porte sur `m.packages` avec une erreur de parsage plutôt qu'une valeur**, c'est que le chantier `requires.packages` d'`installer` n'est pas encore exécuté. **C'est attendu** (voir les contraintes globales) : écrire le manifeste quand même, et **noter dans le rapport que ce test restera rouge jusqu'à ce que l'autre chantier passe**. Ne pas contourner en retirant la clé.

- [ ] **Step 3: Écrire le manifeste**

```yaml
apiVersion: nivuus.dev/v1
name: desk
version: 1.0.0
label: "Bureau distant Windows (WebRTC)"
tier: userspace

requires:
  # 🔴 desk ne doit pas pouvoir s'installer sans console : c'est console qui
  # provisionne la VM Windows, y déploie l'agent (guest/provision/40-agent.ps1)
  # et l'arme en session 1. Sans elle, ce package installerait un service qui
  # n'a rien à piloter.
  packages: [console]
  features: [networking]

# tier userspace : ni modules, ni kernel-cmdline, ni hugepages. console a déjà
# pris VFIO, le GPU et le NVMe ; desk n'ajoute qu'un service et sa config.

apt:
  # coturn : le relais TURN. Sans lui, la traversée NAT n'a pas de repli et la
  # cible « réseaux restrictifs » n'est pas couverte. Déclaré ICI, comme
  # console déclare firewalld et python3-jinja2 : un package porte ses propres
  # dépendances, et l'installation autonome sur une Debian ordinaire doit
  # marcher sans supposer les choix de l'assistant.
  - coturn

wizard:
  questions: wizard.yaml

hooks:
  resolve: hooks/resolve.py
  install: hooks/install.py
  activate: hooks/activate.py
```

- [ ] **Step 4: Écrire le wizard**

```yaml
# Quatre questions, et pas une de plus : tout le reste est DÉRIVÉ (les deux
# adresses TURN, le port) ou TIRÉ AU SORT (le secret de jeton). Une question de
# plus est une occasion de plus de se tromper, et l'opérateur n'a pas les
# éléments pour y répondre.
- key: admin_email
  type: texte
  label: "Adresse de courriel du premier compte"
  required: true

- key: admin_password
  type: secret
  label: "Mot de passe de ce compte"
  required: true

# 🔴 UN MODE, PAS UN ARMEMENT. Une valeur inconnue LÈVE côté service : un repli
# silencieux ferait tourner un mode sous le nom de l'autre, et l'un des deux
# sens est une OUVERTURE (en `pomerium`, l'identité arrive dans un en-tête que
# seule la garde du pair de confiance protège).
- key: auth_mode
  type: choix
  label: "Comment les utilisateurs s'authentifient"
  choices: [motdepasse, pomerium]
  default: motdepasse
  required: true

# 🔴 LICENCE PERSONNELLE SEULEMENT : la pose de VB-Audio s'arme, elle ne se
# suppose pas. Désarmée, le produit le dit déjà de lui-même — `mic: false` dans
# son message `ready`, et le bouton du navigateur ne paraît pas.
- key: vb_audio
  type: bool
  label: "Installer VB-Audio dans la VM (micro) — licence personnelle seulement"
  default: false
```

- [ ] **Step 5: Le voir passer**

Run: `unset -f chpwd 2>/dev/null; python3 tests/test_desk_manifeste.py`
Expected: `OK - tests du manifeste et du wizard passés` — **sauf** la réserve de l'étape 2 si le chantier `installer` n'est pas encore passé.

- [ ] **Step 6: La ROUGE — le tier doit vraiment interdire**

Copier le manifeste (`cp nivuus-package.yaml /tmp/manifeste.copie`), y ajouter sous `platform:` une `kernel-cmdline`, relancer le test.
Expected: **le chargement LÈVE une `ManifestError`** — un `userspace` n'a pas le droit de toucher au noyau. Restaurer **depuis la copie nommée**.

🔴 Sans ce bras, « tier: userspace » serait une déclaration d'intention et non une garantie.

- [ ] **Step 7: Commit**

Message : `package(desk) : le manifeste, quatre questions, et le tier qui INTERDIT de toucher au noyau`.

---
### Task 3: Le hook `resolve` — celui qui peut refuser avant qu'un octet touche le disque

**Files:**
- Create: `hooks/resolve.py`
- Create: `tests/test_desk_resolve.py`

**Interfaces:**
- Consumes: les clés du wizard de la tâche 2 (`admin_email`, `admin_password`, `auth_mode`, `vb_audio`).
- Produces: un événement `facts` portant `{"vm_repond": bool, "node_version": str, "turn_ecoute": str, "turn_relais": str, "port": int}` — persisté par le moteur dans `etc/nivuus/packages.json` et rendu à `activate` au premier démarrage. Ou un événement `refuse` portant sa phrase.

🔴 **`resolve` court AVANT `partition()`** : à cet instant le disque cible n'existe pas encore. C'est ce qui donne sa valeur au refus — il arrive à l'assistant, jamais sur un disque déjà effacé.

- [ ] **Step 1: Écrire les tests qui échouent**

```python
#!/usr/bin/env python3
"""Tests du hook resolve du package desk.

Le hook est éprouvé par son VRAIE interface — un sous-processus nourri de
{"hw":…, "answers":…} sur stdin, qui répond en jsonl sur stdout — et non par
un import : c'est ainsi que le moteur l'appelle, et un package doit pouvoir
tourner sur une Debian qui n'a jamais vu ce moteur.

Run: python3 tests/test_desk_resolve.py
"""
import json
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "resolve.py"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


def appeler(hw=None, answers=None, env=None):
    """Appelle le hook comme le moteur : stdin JSON, stdout jsonl."""
    contexte = json.dumps({"hw": hw or {}, "answers": answers or {}})
    r = subprocess.run([sys.executable, str(HOOK)], input=contexte,
                       capture_output=True, text=True, env=env)
    evenements = []
    for ligne in r.stdout.splitlines():
        ligne = ligne.strip()
        if not ligne:
            continue
        try:
            evenements.append(json.loads(ligne))
        except json.JSONDecodeError:
            pass          # le moteur relaie ces lignes en progress
    return r.returncode, evenements


REPONSES = {"admin_email": "a@b.c", "admin_password": "x",
            "auth_mode": "motdepasse", "vb_audio": False}


def refus(evenements):
    return [e for e in evenements if e.get("event") == "refuse"]


# --- Le refus est une DONNÉE, jamais une exception -----------------------
# Sans VM, le hook doit REFUSER avec une phrase, et sortir 0 : un code non nul
# donne à l'opérateur une trace sur laquelle il ne peut rien agir.
rc, ev = appeler(hw={"vm_windows": False}, answers=REPONSES)
r = refus(ev)
check("sans VM : un refus est émis", len(r), 1)
check("sans VM : le refus porte une phrase", bool(r and r[0].get("reason")), True)
check("sans VM : la phrase nomme la VM",
      bool(r and "vm" in r[0]["reason"].lower()), True)
check("sans VM : code de sortie 0", rc, 0)

# --- Le cas nominal : des faits, aucun refus ------------------------------
rc, ev = appeler(hw={"vm_windows": True}, answers=REPONSES)
check("avec VM : aucun refus", refus(ev), [])
check("avec VM : code de sortie 0", rc, 0)
faits = [e for e in ev if e.get("event") == "facts"]
check("avec VM : un événement facts", len(faits), 1)
mesures = faits[0]["facts"] if faits else {}
check("les DEUX adresses TURN sont dérivées",
      all(k in mesures for k in ("turn_ecoute", "turn_relais")), True)

# 🔴 docker-compose.coturn.yml exige TURN_LISTENING_IP **et** TURN_RELAY_IP,
# toutes deux obligatoires : borner la seule écoute laisserait les allocations
# de relais sur toutes les interfaces — mesuré le 21 août 2026, 23 adresses
# distinctes dont l'adresse publique.

# --- Le mode pomerium fait mordre ses gardes AVANT l'installation --------
rc, ev = appeler(hw={"vm_windows": True},
                 answers={**REPONSES, "auth_mode": "pomerium"})
r = refus(ev)
check("pomerium sans proxy déclaré : refus", len(r), 1)
check("pomerium : le refus nomme le proxy de confiance",
      bool(r and "proxy" in r[0]["reason"].lower()), True)

# --- Une valeur de mode inconnue LÈVE, elle ne se replie pas -------------
rc, ev = appeler(hw={"vm_windows": True},
                 answers={**REPONSES, "auth_mode": "motdepass"})
check("mode inconnu : refus", len(refus(ev)), 1)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook resolve passés")
```

- [ ] **Step 2: Le voir échouer**

Run: `unset -f chpwd 2>/dev/null; python3 tests/test_desk_resolve.py`
Expected: FAIL — le hook n'existe pas.

- [ ] **Step 3: Écrire le hook**

Il lit `{"hw":…, "answers":…}` sur stdin, écrit un objet JSON par ligne sur stdout, et **finit toujours par un `refuse` ou par un `facts`**. Points imposés :

- Le refus est une **donnée** : chaque chemin d'échec émet `{"event":"refuse","reason":"…"}` et sort **0**.
- Node est vérifié contre `>=24.0.0 <25.0.0` — la valeur que `plateforme/package.json` déclare dans son `engines`. 🔴 **La relire dans le fichier, ne pas la recopier depuis ce plan** : un intervalle recopié survit à la réalité qu'il décrivait.
- Les deux adresses TURN sont **dérivées** des interfaces, jamais demandées.
- En mode `pomerium`, l'absence d'un proxy de confiance déclaré est un **refus**, parce que `lireConfig` refuserait de démarrer et que l'opérateur l'apprendrait alors après l'installation.
- Un `auth_mode` inconnu est un **refus**, jamais un repli sur `motdepasse`.

- [ ] **Step 4: Le voir passer**

Run: `unset -f chpwd 2>/dev/null; python3 tests/test_desk_resolve.py`
Expected: `OK - tests du hook resolve passés`

- [ ] **Step 5: La ROUGE du refus — le voir refuser une machine réelle**

Run: `unset -f chpwd 2>/dev/null; echo '{"hw":{},"answers":{}}' | python3 hooks/resolve.py`
Expected: un `{"event":"refuse","reason":"…"}` lisible, **code 0**.

🔴 **Puis muter le hook pour qu'il lève une exception à la place** (copie nommée d'abord), et constater ce que l'opérateur verrait : une trace et un code non nul. Restaurer depuis la copie. C'est ce bras qui donne son sens à « un refus est une donnée ».

- [ ] **Step 6: Commit**

Message : `package(desk) : le hook resolve, et son refus qui arrive AVANT que le disque soit touche`.

---

### Task 4: Le hook `install` — ce qui se pose sur l'hôte

**Files:**
- Create: `hooks/install.py`
- Create: `hooks/assets/desk-plateforme.service`
- Create: `tests/test_desk_install.py`

**Interfaces:**
- Consumes: les `facts` de la tâche 3.
- Produces: sous la racine cible — `/opt/nivuus/desk/` (plateforme, `client/dist`), `/etc/nivuus/desk.env` (mode **600**), `/etc/systemd/system/desk-plateforme.service`, et la configuration coturn. Consommé par la tâche 5.

- [ ] **Step 1: Écrire les tests qui échouent**

Les tests posent une racine temporaire et vérifient, après appel du hook :

```python
# Le secret de jeton est TIRÉ AU SORT, jamais demandé ni constant.
env1 = lire_env(racine1)
env2 = lire_env(racine2)          # deux installations distinctes
check("le secret fait au moins 32 caractères",
      len(env1["PLATEFORME_SECRET_JETON"]) >= 32, True)
check("deux installations ne partagent pas le secret",
      env1["PLATEFORME_SECRET_JETON"] == env2["PLATEFORME_SECRET_JETON"], False)

# 🔴 Un secret qu'on demande à un opérateur est un secret qu'il choisit court ;
# et un défaut aléatoire À CHAQUE DÉMARRAGE invaliderait toutes les sessions —
# c'est la raison pour laquelle PLATEFORME_SECRET_JETON n'a aucun défaut. Il se
# tire UNE FOIS, à l'installation, et se persiste.

check("le fichier d'environnement est en 600",
      oct(os.stat(chemin_env).st_mode & 0o777), "0o600")

# PLATEFORME_PAGE est ce qui rend nginx facultatif : la plateforme sert
# elle-même la page bâtie depuis le 22 août 2026.
check("la page est servie depuis client/dist",
      env1["PLATEFORME_PAGE"].endswith("client/dist"), True)

# 🔴 PLATEFORME_HOTE : en mode pomerium, les quatre écoutes universelles font
# REFUSER le démarrage. L'installation ne doit jamais en poser une.
check("l'écoute n'est jamais universelle",
      env1["PLATEFORME_HOTE"] in ("0.0.0.0", "::", "[::]", "*"), False)
```

- [ ] **Step 2: Le voir échouer**, puis **Step 3: écrire le hook et l'unité systemd**, puis **Step 4: le voir passer**

L'unité `desk-plateforme.service` lance `npm start` depuis `/opt/nivuus/desk/plateforme`, avec `EnvironmentFile=/etc/nivuus/desk.env`, `Restart=on-failure`, et un utilisateur dédié.

- [ ] **Step 5: La ROUGE — le secret court doit être refusé par le service lui-même**

Poser à la main un `PLATEFORME_SECRET_JETON` de 8 caractères dans le fichier d'environnement, démarrer le service.
Expected: **il refuse de démarrer**, et le dit. Restaurer le secret tiré au sort.

🔴 C'est le contrôle qui prouve que la garde du produit couvre l'erreur de l'installateur, et pas seulement celle de l'opérateur.

- [ ] **Step 6: Commit**

Message : `package(desk) : le hook install — un secret tire au sort, un fichier en 600, une ecoute jamais universelle`.

---

### Task 5: Le hook `activate` — armer ce qu'`install` n'a fait que poser

**Files:**
- Create: `hooks/activate.py`
- Create: `tests/test_desk_activate.py`

**Interfaces:**
- Consumes: les `facts` de la tâche 3, les fichiers de la tâche 4, `scripts/build-agent-croise.sh` de la tâche 1.
- Produces: le service armé, le compte initial créé, l'agent enrôlé (`AGENT_VM`, `AGENT_SECRET` écrits dans le fichier d'environnement).

⚠️ **DÉPENDANCE D'ORDRE AVEC LA TÂCHE 6, à ne pas découvrir en la lisant** : `activate` appellera `hooks/vm.py::poser_projfs()` et `poser_vb_audio()`, que la **tâche 6** produit. Cette tâche-ci s'arrête **avant** ce câblage : elle arme le service, crée le compte et enrôle l'agent, rien de plus. C'est la tâche 6 qui ajoute les deux appels et les éprouve — un `activate` qui importerait un module inexistant ne serait pas testable, et la tâche 5 doit l'être seule.

- [ ] **Step 1: Écrire les tests qui échouent**

```python
# 🔴 L'ARMEMENT EST UN SYMLINK, JAMAIS `systemctl enable`. systemctl échoue en
# SILENCE dans un environnement contraint — une sous-commande de requête
# n'imprime rien — donc un `enable` qui « rend la main » ne dit rien. Un lien
# existe ou lève. Neuf entrées de ce même hôte sont des fichiers RÉGULIERS que
# systemd ignore avec « is not a symlink, ignoring ».
check("le service est armé par un lien", os.path.islink(lien), True)
check("le lien pointe vers une unité qui existe",
      os.path.exists(os.path.realpath(lien)), True)

# Le mot de passe ne transite que par stdin : jamais dans argv, donc jamais
# dans la table des processus ni dans l'historique du shell.
check("aucun mot de passe dans la ligne de commande",
      any("--password" in c or reponses["admin_password"] in " ".join(c)
          for c in commandes_vues), False)
```

- [ ] **Step 2: Le voir échouer**, **Step 3: écrire le hook**, **Step 4: le voir passer**

Le hook, dans l'ordre : arme par lien → `daemon-reload` et démarrage → crée le compte (`npm run admin:utilisateur`, mot de passe **sur stdin**) → enrôle l'agent (`npm run admin:agent`) et écrit `AGENT_VM`/`AGENT_SECRET`. ⚠️ `--vm` attend l'**identifiant**, pas le nom, et `--adresse` n'a **aucun défaut**.

- [ ] **Step 5: La ROUGE — un lien vers une unité absente doit lever**

Faire pointer le lien vers un nom d'unité qui n'existe pas.
Expected: le hook **lève** au lieu de créer un lien mort. 🔴 Sans ce bras, « une unité qui a l'air armée » et « une unité armée » seraient indiscernables.

- [ ] **Step 6: Commit**

Message : `package(desk) : le hook activate — un lien plutot qu un enable qui ment, le mot de passe par stdin seul`.

---

### Task 6: Ce que `desk` pose DANS la VM, par le chemin WinRM de `console`

**Files:**
- Create: `hooks/vm.py`
- Create: `tests/test_desk_vm.py`

**Interfaces:**
- Consumes: `/opt/nivuus-packages/console/guest/winrm_exec.py` — **contrat inter-packages**, légitime parce que le manifeste déclare `requires: packages: [console]`.
- Produces: `poser_projfs()` et `poser_vb_audio()`, appelées par `activate` (tâche 5).

- [ ] **Step 1: Écrire les tests qui échouent**

```python
# ProjFS exige un REDÉMARRAGE de la VM. L'activation le CONSTATE et le DIT ;
# elle ne le déclenche pas : redémarrer la VM d'un opérateur sans le lui
# demander est un effet de bord qu'aucune installation ne doit prendre.
etat = poser_projfs(executer=faux_winrm_rendant("RestartNeeded"))
check("le redémarrage requis est rapporté", etat.redemarrage_requis, True)
check("aucun redémarrage n'a été demandé",
      any("Restart-Computer" in c for c in faux.commandes), False)

# VB-Audio : licence PERSONNELLE seulement. Désarmé, rien ne part vers la VM —
# pas seulement « rien ne s'installe ».
faux = faux_winrm_rendant("")
poser_vb_audio(armee=False, executer=faux)
check("désarmé : aucune commande n'atteint la VM", faux.commandes, [])
```

- [ ] **Step 2: Le voir échouer**, **Step 3: écrire le module**, **Step 4: le voir passer**

`poser_projfs` lance `Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS -NoRestart` et lit l'état rendu. `poser_vb_audio` ne fait **rien** si `armee` est faux.

⚠️ **Le chemin de `winrm_exec.py` est résolu, jamais supposé** : s'il est absent, la fonction lève avec une phrase nommant le chemin cherché — un package qui échoue en silence sur un contrat inter-packages est indiscernable d'un package qui n'a rien à faire.

- [ ] **Step 5: La ROUGE — le contrat inter-packages doit se voir manquer**

Appeler avec un chemin de `winrm_exec.py` inexistant.
Expected: une exception nommant le chemin. 🔴 C'est le bras qui distingue « console absent » de « rien à faire ».

- [ ] **Step 6: Commit**

Message : `package(desk) : ProjFS et VB-Audio poses par le chemin WinRM de console, et le redemarrage DIT plutot que pris`.

---

### Task 7: Déposer `agent.exe` là où `console` va le chercher

**Files:**
- Modify: `hooks/activate.py`
- Create: `tests/test_desk_payload.py`

**Interfaces:**
- Consumes: `scripts/build-agent-croise.sh` (tâche 1).
- Produces: `agent.exe` dans le sous-répertoire `agent/` du répertoire de payload de `console`.

🔴 **LE CHEMIN DU PAYLOAD N'EST PAS CODÉ EN DUR CHEZ `console`** : `fetch_payload.py` le reçoit par `--drivers-dir`, et `40-agent.ps1` par un `-PayloadRoot` obligatoire. Cette tâche doit donc **établir par où `desk` apprend ce chemin** — le lire dans l'état du package `console` (`etc/nivuus/packages.json`), ou le recevoir en paramètre — et **écrire ce qu'elle a trouvé**. Inventer un chemin serait recopier une constante qui n'existe pas.

- [ ] **Step 1: Chercher, et écrire ce qu'on a trouvé**

```bash
unset -f chpwd 2>/dev/null
grep -rn "drivers-dir\|PayloadRoot\|drivers_dir" \
  /home/mallanic/Projects/Nivuus/packages/installer/console/ | head -20
```

Le rapport doit dire **où le chemin vit à l'exécution**, pas seulement où il est nommé dans le code.

- [ ] **Step 2: Écrire le test qui échoue**, **Step 3: implémenter**, **Step 4: le voir passer**

Le test vérifie que le fichier déposé s'appelle `agent.exe` et qu'il est dans le sous-répertoire `agent/` — le nom exact que `payload.py::REQUIRED_BINARIES` déclare.

- [ ] **Step 5: La ROUGE — un dépôt sous un autre nom doit être vu**

Déposer le binaire sous `agent-exe` ou dans `drivers/` directement.
Expected: le test rougit. 🔴 Le nom est un contrat avec `console`, pas une convention interne.

- [ ] **Step 6: Commit**

---

### Task 8: Le `Makefile` du package, et son `README`

**Files:**
- Create: `Makefile`
- Create: `README-package.md`

- [ ] **Step 1: Écrire le `Makefile`** sur le modèle de `console/Makefile` — une cible `test` qui joue les suites de `tests/`, une cible `help`.

🔴 **Ces suites ne remplacent pas `scripts/verify-all.sh`** : elles éprouvent le **packaging**, pas le produit. Le `README-package.md` doit le dire, et dire lequel joue quoi.

- [ ] **Step 2: Jouer les deux**

```bash
unset -f chpwd 2>/dev/null
make test
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

Expected: les suites du package passent ; les dix étapes du produit restent vertes. ⚠️ Si `cargo test` rougit sur un `testdata` introuvable : `cargo clean -p agent -p proto` d'abord — le piège du chemin périmé, payé le 28 août 2026.

- [ ] **Step 3: Commit**

---

### Task 9: La revue transverse, et ce que le lot laisse dû

**Files:**
- Create: `docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Relire chaque message de commit CONTRE son diff**

🔴 **Un message de commit est une pièce du dépôt, et personne ne le relit.**

- [ ] **Step 2: Relever les tailles APRÈS la dernière édition de la ronde**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

- [ ] **Step 3: Écrire le document de résultats**

Il porte, en propre, un § « **Ce que ce lot n'établit PAS** » disant au minimum :

- 🔴 **que l'agent bâti en croisé FONCTIONNE** — il se lie, il n'a jamais tourné, et c'est un produit **mingw** là où l'ancien était bâti sur la VM ;
- que **l'installation n'a jamais été jouée de bout en bout** sur une machine neuve, si c'est le cas ;
- qu'**aucun des douze items du lot 3** n'est mesuré.

- [ ] **Step 4: Mettre `CLAUDE.md` à jour**

Trois faits que le dépôt ne dit **nulle part** aujourd'hui, et qui ont coûté une demi-journée à retrouver :

1. **La VM cible est une APPLIANCE** provisionnée par `packages/installer` — plus de `C:\dev`, plus de Rust, plus de montage CIFS, **retirés délibérément**.
2. **WinRM n'accepte plus Basic** : `scripts/winrm.js` (compte `Administrateur`, transport Basic) **ne fonctionne plus**, et le chemin qui marche est `console/guest/winrm_exec.py` en **NTLM**, mot de passe dans `/root/.config/nivuus/windows-admin.pass`.
3. **Le lot 3 est suspendu**, avec la raison, et sa spec et son plan restent valides pour le jour où la VM sera équipée.

⚠️ **Le sort de `scripts/winrm.js`, de `/media/vm` et de `scripts/build-agent.sh`** — qui visent tous une VM de développement qui n'existe plus — **n'est PAS tranché par ce plan**. Les corriger, les retirer ou les garder est une décision du propriétaire du dépôt : la nommer dans le document, ne pas la prendre.

- [ ] **Step 5: Commit, puis proposer la fusion**

⚠️ **La fusion et la poussée appartiennent au propriétaire du dépôt.**

---

## Ce que ce plan ne couvre pas

- **`requires.packages` dans `installer`** — déjà spécifié et planifié là-bas ; ce plan s'y conforme et ne le réimplémente pas.
- **Les douze items du lot 3** — ils attendent une VM équipée, que ce chantier rend atteignable sans la mesurer.
- **La reconstruction complète d'une appliance de bout en bout** : ce plan produit l'agent et l'installe, il ne rejoue pas le provisioning de `console`.
