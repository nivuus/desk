# Chantier A — Audio : recette et résultats

**Date d'exécution** : 28 juillet 2026
**Navigateur (harnais de recette automatique, §1-§2, CDP sans interface)** :
Google Chrome 150.0.7871.181 (build officiel, 64 bits),
`HeadlessChrome/150.0.0.0`, relevé par `google-chrome --version` et
`navigator.userAgent` — pas supposé. **Plateforme** : Debian GNU/Linux 13
(trixie), noyau 6.12.96+deb13-amd64, x86_64 — la machine de développement
(hôte du dépôt), **pas** un poste client réel ni un Chromebook.
**Navigateur et plateforme de la mesure d'écoute humaine (§3)** : **ChromeOS,
sur Chromebook, en WiFi** — la plateforme cliente privilégiée du cadrage
(§6.1) et celle du spike multi-fenêtres
(`2026-07-28-spike-multifenetres-resultats.md`). Les deux mesures de ce
rapport ne portent donc **pas** sur la même plateforme cliente ; voir
« Réserves » pour ce que ça implique.
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

**Le son passe, il est audible, et il est synchrone avec la vidéo.** La
chaîne WASAPI loopback → assemblage de trames → encodage Opus → tampon
circulaire → transport WebRTC → décodage navigateur est prouvée par des
compteurs `getStats()` qui **augmentent** pendant la lecture d'un son connu
et **ne perdent aucun paquet**, avec un silence prolongé (63 s mesurées en
continu) qui ne produit ni coupure, ni dérive, ni resynchronisation brutale.
Le contenu perceptuel du signal — ce que `getStats()` ne peut pas prouver à
lui seul — est confirmé par écoute humaine (§3) : son audible et
intelligible, décalage A/V mesuré à **−10 ms** (audio en retard), très en
deçà du seuil de gêne applicable (125 ms, ITU-R BT.1359).

---

## 1. Tableau des mesures

| # | Mesure | Valeur obtenue | Attendu (brief) | Verdict |
|---|---|---|---|---|
| 1 | Son audible et intelligible | **oui**, confirmé par écoute humaine (voir §3) — méthode par annulation sur une mire flash+clic, décrite en §3 | oui | Atteint |
| 2 | Débit audio pendant lecture (Windows Ding en boucle, fenêtre de 20 s) | Δ bytes = 278 365 → **111,3 kb/s** ; Δ packets = 2002 → 100,1 paquets/s | ~130 kb/s | Atteint (écart de 13 %, expliqué ci-dessous) |
| 3 | Débit audio en silence (DTX, fenêtre continue de 63,2 s) | Δ bytes = 6623 → **0,84 kb/s** ; Δ packets = 6315 → 99,9 paquets/s, soit **1,05 octet/paquet en moyenne** | quelques kb/s | Atteint (mieux que l'estimation) — recoupe la convergence DTX à 1 octet/trame mesurée en tâche 2 (`agent/src/opus.rs`, test `le_silence_prolonge_retombe_a_quelques_octets_par_trame`) |
| 4 | Pertes de paquets | **0** sur tous les essais (silence 63 s + relance sonore + essais avec son) | nulles/marginales sur LAN | Atteint |
| 5 | Gigue (`jitter` `getStats()`) | 1–3 ms sur tous les essais | faible attendu sur LAN | Atteint |
| 6 | Décalage A/V, mesure humaine par annulation (§3) | **−10 ms** (audio en **retard** de 10 ms sur la vidéo — voir §3 pour la lecture du signe et la méthode) | < 45 ms avance, < 125 ms retard (ITU-R BT.1359) | **Atteint** — un douzième du seuil de gêne applicable (retard), et sous la durée d'une image à 60 i/s (16,7 ms). C'est la vérification objective qui manquait à la correction du `wallclock` de la tâche 7 ; `estimatedPlayoutTimestamp` n'a pas pu la fournir (clé absente de `getStats()` sur le Chrome/plateforme de mon propre harnais automatisé, voir §2 et Réserves) |
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

## 3. Écoute humaine — faite, par le partenaire humain

**Je n'ai pas pu entendre le son moi-même** — la preuve automatique du §2
établit que des paquets Opus arrivent et sont décodés en continu, sans
perte, avec un volume cohérent avec la présence ou l'absence de son côté VM,
mais elle ne prouve pas à elle seule que le signal porte le bon contenu.
C'est le partenaire humain qui a conduit le test d'écoute, avec en prime une
mesure de décalage A/V que ma propre chaîne de mesure automatisée ne
pouvait pas fournir (§1, mesure #6 : `estimatedPlayoutTimestamp` absent de
`getStats()` sur le Chrome de mon harnais).

**Résultat : le son est audible et intelligible.** Confirmé humainement,
sans coupure ni artefact grossier.

**Méthode — par annulation, pas par jugement direct** : une mire affichée
par Firefox sur la VM émet un flash visuel et un clic sonore au **même
instant**, une fois par seconde, avec une barre qui balaie et atteint un
repère pile au battement (l'anticipation du repère affine nettement le
jugement par rapport à un flash surprise). Les flèches du clavier,
transmises par le canal d'entrée jusqu'à la VM, décalent le son **à la
source** par pas de 5 ms. L'observateur ajuste jusqu'à la coïncidence
perçue et lit la valeur appliquée.

**Résultat chiffré : −10 ms.** Une valeur négative signifie qu'il a fallu
**avancer** le son pour atteindre la coïncidence perçue ⇒ **l'audio arrivait
10 ms en retard sur la vidéo**, avant compensation.

**Ce que ça établit** : le seuil de gêne ITU-R BT.1359 pour un audio en
retard est de 125 ms. 10 ms, c'est un douzième de ce seuil, et c'est sous la
durée d'une image à 60 i/s (16,7 ms). **C'est la vérification objective qui
manquait à la correction du `wallclock` de la tâche 7** (annoncer l'instant
de capture plutôt que d'écriture comme référence RTCP) — celle que
`estimatedPlayoutTimestamp` n'a pas pu fournir sur mon propre harnais. Voir
mesure #6 du tableau (§1).

**Plateforme du client — relevée, pas supposée** : **ChromeOS, sur
Chromebook, en WiFi.** Deux conséquences directes :

- **C'est la plateforme cliente privilégiée du cadrage** (`2026-07-28-support-jeux-design.md`
  §6.1), et c'est aussi celle sur laquelle le spike multi-fenêtres a été
  mesuré. La mesure porte donc **directement sur la cible**, pas sur une
  approximation — contrairement à mes propres mesures automatisées du §1-§2,
  prises sur l'hôte de développement (Debian Linux). Mais l'inférence ne va
  pas dans l'autre sens : rien ici ne dit ce que donnerait Chrome desktop
  sous Windows, macOS ou Linux (le spike multi-fenêtres avait déjà écrit
  cette même réserve, pour le même type de résultat).
- **Les conditions réseau diffèrent de celles du harnais automatique, et pas
  dans le sens d'un affaiblissement du résultat.** Le harnais (débit,
  pertes, gigue, continuité sur 63 s, §1-§2) tournait en Chrome sans
  interface **sur l'hôte lui-même** : le média ne traversait qu'un pont
  virtuel entre l'hôte et la VM, sans lien radio ni FAI. La mesure d'écoute,
  elle, est passée par un Chromebook **en WiFi** — un vrai lien radio
  jusqu'à l'hôte, puis vers la VM — ainsi que par Pomerium en `wss://` pour
  la page et le signaling (origine unique), le média restant en UDP
  **direct** vers `192.168.3.2`, sans passer par le proxy. Le décalage de
  10 ms a donc été obtenu sur un chemin **plus représentatif** de l'usage
  réel que celui de mes propres mesures automatiques, pas moins.

**Réserves sur cette mesure, sans les adoucir** :

- **Un seul observateur, une seule session, aucune répétition.** Le pas de
  réglage étant de 5 ms, la précision ne dépasse pas ±5 ms.
- **La mesure inclut la mire elle-même.** Rien n'a vérifié indépendamment
  que Firefox, sur la VM, restitue son flash et son clic exactement au même
  instant. L'écart propre au navigateur source est donc compté dans les
  10 ms, et le décalage imputable au transport seul pourrait différer de
  quelques millisecondes.

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

**Résultat : l'activation réussit.** C'est une partie seulement de ce que la
spec §11 (sonde n°4) demande de vérifier — « si l'activation réussit **et**
si des données arrivent » — et seule la première moitié est établie ici.
**Ce qui est acquis** : Windows accepte d'activer une interface
`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` pour un PID donné, sur ce
build (20348) — l'hypothèse d'une indisponibilité de l'API sur cette VM est
écartée. **Ce qui reste ouvert** : que le `IAudioClient` obtenu puisse
réellement être initialisé (`Initialize`, `GetService`,
`IAudioCaptureClient::Start`) et qu'un octet de données audio soit
effectivement capturable pour ce processus — `probe_process_loopback` ne
tente aucune de ces quatre étapes ; son `match` final rend `Ok(...)` dès
l'obtention de l'interface, sans jamais l'utiliser (`_client`, préfixé d'un
souligné, n'est ni initialisé ni lu). Rien dans ce chantier n'a construit
plus loin — le périmètre de la tâche 10 est resté une sonde d'activation,
pas une capture.

```
INFO agent: sonde process loopback pid=3620 rapport="activation réussie : IAudioClient
obtenu pour le PID 3620 (VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, INCLUDE_TARGET_PROCESS_TREE)"
```

Reproduit deux fois de suite, avec le PID d'un vrai processus Firefox de la
session interactive, code de sortie de la tâche planifiée **0** les deux
fois, aucun rapport de plantage Windows corrélé — mais dans les deux cas,
seule l'obtention de l'interface a été observée, jamais son utilisation.

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
- **Réseau — trois régimes, un seul jamais testé.** Ne pas laisser croire
  que tout a été mesuré en boucle locale :
  1. **Boucle hôte↔VM (harnais automatique, §1-§2)** : débit, pertes,
     gigue, continuité sur 63 s — Chrome sans interface tournant **sur
     l'hôte lui-même**, média traversant seulement le pont virtuel
     hôte↔VM, aucun lien radio ni FAI. Gigue 1–3 ms, 0 perte.
  2. **WiFi local (mesure d'écoute, §3)** : Chromebook réel, sur le réseau
     WiFi local, page et signaling via Pomerium en `wss://`, média en UDP
     direct vers la VM. C'est sur ce chemin, plus représentatif que le
     précédent, qu'a été obtenu le décalage de −10 ms.
  3. **Internet quelconque, avec latence/perte réelles** : **jamais
     testé.** Le comportement du FEC in-band, du DTX sous perte, et du
     tampon de gigue du navigateur sous un réseau dégradé reste
     entièrement ouvert — hors périmètre de cette tâche (sonde n°4 et
     livrable du chantier A), du ressort du chantier C (adaptation
     réseau).
- **Décalage A/V — mesuré par l'humain (§3, −10 ms), pas par mon harnais.**
  Ma propre chaîne de mesure automatisée (§1, mesure initiale ; §2) n'a pas
  pu produire ce chiffre : la **clé** `estimatedPlayoutTimestamp` est
  absente de l'entrée `inbound-rtp` sur Chrome 150.0.7871.181 / Debian 13
  Linux headless — vérifié par `Object.keys()` sur les deux pistes, à deux
  relevés espacés de 6 s, pas seulement déduit d'une valeur
  `null`/`undefined`. Une clé structurellement absente, sur les deux
  pistes et de façon répétée, pointe plus fortement vers « champ non
  implémenté par ce Chrome/cette plateforme » que vers « champ implémenté
  mais qui attend un RTCP Sender Report pour se peupler » (dans ce second
  cas, la clé existerait déjà dans l'objet, à `undefined`, en attendant sa
  valeur). **Cela dit, je ne peux pas exclure avec certitude absolue une
  troisième explication non testée** : Chrome `--headless=new` tourne sans
  périphérique de sortie audio réel, et je n'ai pas vérifié si
  l'estimation de restitution dépend d'un périphérique de sortie
  effectivement ouvert plutôt que simplement d'un Sender Report reçu. Le
  code du harnais reste écrit conformément au brief (il fonctionnera
  dès que/si ce champ devient disponible sur le navigateur utilisé) et
  gère l'absence proprement. La mesure humaine du §3 comble ce trou par
  une méthode indépendante (par annulation, sur une mire), avec ses
  propres réserves (§3 : observateur unique, pas de répétition, ±5 ms,
  écart de la mire elle-même inclus).
- **Plateforme cliente — mixte selon la mesure, à ne pas moyenner.** Les
  mesures automatiques (§1-§2, débit/pertes/gigue/continuité) tournent sur
  l'hôte de développement (Debian Linux), **pas** sur la plateforme
  cliente privilégiée du cadrage. La mesure d'écoute (§3), elle, **est**
  sur cette plateforme : ChromeOS, Chromebook, WiFi — la même que celle du
  spike multi-fenêtres. La conclusion « le son est audible et synchrone »
  porte donc directement sur la cible ; les mesures de débit/pertes/gigue
  ne le font pas, et rien ne garantit qu'elles seraient identiques sur
  Chromebook (bien que rien ne suggère non plus un écart, Opus/WebRTC
  étant standard). Et dans les deux cas, l'inférence ne va pas vers
  Chrome desktop (Windows/macOS/Linux), jamais testé ici — même réserve
  que celle déjà écrite par le spike multi-fenêtres pour son propre
  résultat.
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
