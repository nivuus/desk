# Résultats de mesure — chantier réseau adaptatif

## §1 NACK et RTX : ce qui existait déjà

**Contexte de la mesure** : VM Windows démarrée, agent compilé en `release` et
lancé en session interactive (tâche planifiée `/it`), fenêtre Firefox affichant
une page HTML animée en continu (`C:\dev\anim.html`, un carré coloré qui se
déplace à chaque `requestAnimationFrame`, garantissant un flux vidéo constant
plutôt qu'un écran statique dont l'absence de changement masquerait tout
NACK/RTX). Chaîne montée : signaling (port 8080), client Vite (port 5173,
préexistants d'une session antérieure), agent WebRTC connecté à la session
`demo`. Client de mesure : Chrome headless piloté par CDP (dérivé de
`client/verify-webrtc.mjs`), lisant `RTCPeerConnection.getStats()` — les
compteurs tels que le navigateur les calcule, pas une auto-évaluation du
client sous test.

### Question 1 — Le navigateur émet-il des NACK sous perte réelle ?

**OUI.**

Profil réseau `4g` du banc netem (`scripts/netem.sh 4g` : 10 Mbit/s, 60 ms
±20 ms de gigue, 1 % de perte, appliqué dans les deux sens via `ifb`) posé
pendant 25 s sur une session déjà connectée. Deux relevés `getStats()` de
l'entrée `inbound-rtp` vidéo, à 25 s d'intervalle :

| Relevé | `nackCount` | `packetsLost` | `packetsReceived` |
| --- | --- | --- | --- |
| 1 (juste après connexion) | 2 | 1 | 39 |
| 2 (+25 s sous perte réelle) | 155 | 81 | 7391 |

Δ `nackCount` = **153** sur 25 s. Le navigateur émet activement des NACK dès
qu'il détecte des paquets manquants — comportement natif de Chrome, rien à
construire côté client pour cette partie.

### Question 2 — L'agent retransmet-il (RTX) ?

**OUI**, à double preuve (compteur émetteur ET paquets réellement reçus par le
récepteur) :

**Côté agent** — `Event::MediaEgressStats.nacks` (str0m), journalisé
temporairement (voir `agent/src/transport.rs`, retiré après la mesure).
**Départ non observé à 0** : la piste vidéo a été négociée à
`12:18:04.247348Z` et la toute première ligne de stats émise, une demi-seconde
plus tard, affiche déjà `nacks=8` — aucune ligne à `nacks=0` n'existe pour
cette piste dans le journal (les lignes à `nacks=0` visibles dans le fichier
appartiennent à la piste audio, dont le compteur NACK est resté nul pendant
toute la mesure — piste distincte, aucun paquet perdu détecté dessus). Le
delta strictement mesurable depuis les deux lignes suivantes, les plus
anciennes et les plus récentes du journal pour cette piste, est donc
**8 → 155 (Δ147)**, pas 0 → 155 :

```
2026-07-29T12:18:04.763226Z  INFO agent::transport: stats sortantes (mesure temporaire) nacks=8 plis=0 rtt=None
...
2026-07-29T12:18:42.778299Z  INFO agent::transport: stats sortantes (mesure temporaire) nacks=155 plis=0 rtt=Some(154.833296ms)
```

**Côté navigateur** — la même entrée `inbound-rtp` porte les compteurs de
paquets retransmis *effectivement reçus*, pas seulement demandés :

| Relevé | `retransmittedPacketsReceived` | `retransmittedBytesReceived` |
| --- | --- | --- |
| 1 | absent (aucun événement encore) | absent |
| 2 (+25 s) | 157 | 165 182 octets |

157 paquets RTX reçus par le navigateur, à comparer aux 153 NACK émis et aux
155 NACK comptés côté agent (ordres de grandeur cohérents — un NACK peut
redemander plusieurs paquets, ou être suivi de plusieurs sursauts RTX selon la
fenêtre de report). La chaîne complète — perte réelle → NACK Chrome → NACK reçu
par str0m (`MediaEgressStats.nacks`) → RTX renvoyé → RTX reçu par Chrome — est
donc démontrée bout en bout, sans aucun code ajouté au projet.

**RTT observé** (piste vidéo, `rtt` de `MediaEgressStats`, mêmes lignes de
journal que ci-dessus) : minimum et maximum relevés sur les 40 lignes de la
fenêtre de 25 s :

```
2026-07-29T12:18:12.766953Z  INFO agent::transport: stats sortantes (mesure temporaire) nacks=57 plis=0 rtt=Some(94.087128ms)
2026-07-29T12:18:21.769876Z  INFO agent::transport: stats sortantes (mesure temporaire) nacks=97 plis=0 rtt=Some(182.787824ms)
```

Plage **94–183 ms**, cohérente avec le profil `4g` (60 ms ± 20 ms de délai
netem, dans les deux sens ≈ 80–160 ms, plus la latence RTCP normale).

### Question 3 — `transport-wide-cc` est-il négocié ?

**OUI.** C'est la question qui conditionne toute la suite du chantier (tâche
9, BWE) — traitée avec le plus de rigueur.

Réponse SDP réellement produite par `Session::accept_offer` (str0m), capturée
en journalisant temporairement `answer_sdp` (voir `agent/src/transport.rs`,
retiré après la mesure), sous profil `lan` (pas de dégradation — cette
question porte sur la négociation, pas sur le comportement sous perte) :

Extrait pertinent, piste vidéo (`m=video`), un des payload types H.264
(`103`) :

```
a=extmap:2 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time
a=extmap:3 urn:3gpp:video-orientation
a=extmap:4 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01
a=extmap:9 urn:ietf:params:rtp-hdrext:sdes:mid
a=extmap:10 urn:ietf:params:rtp-hdrext:sdes:rtp-stream-id
a=extmap:11 urn:ietf:params:rtp-hdrext:sdes:repaired-rtp-stream-id
...
a=rtpmap:103 H264/90000
a=rtcp-fb:103 transport-cc
a=rtcp-fb:103 goog-remb
a=rtcp-fb:103 ccm fir
a=rtcp-fb:103 nack
a=rtcp-fb:103 nack pli
a=fmtp:103 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42001f
a=rtpmap:104 rtx/90000
a=fmtp:104 apt=103
```

(Le même triplet `a=extmap:4 …transport-wide-cc-extensions-01` /
`a=rtcp-fb:<pt> transport-cc` / `a=rtcp-fb:<pt> nack` se répète pour les six
autres payload types H.264 offerts par Chromium — `107`, `109`, `115`, `117`,
`39` — et pour la piste audio Opus (`111`, avec `a=rtcp-fb:111 transport-cc`
et le même `a=extmap:4`).)

- **`a=rtcp-fb:<pt> nack` présent** : oui, pour tous les payload types vidéo.
- **`a=rtpmap:<pt> rtx/90000` présent** : oui, un payload type RTX (`104`,
  `108`, `114`, `116`, `118`, `40`) associé par `a=fmtp:<pt> apt=<pt-base>` à
  chacun des six payload types H.264.
- **`a=extmap:… transport-wide-cc` négocié** : oui —
  `a=extmap:4 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01`
  est présent à la fois sur la piste vidéo et sur la piste audio, et
  `a=rtcp-fb:<pt> transport-cc` accompagne chaque payload type des deux
  pistes.

**Conséquence pour la tâche 9** : l'extension transport-wide-cc est déjà
négociée par str0m dès aujourd'hui (elle répond simplement à ce que Chromium
offre) — **aucun travail de négociation SDP n'est nécessaire**. Le
prérequis du BWE de str0m (`Rtc::bwe()`) est donc déjà rempli côté
signalisation ; reste seulement à l'activer côté configuration `Rtc` et à
consommer les événements `EgressBitrateEstimate`, comme prévu par le design.

## Synthèse

| Question | Réponse | Preuve |
| --- | --- | --- |
| Le navigateur émet-il des NACK sous perte réelle ? | **Oui** | `nackCount` 2→155 (+153) sur 25 s sous profil `4g` |
| L'agent retransmet-il (RTX) ? | **Oui** | `MediaEgressStats.nacks` 8→155 (Δ147, premier/dernier relevé du journal) côté agent ; `retransmittedPacketsReceived`=157 côté navigateur |
| `transport-wide-cc` est-il négocié ? | **Oui** | `a=extmap:4 …transport-wide-cc-extensions-01` + `a=rtcp-fb:<pt> transport-cc` sur toutes les pistes, réponse SDP réelle de `str0m` |

**La résilience NACK/RTX et la négociation transport-wide-cc sont déjà
entièrement en place, sans aucun code applicatif à écrire.** Ce chantier n'a
donc rien à construire pour ces trois mécanismes ; la tâche 9 (BWE) peut
directement consommer `Rtc::bwe()` / `EgressBitrateEstimate` sans étape de
négociation SDP préalable.

## Méthode et limites (transparence)

- **Contenu capturé** : une page HTML locale animée en continu
  (`C:\dev\anim.html`), pas une application réelle. Choisi pour garantir un
  flux constant de paquets vidéo pendant toute la fenêtre de mesure — un
  écran statique aurait donné trop peu de paquets pour observer des NACK en
  25 s. Les chiffres absolus (paquets/s, débit) ne sont donc pas
  représentatifs d'un usage applicatif ; les mécanismes observés (NACK, RTX,
  extmap négocié) sont eux protocolaires et indépendants du contenu.
- **Écart de comptage NACK vs RTX** (153 NACK émis, 155 comptés côté agent,
  157 RTX reçus) : non expliqué précisément, mais l'ordre de grandeur est
  cohérent et attendu (un NACK peut lister plusieurs numéros de séquence, une
  RTX peut être déclenchée pour un paquet déjà redemandé). Ce n'est pas ambigu
  au sens où cela remettrait en cause la conclusion « NACK et RTX
  fonctionnent » — seule la correspondance exacte 1:1 entre les trois
  compteurs n'est pas établie, et ne l'a pas été recherchée (hors périmètre de
  la question posée).
- **Incident rencontré, non lié au sujet mesuré** : la toute première tentative
  de connexion sous profil `4g` a échoué (`ICE déconnecté` après ~19 s,
  `iceConnectionState` passé à `failed`). Comportement déjà documenté dans un
  rapport antérieur du projet (tâche 12, jalon 1) comme non reproductible de
  façon systématique et sans rapport avec le sujet de cette tâche. Un
  redémarrage de l'agent (`scripts/stop-agent.sh` puis `scripts/run-agent.sh`)
  a suffi ; la tentative suivante a convergé normalement (`iceConnectionState`
  → `connected` en moins de 2 s). Signalé par prudence — non recreusé, hors
  périmètre de cette mesure.
- **Non mesuré dans cette tâche** : le comportement sous les profils
  `congestionné` et `effondrement` (plus sévères que `4g`), et la valeur
  effective du débit estimé par le BWE de str0m (`Rtc::bwe()` n'est pas encore
  activé — c'est précisément l'objet de la tâche 9). Cette tâche répond
  uniquement à « la négociation transport-wide-cc a-t-elle eu lieu ? », pas à
  « l'estimation de bande passante produit-elle une valeur exploitable ? ».
- **Non-régression LAN** : cette tâche n'a modifié aucun code persistant
  (l'instrumentation de `agent/src/transport.rs` est retirée par
  `git checkout` avant le commit du présent document) — la contrainte de
  non-régression LAN du chantier (≥ 55 i/s, latence médiane < 50 ms) ne
  s'applique donc pas à cette tâche, faute de changement à mesurer.

## Commandes de référence

```bash
# Build + lancement agent (session interactive, cf. piège agent.log)
set -a && source .env && set +a
scripts/build-agent.sh
WINDOW_TITLE=firefox RUST_LOG=info scripts/run-agent.sh

# Contenu animé en continu côté VM (garantit un flux vidéo constant)
# C:\dev\launch-anim.ps1 : tue firefox puis relance sur file:///C:/dev/anim.html

# Chaîne signaling + client (si non déjà démarrés)
cd signaling && npx tsx src/index.ts &     # port 8080
cd client && npx vite --host 127.0.0.1 --port 5173 &

# Étape 2 — SDP (profil lan, pas de dégradation)
sudo scripts/netem.sh lan
grep -a -i -E "rtcp-fb|rtx|transport-cc" <(iconv -f UTF-16LE -t UTF-8 /media/vm/dev/agent.log 2>/dev/null || cat /media/vm/dev/agent.log)

# Étape 3 — NACK/RTX sous perte réelle
sudo scripts/netem.sh 4g
# sonde CDP jetable (dérivée de client/verify-webrtc.mjs), lit nackCount /
# retransmittedPacketsReceived de l'entrée inbound-rtp vidéo distante
DURATION_MS=25000 node /tmp/nack-probe.mjs "http://127.0.0.1:5173/?session=demo"

# Nettoyage
sudo scripts/netem.sh off
scripts/stop-agent.sh
git checkout agent/src/transport.rs
```
