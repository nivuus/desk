#!/usr/bin/env python3
"""Tests du hook activate du package desk.

Le hook est éprouvé par son VRAIE interface — un sous-processus appelé
`--phase activate --root <racine>`, nourri de {"hw":…, "answers":…} sur
stdin — exactement comme `installer/packages/runner.py::run_activate`
l'invoque (`hw` porte les facts de resolve, FUSIONNÉS ; aucun `--root`
n'est passé en production, mais `argparse` lui donne le même défaut `/`
que `console/hooks/activate.py`, ce qui rend le hook appelable de la même
façon dans un test).

Chaque test pose sa PROPRE racine sous `tempfile.TemporaryDirectory()` :
jamais `/etc/systemd/system`, `/opt` ou le PATH réel du poste qui fait
tourner ces tests. `npm` et `systemctl` sont remplacés par des scripts
factices posés sur un `PATH` reconstruit pour chaque appel — jamais le
`npm`/`systemctl` du système réel.

🔴 TÂCHE 6 : `main()` résout `winrm_exec.py` avant `plateforme/` ; `appeler()`
fabrique donc par défaut un faux (`packages_dir=False` simule `console` absent).

Run: python3 tests/test_desk_activate.py
"""
import configparser
import json
import os
import pathlib
import stat
import subprocess
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "activate.py"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


REPONSES = {"admin_email": "ada@exemple.test", "admin_password": "hunter2hunter2",
            "auth_mode": "motdepasse", "vb_audio": False}

# Les facts de resolve, telles qu'elles arrivent RÉELLEMENT : fusionnées
# dans hw (installer/packages/runner.py::run_activate, ligne 337), jamais
# sous une clé "facts" séparée.
HW_AVEC_FACTS = {"vm_repond": True, "node_version": "24.9.0",
                  "turn_ecoute": "203.0.113.9", "turn_relais": "203.0.113.9",
                  "port": 9999}


# --- Le faux npm : enregistre chaque invocation, ne touche jamais un vrai
# Node ni une vraie base ---------------------------------------------------

FAUX_NPM = """#!/usr/bin/env python3
import json, os, sys

argv = sys.argv[1:]
stdin_data = sys.stdin.read()
with open({log!r}, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({{"argv": ["npm", *argv], "cwd": os.getcwd(),
                          "stdin": stdin_data,
                          "env_base_url": os.environ.get("PLATEFORME_BASE_URL", ""),
                          }}) + "\\n")

if "admin:utilisateur" in argv:
    sys.stdout.write("u-test-0001\\n")
    sys.exit(0)
if "admin:agent" in argv:
    sys.stdout.write("vm_id=vm-test-uuid\\nprefixe=abcd\\n"
                      "AGENT_SECRET=secret-de-test-0123456789abcdef\\n")
    sys.exit(0)
sys.exit(1)
"""


def poser_faux_npm(bin_dir: pathlib.Path, log: pathlib.Path) -> None:
    script = bin_dir / "npm"
    script.write_text(FAUX_NPM.format(log=str(log)), encoding="utf-8")
    script.chmod(0o755)


# --- Le faux winrm_exec.py (tâche 6, contrat `<script> {mode} <commande>`) --
FAUX_WINRM_EXEC = """#!/usr/bin/env python3
import json, sys
with open({log!r}, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({{"argv": sys.argv[1:]}}) + "\\n")
sys.exit(0)
"""


def poser_faux_winrm_exec(packages_dir: pathlib.Path, log: pathlib.Path) -> None:
    guest_dir = packages_dir / "console" / "guest"
    guest_dir.mkdir(parents=True, exist_ok=True)
    script = guest_dir / "winrm_exec.py"
    script.write_text(FAUX_WINRM_EXEC.format(log=str(log)), encoding="utf-8")
    script.chmod(0o755)


def poser_faux_systemctl(bin_dir: pathlib.Path, log: pathlib.Path) -> None:
    """Un systemctl qui n'agit sur RIEN : juste une trace de ses arguments,
    pour prouver qu'il n'est jamais invoqué sous un --root de test."""
    script = bin_dir / "systemctl"
    script.write_text(
        "#!/bin/sh\n"
        f'echo "$@" >> {log}\n'
        "exit 0\n",
        encoding="utf-8",
    )
    script.chmod(0o755)


def lire_commandes(log: pathlib.Path):
    if not log.exists():
        return []
    commandes = []
    for ligne in log.read_text(encoding="utf-8").splitlines():
        if ligne.strip():
            commandes.append(json.loads(ligne))
    return commandes


# --- Fabrique une racine où `install` aurait déjà tourné -------------------

def poser_racine_installee(root: pathlib.Path, contenu_env: dict = None) -> None:
    unite_dir = root / "etc" / "systemd" / "system"
    unite_dir.mkdir(parents=True, exist_ok=True)
    # Copie l'unité RÉELLE de la tâche 4 — c'est elle dont le WantedBy=
    # décide sous quel .wants/ le lien doit vivre.
    (unite_dir / "desk-plateforme.service").write_text(
        (RACINE / "hooks" / "assets" / "desk-plateforme.service").read_text(encoding="utf-8"),
        encoding="utf-8",
    )

    plateforme_dir = root / "opt" / "nivuus" / "desk" / "plateforme"
    plateforme_dir.mkdir(parents=True, exist_ok=True)

    env_dir = root / "etc" / "nivuus"
    env_dir.mkdir(parents=True, exist_ok=True)
    valeurs = contenu_env if contenu_env is not None else {
        "PLATEFORME_BASE": "sqlite",
        "PLATEFORME_BASE_URL": "/var/lib/nivuus-desk/plateforme.sqlite",
        "PLATEFORME_SECRET_JETON": "x" * 64,
    }
    corps = "\n".join(f"{k}={v}" for k, v in valeurs.items()) + "\n"
    chemin_env = env_dir / "desk.env"
    chemin_env.write_text(corps, encoding="utf-8")
    os.chmod(chemin_env, stat.S_IRUSR | stat.S_IWUSR)


def appeler(root, bin_dir, hw=None, answers=None, root_arg=None, packages_dir=None):
    """Appelle le hook (--phase, --root), PATH vers bin_dir en tête (faux npm/systemctl).
    packages_dir (tâche 6, NIVUUS_PACKAGES_DIR) : None fabrique un faux, False simule console absent."""
    contexte = {"hw": hw if hw is not None else HW_AVEC_FACTS,
                "answers": answers if answers is not None else REPONSES}
    env = dict(os.environ)
    env["PATH"] = str(bin_dir) + os.pathsep + env.get("PATH", "")
    # Le contrôle ② veut prouver que PLATEFORME_BASE_URL atteint npm PAR LA
    # FUSION que le hook fait depuis desk.env — jamais parce que le shell qui
    # fait tourner ces tests l'exportait déjà par accident.
    env.pop("PLATEFORME_BASE_URL", None)
    if packages_dir is None:
        packages_dir = root / "faux-packages-dir"
        poser_faux_winrm_exec(packages_dir, root / "winrm.log")
    elif packages_dir is False:
        packages_dir = root / "console-absent-ici"
    env["NIVUUS_PACKAGES_DIR"] = str(packages_dir)
    cmd = [sys.executable, str(HOOK), "--phase", "activate",
           "--root", str(root_arg if root_arg is not None else root)]
    return subprocess.run(cmd, input=json.dumps(contexte), env=env,
                           capture_output=True, text=True)


def load_unit(path):
    parser = configparser.ConfigParser(strict=False, interpolation=None)
    parser.optionxform = str
    parser.read(path, encoding="utf-8")
    return parser


def lire_env(chemin):
    valeurs = {}
    for ligne in chemin.read_text(encoding="utf-8").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle, _, valeur = ligne.partition("=")
        valeurs[cle] = valeur
    return valeurs


# --- L'unité RÉELLEMENT posée par la tâche 4 pointe-t-elle où ce hook croit ?
# ⚠️ Garde de synchronisation : si `hooks/assets/desk-plateforme.service`
# changeait de cible (WantedBy=), `WANTS_SUBDIR` d'`activate.py` doit suivre
# — ce test rougirait avant que le lien ne se pose au mauvais endroit.
ini_unite = load_unit(RACINE / "hooks" / "assets" / "desk-plateforme.service")
check("l'unite est WantedBy=multi-user.target (hypothese d'activate.py)",
      ini_unite.get("Install", "WantedBy", fallback=""), "multi-user.target")


# === Scénario 1 : activation complète, réussie ============================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)

    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    log_npm = root / "npm.log"
    poser_faux_npm(bin_dir, log_npm)
    log_systemctl = root / "systemctl.log"
    poser_faux_systemctl(bin_dir, log_systemctl)

    r = appeler(root, bin_dir)
    check("le hook reussit (rc=0)", r.returncode, 0)
    if r.returncode != 0:
        failures.append(f"stderr du hook : {r.stderr!r}")

    # --- L'armement : un lien, jamais un enable ---------------------------
    lien = root / "etc" / "systemd" / "system" / "multi-user.target.wants" / "desk-plateforme.service"
    check("le service est arme par un lien", os.path.islink(lien), True)
    check("le lien pointe vers une unite qui existe",
          os.path.exists(os.path.realpath(lien)), True)
    check("le lien pointe bien VERS l'unite posee par install",
          os.path.realpath(lien),
          os.path.realpath(root / "etc" / "systemd" / "system" / "desk-plateforme.service"))

    # --- systemctl JAMAIS invoque sous un --root de test -------------------
    check("aucun daemon-reload/start sous --root != /",
          log_systemctl.exists(), False)

    # --- Le mot de passe ne transite QUE par stdin --------------------------
    commandes_vues = [c["argv"] for c in lire_commandes(log_npm)]
    reponses = REPONSES
    check("aucun mot de passe dans la ligne de commande",
          any("--password" in c or reponses["admin_password"] in " ".join(c)
              for c in commandes_vues), False)

    # --- Il a bien ete recu, mais sur stdin -----------------------------
    entrees = lire_commandes(log_npm)
    appel_utilisateur = next(c for c in entrees if "admin:utilisateur" in c["argv"])
    check("le mot de passe arrive sur stdin de admin:utilisateur",
          appel_utilisateur["stdin"].strip(), reponses["admin_password"])
    check("--email est passe en argv (lui n'est pas un secret)",
          "ada@exemple.test" in appel_utilisateur["argv"], True)

    # --- L'ordre : compte AVANT enrolement ----------------------------------
    ordre = [c["argv"] for c in entrees]
    idx_compte = next(i for i, c in enumerate(ordre) if "admin:utilisateur" in c)
    idx_agent = next(i for i, c in enumerate(ordre) if "admin:agent" in c)
    check("le compte est cree AVANT l'enrolement de l'agent", idx_compte < idx_agent, True)

    # --- admin:agent : mode ENROLEMENT, --vm est un NOM, --adresse posee ---
    appel_agent = next(c for c in entrees if "admin:agent" in c["argv"])
    check("--vm est present", "--vm" in appel_agent["argv"], True)
    check("--adresse est present", "--adresse" in appel_agent["argv"], True)
    check("--roter n'est PAS employe (ce hook n'enrole qu'une fois)",
          "--roter" in appel_agent["argv"], False)

    # --- L'environnement transmis a npm porte la config de desk.env --------
    # 🔴 CORRECTION, RONDE 1 : la version precedente de ce controle regardait
    # "cwd" sous une etiquette qui parlait de PLATEFORME_BASE_URL — il ne
    # pouvait pas rougir, cwd est pose INCONDITIONNELLEMENT par lancer_npm(),
    # que la fusion d'environnement soit correcte ou non. Le faux npm
    # journalise desormais la valeur REELLEMENT recue par le processus fils
    # (env_base_url, voir FAUX_NPM) ; on la compare a celle ecrite par
    # poser_racine_installee() dans desk.env.
    valeur_attendue = "/var/lib/nivuus-desk/plateforme.sqlite"
    check("PLATEFORME_BASE_URL de desk.env atteint le processus npm (admin:utilisateur)",
          appel_utilisateur.get("env_base_url"), valeur_attendue)
    check("PLATEFORME_BASE_URL de desk.env atteint le processus npm (admin:agent)",
          appel_agent.get("env_base_url"), valeur_attendue)

    # --- AGENT_VM / AGENT_SECRET ecrits dans desk.env, APRES le contenu deja
    # la ------------------------------------------------------------------
    env_final = lire_env(root / "etc" / "nivuus" / "desk.env")
    check("AGENT_VM est le vm_id rendu par admin:agent (jamais le nom passe)",
          env_final.get("AGENT_VM"), "vm-test-uuid")
    check("AGENT_SECRET est le secret rendu par admin:agent",
          env_final.get("AGENT_SECRET"), "secret-de-test-0123456789abcdef")
    check("le contenu deja la (pose par install) est toujours present",
          env_final.get("PLATEFORME_SECRET_JETON"), "x" * 64)
    check("desk.env reste en mode 600 (il porte des secrets)",
          oct(os.stat(root / "etc" / "nivuus" / "desk.env").st_mode & 0o777), oct(0o600))

    # --- Idempotence : un second appel ne rejoue NI le compte NI
    # l'enrolement (AGENT_VM/AGENT_SECRET deja presents) ---------------------
    # 🔴 vm.nom n'a AUCUNE contrainte d'unicite : sans la garde d'idempotence,
    # ce second appel creerait une VM ORPHELINE de plus au lieu de refuser.
    # C'est le controle qui la rend visible : zero nouvelle commande npm.
    log_npm.unlink()
    r2 = appeler(root, bin_dir)
    check("un second passage reussit aussi (idempotent)", r2.returncode, 0)
    check("le lien reste un lien apres un second passage", os.path.islink(lien), True)
    check("le second passage ne relance AUCUNE commande npm (deja active)",
          log_npm.exists(), False)
    env_apres_second_passage = lire_env(root / "etc" / "nivuus" / "desk.env")
    check("AGENT_VM est inchange apres le second passage",
          env_apres_second_passage.get("AGENT_VM"), "vm-test-uuid")
    check("AGENT_SECRET est inchange apres le second passage",
          env_apres_second_passage.get("AGENT_SECRET"), "secret-de-test-0123456789abcdef")


# === Scénario 2 : desk.env n'existe pas encore (install n'a pas ete
# complete) — refus propre, jamais une trace Python =========================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    unite_dir = root / "etc" / "systemd" / "system"
    unite_dir.mkdir(parents=True)
    (unite_dir / "desk-plateforme.service").write_text(
        (RACINE / "hooks" / "assets" / "desk-plateforme.service").read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    # Pas de plateforme/, pas de desk.env : install n'a jamais tourne sur
    # cette racine.
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("racine non installee : le hook refuse proprement (rc != 0)",
          r.returncode != 0, True)
    check("le refus nomme le repertoire plateforme absent",
          "plateforme" in (r.stderr or ""), True)
    check("aucune trace Python (Traceback) n'atteint l'operateur",
          "Traceback" in (r.stderr or ""), False)
    # L'armement, lui, a quand meme du se faire : il precede la creation du
    # compte dans l'ordre du hook.
    lien = unite_dir / "multi-user.target.wants" / "desk-plateforme.service"
    check("l'armement a quand meme eu lieu avant le refus", os.path.islink(lien), True)


# === Scénario 2bis (CORRECTION, RONDE 1) : plateforme/ posee, desk.env
# ABSENT — le cas precis que la revue a nomme : install partielle, ou un
# operateur qui a supprime le fichier. Le hook doit REFUSER plutot que
# recreer desk.env en silence — c'est ce refus qui empeche
# ajouter_variables_env() d'ecrire un fichier NEUF (fenetre ecriture-puis-
# chmod), puisqu'il ne l'atteint jamais dans ce cas. ========================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    unite_dir = root / "etc" / "systemd" / "system"
    unite_dir.mkdir(parents=True)
    (unite_dir / "desk-plateforme.service").write_text(
        (RACINE / "hooks" / "assets" / "desk-plateforme.service").read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    # plateforme/ EST posee (install partiellement joue) ...
    (root / "opt" / "nivuus" / "desk" / "plateforme").mkdir(parents=True)
    # ... mais etc/nivuus/desk.env n'existe PAS.
    env_absent = root / "etc" / "nivuus" / "desk.env"
    check("precondition du scenario : desk.env n'existe pas", env_absent.exists(), False)

    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("desk.env absent (plateforme present) : refus propre (rc != 0)",
          r.returncode != 0, True)
    check("le refus nomme desk.env, pas seulement plateforme",
          "desk.env" in (r.stderr or ""), True)
    check("aucune trace Python", "Traceback" in (r.stderr or ""), False)
    check("desk.env n'a PAS ete cree a sa place (aucune ecriture en silence)",
          env_absent.exists(), False)
    check("aucune commande npm n'a ete lancee (le refus precede tout appel)",
          (root / "npm.log").exists(), False)
    # L'armement, lui, precede ce refus : il a quand meme du se faire.
    lien = unite_dir / "multi-user.target.wants" / "desk-plateforme.service"
    check("l'armement a quand meme eu lieu avant ce refus aussi",
          os.path.islink(lien), True)


# === Scénario 3 : reponses manquantes — refus propre =======================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir, answers={"auth_mode": "motdepasse"})
    check("sans admin_email/admin_password : refus propre (rc != 0)",
          r.returncode != 0, True)
    check("aucune trace Python", "Traceback" in (r.stderr or ""), False)


# === Scénario 4 : entree stdin malformee — refus propre, jamais une trace ==
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    env = dict(os.environ)
    env["PATH"] = str(bin_dir) + os.pathsep + env.get("PATH", "")
    r = subprocess.run([sys.executable, str(HOOK), "--phase", "activate",
                         "--root", str(root)],
                        input="ceci n'est pas du JSON", env=env,
                        capture_output=True, text=True)
    check("stdin illisible : refus propre (rc != 0)", r.returncode != 0, True)
    check("aucune trace Python sur une entree malformee",
          "Traceback" in (r.stderr or ""), False)


# === La ROUGE — Step 5 : un lien vers une unite absente doit LEVER =========
# Racine SANS l'unite systemd (install n'a pas ete jusqu'au bout, ou un
# disque corrompu) : armer_unite() doit lever plutot que poser un lien mort.

# --- Bras 1 : la fonction elle-meme, appelee directement -------------------
import importlib.util  # noqa: E402

# TÂCHE 6 : `from vm import ...` exige vm.py sur sys.path (sinon ModuleNotFoundError).
sys.path.insert(0, str(HOOK.parent))

spec = importlib.util.spec_from_file_location("desk_activate", HOOK)
activate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(activate)

with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    (root / "etc" / "systemd" / "system").mkdir(parents=True)
    # L'unite N'EST PAS posee : c'est le cas que la tache doit rougir.
    a_leve = False
    try:
        activate.armer_unite(root, "desk-plateforme.service")
    except FileNotFoundError:
        a_leve = True
    check("armer_unite() leve sur une unite absente (jamais un lien mort)",
          a_leve, True)
    lien_mort = (root / "etc" / "systemd" / "system" / "multi-user.target.wants"
                 / "desk-plateforme.service")
    check("aucun lien mort n'a ete cree", os.path.lexists(lien_mort), False)

# --- Bras 2 : le hook complet, en sous-processus, meme scenario ------------
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    (root / "etc" / "systemd" / "system").mkdir(parents=True)
    (root / "opt" / "nivuus" / "desk" / "plateforme").mkdir(parents=True)
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("ROUGE : le hook complet echoue (rc != 0) sur unite absente",
          r.returncode != 0, True)
    check("ROUGE : le refus nomme l'unite absente",
          "desk-plateforme.service" in (r.stderr or ""), True)
    lien_mort = (root / "etc" / "systemd" / "system" / "multi-user.target.wants"
                 / "desk-plateforme.service")
    check("ROUGE : aucun lien mort n'est laisse derriere", os.path.lexists(lien_mort), False)
    # Preuve que ce chemin d'echec precede TOUT le reste : aucune commande
    # npm n'a jamais ete lancee.
    check("ROUGE : aucune commande npm n'a ete lancee",
          (root / "npm.log").exists(), False)


# === Tâche 6 : câblage de hooks/vm.py — B éprouve poser_projfs, C éprouve
# poser_vb_audio ; leur comportement propre est dans test_desk_vm.py.
def _racine_pour_tache6(tmp):
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")
    return root, bin_dir
def _check_refus_propre(prefix, r, doit_contenir, root):
    """rc != 0, raison nommee, aucune trace Python, aucun npm (le refus precede tout)."""
    check(f"{prefix} : refus propre (rc != 0)", r.returncode != 0, True)
    check(f"{prefix} : le refus nomme {doit_contenir!r}", doit_contenir in (r.stderr or ""), True)
    check(f"{prefix} : aucune trace Python", "Traceback" in (r.stderr or ""), False)
    check(f"{prefix} : aucun npm lance", (root / "npm.log").exists(), False)

# --- B : `console` absent (aucun winrm_exec.py) — refus propre ------------
with tempfile.TemporaryDirectory() as tmp:
    root, bin_dir = _racine_pour_tache6(tmp)
    r = appeler(root, bin_dir, packages_dir=False)
    chemin_attendu = str(root / "console-absent-ici" / "console" / "guest" / "winrm_exec.py")
    _check_refus_propre("console absent", r, chemin_attendu, root)
    lien = root / "etc" / "systemd" / "system" / "multi-user.target.wants" / "desk-plateforme.service"
    check("console absent : l'armement a quand meme eu lieu", os.path.islink(lien), True)

# --- C : VB-Audio ARMÉ (vb_audio=true), aucun payload — refus propre ------
with tempfile.TemporaryDirectory() as tmp:
    root, bin_dir = _racine_pour_tache6(tmp)
    packages_dir, log_winrm = root / "faux-packages-dir", root / "winrm.log"
    poser_faux_winrm_exec(packages_dir, log_winrm)  # ProjFS reussit
    r = appeler(root, bin_dir, answers=dict(REPONSES, vb_audio=True), packages_dir=packages_dir)
    _check_refus_propre("vb_audio arme sans payload", r, "VB-Audio", root)
    check("ProjFS pose avant le refus vb_audio (leve avant winrm_exec.py)",
          len(lire_commandes(log_winrm)), 1)


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook activate passés")
