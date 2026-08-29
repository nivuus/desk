#!/usr/bin/env python3
"""Tests du module `hooks/vm.py` : ce que `desk` pose DANS la VM Windows,
par le chemin WinRM de `console` (ProjFS, VB-Audio).

🔴 AUCUN APPEL WINRM RÉEL N'EST FAIT ICI, ET AUCUN PAQUET NE QUITTE CETTE
MACHINE. La plupart des tests de comportement injectent un exécuteur FACTICE
(`faux_winrm_rendant`), qui court-circuite tout sous-processus.

⚠️ MAIS « AUCUN `subprocess.run` N'EST ATTEINT » SERAIT FAUX, et cet en-tête
l'a écrit jusqu'au 30 août 2026 (relevé par la revue finale de branche) : les
scénarios A et B en bas de ce fichier passent DÉLIBÉRÉMENT par
`executer_winrm_reel`, donc par un vrai `subprocess.run`, pour éprouver son
bras d'échec et son round-trip — deux chemins qu'aucun exécuteur factice ne
peut couvrir. Ce qu'ils lancent est un FAUX `winrm_exec.py` de quatre lignes,
posé sous un `NIVUUS_PACKAGES_DIR` temporaire : le sous-processus est réel,
la VM et le réseau ne le sont jamais.

`hooks/vm.py` n'est pas un hook exécutable seul (pas de `--phase`/stdin
JSON) : c'est un module importé par `hooks/activate.py`, au même titre que
`hooks/commun.py`. Il est donc chargé ici par chemin de fichier explicite
(`importlib`), comme le fait déjà la ROUGE de `test_desk_activate.py` pour
appeler une fonction interne du hook directement.

Run: python3 tests/test_desk_vm.py
"""
import importlib.util
import os
import pathlib
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
MODULE_PATH = RACINE / "hooks" / "vm.py"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


def charger_module():
    spec = importlib.util.spec_from_file_location("desk_vm", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


vm = charger_module()


# --- Le faux exécuteur : enregistre chaque commande, ne touche jamais la
# VM ni le réseau -----------------------------------------------------------

class FauxWinRM:
    def __init__(self, sortie=""):
        self.sortie = sortie
        self.commandes = []

    def __call__(self, mode, commande):
        self.commandes.append(commande)
        return self.sortie


def faux_winrm_rendant(sortie=""):
    return FauxWinRM(sortie)


# === ProjFS : le redémarrage se CONSTATE et se DIT, il ne se prend pas =====

# --- Bras 1 : la VM répond qu'un redémarrage est requis --------------------
faux = faux_winrm_rendant("RestartNeeded")
etat = vm.poser_projfs(executer=faux)
check("le redemarrage requis est rapporte", etat.redemarrage_requis, True)
check("aucun redemarrage n'a ete demande",
      any("Restart-Computer" in c for c in faux.commandes), False)
check("une seule commande a atteint l'executeur", len(faux.commandes), 1)
check("la commande active bien Client-ProjFS",
      "Client-ProjFS" in faux.commandes[0], True)
check("la commande porte -NoRestart (le redemarrage ne se PREND jamais)",
      "-NoRestart" in faux.commandes[0], True)

# --- Bras 2 : la VM répond qu'aucun redémarrage n'est requis ---------------
faux2 = faux_winrm_rendant("")
etat2 = vm.poser_projfs(executer=faux2)
check("aucun redemarrage requis quand la VM ne le rapporte pas",
      etat2.redemarrage_requis, False)
check("aucun redemarrage n'a ete demande (bras 2 non plus)",
      any("Restart-Computer" in c for c in faux2.commandes), False)


# === VB-Audio : licence PERSONNELLE seulement ==============================

# --- Désarmé (le défaut du wizard) : RIEN ne part vers la VM ---------------
faux3 = faux_winrm_rendant("")
vm.poser_vb_audio(armee=False, executer=faux3)
check("desarme : aucune commande n'atteint la VM", faux3.commandes, [])

# --- Désarmé par défaut (sans passer `armee` du tout) ----------------------
faux4 = faux_winrm_rendant("")
vm.poser_vb_audio(executer=faux4)
check("desarme par defaut (armee omis) : aucune commande n'atteint la VM",
      faux4.commandes, [])

# --- Armé : aucun payload n'existe (voir hooks/vm.py::poser_vb_audio) — le
# lot lève FORT plutôt que de deviner un installateur qui n'existe pas.
# Toujours AUCUNE commande ne doit atteindre l'exécuteur avant ce refus. ---
faux5 = faux_winrm_rendant("")
a_leve_vb = False
try:
    vm.poser_vb_audio(armee=True, executer=faux5)
except NotImplementedError:
    a_leve_vb = True
check("arme sans payload : leve fort plutot que d'inventer un installateur",
      a_leve_vb, True)
check("arme sans payload : aucune commande n'a quand meme atteint la VM",
      faux5.commandes, [])


# === La ROUGE — Step 5 : le contrat inter-packages doit se voir manquer ====
# Un `NIVUUS_PACKAGES_DIR` qui ne porte PAS `console/guest/winrm_exec.py`
# doit faire lever `poser_projfs()` (exécuteur réel, non injecté) avec une
# exception NOMMANT le chemin cherché — jamais un appel réseau silencieux,
# jamais une trace Python générique. C'est le bras qui distingue « console
# absent » de « rien à faire ».
ancien_packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR")
try:
    with tempfile.TemporaryDirectory() as tmp:
        os.environ["NIVUUS_PACKAGES_DIR"] = tmp
        chemin_attendu = str(pathlib.Path(tmp) / "console" / "guest" / "winrm_exec.py")

        a_leve = False
        message = ""
        try:
            vm.poser_projfs()  # executer=None : passe par l'exécuteur REEL
        except FileNotFoundError as exc:
            a_leve = True
            message = str(exc)
        check("ROUGE : winrm_exec.py absent fait lever poser_projfs()",
              a_leve, True)
        check("ROUGE : l'exception nomme le chemin cherche",
              chemin_attendu in message, True)
        print(f"ROUGE (sortie reelle) : {message}")
finally:
    if ancien_packages_dir is None:
        os.environ.pop("NIVUUS_PACKAGES_DIR", None)
    else:
        os.environ["NIVUUS_PACKAGES_DIR"] = ancien_packages_dir


# --- Contrôle positif de la résolution : winrm_exec.py PRÉSENT sous
# NIVUUS_PACKAGES_DIR est trouvé sans lever, à l'emplacement exact --------
ancien_packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR")
try:
    with tempfile.TemporaryDirectory() as tmp:
        guest_dir = pathlib.Path(tmp) / "console" / "guest"
        guest_dir.mkdir(parents=True)
        (guest_dir / "winrm_exec.py").write_text("# factice\n", encoding="utf-8")
        os.environ["NIVUUS_PACKAGES_DIR"] = tmp

        chemin = vm.chemin_winrm_exec()
        check("winrm_exec.py present est resolu sans lever",
              str(chemin), str(guest_dir / "winrm_exec.py"))
finally:
    if ancien_packages_dir is None:
        os.environ.pop("NIVUUS_PACKAGES_DIR", None)
    else:
        os.environ["NIVUUS_PACKAGES_DIR"] = ancien_packages_dir


# === Ronde de correction 1 : le bras d'échec RÉEL de `executer_winrm_reel` =
# 🔴 Jusqu'ici, AUCUN test ne passait par le vrai sous-processus : tous les
# tests de comportement ci-dessus injectent `FauxWinRM`, qui court-circuite
# `executer_winrm_reel` (et donc son `subprocess.run`) entièrement. Le bras
# `if proc.returncode != 0: raise RuntimeError(...)` de `hooks/vm.py` n'était
# donc éprouvé par RIEN — un contrôle qu'on n'a jamais vu rouge n'est pas un
# contrôle. Les deux scénarios ci-dessous posent un VRAI fichier
# `console/guest/winrm_exec.py` (un script Python autonome, jamais la VM) et
# appellent `poser_projfs()` SANS exécuteur injecté (`executer=None`), pour
# que `executer_winrm_reel` — donc `subprocess.run` — soit réellement exercé.

def poser_script_winrm_reel(packages_dir: pathlib.Path, corps: str) -> None:
    """Un VRAI fichier `console/guest/winrm_exec.py` (jamais la VM) : un
    script Python autonome dont `corps` est le contenu intégral."""
    guest_dir = packages_dir / "console" / "guest"
    guest_dir.mkdir(parents=True, exist_ok=True)
    script = guest_dir / "winrm_exec.py"
    script.write_text(corps, encoding="utf-8")
    script.chmod(0o755)


# --- A : la commande distante ÉCHOUE (code de sortie non nul, message sur
# stderr) — `executer_winrm_reel` doit lever `RuntimeError`, et le MESSAGE
# de cette exception doit porter la raison distante, pas seulement lever. --
ancien_packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR")
try:
    with tempfile.TemporaryDirectory() as tmp:
        packages_dir = pathlib.Path(tmp)
        poser_script_winrm_reel(packages_dir, (
            "#!/usr/bin/env python3\n"
            "import sys\n"
            # ⚠️ LE TEXTE DIT QU'IL EST FACTICE, ET C'EST DÉLIBÉRÉ : la
            # version précédente imprimait « error: cannot reach guest at
            # 192.168.3.2:5985: timeout », que `make test` affichait telle
            # quelle en sortie NORMALE — la relectrice de la revue finale a
            # cru, en la lisant, que la suite avait touché la VM. Un texte de
            # banc qui ne peut pas se confondre avec une panne réelle coûte
            # une ligne.
            "sys.stderr.write('FAUX winrm_exec.py de test : echec simule, "
            "aucune VM contactee\\n')\n"
            "sys.exit(1)\n"
        ))
        os.environ["NIVUUS_PACKAGES_DIR"] = str(packages_dir)

        a_leve = False
        message = ""
        try:
            vm.poser_projfs()  # executer=None : passe par executer_winrm_reel REEL
        except RuntimeError as exc:
            a_leve = True
            message = str(exc)
        check("executeur reel, echec : RuntimeError leve", a_leve, True)
        check("executeur reel, echec : le message nomme le mode et le code",
              "winrm_exec.py ps a echoue (code 1)" in message, True)
        check("executeur reel, echec : le message porte la raison distante",
              "echec simule" in message, True)
        print(f"ROUGE (executeur reel, echec) : {message}")
finally:
    if ancien_packages_dir is None:
        os.environ.pop("NIVUUS_PACKAGES_DIR", None)
    else:
        os.environ["NIVUUS_PACKAGES_DIR"] = ancien_packages_dir


# --- B (symétrique, promu de Mineure) : la commande distante RÉUSSIT en
# émettant `RestartNeeded` — le round-trip réel stdout -> redemarrage_requis
# est ainsi couvert directement, pas seulement par ricochet via FauxWinRM. --
ancien_packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR")
try:
    with tempfile.TemporaryDirectory() as tmp:
        packages_dir = pathlib.Path(tmp)
        poser_script_winrm_reel(packages_dir, (
            "#!/usr/bin/env python3\n"
            "import sys\n"
            "sys.stdout.write('RestartNeeded\\n')\n"
            "sys.exit(0)\n"
        ))
        os.environ["NIVUUS_PACKAGES_DIR"] = str(packages_dir)

        etat = vm.poser_projfs()  # executer=None : passe par executer_winrm_reel REEL
        check("executeur reel, succes : round-trip stdout -> redemarrage_requis",
              etat.redemarrage_requis, True)
finally:
    if ancien_packages_dir is None:
        os.environ.pop("NIVUUS_PACKAGES_DIR", None)
    else:
        os.environ["NIVUUS_PACKAGES_DIR"] = ancien_packages_dir


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du module vm passés")
