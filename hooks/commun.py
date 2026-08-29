"""Module partagé entre les hooks du package desk (`resolve.py`, `install.py`).

Extrait le 29 août 2026 (ronde de correction 1 sur la tâche 4) : la revue a
relevé `interface_de_route_par_defaut()` et `adresse_ipv4_de()` dupliquées
OCTET POUR OCTET entre les deux hooks, et `PORT_DEFAUT` dupliqué avec sa
raison en double.

⚠️ CE N'EST PAS UNE DÉPENDANCE HORS DU PACKAGE — l'argument « chaque hook
doit rester exécutable seul » (protocole décrit dans `resolve.py`) porte sur
l'absence de dépendance vers un AUTRE dépôt (le moteur, `installer/`), pas
sur l'absence de module frère À L'INTÉRIEUR du package : `console` (dépôt
voisin `installer/console/`) fait exactement cela pour ses propres hooks
(`hooks/install.py` importe `retro.py` et `guest_steps.py` depuis la racine
du package, via `HERE = os.path.dirname(...)` + `sys.path.insert(0, HERE)`).

Ici, aucun `sys.path.insert` n'est nécessaire : `commun.py` vit dans le même
répertoire (`hooks/`) que `resolve.py` et `install.py`, et Python ajoute déjà
automatiquement le répertoire du script exécuté en tête de `sys.path` — c'est
la même raison pour laquelle `resolve.py` importe déjà `guest_steps` sans
bricolage dans le package voisin. Un simple `import commun` suffit depuis
les deux hooks.
"""
import re
import subprocess

# 🔴 LE PORT PAR DÉFAUT EST 3445, ET IL EST DÉRIVÉ, JAMAIS DEMANDÉ.
#
# /etc/pomerium/config.yaml porte une route `from: https://app.allanic.me`
# vers `to: http://127.0.0.1:3445` (relevé le 29 août 2026) : c'est le seul
# port qui fait marcher la route publique déjà en place. Ce n'est pas une
# cinquième question du wizard : l'opérateur n'a aucune information qui lui
# permettrait d'y répondre différemment sans casser la route Pomerium déjà
# en place. Cette raison ne vit désormais qu'ICI — ni `resolve.py` ni
# `install.py` ne la répètent, ils importent la valeur.
PORT_DEFAUT = 3445


def interface_de_route_par_defaut():
    """Le périphérique réseau de la route IPv4 par défaut, ou None."""
    try:
        r = subprocess.run(["ip", "-4", "route", "show", "default"],
                            capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    for ligne in r.stdout.splitlines():
        correspond = re.search(r"\bdev\s+(\S+)", ligne)
        if correspond:
            return correspond.group(1)
    return None


def adresse_ipv4_de(interface: str):
    """La première adresse IPv4 portée par `interface`, ou None."""
    try:
        r = subprocess.run(["ip", "-4", "-o", "addr", "show", "dev", interface],
                            capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    correspond = re.search(r"inet\s+(\d+\.\d+\.\d+\.\d+)", r.stdout)
    return correspond.group(1) if correspond else None
