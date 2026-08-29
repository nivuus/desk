#!/usr/bin/env python3
"""Tests du hook install du package desk.

Le hook est éprouvé par son VRAIE interface — un sous-processus appelé
`--phase install --root <racine>`, nourri de {"hw":…, "answers":…,
"facts":…} sur stdin — exactement comme `installer/packages/runner.py`
l'invoque (`cmd = [sys.executable, hook, "--phase", phase]` puis
`cmd += ["--root", root]` si une racine est fournie), et exactement comme
`console/hooks/install.py` se teste déjà dans le dépôt voisin
(`installer/console/tests/test_console_install.py`) : ce précédent est la
source de la convention `--root`, préférée à une variable d'environnement ou
une clé de contexte parce que c'est ce que le moteur RÉEL envoie.

Chaque test pose sa PROPRE racine sous `tempfile.TemporaryDirectory()` :
jamais `/opt`, `/etc` ou `/etc/systemd/system` du poste qui fait tourner ces
tests.

Run: python3 tests/test_desk_install.py
"""
import configparser
import json
import os
import pathlib
import subprocess
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "install.py"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


REPONSES = {"admin_email": "a@b.c", "admin_password": "hunter2hunter2",
            "auth_mode": "motdepasse", "vb_audio": False}

# Les facts que la tâche 3 (resolve) mesure et que le moteur RENDRAIT à
# activate au premier démarrage — voir `installer/installer/install-engine/
# steps/packages.py::apply_packages` : `run_install` ne reçoit PAS ces facts
# (seul `run_activate` les reçoit, mergées dans `hw`). install.py ne peut
# donc pas en dépendre pour fonctionner dans le moteur réel ; il les accepte
# ici en fallback défensif (si un jour le contrat change, ou pour ce test),
# et DÉRIVE lui-même sinon — exactement ce qu'il fait dans les deux appels
# ci-dessous, l'un AVEC facts, l'autre SANS.
FACTS = {"vm_repond": True, "node_version": "24.9.0",
         "turn_ecoute": "203.0.113.9", "turn_relais": "203.0.113.9",
         "port": 9999}


def appeler(root, hw=None, answers=None, facts=None):
    """Appelle le hook comme le moteur : --phase/--root, stdin JSON."""
    contexte = {"hw": hw if hw is not None else {"vm_windows": True},
                "answers": answers if answers is not None else REPONSES}
    if facts is not None:
        contexte["facts"] = facts
    r = subprocess.run(
        [sys.executable, str(HOOK), "--phase", "install", "--root", str(root)],
        input=json.dumps(contexte), capture_output=True, text=True)
    return r


def lire_env(racine):
    """Parse `etc/nivuus/desk.env` (KEY=VALUE, une ligne par variable)."""
    chemin = pathlib.Path(racine) / "etc" / "nivuus" / "desk.env"
    valeurs = {}
    for ligne in chemin.read_text(encoding="utf-8").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle, _, valeur = ligne.partition("=")
        valeurs[cle] = valeur
    return valeurs, chemin


def load_unit(path):
    """Parse un fichier d'unité systemd (INI, clés sensibles à la casse).

    Même patron que `console/tests/test_console_install.py::load_unit` :
    `strict=False` (systemd tolère une clé répétée, la dernière gagne) et
    `optionxform=str` (systemd est sensible à la casse, configparser
    minusculise par défaut).
    """
    parser = configparser.ConfigParser(strict=False, interpolation=None)
    parser.optionxform = str
    parser.read(path, encoding="utf-8")
    return parser


# --- Installation 1 : AVEC facts (le cas où le moteur les fournirait) -----
with tempfile.TemporaryDirectory() as tmp1:
    root1 = pathlib.Path(tmp1)
    r1 = appeler(root1, facts=FACTS)
    check("installation 1 : code de sortie 0", r1.returncode, 0)

    env1, chemin_env1 = lire_env(root1)

    # 🔴 Le secret de jeton est TIRÉ AU SORT, jamais demandé ni constant.
    check("le secret fait au moins 32 caracteres",
          len(env1.get("PLATEFORME_SECRET_JETON", "")) >= 32, True)

    check("le fichier d'environnement est en 600",
          oct(os.stat(chemin_env1).st_mode & 0o777), oct(0o600))

    # PLATEFORME_PAGE est ce qui rend nginx facultatif : la plateforme sert
    # elle-même la page batie depuis le 22 aout 2026.
    check("la page est servie depuis client/dist",
          env1.get("PLATEFORME_PAGE", "").endswith("client/dist"), True)

    # 🔴 PLATEFORME_HOTE : en mode pomerium, les quatre ecoutes universelles
    # font REFUSER le demarrage. L'installation ne doit jamais en poser une.
    check("l'ecoute n'est jamais universelle",
          env1.get("PLATEFORME_HOTE") in ("0.0.0.0", "::", "[::]", "*"), False)
    check("l'ecoute est non vide", bool(env1.get("PLATEFORME_HOTE")), True)

    # Avec facts fournis : le port et l'adresse TURN viennent d'EUX, pas du
    # defaut ni d'une derivation independante.
    check("le port vient des facts quand ils sont fournis",
          env1.get("PLATEFORME_PORT"), str(FACTS["port"]))
    check("PLATEFORME_HOTE reprend l'adresse TURN des facts",
          env1.get("PLATEFORME_HOTE"), FACTS["turn_ecoute"])

    # PLATEFORME_AUTH vient de la reponse auth_mode, telle quelle.
    check("PLATEFORME_AUTH vient de la reponse auth_mode",
          env1.get("PLATEFORME_AUTH"), "motdepasse")

    # La configuration coturn (TURN_URL/TURN_SECRET) que `plateforme/src/
    # signaling/ice.ts::configurationIce` exige TOUTES LES DEUX pour annoncer
    # un relais.
    check("TURN_URL porte l'adresse ecoute des facts",
          env1.get("TURN_URL"), f"turn:{FACTS['turn_ecoute']}:3478")
    check("TURN_SECRET est pose et non vide",
          bool(env1.get("TURN_SECRET")), True)

    # --- Ce qui vit sous /opt/nivuus/desk/ --------------------------------
    plateforme_dep = root1 / "opt" / "nivuus" / "desk" / "plateforme"
    check("plateforme/package.json est copie",
          (plateforme_dep / "package.json").is_file(), True)
    check("plateforme/src est copie",
          (plateforme_dep / "src").is_dir(), True)
    check("les donnees de DEV ne sont PAS copiees (icones/televersements)",
          (plateforme_dep / "donnees").exists(), False)

    client_dep = root1 / "opt" / "nivuus" / "desk" / "client" / "dist"
    check("client/dist/index.html est copie",
          (client_dep / "index.html").is_file(), True)

    # --- L'unite systemd ----------------------------------------------------
    unite = root1 / "etc" / "systemd" / "system" / "desk-plateforme.service"
    check("l'unite est deposee", unite.is_file(), True)
    ini = load_unit(unite)
    check("l'unite lance npm start",
          ini.get("Service", "ExecStart", fallback=""), "/usr/bin/npm start")
    check("l'unite pointe sur /opt/nivuus/desk/plateforme",
          ini.get("Service", "WorkingDirectory", fallback=""),
          "/opt/nivuus/desk/plateforme")
    check("l'unite lit /etc/nivuus/desk.env",
          ini.get("Service", "EnvironmentFile", fallback=""),
          "/etc/nivuus/desk.env")
    check("l'unite redemarre sur echec",
          ini.get("Service", "Restart", fallback=""), "on-failure")
    check("l'unite arme un utilisateur dedie (DynamicUser)",
          ini.get("Service", "DynamicUser", fallback=""), "yes")
    # ⚠️ Ce champ n'est PAS armé (`systemctl enable`) ici : c'est le travail
    # de la tâche 5 (`activate`), par un LIEN — jamais un `systemctl enable`
    # qui échoue en silence. install ne fait que POSER l'unité.

    # --- La configuration coturn --------------------------------------------
    turnconf = root1 / "etc" / "turnserver.conf"
    check("turnserver.conf est depose", turnconf.is_file(), True)
    contenu_turn = turnconf.read_text(encoding="utf-8")
    check("turnserver.conf porte l'adresse d'ecoute",
          f"listening-ip={FACTS['turn_ecoute']}" in contenu_turn, True)
    check("turnserver.conf porte l'adresse de relais",
          f"relay-ip={FACTS['turn_relais']}" in contenu_turn, True)
    check("turnserver.conf porte le MEME secret que TURN_SECRET",
          f"static-auth-secret={env1['TURN_SECRET']}" in contenu_turn, True)
    check("turnserver.conf est en 600 (il porte un secret)",
          oct(os.stat(turnconf).st_mode & 0o777), oct(0o600))

# --- Installation 2, SANS facts : la derivation independante ---------------
with tempfile.TemporaryDirectory() as tmp2:
    root2 = pathlib.Path(tmp2)
    r2 = appeler(root2, facts=None)
    check("installation 2 (sans facts) : code de sortie 0", r2.returncode, 0)
    env2, chemin_env2 = lire_env(root2)

    # 🔴 Le port par defaut est 3445 : /etc/pomerium/config.yaml route
    # https://app.allanic.me vers 3445, et rien d'autre ne fait marcher la
    # route deja en place (voir hooks/resolve.py::PORT_DEFAUT, la meme
    # raison, dupliquee a dessein — voir le commentaire d'install.py).
    check("sans facts : le port retombe sur le defaut 3445",
          env2.get("PLATEFORME_PORT"), "3445")
    check("sans facts : PLATEFORME_HOTE est quand meme derive, jamais vide",
          bool(env2.get("PLATEFORME_HOTE")), True)
    check("sans facts : l'ecoute n'est toujours pas universelle",
          env2.get("PLATEFORME_HOTE") in ("0.0.0.0", "::", "[::]", "*"), False)

    # 🔴 Deux installations distinctes NE PARTAGENT PAS le secret.
    check("deux installations ne partagent pas PLATEFORME_SECRET_JETON",
          env1["PLATEFORME_SECRET_JETON"] == env2["PLATEFORME_SECRET_JETON"],
          False)
    check("deux installations ne partagent pas TURN_SECRET",
          env1["TURN_SECRET"] == env2["TURN_SECRET"], False)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook install passés")
