# Réveil de la VM au lancement d'une app, arrêt après 30 min d'inactivité — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal :** lancer une app depuis le hub réveille la VM Windows éteinte ; la VM s'éteint après 30 min sans session Moonlight ni fenêtre d'app ouverte (garde CPU conservé).

**Architecture :** `console` (dépôt `installer`) expose un socket Unix de contrôle (`wake`, `busy`) ; la plateforme (`desk`) l'appelle depuis la route de lancement d'app et tant qu'une fenêtre est ouverte ; le hub rejoue le lancement jusqu'à ce que l'agent soit revenu. Le script d'arrêt existant gagne une troisième condition d'activité.

**Tech Stack :** bash + systemd (socket activation, sysusers.d) ; Python (hooks, tests de l'installeur) ; TypeScript/Node (`node:net`, `vitest`) ; aucune dépendance nouvelle.

**Spec :** `docs/superpowers/specs/2026-09-30-vm-reveil-arret-design.md` (lire en entier, y compris les trois corrections datées du 30 septembre 2026).

Deux dépôts, deux branches, deux PR :

| Dépôt | Chemin | Branche | Tâches |
| --- | --- | --- | --- |
| `installer` | `~/Projects/Nivuus/packages/installer` | `feat/vm-control-socket` | 1–3 |
| `desk` | `~/Projects/Nivuus/packages/desk` | `feat/vm-wake-on-launch` | 0, 4–10 |

## Global Constraints

- Code, commentaires, identifiants nouveaux et lignes de journal **en anglais** (règle globale) ; les noms déjà existants (`Orchestrateur`, `Resultat`, `refuser`, `lancerApplication`…) restent tels quels.
- **Exception décidée par le propriétaire (spec §3.4) :** le message d'attente du hub est en français, comme les libellés voisins ; le client lit `etat` (valeur technique), jamais le texte.
- Aucun fichier source > 500 lignes ; `serveur.ts` (467 l.) ne gagne que l'import et ≤ 6 lignes de câblage, le reste vit dans `orchestration/host-wiring.ts`.
- Pas de `catch` muet, pas de retry masqué : tout échec du canal est un `Resultat` refusé ou un `warn`.
- Un utilisateur ne réveille que **sa** VM : le réveil n'est demandé qu'après la garde d'appartenance déjà en place dans `routes-applications.ts`.
- `busy` toutes les **60 s** (`BUSY_PERIOD_MS`) ↔ `APP_HEARTBEAT_S=60` ; fraîcheur max **180 s** (`APP_ACTIVITY_MAX_AGE_S = 3 × APP_HEARTBEAT_S`) ; attente du hub **180 s** max, relance toutes les **3 s**.
- Seules les sessions nommées `w-<N>` (après le préfixe) comptent comme « session d'app » : `bureau` et `fichiers` sont appariées dès que le hub est ouvert.
- Git : remotes en `https://` ; titres de PR en **anglais**, conventional commits ; **squash** sur `installer` (branche protégée).
- Vérification desk : `cd plateforme && npx tsc --noEmit && npx vitest run` ; client : `cd client && npx tsc --noEmit && npx vitest run`.

## Review Focus

Entrées ou pannes que la spec implique et qu'aucun test « nominal » ne couvre ; chacune est épinglée par un test de la tâche indiquée.

1. **Onglet de hub ouvert sans aucune fenêtre** → aucun `busy` (sinon la VM ne s'éteint jamais) — Tâche 8.
2. **Deux lancements d'app simultanés VM éteinte** → un seul `wake` envoyé — Tâche 6.
3. **Socket absent, refusé ou sans écouteur** → refus typé `hote-inaccessible`, jamais une exception ni un blocage — Tâche 5.
4. **`504 delai` ou tout autre refus que `503 + etat: demarrage`** → le hub ne rejoue **pas** le lancement (risque de doublon d'app) — Tâche 9.
5. **`app-activity` illisible ou daté du futur** (horloge) → illisible = pas d'activité, journalisé ; futur = actif (côté sûr) — Tâche 3.

---

### Task 0: Branche desk, spec et plan

**Files:**
- Create (déjà écrits, à committer) : `docs/superpowers/specs/2026-09-30-vm-reveil-arret-design.md`, `docs/superpowers/plans/2026-09-30-vm-reveil-arret.md`

**Interfaces:**
- Produces : la branche `feat/vm-wake-on-launch` sur laquelle les tâches 4–10 committent.

- [ ] **Step 1 : créer la branche depuis `main`**

```bash
cd ~/Projects/Nivuus/packages/desk
git checkout main && git pull --ff-only
git checkout -b feat/vm-wake-on-launch
git status --short   # la spec et le plan doivent apparaître en « ?? »
```

- [ ] **Step 2 : commit**

```bash
git add docs/superpowers/specs/2026-09-30-vm-reveil-arret-design.md docs/superpowers/plans/2026-09-30-vm-reveil-arret.md
git commit -m "docs: design and plan for waking the VM on app launch and idle shutdown

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## PARTIE A — dépôt `installer`

### Task 1: le traitement des requêtes du canal (`vm-control.sh`)

**Files:**
- Create: `console/host/vm-control.sh`
- Create: `console/tests/test_vm_control.sh`

**Interfaces:**
- Produces : un exécutable qui lit **une ligne** sur stdin et écrit **une ligne** sur stdout : `ok`, ou `err <cause>` avec `cause ∈ {no-request, unknown-verb, wake-failed, busy-failed}`. Variable d'environnement `VM_IDLE_STATE_DIR` (défaut `/run/nivuus-vm-idle`) ; fichier écrit : `$VM_IDLE_STATE_DIR/app-activity` (une date epoch, en secondes). Nom d'unité réveillée : `nivuus-vm-wake.service`.

- [ ] **Step 1 : créer la branche**

```bash
cd ~/Projects/Nivuus/packages/installer
git checkout main && git pull --ff-only
git checkout -b feat/vm-control-socket
```

- [ ] **Step 2 : écrire le test qui échoue** — `console/tests/test_vm_control.sh`

```bash
#!/bin/bash
# Tests for console/host/vm-control.sh against a fake systemctl / logger.
#
# The script is the per-connection handler of nivuus-vm-control.socket: one
# request line on stdin, one reply line on stdout. The verb list is an
# allow-list and no verb takes an argument.

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="$SCRIPT_DIR/../host/vm-control.sh"

PASS=0
FAIL=0
pass() { echo "  ✓ $1"; PASS=$((PASS + 1)); }
fail() { echo "  ✗ $1"; FAIL=$((FAIL + 1)); }

# $1 = exit code of the fake systemctl
setup_sandbox() {
    SANDBOX=$(mktemp -d)
    mkdir -p "$SANDBOX/bin" "$SANDBOX/state"
    cat > "$SANDBOX/bin/systemctl" <<EOF
#!/bin/bash
echo "\$*" >> "$SANDBOX/calls"
exit $1
EOF
    cat > "$SANDBOX/bin/logger" <<EOF
#!/bin/bash
echo "\$*" >> "$SANDBOX/log"
EOF
    chmod +x "$SANDBOX/bin/"*
    : > "$SANDBOX/calls"
    : > "$SANDBOX/log"
}
teardown_sandbox() { rm -rf "$SANDBOX"; }

# $1 = the bytes the client sends (printf %b syntax). Sets REPLY_LINE.
run_target() {
    REPLY_LINE=$(printf '%b' "$1" \
        | VM_IDLE_STATE_DIR="$SANDBOX/state" PATH="$SANDBOX/bin:$PATH" bash "$TARGET")
}

echo "== vm-control.sh =="

echo "[1] wake -> starts the wake unit without blocking"
setup_sandbox 0
run_target 'wake\n'
[ "$REPLY_LINE" = "ok" ] && pass "replies ok" || fail "reply was '$REPLY_LINE'"
grep -qx "start --no-block nivuus-vm-wake.service" "$SANDBOX/calls" \
    && pass "runs systemctl start --no-block nivuus-vm-wake.service" \
    || fail "systemctl calls: $(cat "$SANDBOX/calls")"
teardown_sandbox

echo "[2] wake when systemctl fails -> err wake-failed"
setup_sandbox 1
run_target 'wake\n'
[ "$REPLY_LINE" = "err wake-failed" ] && pass "replies err wake-failed" || fail "reply was '$REPLY_LINE'"
teardown_sandbox

echo "[3] busy -> records a fresh epoch timestamp"
setup_sandbox 0
run_target 'busy\n'
[ "$REPLY_LINE" = "ok" ] && pass "replies ok" || fail "reply was '$REPLY_LINE'"
TS=$(cat "$SANDBOX/state/app-activity" 2>/dev/null)
NOW=$(date +%s)
if [[ "$TS" =~ ^[0-9]+$ ]] && [ $((NOW - TS)) -le 5 ] && [ $((NOW - TS)) -ge -1 ]; then
    pass "app-activity holds the current epoch ($TS)"
else
    fail "app-activity content is '$TS' (now=$NOW)"
fi
teardown_sandbox

echo "[4] unknown verb -> refused, nothing executed"
setup_sandbox 0
run_target 'reboot\n'
[ "$REPLY_LINE" = "err unknown-verb" ] && pass "replies err unknown-verb" || fail "reply was '$REPLY_LINE'"
[ ! -s "$SANDBOX/calls" ] && pass "systemctl not called" || fail "systemctl was called"
teardown_sandbox

echo "[5] a verb with an argument is refused (no verb takes one)"
setup_sandbox 0
run_target 'wake extra\n'
[ "$REPLY_LINE" = "err unknown-verb" ] && pass "replies err unknown-verb" || fail "reply was '$REPLY_LINE'"
[ ! -s "$SANDBOX/calls" ] && pass "systemctl not called" || fail "systemctl was called"
teardown_sandbox

echo "[6] empty input -> err no-request"
setup_sandbox 0
run_target ''
[ "$REPLY_LINE" = "err no-request" ] && pass "replies err no-request" || fail "reply was '$REPLY_LINE'"
teardown_sandbox

echo "[7] over-long line without newline -> refused, nothing executed"
setup_sandbox 0
run_target 'wakewakewakewakewakewakewake'
[ "$REPLY_LINE" = "err unknown-verb" ] && pass "replies err unknown-verb" || fail "reply was '$REPLY_LINE'"
[ ! -s "$SANDBOX/calls" ] && pass "systemctl not called" || fail "systemctl was called"
teardown_sandbox

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" -eq 0 ]
```

- [ ] **Step 3 : lancer, vérifier l'échec**

Run: `bash console/tests/test_vm_control.sh`
Expected: échecs (le script cible n'existe pas : `bash: .../vm-control.sh: No such file`), sortie non nulle.

- [ ] **Step 4 : écrire `console/host/vm-control.sh`**

```bash
#!/bin/bash
# Control channel for the Windows VM.
#
# Run once per connection by nivuus-vm-control@.service (systemd socket
# activation, Accept=yes): one request line on stdin, one reply line on
# stdout. The verbs are an allow-list and NONE takes an argument - the VM is
# always the one this host owns, so a client cannot name another target.
#
#   wake -> ask systemd to start nivuus-vm-wake.service (handle-vm-start.sh).
#           --no-block: handle-vm-start.sh waits up to 180 s for the guest IP,
#           and the caller only needs to know the start was REQUESTED.
#   busy -> record "a desk app window is open right now" for
#           vm-idle-shutdown.sh, which reads it on its next pass.

STATE_DIR="${VM_IDLE_STATE_DIR:-/run/nivuus-vm-idle}"
ACTIVITY_FILE="$STATE_DIR/app-activity"
WAKE_UNIT="nivuus-vm-wake.service"
LOG_TAG="vm-control"

reply() { printf '%s\n' "$1"; }

# 17 bytes: one more than the longest verb + newline would need, so an
# over-long request is cut instead of buffered without bound.
if ! IFS= read -r -t 5 -n 17 verb; then
    reply "err no-request"
    exit 0
fi

case "$verb" in
    wake)
        if out=$(systemctl start --no-block "$WAKE_UNIT" 2>&1); then
            logger -t "$LOG_TAG" "wake requested"
            reply "ok"
        else
            logger -t "$LOG_TAG" "wake failed: ${out:0:200}"
            reply "err wake-failed"
        fi
        ;;
    busy)
        # Write-then-rename: the idle check must never read a half-written file.
        if mkdir -p "$STATE_DIR" \
            && date +%s > "$ACTIVITY_FILE.tmp" \
            && mv "$ACTIVITY_FILE.tmp" "$ACTIVITY_FILE"; then
            reply "ok"
        else
            logger -t "$LOG_TAG" "busy: cannot write $ACTIVITY_FILE"
            reply "err busy-failed"
        fi
        ;;
    *)
        reply "err unknown-verb"
        ;;
esac
exit 0
```

```bash
chmod +x console/host/vm-control.sh console/tests/test_vm_control.sh
```

- [ ] **Step 5 : lancer, vérifier le succès**

Run: `bash console/tests/test_vm_control.sh`
Expected: `passed=14 failed=0` (ou le total affiché), code de sortie 0.

- [ ] **Step 6 : enregistrer le test là où les autres tests shell sont joués**

Run: `grep -rn "test_handle_vm_start" --include='*' . 2>/dev/null | grep -v '^./console/tests/test_handle_vm_start.sh'`
Ajouter `test_vm_control.sh` **à côté de chaque occurrence** trouvée (CI, Makefile, `docs/claude/dev-commands.md`). Si aucune occurrence, l'écrire dans `docs/claude/dev-commands.md` sous « host script tests ».

- [ ] **Step 7 : commit**

```bash
git add console/host/vm-control.sh console/tests/test_vm_control.sh docs/claude/dev-commands.md
git commit -m "feat(console): add the VM control request handler (wake, busy)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: unités systemd, groupe, pose et armement

**Files:**
- Create: `console/host/systemd/nivuus-vm-control.socket`, `console/host/systemd/nivuus-vm-control@.service`, `console/host/systemd/nivuus-vm-wake.service`, `console/host/sysusers/nivuus-vm.conf`
- Create: `console/tests/test_console_vm_control_units.py`
- Modify: `console/hooks/install.py` (`HOST_SCRIPTS` l.87, `UNITS` l.97, boucle de pose l.200-213), `console/hooks/activate.py` (`WANTS` l.66, `main` l.292+), `console/tests/test_console_install.py` (~l.160-215), `console/tests/test_console_activate.py` (l.48 et l.118), `console/tests/test_console_host_files.py` (~l.35)

**Interfaces:**
- Consumes : `vm-control.sh` de la tâche 1 (chemin posé : `/usr/local/sbin/vm-control.sh`).
- Produces : socket `/run/nivuus/vm-control.sock`, mode `0660`, groupe `nivuus-vm`. Groupe créé par `usr/lib/sysusers.d/nivuus-vm.conf`.

- [ ] **Step 1 : écrire le test d'unités (échoue)** — `console/tests/test_console_vm_control_units.py`

```python
#!/usr/bin/env python3
"""The control-channel units must agree with the handler and with each other.

A socket whose group does not match the sysusers entry, or a wake unit that
no longer matches the name the handler starts, fails in a way nothing
reports: the platform just sees 'permission denied' or a wake that never
happens.
"""
import configparser
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
HOST = os.path.join(ROOT, "console", "host")
UNITS = os.path.join(HOST, "systemd")

failures = []


def check(label, condition):
    if not condition:
        failures.append(label)


def load_unit(path):
    # strict=False: systemd tolerates a repeated key. optionxform=str:
    # configparser lowercases keys, systemd directives are case-sensitive.
    parser = configparser.ConfigParser(strict=False, interpolation=None)
    parser.optionxform = str
    parser.read(path, encoding="utf-8")
    return parser


def get(parser, section, key, label, expected):
    try:
        check(label, parser[section][key] == expected)
    except KeyError:
        check(f"{label} (directive exists)", False)


sock_path = os.path.join(UNITS, "nivuus-vm-control.socket")
svc_path = os.path.join(UNITS, "nivuus-vm-control@.service")
wake_path = os.path.join(UNITS, "nivuus-vm-wake.service")
sysusers_path = os.path.join(HOST, "sysusers", "nivuus-vm.conf")
handler_path = os.path.join(HOST, "vm-control.sh")

for p in (sock_path, svc_path, wake_path, sysusers_path, handler_path):
    check(f"{os.path.relpath(p, ROOT)} exists", os.path.isfile(p))

if not failures:
    sock = load_unit(sock_path)
    get(sock, "Socket", "ListenStream", "socket listens on the control path",
        "/run/nivuus/vm-control.sock")
    get(sock, "Socket", "SocketMode", "socket mode is 0660", "0660")
    get(sock, "Socket", "SocketGroup", "socket group is nivuus-vm", "nivuus-vm")
    get(sock, "Socket", "Accept", "socket spawns one service per connection", "yes")
    get(sock, "Install", "WantedBy", "socket is wanted by sockets.target", "sockets.target")

    svc = load_unit(svc_path)
    get(svc, "Service", "ExecStart", "template runs the handler",
        "/usr/local/sbin/vm-control.sh")
    get(svc, "Service", "StandardInput", "template reads the request on the socket", "socket")
    get(svc, "Service", "StandardOutput", "template writes the reply on the socket", "socket")

    wake = load_unit(wake_path)
    get(wake, "Service", "Type", "wake unit is oneshot", "oneshot")
    get(wake, "Service", "ExecStart", "wake unit runs handle-vm-start.sh",
        "/usr/local/sbin/handle-vm-start.sh")
    # systemd counts STARTS, not failures: a platform retrying while the guest
    # boots would trip the default 5-per-10s limit (the 2026-07-13 incident
    # documented in host-network-rf-wan.md).
    get(wake, "Unit", "StartLimitIntervalSec", "wake unit has no start limit", "0")

    handler = open(handler_path, encoding="utf-8").read()
    match = re.search(r'^WAKE_UNIT="([^"]+)"', handler, re.MULTILINE)
    check("handler names a wake unit", match is not None)
    if match:
        check("handler's WAKE_UNIT is the unit file that ships",
              match.group(1) == os.path.basename(wake_path))
    check("handler is executable", os.access(handler_path, os.X_OK))

    sysusers = open(sysusers_path, encoding="utf-8").read().split("\n")
    check("sysusers declares the nivuus-vm group",
          any(re.fullmatch(r"g\s+nivuus-vm\s+-", line.strip()) for line in sysusers))

if failures:
    print("FAIL:")
    for f in failures:
        print(f"  - {f}")
    sys.exit(1)
print("ok: control-channel units agree")
```

- [ ] **Step 2 : vérifier l'échec**

Run: `python3 console/tests/test_console_vm_control_units.py`
Expected: `FAIL:` listant les fichiers absents, code 1.

- [ ] **Step 3 : créer les quatre fichiers**

`console/host/systemd/nivuus-vm-control.socket`
```ini
[Unit]
Description=Control socket for the Windows VM (wake, app activity)

[Socket]
ListenStream=/run/nivuus/vm-control.sock
SocketMode=0660
SocketGroup=nivuus-vm
DirectoryMode=0755
Accept=yes

[Install]
WantedBy=sockets.target
```

`console/host/systemd/nivuus-vm-control@.service`
```ini
[Unit]
Description=Windows VM control request

[Service]
ExecStart=/usr/local/sbin/vm-control.sh
StandardInput=socket
StandardOutput=socket
StandardError=journal
TimeoutStartSec=15
```

`console/host/systemd/nivuus-vm-wake.service`
```ini
[Unit]
Description=Start the Windows VM and open its forward-ports (requested through the control socket)
# systemd counts starts, not failures: a caller retrying while the guest boots
# must never disable this unit.
StartLimitIntervalSec=0

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/handle-vm-start.sh
TimeoutStartSec=300
```

`console/host/sysusers/nivuus-vm.conf`
```
# Group allowed to talk to /run/nivuus/vm-control.sock (see nivuus-vm-control.socket).
g nivuus-vm -
```

- [ ] **Step 4 : vérifier le succès**

Run: `python3 console/tests/test_console_vm_control_units.py`
Expected: `ok: control-channel units agree`.

- [ ] **Step 5 : écrire les attentes de pose et d'armement (échouent)**

Dans `console/tests/test_console_install.py` :
- liste `executables` (~l.160) : ajouter `"usr/local/sbin/vm-control.sh",`
- liste `units` (~l.205) : ajouter
  ```python
        "etc/systemd/system/nivuus-vm-control.socket",
        "etc/systemd/system/nivuus-vm-control@.service",
        "etc/systemd/system/nivuus-vm-wake.service",
  ```
- table de comparaison d'octets (~l.190) : ajouter `("vm-control.sh", "usr/local/sbin/vm-control.sh"),` **et** `("sysusers/nivuus-vm.conf", "usr/lib/sysusers.d/nivuus-vm.conf"),`
- à la suite des vérifications de présence, ajouter :
  ```python
    check("usr/lib/sysusers.d/nivuus-vm.conf depose",
          (root / "usr/lib/sysusers.d/nivuus-vm.conf").is_file(), True)
  ```

Dans `console/tests/test_console_activate.py` :
- dictionnaire des liens attendus (~l.48) : ajouter
  ```python
    "etc/systemd/system/sockets.target.wants/nivuus-vm-control.socket":
        "/etc/systemd/system/nivuus-vm-control.socket",
  ```
- tuple des unités créées (~l.118) : ajouter `"nivuus-vm-control.socket"` à `("vm-trigger-47984.socket", "vm-trigger-47989.socket", "vm-idle-shutdown.timer", "nivuus-guest-ready.timer")`.

Dans `console/tests/test_console_host_files.py`, à côté du contrôle de `vm-idle-shutdown.sh` (~l.35), ajouter le même contrôle pour `vm-control.sh` :
```python
control = os.path.join(CONSOLE, "host", "vm-control.sh")
check("vm-control.sh is versioned", os.path.isfile(control))
```

Run: `for t in console/tests/test_console_install.py console/tests/test_console_activate.py console/tests/test_console_host_files.py; do echo "== $t"; python3 $t | tail -5; done`
Expected: des échecs sur les nouvelles attentes.

- [ ] **Step 6 : câbler la pose** — `console/hooks/install.py`

Dans `HOST_SCRIPTS` (l.87), ajouter : `("host/vm-control.sh", "usr/local/sbin/vm-control.sh"),`

Dans `UNITS` (l.97), ajouter : `"nivuus-vm-control.socket", "nivuus-vm-control@.service", "nivuus-vm-wake.service",`

Après `DROPIN_TARGETS` (l.107-111), ajouter :
```python
# A sysusers.d entry, not a groupadd: the standard mechanism, idempotent, and
# applied by activate (systemd-sysusers) before the control socket starts.
SYSUSERS = [
    ("host/sysusers/nivuus-vm.conf", "usr/lib/sysusers.d/nivuus-vm.conf"),
]
```
Dans la boucle de pose, juste après la boucle `for dest in DROPIN_TARGETS:` (l.212-213), ajouter :
```python
    for src, dest in SYSUSERS:
        place(os.path.join(HERE, src), under(dest), mode=0o644)
```
(`under` est l'assistant déjà employé l.213.)

- [ ] **Step 7 : câbler l'armement** — `console/hooks/activate.py`

Dans `WANTS` (l.66), ajouter : `"nivuus-vm-control.socket": "sockets.target.wants",`

Après `start_now` (l.111-133), ajouter :
```python
SYSUSERS_CONF = "/usr/lib/sysusers.d/nivuus-vm.conf"


def apply_sysusers(conf=SYSUSERS_CONF):
    """Create the nivuus-vm group before the control socket starts.

    The socket declares SocketGroup=nivuus-vm; with no such group it fails to
    start. Returns a failure description, or None. Like start_now() it never
    raises and never fails the phase: the unit is already linked, so the next
    boot (which runs systemd-sysusers itself) is correct either way.
    """
    try:
        proc = subprocess.run(["systemd-sysusers", conf],
                              capture_output=True, text=True)
    except OSError as exc:
        return f"systemd-sysusers: {exc}"
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "").strip()[:200]
        return f"systemd-sysusers {conf}: {detail or proc.returncode}"
    return None
```
Dans `main`, dans le bloc `if root == "/":`, **avant** `broken = start_now(list(WANTS))` :
```python
        refused_group = apply_sysusers()
        if refused_group:
            print("console activate: the nivuus-vm group could not be created, "
                  f"so the control socket will not start - {refused_group}",
                  file=sys.stderr)
```

- [ ] **Step 8 : vérifier le succès**

Run: `for t in console/tests/test_console_install.py console/tests/test_console_activate.py console/tests/test_console_host_files.py console/tests/test_console_wake_units.py console/tests/test_console_vm_control_units.py; do echo "== $t"; python3 $t | tail -3; done`
Expected: tous verts. `apply_sysusers` n'a pas de test unitaire (il n'est appelé que quand `root == "/"`) : il est **vérifié en recette** (Tâche 11, `getent group nivuus-vm`).

- [ ] **Step 9 : commit**

```bash
git add console/host/systemd console/host/sysusers console/hooks console/tests
git commit -m "feat(console): ship the VM control socket, its group and its wake unit

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: condition d'activité applicative dans `vm-idle-shutdown.sh`

**Files:**
- Modify: `console/host/vm-idle-shutdown.sh` (l.8-17 constantes, l.52-54 flux, l.69-74 décision)
- Create: `console/tests/test_vm_idle_shutdown.sh`

**Interfaces:**
- Consumes : `$STATE_DIR/app-activity` écrit par `vm-control.sh busy` (epoch en secondes).
- Produces : variables d'environnement de test `VM_IDLE_STATE_DIR`, `NF_CONNTRACK` ; constantes `APP_HEARTBEAT_S=60`, `APP_ACTIVITY_MAX_AGE_S=180`.

- [ ] **Step 1 : écrire le test (échoue)** — `console/tests/test_vm_idle_shutdown.sh`

```bash
#!/bin/bash
# Tests for the idle check of console/host/vm-idle-shutdown.sh.
#
# Only the strike counter is observed. Every scenario seeds 1 strike and stays
# below IDLE_STRIKES_LIMIT (3), so the hibernation branch is never reached.
#
# State file layout written by the script: "<now_ns> <cpu_ns> <strikes>".

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="$SCRIPT_DIR/../host/vm-idle-shutdown.sh"

PASS=0
FAIL=0
pass() { echo "  ✓ $1"; PASS=$((PASS + 1)); }
fail() { echo "  ✗ $1"; FAIL=$((FAIL + 1)); }

setup_sandbox() {
    SANDBOX=$(mktemp -d)
    mkdir -p "$SANDBOX/bin" "$SANDBOX/state"
    # Running guest, zero CPU time so the CPU condition never fires.
    cat > "$SANDBOX/bin/virsh" <<'EOF'
#!/bin/bash
case "$1" in
    domstate) echo running ;;
    domstats) echo "Domain: 'Windows'"; echo "  cpu.time=0" ;;
esac
EOF
    cat > "$SANDBOX/bin/logger" <<EOF
#!/bin/bash
echo "\$*" >> "$SANDBOX/log"
EOF
    chmod +x "$SANDBOX/bin/"*
    : > "$SANDBOX/log"
    : > "$SANDBOX/conntrack"
    # A previous pass existed (PREV_NS != 0) and left 1 strike.
    echo "1 0 1" > "$SANDBOX/state/state"
}
teardown_sandbox() { rm -rf "$SANDBOX"; }

run_target() {
    VM_IDLE_STATE_DIR="$SANDBOX/state" NF_CONNTRACK="$SANDBOX/conntrack" \
        PATH="$SANDBOX/bin:$PATH" bash "$TARGET" >/dev/null 2>&1
}
strikes() { awk '{print $3}' "$SANDBOX/state/state"; }

echo "== vm-idle-shutdown.sh =="

echo "[1] no signal at all -> one more strike"
setup_sandbox
run_target
[ "$(strikes)" = "2" ] && pass "strikes 1 -> 2" || fail "strikes=$(strikes)"
teardown_sandbox

echo "[2] fresh app-activity -> strikes reset"
setup_sandbox
date +%s > "$SANDBOX/state/app-activity"
run_target
[ "$(strikes)" = "0" ] && pass "strikes reset to 0" || fail "strikes=$(strikes)"
teardown_sandbox

echo "[3] stale app-activity (10 min old) -> one more strike"
setup_sandbox
echo $(( $(date +%s) - 600 )) > "$SANDBOX/state/app-activity"
run_target
[ "$(strikes)" = "2" ] && pass "strikes 1 -> 2" || fail "strikes=$(strikes)"
teardown_sandbox

echo "[4] app-activity just inside the window (170 s) -> strikes reset"
setup_sandbox
echo $(( $(date +%s) - 170 )) > "$SANDBOX/state/app-activity"
run_target
[ "$(strikes)" = "0" ] && pass "strikes reset to 0" || fail "strikes=$(strikes)"
teardown_sandbox

echo "[5] app-activity just outside the window (190 s) -> one more strike"
setup_sandbox
echo $(( $(date +%s) - 190 )) > "$SANDBOX/state/app-activity"
run_target
[ "$(strikes)" = "2" ] && pass "strikes 1 -> 2" || fail "strikes=$(strikes)"
teardown_sandbox

echo "[6] unreadable app-activity -> no activity, and the cause is logged"
setup_sandbox
echo "not-a-timestamp" > "$SANDBOX/state/app-activity"
run_target
[ "$(strikes)" = "2" ] && pass "strikes 1 -> 2" || fail "strikes=$(strikes)"
grep -q "not a timestamp" "$SANDBOX/log" && pass "logs why the file was ignored" \
    || fail "no log line about the unreadable file"
teardown_sandbox

echo "[7] app-activity dated in the future (clock skew) -> treated as active"
setup_sandbox
echo $(( $(date +%s) + 3600 )) > "$SANDBOX/state/app-activity"
run_target
[ "$(strikes)" = "0" ] && pass "strikes reset to 0" || fail "strikes=$(strikes)"
teardown_sandbox

echo "[8] established Moonlight flow still resets strikes (unchanged)"
setup_sandbox
echo "tcp 6 431999 ESTABLISHED src=1.2.3.4 dst=5.6.7.8 sport=50000 dport=47984 src=192.168.3.2 dst=1.2.3.4 sport=47984 dport=50000 [ASSURED] mark=0" > "$SANDBOX/conntrack"
run_target
[ "$(strikes)" = "0" ] && pass "strikes reset to 0" || fail "strikes=$(strikes)"
teardown_sandbox

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" -eq 0 ]
```

```bash
chmod +x console/tests/test_vm_idle_shutdown.sh
```

- [ ] **Step 2 : vérifier l'échec**

Run: `bash console/tests/test_vm_idle_shutdown.sh`
Expected: échecs aux scénarios 2, 4, 7 (le script ignore `app-activity`) ; les scénarios 1, 3, 5, 6 peuvent déjà passer **sauf** le log du 6 ; le scénario 8 échoue tant que `NF_CONNTRACK` n'est pas honoré (le script lit `/proc/net/nf_conntrack`).

- [ ] **Step 3 : modifier `console/host/vm-idle-shutdown.sh`**

Remplacer les lignes de constantes (l.8-16) par :
```bash
VM_NAME="Windows"
VM_IP="192.168.3.2"
TCP_PORTS="3389|47984|47989|48010"
UDP_PORTS="47998|47999|48000"
STATE_DIR="${VM_IDLE_STATE_DIR:-/run/nivuus-vm-idle}"
STATE_FILE="$STATE_DIR/state"
NF_CONNTRACK="${NF_CONNTRACK:-/proc/net/nf_conntrack}"
IDLE_STRIKES_LIMIT=3          # checks in a row before shutdown (3 x 10 min)
CPU_ACTIVE_THRESHOLD=50       # % of one vCPU-core averaged since last check
# A desk app window is "open now" when the platform said so recently. The
# platform sends `busy` every APP_HEARTBEAT_S (plateforme BUSY_PERIOD_MS, keep
# them equal); three missed beats mean the last window is gone. The 30 minutes
# of idle time come from IDLE_STRIKES_LIMIT, NOT from this window - widening
# it to 30 min would double-count and keep the VM up for ~1 h.
APP_HEARTBEAT_S=60
APP_ACTIVITY_MAX_AGE_S=$((3 * APP_HEARTBEAT_S))
APP_ACTIVITY_FILE="$STATE_DIR/app-activity"
LOG_TAG="vm-idle-shutdown"
```
Dans les deux `grep` de flux, remplacer `/proc/net/nf_conntrack` par `"$NF_CONNTRACK"`.

Après le calcul de `FLOWS` (et avant `# --- Activity check 2`), insérer :
```bash
# --- Activity check 3: a desk app window reported by the platform ---
# Written by vm-control.sh `busy`. Missing = no window. Unreadable = ignored
# and logged. Dated in the future (clock skew) = active: never cut a session
# because of a clock step.
APP_ACTIVE=0
if [ -r "$APP_ACTIVITY_FILE" ]; then
    APP_TS=$(cat "$APP_ACTIVITY_FILE")
    if [[ "$APP_TS" =~ ^[0-9]+$ ]]; then
        if [ $(( $(date +%s) - APP_TS )) -lt "$APP_ACTIVITY_MAX_AGE_S" ]; then
            APP_ACTIVE=1
        fi
    else
        logger -t "$LOG_TAG" "ignoring $APP_ACTIVITY_FILE: not a timestamp"
    fi
fi
```
Dans la condition de décision, ajouter `|| [ "$APP_ACTIVE" -eq 1 ]` :
```bash
if [ "$FLOWS" -gt 0 ] || [ "$APP_ACTIVE" -eq 1 ] || [ "$CPU_PCT" -ge "$CPU_ACTIVE_THRESHOLD" ] || [ "$CPU_PCT" -lt 0 ] || [ "$PREV_NS" -eq 0 ]; then
```
Et la ligne de journal : `logger -t "$LOG_TAG" "flows=$FLOWS app=$APP_ACTIVE cpu=${CPU_PCT}% strikes=$STRIKES/$IDLE_STRIKES_LIMIT"`.

- [ ] **Step 4 : vérifier le succès**

Run: `bash console/tests/test_vm_idle_shutdown.sh && bash console/tests/test_vm_control.sh && python3 console/tests/test_console_host_files.py | tail -2`
Expected: les trois verts.

- [ ] **Step 5 : enregistrer le test** (comme Task 1, Step 6) puis committer

```bash
git add console/host/vm-idle-shutdown.sh console/tests/test_vm_idle_shutdown.sh docs/claude/dev-commands.md
git commit -m "feat(console): keep the VM up while the platform reports an open app window

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## PARTIE B — dépôt `desk` (branche `feat/vm-wake-on-launch`)

### Task 4: vocabulaire — motif `hote-inaccessible`, état `demarrage`

**Files:**
- Modify: `plateforme/src/orchestration/refus.ts` (`MOTIFS`, `CODE_HTTP`, nouvelle constante), `plateforme/src/orchestration/interface.ts` (`EtatVm`)
- Test: `plateforme/src/orchestration/refus.test.ts`

**Interfaces:**
- Produces : `BACKEND_HOTE = 'hote'` ; motif `'hote-inaccessible'` → HTTP 503 ; `type EtatVm = EtatAgent | 'demarrage'`.

- [ ] **Step 1 : écrire les tests (échouent)** — ajouter à `refus.test.ts`

```ts
import { BACKEND_HOTE, CODE_HTTP, MOTIFS, refuser } from './refus';

describe('vocabulaire du backend hôte', () => {
    it("'hote-inaccessible' est un motif, rendu en 503", () => {
        expect(MOTIFS).toContain('hote-inaccessible');
        expect(CODE_HTTP['hote-inaccessible']).toBe(503);
    });

    it('un refus de l’hôte porte le backend `hote`', () => {
        expect(refuser('hote-inaccessible', 'demarrer', BACKEND_HOTE)).toEqual({
            ok: false,
            motif: 'hote-inaccessible',
            operation: 'demarrer',
            backend: 'hote',
        });
    });
});
```
(Si `refus.test.ts` importe déjà certains de ces noms, fusionner l'import au lieu de le dupliquer.)

Et dans `interface.test.ts` :
```ts
it("`EtatVm` accepte `demarrage` en plus des deux états de l'agent", () => {
    const etats: EtatVm[] = ['prete', 'injoignable', 'demarrage'];
    expect(etats).toHaveLength(3);
});
```
(importer `type EtatVm` depuis `./interface`.)

- [ ] **Step 2 : vérifier l'échec**

Run: `cd plateforme && npx tsc --noEmit; npx vitest run src/orchestration/refus.test.ts src/orchestration/interface.test.ts`
Expected: erreurs de type (`BACKEND_HOTE` non exporté, `'demarrage'` non assignable) et échecs.

- [ ] **Step 3 : implémenter**

`refus.ts` — après `BACKEND_STATIQUE` :
```ts
/// Le backend qui réveille la VM par le socket de contrôle de l'hôte.
export const BACKEND_HOTE = 'hote';
```
Dans `MOTIFS`, avant `'agent-injoignable'` :
```ts
    /// Le canal de contrôle de l'hôte est absent, refusé, ou n'a pas répondu.
    /// Ce motif dit que le RÉVEIL n'a pas été demandé ; il ne dit rien de
    /// l'état de la VM.
    'hote-inaccessible',
```
Dans `CODE_HTTP` :
```ts
    // 503 : le service va bien, c'est l'hôte qui ne répond pas à la demande.
    'hote-inaccessible': 503,
```
`interface.ts` :
```ts
export type EtatVm = EtatAgent | 'demarrage';
```
et remplacer le commentaire qui affirme que l'union a « exactement deux membres » par :
```ts
/// `demarrage` : un réveil a été demandé et l'agent n'a pas encore battu
/// (`orchestration/wake.ts`). `arretee` reste absent : rien n'émet encore
/// cet état, le dire plutôt que l'écrire serait du code mort dans un type.
```

- [ ] **Step 4 : corriger ce que l'élargissement casse**

Run: `cd plateforme && npx tsc --noEmit`
Chaque `Record<…>`/`switch` exhaustif sur `EtatVm` ou `Motif` qui échoue à la compilation est **voulu** (voir l'en-tête d'`interface.ts`) : lui donner sa valeur (`demarrage` ou `hote-inaccessible`) au lieu d'ajouter un `default`. Puis :

Run: `npx vitest run`
Expected: tout vert.

- [ ] **Step 5 : commit**

```bash
git add plateforme/src/orchestration
git commit -m "feat(plateforme): add the host backend vocabulary (hote-inaccessible, demarrage)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: client du socket de contrôle (`host-channel.ts`)

**Files:**
- Create: `plateforme/src/orchestration/host-channel.ts`
- Test: `plateforme/src/orchestration/host-channel.test.ts`

**Interfaces:**
- Produces :
  ```ts
  export const VERBES_HOTE = ['wake', 'busy'] as const;
  export type VerbeHote = (typeof VERBES_HOTE)[number];
  export type ReponseHote = { ok: true } | { ok: false; cause: string };
  export const DELAI_CANAL_MS = 5_000;
  export function envoyerAuCanal(chemin: string, verbe: VerbeHote, delaiMs?: number): Promise<ReponseHote>;
  ```
  La promesse **ne rejette jamais**. Causes : `err <x>` de l'hôte → `x` ; `ENOENT`, `ECONNREFUSED`, `EACCES`… → le code système ; `delai` ; `ferme-sans-reponse` ; `reponse-inattendue`.

- [ ] **Step 1 : écrire le test (échoue)**

```ts
import { afterEach, describe, expect, it } from 'vitest';
import { createServer, type Server } from 'node:net';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { envoyerAuCanal } from './host-channel';

const nettoyages: Array<() => void> = [];
afterEach(() => {
    while (nettoyages.length > 0) nettoyages.pop()!();
});

/// Un vrai socket Unix qui répond `reponse` à la première ligne reçue et
/// retient ce qu'il a reçu.
function ecouter(reponse: string | null): Promise<{ chemin: string; recu: string[] }> {
    const dossier = mkdtempSync(join(tmpdir(), 'host-channel-'));
    const chemin = join(dossier, 'vm-control.sock');
    const recu: string[] = [];
    const serveur: Server = createServer((socket) => {
        socket.setEncoding('utf8');
        let tampon = '';
        socket.on('data', (morceau) => {
            tampon += morceau;
            const fin = tampon.indexOf('\n');
            if (fin === -1) return;
            recu.push(tampon.slice(0, fin));
            if (reponse !== null) socket.end(reponse);
        });
    });
    nettoyages.push(() => {
        serveur.close();
        rmSync(dossier, { recursive: true, force: true });
    });
    return new Promise((resoudre) => serveur.listen(chemin, () => resoudre({ chemin, recu })));
}

describe('envoyerAuCanal', () => {
    it('envoie le verbe suivi d’un saut de ligne et rend ok sur « ok »', async () => {
        const { chemin, recu } = await ecouter('ok\n');
        expect(await envoyerAuCanal(chemin, 'wake')).toEqual({ ok: true });
        expect(recu).toEqual(['wake']);
    });

    it('rend la cause d’un « err <cause> »', async () => {
        const { chemin } = await ecouter('err wake-failed\n');
        expect(await envoyerAuCanal(chemin, 'wake')).toEqual({ ok: false, cause: 'wake-failed' });
    });

    it('rend `reponse-inattendue` pour une réponse hors protocole', async () => {
        const { chemin } = await ecouter('peut-etre\n');
        expect(await envoyerAuCanal(chemin, 'busy')).toEqual({ ok: false, cause: 'reponse-inattendue' });
    });

    it('🔴 socket absent : refus `ENOENT`, jamais une exception', async () => {
        const r = await envoyerAuCanal('/tmp/host-channel-inexistant.sock', 'wake');
        expect(r).toEqual({ ok: false, cause: 'ENOENT' });
    });

    it('🔴 chemin présent mais sans écouteur : refus `ECONNREFUSED`, jamais une exception', async () => {
        // Sous Linux, se connecter à un fichier qui n'est pas un socket rend
        // ECONNREFUSED : c'est exactement le cas d'un service arrêté dont le
        // fichier traîne.
        const dossier = mkdtempSync(join(tmpdir(), 'host-channel-'));
        nettoyages.push(() => rmSync(dossier, { recursive: true, force: true }));
        const fichier = join(dossier, 'pas-un-socket');
        writeFileSync(fichier, '');
        expect(await envoyerAuCanal(fichier, 'wake')).toEqual({ ok: false, cause: 'ECONNREFUSED' });
    });

    it('🔴 l’hôte ne répond pas : `delai`, sans bloquer', async () => {
        const { chemin } = await ecouter(null);
        const debut = Date.now();
        expect(await envoyerAuCanal(chemin, 'wake', 100)).toEqual({ ok: false, cause: 'delai' });
        expect(Date.now() - debut).toBeLessThan(1_000);
    });

    it('l’hôte ferme sans répondre : `ferme-sans-reponse`', async () => {
        const dossier = mkdtempSync(join(tmpdir(), 'host-channel-'));
        const chemin = join(dossier, 'ferme.sock');
        const serveur = createServer((socket) => socket.end());
        nettoyages.push(() => {
            serveur.close();
            rmSync(dossier, { recursive: true, force: true });
        });
        await new Promise<void>((r) => serveur.listen(chemin, () => r()));
        expect(await envoyerAuCanal(chemin, 'wake')).toEqual({ ok: false, cause: 'ferme-sans-reponse' });
    });
});
```

- [ ] **Step 2 : vérifier l'échec**

Run: `cd plateforme && npx vitest run src/orchestration/host-channel.test.ts`
Expected: FAIL (`Cannot find module './host-channel'`).

- [ ] **Step 3 : implémenter `host-channel.ts`**

```ts
// Client du socket de contrôle de l'hôte (`console`, `nivuus-vm-control.socket`).
//
// Le protocole tient en une ligne : on envoie un verbe, on reçoit `ok` ou
// `err <cause>`. Aucun verbe ne prend d'argument — la VM est celle de l'hôte.
//
// 🔴 LA PROMESSE NE REJETTE JAMAIS. Un socket absent, refusé ou muet est une
// valeur (`{ ok:false, cause }`) que l'appelant transforme en refus typé :
// une exception ferait répondre 500 là où le service doit avouer « l'hôte ne
// répond pas ».

import { connect } from 'node:net';

export const VERBES_HOTE = ['wake', 'busy'] as const;
export type VerbeHote = (typeof VERBES_HOTE)[number];

export type ReponseHote = { ok: true } | { ok: false; cause: string };

/// Borne d'une requête. ⚠️ NON CALIBRÉE : `wake` ne fait que demander le
/// démarrage (`--no-block` côté hôte), donc la réponse arrive en quelques
/// millisecondes ; cinq secondes couvrent un hôte chargé.
export const DELAI_CANAL_MS = 5_000;

export function envoyerAuCanal(
    chemin: string,
    verbe: VerbeHote,
    delaiMs: number = DELAI_CANAL_MS,
): Promise<ReponseHote> {
    return new Promise((resoudre) => {
        const socket = connect(chemin);
        let recu = '';
        let fini = false;

        const terminer = (reponse: ReponseHote): void => {
            if (fini) return;
            fini = true;
            socket.destroy();
            resoudre(reponse);
        };

        socket.setEncoding('utf8');
        socket.setTimeout(delaiMs, () => terminer({ ok: false, cause: 'delai' }));
        socket.on('connect', () => socket.write(`${verbe}\n`));
        socket.on('data', (morceau: string) => {
            recu += morceau;
            const fin = recu.indexOf('\n');
            if (fin === -1) return;
            const ligne = recu.slice(0, fin).trim();
            if (ligne === 'ok') return terminer({ ok: true });
            terminer({
                ok: false,
                cause: ligne.startsWith('err ') ? ligne.slice(4) : 'reponse-inattendue',
            });
        });
        socket.on('error', (e: NodeJS.ErrnoException) =>
            terminer({ ok: false, cause: e.code ?? 'erreur-socket' }),
        );
        socket.on('close', () => terminer({ ok: false, cause: 'ferme-sans-reponse' }));
    });
}
```

- [ ] **Step 4 : vérifier le succès**

Run: `cd plateforme && npx vitest run src/orchestration/host-channel.test.ts && npx tsc --noEmit`
Expected: PASS. 

- [ ] **Step 5 : commit**

```bash
git add plateforme/src/orchestration/host-channel.ts plateforme/src/orchestration/host-channel.test.ts
git commit -m "feat(plateforme): add the host control channel client

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: mémoire du réveil, orchestrateur d'hôte, câblage

**Files:**
- Create: `plateforme/src/orchestration/wake.ts`, `plateforme/src/orchestration/host-orchestrator.ts`, `plateforme/src/orchestration/host-wiring.ts`
- Test: `wake.test.ts`, `host-orchestrator.test.ts`, `host-wiring.test.ts` (même dossier)

**Interfaces:**
- Consumes : `envoyerAuCanal`, `ReponseHote`, `VerbeHote` (tâche 5) ; `refuser`, `BACKEND_HOTE`, `Resultat` (tâche 4) ; `inventaireStatique(base, maintenant)` ; `Orchestrateur`, `EtatVm`.
- Produces :
  ```ts
  export const WAKE_PENDING_MAX_MS = 180_000;
  export class Wake {
      constructor(send: (verb: 'wake') => Promise<ReponseHote>, now: () => number);
      pending(vmId: string): boolean;
      request(vmId: string): Promise<Resultat>;
  }
  export function hostOrchestrator(base: Pilote, now: () => number, wake: Wake): Orchestrateur;
  export function hostWiring(base: Pilote, now: () => number, socketPath: string | undefined):
      { orchestrateur: Orchestrateur; activity: AppActivity | undefined };   // AppActivity : tâche 8
  ```
  Cette tâche crée `hostWiring` **sans** l'activité ; la tâche 8 la complète (`activity` vaut `undefined` ici).

- [ ] **Step 1 : tests de `Wake` (échouent)** — `wake.test.ts`

```ts
import { describe, expect, it, vi } from 'vitest';
import { Wake, WAKE_PENDING_MAX_MS } from './wake';

function fabrique(reponse: { ok: true } | { ok: false; cause: string } = { ok: true }) {
    let instant = 1_000_000;
    const send = vi.fn(async (_verb: 'wake') => reponse);
    const wake = new Wake(send, () => instant);
    return { wake, send, avancer: (ms: number) => void (instant += ms) };
}

describe('Wake', () => {
    it('ne prétend pas qu’un réveil est en cours avant d’en avoir demandé un', () => {
        expect(fabrique().wake.pending('v1')).toBe(false);
    });

    it('demande le réveil une fois, puis le marque en cours', async () => {
        const { wake, send } = fabrique();
        expect(await wake.request('v1')).toEqual({ ok: true });
        expect(send).toHaveBeenCalledTimes(1);
        expect(send).toHaveBeenCalledWith('wake');
        expect(wake.pending('v1')).toBe(true);
    });

    it('🔴 deux lancements simultanés VM éteinte → un seul `wake` envoyé', async () => {
        const { wake, send } = fabrique();
        await Promise.all([wake.request('v1'), wake.request('v1')]);
        await wake.request('v1');
        expect(send).toHaveBeenCalledTimes(1);
    });

    it('un réveil par VM : une autre VM a le sien', async () => {
        const { wake, send } = fabrique();
        await wake.request('v1');
        await wake.request('v2');
        expect(send).toHaveBeenCalledTimes(2);
    });

    it('🔴 la borne : en cours jusqu’à WAKE_PENDING_MAX_MS, plus après — et un nouveau réveil est alors demandé', async () => {
        const { wake, send, avancer } = fabrique();
        await wake.request('v1');
        avancer(WAKE_PENDING_MAX_MS);
        expect(wake.pending('v1')).toBe(true);
        avancer(1);
        expect(wake.pending('v1')).toBe(false);
        await wake.request('v1');
        expect(send).toHaveBeenCalledTimes(2);
    });

    it('🔴 un échec du canal est un refus typé, journalisé, et ne marque rien en cours', async () => {
        const avertir = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const { wake } = fabrique({ ok: false, cause: 'ENOENT' });
        expect(await wake.request('v1')).toEqual({
            ok: false,
            motif: 'hote-inaccessible',
            operation: 'demarrer',
            backend: 'hote',
        });
        expect(wake.pending('v1')).toBe(false);
        expect(avertir).toHaveBeenCalledWith(expect.stringContaining('ENOENT'));
        avertir.mockRestore();
    });
});
```

- [ ] **Step 2 : vérifier l'échec**, puis **Step 3 : implémenter `wake.ts`**

Run: `cd plateforme && npx vitest run src/orchestration/wake.test.ts` → FAIL (module absent).

```ts
// La mémoire des réveils demandés.
//
// 🔴 ELLE VIT DANS UN OBJET DE LONGUE DURÉE, pas dans l'orchestrateur : les
// routes construisaient `inventaireStatique` à chaque requête, et un état
// tenu là serait perdu à la requête suivante.

import type { ReponseHote } from './host-channel';
import { BACKEND_HOTE, refuser, type Resultat } from './refus';

/// Durée pendant laquelle un réveil demandé compte comme « en cours ».
/// ⚠️ NON CALIBRÉE : c'est le `MAX_WAIT_SECONDS` de `handle-vm-start.sh` (180 s
/// pour l'IP), sans l'amorçage de l'agent. Au-delà, l'état redevient
/// `injoignable` : une VM qui ne démarre jamais ne reste pas « en démarrage ».
export const WAKE_PENDING_MAX_MS = 180_000;

export class Wake {
    private readonly demandeA = new Map<string, number>();
    private readonly enVol = new Map<string, Promise<Resultat>>();

    constructor(
        private readonly send: (verb: 'wake') => Promise<ReponseHote>,
        private readonly now: () => number,
    ) {}

    /// Un réveil a-t-il été demandé il y a moins de `WAKE_PENDING_MAX_MS` ?
    pending(vmId: string): boolean {
        const a = this.demandeA.get(vmId);
        if (a === undefined) return false;
        if (this.now() - a > WAKE_PENDING_MAX_MS) {
            this.demandeA.delete(vmId);
            return false;
        }
        return true;
    }

    /// Demande le réveil, sauf s'il est déjà en cours (ou en vol).
    /// ⚠️ `ok:true` dit « la demande est partie ou déjà en cours », jamais
    /// « la VM est prête » : c'est l'agent qui le dit, en se reconnectant.
    request(vmId: string): Promise<Resultat> {
        if (this.pending(vmId)) return Promise.resolve({ ok: true });
        const enVol = this.enVol.get(vmId);
        if (enVol !== undefined) return enVol;

        const demande = this.send('wake')
            .then((reponse): Resultat => {
                if (!reponse.ok) {
                    console.warn(`wake refused for vm ${vmId}: ${reponse.cause}`);
                    return refuser('hote-inaccessible', 'demarrer', BACKEND_HOTE);
                }
                this.demandeA.set(vmId, this.now());
                return { ok: true };
            })
            .finally(() => this.enVol.delete(vmId));
        this.enVol.set(vmId, demande);
        return demande;
    }
}
```

Run: `npx vitest run src/orchestration/wake.test.ts` → PASS.

- [ ] **Step 4 : tests de l'orchestrateur d'hôte (échouent)** — `host-orchestrator.test.ts`

```ts
import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { SEUIL_INJOIGNABLE_MS } from '../agents/fraicheur';
import { enroler, marquerVu } from '../depot/agent';
import { hostOrchestrator } from './host-orchestrator';
import { Wake } from './wake';

const MS = 1_787_136_773_742;
let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function poserVm(p: Pilote, id: string, vuA: number | null): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, `n-${id}`, '192.168.3.2']);
    await enroler(p, id, 'empreinte-opaque', `PREFIXE${id}`);
    if (vuA !== null) await marquerVu(p, id, vuA);
}

function monter(instant = MS) {
    const send = vi.fn(async (_v: 'wake') => ({ ok: true }) as const);
    const wake = new Wake(send, () => instant);
    return { wake, send, orchestrateur: hostOrchestrator(base!, () => instant, wake) };
}

describe(`hostOrchestrator, moteur=${MOTEUR}`, () => {
    it('`demarrer` demande le réveil d’une VM connue', async () => {
        base = await baseNeuve('ho-demarrer');
        await poserVm(base, 'v1', null);
        const { orchestrateur, send } = monter();
        expect(await orchestrateur.demarrer('v1')).toEqual({ ok: true });
        expect(send).toHaveBeenCalledTimes(1);
    });

    it('🔴 `demarrer` d’une VM inconnue est refusé SANS toucher l’hôte', async () => {
        base = await baseNeuve('ho-inconnue');
        const { orchestrateur, send } = monter();
        expect(await orchestrateur.demarrer('fantome')).toEqual({
            ok: false,
            motif: 'vm-inconnue',
            operation: 'demarrer',
            backend: 'hote',
        });
        expect(send).not.toHaveBeenCalled();
    });

    it('`etat` rend `demarrage` pour une VM injoignable dont le réveil est en cours', async () => {
        base = await baseNeuve('ho-etat-demarrage');
        await poserVm(base, 'v1', MS - SEUIL_INJOIGNABLE_MS - 1);
        const { orchestrateur } = monter();
        expect(await orchestrateur.etat('v1')).toBe('injoignable');
        await orchestrateur.demarrer('v1');
        expect(await orchestrateur.etat('v1')).toBe('demarrage');
    });

    it('🔴 `etat` rend `prete` pour une VM vivante, même si un réveil est marqué en cours', async () => {
        base = await baseNeuve('ho-etat-prete');
        await poserVm(base, 'v1', MS);
        const { orchestrateur } = monter();
        await orchestrateur.demarrer('v1');
        expect(await orchestrateur.etat('v1')).toBe('prete');
    });

    it('les autres verbes gardent le comportement de l’inventaire statique', async () => {
        base = await baseNeuve('ho-autres');
        await poserVm(base, 'v1', null);
        const { orchestrateur } = monter();
        vi.spyOn(console, 'warn').mockImplementation(() => {});
        expect(await orchestrateur.arreter('v1')).toMatchObject({ ok: false, motif: 'non-supporte' });
        expect(await orchestrateur.instantane('v1', 'x')).toMatchObject({ ok: false, motif: 'non-supporte' });
        expect((await orchestrateur.lister()).map((v) => v.id)).toEqual(['v1']);
    });
});
```

Run: FAIL (module absent). Puis **implémenter `host-orchestrator.ts`** :

```ts
// L'orchestrateur qui sait réveiller la VM.
//
// Il ENVELOPPE `inventaireStatique` plutôt que de le réécrire : l'inventaire,
// la fraîcheur et les refus des autres verbes restent ceux que les tests de
// `inventaire-statique.test.ts` tiennent. Il ne remplace que deux verbes :
// `demarrer` (demande le réveil) et `etat` (dit `demarrage` pendant un réveil).

import type { Pilote } from '../base/pilote';
import type { EtatVm, Orchestrateur } from './interface';
import { inventaireStatique } from './inventaire-statique';
import { BACKEND_HOTE, refuser, type Resultat } from './refus';
import type { Wake } from './wake';

export function hostOrchestrator(base: Pilote, now: () => number, wake: Wake): Orchestrateur {
    const statique = inventaireStatique(base, now);
    return {
        ...statique,

        async etat(vm: string): Promise<EtatVm> {
            const etat = await statique.etat(vm);
            // Seule une VM qu'on ne voit pas peut être « en démarrage » : une
            // VM vivante est `prete`, quoi que la mémoire du réveil dise.
            return etat === 'injoignable' && wake.pending(vm) ? 'demarrage' : etat;
        },

        async demarrer(vm: string): Promise<Resultat> {
            // Une VM inconnue est refusée AVANT d'ouvrir le canal : le socket de
            // l'hôte ne doit jamais être sollicité pour une VM que ce service
            // n'inventorie pas.
            const connue = (await statique.lister()).some((v) => v.id === vm);
            if (!connue) return refuser('vm-inconnue', 'demarrer', BACKEND_HOTE);
            return wake.request(vm);
        },
    };
}
```
Run: `npx vitest run src/orchestration/host-orchestrator.test.ts` → PASS.

- [ ] **Step 5 : test du câblage (échoue)** — `host-wiring.test.ts`

```ts
import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { hostWiring } from './host-wiring';

let base: Pilote | undefined;
afterEach(async () => {
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

describe(`hostWiring, moteur=${MOTEUR}`, () => {
    it('🔴 sans socket configuré : l’orchestrateur statique, `demarrer` refuse `non-supporte`', async () => {
        base = await baseNeuve('hw-sans-socket');
        await base.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', ['v1', 'w1', '192.168.3.2']);
        vi.spyOn(console, 'warn').mockImplementation(() => {});
        const { orchestrateur } = hostWiring(base, Date.now, undefined);
        expect(await orchestrateur.demarrer('v1')).toMatchObject({ ok: false, motif: 'non-supporte' });
    });

    it('avec un socket configuré : `demarrer` passe par l’hôte (ici absent → `hote-inaccessible`)', async () => {
        base = await baseNeuve('hw-avec-socket');
        await base.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', ['v1', 'w1', '192.168.3.2']);
        vi.spyOn(console, 'warn').mockImplementation(() => {});
        const { orchestrateur } = hostWiring(base, Date.now, '/tmp/host-wiring-absent.sock');
        expect(await orchestrateur.demarrer('v1')).toMatchObject({ ok: false, motif: 'hote-inaccessible' });
    });
});
```
Run: FAIL. Puis **implémenter `host-wiring.ts`** :

```ts
// Le câblage de l'hôte : un seul endroit décide si le service sait réveiller
// la VM. `serveur.ts` (467 lignes) n'en reçoit que le résultat.

import type { Pilote } from '../base/pilote';
import { envoyerAuCanal } from './host-channel';
import { hostOrchestrator } from './host-orchestrator';
import type { Orchestrateur } from './interface';
import { inventaireStatique } from './inventaire-statique';
import { Wake } from './wake';

export interface CablageHote {
    orchestrateur: Orchestrateur;
}

/// `socketPath` absent = canal désactivé : on garde l'inventaire statique, dont
/// `demarrer` refuse `non-supporte` (le comportement d'avant ce chantier).
export function hostWiring(
    base: Pilote,
    now: () => number,
    socketPath: string | undefined,
): CablageHote {
    if (socketPath === undefined) {
        return { orchestrateur: inventaireStatique(base, now) };
    }
    const wake = new Wake((verb) => envoyerAuCanal(socketPath, verb), now);
    return { orchestrateur: hostOrchestrator(base, now, wake) };
}
```
Le type `CablageHote` est élargi à la tâche 8 (`activity`) ; le test de cette tâche n'en lit que `orchestrateur` (remplacer `{ orchestrateur, activity }` par `{ orchestrateur }` si besoin).

Run: `npx vitest run src/orchestration && npx tsc --noEmit` → PASS.

- [ ] **Step 6 : commit**

```bash
git add plateforme/src/orchestration
git commit -m "feat(plateforme): wake the VM through the host control channel

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: configuration et routes (le lancement réveille la VM)

**Files:**
- Modify: `plateforme/src/config.ts` (interface `Config`, `lireConfig`), `plateforme/src/http/routes-applications.ts` (`DependancesApplications`, l.355-363), `plateforme/src/http/routes-session.ts` (`DependancesSession`, l.173), `plateforme/src/http/routes-vm.ts` (`DependancesVm`, l.297), `plateforme/src/http/serveur.ts` (imports, objet `deps` ~l.232)
- Test: `plateforme/src/config.test.ts`, `plateforme/src/http/routes-applications.test.ts` (+ adaptation des harnais de `routes-session.test.ts` et `routes-vm.test.ts`)

**Interfaces:**
- Consumes : `hostWiring` (tâche 6).
- Produces : `Config.socketVm?: string` (`PLATEFORME_SOCKET_VM`, chemin absolu, vide = désactivé) ; `orchestrateur: Orchestrateur` **requis** dans `DependancesApplications`, `DependancesSession`, `DependancesVm` ; corps de `503` du lancement : `{ refus: 'agent-injoignable', etat: 'demarrage' }` si le réveil est accepté, sinon `{ refus: 'agent-injoignable', etat: 'injoignable', reveil: <motif> }`.

- [ ] **Step 1 : tests de configuration (échouent)** — ajouter à `config.test.ts` (réutiliser l'environnement minimal valide déjà défini dans ce fichier ; le nommer `ENV_VALIDE` s'il porte un autre nom)

```ts
describe('PLATEFORME_SOCKET_VM', () => {
    it('absent ou vide : le canal est désactivé', () => {
        expect(lireConfig({ ...ENV_VALIDE }).socketVm).toBeUndefined();
        expect(lireConfig({ ...ENV_VALIDE, PLATEFORME_SOCKET_VM: '' }).socketVm).toBeUndefined();
    });

    it('un chemin absolu est retenu tel quel', () => {
        expect(
            lireConfig({ ...ENV_VALIDE, PLATEFORME_SOCKET_VM: '/run/nivuus/vm-control.sock' }).socketVm,
        ).toBe('/run/nivuus/vm-control.sock');
    });

    it('🔴 un chemin relatif est refusé, en nommant la variable', () => {
        expect(() => lireConfig({ ...ENV_VALIDE, PLATEFORME_SOCKET_VM: 'vm-control.sock' })).toThrow(
            /PLATEFORME_SOCKET_VM/,
        );
    });
});
```
Run: `npx vitest run src/config.test.ts` → FAIL. Puis dans `config.ts` : champ `socketVm?: string;` dans `Config`, et avant le `return` de `lireConfig` :

```ts
    const brutSocketVm = env.PLATEFORME_SOCKET_VM;
    const socketVm = brutSocketVm === undefined || brutSocketVm === '' ? undefined : brutSocketVm;
    if (socketVm !== undefined && !socketVm.startsWith('/')) {
        throw new Error(
            `PLATEFORME_SOCKET_VM doit être un chemin absolu, reçu : ${socketVm}. ` +
                'Vide, le canal de réveil de la VM est désactivé.',
        );
    }
```
et `socketVm,` dans l'objet retourné. Run → PASS.

- [ ] **Step 2 : test de la route de lancement (échoue)** — dans `routes-applications.test.ts`, repérer l'`it()` existant qui éprouve `agent-injoignable` (`grep -n "agent-injoignable" plateforme/src/http/routes-applications.test.ts`) et son harnais (fonction qui construit `deps`). Ajouter `orchestrateur` au harnais :

```ts
import { inventaireStatique } from '../orchestration/inventaire-statique';
// dans l'objet de dépendances du harnais :
orchestrateur: inventaireStatique(b, () => instant),
```
(`b` et `instant` : les noms que le harnais emploie déjà pour la base et l'horloge.) Puis ajouter, avec un orchestrateur de test paramétrable :

```ts
it('🔴 VM éteinte : le lancement demande le réveil et répond 503 `etat: demarrage`', async () => {
    const demarrer = vi.fn(async () => ({ ok: true }) as const);
    const url = await servir('ra-reveil', { orchestrateur: { ...statique, demarrer } });
    // … mêmes données que l'`it()` agent-injoignable existant : VM, application, propriétaire …
    const r = await lancer(url, jeton, 'app1');
    expect(r.status).toBe(503);
    expect(await r.json()).toEqual({ refus: 'agent-injoignable', etat: 'demarrage' });
    expect(demarrer).toHaveBeenCalledWith('v1');
});

it('🔴 réveil impossible : 503 `etat: injoignable` et le motif du refus', async () => {
    const demarrer = vi.fn(async () => ({
        ok: false, motif: 'hote-inaccessible', operation: 'demarrer', backend: 'hote',
    }) as const);
    const url = await servir('ra-reveil-refuse', { orchestrateur: { ...statique, demarrer } });
    const r = await lancer(url, jeton, 'app1');
    expect(r.status).toBe(503);
    expect(await r.json()).toEqual({
        refus: 'agent-injoignable', etat: 'injoignable', reveil: 'hote-inaccessible',
    });
});

it('🔴 le réveil n’est PAS demandé pour la VM de quelqu’un d’autre', async () => {
    const demarrer = vi.fn(async () => ({ ok: true }) as const);
    const url = await servir('ra-reveil-autre', { orchestrateur: { ...statique, demarrer } });
    // l'application appartient à une autre VM que celle de l'utilisateur authentifié
    const r = await lancer(url, jetonDeBob, 'app1');
    expect(r.status).toBe(404);
    expect(demarrer).not.toHaveBeenCalled();
});

it('🔴 `504 delai` ne demande aucun réveil', async () => {
    const demarrer = vi.fn(async () => ({ ok: true }) as const);
    // registre dont `lancer` rend 'delai'
    // … attendu : 504 { refus: 'delai' } et `demarrer` jamais appelé
});
```
⚠️ Les noms `servir`, `lancer`, `statique`, `jeton`, `jetonDeBob` sont ceux du harnais à **lire avant d'écrire** : ouvrir le fichier, reprendre la forme de ses `it()` voisins (données de VM, d'application et de jeton comprises) et n'en changer que l'orchestrateur. Le harnais doit accepter un remplacement partiel de `deps` (`servir(nom, surcharges)`) ; s'il ne le fait pas, l'ajouter est la première modification de cette étape.

Run: `npx vitest run src/http/routes-applications.test.ts` → FAIL (type `orchestrateur` inconnu, comportement absent).

- [ ] **Step 3 : implémenter les routes**

`routes-applications.ts` : importer `import type { Orchestrateur } from '../orchestration/interface';`, ajouter `orchestrateur: Orchestrateur;` à `DependancesApplications`, et remplacer le bloc `if (issue === 'agent-injoignable') { … }` (l.357-363) par :

```ts
    if (issue === 'agent-injoignable') {
        // 503 : le service va bien, c'est la VM qui ne répond pas. Rendre 200
        // ferait afficher au hub un succès pour un lancement qui n'a pas eu
        // lieu.
        //
        // 🔴 LE RÉVEIL EST DEMANDÉ ICI, ET SEULEMENT ICI, APRÈS `acces` : un
        // utilisateur ne réveille que la VM qui lui est attribuée. `lancer`
        // n'a envoyé AUCUN ordre à l'agent (aucun socket) : le hub peut donc
        // rejouer ce lancement sans risque de doublon — ce qui n'est PAS vrai
        // du `504 delai` ci-dessous, jamais rejoué.
        const reveil = await deps.orchestrateur.demarrer(application.vm_id);
        repondre(
            rep,
            503,
            reveil.ok
                ? { refus: 'agent-injoignable', etat: 'demarrage' }
                : { refus: 'agent-injoignable', etat: 'injoignable', reveil: reveil.motif },
            cors,
        );
        return true;
    }
```
`routes-session.ts` : importer le type `Orchestrateur`, ajouter `orchestrateur: Orchestrateur;` à `DependancesSession`, remplacer `const orchestrateur = inventaireStatique(deps.base, deps.maintenant);` par `const orchestrateur = deps.orchestrateur;` et supprimer l'import devenu inutile. Ajouter `reveilPossible: boolean;` à `DependancesSession` et, dans le bloc `redemarrage` (l.~210), remplacer l'objet `{ possible: false, motif: 'non-supporte', backend: BACKEND_STATIQUE }` par :
```ts
                redemarrage: deps.reveilPossible
                    ? { possible: true }
                    : { possible: false, motif: 'non-supporte', backend: BACKEND_STATIQUE },
```
(`reveilPossible` vaut `config.socketVm !== undefined` : le hub n'avoue « ne sait pas redémarrer » que si c'est vrai.)
`routes-vm.ts` : même remplacement (`deps.orchestrateur`), même ajout de champ dans `DependancesVm`.

Adapter les harnais de `routes-session.test.ts` et `routes-vm.test.ts` : ajouter `orchestrateur: inventaireStatique(b, () => instant)` à l'objet de dépendances de leur fonction `servir` ; dans celui de `routes-session.test.ts`, ajouter aussi `reveilPossible: false` (paramètre surchargeable comme `orchestrateur`). Ajouter à `routes-session.test.ts` :

```ts
it('🔴 pendant un réveil, `etat` vaut `demarrage` et `redemarrage.possible` vaut true', async () => {
    const send = vi.fn(async () => ({ ok: true }) as const);
    // orchestrateur d'hôte sur la même base, réveil demandé avant la requête,
    // harnais réglé sur `reveilPossible: true`
    // … attendu : 503, corps.etat === 'demarrage', corps.redemarrage deep-equals { possible: true }
});

it('🔴 sans canal configuré, `redemarrage.possible` reste false (l’aveu d’avant ce chantier)', async () => {
    // harnais par défaut : `reveilPossible: false` et `inventaireStatique`
    // … attendu : le test ④b existant, inchangé, reste vert
});
```
(mêmes données que ④a ; construire `hostOrchestrator(b, () => instant, new Wake(send, () => instant))`, appeler `demarrer('v1')` puis `demander`.)

`serveur.ts` : importer `import { hostWiring } from '../orchestration/host-wiring';`, calculer avant `const deps = {` :
```ts
    const { orchestrateur } = hostWiring(base, Date.now, config.socketVm);
```
et ajouter `orchestrateur,` et `reveilPossible: config.socketVm !== undefined,` à l'objet `deps` (l.232).

- [ ] **Step 4 : vérifier**

Run: `cd plateforme && npx tsc --noEmit && npx vitest run`
Expected: tout vert (les harnais de tests anciens passent `inventaireStatique`, donc l'existant est inchangé).

- [ ] **Step 5 : commit**

```bash
git add plateforme/src
git commit -m "feat(plateforme): launching an app on a stopped VM wakes it

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: signal d'activité applicative

**Files:**
- Create: `plateforme/src/signaling/app-activity.ts`, `plateforme/src/signaling/observers.ts`
- Test: `app-activity.test.ts`, `observers.test.ts`
- Modify: `plateforme/src/orchestration/host-wiring.ts` (ajoute `activity`), `plateforme/src/http/serveur.ts` (observateur composé, arrêt à la fermeture)

**Interfaces:**
- Consumes : `ObservateurDeSession` (`signaling/relais.ts`), `decouper` (`agents/prefixe.ts`), `envoyerAuCanal` (tâche 5).
- Produces :
  ```ts
  export const BUSY_PERIOD_MS = 60_000;   // = APP_HEARTBEAT_S de vm-idle-shutdown.sh
  export interface AppActivity extends ObservateurDeSession { stop(): void; windows(): number }
  export function createAppActivity(send: (verb: 'busy') => Promise<ReponseHote>): AppActivity;
  export function composeObservers(...observers: ObservateurDeSession[]): ObservateurDeSession;
  ```
  `CablageHote` devient `{ orchestrateur: Orchestrateur; activity: AppActivity | undefined }`.

- [ ] **Step 1 : tests de l'activité (échouent)** — `app-activity.test.ts`

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BUSY_PERIOD_MS, createAppActivity } from './app-activity';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
});

function monter() {
    const send = vi.fn(async (_v: 'busy') => ({ ok: true }) as const);
    return { send, activity: createAppActivity(send) };
}

describe('createAppActivity', () => {
    it('une fenêtre s’ouvre : `busy` part tout de suite, puis toutes les BUSY_PERIOD_MS', () => {
        const { send, activity } = monter();
        activity.apparie('PFX:w-1');
        expect(send).toHaveBeenCalledTimes(1);
        expect(send).toHaveBeenCalledWith('busy');
        vi.advanceTimersByTime(BUSY_PERIOD_MS);
        expect(send).toHaveBeenCalledTimes(2);
        vi.advanceTimersByTime(BUSY_PERIOD_MS);
        expect(send).toHaveBeenCalledTimes(3);
        activity.stop();
    });

    it('🔴 la session de contrôle `bureau` et le pont `fichiers` ne comptent PAS', () => {
        const { send, activity } = monter();
        activity.apparie('PFX:bureau');
        activity.apparie('PFX:fichiers');
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).not.toHaveBeenCalled();
        expect(activity.windows()).toBe(0);
    });

    it('🔴 un nom sans préfixe ou qui ressemble à `w-1` sans l’être ne compte pas', () => {
        const { send, activity } = monter();
        for (const nom of ['PFX:w-', 'PFX:w-1x', 'PFX:xw-1', 'PFX:w-1:z']) activity.apparie(nom);
        expect(send).not.toHaveBeenCalled();
        expect(activity.windows()).toBe(0);
    });

    it('un nom sans préfixe `w-1` compte (session nue)', () => {
        const { send, activity } = monter();
        activity.apparie('w-1');
        expect(send).toHaveBeenCalledTimes(1);
        activity.stop();
    });

    it('la dernière fenêtre fermée arrête les battements', () => {
        const { send, activity } = monter();
        activity.apparie('PFX:w-1');
        activity.apparie('PFX:w-2');
        activity.separe('PFX:w-1');
        vi.advanceTimersByTime(BUSY_PERIOD_MS);
        expect(send).toHaveBeenCalledTimes(2); // encore une fenêtre : ça bat
        activity.separe('PFX:w-2');
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).toHaveBeenCalledTimes(2); // plus rien
        expect(activity.windows()).toBe(0);
    });

    it('🔴 `apparie` répété et `separe` inconnu ne faussent pas le compte', () => {
        const { send, activity } = monter();
        activity.apparie('PFX:w-1');
        activity.apparie('PFX:w-1');
        activity.separe('PFX:w-9'); // jamais apparié
        expect(activity.windows()).toBe(1);
        activity.separe('PFX:w-1');
        expect(activity.windows()).toBe(0);
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).toHaveBeenCalledTimes(1);
    });

    it('🔴 un échec d’envoi est journalisé en warn et les battements continuent', async () => {
        const avertir = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const send = vi.fn(async (_v: 'busy') => ({ ok: false, cause: 'ENOENT' }) as const);
        const activity = createAppActivity(send);
        activity.apparie('PFX:w-1');
        await vi.advanceTimersByTimeAsync(BUSY_PERIOD_MS);
        expect(send).toHaveBeenCalledTimes(2);
        expect(avertir).toHaveBeenCalledWith(expect.stringContaining('ENOENT'));
        activity.stop();
    });

    it('`stop` arrête tout, même avec des fenêtres ouvertes', () => {
        const { send, activity } = monter();
        activity.apparie('PFX:w-1');
        activity.stop();
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).toHaveBeenCalledTimes(1);
    });
});
```
Run: `npx vitest run src/signaling/app-activity.test.ts` → FAIL (module absent).

- [ ] **Step 2 : implémenter `app-activity.ts`**

```ts
// Ce que la plateforme dit à l'hôte : « une fenêtre d'app est ouverte ».
//
// 🔴 SEULES LES SESSIONS `w-<N>` COMPTENT. La session de contrôle `bureau` et
// le pont `fichiers` sont appariés dès que le hub est ouvert sur une VM
// allumée : les compter ferait qu'un onglet de hub oublié empêcherait
// indéfiniment l'arrêt de la VM.
//
// 🔴 UN ENSEMBLE DE NOMS, PAS UN COMPTEUR : un `apparie` répété ou un
// `separe` inconnu ne peut pas fausser le compte, ni le rendre négatif.

import { decouper } from '../agents/prefixe';
import type { ReponseHote } from '../orchestration/host-channel';
import type { ObservateurDeSession } from './relais';

/// Période d'émission de `busy`. ⚠️ À TENIR ÉGALE à `APP_HEARTBEAT_S` de
/// `console/host/vm-idle-shutdown.sh` (dépôt `installer`) : l'hôte tient une
/// fenêtre pour ouverte tant que le dernier `busy` date de moins de trois
/// périodes.
export const BUSY_PERIOD_MS = 60_000;

const FENETRE = /^w-\d+$/;

export interface AppActivity extends ObservateurDeSession {
    /// Arrête les battements (fermeture du service).
    stop(): void;
    /// Le nombre de fenêtres ouvertes, pour les tests et le diagnostic.
    windows(): number;
}

export function createAppActivity(send: (verb: 'busy') => Promise<ReponseHote>): AppActivity {
    const ouvertes = new Set<string>();
    let minuteur: NodeJS.Timeout | undefined;

    const battre = (): void => {
        void send('busy').then((reponse) => {
            if (!reponse.ok) console.warn(`busy signal failed: ${reponse.cause}`);
        });
    };
    const armer = (): void => {
        if (minuteur !== undefined) return;
        battre();
        minuteur = setInterval(battre, BUSY_PERIOD_MS);
        // Un minuteur de battement ne doit jamais retenir le processus.
        minuteur.unref();
    };
    const desarmer = (): void => {
        if (minuteur === undefined) return;
        clearInterval(minuteur);
        minuteur = undefined;
    };

    return {
        apparie(nomSession) {
            if (!FENETRE.test(decouper(nomSession).nom)) return;
            ouvertes.add(nomSession);
            armer();
        },
        separe(nomSession) {
            ouvertes.delete(nomSession);
            if (ouvertes.size === 0) desarmer();
        },
        stop: desarmer,
        windows: () => ouvertes.size,
    };
}
```
Run: PASS.

- [ ] **Step 3 : test et implémentation du composeur** — `observers.test.ts`

```ts
import { describe, expect, it, vi } from 'vitest';
import { composeObservers } from './observers';

describe('composeObservers', () => {
    it('transmet apparie et separe à chaque observateur, dans l’ordre', () => {
        const appels: string[] = [];
        const a = { apparie: (n: string) => appels.push(`a+${n}`), separe: (n: string) => appels.push(`a-${n}`) };
        const b = { apparie: (n: string) => appels.push(`b+${n}`), separe: (n: string) => appels.push(`b-${n}`) };
        const c = composeObservers(a, b);
        c.apparie('s1');
        c.separe('s1');
        expect(appels).toEqual(['a+s1', 'b+s1', 'a-s1', 'b-s1']);
    });

    it('🔴 un observateur qui lève n’empêche pas les suivants, et l’erreur est journalisée', () => {
        const erreur = vi.spyOn(console, 'error').mockImplementation(() => {});
        const suivant = vi.fn();
        const c = composeObservers(
            { apparie: () => { throw new Error('boom'); }, separe: () => {} },
            { apparie: suivant, separe: () => {} },
        );
        expect(() => c.apparie('s1')).not.toThrow();
        expect(suivant).toHaveBeenCalledWith('s1');
        expect(erreur).toHaveBeenCalledWith(expect.stringContaining('boom'));
        erreur.mockRestore();
    });
});
```
Run: FAIL. Implémenter `observers.ts` :

```ts
// Le relais n'accepte qu'UN observateur de session : ce composeur en enchaîne
// plusieurs sans que l'un dépende de l'autre.
//
// 🔴 `apparie` et `separe` sont appelés depuis le gestionnaire `message` d'un
// socket `ws`, synchrone, où une exception abat tout le processus Node (voir
// `relais.ts::ObservateurDeSession`). Un observateur qui lève est donc
// journalisé, jamais propagé, et ne prive pas les suivants de l'événement.

import type { ObservateurDeSession } from './relais';

export function composeObservers(...observers: ObservateurDeSession[]): ObservateurDeSession {
    const chacun = (nom: 'apparie' | 'separe', session: string): void => {
        for (const o of observers) {
            try {
                o[nom](session);
            } catch (cause) {
                console.error(`session observer ${nom} failed for ${session}: ${String(cause)}`);
            }
        }
    };
    return {
        apparie: (session, utilisateurId) => {
            for (const o of observers) {
                try {
                    o.apparie(session, utilisateurId);
                } catch (cause) {
                    console.error(`session observer apparie failed for ${session}: ${String(cause)}`);
                }
            }
        },
        separe: (session) => chacun('separe', session),
    };
}
```
Run: `npx vitest run src/signaling` → PASS.

- [ ] **Step 4 : compléter le câblage et la fermeture**

`host-wiring.ts` : remplacer par

```ts
import type { Pilote } from '../base/pilote';
import { createAppActivity, type AppActivity } from '../signaling/app-activity';
import { envoyerAuCanal } from './host-channel';
import { hostOrchestrator } from './host-orchestrator';
import type { Orchestrateur } from './interface';
import { inventaireStatique } from './inventaire-statique';
import { Wake } from './wake';

export interface CablageHote {
    orchestrateur: Orchestrateur;
    /// `undefined` quand le canal est désactivé : rien à signaler à l'hôte.
    activity: AppActivity | undefined;
}

export function hostWiring(
    base: Pilote,
    now: () => number,
    socketPath: string | undefined,
): CablageHote {
    if (socketPath === undefined) {
        return { orchestrateur: inventaireStatique(base, now), activity: undefined };
    }
    const wake = new Wake((verb) => envoyerAuCanal(socketPath, verb), now);
    return {
        orchestrateur: hostOrchestrator(base, now, wake),
        activity: createAppActivity((verb) => envoyerAuCanal(socketPath, verb)),
    };
}
```
Ajouter à `host-wiring.test.ts` :
```ts
it('`activity` n’existe que si le canal est configuré', async () => {
    base = await baseNeuve('hw-activity');
    expect(hostWiring(base, Date.now, undefined).activity).toBeUndefined();
    const avec = hostWiring(base, Date.now, '/tmp/host-wiring-absent.sock');
    expect(avec.activity).toBeDefined();
    avec.activity!.stop();
});
```
`serveur.ts` : remplacer `const { orchestrateur } = hostWiring(…)` par `const { orchestrateur, activity } = hostWiring(base, Date.now, config.socketVm);` ; importer `composeObservers` depuis `../signaling/observers` ; remplacer l'argument `observateurDeSession(base, Date.now),` de `createSignalingServer` par :
```ts
        composeObservers(
            observateurDeSession(base, Date.now),
            ...(activity === undefined ? [] : [activity]),
        ),
```
et, dans `close()`, avant `nettoyage.arreter();` : `activity?.stop();`.

- [ ] **Step 5 : vérifier**

Run: `cd plateforme && npx tsc --noEmit && npx vitest run && wc -l src/http/serveur.ts`
Expected: tout vert ; `serveur.ts` ≤ 480 lignes.

- [ ] **Step 6 : commit**

```bash
git add plateforme/src
git commit -m "feat(plateforme): tell the host while an app window is open

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: le hub attend la VM et rejoue le lancement

**Files:**
- Modify: `client/src/hub/catalogue.ts` (`Refus`, `motifDuService`, `lancerApplication`), `client/src/hub/page.ts` (l.~159-172)
- Create: `client/src/hub/lancement.ts`
- Test: `client/src/hub/lancement.test.ts`, `client/src/hub/catalogue.test.ts`

**Interfaces:**
- Consumes : `lancerApplication`, `DepsCatalogue`, `Issue` (`catalogue.ts`).
- Produces :
  ```ts
  export const LAUNCH_RETRY_MS = 3_000;
  export const LAUNCH_STARTUP_MAX_MS = 180_000;
  export interface LaunchClock { now(): number; sleep(ms: number): Promise<void> }
  export type LaunchResult = { kind: 'done'; issue: Issue<null> } | { kind: 'vm-timeout' };
  export function launchWhenReady(id: string, deps: DepsCatalogue, clock: LaunchClock, onStarting: () => void): Promise<LaunchResult>;
  ```
  `Refus` service gagne `etat?: string`.

- [ ] **Step 1 : test de `catalogue.ts` (échoue)** — dans `catalogue.test.ts`, repérer l'`it()` existant sur `lancerApplication` en refus et son double `fetch` ; ajouter :

```ts
it('🔴 lancerApplication rend `etat` quand le service le porte (503 demarrage)', async () => {
    const fetch = async () =>
        ({
            ok: false,
            status: 503,
            json: async () => ({ refus: 'agent-injoignable', etat: 'demarrage' }),
        }) as never;
    const issue = await lancerApplication('app1', { base: 'http://p', jeton: 't', fetch });
    expect(issue).toEqual({
        etat: 'refus',
        refus: { source: 'service', statut: 503, motif: 'agent-injoignable', etat: 'demarrage' },
    });
});

it('un refus sans `etat` ne porte pas le champ', async () => {
    const fetch = async () =>
        ({ ok: false, status: 504, json: async () => ({ refus: 'delai' }) }) as never;
    const issue = await lancerApplication('app1', { base: 'http://p', jeton: 't', fetch });
    expect(issue).toEqual({
        etat: 'refus',
        refus: { source: 'service', statut: 504, motif: 'delai' },
    });
});
```
(Adapter la forme du double `fetch` à celle des tests voisins si `ReponseHttp` exige d'autres champs.)
Run: `cd client && npx vitest run src/hub/catalogue.test.ts` → FAIL.

- [ ] **Step 2 : implémenter dans `catalogue.ts`**

Type : `| { source: 'service'; statut: number; motif: string; etat?: string };`

Remplacer `motifDuService` par deux fonctions (les 4 appelants existants gardent `motifDuService`) :

```ts
/// Le motif ET, s'il existe, l'`etat` technique que le service a rendus.
///
/// 🔴 `etat` EST UNE VALEUR TECHNIQUE (`demarrage`, `injoignable`), jamais un
/// message : le client ne lit aucun texte du service pour décider — ce qui le
/// garde indépendant de la langue.
async function lireRefus(r: ReponseHttp): Promise<{ motif: string; etat?: string }> {
    try {
        const corps = await r.json();
        if (typeof corps === 'object' && corps !== null) {
            const { refus, etat } = corps as { refus?: unknown; etat?: unknown };
            if (typeof refus === 'string') {
                return typeof etat === 'string' ? { motif: refus, etat } : { motif: refus };
            }
        }
    } catch {
        // Un corps illisible n'est pas plus informatif qu'une absence de corps.
    }
    return { motif: `statut ${r.status}` };
}

async function motifDuService(r: ReponseHttp): Promise<string> {
    return (await lireRefus(r)).motif;
}
```
(garder le commentaire d'origine de `motifDuService` au-dessus de celui-ci, et le raccourcir pour renvoyer à `lireRefus`). Dans `lancerApplication`, remplacer la ligne `if (!r.ok) …` par :

```ts
    if (!r.ok) {
        const { motif, etat } = await lireRefus(r);
        return {
            etat: 'refus',
            refus: { source: 'service', statut: r.status, motif, ...(etat === undefined ? {} : { etat }) },
        };
    }
```
Run: `npx vitest run src/hub/catalogue.test.ts` → PASS.

- [ ] **Step 3 : tests de `launchWhenReady` (échouent)** — `lancement.test.ts`

```ts
import { describe, expect, it, vi } from 'vitest';
import { LAUNCH_RETRY_MS, LAUNCH_STARTUP_MAX_MS, launchWhenReady } from './lancement';

/// Un service qui rend, dans l'ordre, les réponses données, puis la dernière.
function service(reponses: Array<{ status: number; corps: unknown }>) {
    let i = 0;
    const fetch = vi.fn(async () => {
        const r = reponses[Math.min(i++, reponses.length - 1)];
        return { ok: r.status < 400, status: r.status, json: async () => r.corps } as never;
    });
    return { fetch, deps: { base: 'http://p', jeton: 't', fetch } };
}

function horloge() {
    let t = 0;
    const attentes: number[] = [];
    return {
        clock: {
            now: () => t,
            sleep: async (ms: number) => { attentes.push(ms); t += ms; },
        },
        attentes,
    };
}

const DEMARRAGE = { status: 503, corps: { refus: 'agent-injoignable', etat: 'demarrage' } };
const SUCCES = { status: 200, corps: { issue: 'raccourci' } };

describe('launchWhenReady', () => {
    it('un lancement abouti du premier coup : rien n’est annoncé, rien n’est attendu', async () => {
        const { deps, fetch } = service([SUCCES]);
        const { clock, attentes } = horloge();
        const onStarting = vi.fn();
        const r = await launchWhenReady('app1', deps, clock, onStarting);
        expect(r).toEqual({ kind: 'done', issue: { etat: 'ok', valeur: null } });
        expect(fetch).toHaveBeenCalledTimes(1);
        expect(onStarting).not.toHaveBeenCalled();
        expect(attentes).toEqual([]);
    });

    it('🔴 VM en démarrage : annonce UNE fois, rejoue toutes les 3 s, puis réussit', async () => {
        const { deps, fetch } = service([DEMARRAGE, DEMARRAGE, DEMARRAGE, SUCCES]);
        const { clock, attentes } = horloge();
        const onStarting = vi.fn();
        const r = await launchWhenReady('app1', deps, clock, onStarting);
        expect(r.kind).toBe('done');
        expect(fetch).toHaveBeenCalledTimes(4);
        expect(onStarting).toHaveBeenCalledTimes(1);
        expect(attentes).toEqual([LAUNCH_RETRY_MS, LAUNCH_RETRY_MS, LAUNCH_RETRY_MS]);
    });

    it('🔴 une VM qui ne revient jamais : `vm-timeout` après LAUNCH_STARTUP_MAX_MS, sans boucle infinie', async () => {
        const { deps, fetch } = service([DEMARRAGE]);
        const { clock } = horloge();
        const r = await launchWhenReady('app1', deps, clock, () => {});
        expect(r).toEqual({ kind: 'vm-timeout' });
        expect(fetch.mock.calls.length).toBe(LAUNCH_STARTUP_MAX_MS / LAUNCH_RETRY_MS + 1);
    });

    it('🔴 `504 delai` n’est JAMAIS rejoué (l’ordre est peut-être parti : risque de doublon)', async () => {
        const { deps, fetch } = service([{ status: 504, corps: { refus: 'delai' } }, SUCCES]);
        const { clock } = horloge();
        const r = await launchWhenReady('app1', deps, clock, () => {});
        expect(fetch).toHaveBeenCalledTimes(1);
        expect(r).toMatchObject({ kind: 'done', issue: { etat: 'refus', refus: { statut: 504, motif: 'delai' } } });
    });

    it('🔴 un 503 injoignable SANS `demarrage` (réveil impossible) n’est pas rejoué', async () => {
        const refus = { status: 503, corps: { refus: 'agent-injoignable', etat: 'injoignable', reveil: 'hote-inaccessible' } };
        const { deps, fetch } = service([refus, SUCCES]);
        const { clock } = horloge();
        const r = await launchWhenReady('app1', deps, clock, () => {});
        expect(fetch).toHaveBeenCalledTimes(1);
        expect(r.kind).toBe('done');
    });

    it('un refus d’autorisation (401/404) n’est pas rejoué', async () => {
        const { deps, fetch } = service([{ status: 404, corps: { refus: 'vm-inconnue' } }, SUCCES]);
        const { clock } = horloge();
        await launchWhenReady('app1', deps, clock, () => {});
        expect(fetch).toHaveBeenCalledTimes(1);
    });
});
```
Run: FAIL (module absent).

- [ ] **Step 4 : implémenter `lancement.ts`**

```ts
// Lancer une application, en attendant la VM si elle démarre.
//
// 🔴 LE CLIENT LIT `etat`, JAMAIS UN MESSAGE : la valeur `demarrage` est
// technique, donc indépendante de la langue.
//
// 🔴 SEUL `503` + `etat: demarrage` EST REJOUÉ. Dans ce cas `registre.lancer`
// n'a envoyé AUCUN ordre (aucun socket d'agent) : rejouer est sans risque de
// doublon. `504 delai` — l'ordre est PARTI et personne n'a répondu — ne l'est
// jamais : l'application est peut-être déjà lancée.

import { lancerApplication, type DepsCatalogue, type Issue } from './catalogue';

/// Intervalle entre deux tentatives pendant le démarrage. ⚠️ NON CALIBRÉ.
export const LAUNCH_RETRY_MS = 3_000;

/// Attente maximale d'une VM qui démarre. ⚠️ À TENIR ÉGALE à `MAX_WAIT_SECONDS`
/// de `handle-vm-start.sh` (180 s, dépôt `installer`) et à
/// `WAKE_PENDING_MAX_MS` de `plateforme/src/orchestration/wake.ts`.
export const LAUNCH_STARTUP_MAX_MS = 180_000;

/// La valeur technique que la plateforme pose dans `etat` pendant un réveil.
const ETAT_DEMARRAGE = 'demarrage';

export interface LaunchClock {
    now(): number;
    sleep(ms: number): Promise<void>;
}

export type LaunchResult = { kind: 'done'; issue: Issue<null> } | { kind: 'vm-timeout' };

function enDemarrage(issue: Issue<null>): boolean {
    return (
        issue.etat === 'refus' &&
        issue.refus.source === 'service' &&
        issue.refus.statut === 503 &&
        issue.refus.etat === ETAT_DEMARRAGE
    );
}

export async function launchWhenReady(
    id: string,
    deps: DepsCatalogue,
    clock: LaunchClock,
    onStarting: () => void,
): Promise<LaunchResult> {
    const debut = clock.now();
    let annonce = false;
    for (;;) {
        const issue = await lancerApplication(id, deps);
        if (!enDemarrage(issue)) return { kind: 'done', issue };
        if (!annonce) {
            annonce = true;
            onStarting();
        }
        if (clock.now() - debut >= LAUNCH_STARTUP_MAX_MS) return { kind: 'vm-timeout' };
        await clock.sleep(LAUNCH_RETRY_MS);
    }
}
```
Run: `npx vitest run src/hub` → PASS.

- [ ] **Step 5 : brancher le hub** — `client/src/hub/page.ts`

Importer : `import { launchWhenReady } from './lancement';` (et retirer `lancerApplication` de l'import si plus employé ailleurs dans le fichier : `grep -n lancerApplication client/src/hub/page.ts`). Remplacer le `return lancerApplication(application.id, deps).then((issue) => { … });` du `lancer` par :

```ts
                const horloge = {
                    now: () => Date.now(),
                    sleep: (ms: number) => new Promise<void>((r) => setTimeout(r, ms)),
                };
                return launchWhenReady(application.id, deps, horloge, () =>
                    dire('neutre', `La VM démarre… ${application.nom} sera lancée dès qu'elle sera prête.`),
                ).then((resultat) => {
                    if (resultat.kind === 'vm-timeout') {
                        dire('danger', "La VM n'a pas démarré à temps. Réessayez dans un instant.");
                        return;
                    }
                    const { issue } = resultat;
                    if (issue.etat !== 'ok') {
                        dire('danger', `${application.nom} n'a pas pu être lancée : ${issue.refus.motif}.`);
                        return;
                    }
                    dire('succes', `${application.nom} a été lancée.`);
                });
```
Run: `cd client && npx tsc --noEmit && npx vitest run`
Expected: tout vert.

- [ ] **Step 6 : commit**

```bash
git add client/src/hub
git commit -m "feat(client): the hub waits for a starting VM and retries the launch

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: déploiement du package et documentation

**Files:**
- Modify: `hooks/install.py` (dictionnaire d'environnement ~l.322-351), `hooks/assets/desk-plateforme.service`, `tests/test_desk_install.py` (attentes sur les clés de `desk.env` et sur l'unité), `docs/claude/variables-environnement.md`, `docs/claude/index-chantiers.md`

**Interfaces:**
- Consumes : groupe `nivuus-vm` et socket `/run/nivuus/vm-control.sock` (tâche 2).

- [ ] **Step 1 : test (échoue)** — dans `tests/test_desk_install.py`, repérer comment le test lit `desk.env` (`grep -n "PLATEFORME_ICONES\|desk.env" tests/test_desk_install.py`) et l'unité posée (`grep -n "desk-plateforme.service" tests/test_desk_install.py tests/desk_install_fixtures.py`). Ajouter, dans le même style :

```python
check("desk.env pose le socket de contrôle de la VM",
      env.get("PLATEFORME_SOCKET_VM") == "/run/nivuus/vm-control.sock")
check("l'unité plateforme rejoint le groupe du socket de contrôle",
      "SupplementaryGroups=nivuus-vm" in unite_texte)
```
(`env` et `unite_texte` : les variables que le test emploie déjà pour le contenu de `desk.env` et de l'unité posée ; si l'unité n'est pas lue, la lire avec `(racine / "etc/systemd/system/desk-plateforme.service").read_text(encoding="utf-8")`.)
Run: `python3 tests/test_desk_install.py | tail -5` → échec sur les deux nouvelles attentes.

- [ ] **Step 2 : implémenter**

`hooks/install.py`, dans le dictionnaire des variables (près de `"PLATEFORME_ICONES"`) :
```python
        # Le socket de contrôle que `console` pose (nivuus-vm-control.socket) :
        # c'est lui qui réveille la VM au lancement d'une app et qui reçoit le
        # signal « une fenêtre est ouverte ». L'unité rejoint le groupe
        # `nivuus-vm` (voir desk-plateforme.service).
        "PLATEFORME_SOCKET_VM": "/run/nivuus/vm-control.sock",
```
`hooks/assets/desk-plateforme.service`, dans `[Service]`, sous `NoNewPrivileges=yes` :
```ini
# Le groupe du socket de contrôle de la VM (console : nivuus-vm-control.socket).
# C'est le SEUL droit ajouté : la plateforme peut demander « réveille la VM » et
# « une fenêtre est ouverte », et rien d'autre (aucun accès à virsh).
# Le groupe est créé par console (sysusers.d) : desk dépend déjà de console
# (nivuus-package.yaml, requires.packages).
SupplementaryGroups=nivuus-vm
```
Vérifier le comportement de systemd face à un groupe absent **avant** de conclure : `systemd-analyze verify hooks/assets/desk-plateforme.service 2>&1 | head` et `man systemd.exec | grep -A6 '^ *SupplementaryGroups='`. Si la page admet un préfixe `-` pour ignorer un groupe absent, **ne pas l'employer** : un groupe absent doit échouer bruyamment (le canal serait silencieusement inutilisable), et la dépendance `requires.packages: [console]` garantit l'ordre.

- [ ] **Step 3 : documentation**

`docs/claude/variables-environnement.md` : ajouter à la table des variables du serveur (`plateforme/`) :
`| PLATEFORME_SOCKET_VM | facultatif | Chemin absolu du socket de contrôle de la VM (`/run/nivuus/vm-control.sock`). Vide : le réveil et le signal d'activité sont désactivés (`demarrer` refuse `non-supporte`). Un chemin relatif est refusé au démarrage. |` — en reprenant le format exact des lignes voisines.

`docs/claude/index-chantiers.md` : ajouter **une ligne** :
`- **vm-reveil-arret** (30 septembre 2026) — réveil de la VM au lancement d'une app, arrêt après 30 min sans session : [spec](../superpowers/specs/2026-09-30-vm-reveil-arret-design.md), [plan](../superpowers/plans/2026-09-30-vm-reveil-arret.md).`
Et mettre à jour `docs/claude/vm-cycle-de-vie.md` : remplacer « n'est pas démarrée automatiquement » par un court paragraphe en tête : « **Produit** : la VM est réveillée par le lancement d'une app depuis le hub (`PLATEFORME_SOCKET_VM`) et hibernée après 30 min sans session Moonlight ni fenêtre ouverte. **Développement** : inchangé, `virsh start Windows`. »

- [ ] **Step 4 : vérifier**

Run: `python3 tests/test_desk_install.py | tail -3; make test 2>&1 | tail -15; cd plateforme && npx tsc --noEmit && npx vitest run | tail -5; cd ../client && npx tsc --noEmit && npx vitest run | tail -5`
Expected: tout vert. Puis la règle des 500 lignes :
`{ git ls-files; git ls-files --others --exclude-standard; } | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/' | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'` → aucune ligne nouvelle par rapport à `windows_source.rs`.

- [ ] **Step 5 : commit**

```bash
git add hooks tests docs/claude
git commit -m "feat(package): give the platform the VM control socket and document it

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: recette réelle, PR et déploiement

**Files:**
- Create: `docs/superpowers/plans/2026-09-30-vm-reveil-arret-resultats.md` (mesures)

La recette se joue sur la console réelle (VM `Windows`). **Ne rien déployer avant d'avoir relu `MEMORY.md` : la règle permanente sur le déploiement sans confirmation s'y trouve** (`deploiement-sans-demander.md`).

- [ ] **Step 1 : PR `installer`**

```bash
cd ~/Projects/Nivuus/packages/installer
git push -u origin feat/vm-control-socket
gh pr create --title "feat(console): VM control socket (wake on demand from the platform, app-activity idle signal)" \
  --body "Adds nivuus-vm-control.socket (wake, busy), the nivuus-vm group (sysusers.d), nivuus-vm-wake.service, and a third idle condition (app window reported by the platform). Spec: desk docs/superpowers/specs/2026-09-30-vm-reveil-arret-design.md

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
gh pr merge --squash --auto
```

- [ ] **Step 2 : poser `console` sur l'hôte et mesurer l'hôte seul**

```bash
getent group nivuus-vm                       # attendu : le groupe existe (apply_sysusers)
systemctl is-active nivuus-vm-control.socket # attendu : active
ls -l /run/nivuus/vm-control.sock            # attendu : srw-rw---- root nivuus-vm
virsh shutdown Windows; until virsh domstate Windows | grep -q "shut off"; do sleep 3; done
printf 'wake\n' | socat - UNIX-CONNECT:/run/nivuus/vm-control.sock   # attendu : ok
until virsh domstate Windows | grep -q running; do sleep 3; done     # la VM démarre
printf 'busy\n' | socat - UNIX-CONNECT:/run/nivuus/vm-control.sock   # attendu : ok
cat /run/nivuus-vm-idle/app-activity                                 # attendu : un epoch récent
printf 'reboot\n' | socat - UNIX-CONNECT:/run/nivuus/vm-control.sock # attendu : err unknown-verb
```
Noter chaque sortie dans le fichier de résultats. Si `socat` est absent : `python3 -c` avec `socket.AF_UNIX`.

- [ ] **Step 3 : PR `desk` puis déploiement du package**

```bash
cd ~/Projects/Nivuus/packages/desk
git push -u origin feat/vm-wake-on-launch
gh pr create --title "feat: wake the VM when an app is launched, hibernate after 30 min without sessions" \
  --body "Launching an app on a stopped VM asks the host control socket to wake it; the hub retries the launch until the agent is back. The platform reports open app windows (w-N sessions only) so the host keeps the VM up. Spec and plan under docs/superpowers.

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```
Déployer par `nivuus check / update / adopt` (voir `Nivuus/CLAUDE.md`). Vérifier : `systemctl status desk-plateforme` actif, `id $(systemctl show -p MainPID --value desk-plateforme | xargs -I{} ps -o user= -p {})` ne suffit pas (DynamicUser) → lire `journalctl -u desk-plateforme -n 50` : aucune ligne `wake refused` ni `busy signal failed` au repos.

- [ ] **Step 4 : les deux scénarios de bout en bout (mesurés, datés)**

1. **Réveil** : VM éteinte (`virsh domstate Windows` → `shut off`), ouvrir le hub, lancer une app. Attendu : « La VM démarre… », puis « … a été lancée. ». Relever le délai total et `journalctl -t vm-control -t vm-trigger-47984 --since -5min`.
2. **Arrêt** : fermer toutes les fenêtres (hub ouvert, **aucune** app), aucune session Moonlight ; `journalctl -t vm-idle-shutdown -f` doit montrer `app=0` puis `strikes=1/3, 2/3, 3/3`, puis l'hibernation, **environ 30 min** après la fermeture de la dernière fenêtre (et non 50-60 min). Le hub laissé ouvert pendant ce temps ne doit **pas** retarder l'arrêt.
3. **Contre-épreuve** : refaire 2 avec **une** fenêtre d'app ouverte : `app=1`, `strikes=0/3` en continu, aucune hibernation.

- [ ] **Step 5 : consigner et clore**

Écrire `docs/superpowers/plans/2026-09-30-vm-reveil-arret-resultats.md` avec les sorties réelles des étapes 2 à 4, dont **ce qui n'a pas pu être mesuré** (par exemple si l'arrêt n'a pas été attendu jusqu'au bout, le dire tel quel). Puis :

```bash
git add docs/superpowers/plans/2026-09-30-vm-reveil-arret-resultats.md
git commit -m "docs: results of the VM wake-on-launch and idle shutdown acceptance run

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
git push
```

---

## Auto-revue (faite à l'écriture)

- **Couverture de la spec** : §3.1 canal → tâches 1-2 ; §3.2 arrêt → tâche 3 ; §3.3 orchestrateur/état/config → 4-7, activité → 8 ; route de lancement → 7 ; §3.4 client → 9 ; erreurs §4 → tests 5, 6, 7, 8 ; déploiement/doc → 10 ; recette §5 → 11. **Hors périmètre** inchangé (`arreter`, `instantane`, réveil à la connexion).
- **Trois corrections de la spec** portées ici : déclencheur sur `POST /application/:id/lancer` ; seuil `busy` de 180 s et non 30 min ; seules les sessions `w-N` comptent.
- **Cohérence des noms** : `envoyerAuCanal`/`ReponseHote`/`VerbeHote` (5) → `Wake` (6) → `hostOrchestrator`/`hostWiring`/`CablageHote` (6, élargi en 8) → `createAppActivity`/`AppActivity`/`composeObservers` (8) → `launchWhenReady` (9). Constantes miroirs : `BUSY_PERIOD_MS` ↔ `APP_HEARTBEAT_S`, `LAUNCH_STARTUP_MAX_MS` ↔ `WAKE_PENDING_MAX_MS` ↔ `MAX_WAIT_SECONDS`.
- **Non testé automatiquement, et dit** : `apply_sysusers` (appelé seulement sous `root == "/"`) et le comportement réel de `SupplementaryGroups` avec `DynamicUser` — tous deux éprouvés en recette (tâche 11).
- **Hypothèses à confirmer en lisant les harnais existants avant d'écrire les tests** (tâche 7) : noms des fonctions `servir`/`lancer` de `routes-applications.test.ts`, et aptitude du harnais à recevoir un `orchestrateur` de remplacement.
