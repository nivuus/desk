#!/usr/bin/env python3
"""Restarting the armed unit on the real machine, then proving it answers.

Moved out of `hooks/activate.py` (it was at 475 lines out of 500): the
start of the unit is its own concern. Imported like `vm.py` and
`administration.py` — Python adds the directory of the LAUNCHED script to
`sys.path`, no manipulation needed.

🔴 RESTART, NEVER START. On a package update (`nivuus update`), `install`
replaces the installed tree, `desk.env` and the unit UNDER the running
process; `systemctl start` on an already active unit does nothing, so the
old code kept running on a deleted tree until someone restarted it by hand.
`restart` starts an inactive unit and replaces an active one. ⚠️ Replacing
the running code interrupts the sessions in progress: that is inherent in
an update, not a side effect of this choice.

🔴 A STARTED UNIT IS NOT A SERVICE THAT ANSWERS. The unit is `Type=simple`,
so `systemctl restart` returns as soon as the process is forked; a process
that dies a second later, or never opens its port, used to leave the hook
on "service armed". The hook now waits for `GET /sante`
(`plateforme/src/http/routes-sante.ts`: 200 `{"etat":"ok"}` when the
database answers, 503 otherwise) on the address and port `desk.env` gives
the service (`PLATEFORME_HOTE`, `PLATEFORME_PORT`), with a bounded deadline.
The default deadline is systemd's own `DefaultTimeoutStartSec` (90 s): the
time systemd itself grants a unit to start.

The non-fatal path is kept for ONE case only: systemd is not the running
init (a container, a chroot), detected as `sd_booted(3)` documents — the
directory `/run/systemd/system/` exists. There `systemctl` is legitimately
unusable, nothing is run, and the link `armer_unite` created starts the unit
at the next real boot. Where the installer runs `activate` — the first-boot
unit `nivuus-package-activate@.service` and `nivuus update` on the
installed host — systemd IS running, so a failed restart or a service that
never answers is fatal there.
"""
import pathlib
import subprocess
import time
import urllib.error
import urllib.request

SYSTEMD_RUNTIME_DIR = pathlib.Path("/run/systemd/system")
DELAI_SANTE_S = 90.0
INTERVALLE_SANTE_S = 1.0
CHEMIN_SANTE = "/sante"

# Never through a proxy: the health target is the service on this host, and
# an `http_proxy` inherited from the environment would probe something else.
_OUVREUR = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def systemd_booted(runtime_dir: pathlib.Path = SYSTEMD_RUNTIME_DIR) -> bool:
    """`sd_booted(3)`: systemd is the running init iff its runtime dir exists."""
    return pathlib.Path(runtime_dir).is_dir()


def restart_now(unite: str, runner=subprocess.run) -> str | None:
    """`daemon-reload` then `restart <unite>`; the first failure, or None."""
    for commande in (["systemctl", "daemon-reload"], ["systemctl", "restart", unite]):
        try:
            proc = runner(commande, capture_output=True, text=True)
        except OSError as exc:  # systemctl absent from this PATH
            return f"{' '.join(commande)}: {exc}"
        if proc.returncode != 0:
            detail = (proc.stderr or proc.stdout or "").strip()[:200]
            return f"{' '.join(commande)}: {detail or f'exit code {proc.returncode}'}"
    return None


def health_url(env: dict) -> str:
    """The `/sante` URL of the service, from its `desk.env` values."""
    hote = str(env.get("PLATEFORME_HOTE") or "").strip()
    port = str(env.get("PLATEFORME_PORT") or "").strip()
    if not hote or not port:
        raise ValueError("desk.env carries no PLATEFORME_HOTE/PLATEFORME_PORT: "
                         "the address the service listens on is unknown")
    if ":" in hote:
        hote = f"[{hote}]"
    return f"http://{hote}:{port}{CHEMIN_SANTE}"


def wait_for_health(url: str, delai_s: float,
                    intervalle_s: float = INTERVALLE_SANTE_S) -> str | None:
    """Polls `url` until it answers 200 or `delai_s` elapses.

    Returns None once the service answered 200, otherwise what was last
    observed — a refused connection and a 503 are different diagnoses.
    """
    echeance = time.monotonic() + delai_s
    dernier = "no answer yet"
    while True:
        restant = echeance - time.monotonic()
        if restant <= 0:
            return dernier
        try:
            with _OUVREUR.open(url, timeout=max(0.1, min(restant, 5.0))) as reponse:
                if reponse.status == 200:
                    return None
                dernier = f"HTTP {reponse.status}"
        except urllib.error.HTTPError as exc:
            dernier = f"HTTP {exc.code}"
        except (urllib.error.URLError, OSError) as exc:
            dernier = f"{getattr(exc, 'reason', exc)}"
        time.sleep(min(intervalle_s, max(0.0, echeance - time.monotonic())))


def bring_up(unite: str, env: dict, *, runtime_dir: pathlib.Path = SYSTEMD_RUNTIME_DIR,
             runner=subprocess.run, delai_s: float = DELAI_SANTE_S) -> tuple:
    """Restarts `unite` and waits for its health route. `(ok, message)`.

    `ok` False is FATAL for the caller. Only called by `activate.py::main()`
    when `--root` is `/`: see its guard, right before the call.
    """
    if not systemd_booted(runtime_dir):
        return True, (f"{unite} linked, not started: systemd is not the running "
                      f"init ({runtime_dir} is absent); it starts at the next boot")
    try:
        url = health_url(env)
    except ValueError as exc:
        return False, f"{unite} not restarted: {exc}"
    echec = restart_now(unite, runner)
    if echec:
        return False, f"{unite} could not be restarted: {echec}"
    observe = wait_for_health(url, delai_s)
    if observe is not None:
        return False, (f"{unite} restarted but {url} did not answer 200 within "
                       f"{delai_s:g} s (last: {observe}); see journalctl -u {unite}")
    return True, f"{unite} restarted and {url} answers"
