# `journaux-lot3` — le harnais de la campagne, et ce qu'il n'est pas

Ce répertoire porte le **harnais** (`instrument/harnais.sh` et
`instrument/etat-vm.sh`) que les tâches suivantes du lot 3 sourcent avant
toute séquence qui s'appuie sur la VM Windows `Windows` (libvirt/QEMU). Il
existe pour une seule raison : cette VM s'éteint seule, par deux mécanismes
distincts (une hibernation initiée **dans** l'invité, et l'hôte qui tue QEMU
par inactivité de `libvirtd`), et douze items du lot tiennent des séquences
assez longues pour la croiser.

## Ce que le harnais FAIT

- **`vm_prete`** — démarre la VM si besoin (`virsh start Windows`), attend
  que WinRM réponde sur le port 5985, **puis** attend un accès **réel** à
  `/media/vm` (jamais `mountpoint -q` : l'entrée CIFS survit dans la table de
  montage à une VM éteinte et une connexion morte). Rend `0` si les deux
  conditions sont atteintes, `1` sinon, avec un message qui nomme laquelle a
  manqué.
- **`agent_absent`** — interroge la VM par WinRM
  (`Get-Process agent -ErrorAction SilentlyContinue | Measure-Object`) et
  échoue (`1`) si au moins un `agent.exe` tourne encore. À appeler avant
  **chaque** tentative de lancement, y compris une tentative qui vient
  d'échouer : un agent survivant tient `agent.log`, et on relirait sinon le
  journal de la tentative précédente en croyant lire le sien.
- **`purger_orphelins`** — lance l'agent avec `MULTIFENETRE_VDD_PURGE=1` et
  rend le compte de purges trouvées dans le journal produit. Une sortie
  virtuelle et une racine ProjFS survivent à un arrêt brutal (`Drop` ne court
  pas sur un `TerminateProcess`).
- **`etat-vm.sh`** — un script autonome (pas une fonction sourcée) qui
  imprime, sur une ligne, l'état courant du domaine libvirt et le compteur
  d'extinctions relevé dans `/var/log/libvirt/qemu/Windows.log` :
  `etat=<...> extinctions=<n> horodatage=<iso>`.

## Ce que le harnais NE FAIT PAS

🔴 **Il ne juge rien.** `etat-vm.sh` rend un état, jamais un verdict — c'est
à l'item qui l'appelle de décider si cet état est celui qu'il attendait. Un
harnais qui classerait lui-même une séquence « valide » masquerait la
distinction entre « la mesure a été faite » et « la VM a tenu » : c'est
exactement le patron que ce dépôt a payé une dizaine de fois avec des
contrôles qui ne pouvaient rendre qu'une seule valeur.

Il ne compile rien, ne lance aucune recette de navigateur, ne mesure aucun
critère d'item : ces gestes restent dans l'instrumentation propre à chaque
tâche.

## Qui l'emploie, et où vivent ses journaux

| Tâche | Item | Répertoire de journaux |
| --- | --- | --- |
| Task 2 | 1 (3.7) — « temporaire + renommage » sur un éditeur réel | `docs/superpowers/plans/journaux-lot3-pont-sauvegarde/` |
| Task 3 | 2 (3.6) — les trois murs du pont, re-situés | `docs/superpowers/plans/journaux-lot3-pont-murs/` |
| Task 4 | 3 (3.8) — le répertoire frère qui disparaît | `docs/superpowers/plans/journaux-lot3-pont-frere/` |
| Task 5 | Le rejeu des douze pilotes (legs du lot 2) | `docs/superpowers/plans/journaux-lot3-pilotes/` |
| Task 6 | 5 (3.3) — l'extinction du superviseur | `docs/superpowers/plans/journaux-lot3-extinction/` (emploie `purger_orphelins`) |
| Task 7 | 6 — la file du capteur en charge réelle | `docs/superpowers/plans/journaux-lot3-file-capteur/` |
| Task 8 | 7 (3.1) — la latence capture → affichage | `docs/superpowers/plans/journaux-lot3-latence/` |
| Task 9 | 8 (3.2) — le propriétaire mono-fenêtre | `docs/superpowers/plans/journaux-lot3-mono/` |
| Task 10 | 9 (3.4) — les deux replis micro | `docs/superpowers/plans/journaux-lot3-micro/` |
| Task 11 | 10 (3.5) — une cause naturelle de mort de capture audio | `docs/superpowers/plans/journaux-lot3-audio/` |
| Task 12 | 11 (3.9) — les 71 applications `NonMesuree` | `docs/superpowers/plans/journaux-lot3-apps/` |
| Task 13 | 12 (3.10) — le maillon fautif du `Resize` | `docs/superpowers/plans/journaux-lot3-resize/` |

Task 1 (les 48 chemins périmés) et Task 14 (la revue transverse) n'emploient
pas le harnais : la première répare les chemins que les pilotes rejoués par
la Task 5 portent en dur, la seconde relit les documents produits par les
tâches précédentes.

## Emploi

```bash
unset -f chpwd 2>/dev/null
set -a && source .env && set +a
source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
vm_prete && agent_absent
```

⚠️ `harnais.sh` est à **sourcer**, jamais à exécuter : ses trois fonctions
n'existent qu'une fois sourcées dans le shell appelant.
