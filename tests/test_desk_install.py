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

sys.path.insert(0, str(RACINE / "hooks"))
from commun import HOTE_DEFAUT, NODE_BIN_DEFAUT, PROXY_DEFAUT  # noqa: E402

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
# 🔴 `hote` et `proxy_confiance` sont VOLONTAIREMENT DIFFÉRENTES de
# `turn_ecoute` ci-dessous : c'est ce qui rend le test capable de détecter
# une régression vers le bug réel corrigé au lot 10A (29 août 2026) — avant
# ce correctif, `install.py` posait `PLATEFORME_HOTE = turn_ecoute`, ce qui
# aurait fait écouter le service sur l'adresse dérivée pour TURN (publique,
# sur la machine réelle) plutôt que sur une adresse interne.
FACTS = {"vm_repond": True, "node_version": "24.9.0",
         "turn_ecoute": "203.0.113.9", "turn_relais": "203.0.113.9",
         "hote": "198.51.100.1", "proxy_confiance": "198.51.100.1",
         "port": 9999}


def appeler(root, hw=None, answers=None, facts=None, env=None):
    """Appelle le hook comme le moteur : --phase/--root, stdin JSON."""
    # `hw` par défaut VIDE : le moteur envoie `detect_all()` verbatim, et
    # `install.py` n'y lit rien — voir tests/test_desk_contrat_hw.py, qui
    # fige ce contrat. Il portait `{"vm_windows": True}`, une clé qu'aucun
    # producteur ne pose (Critique de la revue finale de branche).
    contexte = {"hw": hw if hw is not None else {},
                "answers": answers if answers is not None else REPONSES}
    if facts is not None:
        contexte["facts"] = facts
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

    # 🔴 REGRESSION FERMEE AU LOT 10A (29 aout 2026) : PLATEFORME_HOTE DOIT
    # venir de facts["hote"], JAMAIS de facts["turn_ecoute"] — les deux
    # etaient confondues avant ce correctif (voir hooks/commun.py::
    # HOTE_DEFAUT pour le bug reel que la separation corrige : sur la
    # machine de developpement, la derivation TURN rend l'adresse PUBLIQUE
    # de la route par defaut, et PLATEFORME_HOTE en heritait). FACTS pose
    # deux valeurs DIFFERENTES pour "hote" et "turn_ecoute" precisement pour
    # que ce test ne puisse pas rester vert par accident si la confusion
    # revenait.
    check("PLATEFORME_HOTE vient de facts['hote'], jamais de turn_ecoute",
          env1.get("PLATEFORME_HOTE"), FACTS["hote"])
    check("PLATEFORME_HOTE n'est PAS l'adresse TURN (les deux sont decouplees)",
          env1.get("PLATEFORME_HOTE") == FACTS["turn_ecoute"], False)
    check("PLATEFORME_PROXY_DE_CONFIANCE vient de facts['proxy_confiance']",
          env1.get("PLATEFORME_PROXY_DE_CONFIANCE"), FACTS["proxy_confiance"])

    # PLATEFORME_AUTH vient de la reponse auth_mode, telle quelle.
    check("PLATEFORME_AUTH vient de la reponse auth_mode",
          env1.get("PLATEFORME_AUTH"), "motdepasse")

    # 🔴 Ronde de correction 1, trouvaille ① : DynamicUser=yes rend
    # /opt/nivuus/desk/plateforme EN LECTURE SEULE (ProtectSystem=strict
    # implicite) — le defaut relatif de PLATEFORME_ICONES/
    # PLATEFORME_TELEVERSEMENTS ("donnees/icones"/"donnees/televersements",
    # sous WorkingDirectory) y echouerait en EROFS au premier usage. Les
    # deux DOIVENT pointer sous /var/lib/nivuus-desk, le seul repertoire que
    # StateDirectory= rend inscriptible.
    check("PLATEFORME_ICONES pointe sous le repertoire d'etat inscriptible",
          env1.get("PLATEFORME_ICONES"), "/var/lib/nivuus-desk/icones")
    check("PLATEFORME_TELEVERSEMENTS pointe sous le repertoire d'etat inscriptible",
          env1.get("PLATEFORME_TELEVERSEMENTS"),
          "/var/lib/nivuus-desk/televersements")
    check("les deux repertoires ne sont PAS le defaut relatif sous WorkingDirectory",
          any(v.startswith("donnees/")
              for v in (env1.get("PLATEFORME_ICONES", ""),
                        env1.get("PLATEFORME_TELEVERSEMENTS", ""))),
          False)

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

    # 🔴 TROUVAILLE RÉELLE DU LOT 10A (29 août 2026) : `proto/ts/` n'était
    # PAS copié du tout, et `npm start` échouait dès le premier module qui
    # l'importe (`ERR_MODULE_NOT_FOUND` sur `../../../proto/ts/plateforme`,
    # mesuré sur le vrai service). `proto/ts/` doit être un FRÈRE de
    # `plateforme/` sous la racine déployée, exactement comme dans ce
    # dépôt de développement — jamais sous `plateforme/`.
    proto_dep = root1 / "opt" / "nivuus" / "desk" / "proto" / "ts"
    check("proto/ts/plateforme.ts est copie (le service en depend a l'execution)",
          (proto_dep / "plateforme.ts").is_file(), True)
    check("proto/ts/*.test.ts n'est PAS copie (jamais execute par le service)",
          list(proto_dep.glob("*.test.ts")), [])

    # 🔴 BUG RÉEL TROUVÉ AU LOT 10A (29 août 2026), EN LANÇANT LE VRAI
    # SERVICE : sous l'umask 027 du root de la machine, /opt/nivuus/desk et
    # /opt/nivuus/desk/plateforme naissaient en drwxr-x---, inaccessibles à
    # l'UID ÉPHÉMÈRE que `DynamicUser=yes` crée à chaque démarrage — CHDIR
    # échouait avant la moindre ligne de JavaScript (voir
    # hooks/install.py::rendre_lisible_par_tous). Ce test verifie que TOUT
    # repertoire sous opt/nivuus/desk (le PARENT compris) est traversable
    # par "autre" — le bit precis que DynamicUser exige, distinct du mode
    # LECTURE SEULE que ProtectSystem=strict impose par ailleurs.
    desk_dep = root1 / "opt" / "nivuus" / "desk"
    repertoires_non_traversables = [
        d for d, _dn, _fn in os.walk(desk_dep)
        if (os.stat(d).st_mode & 0o005) != 0o005
    ]
    check("tout repertoire sous opt/nivuus/desk est o+rx (DynamicUser)",
          repertoires_non_traversables, [])

    # --- L'unite systemd ----------------------------------------------------
    unite = root1 / "etc" / "systemd" / "system" / "desk-plateforme.service"
    check("l'unite est deposee", unite.is_file(), True)
    ini = load_unit(unite)
    # 🔴 PROBLEME A DU LOT 10A : l'unite livree portait "/usr/bin/npm start",
    # un chemin qui n'existe sur AUCUNE Debian sans paquet nodejs (verifie le
    # 29 aout 2026 sur cette machine). Le defaut attendu est desormais
    # NODE_BIN_DEFAUT/npm — voir commun.py::lire_node_bin.
    check("l'unite lance npm depuis NODE_BIN_DEFAUT (jamais /usr/bin/npm)",
          ini.get("Service", "ExecStart", fallback=""),
          f"{NODE_BIN_DEFAUT}/npm start")
    check("l'unite pose un PATH qui contient NODE_BIN_DEFAUT (npm est un "
          "script #!/usr/bin/env node, il doit retrouver node)",
          NODE_BIN_DEFAUT in ini.get("Service", "Environment", fallback=""),
          True)
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

    # 🔴 LA REGRESSION LA PLUS IMPORTANTE DE CE FICHIER — SANS FACTS, C'EST
    # LE CHEMIN REELLEMENT EMPRUNTE PAR LE MOTEUR (« install NE reçoit PAS
    # les facts de resolve », voir le docstring de tête d'install.py). Avant
    # le lot 10A (29 août 2026), ce chemin dérivait PLATEFORME_HOTE par
    # `deriver_adresse_hote()` = l'interface de la route IPv4 PAR DÉFAUT —
    # qui, sur CETTE machine, mesurée avec /usr/bin/ip hors de tout alias de
    # shell, est `ppp0` (PPPoE), une adresse PUBLIQUE (90.87.35.18).
    # `install.py` aurait donc posé PLATEFORME_HOTE=90.87.35.18 lors d'une
    # VRAIE installation sans facts sur CET hôte — exposant le bureau
    # distant sur l'internet public sans Pomerium devant lui. Ce contrôle
    # n'existait PAS avant ce lot (l'ancien ne vérifiait que « non vide,
    # jamais universelle » — 90.87.35.18 n'est ni vide ni universelle, donc
    # passait sans rien détecter). Il DOIT désormais valoir le défaut FIXE.
    check("sans facts : PLATEFORME_HOTE vaut le defaut FIXE, jamais la "
          "route par defaut (regression du 29 aout 2026, voir commun.py)",
          env2.get("PLATEFORME_HOTE"), HOTE_DEFAUT)
    check("sans facts : PLATEFORME_PROXY_DE_CONFIANCE vaut le defaut",
          env2.get("PLATEFORME_PROXY_DE_CONFIANCE"), PROXY_DEFAUT)

    # 🔴 Deux installations distinctes NE PARTAGENT PAS le secret.
    check("deux installations ne partagent pas PLATEFORME_SECRET_JETON",
          env1["PLATEFORME_SECRET_JETON"] == env2["PLATEFORME_SECRET_JETON"],
          False)
    check("deux installations ne partagent pas TURN_SECRET",
          env1["TURN_SECRET"] == env2["TURN_SECRET"], False)

# --- Installation 3 : DESK_NODE_BIN configure le chemin de l'interprete -----
# 🔴 PROBLEME A DU LOT 10A : prouve que le chemin est bien CONFIGURABLE
# (jamais un second /usr/bin/npm code en dur ailleurs), avec son propre
# temoin negatif — le defaut de l'installation 1/2 ci-dessus, DIFFERENT de
# cette valeur.
with tempfile.TemporaryDirectory() as tmp3:
    root3 = pathlib.Path(tmp3)
    env_override = dict(os.environ)
    env_override["DESK_NODE_BIN"] = "/opt/nivuus-test/node/bin"
    r3 = appeler(root3, facts=FACTS, env=env_override)
    check("installation 3 (DESK_NODE_BIN) : code de sortie 0", r3.returncode, 0)
    unite3 = root3 / "etc" / "systemd" / "system" / "desk-plateforme.service"
    ini3 = load_unit(unite3)
    check("DESK_NODE_BIN change bien ExecStart",
          ini3.get("Service", "ExecStart", fallback=""),
          "/opt/nivuus-test/node/bin/npm start")
    check("DESK_NODE_BIN change bien le PATH pose",
          "/opt/nivuus-test/node/bin" in ini3.get("Service", "Environment", fallback=""),
          True)
    check("DESK_NODE_BIN : ExecStart n'est PLUS le defaut (temoin negatif)",
          ini3.get("Service", "ExecStart", fallback="") == f"{NODE_BIN_DEFAUT}/npm start",
          False)

# --- Installation 4 : node_modules absent cote SOURCE -> refus -------------
# 🔴 PROBLEME B DU LOT 10A : `npm start` exige `node_modules/.bin/tsx`, et
# rien ne le garantissait avant ce lot — un depot fraichement clone (ou
# empaquete par une pipeline qui n'a jamais lance `npm install`) aurait vu
# `install` "reussir" (code 0) tout en posant un service structurellement
# incapable de demarrer. Ce scenario fabrique une racine SOURCE factice
# (DESK_SOURCE_RACINE) portant un `plateforme/` SANS node_modules, et
# verifie que `install` REFUSE plutot que de laisser passer.
with tempfile.TemporaryDirectory() as tmp4src, tempfile.TemporaryDirectory() as tmp4dst:
    source4 = pathlib.Path(tmp4src)
    poser_source_minimale(source4)
    # Delibrement AUCUN node_modules sous source4/plateforme.

    root4 = pathlib.Path(tmp4dst)
    env_source = dict(os.environ)
    env_source["DESK_SOURCE_RACINE"] = str(source4)
    r4 = appeler(root4, facts=FACTS, env=env_source)
    check("installation 4 (node_modules absent) : code de sortie NON NUL",
          r4.returncode != 0, True)
    check("installation 4 : le refus nomme node_modules/.bin/tsx",
          "node_modules" in (r4.stderr or "") and "tsx" in (r4.stderr or ""),
          True)

    # --- Temoin negatif : la MEME racine source, AVEC node_modules/.bin/tsx,
    # doit reussir --------------------------------------------------------
    bin_dir4 = source4 / "plateforme" / "node_modules" / ".bin"
    bin_dir4.mkdir(parents=True)
    (bin_dir4 / "tsx").write_text("#!/usr/bin/env node\n", encoding="utf-8")
    (bin_dir4 / "tsx").chmod(0o755)
    root4b = pathlib.Path(tempfile.mkdtemp())
    try:
        r4b = appeler(root4b, facts=FACTS, env=env_source)
        check("temoin negatif : node_modules/.bin/tsx present -> code 0",
              r4b.returncode, 0)
    finally:
        import shutil as _shutil
        _shutil.rmtree(root4b, ignore_errors=True)

# --- Installation 6 : facts["hote"] universel DOIT ETRE REFUSE -------------
# 🔴 RONDE DE CORRECTION 1 (29 août 2026) : la revue a démontré que
# `facts.get("hote")`, quand il est VRAI, court-circuitait `lire_hote()` —
# la SEULE fonction qui vérifiait `ECOUTES_UNIVERSELLES` — et traversait
# donc SANS AUCUN CONTRÔLE. `facts = {..., "hote": "0.0.0.0", ...}` rendait
# code 0 et écrivait PLATEFORME_HOTE=0.0.0.0 dans desk.env : exactement le
# défaut que ce lot existe pour fermer, revenu par un second chemin.
# Inatteignable par le moteur réel AUJOURD'HUI (il ne passe aucun `facts` à
# `install`), mais le docstring de tête d'install.py dit lui-même que ce
# canal existe pour « un futur moteur, ou ce fichier de tests » — et le
# contrat de `facts` a gagné des clés PENDANT ce lot même. Ce scénario fige
# le contrat : une écoute universelle dans facts["hote"] DOIT refuser,
# exactement comme DESK_HOTE=0.0.0.0 le fait déjà pour la valeur dérivée.
with tempfile.TemporaryDirectory() as tmp6:
    root6 = pathlib.Path(tmp6)
    facts_hote_universel = dict(FACTS)
    facts_hote_universel["hote"] = "0.0.0.0"
    r6 = appeler(root6, facts=facts_hote_universel)
    check("facts['hote']='0.0.0.0' : code de sortie NON NUL (refus)",
          r6.returncode != 0, True)
    check("facts['hote']='0.0.0.0' : le refus nomme l'ecoute universelle",
          "universelle" in (r6.stderr or "").lower(), True)
    check("facts['hote']='0.0.0.0' : le refus nomme facts[\"hote\"] (pas DESK_HOTE)",
          'facts["hote"]' in (r6.stderr or ""), True)
    check("facts['hote']='0.0.0.0' : desk.env n'est PAS ecrit avec cette valeur",
          (root6 / "etc" / "nivuus" / "desk.env").is_file(), False)

# --- Installation 5 : REJOUER install DEUX FOIS SUR LA MÊME RACINE ---------
# 🔴 BUG RÉEL TROUVÉ AU LOT 10A (29 août 2026) : une réinstallation réelle
# sur `--root /`, avec un `plateforme/node_modules/.bin/` qui porte des
# symlinks (tsx, vite, tsc, …), levait `shutil.Error` — `dirs_exist_ok=True`
# ne couvre que les RÉPERTOIRES, jamais les liens qu'ils contiennent. Aucun
# scénario précédent de ce fichier ne rejouait install DEUX FOIS sur la
# MÊME racine : c'est exactement le patron « un contrôle qu'on n'a jamais
# vu rouge n'est pas un contrôle ». Ce scénario fabrique une racine SOURCE
# minimale portant un VRAI symlink sous node_modules/.bin (reproduisant le
# cas réel), installe deux fois de suite sur la MÊME racine cible, et
# vérifie que la SECONDE installation réussit aussi.
with tempfile.TemporaryDirectory() as tmp5src, tempfile.TemporaryDirectory() as tmp5dst:
    source5 = pathlib.Path(tmp5src)
    poser_source_minimale(source5)
    (source5 / "plateforme" / "node_modules" / "un-paquet").mkdir(parents=True)
    (source5 / "plateforme" / "node_modules" / "un-paquet" / "cli.js").write_text(
        "#!/usr/bin/env node\n", encoding="utf-8")
    bin_dir5 = source5 / "plateforme" / "node_modules" / ".bin"
    bin_dir5.mkdir(parents=True)
    (bin_dir5 / "tsx").symlink_to("../un-paquet/cli.js")

    root5 = pathlib.Path(tmp5dst)
    env_source5 = dict(os.environ)
    env_source5["DESK_SOURCE_RACINE"] = str(source5)

    r5a = appeler(root5, facts=FACTS, env=env_source5)
    check("premiere installation (avec symlink) : code de sortie 0",
          r5a.returncode, 0)
    if r5a.returncode != 0:
        failures.append(f"stderr premiere installation : {r5a.stderr!r}")

    r5b = appeler(root5, facts=FACTS, env=env_source5)
    check("seconde installation SUR LA MEME RACINE : code de sortie 0 "
          "(regression du 29 aout 2026 : shutil.Error sur les symlinks "
          "de node_modules/.bin)", r5b.returncode, 0)
    if r5b.returncode != 0:
        failures.append(f"stderr seconde installation : {r5b.stderr!r}")

    lien5 = root5 / "opt" / "nivuus" / "desk" / "plateforme" / "node_modules" / ".bin" / "tsx"
    check("le symlink survit a la reinstallation, toujours un lien",
          lien5.is_symlink(), True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook install passés")
