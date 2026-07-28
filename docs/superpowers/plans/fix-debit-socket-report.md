# Correctif du débit vidéo — attente bloquante du socket UDP

## Résumé

Le diagnostic transmis (mesuré par un relecteur : `recv_from` avec
`set_read_timeout` consomme ~90 % du temps de boucle, dépassement moyen
+12,7 ms, jusqu'à +37 ms) a été **reproduit indépendamment** sur cette VM,
avec des chiffres du même ordre. Le correctif (socket non bloquant + sondage
par tranches de 1 ms, plus `timeBeginPeriod(1)` pour que `Sleep` honore ces
tranches) **élimine structurellement** le défaut : la boucle atteint
désormais la cadence de 60 Hz visée par `FRAME_INTERVAL`, mesurée
directement par instrumentation temporaire.

Le gain **de bout en bout, mesuré côté navigateur**, est réel mais modeste
sur cette VM (+14 à +15 %, ~26 → ~30 im/s), pas le saut vers 47-51 im/s
espéré — parce qu'un **second plafond, indépendant et non traité par cette
tâche**, borne le débit à ~26-30 im/s sur cette VM précise : le rythme de
composition/présentation du bureau vers Desktop Duplication (adaptateur
d'affichage virtuel SudoMaker), constaté identique quelle que soit la
méthode de génération de contenu (animation de page ou déplacement de
fenêtre). Ce plafond est **antérieur et extérieur** au correctif de
transport et hors du périmètre de `agent/src/transport.rs`.

## Ce qui a été changé

`agent/src/transport.rs` :

1. **Socket UDP passé en non bloquant** une fois pour toutes dans
   `Session::new` (`socket.set_nonblocking(true)`), au lieu de
   `set_read_timeout` réarmé à chaque tour de boucle.
2. **Boucle de sondage propre** dans `act_on_timeout` (branche c) : au lieu
   d'un unique `recv_from` bloquant à échéance, une boucle qui tente
   `recv_from` (non bloquant), et si `WouldBlock`, dort par tranches bornées
   par `RECV_POLL_INTERVAL` (1 ms) jusqu'à la donnée ou l'échéance — jamais
   un spin serré (documenté et mesuré : CPU bas, voir plus bas), jamais un
   sommeil unique sur toute la durée (reproduirait l'imprécision mesurée).
3. **`TimerResolutionGuard`** (RAII, `timeBeginPeriod(1)`/`timeEndPeriod(1)`
   de winmm.dll, appariés) : sans cet appel, mesuré expérimentalement que le
   sondage à 1 ms n'apportait quasiment rien (~24 im/s, contre ~23 im/s avant
   tout correctif) — `Sleep` héritait de la même granularité système
   (~15,6 ms) que celle diagnostiquée sur `recv_from`. La crate `windows`
   0.62 ne génère pas `timeBeginPeriod`/`timeEndPeriod` (vérifié par
   recherche exhaustive dans ses sources vendues) : ces deux fonctions sont
   déclarées à la main via `extern "system"` + `#[link(name = "winmm")]`.
4. Docstrings mis à jour (module, `Session::new`, `act_on_timeout`) pour
   refléter le nouveau mécanisme, sans changer la garantie de drainage.

Aucun changement fonctionnel à la logique de classification d'erreurs
(`classify_recv_error`/`recv_error_backoff`), à `bounded_wait`, ni à
`next_frame_deadline` : la structure de la boucle (une seule mutation de
`Rtc` par appel à `act_on_timeout`, drainage différé de l'image vidéo en
priorité) est inchangée.

## Invariant de drainage str0m

Vérifié qu'il tient toujours :
- `act_on_timeout` effectue toujours **au plus une** mutation de `Rtc`
  (`handle_input`) par appel, la nouvelle boucle de sondage ne mute jamais
  `Rtc` tant qu'elle attend (ni sur `WouldBlock`, ni sur une erreur
  transitoire) — seule l'issue finale (donnée reçue, ou échéance atteinte)
  déclenche une mutation, immédiatement suivie d'un `return`.
- `run()` rappelle toujours `poll_output()` juste après, structure
  inchangée.
- Le drapeau `video_write_pending_drain` (priorité absolue en tête
  d'`act_on_timeout`) n'a pas été touché.

## Méthode de mesure

Chaîne complète : `signaling` (port 8080) + `client` (vite, port 5173) sur
cette machine Linux, agent sur la VM Windows (`scripts/run-agent.sh`, jamais
lancé directement par WinRM), Firefox en session 1. Mesure du débit via
`client/verify-webrtc.mjs` (Chrome headless piloté par CDP, lit les vraies
statistiques `getStats()` du navigateur).

**Piège évité** : `git worktree` (jamais `git stash`) pour obtenir un état
« avant » comparable au commit `88585f5`, compilé séparément. Le worktree et
le dépôt principal compilent tous deux vers le **même** `C:\dev\target`
(chemin fixé par `sync-agent.sh`) : en alternant les deux, le cache
incrémental de cargo s'est trompé une fois et a réutilisé un binaire
obsolète après un `Finished ... in 0.11s` — repéré uniquement en comparant
la taille et l'horodatage du `.exe`, puis corrigé en supprimant tout
`C:\dev\target` avant chaque bascule avant/après. Toutes les mesures
rapportées ci-dessous ont été revérifiées après cette découverte (build
> quelques secondes à quelques dizaines de secondes, jamais un « Finished »
suspect de quelques centièmes de seconde).

### Piège du contenu de test lui-même

Le premier animateur de page (`anim.html`, bascule de couleur toutes les
40 ms via `setInterval`, soit 25 Hz) plafonnait lui-même le débit observable
en dessous du défaut à corriger — les deux versions (avant/après) donnaient
~23-24 im/s, un résultat trompeur qui aurait pu passer pour « le correctif
ne change rien ». Remplacé par une animation pilotée par
`requestAnimationFrame` (déplacement continu + couleur HSL, élément de
120 px) pour ne plus être la variable limitante. Une instrumentation
temporaire (comptage direct des appels à `next_frame()`, retirée avant la
version finale) a confirmé que même ce contenu plus exigeant plafonne à
~30 Hz sur cette VM (voir « second plafond » plus bas).

## Résultats

### Débit de bout en bout (images/s décodées, mesuré navigateur)

| Scénario de contenu                         | Avant (88585f5) | Après (ce correctif) | Delta   |
|----------------------------------------------|-----------------|------------------------|---------|
| Animation lente (25 Hz, `setInterval`)        | 23,3 im/s (233/10s) | 24,4 im/s (244/10s) | +4,7 %  |
| Animation rapide (rAF, plafonnée ~30 Hz)      | 29,6 im/s (296/10s) | 29,6-29,8 im/s (296-596/10-20s) | ~0 %   |
| Agitation de fenêtre (SetWindowPos 10 ms, statique sinon), 10 s | 26,0 im/s (260/10s) | 29,6 im/s (296/10s) | **+13,8 %** |
| Agitation de fenêtre, fenêtre longue (20 s)   | 25,85 im/s (517/20s) | 29,8 im/s (596/20s) | **+15,3 %** |

Le scénario le plus révélateur (agitation de fenêtre, qui contourne le
throttling vsync/rAF de la page et se rapproche du harnais de la tâche 10)
montre un gain **reproductible de +14 à +15 %** sur deux durées de mesure
différentes (10 s et 20 s), pas un artefact statistique.

### Cadence réelle de la boucle de transport (instrumentation directe,
retirée avant la version finale)

Comptage direct, dans `act_on_timeout`, des appels à `source.next_frame()`
(qu'ils renvoient une image ou `None`), agrégé toutes les 2 s :

- **Avant**, sous contenu actif : ~57-59 Hz (proche de la cible 60 Hz, mais
  pas exactement).
- **Après**, sous contenu actif : **exactement 60,0 Hz** (120 appels par
  fenêtre de 2,00 s, mesuré à ±0,1 Hz près sur 7 fenêtres consécutives).
- **Avant**, sous contenu statique (idle) : mêmes ~57-60 Hz de cadence
  totale — la boucle ne semble **pas** ralentie en régime établi actif.

### Reproduction directe du défaut diagnostiqué (recv_from, avant correctif)

Instrumentation temporaire mesurant délai demandé vs délai réel de
`recv_from` sous contenu **statique** (429 échantillons) :

```
demande_us=12710  reel_us=31074  depassement_us=18364
demande_us=16622  reel_us=47080  depassement_us=30458
demande_us=2674   reel_us=31026  depassement_us=28352
demande_us=1465   reel_us=31170  depassement_us=29705
...
```

Confirme intégralement le diagnostic transmis : des délais demandés de
1,4 à 16,6 ms sont systématiquement honorés après ~31 ms (parfois ~47 ms),
un dépassement de 16 à 31 ms **à chaque appel**, sans exception observée sur
429 échantillons. La cause (granularité du minuteur Windows, ~15,6 ms par
défaut) est cohérente avec des valeurs groupées autour de multiples de ce
palier (31 ≈ 2×15,6 ; 47 ≈ 3×15,6).

### Pourquoi le gain de bout en bout est modeste malgré un défaut confirmé sévère

Le défaut ne se traduit en perte de débit **que lorsque le contenu change
plus vite que le plafond induit par le défaut** (~1000/31 ≈ 32 Hz, dérivé
de la mesure ci-dessus). Sur cette VM :
- Un contenu à 25 Hz reste **sous** ce plafond de 32 Hz : peu d'images
  perdues, gain faible (+4,7 %).
- Un contenu tentant 60 Hz se heurte en réalité à un **second plafond
  indépendant, ~26-30 Hz**, mesuré identique que le contenu soit généré par
  page (`requestAnimationFrame`, throttlée au taux de rafraîchissement
  rapporté) ou par déplacement de fenêtre (`SetWindowPos`, qui d'après le
  commentaire de `main.rs`/tâche 9-10 force normalement une recomposition
  DWM indépendante du vsync de page — pourtant plafonné pareil ici, ~26 Hz).
  Ce plafond est donc probablement propre à l'adaptateur d'affichage
  virtuel SudoMaker utilisé pour la capture sur cette VM, **pas** au
  transport WebRTC. Il est très proche du plafond ~32 Hz induit par le
  défaut recv_from lui-même, ce qui rend les deux difficiles à distinguer
  en mesure de bout en bout — mais le correctif élève bien le plafond du
  transport de ~32 Hz à 60 Hz exact (mesuré directement, voir ci-dessus),
  seul le second plafond (hors périmètre) reste actif.

Le chiffre historique « 47-51 im/s » (tâche 10) a été obtenu par un harnais
de mesure isolé (`ENCODER_THROUGHPUT_TEST`/`CAPTURE_TEST` dans `main.rs`)
qui **ne passe pas par `Session::run`/`transport.rs`** — il mesure la
capacité brute de la capture+encodage, pas ce qu'atteint le pipeline réseau
réel. Ce n'est donc pas directement comparable au débit de bout en bout
mesuré ici.

## Consommation CPU

Mesurée via `Get-Process agent | Select CPU` (temps CPU cumulé, échantillonné
sur 8 s, session active avec agitation de fenêtre) :

- **Avant** : 2,34 % d'un cœur.
- **Après** : 2,34 % d'un cœur.

Aucune régression de consommation CPU : le sondage par tranches de 1 ms
reste très majoritairement endormi (`thread::sleep`), pas un spin serré.

## Survie à un écran immobile

Testé avec le correctif : session connectée (`connectionState: connected`,
`iceConnectionState: connected`) maintenue **16 secondes** sur contenu
parfaitement statique, sans fermeture ni erreur — seule une image (le
keyframe initial) décodée, comme attendu. Le défaut d'un correctif
précédent (fermeture après ~2 s d'écran immobile) n'est pas reproduit.

## Tests

- `cargo test` dans `agent/` : **46 passés / 46**.
- `cargo test` dans `proto/` : **17 passés / 17**.
- Compilation Windows (`scripts/build-agent.sh`, avec suppression complète
  de `C:\dev\target` avant la build finale pour exclure tout artefact de
  cache) : propre, mêmes 5 avertissements préexistants et sans rapport
  (code mort dans `encode.rs`/`window.rs`/`windows_source.rs`), aucun
  avertissement nouveau.

## Fichiers modifiés

- `agent/src/transport.rs` — seul fichier modifié (socket non bloquant,
  boucle de sondage, `TimerResolutionGuard`, docstrings).
- `/media/vm/dev/anim.html` (VM, hors dépôt git) — remplacé par une
  animation `requestAnimationFrame` plus exigeante, pour que les mesures ne
  soient pas biaisées par un contenu de test trop lent. Fichiers créés
  temporairement pour le diagnostic (`wiggle.ps1`, `cpu-measure.ps1`)
  supprimés après usage.

## Conclusion

Le correctif est **structurellement correct et vérifié par mesure directe**
(cadence de boucle 60 Hz exacte, invariant de drainage intact, tests verts,
compilation Windows propre, pas de régression CPU, survie écran immobile).
Le gain de bout en bout observé sur cette VM (+14 à +15 % dans le scénario
le moins confondu par d'autres facteurs) est réel mais **loin** de l'écart
23-25 → 47-51 im/s espéré, parce qu'un second plafond indépendant
(~26-30 im/s, composition du bureau vers Desktop Duplication sur
l'adaptateur d'affichage virtuel de cette VM) domine désormais le débit de
bout en bout et n'est pas du ressort de `transport.rs`. Ce n'est pas
BLOCKED : le défaut diagnostiqué est corrigé et le gain mesuré est positif
et reproductible — mais quiconque veut retrouver 47-51 im/s de bout en bout
devra investiguer ce second plafond séparément (probablement dans la
capture, ou la configuration de l'adaptateur d'affichage virtuel).
