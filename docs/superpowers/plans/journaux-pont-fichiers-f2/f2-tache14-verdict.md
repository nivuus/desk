# Tâche 14 — la sonde d'idiome : **R-F2-1 EST LEVÉ**

**21 août 2026.** Deux exécutions : une en **session 0** (WinRM) et une en
**session 1** (tâche planifiée `/it`). Journaux :
`f2-tache14-sonde-idiome-session0.log`, `f2-tache14-sonde-idiome-session1.log`.

## Le verdict

| Outil | session 0 | session 1 | Événements observés |
| --- | --- | --- | --- |
| `[IO.File]::WriteAllText` | **EN PLACE** | **EN PLACE** | `Changed document.txt` ×2 |
| `Add-Content` | **EN PLACE** | **EN PLACE** | `Changed document.txt` |
| `cmd /c echo >` | **EN PLACE** | **EN PLACE** | `Changed document.txt` ×2 |
| `Set-Content` | **EN PLACE** | **EN PLACE** | `Changed document.txt` ×2 |
| **`notepad.exe`** (frappes + Ctrl+S) | NON MESURABLE (session 0) | 🔵 **EN PLACE** | `Changed document.txt` |

**Aucun fichier temporaire, aucun renommage, sur aucun des cinq outils.**

🔴 **R-F2-1 — « aucun outil de cette VM n'écrit EN PLACE » — est donc RÉFUTÉ,
et c'était le seul risque qui rendait F2 non livrable.** Le critère ① a un
instrument, et il est même le plus ordinaire qui soit.

🔵 **Le Bloc-notes en fait partie, et c'est ce qui compte le plus** : c'est
l'outil qu'un humain emploierait, et le contenu final le prouve
(`reecrit par le Bloc-notescontenu initial` — les frappes insérées, puis
`Ctrl+S`). *Il n'aurait pas pu être mesuré depuis WinRM : `SendKeys` n'atteint
aucun bureau depuis la session 0, et la sonde le DIT plutôt que de rendre un
faux « NON MESURE ».*

## ⚠️ Ce que cette sonde n'établit PAS

- **Elle mesure `C:\dev\sonde-idiome`, un répertoire NTFS ORDINAIRE — pas une
  racine ProjFS.** L'idiome d'un outil pourrait différer sous un fournisseur de
  virtualisation. *La recette le tranche de fait : les mêmes appels y écrivent,
  et le pont voit leurs notifications.*
- **Elle ne dit rien de LibreOffice ni de Word**, qui emploient l'idiome
  temp+rename que F2 refuse par construction (§0.2 du plan). Le critère 2 de la
  spec §8 F2 reste **déplacé en F3**.
- **Cinq outils, une exécution chacun par session.** Aucun taux.

## Le piège d'instrument, payé sur place

**Invoquée directement par `winrm.js`, la sonde n'a rendu que ses DEUX premières
lignes** — l'en-tête — et rien de plus, sans aucun message d'erreur. Le reste de
sa sortie n'atteint pas l'appelant. La forme qui marche est un lanceur qui
redirige **tous les flux** vers un fichier (`*>&1 | Out-File`), relu ensuite
depuis l'hôte.

⚠️ **Une sonde qui rend deux lignes au lieu de quarante se lit comme une sonde
qui a échoué**, et rien ne l'en distingue : c'est le même genre de silence que
le défaut à deux réglages de `run-agent.sh`, sous une autre forme.
