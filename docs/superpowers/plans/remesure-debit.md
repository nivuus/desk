# Remesure du débit de bout en bout — le chiffre de référence de la capture était périmé, le plafond ne l'est pas

## Contexte

Le chiffre de référence « capture ~47 im/s » qui justifiait plusieurs
conclusions antérieures s'est avéré périmé : mesuré à neuf avec
`CAPTURE_TEST`, la capture seule délivre **270 images / 3 s ≈ 90 im/s**
(confirmé deux fois, avec et sans le fil d'agitation de fenêtre — voir plus
bas). Mission : remesurer honnêtement le débit de bout en bout, puis, s'il
reste très en dessous de 60 im/s, localiser le goulot sans supposer.

## Un — remesure honnête, chaîne complète

Chaîne montée : signaling (`ws://192.168.3.1:8080`), client Vite
(`http://localhost:5173`), agent sur la VM via `run-agent.sh` (jamais par
WinRM direct), harnais `client/verify-webrtc.mjs` (Chrome sans interface,
compteurs `RTCPeerConnection.getStats()` réels). Contenu : Firefox en
session 1, `C:\dev\anim.html` (`requestAnimationFrame`), fenêtre reprise au
premier plan par un script de focus forcé juste avant chaque mesure — l'agent
lui-même est mono-session (le process se termine après chaque session
`Session::run`, voir `main.rs`), donc chaque mesure exige un redémarrage
d'agent, ce qui donne naturellement une fenêtre propre entre le focus et la
mesure (pas de chiffre bas dû à un lancement de tâche planifiée trop récent).

**Résultat, trois essais indépendants, agent redémarré à chaque fois :**

| Durée | Trames décodées | Débit | Pertes |
|---|---|---|---|
| 10 s | 295 | 29,5 im/s | 0 |
| 15 s | 445 | 29,67 im/s | 0 |
| 20 s | 595 | 29,75 im/s | 0 |

Dimensions : 784×632 (taille réelle de la fenêtre Firefox capturée — l'échec
« résolution attendue 1280×720 » du harnais est un artefact du script, qui
compare à une résolution câblée en dur pour un autre scénario, pas un signe
d'anomalie). `packetsLost=0` dans les trois essais : aucune perte réseau.

**Conclusion du point Un :** le débit de bout en bout est **inchangé**,
~29,5-30 im/s, malgré la capture isolée à ~90 im/s. Le raisonnement qui
motivait la mission (capture à 90 Hz + sondage à 60 Hz ⇒ débit proche de
60 im/s) ne se vérifie pas : l'écart est bien en aval de la capture,
au sens où le localise le point Deux — mais pas là où l'intuition le plaçait.

## Deux — localisation du goulot, par la mesure

### Étape 1 — compteurs dans le chemin réel (`Session::run`)

Instrumentation temporaire dans `WindowsSource::next_frame` (compteurs
atomiques agrégés toutes les 2 s, journalisés puis retirés) : nombre
d'appels (`ticks`), nombre de captures neuves (`captured`), nombre de sorties
encodées récupérées (`produced`).

Résultat, code inchangé (baseline) :

```
ticks_hz    ≈ 60,0 Hz   (la boucle interroge exactement à la cadence visée)
captured_hz ≈ 30,0 Hz   (une capture neuve sur deux tours seulement)
produced_hz ≈ 30,0 Hz   (= captured_hz, aucun décalage mesurable)
```

Ce triplet, à lui seul, ne distingue PAS un plafond côté capture d'un
plafond côté encodeur : les deux mesures évoluent ensemble.

### Étape 2 — comparaison à isolation minimale (`CAPTURE_TEST` + `ENCODE_TEST`)

Pour casser cette ambiguïté, deux essais avec le MÊME contenu (Firefox,
`anim.html`, focus forcé), hors `Session::run` mais avec de VRAIES captures
répétées (pas le harnais à texture unique `ENCODER_THROUGHPUT_TEST`, non
comparable — voir la mise en garde de la mission) :

- **Capture seule, fil d'agitation désactivé** (`CAPTURE_TEST_NO_JITTER=1`,
  ajout temporaire) : 270 images / 3 s ≈ **90 im/s**. Le fil d'agitation
  n'était donc pour rien dans le gain par rapport à l'ancien chiffre de
  47 im/s : le contenu seul (`requestAnimationFrame`) suffit.
- **Capture + conversion + encodage réels, boucle serrée** (`ENCODE_TEST=1`,
  drainage de `poll_output()` à chaque itération) : 400 images encodées /
  5,002 s ≈ **80 im/s**, avec le même encodeur matériel (NVIDIA H.264
  Encoder MFT), la même fenêtre, un bitrate proche (8 Mbps contre 12 Mbps en
  production).

**Preuve positive exigée par la mission** : le même GPU, le même pilote, le
même encodeur soutiennent ~80 im/s ailleurs sur cette machine. Le plafond à
~30 im/s de `Session::run` n'est donc PAS une limite matérielle/pilote — il
est structurel au chemin réel.

### Étape 3 — décomposer le chemin réel : capture puis encodeur, séparément

Deux essais supplémentaires, strictement dans `WindowsSource`/`Session::run`
(instrumentation temporaire, retirée après usage) :

**a) Réessai borné sur la capture seule** (jusqu'à 8 ms/tour, comme
`SUBMIT_POLL_BUDGET` déjà présent pour l'encodeur) : `captured_hz` grimpe
jusqu'à ~58-60 Hz (pleine cadence de la boucle), mais **`produced_hz` reste
scotché à ~30 Hz**. Le débit navigateur ne bouge pas (444-445 trames / 15 s).
⇒ Une fois la capture décorrélée du plafond, elle n'est PLUS le facteur
limitant : le fait que `captured_hz` et `produced_hz` évoluaient ensemble à
l'étape 1 était une coïncidence de mesure, pas une preuve de causalité.

**b) Vérification via la télémétrie interne de l'encodeur** (déjà présente
dans `encode.rs` pour `ENCODE_TEST`, simplement relue depuis
`WindowsSource`) : avec la capture à pleine cadence, `submit()` reçoit des
images neuves à ~60 Hz, mais `EncoderTelemetry::converter_inputs` ne
progresse qu'à ~30 Hz — la majorité des images captées en trop sont
silencieusement abandonnées par `H264Encoder::submit` (`encode.rs` :
`if (pending_nv12.len() as u32) < pending_input_requests { feed_converter(...) }`
— si l'encodeur n'a pas encore redemandé d'entrée, l'image est simplement
jetée, sans erreur). Le drainage de `poll_output()` plus agressif (jusqu'à
8 sorties par tour au lieu d'une seule) n'a pas non plus changé le débit.

**c) Tentative combinée (réessai de capture + drainage agressif de
sortie) : régression.** Sur cet essai précis, `captured_hz` a atteint 0
pendant toute la fenêtre de mesure (60 tentatives/tour épuisant leur budget,
0 capture), alors qu'un `CAPTURE_TEST` isolé lancé immédiatement après (agent
arrêté) redonnait aussitôt ~90 im/s avec le même Firefox/animation — donc pas
un problème de focus ou d'animation arrêtée. Interprétation retenue, sans
l'avoir isolée plus finement faute de budget : contention GPU/pilote sur le
périphérique D3D11 partagé (`SetMultithreadProtected`) entre
`AcquireNextFrame` et les fils internes de Media Foundation, quand les deux
sont sollicités de façon anormalement soutenue en même temps.

### Conclusion du point Deux

**Le goulot est dans l'encodeur (converter + encodeur H.264 matériel), pas
dans la capture, ni dans le réseau/RTP (`packetsLost=0` partout), ni dans la
cadence de la boucle de transport (`ticks_hz` toujours ≈ 60,0 Hz, y compris
dans tous les essais).** Précisément : `H264Encoder::submit`, quand
sollicité au rythme réel de `Session::run` (soumissions espacées
d'exactement 16,7 ms), ne reçoit de nouvelles demandes d'entrée
(`METransformNeedInput`) qu'à ~30 Hz — malgré la preuve positive que le même
encodeur, sur la même machine, soutient ~80 im/s quand soumis en boucle
serrée (`ENCODE_TEST`). La différence entre les deux usages (cadence fixe à
16,7 ms contre boucle continue avec micro-pauses de ~1 ms) reste à
investiguer plus avant — deux pistes plausibles non tranchées ici faute de
budget : (i) un effet de cadence/latence propre au pilote NVENC quand les
soumissions sont trop régulièrement espacées de ~16,7 ms plutôt que
groupées ; (ii) un état interne (allocateur d'échantillons à taille fixe,
déjà documenté comme cause d'un bug antérieur) sensible au rythme de
sollicitation. **Ce n'est donc PAS la conclusion de la ronde précédente**
(« composition GPU/duplication du bureau limitée à ~30 Hz ») : cette
conclusion reposait sur un chiffre de référence de capture périmé et sur des
compteurs (capture, sortie) qui évoluaient ensemble sans que cela prouve
une causalité — corrigé ici par une expérience qui les décorrèle.

## Tentative de correctif — non retenue

Un réessai borné de la capture (8 ms/tour) puis un drainage élargi de
`poll_output()` (jusqu'à 8 sorties/tour, mises en file d'attente) ont été
implémentés et testés. **Aucun des deux, seul ou combiné, n'a amélioré le
débit navigateur** (toujours ~29,5-30 im/s) ; la combinaison a en outre
provoqué une régression ponctuelle (capture totalement à l'arrêt pendant
15 s, cf. étape 3c) sans qu'elle ait pu être diagnostiquée avec certitude
dans le budget imparti. Conformément à la consigne de ne rien laisser à
moitié fait ni risqué, **les deux changements ont été intégralement annulés**
(`git checkout -- agent/src/main.rs agent/src/windows_source.rs`, jamais
`git stash`) plutôt que conservés en l'état. Le dépôt est revenu à
l'identique du commit `4fa752c` avant toute conclusion.

## Vérifications finales

- Recompilation Windows systématiquement vérifiée à chaque changement
  (jamais un « Finished ... in 0.1s » suspect ; durées observées entre 2,0 s
  et 5,8 s selon l'ampleur du changement) et confirmée par grep du contenu
  réellement synchronisé sur `/media/vm/dev/agent/src/*.rs`, jamais par
  supposition.
- Après annulation des deux expériences : `git status` propre sur
  `agent/`, `proto/`, `client/`, `signaling/`, `scripts/` ; recompilation
  Windows propre (3,07 s, mêmes 5 avertissements préexistants, aucun
  nouveau) ; `cargo test` (`agent/`) **53/53** ; `cargo test` (`proto/`)
  **17/17** ; nouvelle mesure de bout en bout après ce retour à l'identique :
  445 trames / 15 s ≈ 29,67 im/s (cohérente avec les trois essais
  précédents).
- Invariant de drainage str0m (`agent/src/transport.rs`) : **fichier non
  modifié durant toute cette ronde** ; `act_on_timeout` reste structurellement
  limité à une seule mutation de `Rtc` par appel, drainée par `poll_output`
  avant la suivante (voir sa docstring, inchangée et toujours exacte).
- Toutes les tâches planifiées et fichiers temporaires créés sur la VM pour
  ce diagnostic (`capture-test-nojitter`, `capture-test-encode`, leurs
  scripts `.ps1`) ont été supprimés ; `run-agent.ps1`/`focus-result.txt`/
  `agent.log` nettoyés ; les serveurs `signaling`/`vite` locaux arrêtés.
  Les fichiers hérités de la ronde précédente (`anim.html`, `static.html`,
  `launch-*.ps1`, `focus-firefox.ps1`, `reset-window.ps1`, hors dépôt git,
  sur `/media/vm/dev` seulement) laissés en place, comme fait précédemment.
- Aucun commit créé (aucun changement conservé sur `agent/`).

## Pour la suite

Le plafond ~30 im/s est réel, reproductible, et maintenant précisément
localisé (rythme de `METransformNeedInput` de l'encodeur H.264 matériel sous
sollicitation à cadence fixe de 16,7 ms) — mais sa cause exacte au niveau du
pilote NVENC/Media Foundation reste à établir. Piste la plus prometteuse non
explorée ici : comparer le rythme de soumission d'`ENCODE_TEST` (boucle avec
`sleep(1ms)`) à celui de `Session::run` (soumissions espacées de 16,7 ms tick
par tick) en faisant varier UNIQUEMENT l'espacement des appels à `submit()`
sur un banc isolé, à contenu et bitrate strictement identiques, pour
confirmer si c'est la régularité/l'espacement de 16,7 ms lui-même (et non le
débit moyen) qui bride le rythme de `METransformNeedInput`.
