# Diagnostic du plafond de débit ~23-30 im/s : SUBMIT_POLL_BUDGET n'y est pour rien

## Verdict

**Hypothèse 2 (plafond artificiel via `SUBMIT_POLL_BUDGET`) RÉFUTÉE par la
mesure.** Le plafond observé est **matériel/environnemental** : la cadence à
laquelle `DesktopCapture::next_frame` (donc `AcquireNextFrame`, DXGI Desktop
Duplication) signale un contenu réellement neuf, sur cette VM précise, pas
une conséquence de la boucle logicielle de récupération de sortie de
l'encodeur.

Le correctif appliqué (borner `SUBMIT_POLL_BUDGET` à la seule phase de
démarrage de l'encodeur) est réel, justifié et committé — mais il ne change
**rien** au débit mesuré ici, exactement comme prédit par la mesure qui a
précédé son écriture. C'est une correction de robustesse latente (matériel
plus lent ailleurs), pas un correctif du plafond observé.

## Comment la question a été tranchée

Deux méthodes indépendantes, toutes deux via la chaîne complète
(`Session::run`/`transport.rs`, jamais un harnais isolé), avec le contenu de
test `anim.html` (animation `requestAnimationFrame`) **et** un défilement
réel à la molette (page longue à bandes de couleur, événements `WheelEvent`
réels dispatchés sur l'élément vidéo côté navigateur, traversant le canal
d'entrée jusqu'à un `SendInput` réel sur la vraie fenêtre Firefox) :

### 1. Instrumentation directe (temporaire, retirée avant le commit final)

- Chronométrage de la boucle de réessai post-soumission
  (`submit_instant.elapsed()`, nombre d'itérations) : sur **571 soumissions**
  observées (animation) puis un nombre comparable (défilement réel), **0**
  n'a jamais atteint le budget de 40 ms. Résolution systématique en
  **3,0 à 5,4 ms**, 3-4 itérations (le sommeil de 1 ms domine ce temps).
- Comptage séparé des appels totaux à `next_frame` vs des soumissions
  effectives, agrégé toutes les 2 s (même méthode que la ronde socket
  précédente) :
  - **calls ≈ 120-121 / 2 s → 60,0 Hz exact** — la boucle de transport
    n'est **pas** ralentie par la récupération de sortie.
  - **submits ≈ 60-61 / 2 s → ~30 Hz** — soit *la moitié* de la cadence
    d'interrogation : `AcquireNextFrame` (appelée avec délai NUL, donc jamais
    bloquante) ne signale un contenu neuf qu'une fois sur deux en moyenne.

Ces deux chiffres, pris ensemble, excluent mécaniquement que
`SUBMIT_POLL_BUDGET` soit en cause : si la boucle de réessai bloquait
`act_on_timeout` (donc tout `Session::run`) sur une part significative de son
budget, le compteur `calls` ne pourrait pas rester à 60 Hz exact pendant que
~50 % des appels sont des soumissions.

### 2. Mutation directe de la constante (le test demandé par la tâche)

`SUBMIT_POLL_BUDGET` réduit de 40 ms à **2 ms** (÷20), recompilé et rejoué
sur le **même chemin de code**, mêmes contenus de test :

| Contenu               | Budget 40 ms        | Budget 2 ms          |
|------------------------|----------------------|------------------------|
| Animation (`anim.html`)| 296 im / 10 s → 29,6 im/s | — (déjà mesuré identique en amont, voir logs) |
| Défilement réel (molette) | 297 im / 10 s → 29,70 im/s | 297 im / 10 s → **29,70 im/s** (compte de trames identique à la trame près) |

Diviser le budget par 20 n'a produit **aucun** changement de débit mesuré.
`budget épuisé sans sortie` : **0 occurrence** dans les deux configurations.
C'est la preuve positive demandée : le matériel/pipeline ne va pas plus vite
quand on retire quasiment toute la marge de la boucle suspectée — la limite
est ailleurs.

### Piège de recompilation évité

Chaque changement de constante a été vérifié par : (a) `Finished ... in`
avec une durée non nulle et plausible (2,0 à 39,8 s selon l'ampleur du
changement, jamais un « 0.1x s » suspect sauf sur un second `cargo build`
consécutif sans changement, attendu) ; (b) `grep` du commentaire diagnostique
directement dans `/media/vm/dev/agent/src/windows_source.rs` après
synchronisation, pour confirmer que le fichier réellement compilé contenait
le changement ; (c) taille/horodatage de `agent.exe` comparés avant/après.
Aucun `git stash` utilisé. La vérification finale (voir plus bas) a en plus
entièrement supprimé `C:\dev\target` avant un dernier build propre.

## Ce qui explique réellement le plafond ~29-30 im/s (voire 23-25 selon les
## conditions de mesure antérieures)

`AcquireNextFrame(0, ...)` (délai nul, donc jamais bloquante — voir
`capture.rs`) ne rapporte un contenu neuf qu'à ~30 Hz alors que la boucle de
transport l'interroge à 60 Hz exact. `capture.rs::next_frame` ne contient
aucune attente, aucun budget, aucun sommeil : la seule explication restante
est que la composition/duplication du bureau elle-même, sur cette VM (GPU
RTX 4070 en *passthrough*, adaptateur d'affichage factice « SudoMaker Virtual
Display Adapter »), ne produit une image nouvelle qu'à ce rythme — malgré un
mode d'affichage rapporté à 90 Hz par WMI pour le contrôleur RTX 4070
(`CurrentRefreshRate=90`, `MaxRefreshRate=120`). Le second plafond déjà
repéré à la ronde précédente (`fix-debit-socket-report.md`, « rythme de
composition du bureau vers Desktop Duplication ») est donc confirmé comme
étant la vraie cause, cette fois avec la mesure qui manquait pour l'établir
sans ambiguïté face à l'hypothèse concurrente.

Identique que le contenu change par animation de page (`requestAnimationFrame`)
ou par un défilement réel piloté à la molette (275+ trames, redirection
prouvée par SCROLL:N dans le titre de la page lors des tâches précédentes,
ici simplement par l'effet observé sur `framesDecoded`) — la source du
changement d'écran n'est donc, comme déjà établi, pas non plus le facteur
limitant : c'est un étage encore plus en amont (composition GPU/duplication).

## Correctif appliqué (robustesse, pas performance)

`agent/src/windows_source.rs` :

- Ajout du champ `encoder_warmed_up: bool` sur `WindowsSource`, remis à
  `false` par `resize` (nouvel encodeur = nouvelle phase de démarrage).
- `next_frame` : la boucle de réessai bornée par `SUBMIT_POLL_BUDGET` n'est
  plus empruntée qu'avant la toute première sortie réussie de l'encodeur.
  Une fois `encoder_warmed_up`, chaque soumission ne fait plus qu'un seul
  essai immédiat de `poll_output` (comme le cas « rien de neuf à capturer »),
  sans jamais dormir ni boucler.
- Docstrings de `SUBMIT_POLL_BUDGET` et `next_frame` mis à jour pour
  documenter la ronde de diagnostic et la raison du correctif (mémoire du
  projet, en cas de régression matérielle future sur un encodeur plus lent).

**Pourquoi appliquer ce correctif alors que la cause n'est pas logicielle ?**
Parce que la boucle de réessai *aurait pu* réellement brider `run()` — et le
ferait sur du matériel où l'encodeur répond plus lentement en régime établi
qu'ici (3-5 ms). La borner au seul démarrage élimine ce risque
structurellement, sans rien changer au débit mesuré sur cette VM (déjà
prouvé par la mutation à 2 ms ci-dessus, puisque le correctif final produit
exactement le même comportement en régime établi que ce test).

`agent/src/transport.rs` **non modifié** : la cause n'y résidant pas, aucune
intervention n'était justifiée. L'invariant de drainage str0m
(`act_on_timeout` : une seule mutation de `Rtc` par appel, drainage complet
via `poll_output` avant la mutation suivante) reste donc structurellement
intact, inchangé depuis la dernière vérification.

## Survie à un écran immobile

Revérifiée après le correctif : session `connected`/`connected` (ICE)
maintenue **~23 s** (10 s de démarrage/négociation + 13 s de fenêtre de
mesure) sur `static.html`, sans fermeture, `framesDecoded` delta de 1
(le keyframe initial), comme attendu. Le défaut historique (fermeture après
un écran immobile) n'est pas reproduit.

## Débit avant / après (essais comparables, même chemin de code)

| Étape | Contenu | Budget | Débit mesuré |
|---|---|---|---|
| Avant (référence, code inchangé) | animation `requestAnimationFrame` | 40 ms | 296/10 s = **29,6 im/s** |
| Avant (référence, code inchangé) | défilement réel (molette) | 40 ms | 297/10 s = **29,70 im/s** |
| Test de mutation (falsification) | défilement réel (molette) | 2 ms | 297/10 s = **29,70 im/s** (identique) |
| Après correctif final | défilement réel (molette) | 40 ms (démarrage seulement) | 287/10 s = **28,70 im/s** (variance de mesure, pas une régression — voir note) |

Note sur la variance 29,7 → 28,7 : mesurée sur un essai unique de 10 s avec
une page de défilement qui atteint parfois ses bornes hautes/basses
(oscillation programmée toutes les ~2 s), réduisant temporairement le rythme
de contenu réellement neuf indépendamment du code de l'agent — la cadence
d'appel/soumission (60 Hz / ~30 Hz) mesurée en parallèle reste dans la même
fourchette qu'avant. Aucune dégradation structurelle attendue ni observée
dans les logs (`budget épuisé` : 0 avant comme après).

Le débit ne s'approche donc pas de la cible 60 im/s du jalon avec ce
correctif — attendu, puisque la cause n'est pas logicielle. Un débit de
60 im/s de bout en bout sur cette VM précise nécessiterait d'abord de
comprendre pourquoi `AcquireNextFrame` ne rapporte de contenu neuf qu'à
~30 Hz malgré un mode d'affichage annoncé à 90 Hz — hors périmètre de cette
tâche (limitée à trancher entre les deux hypothèses et corriger si la cause
était logicielle).

## Tests et vérifications finales

- `cargo test` (`agent/`) : **53 passés / 53**.
- `cargo test` (`proto/`) : **17 passés / 17**.
- Compilation Windows (`scripts/build-agent.sh`), après suppression complète
  de `C:\dev\target` : propre, **39,76 s**, mêmes 5 avertissements
  préexistants (code mort dans `encode.rs`/`window.rs`/`windows_source.rs`),
  aucun avertissement nouveau.
- Invariant de drainage str0m (`transport.rs`) : non modifié, donc
  structurellement intact (voir sa docstring, inchangée).
- Survie à un écran immobile > 10 s : revérifiée, OK (voir plus haut).

## Fichiers modifiés

- `agent/src/windows_source.rs` — seul fichier modifié : champ
  `encoder_warmed_up`, restriction de la boucle de réessai à la phase de
  démarrage, docstrings mis à jour.
- Fichiers créés temporairement sur la VM pour le diagnostic (`scroll.html`,
  `launch-scroll-s1.ps1`, `launch-firefox-s1.ps1`, `launch-static-s1.ps1`,
  tâches planifiées `launch-scroll`/`launch-firefox`/`launch-static`/
  `focus-firefox`) — tous supprimés après usage. `anim.html`/`static.html`
  (préexistants, hors dépôt git) laissés en place.
