# Journaux du sous-bloc F2 (pont fichiers)

## Familles de lecture : **UNE SEULE**

Tous les fichiers de ce répertoire sont des sorties `cargo`, `npm`, `node` ou
`bash` produites **sur l'hôte Linux** — jamais du PowerShell distant. Ils sont en
UTF-8, sans séquence ANSI, sans octet de contrôle : **ils se `grep`ent à plat**,
sans `sed`, sans `grep -a`, sans `iconv`.

⚠️ **Ce n'est pas un mérite** : le défaut à deux réglages de
`scripts/build-agent.sh` et `scripts/run-agent.sh` — qui ne posent pas
`[Console]::OutputEncoding` — reste **non corrigé**. Il n'est simplement pas
rencontré, **parce que F2 n'a joué aucune recette sur la VM** (voir §0 du
document de résultats).

## Ce que ce répertoire NE contient pas

🔴 **Aucun journal d'agent, aucun journal de recette, aucune mesure sur VM.** Les
tâches 14 et 15 du plan n'ont pas été jouées : la VM était tenue par le
sous-bloc **G2**. `instrument/sonde-idiome.ps1` est **écrit et versé, jamais
exécuté**, et son en-tête le déclare.

## Les pièces

| Fichier | Ce qu'il porte |
| --- | --- |
| `f2-tache1-rouges.txt` | les quatre rouges du protocole, **et ce qu'elles n'établissent pas** — la (d) fait tomber deux tests dont un préexistant |
| `f2-tache2-pont-ecriture-transmise.txt` | `PONT_ECRITURE` traverse `run-agent.sh` : vert, contrôle d'atteignabilité, rouge |
| `f2-tache3-garde-abi.txt` | la transposition **verbatim** comparée ligne à ligne, les tailles, et le garde d'ABI éprouvé **en deux temps** |
| `f2-client-rouges.txt` | les six rouges du navigateur, dont celle qui montre `Casse.txt` **écrasé** |

## Les instruments

| Fichier | Ce qu'il fait |
| --- | --- |
| `instrument/rouge.sh` | le harnais de mutation. **Il refuse de compter une rouge dont le diff est vide**, mute **par numéro de ligne**, et restaure par une **copie sans `-p`** — les trois pour des raisons payées sur place, écrites dans son en-tête |
| `instrument/rendre-ps1.sh` | rend LOCALEMENT le `.ps1` que `run-agent.sh` écrit sur la VM, en **extrayant** son heredoc plutôt qu'en le recopiant |
| `instrument/sonde-idiome.ps1` | 🔴 **NON EXÉCUTÉE.** La sonde du risque R-F2-1 : quel idiome d'enregistrement emploie cette VM ? |

## Chaînes de `grep` qui se sont révélées FAUSSES

**Aucune** — parce qu'aucune n'a été prescrite contre un journal d'agent. Les
chaînes que la recette de F2 devra employer sont **établies dans le plan** et
**non vérifiées contre le produit** : c'est le premier geste de qui la jouera.

⚠️ **Trois des quatre `grep` de F1 cherchaient des chaînes que le produit
n'émet pas**, et **deux auraient fait lire un succès comme un échec.** Les
chaînes réellement émises par F2, à confronter au code avant de compter :
`ecriture poussee`, `ecriture acquittee`, `ecriture due retenue`,
`poussee d'ecriture DESARMEE`, `creation poussee`.
