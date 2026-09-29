# Cycle de vie de la VM Windows

> Extrait de [`CLAUDE.md`](../../CLAUDE.md) (lignes 229-321 de l'original), déplacé tel quel. Liens relatifs à ce dossier.

## 🖥️ Cycle de vie de la VM Windows

> 🔴 **CE QUI SUIT DÉCRIT LA VM DE DÉVELOPPEMENT D'AVANT LE CHANTIER
> `package-nivuus` (29 août 2026) — trois faits que ce fichier ne disait
> nulle part avant ce chantier, et qui ont coûté une demi-journée à
> retrouver :**
>
> 1. **La VM cible est désormais une APPLIANCE**, provisionnée par le
>    package voisin `packages/installer` (dépôt `console`) : `C:\dev`, la
>    chaîne Rust et le montage CIFS décrits juste en dessous **n'existent
>    plus, retirés délibérément** — vérifié le 29 août 2026 (`/media/vm` est
>    aujourd'hui un répertoire vide, non monté).
> 2. **WinRM n'accepte plus Basic** : `scripts/winrm.js` (compte
>    `Administrateur`, transport Basic, cité plus bas) **ne fonctionne
>    plus** (401 mesuré le 22 août 2026). Le chemin qui répond aujourd'hui
>    est `installer/console/guest/winrm_exec.py`, en **NTLM**, mot de passe
>    lu depuis `/root/.config/nivuus/windows-admin.pass` (jamais sur
>    l'argv).
> 3. ~~**Le lot 3 du chantier `package-nivuus` (une campagne de douze items de
>    mesure visant cette VM) est SUSPENDU** pour cette raison~~ — **LEVÉ LE
>    5 SEPTEMBRE 2026, PAR UN HARNAIS RE-SITUÉ, PAS PAR UN RÉÉQUIPEMENT DE LA
>    VM.** `docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh`
>    vise `console/guest/winrm_exec.py` (NTLM), `C:\nivuus\agent.log` et la
>    tâche planifiée `guacamole-agent`, et il est posé **À CÔTÉ** de
>    `harnais.sh`, qui reste intact — c'est une pièce datée du 28 août.
>    **Six des douze items portent désormais un chiffre daté, six restent dus** :
>    voir [les résultats partiels](../superpowers/plans/2026-09-05-lot3-campagne-vm-resultats-partiels.md).
>    ⚠️ Le sort de `scripts/winrm.js`, `/media/vm` et `scripts/build-agent.sh`
>    n'est **toujours pas** tranché : ce lot ne les a ni corrigés ni retirés.
>
> Voir
> [`docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md`](../superpowers/plans/2026-08-29-package-nivuus-resultats.md)
> pour le détail.
>
> ⚠️ **Le sort de `scripts/winrm.js`, de `/media/vm` et de
> `scripts/build-agent.sh` ci-dessous — qui visent tous cette VM de
> développement qui n'existe plus sous cette forme — N'EST PAS TRANCHÉ.**
> Les corriger, les retirer ou les garder est une décision du propriétaire
> du dépôt, pas de ce chantier. Ce qui suit reste donc écrit tel quel,
> **à lire désormais comme un relevé historique**, jusqu'à cette décision.

**La VM cible est une machine libvirt/QEMU nommée `Windows`, et elle n'est pas
démarrée automatiquement.** Tout travail touchant l'agent Rust, la capture, la
recette WebRTC ou WinRM exige qu'elle tourne. Symptômes d'une VM éteinte :
`192.168.3.2` injoignable (« Aucun chemin d'accès pour atteindre l'hôte
cible ») et `/media/vm/` vide.

```bash
# État
virsh list --all            # « fermé » = éteinte, « en cours d'exécution » = démarrée

# Démarrer, puis attendre que WinRM réponde (~5 à 60 s)
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done

# Puis attendre que le partage de fichiers soit monté : /media/vm se monte
# après que WinRM répond, pas en même temps — un `scripts/build-agent.sh`
# lancé dès que le port 5985 répond échoue avec « erreur : /media/vm n'est
# pas monté » (`scripts/sync-agent.sh`, qui teste le montage, pas le port).
# `mountpoint -q` ne suffit pas : /media/vm est un montage CIFS
# (//192.168.3.2/c) dont l'entrée persiste dans la table de montage même VM
# éteinte et connexion morte — il faut éprouver un ACCÈS réel, pas la seule
# présence de l'entrée.
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done

# Arrêter proprement
virsh shutdown Windows
```

Une fois la VM démarrée, l'outillage habituel redevient disponible :

```bash
set -a && source .env && set +a      # charge les identifiants (fichier gitignoré)
node scripts/winrm.js '<commande PowerShell>'
scripts/build-agent.sh               # synchronise puis compile SUR la VM
scripts/run-agent.sh                 # lance l'agent en session interactive
```

**Ne jamais supposer la VM allumée.** Vérifier `virsh list --all` avant toute
séquence qui en dépend, et la démarrer si besoin — c'est une opération sûre et
idempotente.

⚠️ **`scripts/build-agent.sh` lancé sans avoir sourcé `.env` s'arrête EN
SILENCE**, après sa ligne « sources synchronisées », sans message ni statut
d'erreur : son `set -euo pipefail` avorte sur l'affectation du quota WinRM, dont
le `2>/dev/null` mange la cause. **Le symptôme se lit exactement comme une
compilation réussie et muette** — on mesure alors le binaire précédent sans
qu'aucune trace ne le dise. Relevé le 2 août 2026, revue finale du sous-bloc D2.
Sourcer `.env` d'abord, et se méfier d'un build qui ne dit rien.

---
