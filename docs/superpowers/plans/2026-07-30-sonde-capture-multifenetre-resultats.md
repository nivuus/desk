# Sonde de capture multi-fenêtres — résultats

**Date** : 30 juillet 2026 · **Branche** : `chantier-sonde-capture` ·
**Plan** : `docs/superpowers/plans/2026-07-30-sonde-capture-multifenetre.md`

Ce chantier n'a livré aucune fonctionnalité. C'était une **sonde de mesure** :
trancher, avant de spécifier le chantier D (« multi-fenêtres ») du produit,
quelle voie de capture d'écran Windows rend une image correcte **par fenêtre**
quand les fenêtres se recouvrent, et à quel coût.

Tout chiffre cité ici provient d'un journal joint dans
`docs/superpowers/plans/journaux-sonde-multifenetre/` (fichiers `.log` produits
par l'agent sur la VM ; les accents y sont corrompus en amont de l'écriture par
la page de code PowerShell — l'ASCII reste lisible), **à une exception près,
signalée à sa place** (le relevé de la session Apollo, section « Relevé DXGI »
ci-dessous). **Rien n'est reconstitué de mémoire, et rien n'est extrapolé
au-delà du rang mesuré.**

---

## Ce qui est acquis

1. **Aucune des quatre voies n'est simplement viable.** Une est éliminée, trois
   sont conditionnelles, chacune pour une raison différente. Le chantier D devra
   **arbitrer**, pas choisir.
2. **La voie de production actuelle (Desktop Duplication recadrée) est
   disqualifiée dès deux fenêtres qui se recouvrent** — mesuré, pas déduit :
   `verdicts_faux` = 449 / 449 / 536 à N = 2 / 4 / 8. C'est le résultat attendu,
   et c'est **ce qui valide le banc** : l'instrument détecte bien la pollution
   qu'il est censé détecter.
3. **La capture ne décroche pas jusqu'à 8 fenêtres.** Voie `duplication`, passe
   capture, cadence *par fenêtre* : 80,2 i/s à N=1 ; 99,4 à N=2 ; 98,8 à N=4 ;
   107,5 à N=8. Toutes les fenêtres d'un même rang sont égales entre elles.
   **Ce que ce montage établit** : partager une acquisition entre N recadrages
   d'aire totale fixe ne coûte rien de plus. `disposition::tuiles` découpe le
   bureau, donc la surface par fenêtre décroît quand N croît (208, 258, 256,
   248 MP/s de débit de pixels à N = 1, 2, 4, 8 — quasi constant par
   construction) ; le débit de pixels mesuré ne dit donc rien sur le cas où N
   fenêtres conserveraient chacune leur propre résolution utile — 8 fenêtres à
   1280×720 valent 2,8× la surface du bureau mesuré, un montage que ce banc
   n'a pas exercé.
4. **`PrintWindow(PW_RENDERFULLCONTENT)` rend l'image *juste* d'une fenêtre D3D
   *recouverte*.** Contre-intuitif — le plan attendait du noir — vérifié en
   revue (recouvrement effectif contrôlé, couleur recalculée, faux positif du
   bitmap vierge écarté). C'est la **seule** voie ayant franchi la porte de
   correction d'image sous recouvrement par une mesure directe.
5. **Le pilote d'affichage virtuel produit bien une sortie DXGI réelle**, portée
   par l'adaptateur qui possède NVENC — mais seulement pendant qu'une session de
   streaming Apollo tourne, et **en remplacement** de l'écran physique.
6. **Plafond d'encodage borné** : 8 instances du pipeline Media Foundation
   complet sur un périphérique D3D11 **partagé**, la 9ᵉ échouant à la liaison du
   type d'entrée.

## Ce qui reste ouvert

- **Le relevé DXGI de la session Apollo n'a pas de journal joint.** `\\.\DISPLAY5`,
  3413×960, vient du rapport de tâche 7 et non d'un fichier de
  `journaux-sonde-multifenetre/` (voir la réserve dans « Relevé DXGI »
  ci-dessus). La session n'est reproductible qu'avec le propriétaire du poste.
  C'est le chiffre le moins bien étayé du document, et il fonde pourtant la
  voie recommandée.
- **Le plafond de sorties virtuelles simultanées.** Non mesuré : la mesure exige
  un **second appareil client apparié** à Apollo, que le propriétaire du poste
  n'a pas. Obstacle matériel, non technique. C'est la question ouverte la plus
  utile au chantier D — deux clients suffiraient à la lever.
- **Le comportement du plafond d'encodage sur des périphériques D3D11
  séparés.** La mesure porte sur un périphérique unique partagé par les 8
  encodeurs. Une isolation par fenêtre (un périphérique par encodeur) pourrait
  déplacer ce plafond à la hausse comme à la baisse. Non mesuré.
- **La correction d'image de la voie 2 sous recouvrement.** Elle est
  *argumentée par construction* (une fenêtre par moniteur ⇒ pas de
  recouvrement), **jamais mesurée** : aucune capture n'a été exercée sur la
  sortie virtuelle. Ne pas la porter au crédit de la voie comme un acquis.
- **`printwindow` à N=4 et N=8** : non mesuré. La passe capture+encodage à N=2
  n'a pas été conclue (arrêtée en cours, à 88 unités pour 176 images, sans
  erreur ni dégradation visible). Ne pas extrapoler la cadence au-delà de N=2.
- **La résolution réellement settable du bureau.** Seul le mode courant
  (2400×1080) a une settabilité établie. Le mode 4096×2160 est *annoncé* par
  WMI ; toutes les tentatives de changement de mode ont échoué avant
  application.
- **`DwmGetDxSharedSurface`** : le symbole est exporté par `user32.dll` sur
  cette VM (résolu par `GetProcAddress` sans échec), mais **jamais appelé** —
  limite fixée par le plan. Cinquième voie potentielle, entièrement non sondée.

---

## Relevé DXGI (l'écart documentaire, tranché)

Deux documents antérieurs se contredisaient en apparence sur l'existence d'un
adaptateur d'affichage virtuel (`fix-debit-socket-report.md:163`, hypothèse
« SudoMaker », contre le commit `4493b24`). **Les deux disent vrai sur des états
différents** ; seul `4493b24` décrit l'état courant.

Relevé au repos (`journaux-sonde-multifenetre/dxgi.log`) — trois adaptateurs
DXGI, **une seule sortie** :

| Adaptateur | Index | Sortie |
| --- | --- | --- |
| NVIDIA GeForce RTX 4070 | 0 | `\\.\DISPLAY1`, attachée, 2400×1080 en (0,0) |
| NVIDIA GeForce RTX 4070 | 1 | aucune (identité non élucidée) |
| Microsoft Basic Render Driver | 2 | aucune |

Le « SudoMaker Virtual Display Adapter » **existe et est sain** (WMI/PnP,
`ROOT\DISPLAY\0003`, `Status: OK`, `Present: True`) mais **n'expose aucune
sortie DXGI** au repos : pilote d'affichage indirect inactif.

**Pendant une session de streaming Apollo** (déclenchée par le propriétaire du
poste, relevée pendant qu'elle tournait), le relevé change :

```
sorties DXGI relevées  nombre=1
sortie  adaptateur=NVIDIA GeForce RTX 4070  index_adaptateur=0  index_sortie=0
        nom=\\.\DISPLAY5  attachee=true  x=0 y=0 largeur=3413 hauteur=960
```

**Réserve — ce relevé n'a pas de journal joint.** Contrairement à tous les
autres chiffres de ce document, celui-ci n'est pas tiré d'un fichier de
`journaux-sonde-multifenetre/` : il vient du rapport de tâche 7, dont le
relecteur a retrouvé l'entrée correspondante sur la VM (identique à la
microseconde) sans qu'elle ait jamais été extraite dans un fichier versé. La
session Apollo qui l'a produite n'est **pas reproductible à volonté** — elle
exige que le propriétaire du poste relance Apollo — et n'a donc pas été
rejouée pour combler ce trou. C'est le chiffre le moins bien étayé de ce
document, et il fonde pourtant la voie recommandée. Reprise en synthèse dans
« Ce qui reste ouvert ».

Trois faits en découlent, et un piège :

- La sortie virtuelle **n'est pas un adaptateur DXGI distinct** : elle est
  portée par la RTX 4070 sous un nouveau nom. Signature d'un pilote IddCx
  (`UpperFilters=IndirectKmd` dans `oem18.inf` / `sudovda.inf`).
- **L'écran physique disparaît** de l'énumération : `\\.\DISPLAY1` n'est plus
  là. Conséquence du réglage `dd_configuration_option = ensure_only_display`
  d'Apollo, dont la description officielle est explicite (« Deactivate other
  displays and activate only the specified display ») : c'est un
  **remplacement**, pas une addition.
- La sortie est portée par **l'adaptateur qui possède NVENC** : aucune
  discordance de périphérique GPU entre source de capture et encodeur matériel
  n'est à craindre pour cette voie.
- **Piège de résolution — tranché.** WMI annonce 5120×1440, DXGI mesure
  **3413×960** (rapport 1,5006 ≈ mise à l'échelle DPI 150 %). **La seule valeur
  mesurée est 3413×960**, deux relevés concordants. 5120×1440 reste une
  hypothèse de résolution native sous-jacente, **affaiblie par la démonstration
  que ce champ WMI est périmé** : il annonçait encore 5120×1440 **68 secondes
  après** que DXGI eut confirmé le détachement de la sortie. Ne pas citer
  5120×1440 comme un fait.
- **Trouvaille actionnable, non encore rencontrée en pratique** : si une tâche
  future calcule un recadrage à l'intérieur d'une texture capturée à partir du
  rectangle `DesktopCoordinates` (3413×960) alors que la texture est aux
  dimensions physiques (5120×1440), le recadrage serait décalé d'un facteur 1,5.
  À vérifier avant que la voie 2 aille plus loin.

---

## Verdict par voie

| Voie | Verdict | Correction sous recouvrement | Chemin GPU | Cadence à N | Notes |
| --- | --- | --- | --- | --- | --- |
| **1 — `Windows.Graphics.Capture`** | **ÉLIMINÉE** | non évaluable — la capture ne démarre pas | oui (par conception) | — | `CreateForWindow` échoue en `0x800706BE` (`RPC_S_SERVER_UNAVAILABLE`), 3 reproductions sans redémarrage |
| **2 — un moniteur virtuel par fenêtre** | **CONDITIONNELLE** | **non mesurée** — garantie par construction seulement | oui — sortie portée par l'adaptateur qui possède NVENC | non mesurée | Sortie DXGI réelle et attachée, 3413×960 ; **remplace** l'écran physique ; plafond au-delà d'une sortie non mesuré (obstacle matériel) |
| **3 — tuilage disjoint** | **CONDITIONNELLE** | **non garantie** — les menus débordent | oui (Desktop Duplication) | 80,2 → 107,5 i/s/fenêtre de N=1 à N=8 (voie `duplication`) | 800×360 par fenêtre à 8 fenêtres sur le bureau réel ; **défaut structurel** ci-dessous |
| **4 — `PrintWindow(PW_RENDERFULLCONTENT)`** | **CONDITIONNELLE** | **OUI, mesurée** — image juste d'une fenêtre D3D recouverte | **non — chemin CPU** | 45,0 i/s à N=1 ; 29,1 i/s/fenêtre à N=2 ; **N=4 et N=8 non mesurés** | Seule voie ayant franchi la porte de correction ; tombe sur la porte « chemin GPU » |

### Voie 1 — `Windows.Graphics.Capture` : ÉLIMINÉE

`journaux-sonde-multifenetre/wgc.log` :

```
GraphicsCaptureSession::IsSupported  supporte=true
verdict WGC : ÉLIMINÉE — la préparation de la capture a échoué
  causes=CreateForWindow sur la mire observée :
         Échec de l'appel de procédure distante. (0x800706BE)
```

Ce qui compte, ce sont les **deux réparations bon marché que la mesure
écarte** :

- `IsSupported()` rend **`true`** — le `E_OUTOFMEMORY` du jalon 1 **ne se
  reproduit plus**. Le mode de défaillance a changé, et aucun crash
  `0xc0000005` n'a été observé.
- Le service `CaptureService_5865d` **existe et tourne** (`StartType: Manual`).

Donc : **ni composant Windows absent, ni service arrêté**. Les deux issues qui
auraient rendu la voie récupérable à peu de frais sont exclues. `0x800706BE`
désigne un serveur RPC injoignable ; la cause profonde n'a pas été élucidée, et
l'échec est reproduit **trois fois à l'identique** sur une VM non redémarrée
entre les essais.

### Voie 2 — un moniteur virtuel par fenêtre : CONDITIONNELLE

Ce qui est **établi** : le mécanisme. Apollo déclenche `AddVirtualDisplay` sur
SudoVDA au démarrage d'une session ; le pilote produit alors une sortie DXGI
réelle, attachée, sur le même GPU que NVENC.

Ce qui est **mesuré** : une sortie, 3413×960 (voir la section DXGI ci-dessus).

Ce qui **n'est pas mesuré** : le plafond. La question décisive — une deuxième
session simultanée produit-elle une deuxième sortie, ou remplace-t-elle la
première ? — exige un second appareil client apparié, **que le propriétaire du
poste n'a pas**. C'est un obstacle matériel, pas une limite du pilote constatée.
Chemin de reprise : (1) un second client apparié ; (2) très vraisemblablement
basculer `dd_configuration_option` de `ensure_only_display` vers
`ensure_active` (« Activate the display automatically », sans désactiver les
autres) — **non vérifié empiriquement**, signalé comme la première chose à
essayer.

### Voie 3 — tuilage disjoint : CONDITIONNELLE, avec un défaut structurel

**Surface.** Grille 3×3 (la plus carrée pour 8), place = `(l/3) & !1` ×
`(h/3) & !1` :

- bureau **réel** 2400×1080 → **800×360** par fenêtre. Nettement sous
  1280×720 : pas de fenêtre de jeu à résolution utile ;
- mode le plus large **annoncé** par WMI 4096×2160 → 1364×720 : la largeur
  dépasse le seuil de 1280, mais la hauteur vaut **exactement** 720, pas
  au-dessus — **et la settabilité de ce mode n'a pas pu être établie**
  (toutes les tentatives de changement ont échoué avant application).

**Le défaut structurel.** Un menu contextuel ouvert près d'un bord de tuile
**déborde sur la tuile voisine**. Mesuré, pas supposé : Bloc-notes posé
exactement sur la tuile (0,0)-(800,360), clic droit à (770,200) — 30 px du bord,
à l'intérieur de la fenêtre ; le menu s'ouvre et s'étend jusqu'à ≈(1084, 502),
soit **≈284 px au-delà du bord droit de la tuile et ≈142 px au-delà de son bord
bas**. Preuve visuelle jointe : `journaux-sonde-multifenetre/menu-deborde-tuile.png`
(capture plein écran 2400×1080, agnostique de la techno de rendu du menu).

*(Le registre du chantier arrondit le débordement bas à 140 px, le rapport de
tâche l'établit à ≈142 px sur l'image ; écart d'arrondi de relecture, sans
incidence sur la conclusion.)*

C'est Windows qui place les menus, et il ne connaît que les frontières de
moniteurs **réels**, pas nos tuiles logicielles. **L'agent ne peut pas
l'empêcher.** Une voie dont l'unique mérite est de garantir le non-recouvrement
ne le garantit donc pas.

### Voie 4 — `PrintWindow(PW_RENDERFULLCONTENT)` : CONDITIONNELLE

`journaux-sonde-multifenetre/replis.log` :

```
PrintWindow(PW_RENDERFULLCONTENT) sur une mire D3D  rendu=true  pixel=(16, 224, 96)  verdict=Juste
verdict PrintWindow : CONDITIONNELLE — image correcte, mais chemin CPU
```

Le pixel `(16, 224, 96)` est exactement `couleur_mire(0, trame_impaire)`
(rouge=16 → identité de la mire 0 ; bleu=96 → mire ; vert=224 → trame impaire).
La mire **recouverte** a bien été rendue, et correctement.

Sa limite n'est donc **pas** la correction d'image, contrairement à ce que le
plan attendait : c'est le **chemin CPU**. `PrintWindow` rapatrie les pixels en
mémoire centrale (`GetDIBits`), qu'il faut ensuite téléverser vers le GPU
(`UpdateSubresource`) pour atteindre NVENC. La spec de conception qualifie le
chemin GPU de **porte éliminatoire** — cette voie y tombe indépendamment de la
justesse de son image.

Coût mesuré, avec ce rapatriement **non contourné** :

**Ce chiffre est un plancher de cette implémentation, pas du chemin CPU en
général.** `VoiePrintWindow::prochaine_image` (`voies.rs:296-322`) alloue et
détruit, **à chaque image**, un DC compatible, un bitmap compatible et leur
sélection (`GetDC`, `CreateCompatibleDC`, `CreateCompatibleBitmap`,
`SelectObject`), plus un `vec![0u8; largeur*hauteur*4]` remis à zéro à chaque
appel (5,2 Mo à N=2). Ces allocations par image ne sont pas ce que la voie
mesure par construction — hisser ces objets hors de la boucle (les allouer une
fois à `ouvrir()`, comme la texture GPU l'est déjà) est à faire avant toute
mesure à N=4/8, pas ici : le vrai chiffre du chemin CPU est meilleur que
45,0/29,1 i/s.

| N | Passe | Cadence/fenêtre | Unités encodées | `verdicts_faux` |
| --- | --- | --- | --- | --- |
| 1 | capture | 45,0 i/s | — | — |
| 1 | capture+encodage | 44,9 i/s | 224 | — |
| 2 | capture | **29,1 i/s** (les deux fenêtres à l'identique) | — | 0 |
| 2 | capture+encodage | *non conclue* | 88 (à l'arrêt, pour 176 images) | 0 |
| 4, 8 | — | **non mesuré** | — | — |

`verdicts_faux = 0` sous recouvrement à N=2 seulement : le banc ne pose de
recouvrement qu'à partir de deux fenêtres (`banc.rs:194`) et ne lit un pixel que
sur la fenêtre recouverte une fois ce recouvrement posé (`banc.rs:209`) — à N=1
aucune lecture n'a jamais lieu, et le `—` des lignes N=1 ci-dessus le reflète.
La justesse mesurée tient donc au banc à N=2, pas à une confirmation
supplémentaire à N=1.

---

## La mesure d'ensemble : la capture ne décroche pas jusqu'à 8

Voie `duplication` (Desktop Duplication recadrée, l'étalon de production),
passe **capture**, cadence **par fenêtre** — journaux
`banc-duplication-{1,2,4,8}.log` :

| N | Cadence/fenêtre | `verdicts_faux` | Suite |
| --- | --- | --- | --- |
| 1 | **80,2 i/s** | — | passe capture+encodage : 75,1 i/s, 369 unités |
| 2 | **99,4 i/s** (×2) | **449** | encodage **sauté** (porte de correction) |
| 4 | **98,8 i/s** (×4) | **449** | encodage **sauté** |
| 8 | **107,5 i/s** (×8) | **536** | encodage **sauté** |

**Aucun décrochage jusqu'à N=8**, et toutes les fenêtres d'un même rang sont
égales entre elles. La cadence *monte* légèrement de N=1 à N=8 ; la cause n'est
**pas établie**. La passe TÉMOIN (mires qui peignent, rien ne capture) va dans
le sens contraire de l'explication qu'on serait tenté d'avancer (« plus de
mires animées ⇒ le bureau change plus souvent ») : elle mesure 7 135 tours/s à
N=2 contre 2 026 à N=8, donc un bureau qui change *moins* souvent, pas plus, à
mesure que N croît. Au moins deux autres explications sont plausibles (effet de
la synchronisation verticale sur la charge GPU totale, artefact de mesure liée
au recadrage) et aucune n'a été vérifiée. Ne pas répéter la parenthèse causale
d'origine ailleurs : elle n'est pas soutenue par la mesure.

**Il n'existe aucune mesure d'encodage multi-fenêtres par cette voie** — non par
oubli, mais **par construction du protocole** : `verdicts_faux > 0` déclenche la
porte de correction, qui coupe la passe d'encodage. C'est le comportement voulu.
La seule mesure d'encodage de cette voie est à N=1 (75,1 i/s, 369 unités).

**Avertissement.** Un premier jeu de mesures de cadences, produit avant la
correction de la famine décrite au piège n°4 ci-dessous, mesurait un artefact
(une fenêtre à ~100 i/s, les autres sous 1,3 i/s) et non un comportement de la
voie. Il est **invalide et n'est pas repris ici** : seules les mesures
produites après correction (« Ronde de correction 1 ») figurent dans ce
document.

---

## Plafond d'encodage mesuré (composant du refus non identifié)

**Formulation bornée, à reprendre telle quelle.** Sur un périphérique D3D11
**unique et partagé**, à 1280×720 / 60 i/s / 8 Mb/s : **8 instances du pipeline
Media Foundation complet réussissent ; la 9ᵉ échoue à la liaison du type
d'entrée** (`MF_E_UNSUPPORTED_D3D_TYPE`, `0xC00D6D76`).
`journaux-sonde-multifenetre/nvenc.log` :

```
encodeur créé  rang=8
plafond NVENC atteint — création du suivant refusée  plafond=8
  causes=Le type d'entrée n'est pas pris en charge pour le périphérique D3D. (0xC00D6D76)
```

**Ce que la mesure n'établit PAS** : que « la limite de sessions NVENC de cette
carte est 8 ». Le transform matériel n°9 **s'instancie pourtant sans
difficulté** — c'est une liaison du type d'entrée qui refuse, sur un périphérique
D3D partagé par neuf clients. Le partage du périphérique est peut-être
lui-même la contrainte limitante.

**Le composant qui refuse n'est pas identifié.** `H264Encoder::new` enchaîne,
entre le log `encodeur matériel retenu rang=9` et l'échec, deux appels
`SetInputType` distincts sans qu'aucun des deux ne portait de `.context()`
avant cette ronde de correction : `configure_input` sur la MFT H.264
(`encode.rs:1426-1436`) **puis** `create_color_converter` sur le Video
Processor MFT (`encode.rs:1153-1222`) — les deux enchaînent un `SetInputType`
qui peut rendre `MF_E_UNSUPPORTED_D3D_TYPE`, et le HRESULT nu ne dit pas
lequel a refusé. Un indice temporel penche pour le convertisseur : l'échec
survient à 27,3 ms quand les rangs réussis prennent ~30 ms chacun — cohérent
avec un refus plus tôt dans la séquence, mais ce n'est **pas une preuve**.
Si le refus vient du Video Processor et non de l'encodeur matériel lui-même,
le plafond serait potentiellement contournable (adapter le convertisseur ou
s'en passer) et le « budget d'ordre 8 » ci-dessous serait faux dans le bon
sens. Corrigé dans ce même passage de revue : les deux `SetInputType`
portent désormais un `.context()` distinct, pour que la prochaine mesure
désigne le composant fautif.

Vérifié par ailleurs : **aucun des 8 n'est un repli logiciel** (`encodeur
matériel retenu` × 8, NVIDIA H.264 Encoder MFT).

Portée : périphérique unique partagé ; 720p / 60 i/s / 8 Mb/s seuls. Une
isolation par fenêtre (un périphérique D3D11 par encodeur) n'est pas couverte.

---

## Pièges rencontrés

Cette section est écrite pour qui reprendra ce terrain. Chaque piège a coûté du
temps réel dans ce chantier.

1. **Il faut *animer* les mires.** Desktop Duplication n'émet une trame que
   **quand le bureau change**. Une mire peinte une fois puis laissée immobile
   fait rendre `DXGI_ERROR_WAIT_TIMEOUT` à toutes les acquisitions suivantes :
   on mesure alors zéro image et on conclut à tort à une panne de capture. Le
   banc repeint les mires à chaque tour, en alternant la composante verte trame
   paire / trame impaire — ce qui sert **aussi** à dater le contenu observé.

2. **Il faut peindre en D3D11, pas en GDI — sans quoi `PrintWindow` serait
   validé à tort.** `PrintWindow` sait redemander à une fenêtre de se redessiner
   par `WM_PRINT` ; sur une fenêtre peinte en GDI, il rendrait donc toujours une
   image juste, quelle que soit sa vraie capacité à capter un contenu composé
   par le GPU. Le résultat aurait été un faux positif franc sur la voie 4.
   L'ensemble des mires est peint via une chaîne d'échange D3D11, et c'est ce
   qui donne son poids au résultat contre-intuitif de cette voie.

3. **Chaque voie doit être sondée dans son propre processus.** Ces API échouent
   par **plantage du processus entier**, pas par code d'erreur : le jalon 1
   avait vu WGC provoquer un `0xc0000005`. Une sonde monolithique perd toutes
   les mesures déjà faites au premier plantage. Le banc est piloté par variables
   d'environnement (`MULTIFENETRE_DXGI`, `MULTIFENETRE_WGC`,
   `MULTIFENETRE_REPLIS`, `MULTIFENETRE_BANC`), un processus par voie, un
   journal par processus.

4. **La lacune de conception du trait `VoieDeCapture` — le piège le plus
   coûteux, et celui qui serait allé en production.** Le trait, tel que le plan
   le définissait, **fondait acquisition et recadrage en un seul appel**
   (`prochaine_image`). Avec N voies partageant une unique
   `IDXGIOutputDuplication`, la première voie du tour consommait
   l'`AcquireNextFrame` et les N-1 suivantes récoltaient `WAIT_TIMEOUT` :
   **famine dès deux fenêtres** (une fenêtre à ~100 i/s, les autres sous
   1,3 i/s). Le premier tableau de mesures ne mesurait que cet artefact.
   Corrigé — l'acquisition est mutualisée une fois par tour
   (`SourceDuplication::amorcer`), chaque voie recadre ensuite vers **sa
   propre** texture par `CopySubresourceRegion`. Deux raisons d'insister :
   - **le plan désignait explicitement ce trait comme « la couture dont le
     chantier D aura besoin »** — la lacune aurait donc été héritée telle quelle
     par le produit ;
   - la correction a fermé **un second défaut latent** que la famine masquait :
     avant elle, toutes les voies écrivaient dans la **même** texture interne de
     `DesktopCapture`, chacune écrasant celle de la précédente. Sans famine pour
     les empêcher d'obtenir une image le même tour, le défaut serait apparu.

5. **DXGI n'autorise qu'*une seule* duplication ouverte par sortie — exactement
   une.** Pas « un nombre très limité », comme un commentaire du code le
   laissait entendre. Un `DesktopCapture` provisoire laissé en vie faisait
   échouer la seconde `DuplicateOutput` en `0x80070057`, rendant **toute** mesure
   de la voie `duplication` impossible quel que soit N. Le correctif est un
   `drop()` explicite au bon endroit.

6. **Un champ WMI de résolution peut être périmé.** `Win32_VideoController`
   annonçait encore 5120×1440 **68 secondes après** que DXGI eut confirmé le
   détachement de la sortie. Pour toute question de résolution, la source de
   vérité est `GetDesc`/`DesktopCoordinates`, pas WMI.

7. **La quasi-totalité des onze rondes de correction de ce chantier portait sur
   des rapports qui affirmaient au-delà de leur relevé, pas sur des bugs.**
   Verdicts prononcés sur une configuration tierce jamais éprouvée, chiffre WMI
   présenté comme « mesuré et confirmé », attente du plan reprise comme
   observation. C'est le mode de défaillance dominant d'un chantier de mesure :
   **le coût n'est pas dans le code, il est dans la discipline de l'énoncé.**

---

## Recommandation pour le chantier D

Aucune voie n'est prête. Voici l'arbitrage, avec son prix.

### Ce que je recommande

**Viser la voie 2 (un moniteur virtuel par fenêtre) comme cible, et retenir la
voie 4 (`PrintWindow`) comme repli mesuré pour les fenêtres non-jeu — après
avoir levé, en tout premier, le plafond de sorties virtuelles.**

Raisonnement :

- **La voie 2 est la seule qui préserve le chemin GPU tout en supprimant le
  problème plutôt qu'en le contournant.** Une fenêtre par moniteur, c'est
  l'absence de recouvrement *par construction* et non par discipline de
  disposition — et la sortie est portée par l'adaptateur qui possède NVENC, donc
  sans copie inter-périphérique. C'est la seule voie qui, si son plafond tient,
  n'impose aucun compromis de qualité.
- **La voie 3 est structurellement disqualifiée**, pas seulement dégradée : le
  débordement des menus détruit sa propriété fondatrice, et l'agent n'a aucun
  moyen d'y remédier. À écarter comme voie principale. (Elle peut survivre comme
  *heuristique de placement* sur un moniteur virtuel unique, en assumant la
  pollution ponctuelle par les menus — pas comme garantie.)
- **La voie 4 est le seul acquis mesuré de correction sous recouvrement.** Son
  chemin CPU la disqualifie pour le jeu, mais 29,1 i/s par fenêtre à N=2 est
  parfaitement acceptable pour un traitement de texte ou un IDE — précisément
  les fenêtres que le modèle multi-fenêtres doit porter à côté du jeu. Il serait
  dommage de la jeter pour une porte qui ne concerne que la famille de contenu
  la plus exigeante.
- **La voie 1 reste architecturalement la bonne réponse** (par fenêtre,
  hors-écran, GPU, sans acrobatie de topologie d'affichage) et c'est la seule
  éliminée pour une raison *d'environnement* plutôt que de conception. Un
  créneau **borné** de diagnostic RPC mérite d'être budgété — mais pas le chemin
  critique, les deux réparations bon marché étant déjà écartées.

### Ce que ça coûte

- **Une dépendance à un pilote d'affichage indirect tiers** (SudoVDA), et
  aujourd'hui à un logiciel tiers pour le piloter (Apollo). Le produit ne peut
  pas durablement dépendre du démarrage d'une session Apollo : il faudra
  soit appeler `AddVirtualDisplay` directement (`SudoVDA.dll`, méthode
  identifiée mais **non éprouvée** dans ce chantier), soit assumer cette
  dépendance.
- **Une topologie d'affichage à gérer.** En `ensure_only_display`, la sortie
  virtuelle *remplace* l'écran physique. Créer et détruire des moniteurs au
  rythme d'ouverture et de fermeture des fenêtres est un état global du système
  d'exploitation, avec ses reconfigurations, ses fenêtres déplacées d'office par
  Windows, et un retour à l'état initial à garantir en cas de plantage de
  l'agent.
- **Deux chemins de capture à maintenir** si le repli voie 4 est retenu — plus
  la règle qui décide lequel s'applique à quelle fenêtre.
- **Un plafond d'encodage mesuré à 8, dont le composant fautif n'est pas
  identifié** (formulation bornée plus haut) : au delà, il faudra suspendre
  l'encodage des fenêtres masquées, ce que la spec de conception prévoyait
  déjà (Page Visibility API), **et** vérifier si un périphérique D3D11 par
  encodeur déplace ce plafond — voire s'il l'annule, si le refus mesuré vient
  du convertisseur de couleur partagé plutôt que de l'encodeur matériel.

### Ce qui reste à lever, par ordre d'utilité

1. **Le plafond de sorties virtuelles.** *Deux appareils clients appariés
   suffiraient.* Essayer d'abord `dd_configuration_option = ensure_active` au
   lieu de `ensure_only_display`. **Sans cette mesure, la voie 2 n'est pas
   spécifiable** : si le plafond est 1, tout l'arbitrage bascule vers la voie 4
   et le chantier D change de nature.
2. **Le plafond d'encodage sur périphériques D3D11 séparés.** Un périphérique
   par encodeur, même banc, même 720p/60/8 Mb/s. Réponse en une demi-journée,
   et elle dimensionne le nombre de fenêtres simultanées du produit.
3. **La correction d'image de la voie 2, réellement mesurée.** Exercer une
   capture sur la sortie virtuelle, avec le banc existant — et en profiter pour
   vérifier le piège d'échelle 1,5 du recadrage (section DXGI).
4. **`printwindow` à N=4 et N=8.** Une heure de mesure, qui dirait si le repli
   tient à l'échelle visée ou seulement à deux fenêtres.
5. **Un créneau borné sur `0x800706BE`** (voie 1) et, si le budget le permet,
   la première instrumentation de `DwmGetDxSharedSurface`, cinquième voie
   jamais sondée.

### Ce que le chantier D hérite, et doit ne pas défaire

- **Le trait `VoieDeCapture` corrigé** : acquisition et recadrage **séparés**,
  une texture de destination **par voie**. Refondre les deux, c'est réintroduire
  la famine et l'écrasement mutuel de textures (piège n°4).
- **Le banc lui-même** (`agent/src/diagnostics/multifenetre/`) : mires D3D11
  animées, disposition en tuiles, porte de correction, journalisation agrégée à
  la seconde. Il est validé, il est réutilisable tel quel pour les mesures 2 à 4
  ci-dessus.
- **La règle de mesure** : jamais de trace par image dans une boucle de
  transport ou de capture (leçon déjà acquise au chantier C) ; agréger à la
  seconde.
