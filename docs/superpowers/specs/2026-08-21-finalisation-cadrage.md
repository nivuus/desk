# Finaliser le projet — le cadrage des quatre lots

> **21 août 2026.** Ce document ne conçoit rien : il **partitionne** le § « Legs
> ouverts » de `CLAUDE.md` selon **qui peut fermer chaque item**, et ordonne le
> travail en quatre lots. Chaque lot obtient sa propre spec, son plan et sa
> recette.
>
> 🔴 **LA QUESTION QUI DÉCIDE N'EST PAS « QUE RESTE-T-IL À FAIRE » MAIS « QUI
> PEUT LE FAIRE ».** Un tiers des legs ne se ferment par aucune quantité de
> code : ils attendent un œil, une oreille, un navigateur avec un humain
> derrière, ou un arbitrage du propriétaire. Les mêler aux autres produirait un
> plan dont une partie ne peut pas aboutir, et dont l'échec se lirait comme un
> manque de rigueur.

## Critère de fin, tel que le propriétaire l'a posé

**Un humain ouvre le produit** : un navigateur atteint le hub derrière le
proxy, et lance une session. C'est le lot 1. Les trois autres suivent, avec un
point d'arrêt après le lot 1.

## Les quatre lots

| Lot | Ce qu'il ferme | Ce qu'il exige |
| --- | --- | --- |
| **1** | la page derrière Pomerium, et la porte de `/auth/moi` | rien d'autre que le dépôt |
| **2** | les neuf legs fermables sans VM ni humain | rien d'autre que le dépôt |
| **3** | la campagne de mesure sur la VM Windows | la VM `Windows` allumée |
| **4** | la préparation des jugements humains | le dépôt ; **le verdict reste au propriétaire** |

---

## Lot 1 — La page derrière Pomerium

Spec : [`2026-08-21-page-derriere-pomerium-design.md`](2026-08-21-page-derriere-pomerium-design.md).

Un servant statique dans la plateforme, les en-têtes de document qui vont avec,
et la garde d'adresse source sur `/auth/moi`.

🔴 **Il retire le blocage ① du critère ⑦ — celui qui était un défaut de
CONCEPTION, non une limite de recette. Le blocage ② (le flux OAuth exige un
humain) demeure, et relève du lot 4.**

---

## Lot 2 — Les neuf legs fermables sans VM ni humain

Tous vérifiables par `verify-all.sh` et les deux passes Vitest. Ordre proposé :
la sécurité d'abord, la dette d'outillage ensuite, le cosmétique en dernier.

| # | Legs | Sous-projet | Nature |
| --- | --- | --- | --- |
| 2.1 | `GET /vm`, `POST /session` et le relais `/signal` ne sont **freinées par rien** | ⑤ | sécurité |
| 2.2 | Le contrat de `SIGNALING_URL` **n'est figé par aucun test** — y écrire `/signal` casserait l'enrôlement en silence, et le test existant passe une base propre, donc n'éprouve pas ce cas | auth-pomerium | correctness |
| 2.3 | Le canal `Message` du registre est **non borné**, et trois sous-blocs l'ont aggravé | ① | robustesse |
| 2.4 | Le magasin d'applications et le cache du pont ne sont **jamais nettoyés** — le disque grossit | ④ ③ | robustesse |
| 2.5 | **Douze pilotes de recette** visent la racine que `auth-pomerium` a fermée (`accent-a1`, `micro-e3` et son `injection-e3.js`, `pont-fichiers` f1→f5, `presse-papier` p1→p3, plus `journaux-micro-e2/pilote-recette-e2.mjs`) | auth-pomerium | outillage |
| 2.6 | `--accent-fenetre` **n'est déclaré nulle part et peint par rien** | ⑥ | défaut |
| 2.7 | `client/src/design/tokens.css` est à **300/300, marge nulle**, avec onze lecteurs qui le nomment par son chemin | ⑥ | dette de taille |
| 2.8 | Les galeries n'ont **aucun test** | ⑥ | couverture |
| 2.9 | Le **WCO** (Window Controls Overlay) n'a jamais été rendu | ⑥ | fonction absente |

⚠️ **2.5 est un chantier à part, pas une retouche** : douze fichiers, et
`?signaling=` reste **explicite** — il ne reçoit pas le suffixe `/signal`. Une
substitution aveugle casserait les pilotes autrement.

⚠️ **2.7 ne se règle pas par compression** : `CLAUDE.md` l'interdit nommément.
C'est une extraction, et la marge regagnée se reperd si on la traite comme
acquise — payé six fois.

---

## Lot 3 — La campagne sur la VM Windows

🔴 **Ce lot dépend d'une VM qui s'éteint seule, par deux mécanismes distincts**
(hibernation dans l'invité ; `libvirtd --timeout 120` qui emporte le domaine).
`virsh list --all` avant et après chaque séquence longue, et le compteur
d'extinctions relevé.

| # | Ce qui reste dû | Sous-projet |
| --- | --- | --- |
| 3.1 | 🔴 **La latence de bout en bout — jamais mesurée par aucun sous-bloc depuis D1** | transverse |
| 3.2 | Le **propriétaire mono-fenêtre n'existe pas** — ni presse-papier, ni accent | ① |
| 3.3 | Le **chemin d'extinction propre du superviseur n'a jamais été exercé** ; c'est ce qui laisse des sorties virtuelles et des racines ProjFS orphelines | D |
| 3.4 | Les **deux replis micro livrés et jamais courus** ; `MICRO_FAUTE_ECRITURE` **jamais armée** | E |
| 3.5 | Aucune **cause naturelle** de mort de capture audio | D |
| 3.6 | Les trois murs du pont : **~33 Kio/s**, **aucun fichier > 128 Kio**, **aucun listage > ~3 150 entrées** | ③ |
| 3.7 | 🔴 L'idiome **« temporaire + renommage » jamais exercé sur un éditeur réel** — *le seul chemin par lequel une sauvegarde peut se perdre en silence* | ③ |
| 3.8 | Un renommage fait **disparaître un répertoire frère** (préexistant ; le cache le prolonge) | ③ |
| 3.9 | **71 applications restent `NonMesuree`** ; une icône qui change sans que le raccourci change n'est jamais revue | ④ |
| 3.10 | Le maillon fautif du `Resize` **non identifié** | D |

⚠️ **3.7 est le seul item de ce lot dont l'échec perdrait des données de
l'utilisateur.** Il passe en tête si le lot est réduit.

---

## Lot 4 — Préparer les jugements humains

🔴 **Je ne peux rendre aucun de ces verdicts.** Ce lot livre l'instrument, la
marche à suivre et le lieu où consigner le jugement ; il ne livre pas le
jugement.

| # | Ce qui attend un humain | Ce que le lot prépare |
| --- | --- | --- |
| 4.1 | **Le critère ⑦** — la page derrière Pomerium, dans un navigateur | la marche à suivre exacte, une fois le lot 1 livré |
| 4.2 | **25 jugements visuels sur ⑥** — aucune page jamais ouverte par un œil, de S1 à S4 | la galerie servie, les 25 jugements listés et numérotés, un endroit pour les consigner |
| 4.3 | 🔴 **Personne n'a écouté le micro** — le critère de fin d'E n'est atteint que par un juge logiciel | un banc d'écoute, et ce qu'il faut écouter |
| 4.4 | Le **niveau 2** du presse-papier — qu'un humain puisse réellement coller | le protocole, et pourquoi il n'est pas mesurable autrement |
| 4.5 | **La calibration de toutes les constantes** — `BPP_MIN` et le `fps` de `Config` (qui se recalibrent **ensemble**), `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`, les cinq du micro, les quatre du pont, le plafond de poids CSS, les paramètres `scrypt`, `SEUIL_INJOIGNABLE_MS` et `PERIODE_BATTEMENT` (⚠️ qui se recalibrent ensemble **et vivent dans deux dépôts distincts**) | le tableau, la plage plausible de chacune, et le geste qui la juge |
| 4.6 | `showDirectoryPicker()` **jamais appelé** — aucune commande CDP n'accepte un sélecteur de fichiers ; la parade nommée est `Xvfb` + `xdotool`, consentie en D8 et jamais suivie d'effet | le montage `Xvfb` + `xdotool`, à défaut du verdict |

---

## Ce qui n'entre dans aucun lot

### Décisions qui appartiennent au propriétaire

Aucune n'est une correction, et **aucun lot ne doit les prendre en douce.**

- 🔴 **L'écho acoustique**, trois voies : ① rassembler la restitution (**défait
  par D7**) ; ② une AEC côté agent (**que la spec exclut nommément**) ; ③ le
  casque, **dit au bon moment** — la seule dont le défaut mesuré ait encore
  besoin, et la seule livrable par un mécanisme déjà construit.
- 🔴 **TURNS sur 443 n'est pas livré**, donc la cible « réseaux restrictifs »
  n'est pas couverte.
- 🔴 **La scalabilité horizontale est impossible par conception** — quatre états
  de routage vivent en mémoire, et `--scale plateforme=2` donne une panne
  **muette** que rien n'empêche.
- Le **secret d'enrôlement en clair sur la VM** (rotation possible, retrait
  non).
- Le **jeton dans `localStorage`**, aucun cookie livré.
- La **portée de la règle des 500 lignes** : le § « Portée » ne liste que
  `client/src/`, la commande attrape `client/verify-webrtc.mjs`. Signalé depuis
  D10, jamais tranché.
- **`FilterAdministratorToken=1` sur la VM** — sans lui, la porte d'élévation de
  G3 reste non mesurable.
- **L'installabilité du hub** : un `<img src>` ne porte pas d'`Authorization`,
  ni un `<link rel="manifest">`. La rendre installable exige **d'ouvrir une
  route aujourd'hui authentifiée** — une décision de sécurité, pas une
  correction. Ses `file_handlers` restent inertes tant qu'elle n'est pas prise.

### Ce qui ne se ferme pas

- Les **six constats de revue perdus** avec un rapport gitignoré —
  définitivement. La leçon est déjà inscrite : *une preuve ne doit jamais vivre
  dans un rapport gitignoré.*
- Les **trois couches inconnues du chantier D** : le plafond de 8 encodeurs, le
  plafond de 4 processus tenant une duplication, le mécanisme de l'abandon du
  mutex DXGI. **Contournées, jamais expliquées.** Et rien ne nettoie le
  registre, dont la pollution fait naître une sortie à la mauvaise taille — par
  GUID, et toujours le quatrième, inexpliqué.
- L'**A/B sur `set_desired_bitrate`** — écarté par décision, condition de
  réouverture nommée : charge d'hôte contrôlée, ≥ 8 paires.
- La **licence VB-Audio est personnelle seulement** : une limite déclarée, pas
  un défaut.
- Le son d'une **autre application n'est pas annulé** (−13 dB, donc
  **amplifié**), et le rééchantillonnage 48 → 44,1 kHz du câble est un
  **remède inapplicable**. ⚠️ Ce qui, lui, **est** corrigeable et relève du lot
  2 ou 3 : *rien ne le dit à l'utilisateur.*

---

## Règles qui valent pour les quatre lots

Elles ne sont pas décoratives : chacune nomme un patron que ce dépôt a payé
plusieurs fois.

1. 🔴 **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Exécuter
   le contrôle, **puis provoquer l'état qu'il doit dénoncer** et lire **quelle
   assertion** rougit.
2. 🔴 **Un zéro n'est interprétable qu'avec un témoin négatif.**
3. 🔴 **Corriger une affirmation exige de la CHERCHER** — `grep -n` avant
   d'écrire, et relire les places une par une après. Payé neuf fois.
4. 🔴 **Extraire, jamais comprimer**, et dans une tâche **dédiée, avant** celle
   qui ajoute.
5. 🔴 **Relever les tailles APRÈS la dernière édition de la ronde**, revue
   transverse comprise.
6. 🔴 **Une revue par tâche ne peut pas voir un défaut qui franchit une
   frontière de tâche** — d'où une **revue transverse en fin de lot**.
7. ⚠️ **Ne jamais `git add -A` dans un arbre partagé**, et un pathspec de
   répertoire ne prend pas le module homonyme.
