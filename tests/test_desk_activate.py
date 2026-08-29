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

🔴 TÂCHE 6 — depuis le câblage de `hooks/vm.py::poser_projfs`/
`poser_vb_audio` dans `main()`, CE HOOK RÉSOUT `winrm_exec.py` PAR
`NIVUUS_PACKAGES_DIR` À CHAQUE APPEL, AVANT MÊME LA VÉRIFICATION DE
`plateforme/`. `appeler()` fabrique donc, PAR DÉFAUT, un faux
`console/guest/winrm_exec.py` FONCTIONNEL (jamais le vrai, jamais un appel
réseau) sous un `NIVUUS_PACKAGES_DIR` de test — sans quoi TOUS les
scénarios ci-dessous échoueraient sur « console absent » avant d'atteindre
la raison qu'ils veulent réellement éprouver. `packages_dir=False` simule
`console` absent, pour les scénarios dédiés qui éprouvent CE refus.

Run: python3 tests/test_desk_activate.py
"""
import json
import os
import pathlib
import subprocess
import sys
import tempfile

from desk_activate_fixtures import (
    HOOK,
    HW_AVEC_FACTS,
    RACINE,
    REPONSES,
    appeler,
    lire_commandes,
    lire_env,
    load_unit,
    poser_faux_npm,
    poser_faux_systemctl,
    poser_faux_winrm_exec,
    poser_racine_installee,
)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


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

    # --- TACHE 13 : la VM tout juste enrolee est ATTRIBUEE au compte cree --
    # Trou trouve en production le 29 aout 2026 : sans cet appel,
    # `vm.utilisateur_id` reste NULL et `GET /vm` rend `{"vms":[]}` pour
    # l'utilisateur pourtant bien cree - voir le commentaire d'activate.py.
    appel_attribuer = next((c for c in entrees if "admin:attribuer" in c["argv"]), None)
    check("admin:attribuer est invoque apres l'enrolement (attribution de la VM)",
          appel_attribuer is not None, True)
    if appel_attribuer is not None:
        check("--email est passe a l'attribution (le compte tout juste cree)",
              "ada@exemple.test" in appel_attribuer["argv"], True)
        check("--vm est passe a l'attribution, et c'est le vm_id rendu par "
              "admin:agent (jamais le nom, jamais invente)",
              "vm-test-uuid" in appel_attribuer["argv"], True)
        idx_attribuer = ordre.index(appel_attribuer["argv"])
        check("l'attribution est lancee APRES l'enrolement, jamais avant",
              idx_attribuer > idx_agent, True)
        check("aucun secret dans l'appel d'attribution (email/vm ne le sont pas)",
              appel_attribuer["stdin"], "")

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

# 🔴 TÂCHE 6 : `activate.py` fait désormais `from vm import ...`, et `vm.py`
# vit à côté de lui dans `hooks/` — exactement comme `commun.py`. Un import
# par CHEMIN (`spec_from_file_location`) ne passe PAS par le mécanisme qui
# ajoute automatiquement le répertoire du script à `sys.path` (ça, c'est
# l'interprète qui le fait pour un script LANCÉ, pas pour un module chargé
# ainsi) : sans cette ligne, `exec_module` lève `ModuleNotFoundError: vm`.
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


# === Tâche 6 : le câblage de hooks/vm.py dans main() — B éprouve
# poser_projfs (via l'absence du contrat inter-packages), C éprouve
# poser_vb_audio (via son armement sans payload). Leur comportement PROPRE
# (redemarrage_requis, l'exécuteur factice, le contrat de winrm_exec.py) est
# déjà éprouvé en détail par tests/test_desk_vm.py ; ici, seul le CÂBLAGE
# dans main() compte. ========================================================

def _racine_pour_tache6(tmp):
    """Racine installée + faux npm, communs aux deux scénarios ci-dessous."""
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")
    return root, bin_dir


def _check_refus_propre(prefix, r, doit_contenir, root):
    """Un refus propre : code de sortie non nul, la raison nommée dans
    stderr, aucune trace Python, et aucune commande npm lancée — le refus
    doit précéder toute tentative de création de compte ou d'enrôlement."""
    check(f"{prefix} : refus propre (rc != 0)", r.returncode != 0, True)
    check(f"{prefix} : le refus nomme {doit_contenir!r}",
          doit_contenir in (r.stderr or ""), True)
    check(f"{prefix} : aucune trace Python", "Traceback" in (r.stderr or ""), False)
    check(f"{prefix} : aucun npm lance", (root / "npm.log").exists(), False)


# --- B : `console` absent (aucun winrm_exec.py sous NIVUUS_PACKAGES_DIR) —
# refus propre du hook COMPLET, pas seulement du module hooks/vm.py isolé --
with tempfile.TemporaryDirectory() as tmp:
    root, bin_dir = _racine_pour_tache6(tmp)
    r = appeler(root, bin_dir, packages_dir=False)
    chemin_attendu = str(root / "console-absent-ici" / "console" / "guest" / "winrm_exec.py")
    _check_refus_propre("console absent", r, chemin_attendu, root)
    # L'armement du service, lui, précède la résolution WinRM dans main() :
    # il a quand même dû se faire avant ce refus.
    lien = (root / "etc" / "systemd" / "system" / "multi-user.target.wants"
            / "desk-plateforme.service")
    check("console absent : l'armement a quand meme eu lieu", os.path.islink(lien), True)


# --- B-bis : LA PORTE DE LA VM WINDOWS, MIGRÉE DEPUIS `resolve` -----------
# 🔴 CORRECTION DE LA CRITIQUE DE LA REVUE FINALE DE BRANCHE (30 août 2026).
# `hooks/resolve.py` portait `if not hw.get("vm_windows"): refuser(...)` :
# une clé qu'AUCUN producteur du moteur ne pose, éprouvée à une phase où la
# VM ne peut pas encore exister. La porte vit désormais ICI, et elle est une
# MESURE — un échange WinRM réel. Ce scénario est sa ROUGE : `console` EST
# installé (le contrat inter-packages se résout), mais le guest ne répond
# pas. Il se distingue du scénario B ci-dessus, où c'est `console` qui
# manque : deux causes différentes, deux messages différents.
with tempfile.TemporaryDirectory() as tmp:
    root, bin_dir = _racine_pour_tache6(tmp)
    packages_dir = root / "faux-packages-dir"
    guest = packages_dir / "console" / "guest"
    guest.mkdir(parents=True)
    (guest / "fetch_payload.py").write_text("", encoding="utf-8")
    (guest / "winrm_exec.py").write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        # Le texte DIT qu'il est factice : voir la meme precaution dans
        # tests/test_desk_vm.py, ou un message imitant une vraie panne
        # reseau avait trompe une relectrice.
        "sys.stderr.write('FAUX winrm_exec.py de test : echec simule, "
        "aucune VM contactee\\n')\n"
        "sys.exit(1)\n", encoding="utf-8")
    (guest / "winrm_exec.py").chmod(0o755)

    r = appeler(root, bin_dir, packages_dir=packages_dir)
    _check_refus_propre("VM injoignable", r, "echec simule", root)
    check("VM injoignable : le refus nomme la VM Windows, pas ProjFS",
          "la VM Windows ne repond pas" in (r.stderr or ""), True)
    check("VM injoignable : le refus dit que console la provisionne",
          "console la provisionne" in (r.stderr or ""), True)
    check("VM injoignable : le refus dit que ce n'est PAS le role de resolve",
          "jamais dans resolve" in (r.stderr or ""), True)


# --- C : VB-Audio ARMÉ (vb_audio=true), aucun payload — refus propre du
# hook complet. ⚠️ Depuis la ronde de correction 1, `hooks/resolve.py`
# refuse déjà ce cas PLUS TÔT, avant l'installation (voir
# tests/test_desk_resolve.py) : ce scénario-ci reste utile en DÉFENSE EN
# PROFONDEUR — il éprouve qu'`activate.py`, appelé seul (comme le ferait un
# rejeu direct du moteur, sans repasser par resolve), ne prétend jamais
# avoir installé VB-Audio quand aucun payload n'existe. ---------------------
with tempfile.TemporaryDirectory() as tmp:
    root, bin_dir = _racine_pour_tache6(tmp)
    packages_dir, log_winrm = root / "faux-packages-dir", root / "winrm.log"
    poser_faux_winrm_exec(packages_dir, log_winrm)  # ProjFS reussit (sortie vide)
    r = appeler(root, bin_dir, answers=dict(REPONSES, vb_audio=True),
                packages_dir=packages_dir)
    _check_refus_propre("vb_audio arme sans payload", r, "VB-Audio", root)
    # ProjFS précède : vb_audio lève avant même d'atteindre le faux
    # winrm_exec.py, donc une seule commande (celle de ProjFS) a été vue.
    check("ProjFS pose avant le refus vb_audio (leve avant winrm_exec.py)",
          len(lire_commandes(log_winrm)), 1)


# === Scenario 5 (TACHE 13) : l'attribution est REFUSEE (VM deja attribuee
# a un autre compte) - le hook doit echouer proprement, nommer la cause, et
# ne PAS ecrire AGENT_VM/AGENT_SECRET comme si de rien n'etait. ============
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "faux-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    os.environ["FAUX_NPM_ATTRIBUER_ECHEC"] = "1"
    try:
        r = appeler(root, bin_dir)
    finally:
        del os.environ["FAUX_NPM_ATTRIBUER_ECHEC"]

    check("attribution refusee : le hook echoue proprement (rc != 0)",
          r.returncode != 0, True)
    check("aucune trace Python", "Traceback" in (r.stderr or ""), False)
    check("le refus nomme l'attribution",
          "attribution" in (r.stderr or ""), True)
    entrees = lire_commandes(root / "npm.log")
    check("compte ET enrolement ont bien tourne avant le refus d'attribution",
          any("admin:agent" in c["argv"] for c in entrees), True)


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook activate passés")
