# Lot 3 — la campagne sur la VM Windows : conception

> **28 août 2026.** Troisième des quatre lots du
> [cadrage de finalisation](2026-08-21-finalisation-cadrage.md). Le lot 1
> (la page derrière Pomerium) et le lot 2 (les huit legs fermables sans VM)
> sont fusionnés dans `main` (commit de fusion `084bc6c`). Le lot 4, les
> jugements humains, reste après celui-ci.
>
> **Ce lot est une CAMPAGNE DE MESURE, pas un lot de correction** — avec une
> exception nommée au §2, et une seule.

## 1. Objet

Douze items restent dus, qui ont en commun de n'être atteignables que sur la
VM Windows. Dix viennent du cadrage (3.1 → 3.10) ; deux sont légués par le
lot 2 : le **rejeu des douze pilotes de recette** réparés à l'aveugle vers
`/signal`, et le **comportement en charge réelle** de la file bornée du
capteur.

**But du lot** : que chacun de ces douze items cesse d'être une affirmation et
devienne un chiffre daté, obtenu par un montage dont la rouge a été vue rouge.

## 2. Ce que ce lot corrige, et ce qu'il se contente de mesurer

**Il corrige un seul cas : celui où la mesure montre une PERTE DE DONNÉES de
l'utilisateur** — c'est-à-dire l'item 1 (§4.1, l'idiome « temporaire +
renommage »), et lui seul selon le cadrage. La correction suit la règle du
dépôt : la rouge est vue rouge **avant** le remède, et le remède la fait
verdir.

**Tout le reste est mesuré, chiffré, et légué nommément.** Les murs du pont
(~33 Kio/s, 128 Kio, ~3 150 entrées) touchent à la conception du transport ;
les relever n'est pas les déplacer, et ce lot ne prétend pas les déplacer.

⚠️ **Un item échappe déjà à cette partition, et le cadrage ne l'avait pas vu**
— voir §4.5 : le chemin d'extinction propre du superviseur **n'a pas
seulement jamais été exercé, il N'EXISTE PAS**. Relevé du 28 août 2026 :
`scripts/stop-agent.sh` ne fait que `schtasks /end` puis
`Stop-Process -Name agent -Force`, et aucun gestionnaire de signal console
(`SetConsoleCtrlHandler`, `ctrlc`) n'existe dans `agent/src`. Les `Drop` qui
libèrent les sorties virtuelles (`moniteurs_virtuels.rs:109`) et les
processus enfants (`superviseur/lanceur.rs:481`) sont donc **structurellement
inatteignables** par l'outillage livré. L'item devient « constater l'absence,
mesurer ce que le `Force` laisse derrière, PUIS décider » — la décision de
construire le chemin est arbitrée dans sa tâche, pas ici.

## 2 bis. Un blocage matériel que ni le cadrage ni cette conception n'avaient vu

🔴 **RELEVÉ LE 28 AOÛT 2026, PENDANT L'ÉCRITURE DU PLAN.** Le dépôt a été
déplacé de `/home/mallanic/Projects/Guacamole` vers
`/home/mallanic/Projects/Nivuus/packages/desk`. **48 scripts exécutables des
répertoires de journaux portent l'ancien chemin EN DUR** — relevé par
`grep -rl "Projects/Guacamole" docs/superpowers/plans/journaux-*/ | grep -E '\.(sh|mjs|js|ps1|py|mts)$' | wc -l`,
**à relancer, jamais à recopier**. Vingt d'entre eux vivent dans les
répertoires des **douze pilotes** que ce lot doit rejouer : `jouer-f2.sh`
ouvre sur `RACINE=/home/mallanic/Projects/Guacamole`, et ce répertoire existe
encore, **vide**.

**Conséquence sur l'ordre du lot** : aucun rejeu n'est possible avant cette
réparation, qui devient la **tâche 1 du plan**, juste après le harnais. Un
rejeu tenté sans elle échouerait pour une raison qui n'a rien à voir avec ce
qu'il mesure — et le dépôt a déjà payé de classer une rouge au lieu de la
diagnostiquer.

⚠️ **Le remède n'est PAS une substitution du chemin** : c'est une **racine
dérivée** (`git rev-parse --show-toplevel` pour les `.sh`,
`fileURLToPath(import.meta.url)` pour les `.mjs`, un **paramètre** pour les
`.ps1`, la VM n'ayant pas le dépôt) — patron que
`journaux-accent-a1/instrument/monter-a1.sh` emploie déjà correctement.

🔴 **LES JOURNAUX NE SE RÉÉCRIVENT PAS.** Les `.log`, `.json` et `.md` qui
portent l'ancien chemin sont des **pièces datées** : ils disent où la mesure a
été faite le jour où elle l'a été. Les réécrire serait **falsifier une
pièce**, ce que ce dépôt s'interdit nommément.

⚠️ **Le même mécanisme avait déjà mordu ailleurs, et pour la même raison** :
les unités compilées de `target/` portaient ce chemin, et une cinquantaine de
tests Rust échouaient sur un `testdata` introuvable — diagnostiqué le même
jour, avant la fusion de `main`, remède `cargo clean -p agent -p proto`.

## 3. Le harnais de campagne — tâche 0, avant tout le reste

🔴 **La VM s'éteint seule, par deux mécanismes distincts** : une hibernation
initiée DANS l'invité (`Kernel-Power` 187/42), et l'HÔTE qui tue QEMU
(`libvirtd --timeout 120`, qui s'arrête sur inactivité et emporte le
domaine). Douze items vont s'appuyer sur cette VM pendant des séquences
longues.

Le harnais est **un point unique** que chaque item appelle. Il fait, dans cet
ordre :

1. `virsh list --all`, relevé **avant** ; démarrage idempotent si éteinte.
2. Attente de WinRM (`/dev/tcp/192.168.3.2/5985`).
3. **Puis** attente d'un **accès réel** à `/media/vm` — `ls /media/vm/dev`,
   jamais `mountpoint -q` : l'entrée CIFS persiste dans la table de montage
   VM éteinte et connexion morte.
4. `Get-Process agent` **avant chaque tentative**, y compris échouée : un
   agent survivant tient `agent.log`, et l'on relit alors le journal de la
   tentative précédente en croyant lire le sien.
5. Purge des sorties virtuelles et racines ProjFS orphelines
   (`MULTIFENETRE_VDD_PURGE=1`), qui survivent à tout arrêt brutal.
6. `virsh list --all`, relevé **après**, et **compteur d'extinctions**
   comparé : une VM morte en cours de séquence rend une mesure indiscernable
   d'un produit en panne.

⚠️ **Le harnais ne juge rien** : il rend l'état, et c'est l'item qui juge. Un
harnais qui classerait lui-même une séquence « valide » masquerait la
distinction entre « la mesure a été faite » et « la VM a tenu ».

## 4. Les douze items, dans l'ordre du risque

### 4.1 — Item 1 (3.7) : « temporaire + renommage » sur un éditeur réel

**Ce qui est dû** : *le seul chemin par lequel une sauvegarde peut se perdre
en silence*, jamais exercé sur un éditeur réel, alors que le pont est livré.

**Le corpus de la VM** (relevé dans `agent/testdata/gapps-corpus-vm.json`)
porte `C:\Windows\system32\notepad.exe`,
`C:\Program Files\Windows NT\Accessories\wordpad.exe` et
`C:\Program Files\Microsoft VS Code\Code.exe` — **ni Word ni LibreOffice**.

**Montage, dans cet ordre** :

1. **Observer avant de supposer.** Ouvrir un fichier de la racine du pont
   avec chacun des trois éditeurs, le modifier, l'enregistrer, et relever ce
   que le pont voit — par les notifications ProjFS elles-mêmes
   (`agent/src/pont/notifications.rs`), pas par une hypothèse sur l'éditeur.
   Le relevé nomme, pour chaque éditeur, la **séquence exacte** d'opérations.
2. **Si aucun des trois n'emploie l'idiome**, l'armer explicitement — VS Code
   porte un réglage de sauvegarde atomique — et **le dire** : le chiffre
   changerait de sens.
3. Mesurer le résultat : **le contenu enregistré arrive-t-il au poste local,
   entier, et sous le bon nom ?**

**Critère** : après enregistrement, le fichier du poste local porte
**exactement** l'octet-à-octet du contenu enregistré, et **aucun temporaire
n'est laissé derrière**.

**Sa rouge** : `PONT_MUTATION=0` refuse renommage et suppression au `PRE_` —
un enregistrement par temporaire + renommage doit alors **échouer côté VM**,
la source rester **présente** et la cible **absente**. ⚠️ Ne pas employer
`PONT_ECRITURE=0` à sa place : il pose `inscriptible=false`, ce qui fait
refuser pour une **autre** raison — une rouge de F3 y a déjà été
disqualifiée.

**Si la mesure montre une perte** : ce lot corrige, et la rouge est rejouée
sur le remède.

### 4.2 — Item 2 (3.6) : les trois murs du pont

**Ce qui est dû** : les trois chiffres de F4 — **30 à 33 Kio/s** soutenus,
**une lecture de plus de 128 Kio échoue**, **aucun listage de plus de
~3 150 entrées n'aboutit** (18 à 24 s à ce rang) — datent du 21 août 2026 et
n'ont pas été relevés depuis F5, qui a ajouté le cache d'énumération.

**Montage** : `PONT_MESURE=1` arme l'émission de la ligne de recensement
(`agent/src/pont/latence.rs`). ⚠️ **Les compteurs sont CUMULATIFS** : une
mesure se lit par **différence** entre deux recensements, jamais sur une
ligne isolée. Paliers de taille de fichier en puissances de deux jusqu'au
refus ; paliers d'entrées de répertoire jusqu'au refus ; débit relevé sur un
palier **plusieurs fois plus long** que la période de recensement (10 s).

**Critère** : les trois murs sont **re-situés** sur le produit
d'aujourd'hui — chacun avec sa valeur, son écart au relevé de F4, et la
raison de l'écart s'il y en a un.

**Sa rouge** : le bras « armé, aucun geste » doit rendre `n:0` partout. C'est
lui qui rend discriminant le bras « armé, un geste » — le contrôle qui vaut
n'est pas que la ligne sorte, c'est qu'elle **compte ce qu'elle dit**.

### 4.3 — Item 3 (3.8) : le répertoire frère qui disparaît

**Ce qui est dû** : un renommage fait disparaître un répertoire frère.
F5 a établi par A/B (`PONT_CACHE=0`) que le défaut est **préexistant** et que
le cache ne fait que le **prolonger** — la cause, elle, n'a jamais été
cherchée.

**Montage** : reproduire la disparition, puis **mesurer par maillon** —
disculper un maillon ne désigne pas le coupable suivant. Les maillons : la
notification ProjFS reçue, ce que le pont en fait, ce que le cache retient,
ce que le poste local affiche.

**Critère** : le maillon fautif est **nommé**, ou l'échec à le nommer est
écrit avec ce qui a été éliminé et par quelle mesure.

**Sa rouge** : `PONT_CACHE=0` doit faire **réapparaître** le frère au listage
suivant sans `Rafraichir` — l'A/B d'attribution de F5, rejoué.

### 4.4 — Item 4 : le rejeu des douze pilotes (legs du lot 2)

**Ce qui est dû** : les douze pilotes ont été réparés vers `/signal` par
**lecture du code**, et **aucun n'a été rejoué** — la VM était hors périmètre
du lot 2. La lecture ne peut trancher ni que la plateforme réponde sur
`/signal` avec les en-têtes attendus pour ces origines, ni que le
comportement métier de chaque recette tienne encore.

**Les douze** : `accent-a1` · `micro-e2` · `micro-e3` (le pilote et son
`injection-e3.js`) · `pont-fichiers` f1 à f5 · `presse-papier` p1 à p3.

**Montage** : chacun rejoué **tel qu'il est**, sans adaptation autre que ce
que le harnais fournit. ⚠️ **Deux familles**, et les confondre casse
l'enrôlement : les pilotes à usage **navigateur seul** suffixent `/signal` ;
ceux à **usage double** gardent `SIGNALING_WS` intacte pour `SIGNALING_URL`
de l'agent — contrat figé par le test
`une_base_portant_deja_signal_casse_le_canal_agent`.

**Critère** : pour chacun, **la session s'établit** et le verdict métier
d'origine est **retrouvé** — ou l'écart est nommé.

**Sa rouge** : un pilote pointé vers la racine nue `/` (l'URL d'avant
`auth-pomerium`) doit **échouer à établir la session**. Sans ce bras, un vert
ne distinguerait pas la réparation d'un chemin qui n'a jamais compté.

### 4.5 — Item 5 (3.3) : l'extinction propre du superviseur

🔴 **Le chemin n'existe pas** — voir §2. L'item se joue en trois temps :

1. **Constater l'absence**, par la commande qui l'établit, et non par mémoire.
2. **Mesurer ce que le `Stop-Process -Force` laisse derrière** : sorties
   virtuelles orphelines, racines ProjFS, processus enfants survivants,
   comptés après extinction, VM restée allumée.
3. **Décider dans la tâche** si le chemin se construit ici : un handler de
   signal console qui laisse courir les `Drop` déjà écrits. Le critère de la
   décision est **la taille du remède**, pas l'envie de le faire — s'il ne
   tient pas dans une tâche, il devient un legs chiffré, avec le compte
   d'orphelins qui le justifie.
   ⚠️ **Cette décision ne se prend pas en douce** : si le remède déborde
   d'une tâche, la tâche s'arrête sur le legs chiffré, et le choix de le
   construire revient au propriétaire du dépôt.

**Critère** : le compte d'orphelins laissés par une extinction brutale est un
**chiffre**, mesuré deux fois.

**Sa rouge** : après purge, ce compte doit être **zéro** — un zéro qui reste
zéro à l'extinction suivante dirait que la mesure ne mesure rien.

### 4.6 — Item 6 : la file du capteur en charge réelle (legs du lot 2)

**Ce qui est dû** : la file bornée par coalescence (`PROFONDEUR_MAX = 64`,
`agent/src/capteur/sommeil/file.rs`) et sa trace de refus au palier — *file
d'une session pleine : message REFUSE (trace au palier, puissance de deux)* —
**n'ont jamais été vues sortir dans un `agent.log` réel**. C'est un contrôle
qu'on n'a jamais vu rouge en conditions réelles, et le lot 2 le déclare
lui-même.

**Montage** : boucher réellement une file — une session dont le fil de
fenêtre ne consomme plus, pendant que les autres poussent. Le nombre de
fenêtres et la cadence sont dérivés des constantes du code, pas choisis.

**Critère** : la trace **sort**, portant `session_cible`, le compte de refus
et `profondeur_max=64` ; et le palier progresse bien **en puissances de
deux**.

**Sa rouge** : le régime nominal, mêmes fenêtres, consommation normale, doit
rendre **zéro refus** — avec un témoin négatif prouvant que le produit
travaille (les lignes d'attache au capteur).

⚠️ **Un `Sommeil` refusé reste PERDU** (tracé, non réémis) : c'est un legs
déclaré par le lot 2, que ce lot **mesure** sans le fermer — le remède
casserait la preuve de terminaison de la boucle actuelle.

### 4.7 — Item 7 (3.1) : la latence capture → affichage

🔴 **Jamais mesurée par aucun sous-bloc depuis D1.**

**Ce sur quoi elle se juge** : l'agent annonce déjà **l'instant de capture**
au pair par le **sender report RTCP** (`agent/src/transport/piste_video.rs`,
tenu par un test). Côté navigateur, `requestVideoFrameCallback` rend
`captureTime` pour chaque image rendue. **La soustraction est la latence** —
image par image, sans caméra, sans OCR.

⚠️ **Le client ne l'appelle pas aujourd'hui** : c'est un **instrument de
recette** à écrire, pas un changement de produit, et le document doit le
dire.

**Montage** : une **mire animée à cadence connue et affichée par la source
elle-même** — une mire immobile ne produit aucune image, Desktop Duplication
n'émettant qu'au changement. Chrome avec
`--disable-background-timer-throttling`,
`--disable-backgrounding-occluded-windows` et
`--disable-renderer-backgrounding` pour toute mesure de plus de 5 minutes ;
visibilité et focus **imposés page par page** ; toute évaluation CDP
**bornée**, la page portant un flux WebRTC actif.

**Critère** : une **distribution** (médiane et queue), pas une moyenne, sur
au moins deux exécutions — et le **régime** dans lequel elle est prise est
nommé (une fenêtre, puis N).

**Sa rouge** : `scripts/netem.sh adsl` doit **déplacer** la distribution vers
le haut. ⚠️ **Toujours reposer `off`.** Sans ce bras, un chiffre plausible
serait indiscernable d'un instrument qui mesure autre chose.

⚠️ **Ce que cet item n'établit PAS** : la latence **verre à verre**. Le
maillon d'affichage du navigateur, après `captureTime`, n'est pas dans la
soustraction, et la boucle d'entrée n'y est pas du tout.

### 4.8 — Item 8 (3.2) : le propriétaire mono-fenêtre

**Ce qui est dû** : ni le presse-papier ni l'accent n'ont de propriétaire en
mode **mono-fenêtre** — les deux mécanismes vivent dans le capteur, qui est
le propriétaire depuis D1, et le chemin sans `CAPTEUR` ni `SUPERVISEUR` n'a
jamais été exercé pour eux.

**Critère** : en mono-fenêtre, un texte copié dans la VM atteint-il le
navigateur, et l'accent de la fenêtre est-il annoncé ? La réponse est un
**oui/non mesuré**, avec le compte de messages.

**Sa rouge** : `PRESSE_PAPIER=0` et `ACCENT=0` — le contrôle qui vaut est le
**zéro de messages**, jamais la trace de désarmement, et c'est le bras SANS
la variable qui rend ce zéro discriminant.

### 4.9 — Item 9 (3.4) : les deux replis micro, et la faute jamais armée

**Ce qui est dû** : les trois branches de repli de `resoudre`
(`agent/src/micro/boucle_locale.rs`) sont **livrées et jamais courues**, et
`MICRO_FAUTE_ECRITURE` (`agent/src/wasapi/ecriture.rs`) n'a **jamais été
armée** — le chemin d'échec d'écriture WASAPI n'a jamais couru.

**Montage** : `MICRO_PERIPHERIQUE` posée vers un nom **introuvable**, puis
vers une sous-chaîne **ambiguë** ; puis `MICRO_FAUTE_ECRITURE=<n>`, budget
**global au processus**.

**Critère** : chaque repli **journalise ce qu'il dit journaliser**, `mic:
false` où il le doit, et l'inventaire des périphériques paraît.

**Sa rouge** : le cas nominal (`MICRO_PERIPHERIQUE` absente, donc la
désignation intégrée `"VB-Audio"`) rend `mic: true` et la trace *cable de
rendu retenu*. ⚠️ **Comparer la valeur RETENUE**, jamais la seule présence de
la ligne.

### 4.10 — Item 10 (3.5) : une cause naturelle de mort de capture audio

**Ce qui est dû** : `AUDIO_FAUTE_LECTURE` et `AUDIO_FAUTE_RECONSTRUCTION`
établissent que le **remède** fonctionne ; **aucune cause naturelle** n'a
jamais été observée sur cette VM.

**Montage** : provoquer les causes plausibles — changement de périphérique de
rendu par défaut en cours de session, mise en veille du point de
terminaison, arrêt du service audio — et relever si la capture meurt.

**Critère** : soit une cause naturelle est **nommée et reproduite**, soit
l'échec à en trouver une est écrit avec **la liste de ce qui a été tenté**.

⚠️ **Un échec à trouver n'est pas une preuve d'absence**, et le document doit
le dire dans ces termes.

### 4.11 — Item 11 (3.9) : les 71 applications `NonMesuree`

**Ce qui est dû** : 71 applications du catalogue restent `NonMesuree`, et une
icône qui change **sans que le raccourci change** n'est jamais revue.

**Montage** : lancer la mesure sur le corpus réel de la VM ; puis muter une
icône **sans toucher au raccourci** et observer si le catalogue la revoit.

**Critère** : le compte de `NonMesuree` **après** campagne, et le verdict
oui/non sur l'icône mutée.

**Sa rouge** : `APPS_SURVEILLANCE=0` — le contrôle qui vaut est l'**absence
de toute ligne `racine surveillée`**, jamais la trace de mode, qui prouve
seulement que la variable a atteint le processus.

### 4.12 — Item 12 (3.10) : le maillon fautif du `Resize`

**Ce qui est dû** : le maillon fautif n'a jamais été identifié — c'est le
plus spéculatif des douze, d'où sa place en queue.

**Montage** : instrumenter **par maillon** le chemin d'un `Resize`, de
l'annonce du navigateur à la taille effectivement encodée.

**Critère** : le maillon est **nommé**, ou la liste des maillons disculpés
est écrite, chacun avec la mesure qui le disculpe.

## 5. Le protocole, qui vaut pour les douze

- **Un critère écrit AVANT la mesure**, et **sa rouge jouée** : provoquer
  délibérément l'état que le contrôle doit dénoncer, et vérifier qu'il le
  dénonce. Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.
- **Une rouge restée verte se DIAGNOSTIQUE, elle ne se classe pas.**
- **Deux exécutions par bras**, minimum.
- **Tout zéro s'accompagne d'un témoin négatif** relevé dans le même relevé.
- **Toute variable neuve part dans une TÂCHE DÉDIÉE**, et le contrôle qui
  vaut est de lire sa ligne dans le `run-agent.ps1` **GÉNÉRÉ sur la VM** —
  jamais de tracer le code.
- **Les journaux bruts sont VERSIONNÉS**, sous
  `docs/superpowers/plans/journaux-lot3-<item>/` : une preuve ne vit jamais
  dans un rapport gitignoré.
- **Ne jamais fabriquer une pièce** : ne jamais réutiliser la sortie d'une
  commande pour répondre à la question d'une autre sans la relancer.
- `git status --porcelain agent/ proto/` **avant chaque build** :
  `build-agent.sh` rsynchronise l'arbre entier.
- `cargo clean --release -p proto -p agent` — **les deux crates** — avant
  toute compilation qui touche `proto/`.
- **Tuer par PID relevé**, jamais par motif : `pkill -f` depuis un shell dont
  la ligne de commande contient le motif tue le shell.
- `unset -f chpwd` avant toute collecte.

## 6. Livrables

| Quoi | Où |
| --- | --- |
| Branche de travail | `campagne-vm`, depuis `main` |
| Cette conception | `docs/superpowers/specs/2026-08-28-lot3-campagne-vm-design.md` |
| Le plan | `docs/superpowers/plans/2026-08-28-lot3-campagne-vm.md`, quatorze tâches |
| Journaux bruts | `docs/superpowers/plans/journaux-lot3-<item>/` |
| Résultats, par groupe | pont (items 1–3) · infrastructure (4–6) · média (7–10) · apps et `Resize` (11–12) |
| Clôture | un document de résultats de lot, et la **revue transverse** |
| `CLAUDE.md` | une ligne à l'index, les legs fermés **et** les legs neufs chiffrés |

## 7. Ce que ce lot n'établira PAS

- **Aucun jugement humain** : ni visuel sur ⑥, ni d'écoute sur E, ni le
  critère ⑦ derrière Pomerium. C'est le lot 4.
- **Aucune constante calibrée.** Les paliers et cadences de ce lot sont
  **dérivés des constantes du code**, jamais des propositions de valeurs.
- **Aucun mur du pont déplacé**, et aucune des trois couches inconnues du
  chantier D expliquée (plafond de 8 encodeurs, plafond de 4 processus,
  abandon du mutex DXGI).
- **La latence verre à verre**, ni la boucle d'entrée — voir §4.7.
- **`showDirectoryPicker()`** reste non appelé : aucune commande CDP
  n'accepte un sélecteur de fichiers, et la parade `Xvfb` + `xdotool` relève
  du lot 4.
