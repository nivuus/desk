# Journaux du sous-bloc F2 (pont fichiers)

## Familles de lecture : **DEUX**, et c'est mesuré

⚠️ **Une rédaction antérieure de ce fichier annonçait « UNE SEULE ».** Elle était
exacte à sa date — F2 n'avait alors joué aucune recette sur la VM — et la recette
du **21 août 2026** l'a rendue fausse : sept journaux d'agent viennent désormais
de la VM.

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| tout ce qui vient de l'hôte (`f2-*.txt`, `pilote-*.log`, `pilote-*.json`, `instrument/`) et les `agent-*-plat.log` | UTF-8, **aucune séquence ANSI** | rien : ils se `grep`ent à plat |
| les **sept** `agent-*.log` **bruts** | UTF-8, **CRLF**, **séquences ANSI de `tracing` PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, **versé pour chacun** |

✅ **AUCUN fichier ne porte d'octet NUL**, relevé par la commande sur les 36 du
premier niveau : contrairement aux journaux de pilote de P1 et de D10, **`grep -a`
n'est obligatoire nulle part ici**.
⚠️ *Le premier contrôle écrit pour l'établir matchait les 36 fichiers —
`grep -qa $'\000'` cherche la **chaîne vide**. Refait en Python.*
⚠️ **Seize fichiers portent des CRLF** (les quatorze journaux d'agent et les deux
de la sonde d'idiome) : cela ne gêne aucun `grep`.

⚠️ **Le défaut à deux réglages de `scripts/build-agent.sh` et
`scripts/run-agent.sh`** — qui ne posent pas `[Console]::OutputEncoding` — reste
**non corrigé**. Il n'a pas mordu ici : les journaux d'agent sont **copiés** du
partage CIFS, jamais renvoyés par PowerShell.

## Ce que ce répertoire contient — et ce qu'il ne contient PAS

**Six exécutions de recette** : `arme-1`, `arme-2` (les critères ① ② et la garde
de casse), `desarme-1` (le rouge du critère ④), `desarme-2` (**abandonnée côté
pilote**, côté produit complète), `reprise-1` et `reprise-2` (le critère ③).

🔴 **`pilote-desarme-2.json` N'EXISTE PAS** : la page-shell a cessé de répondre
aux `eval` après `23:21:31`, le pilote a été arrêté, et **la cause n'est pas
établie**. Son journal d'agent, lui, est versé et complet.

⚠️ **`agent-diagnostic-instrument.log` et `f2-diagnostic-instrument.txt` ne sont
pas une recette** : ce sont les pièces du défaut d'instrument de la première
tentative (`execFileSync` bloquait la boucle d'événements de Node, donc la page
cessait d'être servie, et **l'instrument détruisait ce qu'il mesurait**).

## Les pièces

| Fichier | Ce qu'il porte |
| --- | --- |
| `f2-tache1-rouges.txt` | les quatre rouges du protocole, **et ce qu'elles n'établissent pas** — la (d) fait tomber deux tests dont un préexistant |
| `f2-tache2-pont-ecriture-transmise.txt` | `PONT_ECRITURE` traverse `run-agent.sh` : vert, contrôle d'atteignabilité, rouge |
| `f2-tache3-garde-abi.txt` | la transposition **verbatim** comparée ligne à ligne, les tailles, et le garde d'ABI éprouvé **en deux temps** |
| `f2-client-rouges.txt` | les six rouges du navigateur, dont celle qui montre `Casse.txt` **écrasé** |
| `f2-tache14-verdict.md` + les deux `f2-tache14-sonde-idiome-*.log` | **R-F2-1 LEVÉ** : les cinq outils écrivent **en place**, session 0 **et** session 1 |
| `agent-*.log` / `agent-*-plat.log` | les sept journaux d'agent, bruts et mis à plat |
| `pilote-*.log` / `pilote-*.json` | ce que le navigateur a vu : compteur, relectures OPFS, arbre |

## Les instruments

| Fichier | Ce qu'il fait |
| --- | --- |
| `instrument/rouge.sh` | le harnais de mutation. **Il refuse de compter une rouge dont le diff est vide**, mute **par numéro de ligne**, et restaure par une **copie sans `-p`** — les trois pour des raisons payées sur place, écrites dans son en-tête |
| `instrument/rendre-ps1.sh` | rend LOCALEMENT le `.ps1` que `run-agent.sh` écrit sur la VM, en **extrayant** son heredoc plutôt qu'en le recopiant |
| `instrument/sonde-idiome.ps1` | ✅ **EXÉCUTÉE, 2 fois** (session 0 et session 1). Son critère est une **PRÉSENCE** (un temporaire trouvé), jamais une absence |
| `instrument/jouer-f2.sh` | une exécution de recette. **Tue l'agent AVANT, vérifie APRÈS**, et **efface les artefacts de l'exécution précédente** — un `pilote-*.json` périmé a failli être lu comme le résultat du run en cours |
| `instrument/pilote-f2.mjs` | le pilote CDP. `lireCompteur` **conserve** un échantillon illisible plutôt que de le sauter |
| `instrument/injection-f2.js` | le peuplement OPFS et le `showDirectoryPicker` surchargé. ⚠️ **Gardé par `EST_SHELL`** : sans cela il courait sur les fenêtres d'application, qui repeuplaient le répertoire **pendant que le pont écrivait** |
| `instrument/mesurer-f2.ps1` + `.sh` | la mesure côté VM, lancée par tâche planifiée `/it` |

## Chaînes de `grep` — CONFRONTÉES AU CODE **ET** AUX JOURNAUX

⚠️ **Trois des quatre `grep` de F1 cherchaient des chaînes que le produit
n'émet pas**, et **deux auraient fait lire un succès comme un échec.** Les cinq
chaînes de F2 sont donc vérifiées **des deux côtés** — présentes dans `agent/src`
**et** relevées dans les journaux versés :

| Chaîne | dans le code | dans les journaux |
| --- | --- | --- |
| `ecriture poussee` | 1 fichier | 4 journaux |
| `ecriture acquittee` | 1 | 4 |
| `ecriture due retenue` | 1 | 4 |
| `creation poussee` | 1 | 4 |
| `poussee d'ecriture DESARMEE` | 1 | **2** — les deux bras désarmés, et eux seuls |

🔴 **`AudioMort` a appris à ce dépôt qu'un jeton de protocole peut n'être
journalisé NULLE PART** (D11). La colonne « dans les journaux » est ce qui
l'évite : une chaîne présente dans le code et absente des journaux **d'un bras
vert** est une chaîne qu'il ne faut pas prescrire.
