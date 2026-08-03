# Sous-bloc D6 — partage de la capacité réseau entre N flux : résultats

> Ce document se remplit tâche par tâche. Au 3 août 2026 il ne porte que le §1.

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
