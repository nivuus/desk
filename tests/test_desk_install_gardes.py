#!/usr/bin/env python3
"""Les GARDES du hook install : ce qu'il REFUSE, et ce qu'il n'écrit pas.

Scindé de `tests/test_desk_install.py` le 30 août 2026, dans un commit DÉDIÉ
et AVANT que la revue finale de branche n'y ajoute ses scénarios (le pré-vol
qui refuse avant tout secret, le runtime Node introuvable, les `facts`
malformés) : le fichier des scénarios était à 392 lignes sur 500 et les
additions le portaient à 523. EXTRAIRE, JAMAIS COMPRIMER.

🔴 LA RÈGLE DE SÉLECTION, ÉNONCÉE PLUTÔT QUE SUBIE : ce fichier porte les
scénarios dont l'issue attendue est un REFUS (code de sortie non nul, une
phrase sur stderr, aucune trace Python) ; `test_desk_install.py` porte ceux
dont l'issue attendue est une installation qui ABOUTIT. Un scénario de refus
neuf va ici, un scénario d'aboutissement va là-bas.

Les fixtures sont partagées : `tests/desk_install_fixtures.py`.

Run: python3 tests/test_desk_install_gardes.py
"""
import os
import pathlib
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RACINE / "hooks"))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from desk_install_fixtures import (  # noqa: E402
    FACTS,
    appeler,
    poser_source_minimale,
)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


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


# --- Installation 8 : LE PRÉ-VOL REFUSE AVANT LE PREMIER SECRET -----------
# 🔴 IMPORTANTE DE LA REVUE FINALE DE BRANCHE, DÉMONTRÉE PAR EXÉCUTION.
# `client/dist/` est gitignoré : un dépôt fraîchement cloné n'en porte pas.
# `install.py` copiait `client/dist` AVANT d'atteindre son garde `tsx`, si
# bien qu'il rendait une TRACE PYTHON — et qu'il la rendait APRÈS avoir déjà
# écrit `desk.env` AVEC SES DEUX SECRETS. Ce scénario fige les trois
# propriétés du remède : une phrase, pas une trace ; la liste ENTIÈRE de ce
# qui manque, pas le premier manque ; et `desk.env` JAMAIS créé.
with tempfile.TemporaryDirectory() as tmp8src, tempfile.TemporaryDirectory() as tmp8:
    source8 = pathlib.Path(tmp8src)
    poser_source_minimale(source8)
    # `poser_source_minimale` pose client/dist et proto/ts ; on retire
    # client/dist pour reproduire le depot frais, et node_modules/.bin/tsx
    # n'a jamais ete pose.
    import shutil as _sh
    _sh.rmtree(source8 / "client" / "dist")

    root8 = pathlib.Path(tmp8)
    env8 = dict(os.environ)
    env8["DESK_SOURCE_RACINE"] = str(source8)
    r8 = appeler(root8, facts=FACTS, env=env8)
    check("installation 8 (depot frais) : code de sortie NON NUL",
          r8.returncode != 0, True)
    check("installation 8 : aucune trace Python",
          "Traceback" in (r8.stderr or ""), False)
    check("installation 8 : le refus nomme client/dist",
          "client/dist" in (r8.stderr or ""), True)
    check("installation 8 : le refus nomme AUSSI node_modules/.bin/tsx "
          "(la liste entiere, jamais le premier manque)",
          "node_modules/.bin/tsx" in (r8.stderr or ""), True)
    check("installation 8 : le refus invite a lancer npm run build",
          "npm run build" in (r8.stderr or ""), True)
    # 🔴 LE CONTROLE QUI VAUT : aucun secret n'a ete tire.
    check("installation 8 : desk.env n'est PAS cree (aucun secret orphelin)",
          (root8 / "etc" / "nivuus" / "desk.env").exists(), False)
    check("installation 8 : turnserver.conf non plus",
          (root8 / "etc" / "turnserver.conf").exists(), False)

# --- Installation 9 : LE RUNTIME NODE INTROUVABLE EST UN REFUS NOMMÉ ------
# Le pendant du scénario 7 : quand le préfixe Node ne porte pas ce qu'il
# faut, le hook le DIT (comme il le dit déjà pour `tsx` et pour `proto/ts`),
# il ne pose pas un service muet. Le garde était posé sur deux des trois
# conditions de démarrage ; c'est la troisième.
with tempfile.TemporaryDirectory() as tmp9:
    root9 = pathlib.Path(tmp9)
    prefixe_vide = root9 / "node-incomplet"
    (prefixe_vide / "bin").mkdir(parents=True)
    r9 = appeler(root9, facts=FACTS, node_source=str(prefixe_vide))
    check("installation 9 (runtime Node incomplet) : code de sortie NON NUL",
          r9.returncode != 0, True)
    check("installation 9 : aucune trace Python",
          "Traceback" in (r9.stderr or ""), False)
    check("installation 9 : le refus nomme le runtime incomplet",
          "incomplet" in (r9.stderr or ""), True)
    check("installation 9 : le refus nomme bin/node",
          "bin/node" in (r9.stderr or ""), True)
    check("installation 9 : desk.env n'est PAS cree",
          (root9 / "etc" / "nivuus" / "desk.env").exists(), False)

# --- Installation 10 : les facts NE SONT PLUS CRUS SUR PAROLE -------------
# 🔴 MINEURE #7 DE LA TÂCHE 4, RENVERSÉE PAR LA REVUE FINALE DE BRANCHE : sa
# raison (« `facts` vient de nous, on ne s'en défend pas ») a été RÉFUTÉE
# dans cette branche même, sur `facts["hote"]` — voir l'installation 6. Les
# clés voisines gardaient pourtant la même raison écrite.
for etiquette, cle, valeur, attendu in [
    ("turn_ecoute universelle", "turn_ecoute", "0.0.0.0", "universelle"),
    ("turn_relais universelle", "turn_relais", "::", "universelle"),
    ("proxy_confiance universelle", "proxy_confiance", "*", "universelle"),
    ("port non entier", "port", "trois-mille", "n'est pas un entier"),
    ("port hors plage", "port", 70000, "hors de la plage"),
    ("port booleen", "port", True, "booleen"),
]:
    with tempfile.TemporaryDirectory() as tmp10:
        root10 = pathlib.Path(tmp10)
        facts10 = dict(FACTS)
        facts10[cle] = valeur
        r10 = appeler(root10, facts=facts10)
        check(f"facts {etiquette} : code de sortie NON NUL", r10.returncode != 0, True)
        check(f"facts {etiquette} : aucune trace Python",
              "Traceback" in (r10.stderr or ""), False)
        check(f"facts {etiquette} : le refus nomme la cause",
              attendu in (r10.stderr or "").lower().replace("é", "e"), True)
        check(f"facts {etiquette} : desk.env n'est PAS ecrit",
              (root10 / "etc" / "nivuus" / "desk.env").exists(), False)

# Témoin négatif de l'installation 10 : les MÊMES clés, avec des valeurs
# saines, passent — sans quoi les six refus ci-dessus seraient rendus par un
# hook qui refuse tout.
with tempfile.TemporaryDirectory() as tmp10b:
    root10b = pathlib.Path(tmp10b)
    r10b = appeler(root10b, facts=FACTS)
    check("temoin negatif : des facts sains passent toujours", r10b.returncode, 0)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - gardes du hook install passés")
