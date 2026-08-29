#!/usr/bin/env python3
"""Fixtures partagées par `tests/test_desk_install.py`.

Extrait de ce dernier le 30 août 2026, dans un commit DÉDIÉ et AVANT que la
revue finale de branche n'y ajoute ses scénarios (le pré-vol qui refuse avant
tout secret, le dépôt du runtime Node) : le fichier était à 465 lignes sur
500. Ce dépôt interdit de comprimer pour éviter une extraction — précédent
exact : `tests/desk_activate_fixtures.py`, extrait pour la même raison à la
tâche 6.

Ce module porte les CONSTANTES (les réponses du wizard, les facts) et les
AUXILIAIRES (l'appel du hook, la lecture de `desk.env` et d'une unité systemd,
la fabrique d'une racine source minimale) ; `test_desk_install.py` porte les
SCÉNARIOS et les assertions. Aucune fixture d'ici n'a de valeur seule.

⚠️ Ce fichier ne s'appelle PAS `test_*.py` : il n'est pas une suite, et le
`Makefile` ne le découvre donc pas — même convention, et même raison, que
`desk_activate_fixtures.py`.
"""
import configparser
import json
import os
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "install.py"


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
# 🔴 `hote` et `proxy_confiance` sont VOLONTAIREMENT DIFFÉRENTES de
# `turn_ecoute` ci-dessous : c'est ce qui rend le test capable de détecter
# une régression vers le bug réel corrigé au lot 10A (29 août 2026) — avant
# ce correctif, `install.py` posait `PLATEFORME_HOTE = turn_ecoute`, ce qui
# aurait fait écouter le service sur l'adresse dérivée pour TURN (publique,
# sur la machine réelle) plutôt que sur une adresse interne.
# 🔴 `vm_repond` A DISPARU DU CONTRAT (revue finale de branche, 30 août
# 2026) : ce n'était pas un fait mesuré mais un littéral (`True` en dur
# dans `hooks/resolve.py`), à une phase qui ne peut rien savoir de la VM.
# Voir `tests/test_desk_contrat_hw.py` (garde ⑤).
FACTS = {"node_version": "24.9.0",
         "turn_ecoute": "203.0.113.9", "turn_relais": "203.0.113.9",
         "hote": "198.51.100.1", "proxy_confiance": "198.51.100.1",
         "port": 9999}


def poser_faux_node_source(racine: pathlib.Path) -> pathlib.Path:
    """Un préfixe Node FACTICE : `bin/node`, `bin/npm` (un VRAI lien
    relatif, comme le vrai arbre), et `lib/node_modules/npm/`.

    ⚠️ POURQUOI UN FACTICE PAR DÉFAUT, ET PAS LE VRAI : le runtime réel pèse
    144 Mio (mesuré le 29 août 2026 sur `/opt/nivuus/node`), et chaque
    scénario de cette suite le recopierait. Le scénario dédié
    « installation 7 » ci-dessous emploie, LUI, le VRAI runtime de cette
    machine — c'est lui qui éprouve le chemin de production, les autres n'ont
    pas à le repayer.
    """
    prefixe = racine / "faux-node"
    (prefixe / "bin").mkdir(parents=True, exist_ok=True)
    (prefixe / "bin" / "node").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    (prefixe / "bin" / "node").chmod(0o755)
    npm_cli = prefixe / "lib" / "node_modules" / "npm" / "bin"
    npm_cli.mkdir(parents=True, exist_ok=True)
    (npm_cli / "npm-cli.js").write_text("// factice\n", encoding="utf-8")
    for nom, cible in (("npm", "../lib/node_modules/npm/bin/npm-cli.js"),):
        lien = prefixe / "bin" / nom
        if not lien.is_symlink():
            lien.symlink_to(cible)
    return prefixe


def appeler(root, hw=None, answers=None, facts=None, env=None,
            node_source=True):
    """Appelle le hook comme le moteur : --phase/--root, stdin JSON.

    `node_source` (30 août 2026) : `True` pose `DESK_NODE_SOURCE` vers un
    préfixe Node FACTICE fabriqué sous `root` (voir
    `poser_faux_node_source`) ; `False` laisse le hook dériver le VRAI
    runtime de cette machine ; une chaîne la pose telle quelle (scénarios de
    refus)."""
    # `hw` par défaut VIDE : le moteur envoie `detect_all()` verbatim, et
    # `install.py` n'y lit rien — voir tests/test_desk_contrat_hw.py, qui
    # fige ce contrat. Il portait `{"vm_windows": True}`, une clé qu'aucun
    # producteur ne pose (Critique de la revue finale de branche).
    contexte = {"hw": hw if hw is not None else {},
                "answers": answers if answers is not None else REPONSES}
    if facts is not None:
        contexte["facts"] = facts
    if node_source is not False:
        env = dict(os.environ) if env is None else env
        env["DESK_NODE_SOURCE"] = (
            str(poser_faux_node_source(pathlib.Path(root)))
            if node_source is True else str(node_source))
    r = subprocess.run(
        [sys.executable, str(HOOK), "--phase", "install", "--root", str(root)],
        input=json.dumps(contexte), capture_output=True, text=True, env=env)
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


def poser_source_minimale(racine: pathlib.Path) -> None:
    """Fabrique une racine SOURCE minimale (`plateforme/`, `client/dist/`,
    `proto/ts/`) sous laquelle `DESK_SOURCE_RACINE` peut pointer — utilisée
    par les scénarios qui éprouvent `install` SANS emprunter le vrai dépôt.

    🔴 `proto/ts/plateforme.ts` DOIT EXISTER : depuis le lot 10A
    (29 août 2026, trouvaille réelle), `install.py` copie aussi `proto/ts/`
    et REFUSE si `plateforme.ts` n'y est pas retrouvé après coup (voir
    `hooks/install.py::main`) — sans ce fichier, ces scénarios refuseraient
    tous pour une raison qu'ils n'ont pas l'intention d'éprouver.
    """
    (racine / "plateforme").mkdir(parents=True, exist_ok=True)
    (racine / "plateforme" / "package.json").write_text("{}", encoding="utf-8")
    (racine / "client" / "dist").mkdir(parents=True, exist_ok=True)
    (racine / "client" / "dist" / "index.html").write_text(
        "<html></html>", encoding="utf-8")
    (racine / "proto" / "ts").mkdir(parents=True, exist_ok=True)
    (racine / "proto" / "ts" / "plateforme.ts").write_text(
        "export {};\n", encoding="utf-8")


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
