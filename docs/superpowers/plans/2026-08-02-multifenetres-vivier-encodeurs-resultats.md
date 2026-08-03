# Sous-bloc D5 — le vivier d'encodeurs : résultats

**Date** : 3 août 2026
**Conception** : `docs/superpowers/specs/2026-08-02-multifenetres-vivier-encodeurs-design.md`
**Plan** : `docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs.md`
**Journaux** : `docs/superpowers/plans/journaux-multifenetres-d5/` — UTF-8, avec les
séquences ANSI de `tracing` (`sed 's/\x1b\[[0-9;]*m//g'` pour lire à plat).

---

## 1. La mesure pivot — verdict : `concurrence`

### 1.1 Ce qu'elle éprouvait

La conjecture ouverte depuis le 31 juillet 2026, et jamais jouée par aucun
chantier : **détruire un encodeur libère-t-il la place ?** Et la question que les
trois documents qui posent la première n'avaient jamais posée : le plafond de 8
porte-t-il sur la **concurrence** ou sur les **créations cumulées** ?

### 1.2 Le montage

`agent/src/diagnostics/multifenetre/recyclage.rs`, mode
`MULTIFENETRE_NVENC_CYCLES=10`. Un processus, **un périphérique D3D11 par
encodeur**, 1280×720 à 60 Hz et 8 Mb/s — les paramètres exacts de la seconde
recette de D4, pour que le chiffre soit opposable au sien.

Séquence : monter jusqu'au refus et le **nommer** ; puis, dix fois de suite,
détruire un encodeur et en construire un neuf.

### 1.3 Le relevé

**Quatre exécutions indépendantes**, horodatages de début et de fin tous
distincts, chacune complète :

| Journal | Refus du 9ᵉ | Cycles réussis | Verdict |
| --- | --- | --- | --- |
| `recyclage-1.log` | oui | **10 / 10** | `concurrence` |
| `recyclage-2.log` | oui | **10 / 10** | `concurrence` |
| `recyclage-3.log` | oui | **10 / 10** | `concurrence` |
| `recyclage-4.log` | oui | **10 / 10** | `concurrence` |

**Le témoin de phase 1 est exact et reproduit aux quatre exécutions** : le
neuvième encodeur est refusé à la **configuration du type de sortie** de
l'encodeur H.264 (transform matériel), en `0xC00D6D76`
(`MF_E_UNSUPPORTED_D3D_TYPE`) — **le même appel et le même code** que la mesure ②
du 31 juillet 2026 et que le défaut ouvert de D4. Sans ce témoin, la suite ne
prouverait rien : un 9ᵉ qui réussit après une destruction ne dirait rien s'il
réussissait déjà avant.

Puis, aux dix cycles : `relâchement d'un encodeur` → `relâchement terminé` →
`reconstruction RÉUSSIE`, ramenant à chaque fois le vivier à 8 vivants.

**Conclusion : détruire un encodeur libère la place, et le recyclage tient sur
dix cycles consécutifs.** Le plafond de 8 porte donc sur la **concurrence**, pas
sur les créations cumulées.

### 1.4 La ligne de la table de décision qui s'applique

La règle avait été écrite **avant** la mesure (conception §3.4). Le résultat
active sa première ligne, sans arbitrage :

> Le 9ᵉ réussit, et les dix cycles tiennent → **cible « dépasser 8 » maintenue.
> C2 se remédie en détruisant avant de construire.**

### 1.5 Ce que cette mesure N'établit PAS

- **La couche qui impose le plafond de 8 reste inconnue** — NVENC, pilote
  NVIDIA, Media Foundation, ou virtualisation. D5 la contourne, il ne l'explique
  pas. Question ouverte depuis le 30 juillet 2026.
- **Aucune image n'a été soumise.** Seules la construction et la destruction sont
  mesurées, pas la tenue en cadence.
- **Aucune duplication DXGI n'est ouverte par ce banc.** L'arrangement de
  production en tient N en plus des N encodeurs ; la réserve de « deux variables
  confondues » qui pesait sur la comparaison partagé/séparé du 31 juillet **n'est
  donc pas levée par cette mesure-ci**, elle est seulement contournée d'un autre
  côté.
- **Rien d'autres résolutions ni d'autres débits.**
- **Quatre exécutions à un seul rang de cycles (10).** Un plafond cumulé qui
  n'apparaîtrait qu'au-delà de dix recyclages ne serait pas vu par ce montage.

### 1.6 Un écart de protocole, et pourquoi il est dit ici

Une première tentative d'enchaîner trois exécutions a produit **deux journaux
identiques à la microseconde près**. L'attente cherchait la ligne de fin du banc
dans `agent.log` — **or elle y était déjà**, écrite par l'exécution précédente :
la boucle sortait aussitôt et recopiait l'ancien journal.

Corrigé en **supprimant `agent.log` avant chaque lancement**, puis vérifié par un
contrôle d'indépendance : horodatages de début **et** de fin tous distincts,
présence du refus de phase 1 et des dix cycles dans chacun.

C'est une variante du piège que `CLAUDE.md` documente déjà (« un journal d'agent
s'écrase facilement ») : ici il ne s'écrasait pas trop tôt, il **survivait** trop
longtemps. **Attendre un fait ne suffit pas quand ce fait peut être celui de la
mesure précédente : il faut d'abord détruire la trace de celle-ci.**

### 1.7 Fraîcheur du binaire mesuré

`scripts/build-agent.sh` a rendu `Finished release profile in 19.50s` — une
compilation réelle, et non le `0.13s` qui trahirait un binaire non rebâti.
Binaire : **9 129 984 octets**, daté de la compilation.

Contrôle supplémentaire, que les chantiers précédents n'avaient jamais versé :
`strings` sur le binaire rend **21** occurrences des chaînes propres au banc de
D5 (`recyclage`, `VERDICT`). La fraîcheur n'est donc pas adossée au seul
horodatage.

---

## 2. La recette — le montage, et deux instruments à déclarer

### 2.1 Le montage

Binaire mesuré : bâti le 3 août 2026 à 11:32 UTC depuis le worktree de la
branche, **9 142 784 octets** (`scripts/build-agent.sh`, `Finished release
profile in 17.41s` — une compilation réelle, pas le `0.13s` qui trahirait un
binaire non rebâti). `cargo check --target x86_64-pc-windows-gnu` passé avant,
sortie 0, 10 avertissements `dead_code` et aucun dans les fichiers neufs.

Instrument : `journaux-multifenetres-d5/instrument/pilote-recette-d5.mjs`, versé
dans son état final. Il reprend le squelette CDP de la seconde recette de D4 et
en garde les quatre contraintes de protocole : **source animée** (une fenêtre
Chrome `--app` sur une page `canvas`, **un `--user-data-dir` par fenêtre**),
aucune capture d'écran CDP, toute évaluation CDP **bornée**, survie de la VM
contrôlée après chaque rang.

### 2.2 Le focus est réel ; la VISIBILITÉ est imposée par le pilote

**C'est la limite la plus lourde de cette recette, et elle est de méthode.**

- **Le focus** est posé par `Emulation.setFocusEmulationEnabled` — l'état que la
  page rapporte par `document.hasFocus()` est bien celui-là. Le pilote déclenche
  en plus les événements `focus`/`blur`, qu'un navigateur sans interface n'émet
  pas toujours de lui-même : **il déclenche l'émission, il ne falsifie pas
  l'état.**
- **La visibilité, en revanche, est fabriquée.** Un Chrome sans interface
  rapporte `document.hidden = true` pour **toute** fenêtre d'arrière-plan — ce
  qui ne modélise pas le scénario, qui est « dix fenêtres visibles sur un
  bureau ». Le pilote redéfinit donc `document.hidden`/`visibilityState` sur un
  drapeau qu'il tient, page par page, et déclenche `visibilitychange` quand il
  le change. **La visibilité annoncée au produit est celle du scénario, pas
  celle du navigateur.**

Ce qui est réellement exercé, à partir de `attachVisibilite` : l'encodage du
message, le data channel, le transport, le capteur, le vivier, le fil de
fenêtre. Ce qui ne l'est **pas** : le rapport de visibilité du navigateur
lui-même — donc, en particulier, **la minimisation d'une vraie fenêtre par un
vrai utilisateur n'a pas été jouée.**

Une épreuve isolée est versée (`instrument/essai-visibilite.mjs`) : elle établit
que l'override fonctionne là où l'amorce CDP court (`proprietaire: "document"`),
et **qu'elle ne court PAS sur une page ouverte par `window.open`** — la course
contre la création du document est perdue. C'est pourquoi le pilote pose
l'override explicitement, page par page, avant tout geste.

### 2.3 Cinq exécutions, dont deux abandonnées — et pourquoi

| # | Ce qu'elle a donné | Sort |
| --- | --- | --- |
| 1 | Montée jusqu'au rang 10 ; **découverte** que le navigateur déclare cachées les fenêtres d'arrière-plan | **abandonnée** en cours, l'éviction aurait mesuré autre chose |
| 2 | Montée complète, C2 relevé ; l'éviction endort **huit** fenêtres au lieu d'une — l'override n'avait pas pris | **abandonnée**, diagnostic versé |
| 3 | Complète. Éviction et masquage justes ; réveil 1 mesuré des deux côtés | retenue en confirmation |
| 4 | Complète. Les **deux** réveils mesurés des deux côtés | retenue en confirmation |
| **5** | Complète, plus la lecture du bandeau client | **l'exécution rapportée** |

Les deux abandons sont versés (`essai1-abandonne-*`, `essai2-*`). **Une
exécution par relevé retenue, plus deux confirmations : aucun taux nulle part.**

---

## 3. C1 — dépasser huit fenêtres

### 3.1 Ce qui est TENU : dix fenêtres ouvertes, huit qui diffusent

Exécution retenue (`recette-pilote.log`, `recette-agent.log`) : **10 fenêtres
attachées au capteur**, 10 sorties virtuelles créées, **1 seul capteur**,
**0 `clôture de session amorcée`**, **0 `ERROR`**. La montée, relevée par
`framesDecoded` sur deux relevés espacés :

| Rang | Pages ouvertes | Qui diffusent |
| --- | --- | --- |
| 1 à 8 | 1 à 8 | **toutes** |
| 9 | 9 | **8** |
| 10 | 10 | **8** |
| 11 | 10 (refus) | **8** |

**Le plafond d'éveil tient et se voit** : à dix pages ouvertes, huit décodent des
images et deux sont figées — reproduit aux **trois** exécutions complètes.

Le LRU choisit **les deux plus anciennes** : au palier de 20 s, les endormies
sont `w-2` et `w-4`, les éveillées `w-6` à `w-20`. Le journal montre les deux
évictions de la montée, chacune **finançant** le réveil de la fenêtre neuve, et
dans le bon ordre — le sommeil **précède** le réveil :

```
12:09:34.132836Z  fenêtre endormie … session=w-2      12:09:34.220590Z  fenêtre réveillée session=w-18
12:09:50.022897Z  fenêtre endormie … session=w-4      12:09:50.151062Z  fenêtre réveillée session=w-20
```

**Ce que coûte une endormie : rien de mesurable.** Le compteur du capteur le dit
de lui-même — `session=w-4 images=0 endormie=true cadence="0.0"` — pendant que
les huit éveillées tiennent **58,4 à 68,6 i/s** chacune.

### 3.2 Les deux gestes, et les deux raisons

**Éviction par le focus** — on focalise `w-2`, qui dort :

```
12:11:58.584638Z  fenêtre endormie … session=w-6     (la moins récemment vue des huit)
12:11:58.676297Z  fenêtre réveillée session=w-2 duree_ms=124
```

Toujours huit éveillées après le geste. **Reproduit aux trois exécutions
complètes, avec la même paire (w-2 se réveille, w-6 se fige).**

**Masquage** — on cache `w-8`, éveillée :

```
12:12:16.258870Z  fenêtre endormie … session=w-8
12:12:16.340739Z  fenêtre réveillée session=w-6      (la plus récemment vue des endormies)
```

La place rendue est aussitôt reprise. **Reproduit aux trois exécutions.**

**Les deux raisons sont OBSERVÉES, et pas seulement inférées** — par le texte du
bandeau relevé dans le DOM des pages :

- `« image figée : fenêtre masquée »` → `Raison::Masquee` ;
- `« image figée : trop de fenêtres actives »` → `Raison::Evincee`, relevé sur
  `w-4` au rang 10 pendant que son compteur d'images est figé.

⚠️ **Portée exacte de ce relevé** : `expirer()` masque le bandeau **sans effacer
son texte**. Le texte prouve donc qu'un message `asleep` portant cette raison a
été reçu ; il n'établit pas que le bandeau était affiché à l'instant du relevé.
C'est le seul endroit observable où la raison apparaisse — **ni l'agent ni
l'enfant ne la journalisent.**

### 3.3 Ce qui n'est PAS tenu : l'origine du refus au rang 11

Le critère exigeait que **le refus au rang 11 vienne du pilote**
(`ERROR_TOO_MANY_NAMES`), et non d'une constante du produit. **Il vient de la
constante.** La onzième fenêtre est refusée par `CAPACITE = 10`
(`superviseur/boucle.rs`), et le refus est annoncé **à la page-shell** — jamais
au journal d'agent :

```
« _/C:/dev/anim-d4.html » n'a pas pu s'ouvrir : plus aucune sortie virtuelle disponible.
```

**Le pilote n'est donc jamais sollicité pour une onzième sortie.** Ce qui a été
établi, et qui n'est pas la même chose : **son vivier vaut bien 10 le jour de la
recette**, mesuré depuis un processus neuf douze minutes après elle
(`vivier-pilote.log`) — refus à la 11ᵉ création, `0x80070044`
(« La limite de nom … a été dépassée »), les onze sorties énumérées nommément,
puis topologie restaurée à `\\.\DISPLAY1` seul.

**Énoncé exact, donc : la constante du produit ne borne plus le système en
dessous de lui — elle lui est égale, à la date de la mesure. Mais c'est elle qui
refuse, et le chemin de refus du pilote n'est pas exercé par le produit.**
Dépasser 10 exigerait de rendre des sorties au pilote, ce que la conception a
explicitement écarté (§4.2 : le sommeil ne touche pas la sortie virtuelle).

---

## 4. C2 — le défaut de D4 est mort

**TENU, et largement.** À huit fenêtres éveillées, `set_encode_size` réussit :

| Exécution | `taille d'encodage changée` | `changement … refusé` |
| --- | --- | --- |
| retenue (5) | **53** | **0** |
| confirmation (4) | **42** | **0** |
| confirmation (3) | **103** | **0** |

**198 changements de taille acceptés, 0 refusé**, contre **18 refus sur 18** à la
seconde recette de D4 sur le même appel (`SetOutputType`, `0xC00D6D76`). Le
navigateur rapporte bien les tailles réduites (`640×360` relevé au bandeau et
dans `frameWidth`).

La cause est celle qu'avait nommée la mesure pivot : `set_encode_size` **détruit
l'ancien encodeur avant d'en construire un neuf**, et ne demande donc plus un
neuvième encodeur transitoire au matériel.

⚠️ **Ce que ce chiffre ne dit pas** : le prix assumé du remède — si la
construction du neuf échoue, l'ancien n'est plus là et la source s'épuise
(`self.fatal`) — **n'a pas été exercé**, aucun refus n'ayant eu lieu.

---

## 5. C3 — le réveil est borné

**MESURÉ, des deux côtés, sur les deux gestes**, sans seuil : rien dans le dépôt
ne permettrait d'en calibrer un.

| Geste | Agent (`duree_ms`) | Navigateur (focus → 1ʳᵉ image décodée) |
| --- | --- | --- |
| éviction (retenue) | **124 ms** | **545 ms** |
| masquage (retenue) | **113 ms** | **366 ms** |
| éviction (conf. 4) | 129 ms | 307 ms |
| masquage (conf. 4) | 201 ms | 336 ms |
| éviction (conf. 3) | 130 ms | 280 ms |

Le `duree_ms` de l'agent est le temps de **reconstruire** la duplication DXGI et
l'encodeur, image clé comprise. Sur les 17 réveils de l'exécution retenue il va
de **81 à 207 ms**. Le délai côté navigateur, lui, **inclut le pas de scrutation
du pilote (250 ms)** : c'est un majorant, et l'écart entre les deux colonnes
n'est donc pas un temps de transport.

⚠️ **Cinq mesures au total, sur trois exécutions. Aucun taux.**

---

## 6. Le battement, et l'hystérésis de 2 s

`HYSTERESIS = 2 s` a été posée **non calibrée**, et la recette devait la juger
par le nombre d'endormissements. Exécution retenue : **8 endormissements**, qui
se décomposent exactement ainsi —

| Combien | Quoi |
| --- | --- |
| **4** | transitoires **imputables à l'instrument** : à l'ouverture d'une fenêtre, le navigateur déclare brièvement cachée la précédente, qui se rendort puis se réveille **114 à 218 ms** plus tard (w-2, w-12, w-14, w-16) |
| **2** | évictions par le plafond pendant la montée (rangs 9 et 10) |
| **2** | les deux gestes de la recette |

**Aucun battement en rafale n'a été observé** — mais **le cas qui l'aurait
provoqué n'a pas été joué** : la recette ne fait jamais alterner rapidement le
focus entre deux fenêtres. **L'hystérésis n'est donc ni validée ni réfutée ; ce
qui est établi est seulement qu'elle n'a rien cassé.**

Les 4 transitoires sont, eux, un fait à retenir : dans ce montage, **ouvrir une
fenêtre endort brièvement sa voisine**. Cela vient de la visibilité que rapporte
un navigateur sans interface, pas du produit — et l'hystérésis ne peut rien
contre, par construction : elle ne protège pas contre le masquage, qui est un
geste explicite (`vivier.rs`).

---

## 7. Les contrôles, et un fait à connaître

**Reprise DXGI** : **44 pertes d'accès `0x887a0026`** sur l'exécution retenue,
**toutes encaissées** — 0 session perdue, 0 `ERROR`. Le chiffre est le même à
l'unité près sur les trois exécutions complètes (44 / 44 / 44).

**Les WARN de l'exécution retenue**, tous comptés, **aucun lié au sommeil** :
168 échecs de réception UDP transitoires ignorés, 18 `ProcessInput de l'encodeur
lent`, 10 `aucune estimation de bande passante reçue` (un par session au
démarrage), 10 `allocation TURN impossible` — **la recette s'est jouée sans
relais, sur candidats `host`**, comme celle de D3 —, puis 6 `ICE déconnecté` et
6 `enfant mort de lui-même`, **tous horodatés entre 12:12:56 et 12:12:57, soit
APRÈS le dernier relevé (12:12:33)** : c'est la fermeture du navigateur par le
pilote, pas un incident de la mesure. **0 `réveil refusé`, 0 `visibilité refusée
par le capteur`.**

⚠️ **Fait à connaître, relevé depuis un processus neuf** : après un
`Stop-Process -Force` sur les agents, **neuf sorties virtuelles survivent**
(`topologie-apres-recette.log`) — il n'y a pas de chemin de libération sur une
mort brutale. Le chien de garde du pilote finit par les reprendre (observé une
fois en moins d'une minute, une autre fois pas encore repris au bout de deux),
et `MULTIFENETRE_VDD_PURGE=1` les retire à coup sûr. **Purger entre deux
exécutions**, sans quoi la suivante démarre avec un vivier déjà entamé.

---

## 8. Ce que cette recette N'établit PAS

- **Aucun taux, nulle part.** Une exécution rapportée, deux confirmations.
- **La visibilité est fabriquée par l'instrument** (§2.2) : **la minimisation
  d'une vraie fenêtre n'a jamais été jouée**, et le recouvrement par une autre
  fenêtre n'est pas rapporté par Chrome/Linux — donc jamais exercé non plus.
- **Le refus au rang 11 vient du produit, pas du pilote** (§3.3).
- **La couche qui impose le plafond de 8 encodeurs reste inconnue** — question
  ouverte depuis le 30 juillet 2026. D5 la contourne, il ne l'explique pas.
- **Le mécanisme de l'abandon du mutex DXGI reste inconnu** — depuis D1.
- **La couche qui impose le plafond de 4 processus reste inconnue** — depuis D3.
- **L'hystérésis n'est pas calibrée** (§6), et `REPIT_APRES_ECHEC = 500 ms` ne
  l'est pas davantage : **aucun réveil n'a été refusé**, donc ce chemin n'a
  **jamais couru** en conditions de produit.
- **Le prix du remède de C2 n'a pas été exercé** (§4).
- **Rien de la latence de bout en bout** — jamais mesurée par aucun sous-bloc du
  chantier D ; le délai de réveil de C3 n'est pas la même chose.
- **Rien de la durée** : exécution la plus longue ≈ 5 min 20 s.
- **Une seule application, une seule animation, aucune interaction** : ni
  clavier, ni souris, ni audio, ni redimensionnement, ni déplacement de fenêtre.
- **La mort d'un enfant pendant que les autres diffusent** et **la fermeture
  d'une fenêtre en cours de diffusion** ne sont toujours pas exercées.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé — et le §7 montre ce que coûte son absence.
- **Une fenêtre dont le client n'annonce jamais sa visibilité ne se réveille
  jamais**, et rien ne le journalise : c'est le comportement voulu
  (`capteur/fenetre.rs`), et il n'a pas été éprouvé ici.

---

## 9. Dette de taille de fichier

Relevé **par la commande**, le 3 août 2026, en fin de sous-bloc :

| Fichier | Lignes |
| --- | --- |
| `agent/src/encode.rs` | 1536 |
| `agent/src/windows_source.rs` | **631** (648 avant D5 — le remède de C2 l'a fait **maigrir** de 17) |
| `agent/src/wasapi.rs` | 543 |

**Aucun autre fichier de code source ne dépasse 500 lignes.** Marges étroites :
`encode/arret.rs` **500** (marge 0), `capture.rs` **496** (4),
`superviseur/boucle.rs` **493** (7), `capteur/serveur.rs` **490** (10) —
**non touché par ce sous-bloc, comme le plan l'exigeait**,
`superviseur/table.rs` **489** (11).

Les fichiers neufs de D5 sont tous confortables : `vivier.rs` **261**,
`sommeil.rs` **263**, `recyclage.rs` **165**, `visibilite.ts` **63**.

⚠️ **`CLAUDE.md` annonçait `capteur/distante.rs` à 487 lignes avec une « marge
étroite de 13 » : c'était FAUX, et déjà faux le jour où ce fut écrit.** Le
fichier en fait **235**, ses tests vivant à part depuis D4
(`capteur/distante/tests.rs`, 385). Les quatre occurrences sont corrigées dans
`CLAUDE.md`.
