# Débit et latence : quatre goulots successifs, tous logiciels

**Date** : 28 juillet 2026

Point de départ (recette du jalon 1) : **~30 i/s** pour une cible de ≥ 55, et
une latence de **médiane 276 ms · max 1517 ms** pour une cible de < 50 ms.

Résultat final : **62,6 i/s** et **médiane 47,9 ms**. **Les deux cibles sont
atteintes.**

| Grandeur | Avant | Après | Cible |
|---|---|---|---|
| Débit (images décodées) | 29,5-30 i/s | **62,6 i/s** | ≥ 55 i/s |
| Latence min | 54,1 ms | **43,6 ms** | < 50 ms |
| Latence médiane | 275,95 ms | **47,9 ms** | < 50 ms |

Le document est chronologique : les sections 1 à 4 décrivent une première
ronde qui a porté le débit à ~40-44 i/s et la latence médiane à 60,3 ms sans
atteindre les cibles, la **section 5** la ronde suivante qui les franchit.
Sa conclusion réfute celle des sections précédentes — c'est délibéré, et
instructif : quatre rondes de suite ont attribué au matériel un plafond
d'origine logicielle.

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

## 5. Le dernier goulot : la cadence annoncée aux MFT (`MF_MT_FRAME_RATE`)

**Les deux cibles du jalon sont atteintes.**

| Grandeur | Jalon 1 | Après §1-4 | **Après §5** | Cible |
|---|---|---|---|---|
| Débit (images décodées) | 29,5-30 i/s | 40,4-44,5 i/s | **62,6 i/s** | ≥ 55 i/s |
| Latence médiane | 275,95 ms | 60,3 ms | **47,9 ms** | < 50 ms |
| Latence min | 54,1 ms | 40,6 ms | **43,6 ms** | — |

### Ce que la section « Ce qui reste » concluait, et pourquoi c'était faux

Elle affirmait : « le goulot est maintenant l'encodeur pour de bon :
`produced_hz` plafonne à ~47,5 alors que la capture fournit 68,5 ». Le constat
était juste, l'imputation encore une fois non — pour la même raison que les
trois rondes d'avant : un plafond constaté sur l'encodeur avait été attribué à
l'encodeur.

Deux mesures ont suffi à l'écarter :

- **Le fil n'est occupé qu'à ~2 %** (`capture_pct=0,91`, `submit_pct=0,66`,
  `drain_pct=0,36`, `enc_out_pct=0,013`). Rien ne sature ; un étage qui
  plafonne sans consommer de temps *attend*, il ne peine pas.
- **`need_input_hz` suit `desktop_updates_hz`** proportionnellement
  (68,5 → 47,5 · 59 → 41,5 · 52 → 36,5, ratio constant ≈ 0,70). Un encodeur
  réellement saturé garderait une valeur plate quand la source ralentit. Celui-ci
  ne réclamait que ce qu'on lui donnait : une boucle fermée, pas un plafond.

### La mesure qui a tranché : le bilan matière

Les compteurs de cadence ne bouclaient pas — 68,5 images capturées par seconde
pour 47,5 encodées et 12,5 déclarées périmées, soit **8,5 disparues sans
trace**. Instrumenter les trois issues du convertisseur (`conv_in_hz`,
`conv_out_hz`, `conv_skipped_hz`) a fermé le compte :

```
captured 66 → conv_in 62 (4 refusés) → conv_out 60 → 12,5 périmées + 47,5 encodées = 60 ✓
```

Et a révélé l'anomalie : **`conv_out_hz` valait exactement le `fps` annoncé**,
dans tous les essais (60 → 60,0 · 90 → 90,0 · 120 → 126). Un convertisseur de
format n'a aucune raison d'avoir une cadence propre.

### Cause racine

Le Video Processor MFT est aussi un **convertisseur de cadence**. Le
`MF_MT_FRAME_RATE` que `configure_input`/`create_color_converter` lui
annonçaient — 60/1, codé en dur dans `WindowsSource::new(hwnd, 60, bitrate)` —
n'était pas lu comme une indication mais comme une **cadence de sortie à
tenir** : il produisait 60 échantillons par seconde quoi qu'il arrive, en
rejouant la dernière image convertie quand rien de neuf ne lui parvenait, et
en refusant les entrées excédentaires.

Un bureau à 68,5 Hz passait donc dans un entonnoir à 60, dont l'aval jetait
encore les rejeux : 47,5 images/s à l'encodeur, ~44 i/s au navigateur.

Le `60` n'avait jamais été un choix : c'était la cadence cible du jalon,
recopiée dans un champ qui ne voulait pas dire ça.

### Correctif et balayage

`ENCODER_FPS`, défaut **90** :

| `fps` annoncé | navigateur | `produced_hz` | images neuves (`conv_in_hz`) |
|---|---|---|---|
| 60 (avant) | 44,3 i/s | 47,5 | 62-64 |
| 75 | 53,1 i/s | 55 | 60 |
| **90** | **58,5-62,6 i/s** | **63-68** | **62** |
| 120 | 63,0 i/s | 65,5 | 54 |

90 est l'optimum, et 120 montre pourquoi il ne faut pas monter plus haut : le
débit continue de croître, mais les images **neuves** retombent de 62 à 54/s —
le convertisseur, occupé à tenir sa cadence déclarée, refuse davantage
d'entrées. Au-delà de 90 on n'achète plus que du rejeu.

À 90, `produced_hz = captured_hz = desktop_updates_hz = 68` : le pipeline est
devenu transparent, il ne perd plus une seule image.

### Deux hypothèses réfutées en chemin

Consignées parce qu'elles ont coûté un essai chacune, et qu'elles reviendront :

- **La cadence de sondage.** Passer `FRAME_INTERVAL` de 10 ms à 2 ms
  (`ticks_hz` 100 → 500) laisse `produced_hz` à 47,5, au dixième près. La
  constante reste à 10 ms.
- **La profondeur de la file NV12.** `MAX_PENDING_NV12` porté à 4 ne rend que
  ~3 images/s (47,5 → 51). Reste à 1, la valeur la moins coûteuse en latence.

## Ce qui reste

- **Rejeu résiduel du convertisseur** : à 90, `conv_out_hz = 90` pour
  `conv_in_hz ≈ 62` — environ 28 sorties/s sont des rejeux de la dernière
  image. `dropped_stale` en absorbe l'essentiel (22/s) et le résultat mesuré
  côté navigateur est bon, mais le mécanisme reste un contournement : la
  vraie correction serait un chemin BGRA→NV12 sans conversion de cadence.
- **Compromis qualité non mesuré** : à débit binaire constant (12 Mbps), 62 i/s
  au lieu de 44 signifie moins de bits par image. Aucune mesure de qualité
  perçue n'a été faite ; `BITRATE` est désormais réglable par l'environnement.
- **Une valeur de latence aberrante** : 215,4 ms sur 8 essais (les 7 autres
  entre 43,6 et 66,7 ms), sans gel détecté. Isolée, non expliquée.
- **`playoutDelayHint` / `jitterBufferTarget`** ne sont toujours pas
  positionnés côté client — levier identifié par la recette, jamais exploité :
  la médiane est passée sous la cible sans y toucher.
- **Fragilité du harnais de mesure** : plusieurs essais ont échoué faute de
  focus sur la fenêtre Firefox (vol de focus par `schtasks /it`). Les
  mesures retenues sont celles où la vidéo coulait ; `verify-webrtc.mjs`
  attend désormais explicitement la première image décodée avant d'ouvrir sa
  fenêtre de mesure, au lieu de chronométrer à travers la négociation.

## Instrumentation ajoutée en §5

`SOURCE_TRACE=1` journalise en plus, toutes les 2 s :

- `conv_in_hz` / `conv_out_hz` / `conv_skipped_hz` — le bilan matière du
  convertisseur, sans lequel le compte des images ne bouclait pas ;
- `capture_pct` / `submit_pct` / `drain_pct` / `convert_pct` / `enc_in_pct` /
  `enc_out_pct` — la part de la fenêtre d'observation réellement passée dans
  chaque appel. C'est ce qui distingue un étage qui sature d'un étage qui
  attend, et c'est la mesure qui manquait aux quatre rondes précédentes pour
  cesser d'accuser le matériel.

Variables d'environnement de réglage : `ENCODER_FPS` (défaut 90) et `BITRATE`
(défaut 12 Mbps), toutes deux relayées par `scripts/run-agent.sh`.
