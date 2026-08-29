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


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - gardes du hook install passés")
