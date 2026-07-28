# Chantier A — Audio : recette et résultats

**Date d'exécution** : 28 juillet 2026
**Navigateur (harnais de recette, CDP sans interface)** : Google Chrome
150.0.7871.181 (build officiel, 64 bits), `HeadlessChrome/150.0.0.0`, relevé
par `google-chrome --version` et `navigator.userAgent` — pas supposé.
**Plateforme du navigateur** : Debian GNU/Linux 13 (trixie), noyau
6.12.96+deb13-amd64, x86_64. C'est la machine de développement (hôte du
dépôt), **pas** un poste client réel ni un Chromebook — à la différence du
spike multi-fenêtres (`2026-07-28-spike-multifenetres-resultats.md`), cette
recette n'a pas été menée depuis la plateforme cliente privilégiée du
cadrage. Voir « Réserves ».
**Plateforme agent** : VM Windows (build 20348), GPU NVIDIA RTX 4070,
adaptateur d'affichage virtuel « SudoMaker Virtual Display Adapter ».
**Périphérique de rendu audio et format de mixage** (sonde de la tâche 5,
reconfirmés à chaque session de cette tâche) : deux endpoints de rendu
actifs (`Get-CimInstance Win32_SoundDevice` / `Get-PnpDevice -Class
AudioEndpoint`, état `OK` sur les deux) — « Haut-parleurs (Steam Streaming
Speakers) », **virtuel**, et « HDP-V104 (NVIDIA High Definition Audio) ».
Format de mixage WASAPI réellement relevé, systématiquement identique sur
toutes les sessions : **48 000 Hz, 2 canaux, 32 bits flottant**.

---

## Verdict

**Le son passe.** La chaîne WASAPI loopback → assemblage de trames →
encodage Opus → tampon circulaire → transport WebRTC → décodage navigateur
est prouvée par des compteurs `getStats()` qui **augmentent** pendant la
lecture d'un son connu et **ne perdent aucun paquet**, avec un silence
prolongé (63 s mesurées en continu) qui ne produit ni coupure, ni dérive, ni
resynchronisation brutale.

**Ce que cette mesure ne prouve pas** : que le signal reçu est
perceptuellement le bon son (intelligible, sans bruit) plutôt qu'un flux de
la bonne taille par coïncidence. C'est la limite explicite de toute preuve
fondée sur `getStats()` — voir « À confirmer par écoute humaine ».

---

## 1. Tableau des mesures

| # | Mesure | Valeur obtenue | Attendu (brief) | Verdict |
|---|---|---|---|---|
| 1 | Son audible et intelligible | **non mesuré par cet agent** — voir section dédiée | oui | ⚠️ À CONFIRMER PAR ÉCOUTE HUMAINE |
| 2 | Débit audio pendant lecture (Windows Ding en boucle, fenêtre de 20 s) | Δ bytes = 278 365 → **111,3 kb/s** ; Δ packets = 2002 → 100,1 paquets/s | ~130 kb/s | Atteint (écart de 13 %, expliqué ci-dessous) |
| 3 | Débit audio en silence (DTX, fenêtre continue de 63,2 s) | Δ bytes = 6623 → **0,84 kb/s** ; Δ packets = 6315 → 99,9 paquets/s, soit **1,05 octet/paquet en moyenne** | quelques kb/s | Atteint (mieux que l'estimation) — recoupe la convergence DTX à 1 octet/trame mesurée en tâche 2 (`agent/src/opus.rs`, test `le_silence_prolonge_retombe_a_quelques_octets_par_trame`) |
| 4 | Pertes de paquets | **0** sur tous les essais (silence 63 s + relance sonore + essais avec son) | nulles/marginales sur LAN | Atteint |
| 5 | Gigue (`jitter` `getStats()`) | 1–3 ms sur tous les essais | faible attendu sur LAN | Atteint |
| 6 | Décalage A/V (`estimatedPlayoutTimestamp` audio − vidéo) | **non mesurable** : `estimatedPlayoutTimestamp` n'existe pas comme **clé** dans l'entrée `inbound-rtp`, ni côté audio ni côté vidéo — vérifié par `Object.keys()` sur les deux, pas seulement déduit d'une valeur `null`/`undefined` (voir §2) | < 45 ms avance, < 125 ms retard (ITU-R BT.1359) | NON MESURÉ — la clé absente (et non présente-mais-vide) pointe vers un champ non implémenté par ce Chrome/cette plateforme, plutôt que vers un défaut de la correction de la tâche 7. Voir « Réserves » pour ce qui reste néanmoins incertain |
| 7 | Comportement après 60 s de silence | Continu sur 63,2 s : 0 perte, cadence paquets stable à 99,9–100,2/s (10 ms par trame, sans à-coup), transition vers le son suivante sans coupure ni resynchronisation (bytes/packets progressent sans discontinuité au moment du son) | pas de coupure/dérive/resynchro | Atteint |

Le débit en jeu (mesure #2) a nécessité une deuxième tentative : la première
(boucle de 15 sons, ~15 s) s'était **déjà tue** avant le premier relevé du
harnais, qui attend jusqu'à 20 s (vidéo qui ne progressait pas dans cet essai)
+ 1,5 s avant de prendre sa première mesure. Voir « Ce que la mesure a coûté »
pour le détail — sans cette relecture, le rapport aurait affiché à tort un
débit « en jeu » quasi identique au silence.

**Sur l'écart de 13 % entre 111,3 kb/s mesurés et 128 kb/s visés (mesure #2)**
— hypothèse étayée, pas certaine : `agent/src/opus.rs` appelle
`set_bitrate(Bitrate::Bits(128_000))` mais jamais `set_vbr(false)` ; libopus
reste donc en **débit variable** par défaut, et 128 kb/s n'est qu'une cible
*moyenne*, pas une taille de trame garantie. 278 365 octets / 2002 paquets =
**139 octets/paquet** mesurés, contre ~160 attendus pour une trame pleine à
128 kb/s (commentaire de `MAX_PACKET_BYTES` dans `opus.rs` : 10 ms à
128 kb/s ≈ 160 octets). En notant `x` la fraction de trames DTX (~1 octet,
mesure #3) sur la fenêtre de 20 s : `160·(1−x) + 1·x = 139` donne `x ≈ 13 %`
— cohérent avec une boucle de sons Windows Ding entrecoupée de silences
(chaque `PlaySync()` dure ~1 s, suivi d'un court silence avant la relance
suivante). Cette arithmétique n'a pas été vérifiée indépendamment (pas de
mesure directe de la proportion réelle de trames DTX sur cette fenêtre
précise) ; elle est présentée comme l'hypothèse la plus cohérente avec les
données disponibles, pas comme un fait établi.

## 2. Preuve automatique (harnais `client/verify-webrtc.mjs` étendu)

Deux extraits bruts, représentatifs des essais menés.

**Extrait 1** — fenêtre de 20 s, son en boucle sur la VM, avant la revue
(ancienne sortie, message de décalage A/V non encore distingué) :

```
--- Relevé 1 ---
  [audio] bytesReceived=300121 packetsReceived=2153 packetsLost=0 jitter=2.0ms estimatedPlayoutTimestamp=absent
--- Relevé 2 (+20000ms) ---
  [audio] bytesReceived=578486 packetsReceived=4155 packetsLost=0 jitter=3.0ms estimatedPlayoutTimestamp=absent

Δ audio bytesReceived sur 20000ms : 278365
Δ audio packetsReceived sur 20000ms : 2002
audio : bytesReceived et packetsReceived augmentent — PREUVE que l'audio traverse la chaîne.
décalage A/V : non mesurable (pas de piste audio active sur ce relevé).
```

**Extrait 2** — après la revue, harnais corrigé (message de décalage A/V
distingué, diagnostic `Object.keys()` demandé en revue), fenêtre de 6 s,
même agent :

```
--- Relevé 1 ---
  framesDecoded=2 framesReceived=2 frameWidth=764 frameHeight=484 bytesReceived=53308 packetsReceived=50 packetsLost=0 keyFramesDecoded=2
  [vidéo] estimatedPlayoutTimestamp : CLÉ ABSENTE de l'entrée
  [vidéo] clés de l'entrée getStats() : bytesReceived, codecId, firCount, frameHeight, frameWidth, framesAssembledFromMultiplePackets, framesDecoded, framesDropped, framesReceived, freezeCount, headerBytesReceived, id, jitter, jitterBufferDelay, jitterBufferEmittedCount, jitterBufferMinimumDelay, jitterBufferTargetDelay, keyFramesDecoded, kind, lastPacketReceivedTimestamp, mediaType, mid, nackCount, packetsLost, packetsReceived, packetsReceivedWithCe, packetsReceivedWithEct1, pauseCount, pliCount, qpSum, remoteId, rtxSsrc, ssrc, timestamp, totalAssemblyTime, totalDecodeTime, totalFreezesDuration, totalInterFrameDelay, totalPausesDuration, totalProcessingDelay, totalSquaredInterFrameDelay, trackIdentifier, transportId, type
  [audio] bytesReceived=2254 packetsReceived=2150 packetsLost=0 jitter=2.0ms
  [audio] estimatedPlayoutTimestamp : CLÉ ABSENTE de l'entrée
  [audio] clés de l'entrée getStats() : audioLevel, bytesReceived, codecId, concealedSamples, concealmentEvents, fecPacketsDiscarded, fecPacketsReceived, headerBytesReceived, id, insertedSamplesForDeceleration, jitter, jitterBufferDelay, jitterBufferEmittedCount, jitterBufferMinimumDelay, jitterBufferTargetDelay, kind, lastPacketReceivedTimestamp, mediaType, mid, packetsDiscarded, packetsLost, packetsReceived, packetsReceivedWithCe, packetsReceivedWithEct1, playoutId, remoteId, removedSamplesForAcceleration, silentConcealedSamples, ssrc, timestamp, totalAudioEnergy, totalProcessingDelay, totalSamplesDuration, totalSamplesReceived, trackIdentifier, transportId, type
--- Relevé 2 (+6000ms) ---
  [audio] bytesReceived=2887 packetsReceived=2753 packetsLost=0 jitter=1.0ms

Δ audio bytesReceived sur 6000ms : 633
Δ audio packetsReceived sur 6000ms : 603
audio : bytesReceived et packetsReceived augmentent — PREUVE que l'audio traverse la chaîne.
décalage A/V : non mesurable — la piste audio traverse (voir ci-dessus), mais `estimatedPlayoutTimestamp` est absent de getStats() pour l'une des deux pistes sur ce navigateur/cette plateforme (voir les clés listées dans chaque relevé).
```

**Ce que ce second extrait établit** : la clé `estimatedPlayoutTimestamp` est
**absente de l'objet lui-même** (`'estimatedPlayoutTimestamp' in entry` vaut
`false`), pas seulement présente à `null`/`undefined` — vérifié pour les deux
pistes, sur deux relevés consécutifs de la même session. C'est la distinction
que `a.estimatedPlayoutTimestamp ?? 'absent'` (extrait 1) ne permettait pas de
faire, et qui rendait la conclusion « limite du navigateur » non gagnée : une
clé présente mais vide aurait pu signifier que le champ existe bien dans
l'implémentation mais attend un événement (par exemple un RTCP Sender
Report) pour se peupler — ce qui aurait rouvert la question d'un lien avec la
correction de la tâche 7. Une clé structurellement absente de l'objet retourné
par `getStats()`, sur les deux pistes et sur deux relevés espacés de 6 s,
pointe plus fortement vers un champ non implémenté par ce Chrome/cette
plateforme — sans trancher la question avec une certitude totale (voir
« Réserves »).

Trace de continuité sur 63 s de silence pur (échantillonnage toutes les 9 s,
même session, sans interruption ni redémarrage d'agent) :

```
t=0.0s  {"bytesReceived":95,   "packetsReceived":89,   "packetsLost":0,"jitter":0.002}
t=9.0s  {"bytesReceived":1041, "packetsReceived":991,  "packetsLost":0,"jitter":0.002}
t=18.0s {"bytesReceived":1987, "packetsReceived":1893, "packetsLost":0,"jitter":0.001}
t=27.1s {"bytesReceived":2933, "packetsReceived":2795, "packetsLost":0,"jitter":0.001}
t=36.1s {"bytesReceived":3880, "packetsReceived":3698, "packetsLost":0,"jitter":0.002}
t=45.1s {"bytesReceived":4828, "packetsReceived":4602, "packetsLost":0,"jitter":0.002}
t=54.2s {"bytesReceived":5774, "packetsReceived":5504, "packetsLost":0,"jitter":0.002}
t=63.2s {"bytesReceived":6718, "packetsReceived":6404, "packetsLost":0,"jitter":0.001}
>>> déclenchement du son sur la VM
t=final (+son) : {"bytesReceived":22749,"packetsReceived":7509,"packetsLost":0,"jitter":0.001}
```

L'incrément entre chaque paire de relevés (~946 octets et ~902 paquets par
tranche de 9 s) est remarquablement stable — c'est la signature attendue du
complément de silence de `frames.rs` : aucune trame n'est jamais sautée,
DTX réduit seulement la taille de chaque trame, pas leur cadence. La
transition au son (dernier relevé) montre une augmentation nette de la taille
moyenne par paquet (~14,5 octets/paquet contre ~1,05 en silence) sans aucune
perte ni discontinuité de compteur.

### Garde-fous du harnais, vérifiés

- **Le harnais échoue quand l'audio a été vu puis s'arrête de progresser** :
  logique implémentée dans `client/verify-webrtc.mjs` (`audioGrowing` exigé
  quand `!audioAbsent`), non reproduite par un essai réel dans cette
  recette (aurait exigé une panne injectée, jugé disproportionné) mais
  vérifiée par relecture du code.
- **Le harnais reste vert sur une session vidéo seule** : vérifié en
  conditions réelles avec `TEST_FILE=C:\dev\agent\testdata\testsrc.264`
  (voir §4 « Dégradation ») — code de sortie **0**, `audio : aucun octet
  reçu sur les deux relevés ... session vidéo seule` imprimé, aucune ligne
  d'échec liée à l'audio.
- **`EXPECT_AUDIO=1` (opt-in ajouté en revue)** : par défaut, l'absence
  totale d'audio n'est jamais un échec (point précédent) — un choix
  nécessaire pour la recette vidéo pure, mais qui laisserait le harnais
  vert sur une régression qui couperait tout l'audio d'une session censée
  en avoir. Vérifié dans les deux sens, en conditions réelles :
  - `EXPECT_AUDIO=1` sur une session `TEST_FILE` (vidéo normale, aucun
    audio) → **code de sortie 1**, `ÉCHEC (audio) : EXPECT_AUDIO=1 mais
    aucune piste audio n'a été vue sur les deux relevés.` ;
  - sans `EXPECT_AUDIO`, la même session → **code de sortie 0**, comme
    attendu.
  - Le chemin « `EXPECT_AUDIO=1` et audio réellement présent » n'a pas été
    revérifié par un essai où la vidéo décodait aussi (Firefox statique
    dans les essais audio de cette recette, cf. réserves) : `audioGrowing`
    a bien été observé à `true` sous `EXPECT_AUDIO=1` dans un essai où
    seule la vidéo échouait, ce qui confirme que ce chemin ne déclenche pas
    l'échec « audio attendu et absent » — mais le code de sortie global
    n'a pas pu être confirmé à 0 sur ce cas précis faute de vidéo qui
    coule pendant un essai audio. Vérifié par relecture du code
    (`audioAbsent && expectAudio` est le seul nouveau chemin d'échec,
    disjoint de `audioGrowing`).

## 3. À confirmer par écoute humaine

**Je n'ai pas pu entendre le son.** La preuve automatique établit que des
paquets Opus arrivent, sont décodés en continu, sans perte, et que leur
volume varie de façon cohérente avec la présence ou l'absence de son côté
VM — mais elle ne prouve pas que le signal porte le bon contenu plutôt que
du bruit ou un silence numérique de la bonne taille par coïncidence.

**Reste à faire, avec un humain** :

1. Ouvrir le client (`http://localhost:5174/?session=demo`, avec le
   signaling — `cd signaling && npm start` — et l'agent —
   `WINDOW_TITLE=firefox scripts/run-agent.sh` — actifs ; signaling et
   client laissés démarrés sur cette machine à l'issue de cette tâche,
   ports 8080 et 5174) et cliquer une fois dans la page pour lever la
   politique d'autoplay.
2. Jouer un son connu sur la VM : `(New-Object Media.SoundPlayer
   'C:\Windows\Media\Windows Ding.wav').PlaySync()`.
3. Confirmer que le son est audible, sans coupure ni artefact grossier
   (métallique, haché), et reconnaissable comme un « ding » plutôt que du
   bruit.
4. Si possible, confirmer subjectivement que le son perçu est synchrone
   avec une action visible à l'écran (utile puisque la mesure automatique du
   décalage A/V — §1, mesure #6 — n'a pas pu être obtenue sur ce navigateur).

## 4. Dégradation : session sans audio

`TEST_FILE=C:\dev\agent\testdata\testsrc.264 scripts/run-agent.sh`, puis
harnais sur la même session :

```
--- Relevé 1 ---
  framesDecoded=2146 framesReceived=2147 frameWidth=1280 frameHeight=720 ...
  aucune entrée inbound-rtp audio dans getStats()
--- Relevé 2 (+6000ms) ---
  framesDecoded=2748 framesReceived=2749 frameWidth=1280 frameHeight=720 ...
  aucune entrée inbound-rtp audio dans getStats()

Δ framesDecoded sur 6000ms : 602
audio : aucun octet reçu sur les deux relevés (pas de piste audio active — session vidéo seule, ou silence total).
PREUVE : la vidéo traverse la chaîne (framesDecoded et framesReceived augmentent, dimensions plausibles).
```

Code de sortie du harnais : **0**. Vidéo normale (602 images décodées en
6 s), aucune piste audio (`windows_audio::WindowsAudioSource` n'est jamais
construite quand `config.test_file.is_some()`, cf. `agent/src/main.rs`),
aucune erreur fatale dans `agent.log`. Conforme à l'attendu de l'étape 8 du
brief.

## 5. Sonde process loopback (spec §11, sonde n°4)

**Résultat : activation réussie.** Le risque « la VM est-elle éligible au
process loopback » (build 20348) est **levé**.

```
INFO agent: sonde process loopback pid=3620 rapport="activation réussie : IAudioClient
obtenu pour le PID 3620 (VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, INCLUDE_TARGET_PROCESS_TREE)"
```

Reproduit deux fois de suite, avec le PID d'un vrai processus Firefox de la
session interactive, code de sortie de la tâche planifiée **0** les deux
fois, aucun rapport de plantage Windows corrélé.

Ce résultat n'a été obtenu qu'après correction d'un bogue de **corruption
mémoire** trouvé en revue de code — sans cette correction, le rapport initial
de cette tâche concluait à tort à un résultat « non déterminé ». Le détail
suit, par souci de traçabilité (le bogue et son diagnostic erroné font
partie du résultat, pas seulement le correctif).

### Le bogue trouvé en revue, et pourquoi il expliquait mieux les symptômes que mon hypothèse

`probe_process_loopback` construit un `PROPVARIANT` de type `VT_BLOB` dont
`blob.pBlobData` pointe sur `params`, une **variable de pile**. Or
`PROPVARIANT` implémente `Drop`
(`windows-0.62.2/src/extensions/Win32/System/StructuredStorage.rs:32`,
`fn drop(&mut self) { unsafe { _ = PropVariantClear(self) } }`) : à la sortie
de portée — sur **tout** chemin, y compris le `?` d'
`ActivateAudioInterfaceAsync` juste en dessous — `PropVariantClear` appelle
`CoTaskMemFree` sur l'adresse de `params`, une adresse de **pile**, pas une
allocation `CoTaskMemAlloc`. Comportement indéfini franc.

Mon hypothèse initiale (rapport avant revue) attribuait le silence de la
sonde à un problème de vidage du pipeline `Tee-Object`/tâche planifiée — une
explication plausible en soi, mais qui déplaçait le soupçon de mon code vers
l'outillage sans jamais relire le code de la sonde elle-même. La corruption
mémoire explique **mieux** les symptômes observés : elle survient **à
l'intérieur** de `probe_process_loopback`, donc **avant** tout appel
`tracing::info!`/`tracing::warn!` de la fonction — ce qui correspond très
exactement à « aucune ligne n'atteint `agent.log`, ni succès ni échec ». Et
les deux plantages historiques `0xc0000005` dans **`ntdll.dll`**, que le
rapport initial écartait comme sans rapport (horodatages ne correspondant à
aucune tentative datée), sont la signature canonique d'une libération de tas
invalide — ils corroborent cette explication plutôt qu'ils ne l'infirment,
même si je n'ai pas de preuve que ce sont ces plantages précis qui
correspondaient à mes propres essais.

**Correctif** : envelopper la valeur dans `std::mem::ManuallyDrop` pour
empêcher ce `Drop` de s'exécuter — rien n'a besoin d'être libéré, `blob` ne
référence aucune mémoire dont ce `PROPVARIANT` est propriétaire.

```rust
let mut propriete = std::mem::ManuallyDrop::new(PROPVARIANT::default());
(*propriete.Anonymous.Anonymous).vt = VT_BLOB;
(*propriete.Anonymous.Anonymous).Anonymous.blob = BLOB { ... };
// ...
ActivateAudioInterfaceAsync(..., Some(&*propriete), ...)
```

Après ce correctif : recompilé (`scripts/build-agent.sh`, `Finished
release`), sonde relancée deux fois — voir le résultat en tête de section.

### Ce qui a été fait (compilation)

- La sonde a été implémentée dans `agent/src/wasapi.rs`
  (`probe_process_loopback`) et câblée dans `agent/src/main.rs`
  (`PROCESS_LOOPBACK_PROBE`), conformément au brief : activation via
  `ActivateAudioInterfaceAsync` + `AUDIOCLIENT_ACTIVATION_PARAMS`
  (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`,
  `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`), avec un gestionnaire
  de complétion COM (`IActivateAudioInterfaceCompletionHandler`) implémenté
  via la macro `#[windows::core::implement]`, synchronisé par un couple
  `Mutex`/`Condvar` borné à 10 s.
- **Le code compile et lie sur la cible Windows réelle**
  (`scripts/build-agent.sh`, profil `release`), après deux corrections
  d'API découvertes par la compilation elle-même (non trouvables sans
  compiler sur la cible) — une troisième erreur de compilation initialement
  comptée séparément (le message « trait bound `IUnknownImpl` non satisfait »)
  était en réalité une **conséquence** de la première (résolue par le même
  correctif), pas une cause indépendante :
  1. `windows-core` doit être une dépendance **directe** du crate (pas
     seulement transitive via `windows`) — la macro `#[implement]` génère
     du code qui référence `windows_core::...` littéralement, et ce même
     défaut de résolution de nom fait échouer la satisfaction du trait
     `IUnknownImpl` généré par la macro.
  2. L'auto-déréférencement de `ManuallyDrop` n'est pas appliqué
     automatiquement sur un champ d'union COM (`PROPVARIANT.Anonymous`) :
     il faut un `*` explicite (`(*propriete.Anonymous.Anonymous).vt = ...`).
- Le bogue de corruption mémoire ci-dessus n'a **pas** été détecté par la
  compilation : `unsafe` autorise ce genre d'erreur sans avertissement, il a
  fallu une relecture ciblée du code (revue) pour le trouver.

## 6. Réserves — ce qui n'a pas été vérifié

- **Rééchantillonnage** : refusé par conception
  (`agent/src/wasapi.rs::LoopbackCapture::open`, `bail!` si la fréquence de
  mixage n'est pas 48 000 Hz). Jamais mis en défaut dans cette recette : le
  format de mixage a été 48 000 Hz sur toutes les sessions observées, donc
  ce chemin de refus n'a jamais été exercé.
- **Microphone** : hors périmètre du chantier A (capture loopback de rendu
  uniquement, jamais de capture d'entrée). Non testé, non applicable.
- **Réseau non local** : toutes les mesures ont été prises sur le réseau
  virtuel LAN de la VM (gigue 1–3 ms, 0 perte). Le comportement sur un
  réseau à latence/perte réelles (FEC in-band, DTX sous perte, tampon de
  gigue du navigateur) n'a pas été testé — hors périmètre de cette tâche,
  qui répond à la sonde n°4 et produit le livrable, pas au chantier C
  (adaptation réseau).
- **Décalage A/V non mesuré** (voir §1, mesure #6) : la **clé**
  `estimatedPlayoutTimestamp` est absente de l'entrée `inbound-rtp` sur
  Chrome 150.0.7871.181 / Debian 13 Linux headless — vérifié par
  `Object.keys()` sur les deux pistes, à deux relevés espacés de 6 s (§2),
  pas seulement déduit d'une valeur `null`/`undefined`. Une clé
  structurellement absente, sur les deux pistes et de façon répétée, pointe
  plus fortement vers « champ non implémenté par ce Chrome/cette
  plateforme » que vers « champ implémenté mais qui attend un RTCP Sender
  Report pour se peupler » (dans ce second cas, la clé existerait déjà dans
  l'objet, à `undefined`, en attendant sa valeur). **Cela dit, je ne peux
  pas exclure avec certitude absolue une troisième explication non testée** :
  Chrome `--headless=new` tourne sans périphérique de sortie audio réel, et
  je n'ai pas vérifié si l'estimation de restitution dépend d'un
  périphérique de sortie effectivement ouvert plutôt que simplement d'un
  Sender Report reçu. Le code du harnais reste écrit conformément au brief
  (il fonctionnera dès que/si ce champ devient disponible) et gère
  l'absence proprement (affiche « non mesurable », distingue désormais
  « pas de piste » de « piste présente, champ absent », n'échoue jamais
  dessus). La correction de la tâche 7 (`wallclock` = instant de capture)
  n'a donc **pas** pu être vérifiée objectivement par cette méthode
  précise ; elle reste vérifiée indirectement par la continuité et la
  régularité des compteurs (§1, mesures #3 et #7) et par la relecture du
  code de la tâche 7.
- **Plateforme cliente non privilégiée** : contrairement au spike
  multi-fenêtres, cette recette tourne sur l'hôte de développement
  (Debian Linux), pas sur un Chromebook (plateforme cliente privilégiée du
  cadrage, §6.1). Rien ne suggère un écart de comportement audio entre les
  deux (Opus/WebRTC est standard), mais ce n'est pas vérifié ici.
- **Vidéo peu représentative dans les essais purement audio** : avec
  Firefox statique (aucune page en défilement), Desktop Duplication ne
  produit quasiment aucune image neuve (comportement documenté dans
  `agent/src/main.rs`, pas un défaut introduit par ce chantier) — les
  essais audio ci-dessus montrent donc souvent `framesDecoded` figé à 2.
  Contourné pour un essai en ouvrant une page à contenu défilant
  (`client/recette/scroll-test.html`) ; confirmé sain indépendamment par
  l'essai `TEST_FILE` (§4, 602 images décodées en 6 s). N'affecte aucune
  des mesures audio, qui ne dépendent pas de la vidéo.
- **VM instable pendant la recette** : la VM s'est arrêtée spontanément une
  fois en cours de mesure (comportement déjà documenté à plusieurs reprises
  dans ce projet, cause non identifiée). `virsh start Windows` a suffi à la
  relancer (session interactive retrouvée en quelques secondes). Aucune
  mesure conservée dans ce rapport n'a été prise à cheval sur cet incident.

## 7. Ce que la mesure a coûté, et ce qu'elle a évité

- **Faux négatif de timing évité de justesse** : le premier essai de mesure
  du débit « en jeu » a fait jouer une boucle de 15 sons Windows Ding
  (~15 s) *avant* de lancer le harnais, sans tenir compte du fait que le
  harnais attend jusqu'à 20 s que la vidéo « coule » (jamais le cas ici,
  Firefox statique) plus 1,5 s avant son premier relevé — la boucle sonore
  s'était donc déjà tue avant la moindre mesure, produisant un débit « en
  jeu » quasi identique au silence (526 octets/5 s). Si ce chiffre avait été
  retenu tel quel, le rapport aurait conclu à tort que DTX ne distingue pas
  le silence du son. Corrigé en allongeant la boucle sonore à 50 itérations
  (~50 s, recouvrant largement la fenêtre de mesure réelle) : le débit
  mesuré est alors passé de ~0,8 kb/s à ~111 kb/s, cohérent avec l'attendu.
- **Machinerie COM de la sonde process loopback** : ~1 heure de travail
  (recherche d'API dans les sources vendorées de `windows`/`windows-core`,
  deux itérations de correction de compilation, plusieurs tentatives
  d'exécution infructueuses) pour un résultat d'abord consigné comme « non
  déterminé », conformément au seuil d'arrêt du brief.
- **Un bogue de corruption mémoire manqué, trouvé en revue, qui a coûté un
  premier diagnostic faux.** Le premier jet de ce rapport attribuait le
  silence de la sonde à un problème de vidage du pipeline
  `Tee-Object`/tâche planifiée — une hypothèse plausible en apparence, mais
  qui n'avait jamais été confrontée à une relecture du code de la sonde
  elle-même. La cause réelle (§5) : un `PROPVARIANT` dont le `Drop`
  implicite appelle `CoTaskMemFree` sur une adresse de pile, un
  comportement indéfini qui corrompt la mémoire **avant** que la moindre
  ligne de log ne puisse s'écrire — reproduisant exactement le symptôme
  observé. Le correctif (`ManuallyDrop`) tient en une ligne ; le trouver a
  demandé une relecture ciblée, pas plus de compilation ni plus d'essais
  d'exécution, qui en avaient déjà produit plusieurs sans jamais faire
  apparaître la vraie cause. Leçon directement transférable au chantier D,
  qui reprendra ce code : toute construction Rust d'un `PROPVARIANT` (ou de
  tout autre type `windows-rs` porteur d'un `Drop` non trivial) pointant
  vers une donnée dont la durée de vie n'est PAS gérée par ce même type
  mérite une vérification explicite de son implémentation `Drop` avant
  usage — le compilateur ne signale rien, `unsafe` couvre tout, et le
  symptôme (silence total, aucun rapport de plantage cohérent) peut
  ressembler à n'importe quoi d'autre qu'une corruption mémoire.
- **Collision de tâche planifiée découverte et corrigée** : `schtasks
  /create /f ... ; schtasks /run` n'interrompt pas une instance déjà en
  cours (politique par défaut « ne pas démarrer de nouvelle instance ») —
  un agent laissé actif par un essai précédent (typiquement parce que
  Chrome venait d'être fermé côté harnais mais que l'agent n'avait pas
  encore détecté la déconnexion ICE) absorbait silencieusement les demandes
  `/run` suivantes, produisant des relevés `agent.log` obsolètes qui
  semblaient provenir d'une nouvelle session. Corrigé en insérant
  systématiquement `scripts/stop-agent.sh` avant chaque redémarrage
  d'agent — cette précaution n'était pas nécessaire dans la recette du
  jalon 1 (une seule session longue), elle le devient dès qu'un enchaînement
  rapide de sessions courtes est nécessaire, comme dans cette tâche.
