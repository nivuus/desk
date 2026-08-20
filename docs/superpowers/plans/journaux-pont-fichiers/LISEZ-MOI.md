# Journaux de la recette F1 — pont fichiers

Relevés du 19 et 20 août 2026. Binaire mesuré : **9 914 368 octets**
(`/media/vm/dev/target/release/agent.exe`, bâti par `scripts/build-agent.sh`
après `cargo clean --release -p proto -p agent` — le chantier touche `proto`,
et nettoyer `agent` seul ne suffit pas).

## Familles de lecture — DEUX, et une seule demande un `sed`

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| `agent-*-plat.log`, tous les `.txt`, tous les `.json` | UTF-8, **ANSI déjà retirées** | rien |
| `agent-*.log` (bruts) | UTF-8, **séquences ANSI PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, versé pour chacun |

⚠️ **Toujours `grep -a`** : un journal à queue d'octets NUL est classé
« binaire » et `grep` rend alors une sortie **vide**, indiscernable d'un
compte nul (piège du sous-bloc D10).

## Deux chaînes que le plan de F1 prescrit et qui ne matchent RIEN

- Le Step 4 prescrit `grep -c 'pont lancé'`. **Cette chaîne n'existe pas.**
  La trace réelle est **`pont fichiers lancé`**
  (`agent/src/superviseur/lanceur/pont.rs`). Le `grep` du plan rend `0`, ce
  qui se lit comme un pont absent.
- Le Step 3 prescrit de chercher `ERROR_MOD_NOT_FOUND`. **Cette chaîne
  n'apparaît nulle part dans `agent/src/`** : elle ne vit que dans la spec et
  le plan. Ce que le journal porte est le contexte `anyhow`
  **`chargement de ProjectedFSLib.dll`** suivi de
  **`Le module spécifié est introuvable. (0x8007007E)`**.
  **Grepper `ProjectedFSLib`.**

`grep -c 'capteur lancé'`, lui, est exact.

## Les pièces

| Fichier | Ce que c'est |
| --- | --- |
| `sonde-picker.txt` | 🔴 **La mesure qui décide de l'instrument** : `showDirectoryPicker()` est inutilisable sous Chrome sans interface, et OPFS rend une vraie `FileSystemDirectoryHandle`. À lire avant tout le reste |
| `sha256-origine.txt`, `noms-origine.txt` | Le jeu de données côté hôte : condensat et ensemble de noms |
| `mesure-exec1.txt`, `mesure-exec2.txt`, `mesure-exec5.txt` | Les relevés VM. ⚠️ `C:\dev\mesure.txt` est **écrasé à chaque exécution** : ces copies sont prises entre deux runs, et chacune s'arrête là où le script s'est arrêté |
| `agent-exec{1..5}*.log` | Journaux d'agent des cinq exécutions du chemin nominal |
| `agent-sans-projfs*.log` | **Step 3** : la DLL renommée. 366 mentions de `ProjectedFSLib`, **0 `ERROR`** |
| `agent-rouge-i*.log`, `rouge-i-kill-2s.txt` | **Step 6 rouge (i)** : mise à mort du pont par PID relevé |
| `rouge-i-pid-mal-choisi.txt` | Le **défaut** de la première version du rouge (i) : PID deviné au lieu d'être relevé — l'enfant a été tué, pas le pont |
| `borne-lecture.txt` | Sonde de lecture, **avec la réserve qui dit ce qu'elle ne borne pas** |
| `agent-diag*.log`, `agent-essai*.log`, `agent-verif*.log`, `agent-dbg*.log` | Les runs de mise au point de l'instrument, conservés pour leur diagnostic |
| `pilote-*.json`, `orchestrateur-*.txt` | Sorties du pilote CDP et de l'orchestrateur |
| `instrument/` | L'instrument, versé dans son état final |

## Ce que l'instrument remplace, et ce qu'il ne remplace pas

`showDirectoryPicker()` ne peut pas être **accepté** dans ce montage (mesuré,
`sonde-picker.txt`). La parade est OPFS : `window.showDirectoryPicker` est
surchargé pour rendre une poignée OPFS peuplée depuis le **vrai** jeu de
données de l'hôte, servi en HTTP.

**Le point d'injection est le plus bas possible** : `choisirDossier()` lit
`globalThis.showDirectoryPicker` **à l'appel**. Tout le code produit tourne
donc inchangé derrière — `choisirDossier`, l'affectation `const racine: Racine
= poignee` qui est le contrôle de compatibilité structurelle,
`creerAdaptateur`, `creerServeur`, `connecterCanalFichiers`, et le
gestionnaire de clic de `shell-page.ts`.

**Ce que la recette n'exerce donc PAS** : l'appel `showDirectoryPicker()`
lui-même, le modèle de permission des répertoires choisis par l'utilisateur,
et l'activation utilisateur transitoire.

**Ce qui rend le critère 2 attribuable** : la page calcule le condensat
SHA-256 de ce qu'elle tient réellement en OPFS. Il a été relevé **égal** au
condensat de l'hôte (`2d7d5440…f8b9`) — les octets partent donc justes.
