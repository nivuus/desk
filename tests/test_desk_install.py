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
# ⚠️ `configparser`, `json`, `subprocess` et `HOOK` ont suivi les fixtures
# dans `desk_install_fixtures.py` : une extraction n'est jamais rigoureusement
# verbatim, elle laisse ses imports derrière elle — et un import inutilisé est
# une famille d'échec, pas un détail.
import os
import pathlib
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]

sys.path.insert(0, str(RACINE / "hooks"))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from commun import HOTE_DEFAUT, NODE_BIN_DEFAUT, PROXY_DEFAUT  # noqa: E402
from fichiers_installes import lire_secret_persiste  # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


from desk_install_fixtures import (  # noqa: E402
    FACTS,
    appeler,
    lire_env,
    load_unit,
    poser_source_minimale,
)

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

# --- Installation 7 : LE RUNTIME NODE EST DÉPOSÉ, PAS SUPPOSÉ -------------
# 🔴 IMPORTANTE DE LA REVUE FINALE DE BRANCHE (30 août 2026) : `commun.py::
# NODE_BIN_DEFAUT` désigne `/opt/nivuus/node/bin`, et AUCUN hook n'y déposait
# quoi que ce soit — l'arbre présent sur la machine de développement y avait
# été copié À LA MAIN pendant le lot 10A, par une commande qui ne vivait que
# dans un rapport gitignoré. Une installation neuve posait donc un service
# structurellement incapable de démarrer, sans un mot.
# Ce scénario emploie le VRAI runtime de cette machine (`node_source=False`),
# c'est-à-dire le chemin de production : un préfixe dérivé de
# `process.execPath`, ses vrais liens relatifs, ses ~144 Mio. Les autres
# scénarios emploient un préfixe factice — ils n'ont pas à repayer la copie.
with tempfile.TemporaryDirectory() as tmp7:
    root7 = pathlib.Path(tmp7)
    r7 = appeler(root7, facts=FACTS, node_source=False)
    check("installation 7 (vrai runtime) : code de sortie 0", r7.returncode, 0)
    if r7.returncode != 0:
        failures.append(f"stderr installation 7 : {r7.stderr!r}")
    node_dep = root7 / NODE_BIN_DEFAUT.lstrip("/")
    check("bin/node est depose", (node_dep / "node").is_file(), True)
    check("bin/npm est depose", (node_dep / "npm").exists(), True)
    # 🔴 `npm` DOIT RESTER UN LIEN : sa cible est RELATIVE et pointe dans
    # l'arbre deposé. Le suivre deposerait une COPIE de npm-cli.js sous un
    # nom qui pretend etre npm, et `npm` cesserait de retrouver ses modules.
    check("bin/npm est un LIEN, jamais une copie suivie",
          (node_dep / "npm").is_symlink(), True)
    check("le paquet global npm est depose",
          (node_dep.parent / "lib" / "node_modules" / "npm").is_dir(), True)
    # 🔴 `DynamicUser=yes` fait tourner le service sous un UID ephemere : un
    # `bin/node` en rwxr-x--- ferait echouer ExecStart avant la premiere
    # ligne de JavaScript (bug reel du lot 10A sur /opt/nivuus/desk).
    check("bin/node est executable par autrui (DynamicUser)",
          bool(os.stat(node_dep / "node").st_mode & 0o001), True)
    # Temoin negatif : l'unite deposee pointe bien sur CE chemin-la.
    ini7 = load_unit(root7 / "etc" / "systemd" / "system" / "desk-plateforme.service")
    check("ExecStart pointe sur le npm reellement depose",
          ini7.get("Service", "ExecStart", fallback=""),
          f"{NODE_BIN_DEFAUT}/npm start")

# --- lire_secret_persiste : les cinq cas au niveau unite -------------------
# 🔴 BUG RÉEL TROUVÉ ET CORRIGÉ LE 2026-09-08 : `install.py` tirait
# `PLATEFORME_SECRET_JETON` et `TURN_SECRET` SANS CONDITION à chaque appel
# (`ecrire_secret()` deux fois, jamais de relecture), en contradiction avec
# le docstring d'`ecrire_secret` qui promettait « tiré une seule fois,
# jamais recalculé » — l'idempotence gate du plan de release a détecté la
# non-idempotence (etc/nivuus/desk.env et etc/turnserver.conf changent entre
# deux passes). `lire_secret_persiste` (hooks/fichiers_installes.py) est la
# fonction qui porte désormais réellement cet invariant ; ces cinq
# scénarios éprouvent CHAQUE cas, séparément d'une installation complète,
# parce qu'un seul hook subprocess ne peut pas facilement distinguer "clé
# absente" de "valeur vide" de "fichier absent" dans une seule assertion
# lisible.
with tempfile.TemporaryDirectory() as tmp_ls:
    dossier_ls = pathlib.Path(tmp_ls)

    # Cas 1 : le fichier n'existe pas du tout (premier install).
    absent = dossier_ls / "n-existe-pas.env"
    check("lire_secret_persiste : fichier absent -> None",
          lire_secret_persiste(absent, "PLATEFORME_SECRET_JETON"), None)

    # Cas 2 : le fichier existe mais ne porte pas la cle demandee (une
    # installation anterieure d'une version qui n'ecrivait pas encore
    # cette cle).
    sans_cle = dossier_ls / "sans-cle.env"
    sans_cle.write_text("AUTRE_CLE=une-valeur\n", encoding="utf-8")
    check("lire_secret_persiste : cle absente du fichier -> None",
          lire_secret_persiste(sans_cle, "PLATEFORME_SECRET_JETON"), None)

    # Cas 3 : la cle est presente mais sa valeur est vide, ou faite
    # uniquement d'espaces (fichier tronque ou modifie a la main) — les
    # DEUX formes comptent comme "rien a reutiliser".
    valeur_vide = dossier_ls / "valeur-vide.env"
    valeur_vide.write_text(
        "PLATEFORME_SECRET_JETON=\nTURN_SECRET=   \n", encoding="utf-8")
    check("lire_secret_persiste : valeur vide -> None",
          lire_secret_persiste(valeur_vide, "PLATEFORME_SECRET_JETON"), None)
    check("lire_secret_persiste : valeur faite d'espaces -> None",
          lire_secret_persiste(valeur_vide, "TURN_SECRET"), None)

    # Cas 4 : la cle est presente avec une valeur utilisable -> reutilisee
    # telle quelle.
    valeur_reelle = dossier_ls / "valeur-reelle.env"
    valeur_reelle.write_text(
        "PLATEFORME_SECRET_JETON=abc123\nAUTRE=x\n", encoding="utf-8")
    check("lire_secret_persiste : valeur presente -> reutilisee telle quelle",
          lire_secret_persiste(valeur_reelle, "PLATEFORME_SECRET_JETON"),
          "abc123")

    # Cas 5 : le fichier EXISTE mais sa LECTURE echoue (permissions faussees
    # par une migration partielle, erreur disque, ...) — PAS "absent", donc
    # PAS "rien a reutiliser". 🔴 REVUE DU 2026-09-08 : un `except OSError:
    # return None` trop large avalait CE cas exactement comme le cas 1, et
    # aurait fait tirer un secret NEUF EN SILENCE — la meme rotation
    # silencieuse que le bug d'origine, par une porte plus etroite. Cette
    # fonction doit LEVER, jamais rendre None, pour que install.py puisse
    # refuser au lieu d'halluciner un secret.
    #
    # Un DOUBLE minimal plutot qu'un vrai fichier chmod'e : ces suites
    # tournent en root sur cette machine, qui outrepasse les permissions
    # POSIX — un vrai `chmod 000` ne produirait donc PAS de PermissionError
    # ici, et le scenario resterait vert par accident. `CheminIllisible`
    # n'imite QUE les deux methodes que `lire_secret_persiste` emploie,
    # dans le meme esprit que les autres doubles factices de ce dossier
    # (`poser_faux_node_source`, etc.) : is_file() dit "present", read_text()
    # leve — exactement la forme d'un fichier reel mais illisible.
    class CheminIllisible:
        def is_file(self):
            return True

        def read_text(self, encoding="utf-8"):
            raise PermissionError(
                "permission refusee (factice, cas 5 de ce scenario)")

    illisible = CheminIllisible()
    try:
        valeur_obtenue = lire_secret_persiste(illisible, "PLATEFORME_SECRET_JETON")
        failures.append(
            "lire_secret_persiste : fichier illisible aurait du LEVER une "
            f"OSError, a rendu {valeur_obtenue!r} sans lever (secret neuf "
            "tire en silence si ceci arrivait dans install.py)")
    except FileNotFoundError:
        failures.append(
            "lire_secret_persiste : fichier illisible ne doit PAS etre "
            "confondu avec FileNotFoundError (le fichier EST present)")
    except OSError:
        pass  # attendu : la panne de lecture se propage.

# --- Installation 8 : REJOUER install PROUVE la reutilisation des secrets --
# La propriete que le bug du 2026-09-08 violait, eprouvee de bout en bout
# par le VRAI hook (subprocess complet), pas seulement par
# lire_secret_persiste en isolation : un premier install sur une racine
# VIDE produit bien les deux secrets (rien a reutiliser encore), et un
# SECOND install sur la MEME racine les laisse INCHANGES.
with tempfile.TemporaryDirectory() as tmp8:
    root8 = pathlib.Path(tmp8)

    r8a = appeler(root8, facts=FACTS)
    check("installation 8, premiere passe : code de sortie 0",
          r8a.returncode, 0)
    if r8a.returncode != 0:
        failures.append(f"stderr installation 8 (1ere passe) : {r8a.stderr!r}")
    env8a, _chemin_env8a = lire_env(root8)
    turnconf8 = root8 / "etc" / "turnserver.conf"

    # Premiere passe sur une racine vide : rien a reutiliser -> les deux
    # secrets SONT produits (le cas que le bug ne cassait pas).
    check("premiere passe : PLATEFORME_SECRET_JETON est produit (>= 32 car.)",
          len(env8a.get("PLATEFORME_SECRET_JETON", "")) >= 32, True)
    check("premiere passe : TURN_SECRET est produit (>= 32 car.)",
          len(env8a.get("TURN_SECRET", "")) >= 32, True)

    r8b = appeler(root8, facts=FACTS)
    check("installation 8, seconde passe SUR LA MEME RACINE : code de sortie 0",
          r8b.returncode, 0)
    if r8b.returncode != 0:
        failures.append(f"stderr installation 8 (2e passe) : {r8b.stderr!r}")
    env8b, _ = lire_env(root8)

    # 🔴 LA REGRESSION QUE CE SCENARIO GARDE : avant le correctif, ces deux
    # egalites echouaient a chaque replay (secrets.token_hex(32) tire deux
    # valeurs differentes avec une probabilite ecrasante).
    check("PLATEFORME_SECRET_JETON est REUTILISE, jamais redessine au replay",
          env8b.get("PLATEFORME_SECRET_JETON"),
          env8a.get("PLATEFORME_SECRET_JETON"))
    check("TURN_SECRET est REUTILISE, jamais redessine au replay",
          env8b.get("TURN_SECRET"), env8a.get("TURN_SECRET"))

    # turnserver.conf reste coherent avec TURN_SECRET apres le replay
    # (les deux ecrivains doivent toujours s'accorder, replay ou non).
    contenu_turn8 = turnconf8.read_text(encoding="utf-8")
    check("turnserver.conf porte TOUJOURS le meme secret que TURN_SECRET "
          "apres un replay",
          f"static-auth-secret={env8b['TURN_SECRET']}" in contenu_turn8, True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook install passés")
