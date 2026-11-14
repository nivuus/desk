#!/usr/bin/env python3
"""Tests of `hooks/service_start.py`: restarting the unit, then proving it answers.

🔴 NOTHING HERE REACHES THE REAL `systemctl` OR THE REAL SERVICE. The command
runner is injected (a recorder), the systemd runtime directory is a path
under a `tempfile.TemporaryDirectory()`, and the health target is a stdlib
`http.server` bound to an ephemeral port on 127.0.0.1 — or a port that was
bound then released, so that nothing listens there.

Run: python3 tests/test_desk_service_start.py
"""
import http.server
import pathlib
import socket
import subprocess
import sys
import tempfile
import threading
import time

RACINE = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RACINE / "hooks"))
from service_start import bring_up, health_url, systemd_booted  # noqa: E402

failures = []
UNITE = "desk-plateforme.service"


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


class Recorder:
    """A command runner that records and answers, never executes."""

    def __init__(self, failing=None):
        self.commands = []
        self.failing = failing

    def __call__(self, command, **_kwargs):
        self.commands.append(list(command))
        code = 1 if command[1:2] == [self.failing] else 0
        return subprocess.CompletedProcess(command, code, "", "simulated failure"
                                           if code else "")


def serve(status: int, after_s: float = 0.0):
    """A /sante endpoint answering `status`, refusing connections until `after_s`."""
    debut = time.monotonic()

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):  # noqa: N802 - http.server naming
            pret = time.monotonic() - debut >= after_s
            code = status if pret and self.path == "/sante" else 503
            corps = b'{"etat":"ok"}' if code == 200 else b'{"etat":"degrade"}'
            self.send_response(code)
            self.send_header("content-type", "application/json")
            self.end_headers()
            self.wfile.write(corps)

        def log_message(self, *_args):
            pass

    serveur = http.server.HTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=serveur.serve_forever, daemon=True).start()
    return serveur


def free_port() -> int:
    """A port that was free a moment ago and on which nothing listens."""
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def env_for(port: int) -> dict:
    return {"PLATEFORME_HOTE": "127.0.0.1", "PLATEFORME_PORT": str(port)}


with tempfile.TemporaryDirectory() as tmp:
    booted_dir = pathlib.Path(tmp) / "run-systemd-system"
    booted_dir.mkdir()
    absent_dir = pathlib.Path(tmp) / "no-systemd-here"

    # --- sd_booted(3): the runtime directory decides -------------------------
    check("systemd_booted: directory present", systemd_booted(booted_dir), True)
    check("systemd_booted: directory absent", systemd_booted(absent_dir), False)

    # --- health_url follows desk.env ------------------------------------------
    check("health_url: IPv4 host", health_url(env_for(3445)),
          "http://127.0.0.1:3445/sante")
    check("health_url: IPv6 host", health_url({"PLATEFORME_HOTE": "::1",
                                                 "PLATEFORME_PORT": "8080"}),
          "http://[::1]:8080/sante")

    # --- 1. daemon-reload then RESTART, and a service that answers -------------
    serveur = serve(200)
    runner = Recorder()
    ok, message = bring_up(UNITE, env_for(serveur.server_port),
                           runtime_dir=booted_dir, runner=runner, delai_s=5)
    serveur.shutdown()
    check("answering service: commands", runner.commands,
          [["systemctl", "daemon-reload"], ["systemctl", "restart", UNITE]])
    check("answering service: success", ok, True)
    check("answering service: the message says what was verified",
          "/sante" in message, True)

    # --- 2. a service that comes up late, within the deadline ------------------
    serveur = serve(200, after_s=1.0)
    ok, _ = bring_up(UNITE, env_for(serveur.server_port), runtime_dir=booted_dir,
                     runner=Recorder(), delai_s=6)
    serveur.shutdown()
    check("late but within the deadline: success", ok, True)

    # --- 3. nothing ever listens: fatal ----------------------------------------
    debut = time.monotonic()
    ok, message = bring_up(UNITE, env_for(free_port()), runtime_dir=booted_dir,
                           runner=Recorder(), delai_s=1.5)
    duree = time.monotonic() - debut
    check("never answers: failure", ok, False)
    check("never answers: the deadline is honoured", duree < 5, True)
    check("never answers: the message names the health route", "/sante" in message,
          True)

    # --- 4. the route answers 503 (database down): fatal -----------------------
    serveur = serve(503)
    ok, message = bring_up(UNITE, env_for(serveur.server_port),
                           runtime_dir=booted_dir, runner=Recorder(), delai_s=1.5)
    serveur.shutdown()
    check("503 only: failure", ok, False)
    check("503 only: the last answer is reported", "503" in message, True)

    # --- 5. a failed restart is fatal, and nothing is awaited -------------------
    runner = Recorder(failing="restart")
    debut = time.monotonic()
    ok, message = bring_up(UNITE, env_for(free_port()), runtime_dir=booted_dir,
                           runner=runner, delai_s=30)
    check("failed restart: failure", ok, False)
    check("failed restart: no wait for health", time.monotonic() - debut < 5, True)
    check("failed restart: the message names the command",
          "systemctl restart" in message, True)

    # --- 6. a failed daemon-reload stops before the restart --------------------
    runner = Recorder(failing="daemon-reload")
    ok, _ = bring_up(UNITE, env_for(free_port()), runtime_dir=booted_dir,
                     runner=runner, delai_s=30)
    check("failed daemon-reload: failure", ok, False)
    check("failed daemon-reload: no restart attempted", runner.commands,
          [["systemctl", "daemon-reload"]])

    # --- 7. systemd is not the running init: non-fatal, nothing run -------------
    runner = Recorder()
    ok, message = bring_up(UNITE, env_for(free_port()), runtime_dir=absent_dir,
                           runner=runner, delai_s=30)
    check("constrained environment: non-fatal", ok, True)
    check("constrained environment: systemctl never called", runner.commands, [])
    check("constrained environment: the message says it was not started",
          "not started" in message, True)

    # --- 8. desk.env without the listening address: fatal, said ---------------
    ok, message = bring_up(UNITE, {}, runtime_dir=booted_dir, runner=Recorder(),
                           delai_s=1)
    check("no PLATEFORME_HOTE/PORT: failure", ok, False)

if failures:
    print(f"FAILED ({len(failures)})")
    for f in failures:
        print(" -", f)
    sys.exit(1)
print("OK - service restart and health check")
