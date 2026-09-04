# Lot 3, item 1 (3.7) — « temporaire + renommage » sur un éditeur réel

🔴 **LE SEUL ITEM DU LOT DONT L'ÉCHEC PERDRAIT DES DONNÉES DE L'UTILISATEUR**,
et le seul chemin par lequel une sauvegarde peut se perdre **en silence**.

**Mesuré le 5 septembre 2026**, VM `Windows` (appliance), racine ProjFS
`C:\Users\Administrator\Mes Fichiers`, racine locale OPFS `Mes documents`.

⚠️ **Aucun chiffre de ce document ne se recopie sans relancer sa commande.**

---

## 1. Le verdict

🔵 **AUCUNE SAUVEGARDE N'EST PERDUE, ET AUCUN TEMPORAIRE N'EST LAISSÉ.**
Le Bloc-notes écrit **EN PLACE** dans la racine ProjFS, et l'enregistrement
arrive **entier** au poste local en ~56 ms.

| bras | poste local AVANT | poste local APRÈS | | acquittements |
| --- | --- | --- | --- | --- |
| **vert 1** | 44 o · `12f5b6f56509` | **70 o · `55e8b46d4266`** | CHANGÉ | **1** |
| **vert 2** | 44 o · `12f5b6f56509` | **96 o · `0fdf281c764f`** | CHANGÉ | **1** |
| **ROUGE** (`PONT_ECRITURE=0`) | 44 o · `12f5b6f56509` | **44 o · `12f5b6f56509`** | **INCHANGÉ** | **0** |

`ecriture acquittee : les octets sont sur le poste local
chemin="item1-sauvegarde.txt" octets=70 duree_ms=56` (bras 1),
`octets=96 duree_ms=57` (bras 2).

## 2. L'IDIOME : aucun temporaire, à AUCUN des trois instants

L'observateur relève les **NOMS** de la racine — pas un compte : *un temporaire
créé puis renommé laisserait le compte inchangé*. Trois inventaires par
exécution, **neuf au total**, tous identiques :

```
inventaire AVANT                                  : item1-sauvegarde.txt
inventaire PENDANT (editeur ouvert, apres Ctrl+S) : item1-sauvegarde.txt
inventaire APRES                                  : item1-sauvegarde.txt
nouveaux noms (APRES moins AVANT) :            (vide)
```

🔵 **L'inventaire PENDANT est celui qui compte** : un temporaire d'éditeur peut
ne vivre que le temps de l'enregistrement, et un relevé pris seulement après
coup ne le verrait jamais. Il est pris **l'éditeur encore ouvert**, 3 s après
`Ctrl+S`.

🔴 **CE QUE CELA AJOUTE À LA SONDE DU 21 AOÛT 2026, ET RIEN DE PLUS.** Cette
sonde (`journaux-pont-fichiers-f2/instrument/sonde-idiome.ps1`, réutilisée **par
lecture de son fichier d'origine**) avait établi que cinq outils écrivent en
place — mais dans un **répertoire NTFS ORDINAIRE**, ce qu'elle déclare
elle-même. Cet item porte la même question **DANS LA RACINE ProjFS**, où un
fournisseur de virtualisation aurait pu changer l'idiome. **Il ne le change
pas.**

## 3. La ROUGE, et pourquoi ce n'est PAS celle que le plan prescrivait

Le plan (étape 6) prescrivait `PONT_MUTATION=0`, en avertissant : *« ne pas
employer `PONT_ECRITURE=0` à sa place — une rouge de F3 y a été
DISQUALIFIÉE »*. **Cet avertissement vise le critère de F3, qui est un
RENOMMAGE.**

🔴 **Le critère de CET item n'est pas un renommage.** Il est « la sauvegarde
arrive-t-elle entière au poste local ? », et le § 2 vient d'établir qu'**aucun
renommage n'a lieu**. `PONT_MUTATION=0` désarme le renommage et la
suppression : sur un chemin qui n'en emprunte aucun, **il ne changerait
rien** — la rouge serait **VERTE**, pour une raison étrangère à ce qu'elle
prétend éprouver. C'est le patron « une rouge qui rougit (ou reste verte) pour
la mauvaise raison ».

Le désarmement qui mord sur le mécanisme réellement mesuré est
**`PONT_ECRITURE=0`** : le pont continue de détecter, de journaliser et de
compter les écritures dues, **et n'en pousse aucune**.

**Les trois contrôles du bras rouge, ensemble :**

1. **La variable atteint le processus, et à la bonne POSITION** — lue dans le
   `run-agent.ps1` **GÉNÉRÉ sur la VM** :
   `ligne 35 : $env:PONT_ECRITURE = '0'` contre
   `ligne 61 : & 'C:\nivuus\agent\agent.exe'`. Une ligne posée *après*
   l'invocation ne s'exécuterait jamais (piège payé quatre fois par ce dépôt).
2. **La trace de désarmement sort** :
   `WARN agent::pont::ecriture::fil: poussee d'ecriture DESARMEE
   (PONT_ECRITURE=0) : bras de banc, jamais une configuration livree`.
3. 🔴 **ET LE CONTRÔLE QUI VAUT N'EST NI L'UNE NI L'AUTRE** : c'est que
   **le poste local reste à 44 octets, sha256 inchangé**, pendant que l'invité
   croit avoir enregistré (148 octets côté VM, marqueur présent). **C'est
   littéralement la perte silencieuse que cet item cherche** — provoquée à
   dessein, pour prouver que les bras verts mesuraient bien ce mécanisme.

⚠️ **LA PREMIÈRE TENTATIVE DE CE BRAS ROUGE A ÉTÉ REJETÉE, ET NON CLASSÉE.**
Elle rendait `0` acquittement — le chiffre attendu — mais son pilote avait
échoué avant de monter le pont (`Uncaught (in promise)`) : **le zéro venait de
l'absence de pont, pas du désarmement**. Une rouge qui rougit pour la mauvaise
raison est indiscernable d'une bonne si l'on ne lit que son chiffre. Elle a été
rejouée après correction, pont **monté** (même bandeau que les bras verts).

## 4. Ce que cet item N'ÉTABLIT PAS

- 🔴 **UN SEUL ÉDITEUR A ÉTÉ ÉPROUVÉ, ET C'EST UNE LIMITE DE LA MACHINE.**
  Le plan en nommait trois. Relevé sur l'invité :
  `C:\Windows\system32\notepad.exe` → **True** ;
  `C:\Program Files\Windows NT\Accessories\wordpad.exe` → **False** ;
  `C:\Program Files\Microsoft VS Code\Code.exe` → **False**.
  ⚠️ **VS Code est précisément celui que le plan voulait ajouter** — pour son
  réglage de sauvegarde atomique — et **il n'est pas installé**. L'étape 4 du
  plan (armer cet idiome, et déclarer que le chiffre vient d'un idiome armé)
  est donc **NON JOUÉE**.
- ⚠️ **Word et LibreOffice** — les deux outils dont F2 disait qu'ils emploient
  l'idiome `temp+rename` — **ne sont pas installés non plus**. Le seul cas où
  la perte serait possible reste donc **non éprouvé sur cette machine**.
- ⚠️ **`showDirectoryPicker()` n'est jamais appelé** : l'instrument substitue
  une poignée **OPFS**, qui est une vraie `FileSystemDirectoryHandle` mais
  **n'a aucun modèle de permission**. `queryPermission`/`requestPermission`,
  le mode `readwrite` et l'activation utilisateur transitoire restent **non
  couverts** — limite héritée de F1/F2, inchangée.
- ⚠️ **Un drapeau d'origine sûre a été nécessaire.** La plateforme est servie
  en **HTTP clair** sur `192.168.3.1:3445`, et `navigator.storage` y est
  `undefined` (contexte non sûr, mesuré). Le pilote pose
  `--unsafely-treat-insecure-origin-as-secure`. En usage réel la page vit
  derrière Pomerium, donc en HTTPS ; **ce drapeau rétablit la condition du
  produit réel, il ne l'éprouve pas**.
- ⚠️ **UNE DIVERGENCE OBSERVÉE, NON EXPLIQUÉE, QUI N'EST PAS UNE PERTE.** Au
  début du bras 2, le poste local était à **44 o** (OPFS purgé et réécrit par
  l'injection) alors que l'invité lisait **70 o** — le contenu hydraté par
  ProjFS lors du bras 1, resté sur le disque de la VM. La purge du côté local
  n'est pas propagée à la racine ProjFS. **Le critère n'en souffre pas** (ce
  que l'invité a enregistré est bien arrivé), mais **le comportement d'une
  purge locale sur une racine déjà hydratée n'est pas mesuré ici.**
- ⚠️ **Aucune correction de produit n'a été écrite** — l'étape 8 du plan ne
  s'applique pas : la mesure ne montre **aucune perte**.

## 5. Les pièges payés par cet instrument, tous mesurés

1. **`\"` n'échappe rien en PowerShell** (l'échappement est l'accent grave) :
   le `.cmd` recevait `-Racine \"C:\Users\…` littéralement, cmd.exe coupait au
   premier espace. Symptôme : sortie **absente**, journal **vide**.
2. **L'opérateur `-f` a une précédence plus basse que la virgule** :
   `@("a", ("b") -f $x)` formate **l'array entier** et rend **une** chaîne. Le
   `.cmd` sortait sur **une** ligne, cmd.exe exécutait `echo` avec tout le
   reste en arguments, et `LastTaskResult` valait **0** — un succès apparent.
   🔵 **Le contrôle qui l'a attrapé est de RELIRE le fichier déposé EN
   COMPTANT SES LIGNES** (il doit en faire deux).
3. **Un ménage qui efface les pièces coûte une exécution** : la première
   rédaction supprimait `observer-editeurs.ps1`, `item1.cmd` et la sortie —
   au moment de diagnostiquer, il n'y avait plus rien à lire. La tâche part
   désormais, **les pièces restent**.
4. **Chaque exécution laissait un profil Chrome de ~100 Mio sous `tmpdir()`** :
   vingt-trois d'entre eux ont **rempli le tmpfs de 10 Gio** de cette machine,
   et le symptôme n'était pas « disque plein » mais **une commande qui ne rend
   RIEN**. Les deux pilotes du lot suppriment désormais leur profil.

## 6. Les instruments

| Fichier | Rôle |
| --- | --- |
| `instrument/injection-item1.js` | substitue **`showDirectoryPicker` seul** par une poignée OPFS ; tout l'aval est le produit. Ne peuple OPFS que depuis le **hub** (défaut d'instrument payé par F2) |
| `instrument/observer-editeurs.ps1` | **session 1** obligatoire, ASCII pur, racine **en paramètre** ; ouvre le Bloc-notes, écrit un marqueur, `Ctrl+S`, ferme, et relève les **trois** inventaires |
| `instrument/pilote-item1.mjs` | monte le pont par le **clic réel**, tient la session, relit le poste local (contenu + sha256) |
| `instrument/jouer-item1.sh` | une exécution complète, `--rouge` pour le bras désarmé |

Journaux bruts versionnés : `sequence-{1,2,rouge}.json`,
`observateur-{1,2,rouge}.txt`, `notifications-{1,2,rouge}.log`,
`pilote-{1,2,rouge}.log`.
