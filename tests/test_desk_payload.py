#!/usr/bin/env python3
"""Tests de la tâche 7 : déposer `agent.exe` là où `console` va le chercher.

`console/guest/fetch_payload.py:61` définit
`PACKAGED_AGENT_EXE = Path(__file__).resolve().parent / "payload" / "agent"
/ "agent.exe"` — un chemin FIXE, vendorisé dans le dépôt `console` lui-même
(voir `console/guest/payload/agent/README.md`), retrouvé sous
`<NIVUUS_PACKAGES_DIR>/console/` (même convention que
`hooks/vm.py::chemin_winrm_exec`). C'est cette cible, et NULLE PART
ailleurs, qu'`activate.py::chemin_agent_console()`/`deposer_agent_console()`
visent. Ce n'est PAS `<drivers_dir>/agent/agent.exe` (la racine de
construction de l'ISO) : ce chemin dépend de la réponse de wizard
`guest_workdir` DE `console`, que ce hook ne reçoit jamais (voir le
docstring de tête d'`activate.py` pour le détail de cette élimination).

🔴 AUCUNE COMPILATION RÉELLE ICI. Les tests unitaires injectent un
`construire` Python factice directement dans `deposer_agent_console()` ;
le test de bout en bout (sous-processus) emploie `DESK_BUILD_AGENT_SCRIPT`
(posé par `desk_activate_fixtures.appeler()`) pour remplacer
`scripts/build-agent-croise.sh` par un script qui ne compile rien — jamais
le vrai script, qui prendrait ~40 s et exigerait une boîte à outils croisée.

Run: python3 tests/test_desk_payload.py
"""
import importlib.util
import os
import pathlib
import sys
import tempfile

from desk_activate_fixtures import (
    HOOK,
    appeler,
    poser_faux_fetch_payload,
    poser_faux_npm,
    poser_faux_winrm_exec,
    poser_racine_installee,
)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# `hooks/activate.py` fait `from vm import ...` / `from env_fichier import
# ...` / `from administration import ...` : un import par CHEMIN
# (spec_from_file_location) ne passe pas par le mécanisme qui ajoute
# automatiquement le répertoire du script à sys.path (précédent de
# test_desk_activate.py, tâche 6).
sys.path.insert(0, str(HOOK.parent))
spec = importlib.util.spec_from_file_location("desk_activate_payload", HOOK)
activate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(activate)


class _EnvTemporaire:
    """Pose une variable d'environnement pour la durée du bloc `with`, la
    restaure exactement à sa valeur (ou son absence) d'avant. Évite de
    polluer les tests suivants avec `NIVUUS_PACKAGES_DIR` d'un scénario
    précédent."""

    def __init__(self, **valeurs):
        self.valeurs = valeurs
        self.anciennes = {}

    def __enter__(self):
        for cle, valeur in self.valeurs.items():
            self.anciennes[cle] = os.environ.get(cle)
            os.environ[cle] = valeur
        return self

    def __exit__(self, *_exc):
        for cle, ancienne in self.anciennes.items():
            if ancienne is None:
                os.environ.pop(cle, None)
            else:
                os.environ[cle] = ancienne


# === chemin_agent_console() : résolution, le nom et le sous-répertoire ====
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    packages_dir = root / "packages"
    poser_faux_winrm_exec(packages_dir, root / "winrm.log")
    poser_faux_fetch_payload(packages_dir)

    with _EnvTemporaire(NIVUUS_PACKAGES_DIR=str(packages_dir)):
        cible = activate.chemin_agent_console()

    check("nom du fichier resolu", cible.name, "agent.exe")
    check("sous-repertoire resolu (contrat REQUIRED_BINARIES)", cible.parent.name, "agent")
    check("chemin complet resolu",
          cible,
          packages_dir / "console" / "guest" / "payload" / "agent" / "agent.exe")


# === ROUGE du contrat inter-packages : console absent (fetch_payload.py
# absent) -> chemin_agent_console() LÈVE, jamais une invention de chemin ===
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    packages_dir = root / "packages-sans-console"

    with _EnvTemporaire(NIVUUS_PACKAGES_DIR=str(packages_dir)):
        a_leve = False
        try:
            activate.chemin_agent_console()
        except FileNotFoundError as exc:
            a_leve = True
            message = str(exc)

    check("chemin_agent_console() leve si fetch_payload.py absent", a_leve, True)
    check("le message nomme fetch_payload.py", "fetch_payload.py" in message, True)


# === deposer_agent_console() : le fichier depose EST agent.exe/agent/ =====
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    packages_dir = root / "packages"
    poser_faux_winrm_exec(packages_dir, root / "winrm.log")
    poser_faux_fetch_payload(packages_dir)

    appels = []

    def faux_construire(destination):
        appels.append(destination)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(b"faux-binaire-de-test")

    with _EnvTemporaire(NIVUUS_PACKAGES_DIR=str(packages_dir)):
        cible = activate.deposer_agent_console(construire=faux_construire)

    check("un seul appel a construire", len(appels), 1)
    check("le nom du fichier depose est agent.exe (contrat REQUIRED_BINARIES)",
          cible.name, "agent.exe")
    check("le sous-repertoire est bien 'agent'", cible.parent.name, "agent")
    check("le fichier existe reellement sur le disque", cible.is_file(), True)
    check("le contenu est celui depose par construire()",
          cible.read_bytes(), b"faux-binaire-de-test")


# === ROUGE : construire() qui ne depose rien -> RuntimeError, jamais une
# reussite silencieuse ("un controle qu'on n'a jamais vu rouge n'est pas
# un controle") ============================================================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    packages_dir = root / "packages"
    poser_faux_winrm_exec(packages_dir, root / "winrm.log")
    poser_faux_fetch_payload(packages_dir)

    def construire_muet(_destination):
        pass  # ne cree jamais le fichier : simule un script silencieusement no-op

    with _EnvTemporaire(NIVUUS_PACKAGES_DIR=str(packages_dir)):
        a_leve = False
        try:
            activate.deposer_agent_console(construire=construire_muet)
        except RuntimeError:
            a_leve = True

    check("deposer_agent_console() leve si construire() n'a rien depose", a_leve, True)


# === construire_agent_reel() : le VRAI sous-processus, mais un script
# FACTICE (DESK_BUILD_AGENT_SCRIPT) — jamais build-agent-croise.sh reel ====
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    script = root / "faux-build.py"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import pathlib, sys\n"
        "d = pathlib.Path(sys.argv[1])\n"
        "d.mkdir(parents=True, exist_ok=True)\n"
        "(d / 'agent.exe').write_bytes(b'faux-pe32-de-test')\n",
        encoding="utf-8")
    script.chmod(0o755)

    with _EnvTemporaire(DESK_BUILD_AGENT_SCRIPT=str(script)):
        cible = root / "cible" / "agent" / "agent.exe"
        activate.construire_agent_reel(cible)

    check("construire_agent_reel() invoque le script factice et depose le fichier",
          cible.is_file(), True)
    check("le contenu vient bien du script factice",
          cible.read_bytes(), b"faux-pe32-de-test")

# --- ROUGE : le script echoue (code non nul) -> RuntimeError, portant sa
# sortie -----------------------------------------------------------------
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    script = root / "faux-build-echoue.sh"
    script.write_text("#!/bin/sh\necho 'erreur simulee de build' >&2\nexit 1\n",
                       encoding="utf-8")
    script.chmod(0o755)

    with _EnvTemporaire(DESK_BUILD_AGENT_SCRIPT=str(script)):
        a_leve = False
        try:
            activate.construire_agent_reel(root / "cible" / "agent" / "agent.exe")
        except RuntimeError as exc:
            a_leve = True
            detail = str(exc)

    check("construire_agent_reel() leve si le script echoue", a_leve, True)
    check("le message nomme l'erreur du script", "erreur simulee de build" in detail, True)


# === Le hook COMPLET, en sous-processus : console PARTIELLEMENT present
# (winrm_exec.py present, fetch_payload.py absent) -> refus propre, APRES
# que ProjFS/VB-Audio (tache 6) aient reussi ================================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    packages_dir = root / "packages-incomplets"
    poser_faux_winrm_exec(packages_dir, root / "winrm.log")  # console PARTIEL

    r = appeler(root, bin_dir, packages_dir=packages_dir)
    check("hook complet: refus propre si fetch_payload.py absent (rc != 0)",
          r.returncode != 0, True)
    check("le refus nomme fetch_payload.py",
          "fetch_payload.py" in (r.stderr or ""), True)
    check("aucune trace Python (Traceback) n'atteint l'operateur",
          "Traceback" in (r.stderr or ""), False)
    check("aucune commande npm lancee (le refus precede la creation du compte)",
          (root / "npm.log").exists(), False)
    # ProjFS a quand meme du reussir avant ce refus (un seul appel, le
    # sien) : la preuve que l'ordre place bien le depot APRES la tache 6.
    check("ProjFS a quand meme tourne avant le refus (chemin WinRM present)",
          (root / "winrm.log").exists(), True)


# === Le hook COMPLET, en sous-processus : succes de bout en bout, avec le
# script de build FACTICE pose par defaut par appeler() =====================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("hook complet: succes de bout en bout (rc == 0)", r.returncode, 0)
    if r.returncode != 0:
        failures.append(f"stderr du hook : {r.stderr!r}")
    packages_dir_defaut = root / "faux-packages-dir"
    cible = (packages_dir_defaut / "console" / "guest" / "payload" / "agent"
             / "agent.exe")
    check("agent.exe est bien depose au chemin attendu", cible.is_file(), True)


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du depot de agent.exe (tache 7) passes")
