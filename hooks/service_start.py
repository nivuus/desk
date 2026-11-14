#!/usr/bin/env python3
"""Starting the armed unit on the real machine, for `hooks/activate.py`.

Moved out of `hooks/activate.py` without any behaviour change: the start
of the unit is its own concern, and the activate hook was at 475 lines out
of 500. Imported like `vm.py` and `administration.py` — Python adds the
directory of the LAUNCHED script to `sys.path`, no manipulation needed.
"""
import subprocess


def start_now(unite: str) -> list:
    """`daemon-reload` then `start <unite>`. Never raises.

    `systemctl` is legitimately unusable in a constrained
    environment (see the top docstring of `hooks/activate.py`), and the
    link created by
    `armer_unite` guarantees anyway that the next real boot
    is correct — so each failure is only REPORTED, never fatal.
    Only called by `activate.py::main()` when `--root` is `/`: see its
    guard, right before the call.
    """
    echecs = []
    commandes = [["systemctl", "daemon-reload"], ["systemctl", "start", unite]]
    for commande in commandes:
        try:
            proc = subprocess.run(commande, capture_output=True, text=True)
        except OSError as exc:  # systemctl absent from this PATH
            echecs.append(f"{' '.join(commande)} : {exc}")
            continue
        if proc.returncode != 0:
            detail = (proc.stderr or proc.stdout or "").strip()[:200]
            echecs.append(f"{' '.join(commande)} : {detail or proc.returncode}")
    return echecs
