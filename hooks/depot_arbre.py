#!/usr/bin/env python3
"""Copie d'arbres de fichiers vers la cible d'installation, et les
permissions dont `DynamicUser=yes` a besoin — extrait de `hooks/install.py`
au lot 10A (29 août 2026, réinstallation réelle sur `--root /`), qui
frôlait le plafond de 500 lignes après l'ajout du dépôt de `proto/ts/`
(trouvaille réelle : le service ne démarre pas sans lui, voir
`hooks/install.py`).

Comme `hooks/vm.py`, `hooks/administration.py` et `hooks/env_file.py`,
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


def make_world_readable(racine: pathlib.Path) -> None:
    """Adds `o+r` everywhere under `racine`, and `o+x` on directories and
    on files already executable for the owner or the group —
    the equivalent of `chmod -R a+rX racine`.

    🔴 REAL BUG FOUND AND FIXED IN BATCH 10A (29 August 2026), BY STARTING THE
    SERVICE FOR REAL (never seen by the test suite, which never mounts
    a REAL `DynamicUser=yes` systemd service). `DynamicUser=yes`
    makes systemd create an EPHEMERAL UID/GID at every start — a UID
    that belongs to NO group shared with the files dropped by
    this hook (inherited from the owner AND THE UMASK of the process running
    `install`, `root` most of the time). Measured: under the `027` umask of
    this machine's `root`, `destination.parent.mkdir(parents=True)` (in
    `copier_arbre` below) created `/opt/nivuus/desk` and `/opt/nivuus/
    desk/plateforme` as `drwxr-x---` — UNREACHABLE for the dynamic UID, which
    has neither the owner nor the group. Symptom: the service fails at its
    very first step, before even running a line of JavaScript —
    `Changing to the requested working directory failed: Permission
    denied`, systemd code `200/CHDIR`. `ProtectSystem=strict` (implied by
    `DynamicUser=yes`) makes the tree READ ONLY, which is a
    DIFFERENT constraint (a mount) from READABILITY (POSIX bits) —
    both must be satisfied, and only the second one was missing here.
    """
    for dirpath, _dirnames, filenames in os.walk(racine):
        chemin_dir = pathlib.Path(dirpath)
        mode_dir = chemin_dir.stat().st_mode
        os.chmod(chemin_dir, mode_dir | stat.S_IROTH | stat.S_IXOTH)
        for nom in filenames:
            file_path = chemin_dir / nom
            if file_path.is_symlink():
                # chmod (and this function) follow symlinks: their
                # TARGET is already handled when `os.walk` reaches it in
                # the tree (the relative targets of node_modules/.bin/*
                # all point INSIDE the walked tree).
                continue
            file_mode = file_path.stat().st_mode
            new = file_mode | stat.S_IROTH
            if file_mode & (stat.S_IXUSR | stat.S_IXGRP):
                new |= stat.S_IXOTH
            os.chmod(file_path, new)


def copier_arbre(source: pathlib.Path, destination: pathlib.Path,
                  exclure: tuple = ()) -> None:
    """Copies `source` under `destination`, leaving out the names in `exclure`.

    🔴 REAL BUG FOUND AND FIXED IN BATCH 10A (29 August 2026), by reinstalling
    FOR REAL on `--root /` — never seen by the test suite, which never
    replays `install` TWICE on the SAME root. This comment
    claimed "`dirs_exist_ok=True`: an installation replayed on an
    already laid out root must not raise on an already present directory" —
    TRUE for ordinary files, FALSE for SYMLINKS: `plateforme/
    node_modules/.bin/*` holds a dozen of them (`tsx`, `vite`, `tsc`, …), and
    `shutil.copytree(..., symlinks=True, dirs_exist_ok=True)` raises
    `shutil.Error` when trying to recreate a link that already exists at the
    destination (`dirs_exist_ok` covers DIRECTORIES, never the files
    or links they contain). Measured: a reinstall on this repository,
    after a first `install` that already succeeded, raised on exactly those twelve
    links.

    The chosen remedy: the destination ALWAYS mirrors the current SOURCE
    tree, never a merge with a previous deployment — we first erase
    what existed, then copy afresh. No risk for the state of the
    service: `/opt/nivuus/desk/plateforme` and `/opt/nivuus/desk/client/
    dist` carry NO state (the SQLite database, the icons and the
    uploads live under `/var/lib/nivuus-desk`, outside this
    function — see `hooks/assets/desk-plateforme.service::StateDirectory`).
    """
    if destination.exists() or destination.is_symlink():
        shutil.rmtree(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, destination, symlinks=True,
                     ignore=shutil.ignore_patterns(*exclure) if exclure else None)
