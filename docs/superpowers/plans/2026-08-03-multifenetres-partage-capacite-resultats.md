# Sous-bloc D6 — partage de la capacité réseau entre N flux : résultats

> Ce document se remplit tâche par tâche. Au 3 août 2026 il porte le §1 (la
> porte : ce que le lien porte réellement — **prémisse du sous-bloc réfutée**),
> le §2 (où la charge agrégée décroche, et quel barreau la sauve) et le §3 (la
> recette : les cinq critères, et la calibration de `BUDGET_BPS`).

---

## §1 — La porte : ce que le lien porte réellement, et le verdict sur la prémisse

**Tâche 1. Une exécution du montage WebRTC, trois exécutions du montage brut en
TCP, trois en UDP. Aucun taux n'est revendiqué nulle part.**

### 1.0 Le verdict, d'abord

> ❌ **LA PRÉMISSE DU SOUS-BLOC D6 EST RÉFUTÉE.**
>
> Le pont porte **au moins 1,45 Gb/s en TCP et 1,64 Gb/s en UDP**, avec **zéro
> perte réseau**. Huit fenêtres visant chacune 12 Mb/s, soit **96 Mb/s cumulés,
> ne pouvaient pas le saturer** : c'est **entre 15 et 27 fois** moins que ce que
> le chemin a porté.
>
> L'écart 494,4 → 224,9 i/s et la montée de RTT 2 → 104 ms relevés au sous-bloc
> D4 **ne peuvent donc pas s'expliquer par une saturation du pont**, et
> l'inférence que le rapport de D4 déclarait lui-même comme telle — « aucune
> mesure de charge du pont » — ne tient pas.
>
> Le témoin qui départage, relevé sur **une** fenêtre : le navigateur a **jeté
> 53,6 % des images qu'il a reçues** (3 845 jetées sur 7 177 reçues) tout en
> passant **90,2 % du temps mural dans le seul décodage vidéo** — pendant que le
> réseau perdait **0,0413 %** des paquets. **Le goulot observé est en aval du
> lien, pas dans le lien.**

Ce que cela change pour la suite — et ce que cela ne change pas :

- **Le répartiteur reste juste sur le fond** : huit processus indépendants qui
  sondent chacun vers le même plafond restent une pathologie, et cette mesure en
  montre même l'ampleur (le sondage d'une seule fenêtre a fait monter l'estimation
  jusqu'à **171,4 Mb/s** — voir §1.3).
- **Mais le critère ① ne peut pas se lire comme « la congestion cesse »** : il
  n'y avait pas de congestion de lien à faire cesser. Il retombe sur *le sondage
  cumulé ne dépasse plus le budget*, fait mesurable côté agent (§6.4 de la spec).
- **`BUDGET_BPS` ne peut pas se dériver « de la capacité mesurée avec marge »** :
  cette capacité est de l'ordre du gigabit, et une part de gigabit par fenêtre
  n'a aucun sens de produit. Sa valeur par défaut devient une **décision de
  conception**, à documenter comme non calibrée — pas une constante dérivée d'une
  mesure.

---

### 1.1 Le montage, et ce qu'il n'est pas

Deux montages distincts, parce qu'ils ne répondent pas à la même question.

**(a) Le chemin brut, sans encodeur ni WebRTC.** VM (192.168.3.2) → pont
`internalBridge` → hôte (192.168.3.1), dans le sens du média. Émetteur
PowerShell sur la VM, puits Node sur l'hôte
(`instrument/puits-debit.mjs`, `instrument/debit-tcp.ps1`, `instrument/debit-udp.ps1`).
Journal : `journaux-multifenetres-d6/capacite-brute.log`.

**Pourquoi ce montage existe** : une capacité relevée *à travers* WebRTC est
bornée par l'encodeur, par le sondage BWE et par le décodeur autant que par le
lien. Elle ne peut pas répondre seule à « le pont porte-t-il 96 Mb/s ? ».

**(b) Le produit, une seule fenêtre, plafond très haut.** Superviseur +
capteur + un enfant, `BITRATE=100000000`, source animée
(`instrument/anim-d4.html`, la page canvas de D4), fenêtre Chrome `--app` avec
son propre `--user-data-dir`, navigateur pilote lancé avant le superviseur,
palier de 90 s échantillonné toutes les 5 s.
Instrument : `instrument/pilote-lien-d6.mjs`.
Journaux : `journaux-multifenetres-d6/mesure-lien.log` (pilote),
`agent-mesure-lien.log` (agent, ANSI retirées),
`mesure-lien-navigateur.json` (les 18 échantillons `getStats()` bruts).

**`BITRATE=100000000` est une valeur de SONDE, jamais une valeur de produit** :
elle sert à laisser le BWE monter sans que notre propre plafond ne le borne.

**Le binaire mesuré**, bâti sur la VM par `scripts/build-agent.sh` après avoir
sourcé `.env` (journal versé : `journaux-multifenetres-d6/build-agent.log`,
qui porte `Compiling proto` puis `Compiling agent` — ce n'est pas une
compilation vide) : `C:\dev\target\release\agent.exe`, **9 142 784 octets**,
horodaté `2026-08-03 12:54:18Z`, soit **9 minutes** avant le début de la
séquence de mesure (`13:03:55Z`). Sources à `6e0300b`.

**Contexte de charge, à ne pas taire** : l'hôte portait pendant toute la
séquence une transcodification étrangère (`tdarr-ffmpeg`, ~345 % de CPU) et la
VM (`qemu-system-x86_64`, ~250 %), pour 8 cœurs. Les capacités relevées sont
donc des **planchers**, ce qui ne peut que renforcer le verdict ; la charge CPU
relevée est en revanche **contaminée**, ce qui l'affaiblit comme témoin (§1.5).

---

### 1.2 Grandeur ⓪ — la capacité brute du chemin

Source : `capacite-brute.log`. Les deux bouts comptent le même nombre d'octets à
chaque exécution TCP : ce ne sont pas des estimations.

| Transport | exéc. | Octets, sur 10 s | Débit **calculé** reçu |
| --- | --- | --- | --- |
| TCP | 1 | 1 814 036 480 | **1 448,74 Mb/s** |
| TCP | 2 | 3 240 099 840 | **2 587,39 Mb/s** |
| TCP | 3 | 3 165 650 944 | **2 529,31 Mb/s** |
| UDP | 3 | 2 054 160 000 émis, **tous parvenus au noyau de l'hôte** | **1 643,09 Mb/s** |

**La lecture de l'exécution UDP n°3 est le point le plus important, et elle ne
se lit pas sur le puits.** Le puits Node n'a compté que 362 870 400 octets
(236,44 Mb/s). Les compteurs `/proc/net/snmp` avant/après disent pourquoi :

```
delta: InDatagrams=302712  InErrors=1409408  RcvbufErrors=1409408  InCsumErrors=0
```

`InDatagrams + RcvbufErrors` = **1 712 120** datagrammes parvenus à la couche
socket de l'hôte, contre **1 711 800** émis par la VM (l'excédent de 320 est le
trafic UDP étranger de l'hôte). **La totalité des datagrammes a traversé le
pont** ; les 82 % que le puits n'a pas vus ont été jetés par le noyau faute de
place dans le tampon de réception — **parce que l'instrument ne drainait pas
assez vite, pas parce que le réseau aurait perdu quoi que ce soit.**

⚠️ **Ne jamais citer les 236 à 265 Mb/s des puits UDP comme une capacité de
lien** : ils mesurent le débit de drainage de l'instrument.

---

### 1.3 Grandeur ① — l'estimation BWE en régime

Source : `agent-mesure-lien.log`, lignes `observation réseau` du module
`agent::transport::evenements` (niveau `debug`, activé par
`RUST_LOG='info,agent::transport::evenements=debug'` — aucune trace par paquet
n'a été ajoutée).

**108 lignes**, de `13:04:29.431226Z` à `13:06:16.465479Z`, toutes porteuses
d'une estimation (`absence de bande passante` : **0** occurrence).

| | bits par seconde |
| --- | --- |
| minimum | **53 734 515** |
| médiane | **121 662 684** |
| maximum | **171 432 229** |
| au-dessus de 96 Mb/s | **72 / 108 lignes** |

L'estimation **dépasse le plafond de sonde de 100 Mb/s** posé par
`set_desired_bitrate` : ce plafond n'a donc pas borné le BWE.

⚠️ **Cette estimation n'est PAS une mesure de la capacité du lien, et il ne faut
pas la lire ainsi** : un BWE est borné par ce qui est réellement émis plus le
sondage. Elle plafonne 8 à 15 fois sous la capacité brute du §1.2, ce qui est
**cohérent avec** une borne posée par l'encodeur ou par le sondage plutôt que
par le lien — **sans que la mesure isole laquelle**. C'est précisément pourquoi
le montage brut existe.

---

### 1.4 Grandeurs ② et ③ — ce qui sort, et ce que le navigateur en fait

Source : `mesure-lien-navigateur.json`, 18 échantillons, fenêtre de **85,294 s**
(le palier de 90 s demandé, échantillonné). Grandeurs **relevées** en delta entre
le premier et le dernier échantillon ; les taux sont **calculés**.

| Grandeur | Relevé | Calculé |
| --- | --- | --- |
| Octets vidéo reçus | 73 258 095 → 950 266 257 (Δ 877 008 162) | **82,257 Mb/s** |
| Débit annoncé par l'agent (message `link`) | min 53 400 852, médiane 95 839 480, max 100 000 000 | — |
| Paquets reçus / perdus | 786 789 / **325** | **0,0413 % de perte** |
| Images reçues | Δ **7 177** | — |
| Images **décodées** | Δ **3 307** | 38,77 i/s |
| Images **jetées** (`framesDropped`) | Δ **3 845** | **53,6 % des images reçues** |
| `totalDecodeTime` | 6,048554 s → 82,979655 s (Δ **76,931 s**) | **90,2 % du temps mural** |
| Gels (`freezeCount`) | Δ **1** (1,025 s au total) | — |
| `pliCount` / `nackCount` | Δ **9** / Δ **1** | — |
| RTT navigateur | min 0,001 s, médiane 0,004 s, max 0,069 s | — |
| Chemin ICE | `prflx ↔ host`, **udp** | — |
| Taille encodée | **1280×720** de bout en bout | — |

⚠️ **Le « débit RTP réellement sortant » demandé par le brief n'est PAS relevable
dans `agent.log` avec ce binaire** : `MediaEgressStats` porte un compte d'octets
mais il n'est pas journalisé, et rien d'autre ne le rapporte. Ce que la table
ci-dessus donne est le débit **reçu** par le navigateur ; les 0,0413 % de perte
de paquets bornent l'écart entre les deux. Ce substitut est déclaré, il n'est
pas la mesure demandée.

**La session n'a subi aucun incident** : 0 `ERROR`, 0 `clôture de session
amorcée`, 0 changement de taille d'encodage demandé ou refusé, 0 refus de
réglage de débit à chaud. Les 30 `WARN` du journal sont 29 réceptions UDP
avortées (`os error 10054`) et 1 allocation TURN impossible. Les 29 se répartissent
en **17 pendant l'établissement** (`13:04:28.694956Z` à `13:04:45.221278Z`) et
**12 après la fermeture du navigateur** (à partir de `13:06:10.790792Z`) :
**aucune pendant le palier**. L'allocation TURN ayant échoué, **la session s'est
jouée sans relais, sur candidats directs** (`prflx ↔ host`, udp) — ce qui est
exactement le chemin qu'on voulait mesurer.

---

### 1.5 Grandeur ④ — la charge CPU de l'hôte

Source : `mesure-lien.log`, blocs `CPU HÔTE`, `top -b -n 3 -d 2` sur un hôte à
**8 cœurs**.

| Moment | `%Cpu(s)` inactif | `loadavg` (1 min) |
| --- | --- | --- |
| au repos, avant toute session | 68,5 / 69,5 / 74,4 | 15,79 |
| début du palier | 62,6 / 61,6 / 63,8 | 16,09 |
| fin du palier | 60,7 / 61,7 / 60,3 | 21,35 |

**L'hôte pris globalement n'est PAS saturé** : ~60 % de temps CPU inactif à la
fin du palier. Mais le `loadavg` de 21,35 pour 8 cœurs et la présence continue
de `tdarr-ffmpeg` (~345 % de CPU, **étranger à cette mesure**) rendent ce témoin
**contaminé** : il ne peut ni innocenter ni accuser proprement.

**Le témoin qui vaut n'est donc pas `top`, c'est `totalDecodeTime`** : 76,931 s
de décodage vidéo pour 85,294 s de temps mural, soit **90,2 % d'un cœur passé à
décoder**, sur un Chrome sans interface lancé avec `--disable-gpu`, donc en
décodage logiciel. Ce chiffre vient de `getStats()`, pas de l'hôte, et il n'est
pas contaminé par la charge étrangère. Il désigne un décodeur au plafond.

---

### 1.6 Ce que cette mesure n'établit PAS

- **Elle ne reproduit pas D4.** Ici : **une** fenêtre à 82,257 Mb/s. En D4 :
  **huit** fenêtres à ~10 Mb/s chacune. Le total est du même ordre, le montage
  ne l'est pas. Que la chute de D4 soit due au décodeur du navigateur est
  **cohérent avec** ce relevé — ce n'est **pas prouvé par** lui.
- **Une exécution du montage WebRTC.** Aucun taux, aucune variabilité.
- **Le décodage logiciel est une propriété du montage de recette**, pas du
  produit : `--disable-gpu` sur un Chrome sans interface. Un navigateur à
  décodage matériel n'a pas été éprouvé, ni ici ni en D4.
- **La couche qui borne l'estimation BWE à ~171 Mb/s n'est pas identifiée** :
  encodeur, sondage str0m, pacing — aucune n'est isolée.
- **Les pointes de RTT (jusqu'à 69 ms côté navigateur, 32,7 ms côté agent) à un
  débit 20 fois inférieur à la capacité brute ne sont pas expliquées.** Elles ne
  peuvent pas être une saturation de bande passante du pont ; ce qu'elles sont
  n'a pas été cherché.
- **Rien du sens hôte → VM**, rien d'un régime à N flux concurrents sur le
  montage brut, rien de la gigue, rien de la durée (palier de 90 s).
- **La capacité brute est un PLANCHER** : l'émetteur PowerShell peut avoir borné
  les exécutions UDP, et l'hôte portait une charge étrangère aux trois.
- **Aucune unité H.264 n'a été décodée hors du navigateur ni regardée** : la
  justesse de l'image n'est pas contrôlée.

### 1.7 Contrôle de survie de la VM

`virsh domstate Windows` : « en cours d'exécution » ; accès réel au partage
(`ls /media/vm/dev/anim-d4.html`) : OUI, relevé en fin de mesure
(`mesure-lien.log`, ligne `SURVIE VM (fin de mesure)`). Le journal libvirt ne
porte aucune extinction après `2026-08-03 11:17:44+0000`, soit **1 h 37 avant**
le début de la séquence : la VM a survécu à toute la mesure.

---

## §2 — Où la charge agrégée décroche, et si un barreau plus bas la sauve

**Tâche 1bis. QUATRE exécutions au total : une pour le relevé A (montée
N = 1, 2, 4, 8 à 10 Mb/s par fenêtre) et une par valeur du relevé B (4, 2 puis
1 Mb/s par fenêtre, N = 8 figé). Une exécution par point : aucun taux n'est
revendiqué nulle part.**

### 2.0 Les trois verdicts, d'abord

> **① La charge décroche entre N = 2 et N = 4**, soit entre **18,3 et
> 37,4 Mb/s** cumulés (entre **152 et 294 Mpx/s décodés**). À N = 8 sans budget,
> **18,03 %** des images reçues sont jetées par le navigateur.
>
> **② Le décrochage est intégralement en aval du réseau** : `packetsLost` = **0**
> aux quatre rangs du relevé A **et** aux trois exécutions du relevé B. Zéro.
> Pas « négligeable » — zéro.
>
> **③ ✅ DESCENDRE D'UN BARREAU SAUVE LE DÉCROCHAGE. Le mécanisme de D6 corrige
> bien quelque chose.** À N = 8 : **18,03 %** d'images jetées au barreau plein,
> **7,99 %** un barreau plus bas (1024×576), **1,47 %** trois barreaux plus bas
> (640×360) — et le nombre d'images réellement décodées **monte** en même temps
> (390,74 → 453,02 → 514,69 i/s).
>
> **⚠️ MAIS le témoin à surface CONSTANTE ne sauve rien** : diviser les bits par
> 2,5 sans changer de barreau (4 Mb/s par fenêtre, 7 fenêtres sur 8 restées en
> 1280×720) donne **23,08 %** d'images jetées, c'est-à-dire **pas mieux** qu'au
> barreau plein. **Ce n'est donc pas le débit qui sauve, c'est la RÉSOLUTION.**
> Un `BUDGET_BPS` qui réduirait les bits sans faire franchir de seuil de barreau
> ne corrigerait rien.

### 2.1 Le montage, et le raccourci qui le rend fidèle

Même instrument que le §1, augmenté d'une montée en N et d'un palier par rang
dont les compteurs sont pris en **delta** :
`journaux-multifenetres-d6/instrument/pilote-decrochage-d6.mjs`.
Source animée (`anim-d4.html`), un `--user-data-dir` par fenêtre, palier de
**35 s** par rang, navigateur pilote lancé avant le superviseur, aucune capture
d'écran CDP, évaluations CDP bornées, `agent.log` copié après la fin réelle,
survie de la VM contrôlée après chaque rang, agents tués et sorties virtuelles
purgées entre chaque exécution.

**Le raccourci** : `BITRATE` est hérité **tel quel** par chaque enfant
(`superviseur/lanceur.rs`). Poser `BITRATE = B/N` **simule donc exactement** ce
que D6 donnerait avec des parts égales. Ce qui n'y est **pas** : la majoration
de la fenêtre au premier plan (`FACTEUR_FOCUS`).

**L'instrument le plus lourd, hérité de D5 et obligatoire ici** : un Chrome sans
interface rapporte `document.hidden = true` pour toute fenêtre d'arrière-plan.
Sans l'override de visibilité posé page par page, le vivier de D5 endormirait
les fenêtres et l'on mesurerait le sommeil au lieu de la charge. **Contrôle :
`fenêtre endormie` vaut 0 aux quatre exécutions** (champ `marqueurs.endormie`
des quatre fichiers `decrochage-*.json`).

**L'échelle des barreaux**, pour une source 1280×720 et le `fps: 60` de
`transport.rs`, **calculée** depuis `congestion/echelle.rs`
(`DIVISEURS = [1.0, 1.25, 1.5, 2.0]`, `BPP_MIN = 0.05`) :

| Barreau | Taille | `min_bps` | `BITRATE` par fenêtre qui l'impose |
| --- | --- | --- | --- |
| 0 | 1280×720 | 2 764 800 | ≥ 2,9 Mb/s |
| 1 | 1024×576 | 1 769 472 | 2 Mb/s |
| 2 | 852×480 | 1 226 880 | ~1,4 Mb/s |
| 3 | 640×360 | 691 200 | 1 Mb/s |

C'est ce calcul qui a fait **écarter la valeur de 4 Mb/s que le brief suggérait
comme « un barreau plus bas »** : à 4 Mb/s le barreau **ne bouge pas**. Elle a
été conservée, mais comme **témoin à surface constante** — et c'est elle qui
porte le résultat le plus tranchant du §2.0. Les tailles annoncées ci-dessous
sont **relevées** (`largeur=`/`hauteur=` dans `agent.log`, et `frameWidth`/
`frameHeight` dans `getStats()`), pas déduites de ce tableau.

### 2.2 Relevé A — la montée à 10 Mb/s par fenêtre (UNE exécution)

Source : `decrochage-a-10mbps.log` / `.json`, palier de 35 s par rang. Deltas
**relevés** ; débits, cadences et pourcentages **calculés**.

| N | Mb/s cumulé | i/s décodées | i/s au capteur | images jetées | **% jetées** | décodage cumulé | Mpx/s décodés | `packetsLost` | gels |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 9,479 | 84,94 | 85,03 | 0 | **0 %** | 48,7 % | 78,28 | **0** | 0 |
| 2 | 18,349 | 165,04 | 165,40 | 0 | **0 %** | 96,4 % | 152,10 | **0** | 0 |
| 4 | 37,386 | 319,25 | 336,30 | 623 | **4,66 %** | 270,7 % | 294,22 | **0** | 1 |
| 8 | 43,715 | 390,74 | 477,84 | 3 517 | **18,03 %** | 457,6 % | 360,11 | **0** | 5 |

*(« décodage cumulé » est la SOMME des `totalDecodeTime` rapportés à la durée du
palier, en % d'un cœur ; l'hôte en a 8. « i/s au capteur » vient des lignes
`cadence du capteur` d'`agent.log`, restreintes à la fenêtre du palier.)*

Trois lectures que ce tableau porte et qu'il ne faut pas perdre :

- **le rapport décodé/produit s'effondre avec N** : 0,999 · 0,998 · 0,949 ·
  **0,818**. À N = 8 le capteur produit 477,84 i/s et le navigateur n'en décode
  que 390,74 — **c'est exactement la forme du décrochage de D4** (494,4 produites
  contre 224,9 décodées) ;
- **le réseau est hors de cause à tous les rangs** : `packetsLost` = 0, RTT
  médian de 1 à 13 ms ;
- **le débit cumulé n'atteint jamais les 80 Mb/s visés** (43,7 relevés) : le
  capteur lui-même retombe de 85 i/s par fenêtre à 47,8–79,4. **Les deux bouts
  se dégradent**, et ce montage ne les départage pas.

À N = 8, `agent.log` porte **18** `taille d'encodage changée` : l'adaptation
existante a bien fait descendre des barreaux d'elle-même (1024×576 puis 852×480)
**puis est remontée** — les huit fenêtres sont en 1280×720 à la fin du palier.
C'est le comportement oscillant qu'un budget imposé remplacerait par un barreau
tenu.

### 2.3 Relevé B — à N = 8, un barreau plus bas (UNE exécution par valeur)

Sources : `decrochage-b-{4,2,1}mbps.log` / `.json`. Même montage, N = 8 figé.

| `BITRATE`/fenêtre | Budget de session **équivalent** | Taille **relevée** | Mb/s cumulé | i/s décodées | **% jetées** | décodage cumulé | Mpx/s | `packetsLost` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 10 Mb/s (§2.2) | 80 Mb/s | 1280×720 (8/8) | 43,715 | 390,74 | **18,03 %** | 457,6 % | 360,11 | **0** |
| **4 Mb/s (témoin)** | 32 Mb/s | **1280×720 (7/8)** | 19,574 | 333,25 | **23,08 %** | 408,6 % | 278,26 | **0** |
| 2 Mb/s | 16 Mb/s | **1024×576 (8/8)** | 11,023 | 453,02 | **7,99 %** | 374,2 % | 267,20 | **0** |
| 1 Mb/s | 8 Mb/s | **640×360 (8/8)** | 5,930 | 514,69 | **1,47 %** | 150,4 % | 118,58 | **0** |

Le contrôle que le barreau a réellement bougé, **relevé** dans `agent.log`
(champ `marqueurs.taille_changee`) : **2** changements à 4 Mb/s (une seule
fenêtre est descendue, à 852×480), **16** à 2 Mb/s, **16** à 1 Mb/s — soit deux
lignes par fenêtre pour huit fenêtres, aux deux valeurs qui franchissent un
seuil. **0 refus** (`taille_refusee`) aux quatre exécutions : le défaut de D4,
mort en D5, ne se réveille pas.

**Le témoin à surface constante est ce qui donne son sens au reste.** À 4 Mb/s
la charge de bits est divisée par 2,2 (43,7 → 19,6 Mb/s) et le taux d'images
jetées **ne baisse pas** — il est relevé à 23,08 % contre 18,03 %. *L'écart
entre ces deux nombres, sur une exécution chacun, n'est pas nécessairement
significatif ; ce qui l'est, c'est l'absence de toute amélioration, à opposer
aux baisses nettes obtenues dès qu'un barreau est franchi.*

### 2.4 La valeur proposée pour `BUDGET_BPS`, et le chemin qui y mène

**Proposition : `BUDGET_BPS = 8_000_000` (8 Mb/s de budget de session).**

Le raisonnement, point par point, chaque étape adossée à un relevé :

1. **C'est le seul point mesuré propre à N = 8** : 8 Mb/s de budget donnent
   1 Mb/s par fenêtre, donc le barreau 640×360, donc **1,47 %** d'images jetées
   et **514,69 i/s** décodées — le meilleur des quatre exécutions sur les deux
   grandeurs à la fois.
2. **Le point immédiatement au-dessus est mesuré MAUVAIS** : 16 Mb/s de budget
   (2 Mb/s par fenêtre, 1024×576) donnent **7,99 %**. La marge n'est donc pas
   confortable : **le budget propre et le budget sale ne sont séparés que d'un
   facteur 2**.
3. **Il ne dégrade pas les petits N par rapport à ce qui est mesuré propre** :
   à N = 1 il donne 8 Mb/s, sous les 9,479 Mb/s relevés à 0 % d'images jetées ;
   à N = 2 il donne 4 Mb/s par fenêtre, sous les 9,1–9,2 relevés à 0 %.
4. **Aucune marge n'est ajoutée au-delà de cela**, et c'est délibéré : la marge
   habituelle se prend sur une capacité dont on connaît la forme, or ici la
   grandeur qui commande n'est ni un débit ni une capacité de lien mais **la
   charge de décodage du client**, dont ce montage ne donne que quatre points.

⚠️ **Trois réserves que cette valeur porte, et qui doivent voyager avec elle :**

- **Elle coûte au cas mono-fenêtre.** `BITRATE` vaut aujourd'hui 12 Mb/s par
  défaut ; un budget de session de 8 Mb/s **abaisse** ce que reçoit une fenêtre
  seule. **12 Mb/s à N = 1 n'a jamais été mesuré** — ni propre, ni sale : les
  points existants à N = 1 sont 9,479 Mb/s (0 % jetées, §2.2) et 82,257 Mb/s
  (53,6 % jetées, §1.4).
- **Le barreau intermédiaire n'est pas mesuré.** Un budget de 12 Mb/s donnerait
  1,5 Mb/s par fenêtre à N = 8, donc le barreau 852×480, **entre** les deux
  points mesurés (267 Mpx/s à 7,99 % et 118,6 Mpx/s à 1,47 %). Personne ne sait
  ce qu'il rend.
- **La grandeur qui commande n'est pas celle que le budget règle.** Le témoin à
  surface constante l'établit : ce sont les **pixels décodés par seconde**, pas
  les bits. Le budget n'agit que par l'intermédiaire de l'échelle, en marches
  discrètes. **`BUDGET_BPS` doit donc être choisi pour franchir un seuil de
  barreau à la valeur de N visée, pas pour « laisser de la marge ».**

### 2.5 La charge de l'hôte, et ce qu'elle a fait pendant les quatre exécutions

Relevé par `top -b -n 3 -d 2` et `ps -o pcpu -C tdarr-ffmpeg` à chaque rang
(champ `cpu` des quatre `.json`). Hôte à **8 cœurs**.

| Exécution | `tdarr-ffmpeg` %CPU au repos → à N = 8 | `loadavg` au repos → à N = 8 | `%Cpu` inactif à N = 8 |
| --- | --- | --- | --- |
| A, 10 Mb/s | 111 → 108 | 5,48 → 15,60 | 43,7–48,3 % |
| B, 4 Mb/s | 108 → 107 | 7,19 → 19,01 | 46,5–47,1 % |
| B, 2 Mb/s | 107 → 106 | 16,56 → 18,51 | 46,6–50,2 % |
| B, 1 Mb/s | 106 → 106 | 14,72 → 16,10 | 48,4–52,0 % |

**La contamination signalée au §1 a changé d'ampleur, et il faut le dire** :
`tdarr-ffmpeg` tournait à **~345 %** pendant la mesure de la tâche 1 ; il tourne
à **106–111 %** pendant les quatre exécutions du §2. **Les quatre exécutions du
§2 sont donc comparables entre elles** (variation de 5 points sur toute la
campagne) **et ne le sont PAS avec le §1.**

Le fait qui pèse : **l'hôte n'est globalement jamais saturé** — 43 à 52 % de
temps CPU inactif au rang où 18 % des images sont jetées — et **aucune fenêtre
ne sature son propre décodeur** : les `totalDecodeTime` par fenêtre valent 43,7
à 68,9 % à N = 8, moins qu'à N = 4 (63,4 à 69,6 %). **Le décrochage ne
s'explique donc ni par une machine pleine ni par un décodeur individuel au
plafond**, et ce montage ne dit pas ce qui l'impose.

### 2.6 Ce que le §2 n'établit PAS

- **Une exécution par point. Aucun taux, aucune variabilité, aucune barre
  d'erreur.** Les comparaisons entre exécutions distinctes (A contre B) portent
  cette limite entière.
- **Le décrochage n'est pas bracketé finement** : il y a un trou entre N = 2
  (0 %) et N = 4 (4,66 %), soit entre 18,3 et 37,4 Mb/s et entre 152 et
  294 Mpx/s. Ni N = 3, ni un rang intermédiaire en débit n'ont été joués.
- **La composante qui jette les images n'est pas identifiée.** L'hôte n'est pas
  saturé, aucun décodeur individuel ne l'est, le réseau ne perd rien. `gels`,
  `pliCount` et le rapport décodé/produit disent qu'il se passe quelque chose ;
  **rien ici ne dit quoi.**
- **Les deux bouts se dégradent ensemble** : à N = 8 le capteur lui-même tombe
  de 85 à 47,8–79,4 i/s par fenêtre. Ce montage **ne départage pas** la part du
  décrochage imputable à la VM de celle imputable au navigateur.
- **La valeur trouvée est DE LABORATOIRE.** Le navigateur de recette est un
  Chrome sans interface, `--disable-gpu`, donc en **décodage logiciel**, sur
  l'hôte qui porte aussi la VM et une transcodification étrangère. **Un client
  réel, sur une autre machine, avec décodage matériel, décrocherait ailleurs** —
  probablement bien plus haut. `BUDGET_BPS` ne peut pas être présenté comme une
  constante du produit.
- **`FACTEUR_FOCUS` n'est pas exercé** : le raccourci `BITRATE = B/N` simule des
  parts **égales**, pas la majoration de la fenêtre au premier plan.
- **Rien de la latence de bout en bout**, rien de la durée (paliers de 35 s),
  une seule application, une seule animation, aucun clavier, aucun audio, aucun
  redimensionnement, aucun recouvrement.
- **Le barreau 852×480 à huit fenêtres n'est pas mesuré** (voir §2.4).

### 2.7 Contrôle de survie de la VM

`virsh domstate Windows` : « en cours d'exécution » ; accès réel au partage
contrôlé **après chaque rang** des quatre exécutions (lignes `SURVIE VM`,
`acces_partage=OUI` partout). Le journal libvirt ne porte aucune extinction
après `2026-08-03 11:17:44+0000`, soit **2 h 03 avant** le début du relevé A.
Sorties virtuelles purgées avant chaque exécution et après la dernière
(`purge terminée retirees=… avant=… apres=1`, journal
`decrochage-b-enchainement.log`).

---

## §3 — La recette : le budget arbitré par le capteur, à huit puis dix fenêtres

**Tâche 10. SEPT exécutions du produit complet : cinq à `BUDGET_BPS = 12 000 000`
(A, B, C, D, G) et deux à `8 000 000` (E, F). Une seule d'entre elles, la D, et
deux des exécutions à 8 Mb/s, les E et F, se sont jouées sous une charge d'hôte
comparable à celle du §2. Aucun taux n'est revendiqué nulle part.**

### 3.0 Les verdicts, d'abord

> **① Les cinq critères sont tenus, mais DEUX D'ENTRE EUX SOUS RÉSERVE, et la
> réserve est écrite dans le verdict lui-même** — ① n'est **pas reproduit** hors
> de la charge d'hôte de référence, et ④ n'est **pas systématique** (11
> déplacements de focus sur 14 — mais les trois échecs sont imputés au
> **protocole de mesure**, pas au produit : la promotion a lieu après la fin du
> palier, relevée 63 et 66 s plus tard sur la même fenêtre, voir §3.4 ④). À huit fenêtres, `BUDGET_BPS = 12 000 000` :
> **3,94 %** des images reçues sont jetées (seuil : **< 7,99 %**), les sept
> fenêtres non focalisées se posent au barreau **852×480** — celui que personne
> n'avait mesuré — et la focalisée à 1024×576, la somme des parts vaut
> **11 999 997** pour un budget de 12 000 000, et les deux endormies reçoivent
> exactement `PART_DORMANTE_BPS` en n'émettant **rien**. Le détail critère par
> critère, avec ses réserves, est au §3.4.
>
> **② ⚠️ LE CRITÈRE ① EST TENU SOUS CHARGE D'HÔTE DE RÉFÉRENCE, ET NON
> REPRODUIT AUTREMENT.** 3,94 % à l'exécution D. **Une exécution sur cinq à
> 12 Mb/s passe le seuil ; une sur deux** si l'on ne retient que celles dont
> l'échelle d'encodage s'était posée — la seule population honnête, et celle
> que l'instrument a été modifié pour garantir. Les autres relèvent 8,03 %,
> 16,70 %, 59,32 % et 75,47 %. La dégradation covarie avec le `loadavg` de
> l'hôte **et** avec le non-établissement de l'échelle ; **les deux ne sont pas
> départagées**. **Le produit n'est pas démontré robuste sous la charge d'hôte
> réellement rencontrée pendant la campagne.**
>
> **③ `BUDGET_BPS` est RECONDUIT à 12 000 000, et ce choix est ASSUMÉ, PAS
> DÉMONTRÉ.** Le repli à 8 Mb/s prévu par le brief a été mesuré — **1,46 %** et
> **3,94 %** d'images jetées sur deux exécutions — mais au barreau **640×360**.
> La comparaison est un **arbitrage** et non une domination : 852×480 livre
> **196,4 MP/s** décodés contre 95,5 à 135,6, et en jette 3,94 % contre 1,46 à
> 3,94 %. **Plus de pixels livrés, davantage jetés.** La seule raison qui ne se
> discute pas est que 12 Mb/s **ne coûte rien au cas mono-fenêtre**. Voir §3.5.
>
> **④ ❌ AUCUN TÉMOIN HONNÊTE DE `set_desired_bitrate` N'A ÉTÉ TROUVÉ**, et
> cette recette peut dire **pourquoi** plutôt que de simplement l'avouer : une
> fenêtre endormie, dont l'objectif de sondage vaut 256 000 bps et qui
> n'encode rien, émet **0,000 Mb/s** sur 30 s, aux **six** exécutions où le cas
> est exercé. Ce montage ne distingue donc pas « le sondage est borné à la
> part » de « il n'y a pas de sondage du tout ». ⚠️ **Une voie n'a PAS été
> essayée et aurait pu répondre** : l'A/B différentiel sur ce montage même —
> neutraliser l'appel, opposer les deux trafics cumulés. Voir §3.6.

### 3.1 Le montage, et les trois écarts avec celui du §2

Instrument : `journaux-multifenetres-d6/instrument/pilote-recette-d6.mjs`,
versé dans son état final. Dérivé de `pilote-decrochage-d6.mjs` (§2). Trois
écarts, chacun nécessaire :

1. **Le produit fait le travail, il n'est plus simulé.** Le §2 posait
   `BITRATE = B/N` sur chaque enfant ; ici `BUDGET_BPS` va au capteur, qui
   calcule et pousse les parts sur le canal média. `BITRATE` reste à sa valeur
   de produit (12 000 000) sur tous les enfants : c'est la part qui borne.
2. **Le FOCUS est imposé page par page**, et pas seulement la visibilité.
   `client/src/main.ts` lit `document.hasFocus()` ; la règle de part en dépend
   (`FACTEUR_FOCUS`). Sans cet override, le critère ④ ne mesurerait rien.
3. **Le palier ne commence qu'une fois l'échelle POSÉE** — douze secondes sans
   aucun `taille d'encodage changée`, dans la limite de 90 s. Ce n'était pas le
   cas de l'exécution A, et c'est ce qui la disqualifie (§3.3).

Trois phases enchaînées dans **une seule** session d'agent : palier de 40 s à
huit fenêtres (critères ①②③), deux déplacements de focus de 25 s (critère ④),
puis montée à dix fenêtres avec palier de 30 s (critère ⑤). Source animée
(`instrument/anim-d4.html`), un `--user-data-dir` par fenêtre, navigateur
pilote lancé avant le superviseur, aucune capture d'écran CDP, évaluations CDP
bornées, `agent.log` copié après la fermeture du navigateur, agents tués et
sorties virtuelles purgées entre chaque exécution (⚠️ **sans pièce versée pour
les purges intermédiaires** — voir §3.9).

**Le binaire mesuré** : `C:\dev\target\release\agent.exe`, **9 146 880 octets**,
horodaté `2026-08-03 17:01:34 UTC` (relevé par `ls --time-style=full-iso` sur le
partage), soit **cinq minutes** avant la première exécution (`17:06`). Sources à
`f7557d3`. ⚠️ **Le journal de compilation versé
(`build-agent-t10-queue.log`) n'en est que la QUEUE** — 25 lignes capturées par
un `tee` placé après un `tail`. Il porte `Finished release profile in 11,58s`,
ce qui **ne suffit pas** à prouver une compilation non vide ; la preuve de
fraîcheur est la taille et l'horodatage du binaire (9 142 784 octets au §1
contre 9 146 880 ici, donc un lien réellement refait).

**Les sept exécutions portent le même binaire** : aucune compilation n'a eu
lieu entre la première et la dernière.

**Un champ de trace a été ajouté pour cette recette** (commit `f7557d3`) :
`part de budget appliquee` porte désormais `session=`. Tous les enfants
partagent le même `agent.log` depuis D4 ; sans ce champ, la somme du critère ③
se calculerait sur un multiensemble de nombres anonymes. C'est une divergence
signalée avec le brief, qui donnait cette trace comme source sans qu'elle soit
attribuable.

### 3.2 Le contrôle que la variable est arrivée

Relevé dans les **sept** `agent.log` versés, séquences ANSI retirées :

| Exécution | ligne relevée |
| --- | --- |
| A, B, C, D, G | `budget de debit de la session budget_bps=12000000` — **1** occurrence |
| E, F | `budget de debit de la session budget_bps=8000000` — **1** occurrence |

**La valeur est comparée, pas seulement la présence de la ligne.** Elle égale
ce qui a été posé dans les **sept** cas.

⚠️ **Le contrôle automatique du pilote a rendu `[]` aux sept exécutions, et ce
vide ne voulait rien dire** : `budget_bps()` est un `OnceLock` dont la trace ne
part qu'au **premier calcul de parts**, donc à la première fenêtre — or le
pilote interrogeait le journal six secondes après le lancement du superviseur.
L'instrument versé a été corrigé (contrôle déplacé après la phase 1) et porte
l'avertissement. **Un contrôle anti-piège qui se déclenche trop tôt est un
contrôle qui ne contrôle rien.**

### 3.3 Les sept exécutions, et laquelle compare quoi

Toutes les grandeurs de ce tableau sont **relevées** dans les `.json` de la
phase 1 (champ `phases.palier.cumule` et `phases.palier.cpu`) ; `pc_jetees` est
**calculé** par le pilote comme `100 × images_jetées / images_reçues`, sur des
deltas relevés entre deux appels de `getStats()`.

| Exéc. | Budget | Échelle posée | **% jetées** | i/s décodées | MP/s décodés | Mb/s reçus | RTT médian | `loadavg` | `tdarr` %CPU |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **D** | **12 Mb/s** | oui, 18,4 s | **3,94** | 451,24 | **196,40** | 8,031 | 4 ms | 13,70 → 16,75 | 97,9 |
| **E** | **8 Mb/s** | oui, 12,3 s | **1,46** | 479,40 | 135,57 | 4,931 | 5 ms | 13,01 → 14,64 | 97,1 |
| **F** | **8 Mb/s** | oui, 12,2 s | **3,94** | 414,65 | 95,54 | 4,433 | 4 ms | 15,78 → 18,14 | 97,3 |
| A | 12 Mb/s | **non mesurée** | 8,03 | 467,86 | 182,86 | 8,560 | 5 ms | 14,60 → 20,19 | 108 |
| G | 12 Mb/s | oui, 12,6 s | 16,70 | 391,94 | 168,75 | 7,778 | 8 ms | 23,17 → 29,11 | **351 → 333** |
| B | 12 Mb/s | **non** (60 chgts) | 59,32 | 213,97 | 77,26 | 8,215 | 9 ms | 31,77 → 34,10 | 104 |
| C | 12 Mb/s | **non** (50 chgts) | 75,47 | 141,34 | 56,84 | 9,071 | 63 ms | 35,01 → 38,08 | 98,0 |

**La règle de sélection, écrite ici parce que son absence est ce qui rend un
sous-ensemble possible** : est retenue comme *comparable* une exécution qui
remplit les **deux** conditions — échelle d'encodage **posée** avant le palier,
et charge d'hôte dans la bande du §2. **D, E et F les remplissent ; A, B, C et
G non**, chacune pour une raison nommée :

- **A** : **échelle non posée** — son palier a **chevauché la descente de
  barreau**, le dernier `taille d'encodage changée` tombant 4 s avant la fin,
  et l'attente d'échelle posée n'existait pas encore dans l'instrument. **C'est
  son seul disqualifiant** : sa charge d'hôte (`loadavg` 14,60 → 20,19,
  `tdarr` 108) est **dans la même bande que celle du §2**, et une rédaction
  antérieure de ce paragraphe la rangeait à tort parmi les exécutions « sous
  charge plus lourde ».
- **B** : **échelle non posée** (60 changements) **et** `loadavg` 31,8 → 34,1,
  **et** `RUST_LOG='info,agent::transport::evenements=debug'` posé pour tenter
  d'observer l'estimation BWE (§3.6). ⚠️ **Trois variables : la dégradation
  n'est attribuée à aucune**, et le niveau `debug` a été abandonné.
- **C** : **échelle non posée** (50 changements, 91,1 s sans se poser)
  **et** `loadavg` 35,0 → 38,1.
- **G** : **échelle posée** (12,6 s), mais `tdarr-ffmpeg` à **351 % de CPU**,
  3,6 fois sa valeur de référence. Son exécution a par ailleurs été
  **interrompue en phase 3** par le délai de l'outil qui la portait : sa
  phase 1 et sa phase 2 sont complètes et versées, il n'y a pas de `.json`.

⚠️ **Ce que ce tableau ne permet PAS de dire, et qu'une rédaction antérieure
disait** : « entre D et C, seule la charge étrangère de l'hôte change ». C'est
**faux**, et le tableau au-dessus le contredit lui-même. **Trois choses au
moins diffèrent** :

1. **l'échelle** — D : posée en 18,4 s après 24 changements ; C : **jamais
   posée**, 50 changements en 91,1 s. C'est exactement le disqualifiant appliqué
   à A, et le §3.8 en fait un piège nommé ;
2. **la charge étrangère NOMMÉE est identique** — `tdarr-ffmpeg` vaut **97,9**
   pour D et **98,0** pour C. Le facteur 3,6 invoqué appartient à **G seule**.
   Ce qui sépare D de C est le seul `loadavg`, **dont la composition n'est
   relevée nulle part** ;
3. **le `loadavg` est confondu avec la durée du montage avant le palier** —
   celui-ci commence à 81,8 s pour D et 158,2 s pour C, et `loadavg` est une
   moyenne à une minute : il mesure en partie l'instrument lui-même. Le `%Cpu`
   inactif, lui, ne diffère que de 6 points (44,9–53,0 contre 45,7–46,6).

**L'attribution de la dégradation à la charge de l'hôte est donc INFÉRÉE, pas
établie**, et il reste **deux variables jamais départagées** : la charge d'hôte
et le non-établissement de l'échelle. C'est la règle appliquée à B ; elle vaut
pour C.

Ce que le tableau autorise, et rien de plus : **la performance du produit
mesurée par ce montage varie d'un facteur 19 sur le taux d'images jetées
(3,94 % à 75,47 %) à binaire, budget et protocole identiques.** **Le montage de
recette mesure son hôte au moins autant que le produit.**

### 3.4 Critère par critère

**Les verdicts sont GRADUÉS** — ce dépôt écrit « TENU sur le fond » et « NON
TENU » sur deux lignes d'un même critère quand c'est la vérité (D5), et
« MESURÉ » quand il n'y a pas de seuil.

| # | Critère | Seuil | Verdict | Exécutions |
| --- | --- | --- | --- | --- |
| ① | `framesDropped` en % des images reçues, à 8 fenêtres | **< 7,99 %** | **TENU sous charge d'hôte de référence — NON REPRODUIT autrement** | **1 sur 5** à 12 Mb/s ; **1 sur 2** parmi celles dont l'échelle s'est posée |
| ② | La taille d'encodage des huit fenêtres a **bougé** | le barreau a bougé | **TENU** — 852×480 à 12 Mb/s, 640×360 à 8 Mb/s, contre 1280×720 sans budget | **7 sur 7** |
| ③ | Somme des parts accordées ≤ `BUDGET_BPS` | ≤ budget | **TENU** — jamais dépassé, dans **aucune** phase d'**aucune** exécution | **7 sur 7** |
| ④ | Barreau de la focalisée > celui des autres éveillées | strictement | **TENU en majorité, PAS SYSTÉMATIQUE** — **11 déplacements sur 14** | **7** (2 déplacements chacune) |
| ⑤ | Une endormie reste au plancher, octets RTP au plancher | 256 000 bps | **TENU** — 256 000 bps exactement, **0,000 Mb/s sur 30 s** | **6 sur 6** (G n'atteint pas sa phase 3) |

**① — le verdict, en toutes lettres.** **3,94 %** à l'exécution D, contre un
seuil de 7,99 %. **Une exécution sur cinq à 12 Mb/s passe le seuil, et une sur
deux parmi celles dont l'échelle s'est posée** (D à 3,94 %, G à 16,70 % ; A, B
et C n'ont pas posé leur échelle). La dégradation covarie avec le `loadavg` de
l'hôte **et** avec le non-établissement de l'échelle ; **les deux ne sont pas
départagées** (§3.3). **Le produit n'est pas démontré robuste sous la charge
d'hôte réellement rencontrée pendant la campagne.**

**Le barreau 852×480 à huit fenêtres, que le §2.4 nommait comme le trou du
relevé B, est mesuré**, et il faut lire ce qu'il donne comme un **arbitrage** :

| Barreau | % jetées | MP/s décodés | source |
| --- | --- | --- | --- |
| 1024×576 | 7,99 % | **267,20** | §2.3 (relevé B, 2 Mb/s/fenêtre) |
| **852×480** | **3,94 %** | **196,40** | **§3, exécution D** |
| 640×360 | 1,47 % | 118,58 | §2.3 (relevé B, 1 Mb/s/fenêtre) |

⚠️ **852×480 ne domine PAS le barreau du dessus** : il en jette moitié moins
(−51 %) mais en livre un quart de moins (−26 % de pixels décodés par seconde).
Une rédaction antérieure de ce paragraphe écrivait « meilleur sur les deux
grandeurs à la fois » — **c'est faux**, 196,40 < 267,20. Ce que le point neuf
apporte est qu'il tombe **au-dessus de l'interpolation linéaire** entre ses deux
voisins sur le taux d'images jetées : l'interpolation en prédisait ≈ 4,7 %, il en
donne 3,94 %. ⚠️ **Et cette comparaison croise trois exécutions distinctes de
deux montages distincts** (le §2 simulait le budget par `BITRATE = B/N`, le §3
le fait produire par le capteur) : elle est **indicative, pas contrôlée**.

**② — le barreau a bougé, et il tient.** À 12 Mb/s, les sept fenêtres non
focalisées sont à **852×480** et la focalisée à **1024×576** (champs `taille`
et `lien_taille` de `recette-d-budget12.json`) ; sans budget, le §2.2 relevait
**1280×720 (8/8)**. À 8 Mb/s : **640×360** pour les sept. Le nombre de
changements de barreau nécessaires pour s'y poser : **12** à 12 Mb/s, **10** et
**9** à 8 Mb/s.

⚠️ **Tous les compteurs de barreau de ce §3, y compris les champs `pose` des
`.json` (24, 20, 18, 50, 76, 34, 16…), comptent des LIGNES et valent donc le
DOUBLE du nombre de changements.** Un changement produit **deux** lignes au même
horodatage, à ~70 µs d'intervalle : une d'`agent::windows_source::encodage`
(« taille d'encodage changée sans toucher à la fenêtre ») côté capteur, et une
d'`agent::transport::adaptation` (« taille d'encodage changée ») côté enfant.
Le §2.3 le savait et le disait ; l'instrument de ce §3 les compte toutes deux.
**Défaut préexistant, sans effet sur aucune conclusion** — le critère « échelle
posée » se juge sur 12 s **sans aucune ligne**, ce qu'un double comptage ne
change pas — mais les nombres bruts sont à diviser par deux.

**③ — la somme des parts.** Relevé nominal, session par session, dans les
traces `part de budget appliquee session=… part_bps=…` :

| Exécution | phase | somme relevée | budget | dépassement |
| --- | --- | --- | --- | --- |
| A, B, C, D, G | palier 8 fen. | **11 999 997** | 12 000 000 | non |
| B, C, D | 10 fen., 2 endormies | **11 999 996** | 12 000 000 | non |
| A | 10 fen., 2 endormies | **12 000 000** | 12 000 000 | non |
| E, F | palier 8 fen. | **7 999 992** | 8 000 000 | non |
| E, F | 10 fen., 2 endormies | **8 000 000** | 8 000 000 | non |

Le régime 1 de `repartir` (le seul qui garantisse le non-dépassement) est donc
le seul rencontré. **Les régimes 2 et 3 n'ont jamais été exercés** — il aurait
fallu un budget dérisoire.

**④ — le focus : 11 déplacements sur 14, et les échecs ne s'expliquent pas
tous.** Les sept exécutions ont toutes joué leur phase 2, soit **14**
déplacements. Le verdict porte sur la taille **annoncée par l'agent**
(`lien_taille` du message `link`), qui est la grandeur que le brief désigne, et
la règle est « strictement au-dessus de **toutes** les autres éveillées » :

| Exéc. | Budget | 1ᵉʳ déplacement | 2ᵉ déplacement |
| --- | --- | --- | --- |
| A | 12 Mb/s | ✅ 1024×576 vs 852×480 | ✅ 1024×576 vs 852×480 |
| B | 12 Mb/s | ✅ 1024×576 vs {852×480, 640×360} | ✅ idem |
| C | 12 Mb/s | ✅ 1024×576 vs {852×480, 640×360} | ✅ 1024×576 vs 852×480 |
| D | 12 Mb/s | ✅ 1024×576 vs 852×480 | ✅ 1024×576 vs 852×480 |
| **G** | 12 Mb/s | **❌ 852×480 vs 852×480** | **❌ 852×480 vs {852×480, 640×360}** |
| E | 8 Mb/s | ✅ 1024×576 vs 640×360 | ✅ 1024×576 vs 640×360 |
| F | 8 Mb/s | ✅ 1024×576 vs 640×360 | **❌ 640×360 vs 640×360** |

**Soit 11 sur 14 : 8 sur 10 à 12 Mb/s, 3 sur 4 à 8 Mb/s.**

⚠️ **Une rédaction antérieure de ce paragraphe annonçait « 4 déplacements sur 4
à 12 Mb/s », sur un sous-ensemble de 8 des 14, sans énoncer de règle de
sélection** — et l'exécution qui manquait est précisément celle qui échoue. La
règle de sélection est désormais au §3.3 ; ici, **les 14 sont rapportés**.

**Les trois échecs ne sont PAS des échecs du produit : ce sont des échecs du
protocole de mesure, et les journaux le montrent.** Une rédaction antérieure de
ce paragraphe déclarait les deux échecs de G « INEXPLIQUÉS » ; ils s'expliquent,
sur les pièces déjà versées.

**Le produit s'impose lui-même une temporisation de 20 s.** `DELAI_REMONTEE`
(`agent/src/congestion/hysteresis.rs`) exige qu'une cible plus haute **tienne
20 s** avant qu'une remontée de barreau ne soit appliquée. **Le palier de focus
de cette recette dure 25 s** : il doit absorber dans ses 5 s de marge l'annonce
de focus, la redistribution des parts, la montée de l'estimation BWE au-dessus
du `min_bps` du barreau visé, **et** 20 s sans un seul creux qui rearme le
compteur. **La fenêtre d'observation n'excède la temporisation du mécanisme
observé que de 25 %.**

Le relevé de `lien_taille` et `lien_bitrate` fenêtre par fenêtre, échantillon
par échantillon, montre que **la promotion a bien eu lieu dans les trois cas,
après la fin du palier** :

| Exéc. | fenêtre | à la fin du palier de focus | plus tard, même fenêtre, toujours focalisée |
| --- | --- | --- | --- |
| **G** | `w-6` | `18:01:39` — **852×480**, part 2 666 666 | `18:02:42` (**+63 s**) — **1024×576 / 2 552 888** |
| **G** | `w-4` | `18:01:14` — **852×480**, part 2 509 810 | jamais : le focus lui est **retiré 25 s plus tard**, sa promotion est **préemptée** |
| **F** | `w-6` | `17:51:22` — **640×360**, part 1 777 776 | `17:52:28` (**+66 s**) — **852×480 / 1 664 000** |

Et **G tient le critère ④ en phase 1**, où sa focalisée `w-2` a disposé de
**45 s** : elle y est à **1024×576 / 2 666 666** contre 852×480 / 1 333 333 pour
les sept autres. **La promotion de focus fonctionne dans G.** Ce que le
protocole a mesuré n'est pas son absence, mais sa **latence**.

**L'imputation correcte, à écrire à la place d'« inexpliqué »** : *promotion
retardée au-delà de la fenêtre d'observation ; relevée 63 s (G) et 66 s (F) plus
tard sur la même fenêtre, le palier de 25 s n'excédant que de 5 s le
`DELAI_REMONTEE` de 20 s du produit.*

⚠️ **Le comptage 11/14 ne bouge pas** : la mesure a bien échoué à ces trois
instants, et le tableau ci-dessus reste le relevé. **C'est son IMPUTATION qui
change** — le protocole, pas le mécanisme.

⚠️ **Et l'explication arithmétique que je donnais pour F est REMISE À SA
PLACE.** À 8 Mb/s, la part majorée vaut 1 777 776 bps pour un `min_bps` de
**1 769 472** au barreau 1024×576 : **8 304 bps de marge, soit 0,47 %** — c'est
exact, et cela reste un facteur plausible pour expliquer que `w-6` ait fini à
852×480 plutôt qu'à 1024×576. **Mais ce n'est PAS la raison pour laquelle le
critère a été manqué** : la promotion a eu lieu, hors fenêtre. La marge de
0,47 % est donc un **facteur candidat sur le barreau atteint**, pas la cause de
l'échec de mesure. L'autre déplacement de F, et les deux de E, ont d'ailleurs
réussi dans les mêmes 25 s au même budget.

**Conséquence pour la calibration** : `FACTEUR_FOCUS` ne départage toujours pas
les deux budgets — et on sait maintenant que **8/10 contre 3/4 mesure surtout la
durée des paliers**, pas le mécanisme.

⚠️ **Une nuance de mesure, à ne pas taire** : au second déplacement de
l'exécution E, l'agent annonçait bien 1024×576 pour la focalisée alors que le
navigateur décodait encore du 640×360 à l'instant de l'échantillon. **La taille
décodée retarde sur la taille annoncée** ; le verdict porte sur la seconde.

**⑤ — les endormies.** À dix fenêtres ouvertes (`CAPACITE` = 10,
`PLAFOND_EVEIL` = 8), deux s'endorment. Aux **six** exécutions dont la phase 3
est allée à son terme (toutes sauf G), les deux endormies reçoivent
**exactement 256 000 bps** (`PART_DORMANTE_BPS`) et leur trafic vidéo entrant
est relevé à **0 image et 0,000 Mb/s** sur les 30 s du palier — pendant que les
huit éveillées tiennent 42 à 78 i/s. Le bandeau `#status` de leur page dit
« Réseau insuffisant pour le jeu nerveux — …, 0.2 Mb/s ».

⚠️ **Ce ne sont PAS « les deux plus anciennes »**, comme une rédaction
antérieure l'écrivait : les évincées sont **`w-8` et `w-10`** aux exécutions
B à F, soit les **4ᵉ et 5ᵉ ouvertes** (`w-2` et `w-8` à l'exécution A). Le
vivier de D5 arbitre par **récence du dernier signal**, et les trois premières
fenêtres avaient reçu un focus pendant la phase 2 : c'est le protocole de
recette qui détermine lesquelles s'endorment, pas leur ancienneté.

### 3.5 La valeur retenue, et pourquoi le choix est ASSUMÉ et non démontré

**`BUDGET_BPS` reste 12 000 000.** Le brief prévoyait de descendre à 8 000 000
si le taux dépassait 7,99 % ; **il ne le dépasse pas** sur l'exécution jouée
sous la charge d'hôte de référence.

**Deux raisons survivent à la relecture des relevés, et une troisième est
tombée.**

1. **Le seuil est tenu d'un facteur 2** sur l'exécution comparable : 3,94 %
   contre 7,99 % (exécution D). ⚠️ **Une seule exécution.**
2. **12 Mb/s ne coûte rien au cas mono-fenêtre**, là où 8 Mb/s lui retirerait
   un tiers de son débit. **C'est la seule raison qui ne se discute pas** — et
   c'est exactement le prix que le §2.4 avait nommé pour 8 Mb/s.
   ⚠️ **Le cas mono-fenêtre à 12 Mb/s n'a pourtant JAMAIS été mesuré**, ni ici
   ni au §2 (les points existants à N = 1 sont 9,479 Mb/s à 0 % jetées et
   82,257 Mb/s à 53,6 %). Cette raison protège donc un acquis **supposé**.
3. ❌ **« La majoration de focus n'est robuste qu'à 12 Mb/s » — RETIRÉE.**
   Cette raison figurait dans une rédaction antérieure de ce §, dans le rapport
   de tâche et dans le commentaire de `parts.rs` : elle allait entrer dans la
   mémoire longue du dépôt. **Elle est réfutée par une pièce versée ici même**
   (`critere-g-budget12-partiel.log`) : l'exécution G, à 12 Mb/s et avec 51 %
   de marge sur le seuil de barreau, rate ses **deux** déplacements de focus.
   Le relevé complet est **8/10 à 12 Mb/s contre 3/4 à 8 Mb/s** (§3.4 ④) —
   **`FACTEUR_FOCUS` ne départage pas les deux valeurs.**

**Ce que la comparaison des deux budgets donne réellement est un ARBITRAGE que
la mesure ne tranche pas :**

| Budget | barreau des 7 non focalisées | % jetées | MP/s décodés | focus réussi |
| --- | --- | --- | --- | --- |
| **12 Mb/s** | 852×480 | **3,94 %** (1 exéc. comparable) | **196,40** | 8/10 |
| **8 Mb/s** | 640×360 | **1,46 %** et **3,94 %** (2 exéc.) | 95,54 et 135,57 | 3/4 |

**Plus de pixels livrés, davantage jetés.** Aucune des deux valeurs ne domine
l'autre sur les deux grandeurs, et les effectifs (1 exécution contre 2) ne
permettent aucune comparaison statistique. **Le choix de 12 Mb/s repose donc
sur la raison n°2 — le cas mono-fenêtre — et sur elle seule.** Il est assumé,
pas démontré, et il se réviserait sans embarras si le cas mono-fenêtre était
mesuré et se révélait déjà mauvais à 12 Mb/s.

⚠️ **Trois réserves qui doivent voyager avec cette valeur :**

- **Une seule exécution comparable la porte.** Aucun taux, aucune variabilité.
- **Une exécution sur cinq à 12 Mb/s passe le seuil**, une sur deux parmi
  celles dont l'échelle s'est posée. **La marge est une marge de laboratoire**,
  et **le produit n'est pas démontré robuste** sous la charge d'hôte
  réellement rencontrée pendant la campagne.
- **Le repli à 8 Mb/s n'a JAMAIS été éprouvé sous charge élevée.** Que 12 Mb/s
  s'y dégrade plus vite est **plausible et non établi** : il n'existe aucune
  exécution à 8 Mb/s sous `loadavg` supérieur à 18,1.

### 3.6 Le sondage : ce que cette recette n'a PAS pu établir, et pourquoi

`rtc.bwe().set_desired_bitrate` est l'appel que la conception désigne comme le
plus important — celui qui empêche N fenêtres de sonder chacune le lien entier.
Il **n'a aucun test unitaire**, et il a été établi qu'il ne peut pas en avoir
un honnête : str0m n'expose aucun getter, son `Debug` est un stub, et
`configure_pacer` ne dépend pas de cette valeur. Sa couverture avait été
reportée sur cette recette.

**Elle n'y est pas.** Trois pistes ont été suivies, les trois échouent :

1. **Le trafic sortant cumulé.** Il reste sous le budget (8,031 Mb/s pour
   12 000 000 ; 4,931 et 4,433 pour 8 000 000). Mais `Controleur::changer_plafond`
   borne à lui seul ce que l'encodeur produit : **l'observation est entièrement
   expliquée sans invoquer `set_desired_bitrate`.**
2. **Le comportement de montée.** `packetsLost` vaut **0** aux sept exécutions,
   et le lien porte ≥ 1,45 Gb/s (§1) : il n'y a aucune congestion de lien dont
   une montée bornée se distinguerait d'une montée libre.
3. **L'estimation BWE brute** (`agent::transport::evenements=debug`). Tentée à
   l'exécution B ; le journal passe de 1 128 à 2 317 lignes et l'exécution rend
   59,32 % d'images jetées. ⚠️ **Cette dégradation n'est attribuée ni au niveau
   de trace ni à la charge de l'hôte** : les deux ont changé ensemble. La piste
   a été abandonnée plutôt que de risquer que la mesure détruise ce qu'elle
   mesure — le mode de défaillance déjà payé au chantier TURN.

**Le relevé qui explique l'échec, et qui a sa valeur propre** : une fenêtre
endormie a `set_desired_bitrate` à 256 000 bps et n'encode plus rien (D5 a
relâché son encodeur). Son trafic vidéo entrant est relevé à **0,000 Mb/s sur
30 s, aux six exécutions dont la phase 3 aboutit**. **str0m n'émet donc aucun bourrage de sondage
mesurable quand aucun média ne part** — ce qui répond au passage à l'hypothèse
explicitement déclarée non vérifiée dans la doc de `PART_DORMANTE_BPS` : sur ce
montage, le plancher « ne coûte que sa ligne ».

⚠️ **Portée exacte** : cela ne dit rien du bourrage émis quand un média *actif*
sonde à la hausse. Mais cela ferme la voie du témoin par le trafic : sur ce
montage, **« sondage borné à la part » et « pas de sondage du tout » se lisent
identiquement.** `set_desired_bitrate` reste donc **non couvert**, ni par un
test, ni par cette recette. C'est le point ouvert le plus important de D6.

⚠️ **« Aucun témoin trouvé » n'est PAS « aucun témoin n'était possible », et la
voie la plus simple n'a pas été essayée.** Trois remèdes se présentent, et le
troisième est celui que ce dépôt a payé cher pour apprendre au chantier des
duplications parallèles — *retirer la variable suspecte et voir si le symptôme
survit* :

1. un montage où le sondage produit du trafic observable — média actif, plafond
   très haut, lien réellement contraint ;
2. instrumenter str0m pour exposer la valeur ;
3. **un A/B différentiel sur CE montage** : une exécution avec l'appel
   `rtc.bwe().set_desired_bitrate` neutralisé, opposée à une exécution
   nominale, sur le trafic sortant cumulé des huit fenêtres. **Cette voie n'a
   pas été jouée**, et rien dans les relevés ne dit qu'elle aurait échoué.

### 3.7 Ce que le §3 n'établit PAS

- **Aucun taux, nulle part.** Sept exécutions, dont **trois seulement**
  comparables entre elles, et **une seule** à 12 Mb/s dans ces conditions.
- **Le critère ① n'est pas reproduit** : 1 exécution sur 5 à 12 Mb/s le passe,
  1 sur 2 parmi celles dont l'échelle s'est posée. **La cause de la dégradation
  n'est pas isolée** — charge d'hôte et non-établissement de l'échelle
  covarient sans être départagées (§3.3).
- **L'arbitrage 12 Mb/s contre 8 Mb/s n'est pas tranché par la mesure** : ni
  le taux d'images jetées, ni le débit de pixels, ni la réussite de la
  majoration de focus ne désignent un gagnant (§3.5). Le choix tient à une
  raison — le cas mono-fenêtre — **elle-même jamais mesurée à 12 Mb/s**.
- **Les trois échecs de promotion de focus sont imputés au PROTOCOLE**, sur les
  pièces versées (promotion relevée 63 et 66 s après la fin du palier, une
  quatrième préemptée par le déplacement suivant) — **mais aucune exécution n'a
  été rejouée avec un palier plus long**. Que 45 à 60 s suffiraient est
  **plausible et non vérifié**. La latence de promotion elle-même n'est pas
  mesurée : elle est **bornée** par deux échantillons distants de 63 et 66 s,
  faute d'une trace datée et attribuable — corrigée depuis (`99e5641`) et
  inutilisable rétrospectivement.
- **La valeur est DE LABORATOIRE**, exactement comme celle du §2 : navigateur
  Chrome sans interface, `--disable-gpu`, donc **décodage logiciel**, sur l'hôte
  qui porte aussi la VM et une charge étrangère variable d'un facteur 3,6.
  **Un client réel, sur une autre machine, avec décodage matériel, décrocherait
  ailleurs.** `BUDGET_BPS` n'est pas une constante du produit.
- **`set_desired_bitrate` n'est pas couvert** (§3.6), et **l'A/B différentiel
  qui aurait pu le couvrir n'a pas été joué**.
- **Les régimes 2 et 3 de `repartir` ne sont pas exercés** : seul le régime 1,
  celui qui garantit le non-dépassement, a été rencontré.
- **`HYSTERESIS`, `REPIT_APRES_ECHEC`, `FACTEUR_FOCUS` et `PART_DORMANTE_BPS`
  restent NON CALIBRÉES.** Cette recette montre que `FACTEUR_FOCUS` fait
  franchir un barreau **11 fois sur 14** ; elle ne le calibre pas, et elle
  **ne permet pas de dire qu'il est plus robuste à un budget qu'à l'autre**.
- **La visibilité et le focus sont IMPOSÉS par le pilote**, page par page,
  parce qu'un Chrome sans interface rapporte `document.hidden = true` pour toute
  fenêtre d'arrière-plan. **Aucune minimisation de vraie fenêtre, aucun clic
  réel n'a été joué** — limite héritée de D5, et la plus lourde de ce montage.
- **Rien de la latence de bout en bout**, rien de la durée (exécution la plus
  longue : 8 min 47 s ; palier le plus long : 40 s), une seule application, une
  seule animation, aucun clavier, aucune souris, aucun audio, aucun
  redimensionnement, aucun recouvrement, aucun déplacement de fenêtre.
- **La mort d'un enfant pendant que les autres diffusent** et **le chemin
  d'extinction propre du superviseur** ne sont toujours pas exercés.
- **Rien au-delà de dix fenêtres** : `CAPACITE` vaut 10 et la montée s'y arrête,
  donc sur le produit et non sur un plafond du système.
- **Les trois couches inconnues le restent** : celle du plafond de 8 encodeurs,
  celle du plafond de 4 processus, et le mécanisme de l'abandon du mutex DXGI.

### 3.8 Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Un contrôle anti-piège qui se déclenche trop tôt ne contrôle rien.** Le
  contrôle « la variable est-elle arrivée ? », exigé par le brief et écrit avec
  soin, a rendu `[]` aux sept exécutions : la trace vient d'un `OnceLock`
  initialisé à la première fenêtre, et le contrôle courait six secondes après le
  lancement du superviseur. **Il aurait masqué une variable réellement
  manquante.** Vérifier qu'un contrôle peut échouer avant de s'y fier.
- ⚠️ **Un `grep` direct sur `agent.log` rend zéro pour une ligne qui s'y
  trouve** : `tracing` intercale des séquences ANSI entre le message et ses
  champs, donc `grep "message champ=valeur"` ne matche jamais. `sed
  's/\x1b\[[0-9;]*m//g'` d'abord — le pilote le fait, la ligne de commande non.
- ⚠️ **La déduplication d'annonce de `visibilite.ts` peut faire disparaître le
  focus.** Une page tout juste ouverte annonce `focalisee=true` si `main.ts`
  s'attache avant l'amorce du pilote ; le `blur` qu'on lui envoie ensuite vide
  le champ côté capteur, et la page qu'on **veut** focalisée, dont l'état n'a
  pas changé, ne réémet rien. Résultat mesuré à l'exécution A : plus aucune
  fenêtre focalisée, parts toutes égales à `reste/8` au lieu de `reste/9`.
  **Faire passer la cible par `blur` puis `focus`.**
- ⚠️ **Mesurer un palier avant que l'échelle ne se pose mélange deux régimes.**
  L'exécution A y a perdu sa comparabilité. **Attendre le fait** — douze
  secondes sans changement de barreau — et non une durée.
- ⚠️ **Sur un hôte qui porte d'autres charges, relever la charge à chaque
  palier ne suffit pas : il faut la relever NOMMÉMENT et refuser de comparer
  au-delà d'un écart.** Ici la charge de l'hôte a varié d'un facteur **3,6**
  (`tdarr-ffmpeg` de 97 % à 351 % de CPU en une heure) et les taux d'images
  jetées d'un facteur **19** — **sans que les deux soient appariés**. Les deux
  pires exécutions ont `tdarr` à **104** et **98**, et le seul rang à **351 %**
  rend 16,70 % : **le verdict ne suit pas `tdarr`.** *(Une rédaction antérieure
  de ce piège écrivait « et le verdict du critère ① bascule avec lui » — c'était
  exactement l'attribution que le §3.0 ③ venait de déclarer inférée et non
  établie, réintroduite dans la section destinée à être recopiée.)*
- **Une part majorée qui atterrit à 0,47 % d'un seuil de barreau est un tirage
  au sort, pas une majoration.** Vérifier la marge d'une constante contre
  l'échelle **avant** de croire qu'elle produit l'effet voulu. ⚠️ **Mais une
  marge insuffisante n'était PAS la cause des échecs de focus ici** — c'était la
  durée du palier (piège ci-dessus) : l'exécution G rate ses deux promotions
  avec 51 % de marge, et les obtient plus tard.
- ⚠️ **UN PALIER DE MESURE DOIT ÊTRE PLUSIEURS FOIS PLUS LONG QUE LA
  TEMPORISATION DU MÉCANISME QU'IL OBSERVE.** Celui du focus valait **25 s**
  pour un `DELAI_REMONTEE` de **20 s** : 25 % de marge, dans laquelle devaient
  encore tenir l'annonce de focus, la redistribution des parts et la montée de
  l'estimation. Trois promotions sur quatorze sont arrivées **après** la fin du
  palier (relevées 63 et 66 s plus tard), et une quatrième a été **préemptée**
  par le déplacement de focus suivant. **Le critère en devenait aléatoire, et
  l'échec s'imputait au produit.** Le remède est gratuit — 45 à 60 s. **Lire les
  constantes de temporisation du code AVANT de dimensionner un palier.**
- ⚠️ **UN SOUS-ENSEMBLE SANS RÈGLE DE SÉLECTION EST UN SOUS-ENSEMBLE CHOISI**,
  même quand on ne l'a pas choisi. Le premier énoncé du critère ④ annonçait
  « 4 déplacements sur 4 » sur 8 des **14** relevés, en écartant sans le dire
  les exécutions jugées non comparables **sur un autre critère** — celui de la
  charge d'hôte, qui ne dit rien de la promotion de focus. L'exécution ainsi
  écartée était précisément celle qui échoue, et la conclusion fausse qui en
  sortait — « `FACTEUR_FOCUS` n'est robuste qu'à 12 Mb/s » — partait vers la
  mémoire longue du dépôt. **Énoncer la règle de sélection AVANT de compter, et
  vérifier qu'elle est pertinente pour la grandeur qu'on juge.**
- ⚠️ **« Meilleur » n'a de sens que sur une grandeur à la fois.** Le barreau
  852×480 jette moitié moins d'images que 1024×576 et livre un quart de pixels
  en moins : c'est un **arbitrage**, pas une domination, et l'écrire comme une
  domination contredisait l'argument voisin, qui traitait les pixels décodés
  comme « plus haut = mieux ».
- ⚠️ **« Seule X change » se vérifie contre son propre tableau.** L'affirmation
  « entre D et C seule la charge de l'hôte change » était réfutée par la ligne
  imprimée trois paragraphes plus haut (l'échelle de C ne s'est jamais posée),
  et par le fait que la charge étrangère **nommée** était identique (98 % dans
  les deux cas). La règle « deux variables, aucune attribution », correctement
  appliquée à une autre exécution du même tableau, ne l'avait pas été à
  celle-ci.

### 3.9 Contrôle de survie de la VM et état final

`virsh domstate Windows` : « en cours d'exécution » ; accès réel au partage
contrôlé **après chaque phase** — trois contrôles par exécution, **sauf G qui
n'en a que deux**, son interruption l'ayant privée de sa phase 3. Lignes
`SURVIE VM`, `acces_partage=OUI` partout. Le journal libvirt ne porte aucune
extinction après `2026-08-03 11:17:44+0000`, soit **5 h 49 avant** le début de
la première exécution.

**Purge finale, avec sa pièce** : `purge terminée retirees=0 avant=1 apres=1`,
une seule sortie attachée, la physique (`purge-finale-t10.log`). Zéro processus
`agent` survivant, relevé depuis un processus neuf.

⚠️ **Les purges INTERMÉDIAIRES sont affirmées sans pièce.** Les agents ont bien
été tués et les sorties virtuelles purgées avant chaque exécution — la sortie de
chaque `MULTIFENETRE_VDD_PURGE=1` a été lue au terminal, dont un
`retirees=10 avant=11 apres=1` qui a rattrapé les sorties laissées par
l'exécution B —, **mais aucun pilote ne journalise ces purges et rien n'en a été
versé**. Le §2 avait versé son équivalent
(`decrochage-b-enchainement.log`) ; ce §3 ne l'a pas fait. **C'est une
affirmation sans trace, et elle est écrite comme telle.**
