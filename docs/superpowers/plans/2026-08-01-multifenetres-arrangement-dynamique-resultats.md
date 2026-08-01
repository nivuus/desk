# Sous-bloc D2 — arrangement multi-fenêtres dynamique : résultats

**Date** : 1ᵉʳ août 2026. **Tâche 12** (dernière) du plan
`docs/superpowers/plans/2026-08-01-multifenetres-arrangement-dynamique.md`.
**Branche** : `chantier-multifenetres-d2`, base `f3edc9b`.
**Conception** : `docs/superpowers/specs/2026-08-01-multifenetres-arrangement-dynamique-design.md`.
**Antécédent** : le sous-bloc D1, non reçu
(`plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`).

Pièces versées, toutes dans `docs/superpowers/plans/journaux-multifenetres-d2/`
(UTF-8 ; les journaux d'agent portent les séquences ANSI de `tracing` —
`sed 's/\x1b\[[0-9;]*m//g'` pour les lire à plat). **Une exception d'encodage**,
déclarée : `build-agent-11bis.log`, dont les lignes revenant du PowerShell
distant sont mutilées (§5).

| Pièce | Ce qu'elle porte |
| --- | --- |
| `reprise-n{1,2,4}.log` | **1ᵉʳ tirage** du banc — réfuté |
| `reprise-recalibree-n{1,2,4}.log` | **2ᵉ tirage** — réfuté une seconde fois |
| `reprise-relachee-n{1,2,4}.log` | **3ᵉ tirage** — reçu aux trois rangs |
| `topologie-{,recalibree-,relachee-}{avant,apres}-n*.log` | Contrôles d'absence de fuite, **depuis un processus neuf** |
| `construction-recalibree.log`, `build-agent.log`, `build-agent-11bis.log`, `fraicheur-binaire-11bis.log` | Fraîcheur des binaires mesurés |
| `demonstration-D-blocs-notes.log` + `agent-D.log` | **Le passage décisif** de la démonstration bout en bout |
| `demonstration-C-applications-variees.log` + `agent-C.log` | Applications variées, mêmes conclusions |
| `demonstration-A-viewport-non-apparie.log`, `demonstration-B-navigateur-residuel.log` | Les deux passages perdus, versés parce que chacun documente un piège |
| `demonstration-ouverture-retentee.log` + `agent-ouverture-retentee.log` | L'exécution de vérification du réessai à l'ouverture |
| `epreuve-downcast.log` | L'échec provoqué qui prouve la lecture du HRESULT |
| `topologie-{avant,apres-A,entre-C-et-D,apres-D,avant-11bis,apres-11bis}.log`, `purge-entre-C-et-D.log` | Contrôles de topologie de la recette |
| `captures-d/` | Six captures d'écran de fin de séquence |
| `instrument/` | `pilote-recette-d2.mjs` et `pilote-recette-11bis.mjs` **dans leur état d'exécution**, `vm-it.sh`, les scripts PowerShell |

---

## 0. Le verdict, et il est double

**Le défaut central de D1 est réparé, et démontré réparé en conditions de
produit.** Créer une sortie d'affichage virtuelle ne tue plus les captures en
cours : sur le passage décisif, **44** pertes d'accès `0x887A0026` ont été
encaissées et **aucune session n'en est morte** ; les montées 1→2, 2→3 et 3→4
sont propres ; il n'existe **qu'une seule** ligne `clôture de session amorcée`
sur tout le passage et elle est **sollicitée** — elle tombe 0,7 s après un
`WM_CLOSE` réel. Le tout sur de vraies applications, pas des mires.

**Et le critère de réception n'est pas atteint.** Il exigeait « répété jusqu'à
**cinq** fenêtres simultanées » ; on en atteint **quatre**. La cinquième
duplication DXGI, dans un cinquième processus, est refusée en `0x887A0022`
(`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`) par une limite de **concurrence** qui
**résiste à trois secondes de patience explicite** — ce qu'aucune mesure
antérieure de ce dépôt n'avait éprouvé. **La couche qui l'impose n'est pas
identifiée**, et **rien n'établit que 4 soit une borne du système** : c'est le
nombre auquel la montée s'est arrêtée.

**Donc : D2 n'est pas reçu au sens de son critère, et il répare pourtant ce pour
quoi il existait.** Les deux propositions valent ensemble ; ni l'une ni l'autre
n'annule sa voisine.

Il a fallu **trois mesures** là où le plan en prévoyait une, dont **deux
réfutations**, avant que la cause soit trouvée : `rouvrir()` demandait une
seconde duplication de la même sortie **sans avoir relâché la première**.

---

## 1. Ce qui a été exécuté, et ce qui a été écarté

### Exécuté

Les **quatre points et demi** que la conception (§2.1) se donnait, et rien de
plus :

| Point de la spec | Ce qui a été fait |
| --- | --- |
| §3 — la perte d'accès devient récupérable | `EchecAcquisition`, `capture/reprise.rs`, fenêtre de reprise bornée en **durée** (8 s, pas de 150 ms), non bloquante |
| §4 — désigner la sortie par son nom | `\\.\DISPLAYn` partout : `DesktopCapture::sur_sortie`, `SORTIE_DXGI`, table du superviseur, `prises` |
| §5.1 — apparier une sortie fraîche | viewport arrondi en pair côté client, `TOLERANCE_PX = 4`, attente **sur condition observable** (`LIMITE_RATTACHEMENT = 5 s`, `PAS_RATTACHEMENT = 100 ms`) |
| §5.2 — le clavier | `SetForegroundWindow` avant injection, **retour vérifié et journalisé** |
| §5.3 — une fenêtre vivante n'est plus oubliée | `Etat::SansSession`, `relancer_les_orphelines`, `RELANCES_MAX = 3` |

**Douze tâches planifiées, dix-sept exécutées** : cinq tâches ont été ajoutées en
cours de route (6 bis, 6 ter, 6 quater, 6 quinquies, 11 bis), toutes rendues
nécessaires par une mesure — jamais par un choix de confort. Deux d'entre elles
(6 ter, 6 quinquies) sont des **rejeux du point d'arrêt**, deux (6 bis,
6 quater) des correctifs qu'une réfutation a imposés, une (11 bis) un correctif
qu'un défaut révélé par la recette a imposé.

**État des suites automatisées à la fin** : `cargo test` → **271 passés**
(251 au départ du plan) ; `npm test` côté client → **88 passés** (4 neufs,
tâche 8).

### Écarté

- **La sérialisation superviseur→enfants** (spec §8), voie de repli si l'étape 1
  réfutait la reprise. Elle a failli être engagée après la **seconde**
  réfutation ; la relecture du code a trouvé la vraie cause avant. **Elle n'a
  jamais été implémentée** et reste disponible.
- **Toute mesure neuve** que la spec §2.3 excluait : plafond d'encodeurs en
  multi-processus, latence, cadence. Le plafond de **duplications** rencontré en
  §3.1 n'a pas été cherché — il s'est imposé.
- **Le redimensionnement d'une fenêtre déjà ouverte** (le pilote SudoVDA n'a pas
  de `SET_MODE`), tranché en D1, non rouvert.
- **Le caractère global de `SendInput`** : posé, observé, déclaré — pas corrigé.
- **La destruction d'une sortie sous duplication ouverte** n'était pas exigée.
  Elle a pourtant été exercée et encaissée : voir §2.4.

---

## 2. Le déroulé, avec son issue réellement observée

### 2.1 Étape 1 du plan de recette — un point d'arrêt franchi au troisième tirage

Le banc `MULTIFENETRE_REPRISE=<k>` crée *k* sorties virtuelles, y ouvre *k*
duplications, capture, **crée une sortie de plus**, et vérifie que les *k*
duplications reprennent et rendent des images justes. Trois rangs : k = 1, 2, 4.
**Une exécution par rang à chaque tirage — aucun taux n'en découle.**

| | 1ᵉʳ tirage | 2ᵉ tirage | **3ᵉ tirage** |
| --- | --- | --- | --- |
| Journaux | `reprise-n*.log` | `reprise-recalibree-n*.log` | `reprise-relachee-n*.log` |
| Calibrage | 3 tentatives, sans délai | fenêtre 8 s, pas 150 ms | fenêtre 8 s, pas 150 ms (**inchangé**) |
| Ordre dans `rouvrir()` | acquiert **sans** relâcher | acquiert **sans** relâcher | **relâche puis acquiert** |
| Tentatives max par voie | 3 (le budget entier) | **54** (le maximum théorique) | **1** |
| Échecs de `rouvrir()` | — | **0 sur 378** | **0 sur 7** |
| HRESULT | inféré | inféré | **relevé : `0x887a0026`** |
| `images_apres` (k=1/2/4) | `[0]` / `[0,0]` / `[0,0,0,0]` | idem | `[892]` / `[889,888]` / `[884,883,883,882]` |
| `voies_vivantes_apres` | 0 / 0 / 0 | 0 / 0 / 0 | **1 / 2 / 4** (= `voies_totales`) |
| `verdicts_faux_apres` | 0 — **vacueux** | 0 — **vacueux** | 0 — **sur 892 / 885 / 876 lectures** |
| **Verdict** | RÉFUTÉ | RÉFUTÉ | **REÇU** |

**Première réfutation** (tâche 6). Les trois tentatives sans délai sont brûlées
en **14 à 21 ms**, les voies meurent 66 à 232 ms après la perturbation, zéro
survivante sur sept. Diagnostic posé alors : *le budget compte des tentatives là
où le phénomène a une durée*. La spec §3.4 a été **réécrite** (commit `9ca1ee5`)
pour une fenêtre bornée en durée, non bloquante — décision du propriétaire du
plan, pas de l'implémenteur.

**Seconde réfutation** (tâche 6 ter). La fenêtre de 8 s est consommée **en
entier** : **54 tentatives par voie** aux sept voies des trois rangs, soit
exactement le maximum théorique (`8000 / 150 = 53,3`, **calculé**). **Zéro échec
de `rouvrir()` sur 378 tentatives** — la duplication se reconstruit à chaque fois
et rend **aussitôt** une nouvelle perte d'accès. Le délai est donc écarté comme
cause unique : 8 s de réessais échouent là où une sonde post-mortem qui
reconstruit tout réussit **7 fois sur 7**, 3 s plus tard.

**La cause, trouvée par relecture du code et non par une mesure de plus.**
`capture.rs` appelait `dupliquer()` alors que le champ détenait **encore**
l'ancienne duplication, relâchée seulement à l'affectation du résultat. Or
`CLAUDE.md` porte déjà la doctrine : **DXGI n'autorise qu'une duplication ouverte
par sortie**. L'appel réussissait et rendait un objet mort-né. C'est l'explication
la plus économique des deux réfutations — et le correctif était juste de toute
façon.

**Le témoin qui départage** (tâches 6 quater et 6 quinquies). Relâchement
explicite avant acquisition, HRESULT porté nu par `AccesPerdu(i32)`. Résultat :
tentatives par voie **54 → 1**, et le critère de réception atteint aux trois
rangs.

Délai entre la perturbation et la réouverture — **relevé** : **+48,3 ms** (k=1),
**+70,5 et +81,4 ms** (k=2), **+104,8 à +143,5 ms** (k=4). Durée d'interruption
**calculée** (déficit d'images ÷ 90 i/s nominaux, donc borne **supérieure**) :
≈ **89 ms**, ≈ **133 ms**, ≈ **189–200 ms**.

> **Conséquence de calibrage, à consigner** : la fenêtre de 8 s consomme
> **1 tentative sur 54** et **1 à 2,5 % de sa durée** (calculé) pour le cas
> mesuré. Elle est **très surdimensionnée** pour ce cas-là. Ce n'est pas un
> défaut en soi — c'est un plafond, pas une attente — mais **rien ici ne
> justifie sa valeur** : aucun cas lent n'a été observé, donc aucune donnée ne
> dit à partir de quelle valeur elle deviendrait trop courte. **Ne pas la
> réduire sur la foi de ce seul relevé.**

**Ce que l'attribution causale vaut, exactement.** Entre le 2ᵉ et le 3ᵉ tirage,
une seule variable de **comportement** a changé — l'ordre `relâcher / acquérir`.
Les deux autres modifications sont la journalisation du HRESULT (sans effet) et
un correctif de relecture qui ne s'active que si `rouvrir()` rend une erreur, ce
qui n'est jamais arrivé (0 échec sur 7). L'attribution est donc propre, **à la
réserve près qu'elle repose sur une exécution par rang de part et d'autre**.

### 2.2 Étape 2 — la démonstration bout en bout (tâche 11)

Superviseur, page-shell, navigateur, applications Windows réelles. Quatre
passages, **deux exploités** (C et D). Départ : **k = 1** fenêtre préexistante,
une seule sortie DXGI.

Passage D, montée pas à pas (`demonstration-D-blocs-notes.log`) :

| # | Étape | Pages d'application | `clôture` | `réouverture` | `0x887A0022` | Ligne |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Bloc-notes 2 | 1 → **2** | 0 | 0 → 1 | 0 | l. 62 |
| 2 | Bloc-notes 3 | 2 → **3** | 0 | 1 → 3 | 0 | l. 110 |
| 3 | Bloc-notes 4 | 3 → **4** | 0 | 3 → 6 | 0 | l. 168 |
| 4 | Bloc-notes 5 | 4 → **4** | 0 | 6 → 38 | **0 → 4** | l. 233 |
| 5 | **Fermeture réelle** du Bloc-notes 2 | 4 → **3** | **0 → 1** | 38 → 41 | 4 | l. 281 |
| 6 | Bloc-notes 6 (après la fermeture) | 3 → **4** | 1 | 41 → 44 | 4 | l. 339 |

Totaux du journal d'agent (`agent-D.log`) — tous **relevés** :

| Motif | Compte |
| --- | --- |
| `clôture de session amorcée` | **1** (sollicitée) |
| `accès à la duplication perdu, réouverture` | 44 |
| `0x887a0026` (mutex abandonné) | 44 |
| `0x887A0022` (duplication refusée) | 4 |
| `enfant lancé` / `enfant mort de lui-même` | 9 / 4 |
| `sortie virtuelle créée` | 9 |
| `premier plan obtenu` / `SetForegroundWindow refusé` | 4 / **0** |
| lignes `ERROR` (toutes causes) | **0** |
| `création de sortie refusée` (pilote) | **0** |

**La clôture unique est vérifiée sollicitée, pas supposée** : elle tombe à
`20:18:01.27`, soit **0,7 s** après l'envoi du `WM_CLOSE` de l'étape 5, et la
page correspondante se détache dans le même intervalle. Le passage C, sur une
séquence bien plus agitée (16 enfants lancés, 12 morts), porte **zéro** clôture.

Également démontré, en conditions de produit :

1. **Quatre fenêtres navigateur, chacune montrant son application et elle seule**,
   plein cadre à 1280×720, RTT 1–3 ms (`captures-d/`, superposition de
   statistiques lisible sur l'image).
2. **Fermer une fenêtre ferme sa session, et elle seule.**
3. **La topologie revient à son état de départ, nom pour nom**, contrôlée
   **depuis un processus neuf**, superviseur pourtant tué net. **Aucune sortie
   n'a fuité**, ni au passage D ni à l'exécution de la tâche 11 bis.
4. **Le son n'est porté que par une fenêtre** — `octetsA = 24321` sur `w-1`,
   `null` sur les trois autres, ce que le côté agent confirme.

### 2.3 Le clavier — un acquis que D1 ne pouvait ni démontrer ni réfuter

`premier plan obtenu` = **4**, `SetForegroundWindow refusé` = **0**. Après une
frappe dans chacune des quatre pages, la relecture `WM_GETTEXT` des cinq
Bloc-notes montre que **les quatre fenêtres qui ont une session ont reçu du
texte, la cinquième — qui n'en a pas — n'a rien reçu**, et que **chacune a reçu
exactement les quatre caractères qui lui étaient destinés**, non le cumul des
quatre frappes.

**Portée exacte, à ne pas élargir.** Cela montre que quatre frappes distinctes
ont atteint quatre fenêtres distinctes, chacune la sienne. Ce n'est **pas** une
preuve que le routage est correct en général : une seule frappe par fenêtre,
aucune frappe concurrente, aucune fenêtre non-Bloc-notes relue, et la sonde est
**séquentielle** — `SetForegroundWindow` a eu tout le loisir de s'appliquer entre
deux frappes. **`SendInput` reste global à la session Windows** ; ce relevé ne
dit rien du cas où deux utilisateurs frapperaient en même temps. La limite que
la spec §5.2 annonçait est donc **posée, observée, et toujours entière**.

### 2.4 Deux autres acquis que D1 déclarait ouverts

- **La DESTRUCTION d'une sortie abandonne le mutex elle aussi — et la reprise
  l'encaisse.** D1 écrivait « le cas n'a jamais été exercé ». Il l'est :
  `agent-D.log` l. 373-376 montre une destruction suivie à 22 et 25 ms de deux
  pertes d'accès `0x887a0026`, **sans qu'aucune création ne s'intercale**. Aucune
  session n'en meurt.
- **La piste du périphérique D3D11 conservé est devenue SANS OBJET — et non pas
  « réfutée ».** Elle avait été formulée après la seconde réfutation, comme la
  différence la moins explorée entre la sonde post-mortem (qui bâtit un
  périphérique neuf) et `rouvrir()` (qui le conserve, l'encodeur y étant lié).
  **Elle n'a jamais été mise à l'épreuve** ; elle n'a simplement plus rien à
  expliquer, le symptôme ayant disparu avec la correction de l'ordre
  d'opérations. Elle reste disponible si un symptôme voisin réapparaissait. Ne
  pas écrire qu'elle a été réfutée.

### 2.5 Le réessai à l'ouverture (tâche 11 bis) — mécanisme acquis, bénéfice non produit

La tâche 11 a révélé que **l'ouverture initiale de la duplication n'avait aucune
reprise**, alors que le HRESULT rencontré se nomme lui-même « pourra l'être
ultérieurement ». Une fenêtre de réessai de **3 s** (`DUREE_FENETRE_OUVERTURE`)
a été posée sur la seule `dupliquer`.

**Le mécanisme est prouvé fonctionner**, par deux observations indépendantes :

- **épreuve provoquée** — une duplication d'amorçage volontairement gardée
  vivante fait échouer l'ouverture suivante, et le journal écrit
  `hresult="0x80070057" attendu_ms=0` : le code est **lu** sous le `.context()`,
  et la classification est **juste** (ce code n'est pas retentable, abandon
  immédiat) ;
- **en production** — **80** lignes de réessai et **4** abandons à
  `attendu_ms = 3030, 3020, 3020, 3020`, soit 20 × 150 ms ≈ la fenêtre, tenue au
  millième.

**Et le bénéfice visé ne s'est pas produit : 44 réouvertures avant, 44 après**,
sur les huit marqueurs que les deux exécutions partagent (`clôture`,
`réouverture`, `0x887a0026`, `0x887A0022`, `sortie virtuelle créée`,
`enfant lancé`, `enfant mort de lui-même`, plafond de fenêtres) — **identiques**.

**Pourquoi, et c'est le fait neuf** : **aucun** des échecs d'ouverture observés
n'était un transitoire. Les quatre séquences de réessai ont couru leur fenêtre
**entière** puis renoncé — **zéro reprise réussie sur quatre** — et aux étapes 1
à 3 il n'y a eu **aucun** échec d'ouverture du tout. Un réessai n'absorbe qu'un
transitoire ; il n'y en a pas eu.

⚠️ **Ce que ce relevé établit est « durable au-delà de 3 s ». « Plafond de
concurrence » est une attribution, et elle vient d'ailleurs** — de la tâche 11
(la 5ᵉ refusée quand quatre tiennent, fermer-puis-rouvrir qui réussit), pas de
cette exécution-ci. C'est la lecture la plus raisonnable ; **le code, lui, ne
distingue pas** une reconfiguration passagère d'un plafond durable.

**Le réessai a été conservé malgré son coût** (≈ 3 s par enfant condamné,
≈ 12 s avant que le refus remonte à l'utilisateur), pour un motif unique et
assumé : **l'abandon est devenu diagnosticable et porte son HRESULT.** C'est par
cette ligne `ERROR` que le plafond se lit désormais, là où l'enfant mourait sur
une erreur nue.

---

## 3. Ce qui a échoué

### 3.1 Le critère à cinq fenêtres — un plafond de quatre, non anticipé par le plan

À la cinquième fenêtre, l'enfant meurt **avant d'avoir capturé une image**, sur
l'ouverture de sa duplication :

```
Error: duplication de la sortie écran
Caused by:
    Une ressource n'est pas disponible au moment de l'appel, mais elle pourra
    l'être ultérieurement. (0x887A0022)
```

**L'épreuve qui départage un plafond d'une indisponibilité passagère.** Deux
lectures tenaient. L'étape 5 puis l'étape 6 du passage D les séparent : fermer
une fenêtre (4 → 3 sessions) puis en ouvrir une neuve **réussit** (3 → 4)
**sans un seul refus de plus**. La place libérée suffit ; la même opération qui
échouait à 4 réussit à 3.

**Et il résiste à de la patience explicite** : trois secondes de réessais, à
quatre reprises, n'en ont jamais absorbé un seul (§2.5). **Aucune mesure
antérieure de ce dépôt n'avait éprouvé cela.**

**Ce que ce plafond n'est pas, et ce qu'on ignore** :

- **ce n'est pas le plafond de sorties virtuelles** (10, mesuré le 31 juillet) :
  9 sorties ont été créées dans D **sans un seul refus du pilote** ;
- **ce n'est pas le plafond d'encodeurs NVENC** (8, mesuré le 31 juillet) : la
  mort survient à l'ouverture de la duplication, **avant tout encodeur** ;
- **le chantier du 31 juillet a tenu 8 duplications de front — mais dans un seul
  processus.** Ici elles sont dans N processus distincts. Que la différence
  tienne à cela est une **inférence**, appuyée sur le rapprochement de deux
  mesures faites sur des montages différents : **rien ici ne l'établit** ;
- **la couche qui impose ce 4 n'est pas identifiée** (Windows, pilote NVIDIA,
  virtualisation, SudoVDA) ;
- **4 n'a pas été cherché comme une borne** : c'est le nombre auquel la montée
  s'est arrêtée. Vu **trois fois** — passages C et D de la tâche 11, exécution de
  la tâche 11 bis — sur la même VM, le même binaire pour les **deux premières**.
  **Trois observations concordantes, pas un taux.**

### 3.2 Le bénéfice du réessai à l'ouverture

Voir §2.5 : mécanisme acquis, bénéfice non produit, cause nommée au §7.1.

### 3.3 Deux passages de recette perdus, et un troisième amputé

- **Passage A** — perdu sur un défaut d'instrument : le pop-up de Chrome annonce
  **1280×632**, le pilote crée bien une sortie mais **Windows la rend en
  1280×720**, soit 88 px d'écart, très au-delà de la tolérance de 4 px. Le
  superviseur rend la sortie et refuse la fenêtre, en boucle jusqu'à l'abandon.
  ⚠️ **Le journal d'agent de ce passage n'a pas été conservé** — écrasé par un
  relevé de topologie pris ensuite. La ligne de refus a été lue en direct ;
  **elle n'est montrable dans aucune pièce versée**, et n'est donc qu'un constat
  rapporté.
- **Passage B** — avorté en 0,5 s par le contrôle d'identité du navigateur, qui a
  levé une **vraie** collision (`le port 9982 est tenu par le pid 1345810, pas
  par notre Chrome`).
- **Passage C** — n'a pas rendu ses captures d'écran : `Page.captureScreenshot`
  sur une page portant un flux WebRTC actif n'a **jamais** rendu, et le pilote y
  est resté suspendu sans fin.

### 3.4 Trois défauts d'énoncé, du même type que ceux des chantiers précédents

Consignés parce que le mode de défaillance dominant de ce dépôt est **l'énoncé**,
et que ce sous-bloc en porte trois exemples de plus :

1. Le message de commit `8b2fca4` affirme que « le solde de lignes reste
   négatif » pour `windows_source.rs` ; il est **nul** (648 → 648). Formule
   recopiée du brief sans être vérifiée.
2. Le message de commit `e0e54aa` affirme que « l'état `None` ne s'échappe jamais
   de `rouvrir` » — **faux**, et gravé dans l'historique. La revue de la même
   tâche l'a relevé comme critique : ce `None` rompait la boucle de reprise au
   premier échec, et **corrompait les deux chiffres mêmes que le témoin devait
   lire**. Un témoin dont l'échec ne s'attribue plus ne témoigne de rien.
3. La tâche 11 bis a transmis au coordinateur un piège (« deux processus peuvent
   dupliquer la même sortie DXGI ») comme un **fait établi** alors qu'il ne
   l'était pas : le journal qui l'aurait montré avait été écrasé. **L'implémenteur
   l'a signalé de lui-même**, et l'affirmation a été retirée. Elle n'est ni
   établie ni réfutée — **personne ne devrait s'appuyer dessus dans un sens ou
   dans l'autre**.

---

## 4. Ce que cette démonstration N'établit PAS

- **Aucun taux, nulle part.** Une exécution par rang au banc (aux trois tirages),
  une exécution exploitée par configuration à la recette. Une reprise qui marche
  une fois ne prouve pas qu'elle marche toujours — et le sous-bloc D1, lui, avait
  reproduit son défaut trois fois sur trois.
- **Rien au-delà de quatre fenêtres simultanées**, et **4 n'est pas prouvé être
  une borne du système**.
- **La couche qui refuse la 5ᵉ duplication n'est pas identifiée.**
- **Le mécanisme de l'abandon du mutex reste inconnu.** On sait le traiter, pas
  l'expliquer — c'était un risque assumé d'avance (spec §9), il est confirmé
  entier.
- **Le seuil réel de stabilisation de la topologie n'est pas mesuré.** On sait
  que ~20 ms ne suffisaient pas au premier tirage, et que la reprise a
  effectivement abouti en **48 à 144 ms** (relevé) sur le cas mesuré. Rien n'a
  été observé d'un cas lent : la fenêtre de 8 s reste **majorante et non
  calibrée**.
- **Rien de la latence, de la cadence, ni de la durée.** Session la plus longue
  ≈ **4 min 30 s** (`w-1` du passage D). RTT relevés ponctuellement seulement.
- **Rien du plafond d'encodeurs en multi-processus**, que D1 devait relever et
  n'a pas approché : la mort survient avant tout encodeur, et le maximum
  construit de front reste **4**, dans 4 processus. Le **8** connu reste un
  chiffre de **processus unique**.
- **Une seule application réelle au passage décisif** (Bloc-notes ×5). Le passage
  C ajoute Paint, WordPad et l'Explorateur, mais son compte de fenêtres n'est pas
  contrôlé — **Paint ouvre deux fenêtres éligibles**.
- **Aucun redimensionnement de fenêtre, aucun recouvrement, aucun déplacement**
  n'a été exercé.
- **Le chemin `resize` n'est pas exercé.** `new_sans_attente` (`Duration::ZERO`,
  correctif de la ronde de la tâche 11 bis) est vérifiée par la compilation et la
  lecture, **pas par une exécution**.
- **La branche `est_ouverture_retentable(ACCES_PERDU)` n'a jamais été exercée à
  l'ouverture** : seuls `0x887A0022` et `0x80070057` y ont été observés. Elle
  repose sur le test unitaire et sur rien d'autre.
- **Aucune unité H.264 n'a été décodée** hors du décodeur du navigateur ; la
  justesse de l'image repose sur le contrôle visuel des quatre captures, et sur
  les verdicts **échantillonnés** du banc (contrôle en rotation, une voie par
  tour : ≈ 876 lectures pour 3 532 images capturées au rang k=4).
- **Le chemin d'extinction propre du superviseur n'a jamais été exercé** : il a
  été tué net (`schtasks /end`) à chaque fois.
- **La comparaison tâche 11 / tâche 11 bis n'est pas un A/B contrôlé** : deux
  exécutions uniques, même VM et même protocole, mais rien n'a été tenu constant
  par construction.
- **La fraîcheur du binaire de l'exécution de vérification de la tâche 11 bis
  n'est adossée à aucune pièce** — `build-agent-11bis.log` ne porte aucun
  horodatage, et deux constructions ont eu lieu dont une seule est versée.
  `fraicheur-binaire-11bis.log` couvre la **ronde de correction**, pas
  l'exécution de vérification. *(Celle de la tâche 11, elle, est couverte :
  `build-agent.log` + `git status` vide à `fbbc6dd`.)*
- **Un verdict « noire » unique reste inexpliqué** : `verdicts_faux_avant = 1` au
  rang k=4, dans la passe **témoin**, présent dès la première ligne périodique et
  jamais reproduit ensuite. Artefact récurrent du démarrage de passe (vu aussi
  aux tirages précédents), **cause non établie**. Il ne touche pas le critère,
  qui porte sur `verdicts_faux_apres`.

---

## 5. Pièges rencontrés — à connaître avant de retoucher ce terrain

- **Le pilote de sortie virtuelle QUANTIFIE la résolution demandée.** Demander
  1280×632 rend une sortie **1280×720**. L'appariement tolère 4 px : aucune
  fenêtre ne s'ouvre alors, et **rien ne le dit hors du journal d'agent**.
  Imposer au navigateur une taille que le pilote rend à l'identique.
- **`Page.captureScreenshot` peut ne jamais rendre** sur une page portant un flux
  WebRTC actif — et la même famille de gel frappe **toute** évaluation CDP sur
  une telle page (la boucle de relecture de `window.__console` du pilote s'y est
  figée à son tour). **Borner toute évaluation CDP.**
- **Un port de débogage qui répond ne prouve pas que c'est le bon navigateur** —
  piège hérité de D1, et le contrôle d'identité a levé une vraie collision.
- **Compter les pages « d'application » en excluant `about:blank`.**
- **Paint ouvre deux fenêtres éligibles** (`Paint` + `UIRibbonWorkPane`) : sur une
  recette qui compte des fenêtres, n'employer que des applications à fenêtre
  unique — ou compter les fenêtres, jamais les lancements.
- **Un Bloc-notes modifié ne se ferme pas sur `WM_CLOSE`** (dialogue
  « Enregistrer ? »).
- **`pkill -f <motif>` depuis un shell dont la ligne de commande contient le
  motif tue le shell lui-même** (exit 144, et la suite de la chaîne ne s'exécute
  pas). Tuer par PID relevé.
- **`scripts/run-agent.sh` ne transmettait pas `MULTIFENETRE_REPRISE`** — même
  piège que `SUPERVISEUR` en D1, corrigé de la même façon. **Toute variable neuve
  du banc doit y être ajoutée explicitement.**
- **`build-agent.sh` ne pose pas `[Console]::OutputEncoding`** : les lignes
  revenant du PowerShell distant en reviennent mutilées (« Au caract⏎re
  Ligne:1 »). C'est exactement le **défaut à deux réglages** que `CLAUDE.md`
  documente. Le script est partagé : le défaut est **signalé, non corrigé**.
- **Un journal d'agent s'écrase facilement.** Deux pièces ont été perdues de
  cette façon dans ce sous-bloc (le passage A, et le journal qui aurait étayé
  l'affirmation retirée du §3.4). **Copier le journal avant tout relevé qui
  écrit au même endroit.**
- **La VM s'est éteinte spontanément deux fois** en cours de sous-bloc,
  conformément à ce que `CLAUDE.md` décrit. Vérifier `virsh list --all` **et**
  `Get-Process agent` avant chaque séquence.
- **Leçon de méthode n°1 — un défaut du code fourni par un plan doit être
  signalé, pas recopié.** La tâche 9 a implémenté verbatim un code de brief qui
  portait un bug (sentinelle `bool` initialisée à `false`, rendant **muet le tout
  premier refus** — précisément le cas que la trace existait pour révéler). La
  consigne « signaler un défaut du plan » a ensuite été explicitée comme valant
  aussi pour un **bug** dans le code fourni, et non seulement pour une divergence
  de spécification. Elle a porté : la tâche 10 a trouvé et signalé d'elle-même un
  décalage d'un cran dans le test de son brief.
- **Leçon de méthode n°2 — une relecture de code vaut parfois une troisième
  mesure.** Deux réfutations coûteuses ont été closes non par un tirage de plus
  mais par la lecture d'une ligne. Quand deux mesures successives réfutent une
  hypothèse **sans que le symptôme change de forme**, relire le chemin avant de
  recalibrer.

---

## 6. État de la VM à la fin

`virsh list --all` → `Windows` **en cours d'exécution**, `/media/vm/dev`
accessible, aucun processus `agent` résiduel.

**Topologie** : contrôlée **depuis un processus neuf** après chacune des
séquences ; `topologie-apres-D.log` et `topologie-apres-11bis.log` portent tous
deux `1 sortie(s) attachée(s)`, `\\.\DISPLAY1`, identique aux relevés « avant ».
**Aucune sortie virtuelle n'a fuité** au terme d'aucune des mesures, superviseur
pourtant tué net à chaque fois.

Une seule sortie a survécu à un arrêt, **entre** les passages C et D
(`topologie-entre-C-et-D.log`) ; la **purge autonome** l'a retirée
(`purge-entre-C-et-D.log` : `purge terminée retirees=1 avant=2 apres=1`). C'est
le comportement attendu d'un processus tué net, pas un défaut du produit.

**La VM s'est éteinte spontanément deux fois pendant le sous-bloc** (une fois en
tâche 4, une fois en tâche 11 bis entre la compilation et l'épreuve), et a été
redémarrée à chaque fois. Elle a survécu à toutes les séquences de mesure
exploitées.

---

## 7. Ce qu'il reste à régler

### 7.1 En premier — ne plus détruire puis recréer la sortie à chaque relance d'enfant

**C'est la vraie cause des 32 réouvertures parasites**, et elle n'est pas celle
qu'on croyait. Quand un enfant meurt, le superviseur rend sa sortie virtuelle au
pilote puis en **recrée une** à la relance suivante — et **c'est cette
recréation** qui abandonne les mutex de toutes les duplications voisines. À
l'étape 4 du passage D, une seule fenêtre condamnée fait passer le compteur de
réouvertures de **6 à 38**.

Le réessai à l'ouverture posé en tâche 11 bis a été ajouté pour cela et **n'en a
supprimé aucune** : aucun de ces échecs n'était un transitoire. **Aucun réessai
ne peut rien contre cette cause-là.** Conserver la sortie déjà créée entre la
mort d'un enfant et sa relance est la première chose à corriger.

### 7.2 Le plafond de quatre — identifier la couche

Ce qu'on en sait : il porte sur le nombre de **duplications DXGI simultanées**
tenues par des **processus distincts** ; il vaut 4 sur cette VM ; il est
**durable**, résistant à 3 s de patience explicite ; libérer une place le rend
franchissable immédiatement.

Ce qu'on n'en sait pas : **quelle couche l'impose** (Windows, pilote NVIDIA,
virtualisation, SudoVDA), s'il tient à d'autres résolutions ou d'autres
configurations, **et si 4 est une borne du système** — ce n'est que le point
d'arrêt observé. Le rapprochement avec les 8 duplications d'un **seul** processus
du 31 juillet est une **inférence**, que rien dans ce sous-bloc n'établit.

C'est le point bloquant pour la cible de **huit** fenêtres du chantier D.

### 7.3 Une fuite de capacité reste ouverte

Une fenêtre **neuve** dont la page-shell ne répond **jamais** reste en
`AttendLeViewport` sans être ni relancée ni abandonnée, et consomme sa place
indéfiniment. **Ce n'est pas une régression de D2** — le défaut préexiste — et le
garde-fou posé en tâche 10 (`DELAI_ATTENTE_VIEWPORT_MAX = 30 s`, **majorant non
calibré**) a été volontairement borné aux entrées **relancées**, pour ne pas
étendre le périmètre.

**Remède proposé par la revue, si le cas est repris** : tamponner
`attente_depuis` **paresseusement**, au premier passage de
`relancer_les_orphelines` sur une entrée `AttendLeViewport` non tamponnée.
`Table` resterait pure et aucun appelant ne bougerait.

### 7.4 Points mineurs différés, à traiter à l'occasion

- `agent/src/diagnostics/capture.rs` — deux lignes au-delà du défaut `rustfmt`
  (l. 147 à 111 caractères, l. 328 à 102).
- `agent/src/capture/types.rs` héberge `CibleCapture`, qui n'est pas un échec.
  Le fichier a déjà été renommé (`echec.rs` → `types.rs`) ; le rangement reste
  approximatif.
- `agent/src/capture/reprise/passes.rs` (banc) — le message d'échec de peinture
  affirme que « les mires ne changent plus, DONC le bureau non plus, donc le
  compte d'images cesse d'avancer ». `Mires::peindre` échoue au premier `Present`
  fautif **après** avoir présenté les précédentes : rien ne garantit que la
  duplication cesse d'émettre pour ces voies. Formulation juste : « la peinture
  est partielle ou nulle à partir d'ici, le compte d'images ne mesure plus la
  capture ».
- **`rouvrir` en mode `CibleCapture::Bureau` peut résoudre une sortie portée par
  un autre adaptateur que `self.device`**, et `DuplicateOutput` la refuserait.
  Adjugé non bloquant : l'issue est alors `Panne`, c'est-à-dire le comportement
  actuel — pas une régression. **La VM est mono-GPU** ; c'est une limite connue,
  pas un défaut observé.
- **Les deux chemins d'abandon émettent `AnnoncerRefus` sans `AnnoncerFermeture`** :
  si une fenêtre navigateur avait été ouverte pour la session relancée, rien ne
  demande sa fermeture. **Hérité, pas introduit par D2.**
- Le commentaire de `boucle.rs` annonçait « jusqu'à `RELANCES_MAX + 1` (soit 4)
  refus » là où le compte réel est **`RELANCES_MAX + 2` = 5** — quatre tentatives
  avortées (l'originale plus les trois relances) **plus** l'abandon final.
  **Corrigé par la présente tâche.**
- ⚠️ **À ne pas reprendre ailleurs** : le « ~108 lignes » annoncé par le rapport
  de la tâche 6 bis (le texte réel dit « plusieurs dizaines de lignes ») et le
  « solde de lignes négatif » du commit `8b2fca4` (il est **nul**).

### 7.5 Ce qui reste dû à un sous-bloc ultérieur

- Le **plafond d'encodeurs en multi-processus** — toujours non approché.
- **Latence, cadence, durée** — rien n'a été mesuré.
- Le **chemin d'extinction propre** du superviseur — jamais exercé.
- **`SendInput` global à la session Windows** — la réponse structurelle
  (injection ciblée par messages de fenêtre, ou un pilote) reste hors périmètre.
- **La voie de repli de la spec §8** (sérialisation superviseur→enfants) reste
  disponible et non implémentée, si le plafond de §7.2 devait imposer un autre
  arrangement.

---

## 8. Contrôle de la dette de taille de fichier

Commande de `CLAUDE.md`, à la fin du sous-bloc :

```
1536 agent/src/encode.rs
 648 agent/src/windows_source.rs
 543 agent/src/wasapi.rs
```

**Les trois fichiers de dette gelée, et aucun de plus.** `windows_source.rs`
**n'a pas grossi** (648 → 648) : toutes les additions de D2 sont allées dans des
modules enfants (`capture/reprise.rs`, `capture/ouverture.rs`, `capture/types.rs`,
`superviseur/table/tests_relance.rs`, `windows_source/redimensionnement.rs`).

⚠️ **`agent/src/capture.rs` est à 485 lignes : sa marge est de 15 lignes.** Il
avait atteint **exactement 500** en tâche 6 quater, et n'est redescendu que parce
que la tâche 11 bis a extrait `creer_peripherique` vers `capture/ouverture.rs`.
**Toute addition future à ce fichier appelle une extraction.**
