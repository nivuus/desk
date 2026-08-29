#!/usr/bin/env python3
"""Copie d'arbres de fichiers vers la cible d'installation, et les
permissions dont `DynamicUser=yes` a besoin — extrait de `hooks/install.py`
au lot 10A (29 août 2026, réinstallation réelle sur `--root /`), qui
frôlait le plafond de 500 lignes après l'ajout du dépôt de `proto/ts/`
(trouvaille réelle : le service ne démarre pas sans lui, voir
`hooks/install.py`).

Comme `hooks/vm.py`, `hooks/administration.py` et `hooks/env_fichier.py`,
ce module N'EST PAS un hook exécutable seul (pas de `--phase`/stdin JSON) :
`hooks/install.py` l'importe (`from depot_arbre import copier_arbre,
rendre_lisible_par_tous`) — Python ajoute automatiquement le répertoire du
script LANCÉ (`hooks/`) à `sys.path`, donc cet import résout sans
manipulation supplémentaire.

Les DEUX fonctions ci-dessous portent des bugs RÉELS trouvés en lançant le
VRAI service sur cette machine, jamais vus par la suite de tests avant ce
lot — voir leurs docstrings respectifs pour le détail complet.
"""
import os
import pathlib
import shutil
import stat


def rendre_lisible_par_tous(racine: pathlib.Path) -> None:
    """Ajoute `o+r` partout sous `racine`, et `o+x` sur les répertoires et
    sur les fichiers déjà exécutables pour le propriétaire ou le groupe —
    l'équivalent de `chmod -R a+rX racine`.

    🔴 BUG RÉEL TROUVÉ ET CORRIGÉ AU LOT 10A (29 août 2026), EN LANÇANT LE
    SERVICE POUR DE VRAI (jamais vu par la suite de tests, qui ne monte
    jamais un VRAI service systemd `DynamicUser=yes`). `DynamicUser=yes`
    fait créer par systemd un UID/GID ÉPHÉMÈRE à chaque démarrage — un UID
    qui n'appartient à AUCUN groupe partagé avec les fichiers déposés par
    ce hook (hérités du propriétaire ET DE L'UMASK du processus qui exécute
    `install`, `root` la plupart du temps). Mesuré : sous l'umask `027` du
    `root` de cette machine, `destination.parent.mkdir(parents=True)` (dans
    `copier_arbre` ci-dessous) a posé `/opt/nivuus/desk` et `/opt/nivuus/
    desk/plateforme` en `drwxr-x---` — INACCESSIBLES à l'UID dynamique, qui
    n'a ni le propriétaire ni le groupe. Symptôme : le service échoue au
    tout premier geste, avant même d'exécuter une ligne de JavaScript —
    `Changing to the requested working directory failed: Permission
    denied`, code systemd `200/CHDIR`. `ProtectSystem=strict` (impliqué par
    `DynamicUser=yes`) rend l'arbre en LECTURE SEULE, ce qui est une
    contrainte DIFFÉRENTE (un montage) de la LISIBILITÉ (des bits POSIX) —
    les deux doivent être satisfaites, et seule la seconde manquait ici.
    """
    for dirpath, _dirnames, filenames in os.walk(racine):
        chemin_dir = pathlib.Path(dirpath)
        mode_dir = chemin_dir.stat().st_mode
        os.chmod(chemin_dir, mode_dir | stat.S_IROTH | stat.S_IXOTH)
        for nom in filenames:
            chemin_fichier = chemin_dir / nom
            if chemin_fichier.is_symlink():
                # chmod (et cette fonction) suivent les symlinks : leur
                # CIBLE est déjà traitée quand `os.walk` l'atteint dans
                # l'arbre (les cibles relatives de node_modules/.bin/*
                # pointent toutes À L'INTÉRIEUR de l'arbre parcouru).
                continue
            mode_fichier = chemin_fichier.stat().st_mode
            nouveau = mode_fichier | stat.S_IROTH
            if mode_fichier & (stat.S_IXUSR | stat.S_IXGRP):
                nouveau |= stat.S_IXOTH
            os.chmod(chemin_fichier, nouveau)


def copier_arbre(source: pathlib.Path, destination: pathlib.Path,
                  exclure: tuple = ()) -> None:
    """Copie `source` sous `destination`, en écartant les noms d'`exclure`.

    🔴 BUG RÉEL TROUVÉ ET CORRIGÉ AU LOT 10A (29 août 2026), en réinstallant
    POUR DE VRAI sur `--root /` — jamais vu par la suite de tests, qui ne
    rejoue jamais `install` DEUX FOIS sur la MÊME racine. Ce commentaire
    affirmait « `dirs_exist_ok=True` : une installation rejouée sur une
    racine déjà posée ne doit pas lever sur un répertoire déjà présent » —
    VRAI pour des fichiers ordinaires, FAUX pour des SYMLINKS : `plateforme/
    node_modules/.bin/*` en porte une douzaine (`tsx`, `vite`, `tsc`, …), et
    `shutil.copytree(..., symlinks=True, dirs_exist_ok=True)` lève
    `shutil.Error` en tentant de recréer un lien qui existe déjà à la
    destination (`dirs_exist_ok` couvre les RÉPERTOIRES, jamais les fichiers
    ou liens qu'ils contiennent). Mesuré : une réinstallation sur ce dépôt,
    après un premier `install` déjà réussi, a levé sur exactement ces douze
    liens.

    Le remède retenu : la destination reflète TOUJOURS l'arbre SOURCE
    actuel, jamais une fusion avec un dépôt précédent — on efface d'abord
    ce qui existait, puis on copie à neuf. Sans risque pour l'état du
    service : `/opt/nivuus/desk/plateforme` et `/opt/nivuus/desk/client/
    dist` ne portent AUCUN état (la base SQLite, les icônes et les
    téléversements vivent sous `/var/lib/nivuus-desk`, hors de cette
    fonction — voir `hooks/assets/desk-plateforme.service::StateDirectory`).
    """
    if destination.exists() or destination.is_symlink():
        shutil.rmtree(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, destination, symlinks=True,
                     ignore=shutil.ignore_patterns(*exclure) if exclure else None)
