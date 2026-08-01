# Sous-bloc D1 — tranche verticale multi-fenêtres : résultats de la démonstration

**Date** : 1ᵉʳ août 2026. **Tâche 13** du plan
`docs/superpowers/plans/2026-08-01-multifenetres-tranche-verticale.md`.
**Branche** : `chantier-multifenetres-d1`. **Binaire mesuré** : construit le
1ᵉʳ août à 11:37:39 (heure hôte) depuis l'arbre de `484310f`
(`journaux-multifenetres-d1/compilation-tache13.log`).

Pièces versées, toutes dans `docs/superpowers/plans/journaux-multifenetres-d1/`
(UTF-8 sans BOM ; les journaux d'agent portent les séquences ANSI de `tracing`,
comme ceux des chantiers précédents) :

| Pièce | Ce qu'elle porte |
| --- | --- |
| `compilation-tache13.log` | La construction du binaire mesuré |
| `demonstration-{1,2,3}-*-agent.log` | Le journal de l'agent des trois exécutions exploitées (superviseur **et** enfants mêlés) |
| `demonstration-{1,2,3}-*-navigateur.log` | Le journal du pilote de recette, côté navigateur |
| `demonstration-2-fenetre-{bloc-notes,explorateur,firefox}.png` | Le point 1 : trois fenêtres navigateur, trois applications distinctes |
| `demonstration-3-verrouillage-pointeur-non-accorde.png` | Le point 4 : le bandeau de verrouillage du pointeur resté affiché (son nom d'origine, `f-point3-5-app.png`, est tracé à la l. 184 du journal navigateur de F) |
| `demonstration-relecture-bloc-notes.txt` | Le point 3 : la relecture `WM_GETTEXT` des deux Bloc-notes, vides |
| `environnement-signaling.txt` | L'environnement du processus qui écoute réellement sur `:8080` |
| `veille-prolongee-vm.txt` | Les mises en veille prolongée de la VM, et ce qu'elles excluent |
| `topologie-*.log`, `purge-finale.log` | Les contrôles d'absence de fuite, depuis un processus neuf |
| `pilote-recette.mjs`, `pilote-vm-it.sh` | L'instrument, dans son **état final** (celui de F) |

---

## 0. Le verdict en une phrase

**Le cœur de D1 marche, et il ne tient pas.** Une page-shell ouvre bien une
fenêtre navigateur par fenêtre Windows, chacune sur sa propre sortie
d'affichage virtuelle, chacune capturée et encodée par son propre processus,
chacune montrant **son** application et elle seule — **jusqu'à quatre
simultanément, vérifié**. Mais l'arrangement est détruit **à chaque fois qu'une
sortie virtuelle est créée**, c'est-à-dire à chaque nouvelle fenêtre : toutes
les sessions déjà en cours meurent ensemble.

⚠️ **Et il faut lire ce « jusqu'à quatre » avec sa réserve, qui en change le
sens : ces quatre fenêtres PRÉEXISTAIENT au démarrage du superviseur.** Elles
sont celles que l'énumération initiale trouve. **Le cas produit — un
utilisateur ouvre une application — a été tenté deux fois et a échoué deux
fois.** Ce que D1 sait faire aujourd'hui, c'est prendre un bureau tel qu'il est
et l'éclater en fenêtres navigateur ; il ne sait pas en accueillir une de plus.

La démonstration bout en bout du brief n'est donc **pas** obtenue. Sur ses huit
points : **2 obtenus** (1 et 5), **2 partiels** (4 et 8), **4 non obtenus**
(2, 3, 6 et 7).

---

## 1. Ce qui a été exécuté, et ce qui a été écarté

Huit exécutions du scénario ont été lancées — les six qui portent un numéro
ci-dessous, plus deux écartées avant d'avoir rien produit. **Trois sont
exploitées**, et ce sont les seules dont les journaux sont versés.

| # | Superviseur démarré (UTC) | Ce qu'elle vaut |
| --- | --- | --- |
| 1 | 09:39 env. | **Écartée** : la VM s'est mise en veille prolongée 15 s après le lancement (§7). Rien de mesuré. |
| 2 | — | **Écartée** : Chrome n'a pas démarré, le port CDP était tenu par une instance résiduelle d'un chantier antérieur. Rien de mesuré. |
| **A** | **10:17:12** | **Exploitée** — viewport tel que le navigateur l'annonce (1280×713). `demonstration-1-viewport-impair-*.log` |
| **B** | **10:28:14** | **Exploitée** — viewport forcé à 1280×720. `demonstration-2-viewport-pair-*.log` |
| C | 10:31:35 | Partiellement exploitée (4 sessions établies, effondrement au point 3). Redondante avec F, non versée. |
| D | 10:39:31 | **Écartée** : veille prolongée de la VM à 10:40:10, en cours d'exécution. |
| E | 10:57 env. | **Écartée** : le pilote s'est attaché à une instance Chrome périmée (§7). Rien de mesuré. |
| **F** | **10:58:46** | **Exploitée** — viewport pair, aucune nouvelle fenêtre Windows ouverte pendant la mesure. `demonstration-3-arrangement-stable-*.log` |

### Méthode, et ses deux écarts assumés au brief

Aucun accès interactif n'étant disponible, le navigateur est piloté par CDP
brut (même approche que `client/recette/paire-candidats.mjs`), Chrome en
`--headless=new`, avec les trois drapeaux anti-gel de la recette du 30/07/2026
**et `--disable-popup-blocking`** — sans lui, la page-shell n'ouvre **aucune**
fenêtre, puisqu'elle les ouvre depuis un gestionnaire de message WebSocket,
donc hors geste utilisateur. Les fenêtres Windows sont ouvertes et fermées par
tâche planifiée `/IT`, mécanisme établi par la tâche 6.

Deux écarts, tous deux nécessaires et tous deux à porter au compte des
résultats :

1. **Le viewport de B et F est imposé par `Emulation.setDeviceMetricsOverride`
   à 1280×720.** Sans cela, la fenêtre pop-up de Chrome sans interface annonce
   **1280×713** — une hauteur impaire, qui bloque tout (§3.1). Le code produit
   n'a pas été touché ; c'est l'instrument qui impose la taille, comme un
   utilisateur qui redimensionnerait sa fenêtre.
2. **F saute les points 2 et 6** (ouvrir puis fermer le Bloc-notes sur la VM).
   Ce n'est pas un contournement de complaisance : l'ouverture d'une fenêtre
   détruit l'arrangement (§3.3, prouvé dans A, B et C), et sans ce saut aucun
   des points 3 à 8 n'aurait de session vivante à exercer. F mesure donc
   **délibérément** ce que D1 sait faire quand rien ne bouge.

Le signaling a été relancé **sans variable TURN** (`coturn` n'était pas
démarré) : toutes les sessions sont directes sur le lien local. Vérifié sur
l'environnement du processus qui écoute réellement, pas sur celui qu'on croit
avoir lancé — `environnement-signaling.txt` porte le relevé : le processus qui
tient `:8080` a démarré à 11:36:44, avant les trois exécutions, et son
environnement ne contient aucune variable `TURN`.

L'instrument lui-même est versé (`pilote-recette.mjs`, `pilote-vm-it.sh`), avec
la réserve qui compte : **il a été modifié entre les exécutions**, et son état
versé est celui de F.

### Un correctif d'outillage a été nécessaire pour que la tâche démarre

`scripts/run-agent.sh` ne transmettait **pas** `SUPERVISEUR` à la VM : la
variable est posée côté hôte, la tâche planifiée ne la voyait jamais, et
l'agent démarrait en mode mono-fenêtre. Ligne ajoutée sur le modèle des
`MULTIFENETRE_*` voisines. Sans elle, aucune des exécutions ci-dessus n'aurait
été un superviseur.

---

## 2. Le déroulé point par point, avec son issue réellement observée

| # | Point du brief | Issue observée |
| --- | --- | --- |
| 1 | La page-shell liste les fenêtres déjà ouvertes et en ouvre une par fenêtre | ✅ **OBTENU.** B : 3 fenêtres listées, 3 fenêtres navigateur ouvertes, 3 sessions `connected` à 1280×720, RTT 2–3 ms. F : **4** fenêtres, 4 sessions simultanées. Chaque fenêtre navigateur montre son application et **elle seule**, plein cadre (captures §2.1) |
| 2 | Ouvrir le Bloc-notes → une nouvelle fenêtre navigateur s'ouvre, montrant le Bloc-notes et lui seul | ❌ **NON OBTENU.** La fenêtre est bien détectée et une session lui est bien créée, mais la création de sa sortie virtuelle **tue toutes les sessions en cours**, la sienne comprise (§3.3). Exercé dans A et B, même issue |
| 3 | Taper au clavier dans la fenêtre du Bloc-notes → le texte y apparaît, et pas ailleurs | ❌ **NON OBTENU.** Frappe émise dans les 4 pages de F ; relecture du contrôle d'édition des **deux** Bloc-notes par `WM_GETTEXT` : les deux sont **vides**, et aucun titre ne porte l'astérisque de modification (`demonstration-relecture-bloc-notes.txt`, qui porte aussi la même relecture après l'exécution D). Cause non départagée (§3.5) |
| 4 | Cliquer et déplacer la souris dans chaque fenêtre → le pointeur agit dans la bonne | ⚠️ **PARTIEL, et par le négatif.** Le clic synthétique est consommé par la demande de verrouillage du pointeur (bandeau « cliquez dans l'image pour prendre la souris » visible sur `demonstration-3-verrouillage-pointeur-non-accorde.png`), qui exige une activation utilisateur qu'un clic CDP ne fournit pas en mode sans interface. **Aucun mouvement de pointeur n'a donc été porté jusqu'à la VM.** Ce que la mesure établit tout de même : les quatre fenêtres Windows sont bien posées chacune sur sa sortie, à `+2400`, `+3680`, `+4960`, `+6240` (relevé F, « fenêtres VM après frappe ») — la géométrie sur laquelle le calcul de pointeur s'appuie est donc juste |
| 5 | Jouer un son sur la VM → le son sort **d'une seule** fenêtre navigateur | ✅ **OBTENU, avec une réserve de méthode.** Sur les 4 sessions de F, **une seule** porte une piste audio (`w-1`) ; les trois autres n'en ont aucune (`audio: null`), et l'agent le dit de son côté : `audio activé` une fois, `son désactivé sur cet enfant : une seule fenêtre le porte` trois fois. Pendant les quatre `Windows Ding.wav` (`pilote-recette.mjs`, `1..4 | … Media.SoundPlayer … PlaySync()`), les octets RTP audio de `w-1` passent de 1 592 à 61 302, soit **+59 710 octets en 9,4 s**. Avant cela, la piste n'avait accumulé que 1 592 octets. ⚠️ **Sur quelle durée : ≈15 s, et non 26.** Le pilote horodate en secondes depuis son propre démarrage, l'agent en UTC ; en recalant les deux (première ligne d'agent `10:58:46.45` ↔ `t≈4,3 s` du pilote, `audio activé` `10:58:56.547` ↔ `t≈14,4 s`, première lecture à `t=29,3 s`), la piste audio de `w-1` n'a vécu que **≈15 s** avant la lecture. Une première rédaction écrivait 26 s, ce qui surévaluait la fenêtre de comparaison d'une dizaine de secondes. Les débits qui en découlent : **≈106 octets/s avant, ≈6 350 octets/s pendant**, soit un facteur ≈60 — la conclusion est inchangée, et elle ne porte que sur `w-1`, les trois autres sessions n'ayant aucune piste audio à compter. **Réserve** : `totalAudioEnergy` reste à 0 — la page n'a jamais reçu de geste utilisateur, son `AudioContext` est resté suspendu. **Le son est arrivé, il n'a pas été joué** : c'est prouvé par le débit reçu, pas par une écoute |
| 6 | Fermer le Bloc-notes sur la VM → sa fenêtre navigateur se ferme | ❌ **NON EXERCÉ.** Aucune exécution n'a atteint ce point avec une session vivante |
| 7 | Fermer une page navigateur → l'application reste ouverte, la shell la liste « fermée », « Rouvrir » la ramène | ❌ **NON EXERCÉ.** Dans F, le pilote a fermé la mauvaise page (`about:blank`) — défaut de mon script, pas du produit — et l'arrangement s'était de toute façon effondré 3 s plus tôt. Ce qui est tout de même observé : **les 4 applications Windows sont restées ouvertes** après la mort de leurs sessions (relevé « fenêtres VM après fermeture de la page navigateur ») |
| 8 | Déplacer une fenêtre hors de sa sortie → le superviseur la replace dans la seconde | ⚠️ **PARTIEL.** Le mécanisme **fonctionne et il est visible** : `fenêtre sortie de sa sortie, replacement session=w-1 de="160x668+2400+0" vers="1280x720+2400+0"` (B). **Sept occurrences en tout dans A et B** (2 + 5), et six de plus dans F. Mais il n'a **jamais** été déclenché par un déplacement délibéré sur une session vivante : à chaque fois il rattrapait une fenêtre que le chemin de redimensionnement de l'enfant venait de rétrécir (§3.2). Le déplacement forcé du point 8 de F s'est produit alors qu'aucune session ne vivait |

### 2.1 La pièce du point 1

Trois captures d'écran de trois fenêtres navigateur distinctes, prises dans la
même seconde de l'exécution B :

- `demonstration-2-fenetre-bloc-notes.png` — le Bloc-notes, plein cadre,
  surimpression « 1280×720 · RTT 3.0 ms · audio 1 kb/s » ;
- `demonstration-2-fenetre-explorateur.png` — l'explorateur « Ce PC », plein
  cadre, « audio absente » ;
- `demonstration-2-fenetre-firefox.png` — Firefox et son animation, plein
  cadre, « audio absente ».

Aucune ne montre le bureau ni une autre application. C'est le fait central de
D1, et il est acquis.

---

## 3. Ce qui a échoué, et pourquoi

Cinq défauts, tous reproduits, présentés dans l'ordre où on les rencontre.

### 3.1 Une hauteur de viewport impaire rend toute fenêtre impossible

Exécution A. Le navigateur annonce **1280×713**. Le pilote accepte cette
définition et crée la sortie. Puis, 1,5 s plus tard, la topologie DXGI rend
`\\.\DISPLAY5 1280x720` — la définition demandée n'est **pas encore
appliquée** — et l'égalité stricte de `placement::sortie_par_dimensions`
échoue :

```
ERROR agent::superviseur::boucle: sortie créée mais introuvable dans la
topologie DXGI — elle est rendue au pilote session=w-2 demande="1280x713"
apparues=["\\\\.\\DISPLAY5 1280x720"]
```
*(`demonstration-1-viewport-impair-agent.log:22`)*

La trace des candidats ajoutée au correctif I-6 de la tâche 12 a fait
exactement son travail : sans elle, ce diagnostic restait entier. **Ce n'est
pourtant pas le facteur DPI de 1,5 qui était redouté** — le rapport ici est de
713/720, c'est une **course**, pas une mise à l'échelle. À l'essai suivant,
1,5 s plus tard, la même sortie est rendue à 1280×713 et l'appariement réussit :
`DELAI_RATTACHEMENT = 1500 ms` n'est donc pas toujours suffisant, et le
comportement est non déterministe.

Et même quand l'appariement réussit, l'affaire est perdue en aval :
`WindowsSource::resize` force les dimensions paires (`& !1`,
`windows_source.rs:171`), donc 713 devient 712, donc la taille demandée ne peut
**jamais** égaler la taille de la source. Le premier `Resize` que le navigateur
envoie à la connexion part alors dans le chemin décrit ci-dessous.

**Conséquence produit** : un viewport de hauteur impaire est le cas banal (une
fenêtre de navigateur ordinaire), et il rend D1 inopérant.

### 3.2 Le chemin de redimensionnement de l'enfant ignore le mode « sortie DXGI entière »

Le navigateur envoie un `Resize` dès la connexion. `WindowsSource::resize` est
écrit pour le mode **recadrage de fenêtre** : il redimensionne la fenêtre
Windows elle-même, puis reconstruit une duplication du **bureau**. En mode
`sur_sortie`, la fenêtre est sur une sortie virtuelle hors du bureau physique,
et l'appel échoue :

```
INFO  agent::demarrage: contrôle reçu Resize { version: 3, width: 1280, height: 720 }
INFO  agent::capture: duplication de sortie établie desktop_width=2400 desktop_height=1080
WARN  agent::windows_source: reconstruction de la chaîne d'encodage échouée,
      capture de secours restaurée erreur=la fenêtre est hors de l'écran
WARN  agent::transport::redimensionnement: échec du redimensionnement, ignoré
      erreur=la fenêtre est hors de l'écran width=1280 height=720
```
*(`demonstration-2-viewport-pair-agent.log:117,123,124,125` — **quatre lignes non contiguës**, entre `10:28:22.802866` et `10:28:22.909242`. Les cinq lignes sautées sont des traces de contexte D3D11 ; l'une d'elles, l. 120, répète mot pour mot la l. 123 — la reconstruction ouvre deux duplications successives du bureau avant d'échouer.)*

Trois conséquences observées :

- la fenêtre Windows est **rétrécie au passage** (relevés `160x668`, `161x720`,
  `516x720`) ; c'est le contrôle périodique de placement du superviseur qui la
  rattrape une seconde plus tard, et c'est **la seule raison** pour laquelle ce
  contrôle a été vu à l'œuvre (point 8) ;
- quand la reconstruction échoue **et** que la capture de secours part sur le
  bureau physique, l'enfant journalise
  `soumission à l'encodeur échouée erreur=récupération de l'image convertie en
  NV12` **une fois par image soumise**, sur un partage CIFS : 32 lignes dans la
  seconde `10:17:19`, 14 dans la suivante, puis plus rien — non parce que le
  débit retombe, mais parce que la source meurt (§3.3). L'exécution A en porte
  **46 au total**, l'exécution B **28**, la F **20**. Le taux est donc borné
  ici par la brièveté de l'état, pas par le code : rien ne limite cette trace,
  et une session qui durerait la produirait à la cadence d'encodage. Le dépôt a
  déjà écrit qu'on ne trace jamais par paquet dans une boucle de transport ;
  c'est la même faute, sur le chemin d'erreur ;
- ce redimensionnement de fenêtre engendre des événements `SHOW`, donc de
  nouvelles sessions, donc de nouvelles sorties, donc §3.3.

### 3.3 Créer une sortie virtuelle tue toutes les captures en cours — le défaut central

C'est le défaut qui empêche la démonstration d'exister.

Dès qu'une sortie virtuelle est **créée**, DXGI abandonne le mutex partagé des
duplications **déjà ouvertes**. Chaque enfant vivant le voit ainsi :

```
ERROR agent::windows_source: capture interrompue, source déclarée épuisée
      erreur=acquisition d'image : Le mutex indexé a été abandonné. (0x887A0026)
INFO  agent::transport::controle: clôture de session amorcée reason="source vidéo épuisée"
```

et se termine. Dans F, la 5ᵉ sortie est créée à `10:59:46.881246`
(`demonstration-3-arrangement-stable-agent.log:342`) et les **quatre** enfants
meurent 49,1 ms plus tard, en **8,28 ms** (l. 343, 345, 347 et 349 :
`.930364`, `.935785`, `.936764`, `.938648`), chacun suivi de sa `clôture de
session amorcée`.

**La correspondance est exacte, et c'est ce qui fait la preuve.** Sur les trois
journaux versés :

| | créations de sortie | erreurs `0x887A0026` |
| --- | --- | --- |
| **Aucune duplication ouverte** (avant le lancement du premier enfant) | 9 (A : 2, B : 3, F : 4) | **0** |
| **Duplications ouvertes** | 4 (A : 2, B : 1, F : 1) | **9** — soit `1, 1, 3, 4`, **exactement le nombre de duplications ouvertes à chaque fois** |

Le témoin négatif est donc dans les mêmes journaux : créer une sortie **avant**
qu'une duplication n'existe ne produit rien.

⚠️ **La destruction, elle, n'est PAS mise en cause : le cas n'a jamais été
exercé.** Les **17** destructions des trois journaux tombent toutes hors de
toute duplication ouverte — en A, `sortie virtuelle détruite id=264` survient 1,6 s
avant le lancement du premier enfant ; en B et F, toutes les destructions
tombent 3,0 à 4,5 s **après** que tous les enfants sont déjà morts. Une
première rédaction écrivait « créée (ou détruite) » : **cette parenthèse
doublait la portée du défaut sans la moindre preuve**, et elle est retirée.
Que la destruction ait ou non le même effet reste ouvert.

Reproduit dans **trois exécutions versées sur trois** (A, B, F), plus une
quatrième (C) dont les journaux ne sont pas joints — voir §1.

S'ensuit un emballement, lui aussi reproduit à chaque fois :

1. les enfants morts libèrent leurs sorties, que le superviseur rend au pilote ;
2. les enfants qu'on venait de lancer démarrent avec un index de sortie DXGI qui
   n'existe plus — `Error: aucune sortie DXGI à l'index adaptateur 0, sortie 5`
   — et sortent en code 1 ;
3. les fenêtres Windows, replacées ou redimensionnées entre-temps, réémettent
   `SHOW`, et comme leur entrée de table a été retirée par `enfant_mort`, elles
   reçoivent une **nouvelle** session (`w-5`, `w-6`, `w-7`, `w-8`…) ;
4. la boucle recommence, jusqu'à ce que plus rien ne bouge — et la shell
   n'affiche alors **plus aucune fenêtre**, alors que les quatre applications
   Windows sont toujours ouvertes.

Deux défauts de conception se lisent au passage :

- **l'index `(adaptateur, sortie)` n'est pas un identifiant.** Il est
  positionnel : il change dès qu'une sortie apparaît ou disparaît. Le
  superviseur le passe pourtant à l'enfant par variable d'environnement, et
  l'enfant le résout à son démarrage, plus tard. Le `nom_sortie`
  (`\\.\DISPLAYn`), lui, est stable, et le superviseur l'a déjà sous la main ;
- **une session morte fait oublier la fenêtre.** `enfant_mort` retire l'entrée
  de la table ; plus rien ne la rappelle, sauf un `SHOW` fortuit de Windows.
  Une fenêtre bien vivante peut donc disparaître de la shell pour toujours.

### 3.4 L'ordre d'arrivée des annonces n'est pas rattrapable

Le signaling ne mémorise que les offres SDP (tâche 9). Les annonces
`fenetre-ouverte` émises par le superviseur **avant** que la page-shell ne soit
connectée sont perdues sans trace. En pratique cela impose de lancer le
navigateur **avant** le superviseur — ce que fait le pilote de recette, et ce
que le brief ne dit pas. Ce n'est pas un défaut du code, c'est une contrainte
d'exploitation non écrite.

### 3.5 Le clavier n'atteint pas la fenêtre visée — cause non départagée

Les deux Bloc-notes sont restés vides. Deux explications sont visibles dans le
code, **et la mesure ne permet pas de choisir** :

- côté navigateur, le premier clic sur l'image est consommé par
  `requestPointerLock` (`client/src/pointer.ts:95-105`), qui exige une
  activation utilisateur transitoire qu'un `Input.dispatchMouseEvent` de CDP ne
  fournit pas — `demonstration-3-verrouillage-pointeur-non-accorde.png` montre le bandeau resté affiché ;
- côté agent, l'injection passe par `SendInput` (`agent/src/input.rs:197`), qui
  est **globale à la session Windows** : le clavier va à la fenêtre au premier
  plan, quelle qu'elle soit. Aucun `SetForegroundWindow` n'est fait sur la
  fenêtre de la session. Avec N fenêtres sur N sorties, une seule peut être au
  premier plan.

Le point 3 du brief — « le texte apparaît dans le Bloc-notes, **et pas dans
l'autre application** » — n'est donc ni démontré ni réfuté. **Le second point
ci-dessus est structurel** : il faudra le traiter dans D2, indépendamment de ce
que le premier vaut.

---

## 4. Le plafond d'encodeurs en multi-processus : **non atteint, donc non mesuré**

C'était le relevé attendu de cette tâche. Il ne peut pas être livré.

Ce qui est observé :

- **4 encodeurs matériels NVENC construits simultanément, dans 4 processus
  distincts, sans un seul refus** (exécution F : quatre
  `encodeur matériel retenu encodeur=NVIDIA H.264 Encoder MFT`, quatre sessions
  `connected` en même temps) ;
- **6 sorties virtuelles attachées simultanément** (7 sorties DXGI au total avec
  l'écran physique), dans B comme dans F, sans refus du pilote ;
- **aucune occurrence** de `MF_E_UNSUPPORTED_D3D_TYPE` ni d'aucun échec de
  construction d'encodeur dans les trois journaux versés.

Le plafond de 8 relevé en processus unique (mesures préalables du 31 juillet)
n'a donc **pas** été approché : le défaut §3.3 fait mourir les enfants bien
avant. **Rien dans cette recette ne confirme ni n'infirme que ce plafond de 8
vaut aussi entre processus.**

---

## 5. Limites connues et assumées de D1

Reprises de `specs/2026-08-01-multifenetres-tranche-verticale-design.md` §8,
pour qu'aucune ne soit lue comme une découverte de cette recette :

- **le lien est sur-souscrit à plusieurs fenêtres actives** : chaque enfant
  garde son propre contrôleur de congestion et estime sa part comme s'il était
  seul. Aucun partage de capacité (D3). *Non observé ici : les sessions n'ont
  jamais vécu assez longtemps ni assez chargé le lien pour que cela se voie* ;
- **le son est perdu si la fenêtre porteuse se ferme** : aucune redésignation
  de porteuse (D2) ;
- **pas de redimensionnement d'une fenêtre déjà ouverte** : le pilote n'expose
  aucun changement de mode ;
- ⚠️ **« une sortie libérée n'est rendue au pilote qu'à l'arrêt du
  superviseur » : cette limite est PÉRIMÉE et il faut cesser de l'écrire.** Elle
  ne figure pas dans le §8 de la conception, mais dans le plan
  (`plans/2026-08-01-multifenetres-tranche-verticale.md`, lignes 3544 et 3575),
  qui se contredit lui-même trente lignes plus haut (l. 3262, « Rendue
  MAINTENANT, pas à l'arrêt du superviseur »). La tâche 12 a câblé
  `DetruireSortie` sur `Sorties::detruire` dans le tour même, et la recette le
  montre à l'œuvre : `sortie virtuelle rendue au pilote sortie_pilote=259` en B
  et en F (A porte 256, 257, 262 et 263) — **16 rendus** en tout sur les trois
  journaux, pour **17 destructions** côté pilote : l'écart d'une unité est la
  sortie que A rend par `rendre_sans_apparier`, chemin qui ne journalise pas ce
  message ;
- **plein écran** (D4) et **mise en sommeil des fenêtres masquées** (D5) : hors
  périmètre, non exercés ;
- **audio par fenêtre** (loopback de processus, D3) : hors périmètre. Ce que D1
  fait est un son unique porté par une fenêtre, et c'est ce qui a été vérifié.

---

## 6. Ce que cette démonstration n'établit pas

- **Le plafond d'encodeurs en multi-processus n'est PAS relevé** — c'était
  pourtant le chiffre attendu de cette recette, et c'est ici qu'un lecteur
  pressé le cherchera. Il n'a pas été approché : voir §4.
- **Le cas produit n'est pas couvert** : les quatre fenêtres tenues
  simultanément **préexistaient au démarrage du superviseur**. Aucune fenêtre
  ouverte pendant une session n'a jamais abouti — deux tentatives, deux échecs.
- **La destruction d'une sortie virtuelle n'est pas mise en cause, faute
  d'avoir été exercée** pendant qu'une duplication était ouverte (§3.3).
- **Aucune mesure de cadence ni de latence.** Les `framesDecoded` relevés (4 à
  29 images cumulées à t+29 s) ne mesurent rien : les contenus capturés sont
  statiques, et Desktop Duplication n'émet une trame qu'au changement du bureau.
  Le RTT de 1 à 3 ms est celui de la paire ICE, pas la latence de bout en bout.
- **Une seule exécution exploitée par configuration.** Aucun taux, aucune
  variabilité. Le défaut central du §3.3 est, lui, reproduit sur **trois
  exécutions versées sur trois**, plus une quatrième dont les journaux ne sont
  pas joints.
- **Quatre fenêtres au plus, jamais huit.** La cible du chantier n'est pas
  approchée.
- **Trois applications seulement** — Bloc-notes, Explorateur, Firefox — toutes
  fenêtrées, aucune en plein écran, aucun jeu, aucun contenu Direct3D
  exclusif.
- **Aucune durée longue** : la plus longue session vivante mesurée dure environ
  **50 secondes** (F, de 10:58:56 à 10:59:46).
- **Aucune unité H.264 n'a été décodée puis inspectée** autrement que par
  l'image que le navigateur affiche ; les captures d'écran prouvent que l'image
  est juste à l'instant où elles sont prises, pas qu'elle l'est en continu.
- **Le son n'a pas été entendu**, seulement compté en octets reçus (§2).
- **Le clavier et la souris ne sont pas démontrés**, ni en bien ni en mal
  (§3.5).
- **Rien du comportement quand Apollo/Sunshine consomme le même vivier de
  sorties** — le service tournait pendant toute la recette, et n'a été vu tenir
  aucune sortie ; rien ne dit ce qui se passerait s'il en tenait.
- **La cause de la course du §3.1 n'est pas isolée** : on ne sait pas si 1,5 s
  est simplement trop court, ou si le pilote applique parfois une définition
  approchée avant la bonne.

---

## 7. Pièges rencontrés — à connaître avant de retoucher ce terrain

- **La VM se met en veille prolongée toute seule, et elle l'a fait deux fois en
  pleine mesure.** Les deux extinctions sont horodatées **09:40:09 et 10:40:10
  UTC**, à une heure d'intervalle et à la même minute. Ce n'est pas une
  minuterie d'inactivité (`STANDBYIDLE` et `HIBERNATEIDLE` sont tous deux à 0,
  relevé dans `veille-prolongee-vm.txt`) : le journal Windows nomme
  l'initiateur, `\Windows\System32\shutdown.exe` (Kernel-Power 187), pour une
  transition de type hibernation (Kernel-Power 42).
  ⚠️ **Le motif horaire n'est PAS établi pour autant, et il ne faut pas
  l'écrire** : le même relevé porte une **troisième** hibernation, antérieure à
  la recette, à 08:29:50 UTC — qui ne tombe ni sur la minute :40, ni sur
  l'intervalle d'une heure. Le déclencheur exact **n'est pas identifié** : la
  seule tâche planifiée qui appelle `shutdown` est désactivée depuis avril 2025,
  et `sunshine`/`sunshinesvc` tournent sur la VM sans qu'aucune mesure ne les
  mette en cause. La seule règle prudente que ces trois points autorisent est
  donc : **ne pas lancer de séquence longue sans vérifier ensuite que la VM a
  survécu**. Symptôme côté hôte : `/media/vm` répond « L'hôte cible est arrêté
  ou en panne », `virsh list --all` dit « fermé », et `scripts/run-agent.sh`
  échoue en écrivant son `.ps1`.
- **Un agent survit à l'hibernation de la VM.** Le superviseur de l'exécution D
  écrivait encore dans `agent.log` après la reprise. `run-agent.sh` ne tue pas
  l'agent existant : il recrée la tâche planifiée et la lance. **Vérifier
  `Get-Process agent` avant toute nouvelle exécution**, sans quoi on mesure le
  processus précédent.
- **Chrome sans interface survit à la mort de son pilote.** Une exécution
  entière a été perdue parce que `attendreDevtools` s'est connecté à une
  instance résiduelle, sur un port réutilisé, dont les pages `shell.html`
  périmées ne recevaient plus rien. **Un port de débogage qui répond ne prouve
  pas que c'est le bon navigateur** : contrôler que les cibles présentes sont
  bien celles qu'on vient de créer.
- **Sans `--disable-popup-blocking`, la démonstration est vide et muette.** La
  shell ouvre ses fenêtres depuis un gestionnaire de message WebSocket, hors
  geste utilisateur : Chrome les refuse toutes, et la seule trace est le message
  de la page-shell (« le navigateur a bloqué la pop-up »), qui n'apparaît que si
  l'on pense à le lire.
- **Une capture d'écran CDP suffit à déclencher l'effondrement.** Les
  `Page.captureScreenshot` provoquent un `Resize` côté page, qui part dans le
  chemin du §3.2, qui rétrécit la fenêtre Windows, qui engendre un `SHOW`, qui
  crée une session, qui crée une sortie, qui tue toutes les captures.
  **L'instrument détruisait ce qu'il mesurait** — c'est la même leçon que la
  trace par paquet du chantier TURN, sous une autre forme.
- **Le journal de l'agent porte tout, superviseur et enfants confondus.** Les
  enfants héritent de la sortie standard du superviseur : `agent.log` mêle N+1
  processus sans les distinguer, et rien dans la ligne ne dit de quel enfant
  elle vient (seul le contexte `session=` de certaines lignes le permet).

---

## 8. État de la VM à la fin

Contrôle depuis un **processus neuf**, superviseur arrêté, en comparant des
**ensembles de noms** :

| Moment | Sorties attachées |
| --- | --- |
| Avant tout (`topologie-avant.log`) | `{\\.\DISPLAY1}` |
| Après l'exécution A (`topologie-apres-run3.log`) | `{\\.\DISPLAY1}` |
| Après l'exécution B (`topologie-apres-run4.log`) | `{\\.\DISPLAY1}` |
| Après l'exécution F (`topologie-apres-run8.log`) | `{\\.\DISPLAY1}` |

**Ensemble identique au départ, aux trois contrôles : aucune sortie n'a fuité**,
alors même que le superviseur a été tué net à chaque fois (`Stop-Process`),
donc sans que `Sorties::drop` ne coure jamais. La purge autonome finale le
confirme et ne trouve rien à retirer :
`purge terminée retirees=0 avant=1 apres=1` (`purge-finale.log`).

**Ce que ce contrôle n'établit pas** : il porte sur l'ensemble des **noms** de
sorties attachées, et sur rien d'autre — ni sur l'état interne du pilote, ni sur
d'éventuelles ressources DXGI. Et il n'attribue pas le mérite : rien ici ne dit
si les sorties ont été rendues par le superviseur, par la garde, ou reprises par
le chien de garde du pilote faute de ping.

Deux Bloc-notes, un explorateur et un Firefox restent ouverts sur la VM, aux
positions où le dernier replacement les a laissés. Les neuf tâches planifiées
créées pour la recette — dont `guacamole-agent` — **ont été supprimées, et leur
absence contrôlée** ; les scripts `.ps1` jetables restent dans `C:\dev`.

---

## 9. Ce qu'il faut régler avant que D1 soit démontrable

Par ordre de blocage, sans préjuger des solutions :

1. **§3.3 — la création d'une sortie ne doit pas tuer les captures en cours.**
   Tant que ce point tient, D1 ne peut pas dépasser « les fenêtres qui
   existaient au démarrage ». C'est le seul défaut qui interdit la
   démonstration ; les autres la dégradent.
2. **§3.3 bis — désigner la sortie par son nom, pas par son index.**
3. **§3.2 — `resize` doit connaître le mode `sur_sortie`**, ou n'y rien faire du
   tout : en mode « une sortie par fenêtre », la taille de la source est celle
   de la sortie, et il n'y a rien à redimensionner.
4. **§3.1 — arrondir le viewport annoncé à des dimensions paires**, et rendre
   l'appariement tolérant à la latence d'application du mode.
5. **§3.5 — donner le premier plan à la fenêtre de la session avant d'injecter
   du clavier**, ou renoncer à `SendInput` pour le clavier.
6. **§3.2 — ne pas inonder le journal sur le chemin d'erreur** (compter,
   échantillonner).
7. **§3.3 — ne pas perdre une fenêtre vivante quand sa session meurt.**
