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
