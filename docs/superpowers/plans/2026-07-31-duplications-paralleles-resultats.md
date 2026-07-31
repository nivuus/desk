# N duplications DXGI de front sur N sorties virtuelles — résultats

> Troisième chantier de **mesure** de la série, après la sonde de capture
> multi-fenêtres (`2026-07-30-sonde-capture-multifenetre-resultats.md`) et les
> mesures préalables (`2026-07-31-mesures-prealables-chantier-d-resultats.md`).
> Conception : `../specs/2026-07-31-duplications-paralleles-design.md`.
> Journaux : `journaux-duplications-paralleles/`.

**Chaque chiffre de ce document cite le journal et la ligne d'où il vient.** Les
numéros de ligne sont ceux des fichiers versés, lus après suppression des
séquences ANSI (qui ne changent pas la numérotation).

---

## 1. Verdict de réception

Le critère posé d'avance par la conception (§4) est :

> **Reçue** si, à N=8 : chaque fenêtre tient **≥ 60 i/s en capture+encodage**
> **et** aucun verdict faux sur aucune voie.

### **REÇUE.**

À N=8, huit duplications DXGI ouvertes **de front** sur huit sorties virtuelles
distinctes rendent **90,1 i/s par fenêtre en capture+encodage** —
`paralleles-n8.log:146`, `cadences=[90.1, 90.1, 90.1, 90.1, 90.1, 90.1, 90.1,
90.1]` — soit **1,50 fois** le seuil de 60, avec **`verdicts_faux=0`** sur la
même ligne et `voisines: 0, noires: 0, inconnues: 0`.

Le critère est tenu au rang qui compte, et il l'est aussi aux trois autres.

**Ce que ce verdict porte, et rien de plus** : l'arrangement que la voie
recommandée du chantier D propose réellement — une sortie virtuelle par fenêtre,
une duplication par sortie, un encodeur par sortie — tient à 8 fenêtres de
1280×720 sur cette machine, sur une exécution par rang. Il ne dit rien du
plafond, de la latence, ni d'applications réelles : §6.

---

## 2. Ce que ce montage a de neuf

Tous les bancs antérieurs de ce projet ont mesuré **à aire totale fixe**.
`disposition::tuiles` découpe le bureau : la surface par fenêtre décroît quand N
croît, le débit de pixels reste donc quasi constant *par construction*, et le
nombre de fenêtres n'y est **pas prouvé neutre en soi**. C'est la réserve que la
sonde avait explicitement laissée ouverte.

Ici, chaque fenêtre a **sa** sortie virtuelle de 1280×720 :
`largeur_annoncee=1280 hauteur_annoncee=720` sur les huit lignes
`sortie virtuelle retenue` de `paralleles-n8.log:34-41`, et les `places_texture`
valent 1280×720 aux quatre rangs — **facteur d'échelle 1**, sans le piège DPI de
1,5 relevé par la sonde. **L'aire totale croît donc linéairement avec N**, de
0,92 Mpx à N=1 à **7,37 Mpx à N=8**.

C'est la première fois que ce projet fait varier N sans faire varier la surface
par fenêtre.

---

## 3. Les quatre rangs

### 3.1 Passe capture

| N | ligne | cadences par fenêtre (i/s) | justes | voisines | noires | inconnues | `verdicts_faux` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `paralleles-n1.log:33` | `[90.0]` | 900 | 0 | 0 | 0 | 0 |
| 2 | `paralleles-n2.log:41` | `[90.1, 90.0]` | 901 | 0 | 0 | 0 | 0 |
| 4 | `paralleles-n4.log:57` | `[90.1, 90.1, 90.1, 90.0]` | 900 | 0 | 0 | 0 | 0 |
| 8 | `paralleles-n8.log:89` | `[90.1 ×7, 90.0]` | 901 | 0 | 0 | 0 | 0 |

### 3.2 Passe capture + encodage

| N | ligne | cadences par fenêtre (i/s) | justes | voisines / noires / inconnues | `verdicts_faux` | `unites` (par encodeur) |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `paralleles-n1.log:55` | `[90.1]` | 901 | 0 / 0 / 0 | 0 | `[450]` |
| 2 | `paralleles-n2.log:68` | `[90.1, 90.1]` | 901 | 0 / 0 / 0 | 0 | `[450, 450]` |
| 4 | `paralleles-n4.log:94` | `[90.1] ×4` | 901 | 0 / 0 / 0 | 0 | `[450] ×4` |
| 8 | `paralleles-n8.log:146` | `[90.1] ×8` | 901 | 0 / 0 / 0 | 0 | `[450] ×8` |

**En capture+encodage, la cadence vaut exactement `90.1` sur les quinze voies des
quatre rangs.** En capture nue elle vaut 90,0 ou 90,1 selon la voie. L'écart
entre les deux passes est d'une image sur ~900.

À N=8, ce sont **3 600 unités H.264** sur la passe (8 encodeurs × 450), et
**7 208 images capturées** (8 × 901).

### 3.3 Aire totale et débit de pixels

| N | aire totale | débit de pixels, **calculé** à 90,1 i/s |
| --- | --- | --- |
| 1 | 0,92 Mpx | 83,0 MP/s |
| 2 | 1,84 Mpx | 166,1 MP/s |
| 4 | 3,69 Mpx | 332,1 MP/s |
| 8 | **7,37 Mpx** | **664,3 MP/s** |

> ⚠️ **Ces débits sont calculés, non relevés.** Le banc journalise des cadences
> et des comptes d'images, jamais des pixels. Le calcul est
> `N × 1280 × 720 × 90,1`. Les cadences, elles, sont relevées (§3.1, §3.2).

### 3.4 Recollage à la série de la sonde — deux séries qui ne mesurent pas la même chose

| N | sonde, voie `duplication` (aire **fixe**) | ce chantier (aire **croissante**) |
| --- | --- | --- |
| 1 | 80,2 i/s — 208 MP/s | 90,1 i/s — 83,0 MP/s |
| 2 | 99,4 i/s — 258 MP/s | 90,1 i/s — 166,1 MP/s |
| 4 | 98,8 i/s — 256 MP/s | 90,1 i/s — 332,1 MP/s |
| 8 | 107,5 i/s — 248 MP/s | 90,1 i/s — **664,3 MP/s** |

*(Colonne de gauche : `2026-07-30-sonde-capture-multifenetre-resultats.md`, §3
de son sommaire. Ses chiffres sont ceux de sa passe **capture** ; sa passe
d'encodage était coupée à N>1 par la porte de correction, par construction du
protocole.)*

**Les deux colonnes ne se comparent pas terme à terme, et il faut le dire
explicitement** :

- la série de la sonde partage **une** acquisition entre N recadrages d'un
  bureau d'aire fixe : son débit de pixels est quasi constant *par
  construction*, et sa cadence par fenêtre monte quand N croît parce que la
  surface de chaque fenêtre baisse ;
- la série de ce chantier ouvre **N acquisitions distinctes** sur N sorties de
  surface constante : son débit de pixels croît en N par construction, et c'est
  sa **cadence** qui est le fait mesuré.

**Aucun rapprochement chiffré n'est donc tiré de ce tableau — ni sur les
cadences, ni sur les débits.** Un débit de pixels est le produit d'une cadence
par une aire : si les deux séries ne sont pas commensurables sur les cadences,
elles ne le sont pas davantage sur un nombre qui en dérive. Le tableau est là
pour **situer les deux montages l'un par rapport à l'autre**, pas pour les
comparer.

*(Rédaction antérieure retirée : elle annonçait « le débit passe de 248–258 MP/s
à 664,3 MP/s sans que la cadence bouge » comme un fait de ce montage. C'était
faux à trois titres — 248–258 MP/s vient de la **sonde**, la série de ce chantier
allant de 83,0 à 664,3 ; la cadence, d'une série à l'autre, **bouge** de 107,5 à
90,1 i/s ; et la borne basse de la sonde, 208 MP/s à N=1, était écartée sans le
dire. C'est exactement la comparaison inter-séries que les deux puces ci-dessus
interdisent.)*

Le fait de ce chantier, lui, se lit **entièrement dans sa propre colonne** et
n'a besoin d'aucune autre série : **l'aire totale est multipliée par 8 de N=1 à
N=8 — 0,92 → 7,37 Mpx, relevé — et la cadence par fenêtre ne bouge pas** (90,1
i/s aux quatre rangs en capture+encodage, relevé). Le débit de pixels
correspondant, **calculé**, passe de 83,0 à 664,3 MP/s. Cela n'établit pas où la
voie s'arrête (§6).

### 3.5 Passe témoin — la source ne décroche pas

Sans elle, un décrochage de la capture à N=8 serait indiscernable d'un
décrochage des mires elles-mêmes.

| N | ligne | `trames` (tours de boucle) | cadence journalisée (tours/s) | tours × N = mires peintes/s |
| --- | --- | --- | --- | --- |
| 1 | `paralleles-n1.log:14` | 126 409 | 12 640,9 | 12 640,9 |
| 2 | `paralleles-n2.log:18` | 66 653 | 6 665,3 | 13 330,5 |
| 4 | `paralleles-n4.log:26` | 33 255 | 3 325,4 | 13 301,6 |
| 8 | `paralleles-n8.log:42` | 17 059 | 1 705,8 | 13 646,6 |

`trames` compte les **tours**, et chaque tour peint les N mires : la cadence
journalisée décroît en 1/N par construction. Le produit tours × N — le nombre de
mires réellement peintes par seconde — reste dans une bande de **12 641 à
13 647** sur les quatre rangs. La peinture ne décroche donc pas quand les mires
vivent sur N sorties distinctes.

---

## 4. Ce que les journaux établissent en plus des cadences

**Huit duplications ouvertes de front, journalisées nommément.**
`paralleles-n8.log:78` : `les N duplications sont ouvertes de front nombre=8`,
après les huit ouvertures individuelles (l. 49/53/57/61/65/69/73/77). Aucune
ligne `duplication REFUSÉE` dans aucun des quatre journaux.

**Huit sorties virtuelles distinctes, désignées par leurs noms.**
`\\.\DISPLAY5` à `\\.\DISPLAY12`, x = 2400 à 11360 par pas de 1280, toutes
`attachee=true` — `paralleles-n8.log:34-41`.

**Huit encodeurs H.264 matériels construits sans refus.** Huit lignes
`encodeur matériel retenu encodeur=NVIDIA H.264 Encoder MFT`,
`paralleles-n8.log:93/96/99/102/105/108/111/114`, entre 19:38:04.956932 et
19:38:05.343789. Aucun `MF_E_UNSUPPORTED_D3D_TYPE`, aucune ligne `ERROR` ni
`WARN` dans aucun des quatre journaux.

**Huit périphériques D3D11 distincts — inférence adossée au journal, non lecture
du seul code.** C'est bien une **inférence**, et elle est déroulée ci-dessous
pour pouvoir être contestée : aucun relevé ne montre huit pointeurs de
périphérique distincts. Ce qui est relevé, ce sont huit
lignes `protection multi-fils activée sur le contexte immédiat D3D11`,
`paralleles-n8.log:47/51/55/59/63/67/71/75`, **toutes avec
`protection_precedente=false`**. Ce champ est la valeur *retournée* par
`SetMultithreadProtected(true)` (`agent/src/capture.rs:138-141`), donc l'état
**antérieur** du contexte : un même contexte sollicité huit fois rendrait `true`
aux sept appels suivants. Huit `false` consécutifs disent huit contextes
immédiats jamais protégés auparavant.

**Le chien de garde du pilote a été pingué sans trou.**
`intervalle_ping_max_ms` = 1000 aux rangs 1, 2 et 4 (`n1:61`, `n2:74`, `n4:100`)
et **1002** à N=8 (`n8:152`). Deux millisecondes au-dessus de la cadence visée
d'un battement par seconde. Les quatre rangs portent leurs **4 lignes sur 4** de
contrôle de survie « les N sorties virtuelles sont encore là ET attachées »
(témoin, capture, capture+encodage, bilan) : aucune ligne `DISPARU`, aucune
`DÉTACHÉES`.

**La topologie est rendue intacte, par les NOMS.** Ensemble de départ relevé
depuis un processus neuf : `{ \\.\DISPLAY1 }`
(`paralleles-controle-depart-dxgi.log:4-6`). Ensemble final, relevé depuis un
processus qui n'a rien créé : `{ \\.\DISPLAY1 }`
(`paralleles-controle-final-dxgi.log:4-6`) — **identique nom pour nom**. Les
quatre journaux se terminent par `état initial restauré — mêmes sorties,
nommément noms=["\\\\.\\DISPLAY1"]` (`n1:70`, `n2:84`, `n4:112`, `n8:168`), et
les cinq purges (départ, avant chaque rang, finale) rendent toutes
`retirees=0 avant=1 apres=1`. **Aucun rang n'a démarré sur un état sale ni
laissé de sortie derrière lui.**

**Aucun rang n'a été rejoué.** Chaque rang est une exécution unique, prise du
premier coup. Aucun des **quatre** incidents prévus par le protocole — refus de
`DuplicateOutput`, verdict `Voisine`, gel à la libération, plantage — ne s'est
produit.

---

## 5. La clé de lecture des `unites` — un constat, pas une cause

Aux quatre rangs, chaque encodeur rend **450 unités H.264 pour 901 images
soumises**. Ce rapport se lit dans les lignes périodiques, et il est **exactement
d'un demi et parfaitement linéaire** — `paralleles-n1.log:40-49`, une ligne par
seconde :

| images | 91 | 181 | 271 | 361 | 451 | 541 | 631 | 721 | 811 | 901 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `unites` | 45 | 90 | 135 | 180 | 225 | 270 | 315 | 360 | 405 | 450 |

Soit **+90 images et +45 unités par seconde**, sans dérive sur les dix
intervalles. Le même profil se lit à N=2, 4 et 8 (`n2:60`, `n4:82`, `n8:126`,
identiques voie par voie).

**La fermeture arithmétique est complète** et vérifiée aux quatre rangs :

> `images = unites + nv12_ecartees + au plus 1 en vol`

- N=1 : 901 = 450 + 450 + 1 (`n1:50`, `nv12_ecartees=[450]`)
- N=2 : 901 = 450 + 450 + 1 (`n2:61`)
- N=4 : 901 = 450 + 450 + 1 (`n4:83`)
- N=8 : 901 = 450 + 450 + 1 (`n8:127`)

Aucune image ne disparaît sans être comptée : la moitié des images converties
part dans `dropped_stale_nv12` — une image convertie puis écartée parce qu'une
plus récente est arrivée avant que l'encodeur ne la réclame.

> ⚠️ **Correction d'une attribution fausse.** Le commentaire de
> `agent/src/diagnostics/multifenetre/paralleles/passes.rs` imputait ce rapport
> à l'écart entre les 60 i/s de configuration des encodeurs et les ~90 i/s
> soumis. **Cette attribution ne tient pas devant le relevé** : un rapport 90/60
> prédirait 600 unités pour 900 images, soit 60 unités par seconde ; le journal
> en montre **45**, c'est-à-dire exactement la moitié des images, sur les dix
> intervalles. Le commentaire a été réécrit en constat. **La cause de ce rapport
> d'un demi n'est pas établie** — et surtout, elle n'a pas besoin de l'être ici :
> ce qui compte pour cette mesure est que le rapport soit **identique aux quatre
> rangs**, donc indépendant du parallélisme, et que la fermeture arithmétique ne
> laisse aucune image inexpliquée.

Corollaire à ne pas oublier au chantier D : **les 90,1 i/s sont une cadence de
capture, pas une cadence d'unités H.264 délivrées.** Ce montage délivre 45
unités par seconde et par fenêtre. Rien ici ne dit ce que rendrait un encodeur
configuré pour la cadence à laquelle on le nourrit.

---

## 6. Ce que cette mesure n'établit PAS

La conception (§4) en fixait la liste d'avance ; l'exécution en a ajouté.

**Prévu d'avance par la conception :**

- **Rien au-delà de 8 sorties, et rien entre 4 et 8.** Le plafond de
  duplications DXGI simultanées n'est **pas mesuré** : 8 est ce qui a été
  demandé et obtenu, **pas une limite trouvée**. Le vivier de sorties virtuelles
  étant de 10, un rang 9 ou 10 serait mesurable ; il ne l'a pas été.
- **Rien d'autres résolutions ni d'autres débits.** 1280×720 à 60 Hz, 8 Mb/s,
  passes de 10 s, et rien d'autre. Aucune dérive thermique ou de mémoire ne
  pourrait apparaître en 10 s.
- **Rien de la latence.** Seule la cadence est relevée ; le pipeline de bout en
  bout n'est pas exercé.
- **Rien du comportement quand un tiers consomme le même vivier.** Apollo n'a
  consommé aucune sortie pendant ces mesures. Le vivier de 10 est peut-être
  partagé ; rien ici ne le dit.
- **Rien de la couche qui imposerait un plafond**, s'il y en a un.
- **Rien de la mise en sommeil des fenêtres masquées.** Que détruire un encodeur
  libère la place reste une **conjecture non éprouvée** : la séquence « créer 8
  → en détruire 1 → tenter un 9ᵉ » n'a pas été jouée.

**Ajouté par l'exécution :**

- **Une exécution par rang, donc aucun taux.** Chaque chiffre du §3 vient d'une
  exécution unique. Rien ne dit à quelle fréquence un rang échouerait, ni ne
  borne la variabilité des cadences. En particulier, le gel de
  `IMFShutdown::Shutdown` vu « une fois sur six à N=4 » au chantier de correctif
  (§7) n'est **ni observé ni exclu** par une exécution unique à N=4.
- **La justesse est échantillonnée, pas exhaustive.** Le contrôle tourne d'une
  voie par tour. À N=8, chaque voie est contrôlée un tour sur huit, soit
  **~113 lectures sur ses 901 images**. « Zéro verdict faux » signifie « aucun
  faux sur les 901 lectures effectuées », **pas** « aucune des 7 208 images
  capturées n'était fausse ». (Ce choix est délibéré et documenté : contrôler
  les N voies à chaque tour ferait croître le coût du contrôle avec N et rendrait
  les rangs non comparables — l'erreur déjà payée au chantier précédent.)
- **Les débits de pixels sont calculés, pas relevés** (§3.3).
- **Les mires ne sont pas des applications.** Chaque sortie porte une mire peinte
  par chaîne d'échange D3D11, **plein cadre**, sans occlusion, sans interaction,
  sans redimensionnement. Une vraie fenêtre applicative ne produit pas la même
  charge de composition — et le chantier D en posera de vraies.
- **Aucune unité H.264 n'a été décodée ni regardée.** Les `unites` sont des
  unités comptées en sortie de l'encodeur ; leur contenu n'est pas contrôlé. Les
  verdicts, eux, portent sur les images **capturées**, avant encodage.
- **La cause de la constance à 90,1 i/s n'est pas établie.** Que la cadence ne
  bouge pas de N=1 à N=8 est un fait relevé ; **ce qui la borne à ~90** n'est pas
  mesuré. Un plafond situé juste au-dessus et un plafond très au-dessus se
  liraient pareil ici.
- **La restauration constatée n'est pas une preuve d'absence d'effet résiduel.**
  Elle porte sur l'ensemble des **noms** de sorties attachées, rien d'autre.
- **La fraîcheur du binaire n'est adossée à aucune pièce versée.** La sortie de
  `scripts/build-agent.sh` a été recopiée du terminal, pas capturée dans un
  fichier. Ce qui reste vérifiable depuis le dépôt : `git status --porcelain
  agent/ scripts/` vide à `db8cc85`, et la date du binaire sur la VM
  (31 juil. 21:13) antérieure de 1 min 40 s à l'horodatage du commit
  (2026-07-31 21:14:40 +0200) — cohérent avec un binaire bâti sur ces sources,
  sans le prouver.
- **Le comportement d'exploitation réel n'est pas couvert** : ouvertures et
  fermetures entrelacées, encodeurs de durées de vie différentes,
  redimensionnements. Le banc crée ses N encodeurs d'un coup et les détruit
  d'affilée à la fin ; il ne détruit **jamais** un encodeur pendant que les
  autres encodent. C'est la différence la plus importante avec le chantier D.

---

## 7. Le défaut ouvert hérité — diagnostiqué, corrigé, et ce qui reste

Les mesures préalables laissaient un défaut **localisé, non diagnostiqué** : sur
la voie `duplication`, la passe d'encodage **tuait le processus à la sortie de sa
boucle** — donc à la libération des encodeurs — et laissait une sortie virtuelle
orpheline. Il bloquait ce chantier de front, et il n'était pas contournable :
dans le chantier D, fermer une fenêtre détruira son encodeur, par le même chemin,
en production.

### 7.1 Le défaut était **intermittent**, pas déterministe

**Correction d'une lecture que plusieurs documents du dépôt portaient encore.**
Ils le décrivaient comme reproductible à coup sûr — le bornage de la sonde
disait « les deux exécutions » sans dire combien avaient passé. La campagne de
diagnostic l'a mesuré : **2 plantages sur 6 exécutions** dans la configuration
comparable (`duplication`, N=1). Et un rapport antérieur avait enchaîné **quatre
exécutions propres** sur un binaire non corrigé.

C'est une correction de méthode, pas de détail : **un défaut intermittent qu'on
croit déterministe se déclare « corrigé » à la première exécution qui passe.**
Toute la campagne de vérification en découle.

### 7.2 La faute, désignée par sa pile

Deux plantages capturés avec leur pile, **identiques cadre pour cadre**. Le bloc
ci-dessous n'est **pas** un extrait verbatim : il **fusionne deux pièces** — les
cadres `module+décalage` viennent des journaux bruts
(`2bis-plantage-{1,2}-pile-exception.log`, qui portent `<hors module>` et **aucun
nom de symbole** pour `#07` et `#08`), les noms de symboles viennent de la
symbolisation hors ligne (`2bis-symbolisation.md`). Les valeurs des deux pièces
sont exactes ; leur juxtaposition est le fait de ce document.

```
#05 ntdll.dll+0x19daa   RtlpEnterCriticalSectionContended+0x1da   <== FAUTE
#06 ntdll.dll+0x174c2   RtlEnterCriticalSection+0x42
#07 nvEncMFTH264x.dll+0x735d
#08 nvEncMFTH264x.dll+0x34ac
#09 RTWorkQ.dll+0xbdca  CSerialWorkQueue::QueueItem::ExecuteWorkItem+0xca
```

Écriture à l'adresse `0x24` (`DebugInfo->ContentionCount++` avec `DebugInfo`
nul), **sur un fil de pool, pas sur le fil principal**. En clair : **la MFT
NVIDIA a encore un élément de travail en vol quand on relâche l'encodeur**, et
cet élément entre dans un verrou qui n'existe plus. Journaux :
`2bis-plantage-{1,2}-pile-exception.log`, méthode dans `2bis-symbolisation.md`.

*Non établi, et à ne pas écrire comme un fait* : **qui** a détruit ce verrou, et
quand. Que la MFT l'ait détruit en se libérant est la lecture cohérente, pas un
relevé.

### 7.3 `MFShutdown` n'était **pas** en cause — réfuté par la mesure

Le diagnostic concluait d'abord à une course avec `MFShutdown`, que le fil
principal exécutait dans les deux vidages. **Cette lecture est fausse, et elle a
été réfutée expérimentalement** : `MFShutdown` retiré **entièrement** du chemin
(Media Foundation démarré une fois pour le processus, jamais arrêté), la faute
revient — **1 récidive sur 5 exécutions**, même décalage `0x19daa`, même pile,
cette fois **après** la libération complète de l'encodeur et des voies de
capture, avec `MFShutdown` appelé nulle part. Journaux :
`2ter-recidive-sans-mfshutdown-agent.log` et `…-pile.log`.

Sa présence dans les deux premières piles était **fortuite** : c'est ce que le
fil principal faisait à cet instant, pas ce qui causait quoi que ce soit.

*Ce que cette réfutation n'établit pas* : que `MFShutdown` serait inoffensif dans
toute autre séquence. Elle montre qu'il n'est pas une condition **nécessaire** de
la faute ; deux chemins menant au même symptôme peuvent coexister.

**Fait acquis, réutilisable au-delà de ce chantier** :
`IMFShutdown::Shutdown` rendant `MFSHUTDOWN_COMPLETED` **ne prouve pas** l'absence
d'élément de travail en vol concernant la MFT — mesuré, la faute revient quand
même (1 récidive sur 10, `2ter-recidive-apres-imfshutdown-pile.log`).

### 7.4 Le correctif retenu

Imposer à la MFT NVIDIA **notre propre file de travail Media Foundation
sérialisée** (`IMFRealTimeClientEx::SetWorkQueueEx`, **une file par encodeur**),
puis, à la destruction, **y déposer une sentinelle et attendre qu'elle soit
invoquée** avant de relâcher quoi que ce soit. Une file sérialisée exécute un
élément à la fois : si la sentinelle passe, tout ce qui a été déposé avant elle a
fini. C'est une **attente bornée sur une condition observable**, pas un délai.

Que le travail de la MFT transite bien par cette file est **éprouvé, pas
supposé** : boucher la file 3 000 ms au milieu d'une passe arrête l'encodage net
pendant exactement la durée du bouchon, tandis que la capture continue
(`2ter-epreuve-file-bouchee.log`).

Code : `agent/src/encode/arret.rs` (nouveau), `agent/src/encode.rs`.

**Résultat de la campagne de vérification** : **0 récidive sur 20 exécutions** du
cas comparable (`duplication`, N=1, binaire recompilé depuis le commit vérifié
`9b2e5e6`), **contre 2 sur 6** avant correctif. Les exécutions à N=4 et sur
d'autres voies exercent autre chose et **ne s'y additionnent pas**.

> **Ce n'est pas une preuve d'absence.** L'énoncé doit toujours porter son
> nombre d'exécutions : *aucune récidive observée sur 20 exécutions du cas
> comparable*. Le taux de référence lui-même vient d'un échantillon de 6 et n'est
> pas une fréquence.

Ce chantier apporte un élément de plus, non couvert par cette campagne : **les
quatre rangs ci-dessus, dont N=8 avec huit encodeurs détruits d'affilée, n'ont
produit aucun plantage ni aucun gel** — à N=8 les huit paires
`libération d'un encodeur : avant`/`après` tiennent en **4,0 ms**
(`paralleles-n8.log:129-144`, 19:38:15.396186 → 19:38:15.400144), et la ligne de
bilan est atteinte (`n8:146`). C'est **une** exécution par rang : cela conforte,
cela ne prouve pas.

### 7.5 Deux risques restent ouverts et assumés

1. **`IMFShutdown::Shutdown` est non borné dans un `Drop`, et un gel y a été
   OBSERVÉ** — une exécution sur six à N=4, trace `Shutdown : avant` écrite,
   `après` jamais, processus encore vivant treize minutes plus tard
   (`2ter-gel-n4-shutdown.log`). **Ce n'est pas un risque théorique.** Sa cause
   n'est **pas attribuée** : cette exécution portait aussi une file imposée au
   convertisseur, retirée depuis, et le départage n'a pas été fait — que le gel
   ait disparu avec cette file ne prouve pas qu'il en venait.

   **Le retirer n'est pas une option** : sans lui, la faute revient **2 fois sur
   5**, barrière pourtant franchie (`2ter-recidive-barriere-seule-agent.log`).
   Arrêt et barrière ne sont donc pas redondants — chacun retiré séparément
   laisse la faute revenir. Le borner exigerait de déporter l'appel sur un autre
   fil, donc de faire traverser une interface COM à une frontière d'appartement
   (le fil principal est dans un STA) : échanger un risque contre un défaut
   certain.

2. **Le convertisseur de couleur n'est couvert par rien.** Il n'expose pas
   `IMFShutdown` (`0x80004002`), et la file qu'on lui avait donnée a été retirée
   sur relevé (voir le gel ci-dessus). Sur les machines éprouvées,
   `create_color_converter` retombe toujours sur `CLSID_VideoProcessorMFT`, une
   MFT **synchrone** — donc sans travail en vol. **Sur un hôte doté d'un Video
   Processor matériel, ce serait une MFT matérielle sans barrière.** Aucune
   machine éprouvée n'expose cette configuration : elle n'est donc **pas
   mesurée**, et le risque est nommé plutôt que couvert.

**Non établi non plus** : que la barrière couvre *tout* le travail de la MFT
(si elle empile sa propre file sérialisée sur la nôtre, notre sentinelle ne
serait ordonnancée que derrière **un** de ses éléments — l'épreuve du bouchon ne
départage pas les deux cas) ; pourquoi arrêt et barrière sont **tous deux**
nécessaires ; et lequel de `MFT_MESSAGE_COMMAND_FLUSH` ou de
`SET_D3D_MANAGER(nul)` provoquait le blocage de 13 minutes qui les a fait sortir
(`2ter-blocage-n4-flush-setd3dmanager.log`).

---

## 8. Pièges neufs — à connaître avant de toucher à ce terrain

- **Ne jamais se fier à la pile du fil principal pour désigner une cause.** Dans
  les deux vidages, elle montrait `MFShutdown` ; c'était une coïncidence de
  minutage. La réfutation a coûté une campagne entière. **Ce qui tranche est de
  retirer la variable suspecte et de voir si le symptôme survit.**
- **`MFSHUTDOWN_COMPLETED` ne veut pas dire « plus rien en vol ».** Une MFT peut
  rendre cet état immédiatement (`attente_ms=0`) et faire quand même planter le
  processus quelques instants plus tard.
- **Un défaut intermittent qu'on croit déterministe se déclare corrigé à la
  première exécution qui passe.** Mesurer le taux **avant** de corriger, et
  opposer au correctif une campagne d'un ordre de grandeur au-dessus.
- **Une absence d'observation n'est pas une preuve d'absence** — l'énoncé porte
  son nombre d'exécutions, toujours.
- **Ne pas totaliser des exécutions qui n'exercent pas la même chose.** Additionner
  N=1, N=4 et d'autres voies produit un nombre sans référent : seule la ligne
  comparable s'oppose à la référence.
- **Ne pas utiliser `git add -A` dans un arbre partagé.** Un `git add -A
  agent/src` a emporté dans un commit le travail concurrent d'une autre tâche
  dans le même arbre, sans sa déclaration de module : le commit ne compilait pas.
  **Nommer les fichiers.**
- **Le débit de pixels d'un banc à aire fixe et celui d'un banc à aire croissante
  ne se comparent pas.** Les deux séries de ce projet existent maintenant ; les
  confondre ferait conclure à un décrochage ou à un gain qui n'est ni l'un ni
  l'autre.
- **Corriger une affirmation réfutée exige de la CHERCHER, pas de la corriger là
  où on nous l'a montrée.** Les documents longs ont un sommaire, et c'est lui
  qu'on lit : traiter le chapitre de détail en laissant le sommaire intact laisse
  le lecteur repartir avec une tâche déjà faite. Un balayage sur les formules
  (« encore due », « non diagnostiqué », « jamais expliqué »…) en a trouvé
  **trois** de plus, dans les deux endroits les plus exposés.
- **Corollaire, payé une ronde plus tard : chercher par le SENS, pas par la
  formule.** Ce balayage-là cherchait « **non** diagnostiqué » ; la phrase qui a
  survécu disait « **pas** diagnostiqué » — conclusion d'une section entière,
  contredisant frontalement l'encadré posé soixante lignes plus haut. Une
  négation se dit de plusieurs façons, et **c'est celle qu'on n'a pas listée qui
  survit**. Balayer sur la *chose niée* (un diagnostic, une mesure, une
  explication), en énumérant les tournures : « pas / non / jamais / seulement
  localisé / sans explication / reste ouvert / n'est établi par rien ».
- **Annoter l'affirmation elle-même, pas sa voisine.** La note qui manquait avait
  été posée sur la puce d'à côté — la même méprise, d'un cran plus fin, que celle
  du point précédent.
- **Une clé de lecture posée dans le code doit être vérifiée contre le journal
  avant d'être recopiée.** Le commentaire de `passes.rs` expliquait le rapport
  `unites`/`images` par un ratio 90/60 qui prédisait 600 unités là où le journal
  en montrait 450 (§5). La fermeture arithmétique était juste ; **l'attribution
  causale ne l'était pas**, et elle était écrite pour être reprise telle quelle
  par ce document.

---

## 9. Journaux versés

Sous `journaux-duplications-paralleles/`, copies **verbatim** de
`/media/vm/dev/agent.log`, non retouchées (séquences ANSI de `tracing`
conservées). **Tous en UTF-8**, accents `grep`-ables tels quels, aucune
conversion nécessaire.

### Les quatre rangs et leurs contrôles

| Fichier | Rôle | Lignes |
| --- | --- | --- |
| `paralleles-n1.log` | rang témoin N=1 | 70 |
| `paralleles-n1-r1.log` | référence N=1 de la tâche 5, pour concordance | 70 |
| `paralleles-n2.log` | rang N=2 | 84 |
| `paralleles-n4.log` | rang N=4 | 112 |
| `paralleles-n8.log` | rang N=8 | 168 |
| `paralleles-controle-depart-dxgi.log` | état de départ, processus neuf | 6 |
| `paralleles-controle-final-dxgi.log` | état final, processus neuf | 6 |
| `paralleles-purge-depart.log` | purge préalable | 10 |
| `paralleles-purge-avant-n{2,4,8}.log` | purges inter-rangs | 10 chacune |
| `paralleles-purge-finale.log` | purge finale | 10 |

**Concordance des deux exécutions N=1** (celle de ce jeu et la référence de la
tâche 5, prises à 15 minutes d'intervalle sur le même binaire) : cadences,
`unites`, `intervalle_ping_max_ms`, lignes de survie et absence d'`ERROR`
concordent. Deux écarts, d'**une image sur ~900** chacun et de même sens : la
passe capture+encodage rend `[90.1]`/901 justes ici contre `[90.0]`/900 à la
référence, et `nv12_ecartees` vaut 450 contre 449 — le tour supplémentaire capté
ici a été converti puis écarté.

### Le défaut hérité

`2bis-*` (diagnostic : piles, symbolisation, autotest de l'instrument),
`2ter-*` (correctif : récidives avec leurs piles, épreuve de la file bouchée, gel
à N=4, blocage `FLUSH`/`SET_D3D_MANAGER`, exécutions représentatives),
`defaut-liberation-*` (instrumentation initiale).

---

## 10. Ce que le chantier D peut désormais tenir pour acquis

- **N sorties virtuelles × 1 duplication DXGI × 1 encodeur tient à N=8**, à
  1280×720 par fenêtre, avec 50 % de marge sur la cible de 60 i/s et sans
  appariement croisé détecté. C'était la dernière mesure que les mesures
  préalables déclaraient due.
- **La cadence n'a pas fléchi à 664,3 MP/s calculés** sur cette machine. Cela ne
  désigne aucun facteur limitant : ce qui borne la cadence à ~90 i/s n'est pas
  mesuré (§6).
- **Le chemin de destruction d'un encodeur n'a plus tué le processus** sur les
  20 exécutions du cas comparable ni sur les quatre rangs de ce chantier — ce
  qui n'est pas une preuve d'absence, sous les réserves du §7.5, et sans que le
  cas d'exploitation réel (fermer une fenêtre pendant que les autres encodent)
  soit couvert.

**Ce qu'il doit encore mesurer ou décider** : le plafond réel de duplications ;
la mise en sommeil des fenêtres masquées, dont le mécanisme repose toujours sur
une conjecture ; le partage du vivier de 10 sorties avec Apollo ; la latence ; et
le comportement sous fenêtres applicatives réelles plutôt que sous mires plein
cadre.
