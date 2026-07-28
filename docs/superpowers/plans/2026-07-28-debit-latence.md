# Débit et latence : trois goulots successifs, tous logiciels

**Date** : 28 juillet 2026

Point de départ (recette du jalon 1) : **~30 i/s** pour une cible de ≥ 55, et
une latence de **médiane 276 ms · max 1517 ms** pour une cible de < 50 ms.

Résultat : **~40-44 i/s** et **médiane 60,3 ms · max 131 ms**. Aucune des deux
cibles n'est atteinte, mais la traîne de latence — le symptôme le plus
spectaculaire, un facteur 28 entre le meilleur et le pire essai — a disparu.

| Grandeur | Avant | Après | Cible |
|---|---|---|---|
| Débit (images décodées) | 29,5-30 i/s | **40,4-44,5 i/s** | ≥ 55 i/s |
| Latence min | 54,1 ms | **40,6 ms** | < 50 ms |
| Latence médiane | 275,95 ms | **60,3 ms** | < 50 ms |
| Latence max | 1516,8 ms | **131,1 ms** | < 50 ms |

Les mesures « après » viennent de la même chaîne complète que la recette
(`verify-webrtc.mjs` et `recette/harness.mjs`, compteurs `getStats()` réels
d'un Chrome piloté par CDP), sur le même contenu.

## Ce que les rondes précédentes avaient conclu, et pourquoi c'était faux

Trois documents (`fix-debit-socket-report.md`,
`diagnostic-plafond-debit.md`, `remesure-debit.md`) convergeaient vers :
« le plafond ~30 i/s vient de l'encodeur matériel, qui ne réclame une entrée
qu'à ~30 Hz sous sollicitation à cadence fixe — une interaction non résolue
avec le pilote NVENC ».

Le constat était juste, l'imputation non. `METransformNeedInput` à 23-30 Hz
n'était pas un comportement du pilote : c'était la conséquence de trois
défauts logiciels indépendants, chacun masquant les autres. Deux expériences
censées trancher ne le pouvaient pas :

- `ENCODE_TEST`/`ENCODER_THROUGHPUT_TEST` resoumettent **la même texture** en
  boucle. Y jeter une image ne coûte rien — d'où les ~80 i/s qui semblaient
  disculper le code.
- Forcer la capture à 60 Hz (`remesure-debit.md`, étape 3a) n'a rien changé
  parce que les captures supplémentaires retombaient dans le garde qui les
  jetait.

## Les trois goulots, dans l'ordre où ils se sont révélés

Chacun n'est devenu visible qu'une fois le précédent levé — d'où
l'impression, aux rondes précédentes, d'un plafond unique et irréductible.

### 1. `encode.rs::submit` jetait l'image quand l'encodeur ne réclamait rien

```rust
if (self.pending_nv12.len() as u32) < self.pending_input_requests {
    self.feed_converter(frame, sample_time, duration)?;   // sinon : image perdue
}
```

L'intention (« ne pas convertir d'avance, ce ne serait que de la latence »)
vaut pour une source qu'on peut réinterroger. `AcquireNextFrame` ne signale un
contenu qu'**une fois** : l'image écartée n'était pas remise à plus tard, elle
était perdue. Au tour suivant, l'encodeur réclamait une entrée que la capture
ne pouvait plus fournir.

Correctif : convertir sans condition, garder d'avance `MAX_PENDING_NV12 = 1`
échantillon (la plus récente — une image plus ancienne est périmée pour un
flux interactif), servir les demandes avec ce qui est prêt.

### 2. `capture.rs::next_frame` détenait l'image DXGI jusqu'au tour suivant

`release_frame()` était appelée en **tête** de fonction, jamais après la
copie. La duplication restait donc immobilisée pendant tout l'intervalle entre
deux tours, alors que `crop` avait déjà recopié les pixels dans notre propre
texture.

Mesure : la même fenêtre Firefox rendait **74 i/s** à `CAPTURE_TEST` (boucle
serrée, relâchement toutes les ~2 ms) contre **22 i/s** dans `Session::run`
(relâchement toutes les ~16,7 ms). Ce n'était ni la composition du bureau, ni
le pilote, ni la contention GPU avec l'encodeur.

Correctif : relâcher immédiatement après le recadrage. **22 → 45,5 i/s.**

### 3. Le pipeline n'avançait que d'un étage par tour de boucle

`submit` et `poll_output` n'étaient appelés qu'une fois chacun par tour de
16,7 ms. Or le cycle de l'encodeur asynchrone compte plusieurs étapes —
soumettre, produire, récupérer la sortie, libérer l'emplacement, redemander
une entrée — et une seule était franchie par tour. D'où un débit égal à la
cadence de boucle divisée par le nombre d'étapes : **`need_input_hz = 23` pour
`ticks_hz = 60`**, exactement le « plafond NVENC » des rondes précédentes.

C'est ce que `ENCODE_TEST` ne reproduisait pas : sa boucle serrée
(`while let Some(unit) = poll_output()?`) enchaîne ces étapes en quelques
microsecondes.

Correctif : `drain_ready_output` vide toutes les sorties prêtes puis appelle
`H264Encoder::flush_pending_inputs`, qui rend à l'encodeur les entrées que ce
drainage vient de lui permettre d'accepter — le tout dans le même tour.
**23 → 46 i/s de demandes d'entrée**, débit navigateur **22 → 43 i/s.**

### 4. (corollaire) Interroger à 60 Hz une source qui change à 68,5 Hz

Une fois les trois précédents levés, `captured_hz = 46` pour
`desktop_updates_hz = 68,5` : chaque tour ne remonte qu'une image, quel que
soit le nombre de mises à jour fusionnées par DXGI entre-temps.

`FRAME_INTERVAL` passé de 16,67 ms à 10 ms. Ce n'est pas une cadence
d'émission (`next_frame` rend `None` quand rien n'est prêt) mais une cadence
d'interrogation, et un tour à vide ne coûte qu'un aller-retour DXGI immédiat.
**`captured_hz` atteint alors 68,5 — exactement le rythme du bureau, plus
aucune perte à la capture.**

## Latence : l'horodatage RTP était un compteur d'images

`windows_source.rs` faisait `next_pts_90k += CLOCK_RATE_HZ / fps` **par image
soumise**, ce qui suppose une source parfaitement régulière. Desktop
Duplication ne rend une image que sur changement : les soumissions sont
espacées de 16,7 ms, de 33 ms, ou de plusieurs secondes sur un écran immobile
— alors que le compteur avançait invariablement de 16,7 ms.

La ligne de temps RTP dérivait donc du temps réel sans jamais se recaler. Le
récepteur WebRTC calcule sa gigue sur l'écart entre l'espacement d'arrivée et
l'espacement annoncé : un décalage systématiquement positif lui fait gonfler
sa cible de tampon, image après image, jusqu'à une resynchronisation brutale.

C'est exactement la signature relevée en recette et laissée sans explication :
641,8 → 877,6 → 1164,0 → 1449,9 ms, puis retour net à 83,0 ms, **sans qu'aucun
gel ne soit détecté** — ce qui excluait déjà un arrêt de la capture ou de
l'encodage, et que le document notait comme « hypothèse non tranchée ».

Correctif : lire une horloge réelle à l'instant de la capture
(`WindowsSource::next_pts_90k`), avec garantie de stricte croissance.

**Médiane 276 → 60,3 ms, max 1517 → 131 ms.** La traîne a disparu : plus
aucune valeur au-delà de 131 ms sur 8 essais exploitables.

## Compilation : toutes les mesures antérieures étaient en `debug`

`scripts/build-agent.sh` avait `PROFILE="${1:-debug}"` et `run-agent.sh`
lançait `C:\dev\target\debug\agent.exe` en dur ; `/media/vm/dev/target/` ne
contenait que `debug`. Ce n'était pas un choix, seulement une valeur par
défaut jamais revue. Les deux scripts sont passés à `release`
(`AGENT_PROFILE=debug` reste possible).

## Instrumentation conservée

`SOURCE_TRACE=1` journalise toutes les 2 s : `ticks_hz`, `captured_hz`,
`produced_hz`, `acquire_hz`, `hits_hz`, `desktop_updates_hz`, `need_input_hz`,
`encoder_inputs_hz`, `dropped_stale_hz`.

Deux choix délibérés :

- **Des statiques de processus, pas des champs.** `resize` remplace
  l'encodeur, donc sa `EncoderTelemetry` : une poignée prise au démarrage
  cesse d'être alimentée dès le premier redimensionnement — c'est-à-dire dans
  toutes les sessions réelles. C'est précisément ce qui avait rendu ce
  diagnostic si long à établir.
- **`desktop_updates_hz`** vient de
  `DXGI_OUTDUPL_FRAME_INFO::AccumulatedFrames`, le nombre de mises à jour du
  bureau que DXGI a fusionnées. C'est la seule mesure qui distingue « la
  source ne produit pas plus » de « nous l'interrogeons trop rarement » — la
  question sur laquelle deux rondes précédentes ont conclu à tort.

## Ce qui reste

- **Débit : 40-44 i/s contre ≥ 55 visés.** Le goulot est maintenant
  l'encodeur pour de bon : `produced_hz` plafonne à ~47,5 alors que la capture
  fournit 68,5. À explorer : le débit configuré (12 Mbps en production contre
  8 Mbps dans `ENCODE_TEST`, qui atteignait 66 i/s), et le fait que
  `MAX_DRAIN`/`MAX_PENDING_NV12` sont réglés au plus serré pour la latence.
- **Écart réception/décodage** : `framesReceived` ≈ 44 i/s pour
  `framesDecoded` ≈ 40 i/s, `packetsLost = 0`. Des images arrivent sans être
  décodées ; non expliqué.
- **Latence médiane 60,3 ms contre < 50 visés.** `playoutDelayHint` /
  `jitterBufferTarget` ne sont toujours pas positionnés côté client — levier
  identifié par la recette, non exploité ici.
- **Fragilité du harnais de mesure** : plusieurs essais ont échoué faute de
  focus sur la fenêtre Firefox (vol de focus par `schtasks /it`). Les
  mesures retenues sont celles où la vidéo coulait ; `verify-webrtc.mjs`
  attend désormais explicitement la première image décodée avant d'ouvrir sa
  fenêtre de mesure, au lieu de chronométrer à travers la négociation.
