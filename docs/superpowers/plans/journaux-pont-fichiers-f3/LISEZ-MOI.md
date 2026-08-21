# Journaux du sous-bloc F3 — pont fichiers

Analyse : `../2026-08-20-pont-fichiers-f3-resultats.md`.

## Familles de lecture — DEUX, et c'est MESURÉ

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| les `*-agent.log` **bruts** | 7 | **séquences ANSI de `tracing` PRÉSENTES** : `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, **versé pour chacun** |
| tout le reste | — | rien de plus que le point ci-dessous |

🔴 **`grep -a` EST OBLIGATOIRE SUR TOUS LES JOURNAUX DE CE RÉPERTOIRE.** Ils
portent des octets NUL (lecture CIFS concurrente d'un fichier que Windows
réécrit). **Sans `-a`, `grep` classe le fichier « binaire » et rend une sortie
VIDE — pas un zéro**, et les deux se lisent pareil. C'est le piège de D10.

Relevé par la commande, après la dernière écriture :

```bash
grep -lP '\x1b\[' *.log *.json *.txt          # les 7 bruts, et eux seuls
for f in *.log *.txt; do grep -qc $'\000' "$f" && echo "NUL: $f"; done
```

## Ce que chaque exécution est

| Préfixe | Exéc. | Ce que c'est |
| --- | --- | --- |
| `exec1`, `exec2` | 2 | la recette nominale — critères ①, ②, ④ |
| `rouge-mutation` | 1 | **ROUGE** de ① et ② : `PONT_MUTATION=0`, `protege-en-ecriture=9` |
| `coupure-1` | 1 | critère ③, coupure **sur commande en vol** → `abandonnee=1` |
| `coupure-2` | 1 | critère ③, coupure qui a **manqué sa fenêtre** — versée telle quelle |
| `s1-1` | 1 | la sonde S1, armée : les trois questions de plateforme |
| `s1-0-DISQUALIFIEE` | 1 | 🔴 **montage réfuté par le contrôle du plan** — `PONT_ECRITURE=0` refuse aussi les `PRE_` de mutation, donc pour la mauvaise raison. Conservée : elle mesure que `protege-en-ecriture` est atteignable par un geste réel |
| `rouge-ecriture-DISQUALIFIEE` | 1 | 🔴 seconde tentative sous `PONT_ECRITURE=0` : la racine ne se monte pas, `total=0`, **rien à mesurer** |
| `s2-move-casse.txt` | 2 | la sonde S2, côté navigateur, sur l'hôte |

⚠️ **Les deux exécutions DISQUALIFIÉES sont versées avec leur diagnostic**, pas
effacées : un montage réfuté est une pièce.

## L'instrument

`instrument/` porte le pilote CDP, la mesure PowerShell et son lanceur, dans
leur état final. Trois réglages gouvernent ce qu'une exécution mesure :

| Variable | Effet |
| --- | --- |
| `FAUTES_FICHIERS=1` | arme l'injection de fautes côté page (`?faute-fichiers=1`) |
| `DIVERGER=1` | fabrique une dérive du miroir **côté local seulement** |
| `COUPURE_SUR_MARQUE=1` | pose la coupure du canal **sur le FAIT** — une marque écrite juste avant la commande qui ne revient jamais. `COUPURE_APRES_MARQUE_MS` départage `abandonnee` (0 ms) de `canal-ferme` (1500 ms) |

🔴 **`COUPURE_MS` existe encore et NE DOIT PAS ÊTRE EMPLOYÉE seule** : trois
coupures calées sur une horloge sont tombées dans le vide, le canal se fermant
proprement avec RIEN en vol.
