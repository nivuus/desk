# Défaut 1 — l'identité du pont fichiers : rouge, remède, vert

**Instrument** : `instrument/compter-enrolements.sh`. Plateforme neuve sur le
port 8090 (base SQLite jetable, VM enrôlée à chaque relevé), superviseur lancé
sur la VM Windows par `scripts/run-agent.sh`, fenêtre d'observation bornée,
puis comptage dans `agent.log`.

**UNE exécution par colonne. Aucun taux n'est revendiqué.**

| Grandeur, comptée dans `agent.log` | ROUGE `f0…`/`338c535` | VERT `2db9956` |
| --- | --- | --- |
| binaire mesuré (octets) | **10 035 712** | **10 043 392** |
| fenêtre | 64 s | 64 s |
| lignes de journal | 505 | **80** |
| `agent enrôlé auprès de la plateforme` | **95** | **1** |
| fermetures `remplace` (évictions) | **94** | **0** |
| `reprise du canal /agent` | **94** | **0** |
| `pont fichiers lancé` | 1 | 1 |
| `identité héritée du superviseur` | 0 | **1** |
| `catalogue reconcilie` | **6** | **3** |
| `reenrolement observe` | **93** | **2** |

## Ce que chaque ligne établit

- **95 / 94 / 94 → 1 / 0 / 0.** Le superviseur et le pont ne s'évincent plus.
  La recette G1 relevait 94 / 93 ; ce relevé-ci, joué sur le binaire déployé du
  jour, en trouve 95 / 94 — même ordre, même mécanisme.
- **Le pont est lancé UNE fois dans les deux colonnes, et il survit dans les
  deux.** Le cycle ne tenait donc PAS à la mort et à la relance du pont
  (l'hypothèse de la recette G1) : **deux sockets vivants suffisent**, parce
  que le repli exponentiel du canal repart de zéro après tout enrôlement
  réussi. C'est un fait que ce relevé ajoute au diagnostic de G1.
- **6 → 3 réconciliations de catalogue.** Exactement le double avant : les
  **deux boucles de découverte** que G1 avait relevées, une par processus
  enrôlé, chacune faisant tout le travail COM/Shell toutes les 30 s.
- **93 → 2 « reenrolement observe ».** Chaque éviction forçait le renvoi du
  catalogue COMPLET — 154 entrées — au lieu d'un delta. C'est la cause des
  messages incrémentaux perdus.

## Ce que ce relevé n'établit PAS

- **Aucun taux** : une exécution par colonne.
- **Aucune fenêtre n'était ouverte**, donc **aucun enfant de fenêtre n'a été
  lancé**, ni au rouge ni au vert. Le correctif porte pourtant aussi sur
  `lancer` (`agent/src/superviseur/lanceur.rs`), et cette moitié-là n'est
  éprouvée que par la lecture du code et par la compilation croisée — **pas
  mesurée**. C'est exactement l'angle mort qui a rendu le défaut invisible à la
  recette G1 sur cette même moitié.
- **Le cas HiDPI, le cas multi-fenêtres, la durée longue** : rien.
- Les deux « reenrolement observe » du vert ne sont pas des réenrôlements :
  `apps::boucle` veille sur l'identité, et **le jeton change à chaque battement
  de cœur** (30 s). Le catalogue complet est donc renvoyé toutes les 30 s même
  en marche nominale. **Comportement PRÉEXISTANT, hors périmètre de cette
  correction, non corrigé** — mais il est désormais le seul renvoi complet qui
  reste, et il mérite d'être nommé.
