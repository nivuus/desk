#!/usr/bin/env python3
"""Assemble l'ARBITRE (chronomètre de la VM) et la DÉCOMPOSITION (histogramme
du pont) autour de chaque geste, et nomme leur DIFFÉRENCE.

    analyser-f4.py <pilote-X.json> <agent-X-plat.log>

🔴 L'ATTRIBUTION SE FAIT PAR HORODATAGE, ET C'EST GRATUIT. Le chronomètre écrit
`debut_iso`/`fin_iso` avec l'horloge de la VM ; `agent.log` est écrit par la
MÊME horloge. Aucune synchronisation entre machines n'est nécessaire, et aucun
instrument de plus.

⚠️ LE RÉSIDU EST NOMMÉ, JAMAIS MESURÉ. `mur-à-mur − Σ(traversées)` est une
soustraction entre deux grandeurs prises par deux horloges sur deux machines ;
il en porte les deux incertitudes. Il se rapporte en ORDRE DE GRANDEUR.

⚠️ LES COMPTEURS SONT CUMULATIFS : la fenêtre d'un geste est
`[dernier recensement AVANT debut_iso, dernier recensement AVANT fin_iso+repos]`.
Le repos de 25 s garantit DEUX recensements dans cette fenêtre.
"""
import json, re, sys
from datetime import datetime, timedelta

FAMILLES = ['attributs', 'lister', 'lire', 'ecrire', 'mutation']
RE_LIGNE = re.compile(r'^(\S+Z)\s+INFO\s+\S+:\s+traversees (.*)$')


def horo(s):
    return datetime.fromisoformat(s.replace('Z', '+00:00'))


def recensements(chemin):
    """Les lignes `traversees`, en (instant, {famille: (n, moy_us, max_us)}, seaux)."""
    sortie = []
    with open(chemin, 'r', encoding='utf-8', errors='replace') as f:
        for ligne in f:
            m = RE_LIGNE.match(ligne.rstrip('\n').rstrip('\r'))
            if not m:
                continue
            tetes, _, seaux = m.group(2).partition(' | seaux_ms ')
            fam = {}
            for f_ in FAMILLES:
                g = re.search(rf'\b{f_}=n:(\d+) moy_us:(\d+) max_us:(\d+)', tetes)
                if g:
                    fam[f_] = tuple(int(x) for x in g.groups())
            sk = {}
            for f_ in FAMILLES:
                g = re.search(rf'\b{f_}=([0-9:,a-z]+)', seaux)
                if g:
                    sk[f_] = {p.split(':')[0]: int(p.split(':')[1]) for p in g.group(1).split(',') if ':' in p}
            sortie.append((horo(m.group(1)), fam, sk))
    return sortie


def dernier_avant(rec, t):
    vus = [r for r in rec if r[0] <= t]
    return vus[-1] if vus else None


def main():
    pilote = json.load(open(sys.argv[1], encoding='utf-8'))
    rec = recensements(sys.argv[2])
    mv = pilote.get('mesure_vm') or {}
    repos = mv.get('repos_s', 25)
    print(f"# {sys.argv[1]}  —  {len(rec)} recensements, repos={repos} s, "
          f"fini={mv.get('fini')}, gestes={len(mv.get('releves', []))}")
    if not rec:
        print("⛔ AUCUNE ligne `traversees` : PONT_MESURE n'a pas atteint le processus, "
              "ou le pont n'a pas vécu deux périodes de recensement.")
    print(f"{'geste':>16} {'arbitre_ms':>11} | {'famille':>9} {'dn':>4} {'moy_ms':>8} {'max_ms':>8} "
          f"{'somme_ms':>9} | {'residu_ms':>10} {'residu_%':>8}")
    for r in mv.get('releves', []):
        if 'debut_iso' not in r:
            continue
        a = dernier_avant(rec, horo(r['debut_iso']))
        b = dernier_avant(rec, horo(r['fin_iso']) + timedelta(seconds=repos))
        arbitre = r.get('ms', 0)
        if a is None or b is None or a[0] == b[0]:
            print(f"{r['geste']:>16} {arbitre:11.1f} | ⛔ fenêtre de recensement VIDE "
                  f"(avant={a[0].isoformat() if a else None}, apres={b[0].isoformat() if b else None})")
            continue
        somme = 0.0
        lignes = []
        for f_ in FAMILLES:
            na, ma, xa = a[1].get(f_, (0, 0, 0))
            nb, mb, xb = b[1].get(f_, (0, 0, 0))
            dn = nb - na
            if dn <= 0:
                continue
            # moyenne du DIFFÉRENTIEL : (somme_b − somme_a) / dn, où somme = n·moy
            d_somme_us = nb * mb - na * ma
            somme += d_somme_us / 1000.0
            lignes.append((f_, dn, d_somme_us / dn / 1000.0, xb / 1000.0))
        # 🔴 AUCUN RÉSIDU POUR UNE DURÉE IMPOSÉE. `explorer` tient sa fenêtre
        # 30 s par un `Start-Sleep`, et `sommeil` est un repos : leur mur-à-mur
        # n'est pas une latence, donc leur « résidu » n'en est pas un non plus.
        # L'afficher rendrait 98 % — un chiffre qui SE LIRAIT comme du temps
        # perdu dans ProjFS.
        impose = r['verbe'] in ('explorer', 'sommeil', 'application')
        # 🔴 UN RESIDU N'A DE SENS QUE SUR DES TRAVERSEES SERIALISEES.
        # `pont::lecture::MORCEAUX_EN_VOL = 4` : jusqu'a QUATRE lectures sont en
        # vol EN MEME TEMPS, et leur somme n'est alors pas une duree ecoulee.
        # Un residu negatif de −141 % n'est pas une anomalie de mesure : c'est
        # une soustraction qui ne veut rien dire, et l'afficher SE LIRAIT comme
        # du temps que ProjFS aurait rendu.
        concurrent = any(f_ == 'lire' and (b[1].get(f_, (0,))[0] - a[1].get(f_, (0,))[0]) > 1
                         for f_ in FAMILLES)
        residu = arbitre - somme
        pct = (residu / arbitre * 100) if arbitre else 0
        if not lignes:
            print(f"{r['geste']:>16} {arbitre:11.1f} | (aucune traversée) "
                  f"{'':>32}"
                  + ('| durée IMPOSÉE' if impose else f"| {residu:10.1f} {pct:7.1f}%"))
        for i, (f_, dn, moy, mx) in enumerate(lignes):
            tete = f"{r['geste']:>16} {arbitre:11.1f}" if i == 0 else ' ' * 28
            if impose: q = '| durée IMPOSÉE'
            elif concurrent: q = '| CONCURRENT : pas de résidu'
            else: q = f"| {residu:10.1f} {pct:7.1f}%"
            queue = q if i == 0 else ''
            print(f"{tete} | {f_:>9} {dn:4d} {moy:8.1f} {mx:8.1f} {somme:9.1f} {queue}")
        if not r.get('ok', True):
            print(f"{'':>28} ⚠️  ÉCHEC : {str(r.get('erreur', '')).strip()[:70]} "
                  f"— la durée est CENSURÉE par le budget du produit, ce n'est PAS une latence")


main()
