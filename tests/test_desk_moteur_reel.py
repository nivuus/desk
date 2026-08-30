#!/usr/bin/env python3
"""La chaîne resolve → install → activate, JOUÉE PAR L'API DU MOTEUR.

🔴 POURQUOI CETTE SUITE EXISTE. La revue finale de branche (30 août 2026) a
désigné comme CAUSE RACINE de son unique Critique le fait qu'« aucune
installation n'a jamais été jouée par le moteur réel ». Le défaut concret
était `hooks/resolve.py` refusant sur `hw["vm_windows"]`, une clé qu'AUCUN
producteur du moteur ne pose : le hook refusait donc TOUJOURS, et
`installer/installer/install-engine/steps/packages.py::plan_packages`
traduit un refus en `StepError`, ce qui arrête l'installation ENTIÈRE. Neuf
revues et huit suites de tests ne l'ont pas vu, pour UNE seule raison :
**toutes fabriquaient elles-mêmes le contexte d'entrée**
(`{"hw": {"vm_windows": True}}`). Un test qui invente son entrée ne peut pas
découvrir que personne ne la produit.

`tests/test_desk_contrat_hw.py` a posé le garde STATIQUE (les clés du
producteur, lues par `ast`). Cette suite-ci est le cran au-dessus : elle ne
lit pas le contrat, elle le FAIT COURIR — `hw` vient de
`installer/installer/common/hardware.py::detect_all()`, le manifeste de
`packages/manifest.py::load_manifest`, les réponses de
`packages/wizard.py::validate_answers`, et les trois hooks sont lancés par
`packages/runner.py`, en sous-processus, par le protocole jsonl réel.

--- 🔴 CE QUE CETTE SUITE S'INTERDIT, ET POURQUOI ------------------------

Le moteur complet PARTITIONNE ET EFFACE DES DISQUES : `install-engine/run.py`
appelle `partition.partition_and_format()` (ligne 82) juste après
`plan_packages()` (ligne 72). Cette suite N'APPELLE JAMAIS `run.py`, ni
`plan_packages()`, ni `apply_packages()` — ce dernier fait en plus
`chroot_run(target, ["apt-get", ...])` et `os.symlink` dans
`<target>/etc/systemd/system/multi-user.target.wants/`. Elle appelle les
TROIS fonctions de `packages/runner.py` qui exécutent les hooks, et elles
seules.

🔴 UNE SEULE SUBSTITUTION, ET ELLE EST NOMMÉE : `run_activate` NE PASSE
AUCUN `--root` (voir `runner.py::run_activate`, qui appelle `_run_hook(...)`
sans le paramètre `root`, dont le défaut est `""`), donc en production le
hook `activate` travaille sur `/`. Sur CETTE machine, `/etc/systemd/system/
desk-plateforme.service` EXISTE (l'installation réelle du lot 10A) : appeler
`run_activate` tel quel y créerait le lien `multi-user.target.wants/` puis
lancerait `systemctl daemon-reload` et `systemctl start` — sur le systemd du
propriétaire, en root. Cette suite appelle donc `runner._run_hook(...,
root=<racine temporaire>)`, la fonction que `run_activate` emploie
elle-même, avec le MÊME contexte (`merge_into_hw(hw, facts)`, la vraie
fusion du moteur) et un seul argument de plus. L'étape ⑦ ci-dessous mesure
que l'hôte n'a pas bougé, et le contrat de `run_activate` (aucun `root`
dans sa signature) est éprouvé à l'étape ⑥ plutôt que supposé.

⚠️ `NIVUUS_PACKAGES_DIR` est délibérément pointé vers un répertoire VIDE
avant la phase `activate` : `hooks/vm.py::chemin_winrm_exec` y cherche
`console/guest/winrm_exec.py`, et un `console` réellement trouvé ferait
partir un vrai échange WinRM vers la VM Windows. Le refus attendu est donc
DÉTERMINISTE, et aucun paquet ne quitte cette machine.

--- Ce que cette suite NE COUVRE PAS (nommé plutôt que laissé croire) -----

  - le PARTITIONNEMENT et le formatage (`partition_and_format`), le
    debootstrap, le bootloader : jamais appelés, par construction ;
  - `apply_packages()` lui-même — l'`apt-get` en chroot, l'unité
    d'activation du premier boot, l'écriture de `etc/nivuus/packages.json` ;
  - le PREMIER DÉMARRAGE RÉEL : aucun `systemctl` ne court ici, donc rien
    n'établit que le service démarre — c'est ce que le lot 10A avait établi
    à la main, sur cette machine, et qu'aucun test ne rejoue ;
  - le MATÉRIEL de la cible : `detect_all()` décrit la machine qui fait
    tourner ces tests, jamais l'appliance ;
  - la fin d'`activate` (compte administrateur, enrôlement, attribution de
    la VM) : elle est INATTEIGNABLE sans VM Windows, et l'étape ⑥ mesure
    précisément qu'on s'arrête à cette porte-là.

--- CE QUI A ÉTÉ MESURÉ, LE 30 AOÛT 2026 (porté ICI, pas dans un rapport
    gitignoré : « une preuve ne doit jamais vivre dans un rapport
    gitignoré ») ------------------------------------------------------------

La ROUGE a été jouée en réinsérant la porte d'origine —
`if not hw.get("vm_windows"): refuser(...)` — dans une COPIE HORS DE
L'ARBRE SUIVI (`/var/tmp/desk-rouge-vm-windows/paquet`, `hooks/` copié en
dur, `plateforme/`, `client/`, `proto/` et les deux YAML par liens), puis en
pointant cette suite dessus par `DESK_PAQUET_RACINE`. Sortie :

    FAIL (3)
      - resolve accepte cette machine: got False, want True
      - resolve ne donne aucune raison de refus: got 'aucune VM Windows
        detectee sur cette machine', want ''
      - resolve a REFUSE : l'installation entiere s'arreterait ici
        (StepError). Raison rendue par le hook : 'aucune VM Windows
        detectee sur cette machine'

🔴 ET LA PREUVE QUI COMPTE VRAIMENT : sur LA MÊME copie mutée, la suite
`tests/test_desk_resolve.py` **telle qu'elle était la veille de la
correction** (`git show dd263bb~1:tests/test_desk_resolve.py`, celle qui
appelle `appeler(hw={"vm_windows": True}, ...)` à sept endroits) rend
`OK - tests du hook resolve passés`, code 0. Le défaut d'hier, la suite
d'hier : VERTE. C'est le patron entier de la Critique, reproduit et mesuré.

VERTE sur le produit d'aujourd'hui : `make test` rend les onze suites
vertes, dont `OK - chaine resolve/install/activate jouee par l'API du
moteur`.

Run: python3 tests/test_desk_moteur_reel.py
"""
import inspect
import json
import os
import pathlib
import shutil
import sys
import tempfile

RACINE_DEPOT = pathlib.Path(__file__).resolve().parents[1]
# Surchargeable — MÊME CONVENTION que `DESK_INSTALLER_RACINE` ci-dessous, et
# c'est par elle que la ROUGE de cette suite se joue : on pointe une COPIE
# hors de l'arbre suivi, dans laquelle la porte `hw["vm_windows"]` d'origine
# a été réinsérée. Voir le § « Ce que cette suite s'interdit ».
PAQUET = pathlib.Path(os.environ.get("DESK_PAQUET_RACINE") or RACINE_DEPOT)
INSTALLER = pathlib.Path(os.environ.get("DESK_INSTALLER_RACINE")
                         or (RACINE_DEPOT.parent / "installer"))
sys.path.insert(0, str(INSTALLER / "installer"))

# ⚠️ SI LE DÉPÔT VOISIN EST ABSENT, CETTE SUITE ÉCHOUE, elle ne se saute
# pas : « un `||` de repli transforme fichier absent en contrôle vert ».
from common import hardware                                    # noqa: E402
from packages import runner                                    # noqa: E402
from packages.dependencies import missing_dependencies         # noqa: E402
from packages.discovery import discover                        # noqa: E402
from packages.facts import merge_into_hw, shadowed_facts       # noqa: E402
from packages.manifest import load_manifest                    # noqa: E402
from packages.wizard import load_questions, validate_answers   # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


def check_vrai(label, condition, detail=""):
    if not condition:
        failures.append(f"{label}: faux{(' — ' + detail) if detail else ''}")


def terminer(base):
    """Efface la racine temporaire, imprime le verdict, sort."""
    if base is not None and str(base).startswith(tempfile.gettempdir()):
        shutil.rmtree(base, ignore_errors=True)
    if failures:
        print(f"FAIL ({len(failures)})")
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("OK - chaine resolve/install/activate jouee par l'API du moteur")
    sys.exit(0)


class Collecteur:
    """Le puits de progression que le moteur attend d'un appelant.

    C'est la SEULE pièce que cette suite fournit elle-même, et elle ne porte
    aucune donnée d'entrée : le vrai `emit` est celui du portail
    (`install-engine/progress.py`), un observateur, jamais une source de
    contexte. Il est gardé pour que la trace jsonl des hooks soit lisible
    quand un contrôle rougit.
    """

    def __init__(self):
        self.lignes = []

    def info(self, etape, pct, msg):
        self.lignes.append(("info", etape, pct, msg))

    def warn(self, etape, pct, msg):
        self.lignes.append(("warn", etape, pct, msg))


# --- ⓪ La garde de sûreté, avant toute chose ------------------------------
# Elle ne protège pas d'une erreur d'inattention : elle protège du cas où
# `tempfile` serait détourné. Une racine cible qui ne serait pas sous le
# répertoire temporaire fait échouer la suite AVANT d'écrire un octet.

base = pathlib.Path(tempfile.mkdtemp(prefix="desk-moteur-reel-"))
cible = base / "cible"
cible.mkdir()
sans_console = base / "sans-console"
sans_console.mkdir()
catalogue = base / "paquets"
catalogue.mkdir()

if not str(cible.resolve()).startswith(tempfile.gettempdir()):
    print(f"REFUS : la racine cible {cible} n'est pas sous "
          f"{tempfile.gettempdir()} ; rien n'a ete ecrit.", file=sys.stderr)
    sys.exit(1)
check_vrai("la racine cible n'est jamais /", str(cible.resolve()) != "/")

# --- ① Le `hw` vient du MOTEUR, jamais de nous ----------------------------
# `install-engine/run.py:68` fait `hw = hardware.detect_all()` et le passe
# VERBATIM à `plan_packages` (`:72`) puis à `run_resolve`. C'est cet appel-là
# qu'on refait — la détection est entièrement en LECTURE (lsblk, ip, lspci,
# /proc), aucune commande n'écrit.

hw = hardware.detect_all()

check_vrai("detect_all() rend un mapping", isinstance(hw, dict))
check("les huit cles du producteur", sorted(hw), sorted([
    "disks", "ethernet", "wifi", "gpus", "cpu", "iommu", "memory_mib",
    "passthrough_candidates"]))
# 🔴 LE CONTRÔLE QUI PORTE LA CRITIQUE : la clé sur laquelle `resolve.py`
# refusait n'est produite par PERSONNE. Ce n'est pas une opinion sur le code,
# c'est le producteur réel, exécuté.
check_vrai("aucun producteur ne pose vm_windows", "vm_windows" not in hw,
           f"cles rendues : {sorted(hw)}")

# --- ② Le manifeste et le catalogue, par le moteur ------------------------
# `discover()` est la fonction que `plan_packages` appelle ; on lui donne un
# catalogue temporaire (lien vers ce dépôt, lien vers `console`) plutôt que
# de poser `NIVUUS_PACKAGES_DIR` dans l'environnement — cette variable est
# aussi celle que `hooks/vm.py` lit, et un `console` trouvé ferait partir un
# vrai échange WinRM (voir le docstring de tête).

os.symlink(PAQUET, catalogue / "desk")
if (INSTALLER / "console" / "nivuus-package.yaml").is_file():
    os.symlink(INSTALLER / "console", catalogue / "console")

manifestes, erreurs = discover(root=str(catalogue))
check("aucun manifeste refuse par discover()", erreurs, [])
noms = sorted(m.name for m in manifestes)
check_vrai("le moteur decouvre desk", "desk" in noms, f"decouverts : {noms}")
check_vrai("le moteur decouvre console (pre-requis dur)", "console" in noms,
           f"decouverts : {noms}")

manifeste = load_manifest(str(PAQUET / "nivuus-package.yaml"))
check("le manifeste joue est bien desk", manifeste.name, "desk")

# La porte de dépendance du moteur, éprouvée DANS LES DEUX SENS — un contrôle
# qu'on n'a jamais vu rouge n'est pas un contrôle.
seul = missing_dependencies([manifeste], manifestes)
check_vrai("desk seul manque console",
           [m.requires for m in seul] == ["console"],
           f"manquants : {[m.requires for m in seul]}")
avec = missing_dependencies(manifestes, manifestes)
check("desk avec console ne manque rien", avec, [])

# --- ③ Les réponses, validées par le VRAI wizard --------------------------
# Les réponses BRUTES sont celles d'un opérateur (c'est leur nature : le
# wizard les lui demande). Ce qui vient du moteur est leur VALIDATION et le
# remplissage des défauts — `vb_audio` n'est pas écrit ici, c'est
# `validate_answers` qui doit le poser à `False` depuis `wizard.yaml`.

questions = load_questions(str(PAQUET / manifeste.questions_file))
answers = validate_answers(questions, {
    "admin_email": "operateur@example.test",
    "admin_password": "un mot de passe de recette, jamais un secret reel",
    "auth_mode": "motdepasse",
})
check("le wizard pose le defaut vb_audio", answers.get("vb_audio"), False)
check("quatre reponses validees", sorted(answers), sorted(
    ["admin_email", "admin_password", "auth_mode", "vb_audio"]))

# --- ④ resolve, par `run_resolve` ----------------------------------------
# 🔴 C'EST LE CONTRÔLE QUI ROUGIT SUR LE DÉFAUT D'ORIGINE. Avec la porte
# `hw["vm_windows"]` en place, `resolution.ok` vaut False et `reason` porte
# la phrase de refus — et côté moteur ce refus devient un `StepError` qui
# arrête l'installation entière (`steps/packages.py:206-207`).

emetteur = Collecteur()
resolution = runner.run_resolve(manifeste, hw, answers, emetteur)

check("resolve accepte cette machine", resolution.ok, True)
check("resolve ne donne aucune raison de refus", resolution.reason, "")
if not resolution.ok:
    failures.append(
        "resolve a REFUSE : l'installation entiere s'arreterait ici "
        f"(StepError). Raison rendue par le hook : {resolution.reason!r}")
    terminer(base)

# tier `userspace` : le moteur doit rendre un bloc plateforme vide.
check("aucun module noyau resolu", resolution.platform.modules, ())
check("aucune ligne de commande noyau resolue",
      resolution.platform.kernel_cmdline, ())

# Les facts sont ceux que le hook a émis, relus par `parse_facts_event`.
check("les six faits du canal resolve -> activate", sorted(resolution.facts),
      sorted(["node_version", "turn_ecoute", "turn_relais", "hote",
              "proxy_confiance", "port"]))
check_vrai("le hook a parle le protocole de progression",
           any(l[3].startswith("[desk]") for l in emetteur.lignes),
           f"lignes : {emetteur.lignes}")

# --- ⑤ install, par `run_install`, sur une racine temporaire --------------
# ⚠️ `run_install` NE REÇOIT PAS LES `facts` : c'est ce que fait
# `apply_packages` (`run_install(manifest, hw, answers, target, emit)`), et
# c'est reproduit tel quel. Le hook DÉRIVE donc lui-même ce dont il a besoin.

emetteur_install = Collecteur()
runner.run_install(manifeste, hw, answers, str(cible), emetteur_install)

env_pose = cible / "etc" / "nivuus" / "desk.env"
check_vrai("desk.env est pose sous la racine cible", env_pose.is_file(),
           str(env_pose))
check("desk.env n'est lisible que par son proprietaire",
      oct(env_pose.stat().st_mode & 0o777), "0o600")

valeurs = {}
for ligne in env_pose.read_text(encoding="utf-8").splitlines():
    if "=" in ligne and not ligne.startswith("#"):
        cle, _, valeur = ligne.partition("=")
        valeurs[cle.strip()] = valeur.strip()

check("PLATEFORME_AUTH vient de la reponse validee",
      valeurs.get("PLATEFORME_AUTH"), answers["auth_mode"])
check_vrai("PLATEFORME_HOTE n'est jamais une ecoute universelle",
           valeurs.get("PLATEFORME_HOTE") not in
           ("0.0.0.0", "::", "[::]", "*", None),
           f"valeur : {valeurs.get('PLATEFORME_HOTE')!r}")
check_vrai("le secret de jeton fait au moins 32 caracteres",
           len(valeurs.get("PLATEFORME_SECRET_JETON", "")) >= 32)

unite = cible / "etc" / "systemd" / "system" / "desk-plateforme.service"
check_vrai("l'unite systemd est POSEE", unite.is_file(), str(unite))
check_vrai("le jeton __NODE_BIN__ est substitue",
           "__NODE_BIN__" not in unite.read_text(encoding="utf-8"))
check_vrai("l'unite n'est PAS armee par install",
           not (cible / "etc" / "systemd" / "system" /
                "multi-user.target.wants" / "desk-plateforme.service").exists())
check_vrai("proto/ts est deploye (ERR_MODULE_NOT_FOUND sinon)",
           (cible / "opt/nivuus/desk/proto/ts/plateforme.ts").is_file())
check_vrai("tsx est deploye (npm start = tsx src/index.ts)",
           (cible / "opt/nivuus/desk/plateforme/node_modules/.bin/tsx").exists())
check_vrai("le runtime node est DEPOSE, pas suppose",
           (cible / "opt/nivuus/node/bin/node").is_file())
check_vrai("turnserver.conf est pose",
           (cible / "etc" / "turnserver.conf").is_file())

# --- ⑥ activate : le contrat de `run_activate`, puis la chaine ------------
# 🔴 LE CONTRAT EST ÉPROUVÉ, PAS SUPPOSÉ : c'est parce que `run_activate` ne
# prend aucun `root` que cette suite ne peut pas l'appeler telle quelle sur
# cette machine (voir le docstring de tête).

signature = inspect.signature(runner.run_activate)
check_vrai("run_activate ne prend AUCUN parametre root",
           "root" not in signature.parameters,
           f"parametres : {list(signature.parameters)}")
check_vrai("run_activate recoit bien les facts",
           "facts" in signature.parameters)
check_vrai("run_install ne recoit AUCUN facts",
           "facts" not in inspect.signature(runner.run_install).parameters)

# La fusion est celle du moteur, pas la nôtre.
hw_active = merge_into_hw(hw, resolution.facts)
check("aucun fait n'est masque par la detection fraiche",
      shadowed_facts(hw, resolution.facts), [])
check("le port mesure par resolve survit la fusion",
      hw_active.get("port"), resolution.facts["port"])

# Aucun `console` ici : la porte VM refuse sans jamais toucher le réseau.
os.environ["NIVUUS_PACKAGES_DIR"] = str(sans_console)

avant = {
    "unite hote": os.lstat("/etc/systemd/system/desk-plateforme.service")
    if os.path.exists("/etc/systemd/system/desk-plateforme.service") else None,
    "env hote": os.lstat("/etc/nivuus/desk.env")
    if os.path.exists("/etc/nivuus/desk.env") else None,
}

emetteur_activate = Collecteur()
erreur_activate = None
try:
    runner._run_hook(manifeste, "activate", hw_active, answers,
                     root=str(cible), emit=emetteur_activate)
except runner.HookError as exc:
    erreur_activate = str(exc)

# 🔴 CE QUE LA CHAÎNE ÉTABLIT ICI : `activate` ARME l'unité (un lien, sous la
# racine temporaire), puis S'ARRÊTE À LA PORTE DE LA VM — la porte que la
# revue finale a fait migrer de `resolve` vers `activate`. Un refus est le
# résultat JUSTE sur une machine sans VM Windows ; ce qui compte est QUELLE
# assertion refuse, pas le seul code de sortie.
lien = (cible / "etc" / "systemd" / "system" / "multi-user.target.wants"
        / "desk-plateforme.service")
check_vrai("activate a ARME l'unite, sous la racine temporaire",
           lien.is_symlink(), str(lien))
check("le lien arme est RELATIF", os.readlink(lien),
      "../desk-plateforme.service")
check_vrai("activate refuse a la porte de la VM Windows",
           erreur_activate is not None
           and "la VM Windows ne repond pas" in erreur_activate,
           f"erreur rendue : {erreur_activate!r}")
check_vrai("le refus nomme winrm_exec.py, jamais un echec reseau",
           erreur_activate is not None
           and "winrm_exec.py introuvable" in erreur_activate,
           f"erreur rendue : {erreur_activate!r}")

# --- ⑦ L'HÔTE N'A PAS BOUGÉ ----------------------------------------------
# Le contrôle qui vaut : si cette suite avait appelé `run_activate` comme la
# production le fait, le lien `multi-user.target.wants/` de CETTE machine et
# `systemctl start` auraient couru. On mesure l'inverse.

apres = {
    "unite hote": os.lstat("/etc/systemd/system/desk-plateforme.service")
    if os.path.exists("/etc/systemd/system/desk-plateforme.service") else None,
    "env hote": os.lstat("/etc/nivuus/desk.env")
    if os.path.exists("/etc/nivuus/desk.env") else None,
}
for nom in avant:
    a, b = avant[nom], apres[nom]
    check(f"{nom} : mtime inchange",
          None if a is None else a.st_mtime_ns,
          None if b is None else b.st_mtime_ns)
terminer(base)
