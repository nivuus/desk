#!/usr/bin/env python3
"""Le contrat de `hw` : ce que le MOTEUR envoie, jamais ce qu'un test invente.

🔴 CETTE SUITE EXISTE À CAUSE D'UNE CRITIQUE DE LA REVUE FINALE DE BRANCHE
(30 août 2026). `hooks/resolve.py` refusait sur `hw.get("vm_windows")` —
une clé qu'AUCUN producteur du moteur ne pose. Le package ne pouvait donc
pas s'installer : le refus devient un `StepError` côté moteur
(`installer/installer/install-engine/steps/packages.py`), qui arrête
l'installation ENTIÈRE.

**Les huit suites existantes ne pouvaient pas le voir, et c'est la leçon** :
elles FABRIQUENT le contexte qu'elles envoient (`{"hw": {"vm_windows":
True}}`), donc elles produisaient elles-mêmes le fait dont elles vérifiaient
la consommation. Un test qui invente son entrée ne peut jamais découvrir que
personne ne la produit.

Ce que cette suite fige, et que rien d'autre ne fige :

  ① l'ensemble des clés que `installer/installer/common/hardware.py::
     detect_all()` RETOURNE réellement — lu dans le dépôt voisin, par
     analyse syntaxique (`ast`), jamais exécuté (la détection réelle
     invoque `lsblk`, `lspci`… : on veut le CONTRAT, pas la machine) ;
  ② que `hooks/resolve.py` ne lit AUCUNE clé de `hw` absente de cet
     ensemble — c'est exactement la Critique, figée ;
  ③ que `hooks/activate.py` ne lit aucune clé de `hw` absente de cet
     ensemble ÉLARGI aux `facts` que `resolve` émet lui-même : le moteur
     les fusionne dans `hw` avant d'appeler `activate`
     (`installer/installer/packages/runner.py::run_activate`,
     `merge_into_hw`) ;
  ④ qu'un `resolve` nourri du contexte que le MOTEUR enverrait — un `hw`
     portant exactement les clés de `detect_all()`, et rien de plus —
     n'émet AUCUN refus ;
  ⑤ qu'AUCUN fait émis par `resolve` n'est un LITTÉRAL codé en dur dans le
     dict d'émission — c'est le TROU que ③ laissait ouvert, sous un nom
     voisin de la Critique : ③ autorise `activate` à lire toute clé
     PRÉSENTE dans `CLES_FACTS`, mais ne dit RIEN sur la façon dont cette
     clé a été obtenue. Un fait posé en dur (`"vm_repond": True`) est une
     clé de `CLES_FACTS` comme une autre, et ③ le laisserait donc passer
     tel quel — c'est exactement la forme du défaut réel trouvé le 30 août
     2026 (`hooks/resolve.py:393`, avant correction), et le motif pour
     lequel la Critique ci-dessus a pu se reproduire sous un nom différent
     malgré ③ déjà en place.

⚠️ SI LE DÉPÔT VOISIN EST ABSENT, CETTE SUITE ÉCHOUE, elle ne se saute pas :
un contrat qu'on ne peut pas vérifier n'est pas un contrat vérifié, et « un
`||` de repli transforme fichier absent en contrôle vert ». Le chemin est
surchargeable par `DESK_INSTALLER_RACINE` — même convention que
`tests/test_desk_manifeste.py`, qui importe déjà le moteur voisin.

Run: python3 tests/test_desk_contrat_hw.py
"""
import ast
import json
import os
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
INSTALLER = pathlib.Path(os.environ.get("DESK_INSTALLER_RACINE")
                          or (RACINE.parent / "installer"))
HARDWARE = INSTALLER / "installer" / "common" / "hardware.py"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# --- ① Les clés que le moteur produit RÉELLEMENT --------------------------

def cles_de_detect_all(chemin: pathlib.Path) -> set:
    """Les clés du dict littéral que `detect_all()` retourne.

    Lu par `ast`, jamais exécuté : `detect_all()` shelle vers `lsblk`,
    `lspci`, `/proc/meminfo`… — on veut la FORME du contrat, pas l'état de
    la machine qui fait tourner ces tests.
    """
    arbre = ast.parse(chemin.read_text(encoding="utf-8"))
    for noeud in ast.walk(arbre):
        if isinstance(noeud, ast.FunctionDef) and noeud.name == "detect_all":
            for interne in ast.walk(noeud):
                if isinstance(interne, ast.Return) and isinstance(interne.value, ast.Dict):
                    return {cle.value for cle in interne.value.keys
                            if isinstance(cle, ast.Constant)}
    return set()


if not HARDWARE.is_file():
    failures.append(
        f"le producteur de `hw` est introuvable : {HARDWARE} — cette suite "
        "ne peut PAS vérifier le contrat sans lui, et un contrôle qui se "
        "saute quand sa pièce manque est un contrôle vert par accident. "
        "Surcharger DESK_INSTALLER_RACINE si le dépôt voisin vit ailleurs."
    )
    CLES_MOTEUR = set()
else:
    CLES_MOTEUR = cles_de_detect_all(HARDWARE)

check("detect_all() rend un ensemble de clés non vide", bool(CLES_MOTEUR), True)


# --- Ce qu'un hook LIT dans `hw`, et ce que `resolve` ÉMET en facts -------

def cles_lues_dans(chemin: pathlib.Path, nom_variable: str) -> set:
    """Les clés littérales lues sur `<nom_variable>` — `x.get("k")` et
    `x["k"]` — dans le fichier donné."""
    arbre = ast.parse(chemin.read_text(encoding="utf-8"))
    cles = set()
    for noeud in ast.walk(arbre):
        if (isinstance(noeud, ast.Call)
                and isinstance(noeud.func, ast.Attribute)
                and noeud.func.attr == "get"
                and isinstance(noeud.func.value, ast.Name)
                and noeud.func.value.id == nom_variable
                and noeud.args and isinstance(noeud.args[0], ast.Constant)
                and isinstance(noeud.args[0].value, str)):
            cles.add(noeud.args[0].value)
        if (isinstance(noeud, ast.Subscript)
                and isinstance(noeud.value, ast.Name)
                and noeud.value.id == nom_variable
                and isinstance(noeud.slice, ast.Constant)
                and isinstance(noeud.slice.value, str)):
            cles.add(noeud.slice.value)
    return cles


def cles_des_facts(chemin: pathlib.Path) -> set:
    """Les clés du dict `facts` que `resolve.py` émet."""
    arbre = ast.parse(chemin.read_text(encoding="utf-8"))
    for noeud in ast.walk(arbre):
        if not isinstance(noeud, ast.Dict):
            continue
        for cle, valeur in zip(noeud.keys, noeud.values):
            if (isinstance(cle, ast.Constant) and cle.value == "facts"
                    and isinstance(valeur, ast.Dict)):
                return {k.value for k in valeur.keys
                        if isinstance(k, ast.Constant)}
    return set()


CLES_FACTS = cles_des_facts(RACINE / "hooks" / "resolve.py")
check("resolve.py émet bien un dict `facts` non vide", bool(CLES_FACTS), True)


# --- ② `resolve` ne lit que ce que `detect_all()` produit -----------------
# 🔴 C'EST LA CRITIQUE, FIGÉE. Avant le 30 août 2026, cette assertion
# aurait rendu {'vm_windows'} — une clé introuvable dans TOUT le dépôt
# voisin (`grep -rn vm_windows ../installer/` : aucune sortie).
lues_resolve = cles_lues_dans(RACINE / "hooks" / "resolve.py", "hw")
check("resolve ne lit dans hw aucune clé que detect_all() ne produit pas",
      sorted(lues_resolve - CLES_MOTEUR), [])


# --- ③ `activate` : detect_all() ÉLARGI aux facts de resolve --------------
# Le moteur fusionne les facts DANS hw avant d'appeler activate
# (runner.py::run_activate -> merge_into_hw), donc une clé de facts y est
# légitime — et une clé qui n'est NI dans detect_all() NI dans facts ne
# peut venir de nulle part.
lues_activate = cles_lues_dans(RACINE / "hooks" / "activate.py", "hw")
check("activate ne lit dans hw que detect_all() ou les facts de resolve",
      sorted(lues_activate - (CLES_MOTEUR | CLES_FACTS)), [])


# --- ④ Le contexte que le MOTEUR envoie ne fait refuser personne ----------
# 🔴 CE N'EST PAS UN CONTEXTE FABRIQUÉ : ses clés sont exactement celles que
# `detect_all()` retourne, relevées ci-dessus. Les VALEURS sont vides (aucun
# disque, aucun GPU…) — c'est le pire cas, et c'est celui qu'une machine
# neuve où rien n'est encore partitionné ressemble le plus.
contexte_moteur = {cle: [] for cle in sorted(CLES_MOTEUR)}
REPONSES = {"admin_email": "a@b.c", "admin_password": "x",
            "auth_mode": "motdepasse", "vb_audio": False}
proc = subprocess.run(
    [sys.executable, str(RACINE / "hooks" / "resolve.py")],
    input=json.dumps({"hw": contexte_moteur, "answers": REPONSES}),
    capture_output=True, text=True)
evenements = []
for ligne in proc.stdout.splitlines():
    ligne = ligne.strip()
    if not ligne:
        continue
    try:
        evenements.append(json.loads(ligne))
    except json.JSONDecodeError:
        pass
refus = [e for e in evenements if e.get("event") == "refuse"]
check("contexte du MOTEUR : code de sortie 0", proc.returncode, 0)
check("contexte du MOTEUR : AUCUN refus", [e.get("reason") for e in refus], [])
check("contexte du MOTEUR : un événement facts est émis",
      len([e for e in evenements if e.get("event") == "facts"]), 1)


# --- ⑤ Aucun fait émis n'est un littéral codé en dur -----------------------
# 🔴 CE QUI A ÉCHAPPÉ À ③ CI-DESSUS : ③ vérifie que la clé est CONNUE
# (présente dans `CLES_FACTS`), jamais que sa VALEUR a été MESURÉE. Une
# valeur mesurée vient toujours d'une EXPRESSION (une variable déjà validée
# plus haut dans le hook, un appel) ; une constante écrite à la main
# directement dans le dict d'émission (`True`, `None`, un nombre, une
# chaîne littérale) ne peut PAS être une mesure — par construction, elle ne
# dépend d'aucune entrée. C'est exactement la forme de `"vm_repond": True`.
def facts_avec_litteraux(chemin: pathlib.Path) -> list:
    """Les clés du dict `facts` dont la valeur est un littéral en dur."""
    arbre = ast.parse(chemin.read_text(encoding="utf-8"))
    for noeud in ast.walk(arbre):
        if not isinstance(noeud, ast.Dict):
            continue
        for cle, valeur in zip(noeud.keys, noeud.values):
            if (isinstance(cle, ast.Constant) and cle.value == "facts"
                    and isinstance(valeur, ast.Dict)):
                return sorted(
                    k.value for k, v in zip(valeur.keys, valeur.values)
                    if isinstance(k, ast.Constant) and isinstance(v, ast.Constant)
                )
    return []


check("aucun fait émis par resolve n'est un littéral codé en dur",
      facts_avec_litteraux(RACINE / "hooks" / "resolve.py"), [])

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print(f"OK - contrat de hw : {len(CLES_MOTEUR)} cles produites par "
      f"detect_all(), {len(CLES_FACTS)} facts emis par resolve")
