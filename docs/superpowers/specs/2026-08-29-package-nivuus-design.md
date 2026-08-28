# `desk` devient un package Nivuus : conception

> **29 août 2026.** Ce chantier interrompt le lot 3 (la campagne sur la VM),
> et il l'interrompt pour une raison mesurée, pas par préférence : **la VM
> cible n'est plus une machine de développement, c'est une appliance**, et le
> dépôt `desk` ne sait pas s'installer dessus.

## 1. Ce qui a rendu ce chantier nécessaire

Le lot 3 a buté sur une VM qui refusait les identifiants du dépôt. Le
diagnostic, poussé jusqu'au bout le 28 août 2026, a établi ceci :

| Ce que `desk` suppose | Ce que la VM est aujourd'hui |
| --- | --- |
| compte `Administrateur` (français), WinRM **Basic** | `NIVUUS-WIN`, compte `Administrator`, WinRM **Negotiate seul** — Basic rend 401 |
| sources en `C:\dev`, Rust présent, compilation **sur la VM** | ni `C:\dev`, ni Rust — **retirés délibérément** |
| montage CIFS `//192.168.3.2/c` | **retiré**, et `run-agent.ps1` de `console` le dit : *« it requires the //192.168.3.2/c CIFS mount that the cutover removes »* |
| ProjFS actif, VB-Audio posé | ProjFS **désactivé**, VB-Audio **absent** |

🔴 **Rien de tout cela n'est une panne.** Le paquet voisin
`packages/installer` provisionne désormais cette VM en **appliance** :
`console/guest/provision/40-agent.ps1` y dépose l'agent dans
`C:\nivuus\agent` et l'arme par une tâche planifiée en session 1, et
`console/guest/winrm_exec.py` porte le constat que ce dépôt a mis une heure
à refaire — *« pywinrm with the ntlm transport negotiates correctly
(measured 2026-08-22, Basic returned 401) »*.

🔴 **ET LE TROU QUE CE CHANTIER FERME EST NOMMÉ PAR `console` LUI-MÊME** :
`guest/payload.py` déclare `("agent", "agent.exe", "Guacamole agent,
extracted before the wipe")`, et `fetch_payload.py` écrit *« Not fetched, and
never fetchable »*. **L'appliance se reconstruit aujourd'hui autour d'un
binaire que personne ne sait refabriquer.** C'est `desk` qui doit le
produire, et c'est la raison la plus concrète d'en faire un package.

## 2. Ce que ce chantier livre, et ce qu'il ne livre pas

**Il livre** : la dépendance inter-packages dans le moteur d'`installer`, et
le package `desk` — manifeste, wizard, trois hooks, tests, et la production
reproductible d'`agent.exe`.

**Il ne livre pas** : le lot 3. Les douze items de mesure attendent une VM
équipée, et ce chantier est ce qui rend cette VM atteignable — il ne mesure
rien lui-même.

## 3. Volet A — la dépendance inter-packages, dans `installer`

Le manifeste `nivuus.dev/v1` ne sait pas exprimer qu'un package en exige un
autre : `requires` ne connaît que `capabilities` (matériel : `iommu`,
`gpu-discrete`, `nvme-dedicated`, `cpu-hybrid`) et `features`
(fonctionnalités de l'installeur). Or **`desk` ne doit pas pouvoir
s'installer sans `console`.**

On ajoute `requires.packages`, en trois endroits :

1. **`installer/packages/manifest.py`** — parsage de `requires.packages`,
   traité comme ses deux voisins (liste de chaînes, dédupliquée, ordre
   préservé). ⚠️ Le module **refuse ce qu'il ne comprend pas**, à dessein :
   la clé doit donc y être connue explicitement, elle ne peut pas être
   « tolérée ».
2. **`installer/packages/discovery.py::eligibility()`** — un quatrième motif
   de rejet, avec sa phrase, sur le modèle exact des trois autres :
   `package requis non sélectionné : console`. **Un package inéligible
   voyage avec la raison de son inéligibilité** — c'est la thèse du module,
   et elle ne souffre pas d'exception ici.
3. **L'ordre d'installation** — `desk` exige que la VM existe au moment de
   son `activate`, donc `console` s'installe **avant** lui. Tri topologique
   des packages sélectionnés, et **refus explicite sur un cycle** : un cycle
   n'a pas d'ordre, et en choisir un au hasard installerait dans un ordre que
   personne n'a demandé.

🔴 **RÉTRO-COMPATIBILITÉ, DANS UN SENS SEULEMENT, ET IL FAUT LE DIRE** : un
manifeste portant `requires.packages` est **rejeté par un moteur antérieur**,
puisque le parseur refuse ce qu'il ne comprend pas. Les deux dépôts avancent
donc ensemble, et le package `desk` n'est installable qu'à partir de la
version du moteur qui connaît la clé. Ce n'est pas un défaut du parseur :
c'est ce qui empêche un manifeste à moitié compris d'être à moitié appliqué.

⚠️ **LA DÉPENDANCE PORTE SUR LA SÉLECTION, PAS SUR L'ÉTAT DE LA MACHINE**, et
les deux questions ne se confondent pas :

| Question | Qui y répond | Quand |
| --- | --- | --- |
| « `console` est-il coché dans cet assistant ? » | `requires.packages` | **avant** qu'un octet touche le disque |
| « une VM Windows répond-elle sur cette machine ? » | le `resolve` de `desk` | avant la partition, mais après la sélection |

Deux gardes, deux moments, **deux phrases distinctes**. Confondre les deux
donnerait soit un package qui s'installe sur une machine sans VM, soit un
package qu'on ne peut jamais cocher en premier.

## 4. Volet B — le package `desk`

### 4.1 Tier : `userspace`

`desk` n'ajoute ni module noyau, ni ligne de commande noyau, ni hugepages :
`console` a déjà pris VFIO, le GPU et le NVMe. Le contrat interdit à un
package `userspace` de déclarer ces trois choses — c'est exactement la
garantie qu'on veut donner, et elle est vérifiée par le moteur, pas promise
par nous.

### 4.2 La frontière avec `console`, telle que le code la dessine déjà

| | `console` | `desk` |
| --- | --- | --- |
| possède | la VM, son provisioning, son cycle de vie | le service côté Debian : plateforme, page, TURN |
| **déploie** l'agent | oui — `40-agent.ps1`, `C:\nivuus\agent`, tâche en session 1 | non |
| **produit** l'agent | non — il l'extrait de la VM avant de l'effacer | **oui — ce que ce chantier ajoute** |

### 4.3 La production d'`agent.exe` — mesurée, pas supposée

**Éprouvé le 28 août 2026 sur l'hôte** :
`cargo build --release --target x86_64-pc-windows-gnu -p agent` **réussit**,
code de retour **0**, et rend un `agent.exe` de **20 468 152 octets**. La
cible et `x86_64-w64-mingw32-gcc` étaient déjà installées.

🔴 **CE QUE CETTE MESURE ÉTABLIT, ET CE QU'ELLE N'ÉTABLIT PAS.** Elle établit
que l'agent **se lie** hors de la VM. Elle n'établit **pas qu'il
fonctionne** : l'agent était jusqu'ici bâti *sur* la VM, ce binaire-ci est un
produit **mingw** qui n'a jamais tourné, et ce dépôt juge sur la relecture,
jamais sur un code de retour. **Un critère de recette devra l'exécuter sur la
VM** — capture, encodage, injection d'entrées — avant qu'on puisse écrire que
la compilation croisée remplace l'ancienne.

⚠️ **La taille du binaire ne prouvera rien**, dans les deux sens : ce dépôt a
mesuré que deux compilations de la même source rendent deux tailles, et qu'un
binaire neuf peut peser exactement autant que celui qu'il remplace. Ce qui
tranchera est **une chaîne posée soi-même**, cherchée sur le chemin que
l'appliance lance, avec son témoin négatif.

### 4.4 Les trois hooks

**`resolve`** — lecture seule, et **un refus est une donnée, jamais une
exception** : tout chemin qui peut échouer finit par un événement `refuse`
portant une phrase, parce que ce hook court **avant `partition()`**, donc
avant qu'un octet touche le disque. Il vérifie ce que le manifeste ne peut
pas savoir :

- qu'une **VM Windows répond réellement** (le domaine libvirt existe, et
  WinRM répond en NTLM) — la seconde des deux questions du §3 ;
- que **Node est présent en `>=24.0.0 <25.0.0`** — le `engines` que
  `plateforme/package.json` déclare, relu et non recopié ;
- que le **port d'écoute est libre** ;
- et il **dérive les deux adresses TURN** depuis les interfaces.
  ⚠️ `docker-compose.coturn.yml` exige `TURN_LISTENING_IP` **et**
  `TURN_RELAY_IP`, toutes deux obligatoires : borner la seule écoute
  laisserait les allocations de relais sur toutes les interfaces — mesuré
  le 21 août 2026, **23 adresses distinctes dont l'adresse publique**.

**`install`** — pose ce qui vit sur l'hôte : la plateforme et le client
**bâti**, l'unité systemd, le répertoire de données, la configuration
coturn, et les secrets en **mode 600**.

🔴 **LE SECRET DE JETON NE SE DEMANDE PAS, IL SE TIRE AU SORT** —
32 caractères au moins, ce que `PLATEFORME_SECRET_JETON` exige déjà en
refusant plus court. Un secret qu'on demande à un opérateur est un secret
qu'il choisit court ; et un défaut aléatoire **à chaque démarrage**
invaliderait toutes les sessions, ce qui est la raison pour laquelle la
variable n'a **aucun défaut**. Il est donc tiré **une fois**, à
l'installation, et persisté.

⚠️ **`PLATEFORME_PAGE` est ce qui rend nginx facultatif** : depuis le
22 août 2026 la plateforme sert elle-même la page bâtie. L'installation la
pose vers `client/dist`. **Absente ou vide, `GET /` rend le 404 d'hier à
l'octet près** — c'est ce qui rend le servant strictement additif, et c'est
pourquoi elle n'a **aucun défaut** : un défaut ferait publier le répertoire
courant du service.

**`activate`** — arme ce que `install` n'a fait que poser.

- **Par symlink, jamais `systemctl enable`** : `systemctl` échoue en silence
  dans un environnement contraint — une sous-commande de requête n'imprime
  rien — donc un `enable` qui « rend la main » ne dit rien, là qu'un lien
  existe ou lève. Chaque lien est vérifié pointer vers une unité existante.
- Crée le **compte initial** (`npm run admin:utilisateur`, mot de passe **sur
  stdin seul, jamais en `argv`**).
- **Enrôle l'agent** (`npm run admin:agent`), ce qui rend `AGENT_VM` et
  `AGENT_SECRET`. ⚠️ `--vm` attend l'**identifiant**, pas le nom, et
  `--adresse` n'a aucun défaut.
- **Dépose `agent.exe` dans le payload de `console`**, là où
  `40-agent.ps1` va le chercher : le sous-répertoire `agent/` du répertoire
  de payload, `agent.exe` étant l'entrée que `payload.py::REQUIRED_BINARIES`
  déclare sous ce nom. ⚠️ **Ce répertoire n'est PAS codé en dur** — `console`
  le reçoit en paramètre (`fetch_payload.py --drivers-dir`), et
  `40-agent.ps1` prend un `-PayloadRoot` obligatoire. Le plan devra donc
  établir **par où** `desk` apprend ce chemin, plutôt que d'en inventer un :
  c'est une pièce du contrat inter-packages du §4.5, pas un détail
  d'implémentation.
- **Pose dans la VM ce dont `desk` a besoin** — §4.5.

### 4.5 Ce que `desk` écrit dans la VM, et par quel chemin

**ProjFS est désactivé et VB-Audio absent** : deux dépendances de `desk` qui
vivent dans une machine dont `console` est propriétaire. **`desk` les pose
lui-même, par le chemin WinRM de `console`** — décision du propriétaire du
dépôt, cohérente avec le principe que `console` énonce déjà pour `firewalld`
et `python3-jinja2` : *« a package carries its own dependencies »*.

**Le contrat inter-packages est donc explicite** :
`/opt/nivuus-packages/console/guest/winrm_exec.py`, appelable par un package
qui déclare `requires.packages: [console]`. Il lit son mot de passe dans un
**fichier** (`/root/.config/nivuus/windows-admin.pass`), jamais dans `argv` —
« so it cannot leak into the process table or shell history ».

🔴 **DEUX CHEMINS D'ÉCRITURE COEXISTENT DANS LA VM, ET C'EST ASSUMÉ PLUTÔT
QUE MASQUÉ** : le **payload** sert la **reconstruction** de l'appliance —
elle se rebâtit sans que `desk` soit installé — et le **WinRM** sert
l'**installation de `desk` sur une VM déjà bâtie**. Prétendre qu'il n'y en a
qu'un obligerait à choisir entre une appliance qui ne se reconstruit pas et
un `desk` qui exige un reprovisionnement complet pour s'installer.

⚠️ **ProjFS exige un redémarrage de la VM**
(`Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS`).
L'activation ne le déclenche **pas d'elle-même** : elle pose la
fonctionnalité, constate l'état `RestartNeeded`, et **le dit**. Redémarrer la
VM d'un opérateur sans le lui demander serait un effet de bord qu'aucune
installation ne doit prendre.

🔴 **VB-AUDIO A UNE LICENCE PERSONNELLE SEULEMENT**, ce que ce dépôt porte
déjà comme un legs du chantier E. **Sa pose est donc une option du wizard,
désarmée par défaut, jamais un défaut** — et quand elle est désarmée, le
produit le dit déjà de lui-même par `mic: false` dans son message `ready`,
et le bouton du navigateur ne paraît pas.

### 4.6 Le wizard — quatre questions, et pas une de plus

| Clé | Type | Pourquoi elle est posée |
| --- | --- | --- |
| `admin_email` | texte | le compte initial ; rien ne peut le deviner |
| `admin_password` | `secret` | idem — et il ne transite que par stdin |
| `auth_mode` | choix `motdepasse` / `pomerium` | **un mode, pas un armement** : une valeur inconnue LÈVE, parce qu'un repli silencieux ferait tourner un mode sous le nom de l'autre, et que l'un des deux sens est une **ouverture** |
| `vb_audio` | booléen, défaut **faux** | la licence personnelle du §4.5 |

Tout le reste est **dérivé** (les adresses TURN, le port) ou **tiré au sort**
(le secret de jeton). ⚠️ **Ne jamais demander ce qu'on peut dériver** : une
question de plus est une occasion de plus de se tromper, et l'opérateur n'a
pas les éléments pour répondre.

⚠️ **En mode `pomerium`, deux gardes supplémentaires mordent, et le wizard
doit les faire mordre AVANT l'installation, pas après** :
`PLATEFORME_PROXY_DE_CONFIANCE` devient **obligatoire** (`lireConfig` refuse
de démarrer sans elle), et `PLATEFORME_HOTE` **refuse** les quatre écoutes
universelles `0.0.0.0`, `::`, `[::]` et `*`. Un montage à `0.0.0.0` qui
tournait la veille ne démarre plus du tout.

### 4.7 Les tests que le package porte

Sur le modèle du `Makefile` de `console` — dont le commentaire dit que ses
tests y ont été rapatriés **en préparation d'un `git filter-repo --path
console`** —, `desk` reçoit une cible `test` qui joue **ses** suites de
package : le manifeste se parse, le wizard se lit, chaque hook rend ce qu'il
promet, et le refus de `resolve` **se voit rouge**.

🔴 **Ces suites ne remplacent pas `scripts/verify-all.sh`** : elles éprouvent
le **packaging**, pas le produit. Les dix étapes existantes restent la
vérification du produit, et la cible `test` du package s'y ajoute au lieu de
s'y substituer.

## 5. Ce que ce chantier n'établit PAS

- **Que l'agent bâti en croisé FONCTIONNE** — voir §4.3. La compilation est
  mesurée ; l'exécution ne l'est pas, et aucune ligne de cette conception ne
  doit se lire comme si elle l'était.
- **Aucun des douze items du lot 3.** Ils attendent la VM que ce chantier
  rend atteignable.
- **Que l'installation ait jamais été jouée de bout en bout** sur une machine
  neuve : le plan devra dire comment elle s'éprouve, et sur quoi.
- **Le sort du montage CIFS `/media/vm`** et de `scripts/build-agent.sh`, qui
  visent une VM de développement que l'appliance n'est plus. Les retirer ou
  les garder est une décision qui appartient au propriétaire du dépôt, et ce
  chantier ne la prend pas en douce.
