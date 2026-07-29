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

---

## §2 Recette par profil (tâche 12)

**Contexte de la mesure** : VM Windows démarrée, agent compilé en `release`
(recompilé sur la VM au début de cette tâche pour couvrir le changement de
`transport.rs` de la tâche 10, jamais vérifié sur la VM jusqu'ici — compile
sans erreur, 3 avertissements `dead_code` préexistants). Contenu vidéo :
`C:\dev\anim.html` (carré animé en continu, garantit un flux constant).
Contenu audio (nécessaire uniquement pour la propriété FEC, profil `4g`) :
lecture en boucle de `C:\Windows\Media\Alarm01.wav` via une tâche planifiée
dédiée (`audio-loop.ps1`, voir §6) — sans cela, le flux audio capturé par le
loopback WASAPI système est resté quasi silencieux (≈1 kb/s) pendant toute la
recette, faussant toute mesure du débit audio. Client de mesure : Chrome
headless piloté par CDP, deux instruments :
- `client/recette/probe-link.mjs` (créé pour cette tâche — voir §6) :
  échantillonne `#status` (texte de l'indicateur) et `#stats` (overlay) toutes
  les 2 s pendant ~65-75 s, pour observer l'évolution de la résolution/du
  débit/du texte d'alerte sous profil constant.
- `client/recette/harness.mjs latency` : latence tactile→photon, méthode du
  chantier 0.

**Discipline de mesure imposée par un piège découvert en cours de tâche** :
une tentative de connexion sur un agent déjà utilisé une fois reste bloquée en
`connectionState=new` indéfiniment (aucune erreur, juste rien ne se passe).
Il faut redémarrer l'agent (`stop-agent.sh` puis `run-agent.sh`) avant
**chaque** tentative de connexion, pas seulement une fois par profil comme le
brief le suggérait. Voir §6 pour le détail.

| Grandeur | `adsl` (8 Mb/s, 30 ms ±5, 0 % perte) | `4g` (10 Mb/s, 60 ms ±20, 1 % perte) | `congestionné` (3 Mb/s, 100 ms ±20, 3 % perte) | `effondrement` (500 kb/s, 150 ms ±30, 5 % perte) |
| --- | --- | --- | --- | --- |
| **Estimation BWE atteinte** | 538 210 – 7 056 620 b/s | 454 073 – 6 200 485 b/s | 182 490 – 2 638 438 b/s | 129 931 – 843 808 b/s (dépasse ponctuellement le débit posé — sondage BWE normal, sans effet sur le débit réellement appliqué, voir §3.1) |
| **Débit d'encodage retenu** (overlay, plage observée) | ≈0,16 – 5,9 Mb/s | ≈0,03 – 1,19 Mb/s | ≈0,02 – 2,56 Mb/s | ≈0,01 – 0,43 Mb/s |
| **Barreau atteint et instants des changements** (depuis connexion) | 764×484 → **508×322 @ +18,0 s** → **382×242 @ +39,0 s** → **764×484 @ +67,0 s** | Deux trajectoires mesurées, non identiques (voir §3.2) : session A (sans audio) 764×484 → **382×242 @ +12 s** puis stable ; session B (avec audio) 764×484 → **508×322 @ +5,9 s** → **382×242 @ +13,9 s** → **610×386 @ +47,9 s** | 764×484 → **508×322 @ +7,1 s** → **382×242 @ +14,1 s**, stable ensuite | 764×484 → **382×242 @ +4,6 s** (saut direct au plancher), stable ensuite |
| **Changements de barreau en 60 s** | 2 dans les 60 premières secondes, un 3ᵉ à 67 s (une fenêtre glissante démarrant à t≈8 s en capture 3) | Session A : 1. Session B : 3 dans les 60 premières secondes | 2, tous deux dans les 60 premières secondes, puis stable | 1 |
| **Latence médiane** (harnais tactile→photon) | **non mesurée** — harnais non exécuté sous ce profil faute de temps (voir §6) ; proxy overlay (« ≈ », non équivalent) : ≈44–58 ms au barreau réduit, jusqu'à ≈460 ms pendant les transitions | 201,0 ms bruts (6/10 essais) — **valeur peu fiable** : l'échantillon contient une lecture négative aberrante (−1518,2 ms, artefact de méthode, voir §6) ; hors cette valeur, médiane des 5 restants = 215,6 ms | 261,8 ms bruts (6/10 essais), même artefact ; hors valeur aberrante (5 restants) = 286,5 ms | 513,7 ms (3/8 essais seulement — faible rendement, cohérent avec un lien à 500 kb/s) |
| **Débit d'images** | non mesuré en continu (pas de passage `harness.mjs stats` dédié sous ce profil, voir §6) ; lectures ponctuelles de l'overlay 40–92 i/s | non mesuré en continu ; lectures ponctuelles 0–90 i/s (0 pendant les paliers de tampon) | non mesuré en continu ; lectures ponctuelles 0–266 i/s (rafales de rattrapage après vidage de tampon) | non mesuré en continu ; lectures ponctuelles majoritairement à 0 i/s, rafales jusqu'à ≈200 i/s |
| **Texte de l'indicateur** | `WxH, X Mb/s` (Bonne) ↔ `Image réduite par le réseau — WxH, X Mb/s` (Degradee) ; **jamais** « Réseau insuffisant » | `Image réduite par le réseau — …` ↔ `Réseau insuffisant pour le jeu nerveux — …`, alternance cohérente avec le barreau/débit courant | idem 4g, alternance cohérente, majoritairement « Réseau insuffisant » en fin de fenêtre | **« Réseau insuffisant pour le jeu nerveux »** dès +14 s, reste affiché **sans interruption** jusqu'à la fin de la fenêtre (+68 s) — jamais de retour au vert |

## §3 Verdict sur les quatre propriétés attendues

### 3.1 Le débit suit — **TENUE**

Sur les quatre profils, le débit d'encodage réellement retenu (colonne
« Débit d'encodage retenu » ci-dessus) est resté sous le plafond posé par
`netem`, marge comprise, à chaque relevé. La seule valeur qui dépasse le
débit posé est l'**estimation BWE brute** sous `effondrement` (843 808 b/s
pour un lien à 500 kb/s) — ce n'est pas une violation de la propriété : le
sondage à la hausse fait partie du fonctionnement normal d'un algorithme de
type GCC (il teste la capacité disponible), et `Controleur` n'applique jamais
l'estimation brute telle quelle (`MARGE` = 0,9, plus le calcul du barreau
financé) — le débit *appliqué* correspondant, lu à l'overlay au même
instant, est resté ≤0,43 Mb/s, bien en dessous des 500 kb/s. Distinction
vérifiée sur les quatre profils, pas seulement supposée.

### 3.2 La résolution descend et remonte sans battre — **PARTIELLE**

**Découverte structurante de cette tâche** : la première mesure sous `adsl`
(avant tout ajustement) a montré **4 changements de barreau en 49 s**
(764→508→382→764→508), très au-delà du « au plus deux » attendu. Preuve
tracée dans le journal de l'agent : une remontée au barreau plein
(`taille d'encodage changée largeur=764 hauteur=484` à 16:24:09.545Z) est
suivie, **une seconde plus tard**, d'un effondrement de l'estimation BWE
d'un facteur ×10 en une seule observation
(`estimation=Some(6639480)` à 16:24:10 puis `estimation=Some(619982)` à
16:24:11) — cohérent avec l'image clé que le changement de résolution
déclenche lui-même, interprétée par l'estimateur comme une surcharge. La
remontée suivante retombe alors 5,0 s plus tard, pile le plancher
`SEJOUR_MINIMAL`.

**Correctif appliqué** (voir §5) : `DELAI_REMONTEE` porté de 10 s à 20 s.
Remesuré sous le même profil `adsl` : **2 changements dans les 60 premières
secondes** (amélioration nette par rapport aux 4 précédents), mais un
**3ᵉ change survient à +67 s** — une fenêtre glissante de 60 s démarrant
juste avant le premier changement (t≈8 s) capture les trois. Sous `4g`,
deux sessions mesurées donnent des résultats différents : 1 changement dans
une session, 3 dans l'autre (764→508→382→610 en 48 s). Sous `congestionné`
et `effondrement`, la propriété est en revanche solidement tenue (2 et 1
changement respectivement, tous dans les 60 premières secondes, puis stable).

**Verdict, sans arrondir** : le correctif réduit réellement l'oscillation
(rythme des changements sous `adsl` environ divisé par deux) et la propriété
est proprement tenue sous les deux profils les plus sévères
(`congestionné`, `effondrement`). Mais sous `adsl` et `4g` — les profils où
l'estimation oscille le plus près des seuils de barreau — une lecture stricte
« aucune fenêtre de 60 s ne doit voir plus de deux changements » n'est **pas**
systématiquement respectée. D'où **partielle**, pas tenue : le chantier a
amélioré la situation sans l'avoir complètement résolue, et l'échantillon
(une à deux sessions par profil) est trop court pour trancher si un nouvel
allongement de `DELAI_REMONTEE` réglerait le reste ou si la cause (BWE
perturbé par les images clés du contrôleur lui-même) demande un remède
différent (voir réserves, §6).

### 3.3 L'indicateur dit vrai — **TENUE**

C'est la propriété la mieux établie par cette recette, et la plus
importante : sous `effondrement`, l'indicateur affiche « Réseau insuffisant
pour le jeu nerveux » dès +14 s et **le garde affiché sans interruption
jusqu'à la fin de la fenêtre observée (+68 s)** — jamais de retour au vert
pendant que le lien reste saturé. Sous `congestionné`, la même alerte
apparaît et alterne de façon cohérente avec « Image réduite par le réseau »
selon que le barreau courant est ou non le plancher de l'échelle. Sous `4g`,
même comportement. Sous `adsl` — le profil le moins sévère des quatre —
l'indicateur ne déclenche **jamais** « Réseau insuffisant », alternant
seulement « Bonne » et « Image réduite » : comportement cohérent avec un lien
à 8 Mb/s qui n'atteint jamais le plancher de l'échelle. Aucune session,
sur aucun profil, n'a laissé l'indicateur au vert alors que le lien était
dégradé.

### 3.4 Le FEC opère — **PARTIELLE / signature attendue non confirmée**

Mesuré sous `4g` (1 % de perte), avec audio réel (voir §6 sur la nécessité
d'un correctif de méthode ici). Trois résultats, à ne pas arrondir :

1. **Pas de coupure totale** : le flux audio n'a jamais cessé — octets et
   paquets reçus ont continué de croître sur toute la fenêtre mesurée
   (25-30 s), sans palier à zéro. Mais « pas de coupure **audible** » n'a
   **pas été vérifié par une écoute humaine** dans le budget de cette tâche
   — seul un proxy automatique est disponible : 22 `concealmentEvents` /
   11 573 `concealedSamples` sur 25 s (≈240 ms d'audio dissimulé au total sur
   25 s, soit ≈1 % du temps), un taux modeste mais non nul.
2. **La signature de débit attendue par le brief n'apparaît pas** : débit
   audio mesuré ≈118,9–120,6 kb/s sur deux relevés de 25-30 s — **au niveau
   du débit nominal de 128 kb/s, pas sensiblement au-dessus**. Ce résultat
   n'est PAS attribué à un FEC inopérant : il **corrobore exactement** la
   découverte de la tâche 8 (« sous un débit cible fixe, LBRR ne s'ajoute
   pas aux octets, il les redistribue » — `docs/.../2026-07-28-*` et le
   rapport de tâche 8), qui avait déjà réfuté l'hypothèse d'une inflation de
   débit comme preuve du FEC. Le brief de cette tâche 12 reprenait
   pourtant cette hypothèse sans la corriger — écart signalé ici plutôt que
   silencieusement corrigé.
3. **`fecPacketsReceived` s'est révélé être la mauvaise métrique** :
   toujours à 0 sur la session mesurée, malgré 23 paquets perdus. Investigué
   avant d'écrire une conclusion hâtive : ce compteur `getStats()` mesure une
   famille de FEC RTP **séparée** (RED/ulpfec, des paquets FEC distincts),
   pas la redondance encodée **à l'intérieur** de chaque trame Opus (LBRR).
   Sa valeur nulle ne prouve donc **ni** que le FEC in-band est inactif
   **ni** qu'il fonctionne — ce n'est simplement pas le bon canal
   d'observation pour ce mécanisme.
4. **Ce qui est établi indirectement** : le journal de l'agent montre des
   valeurs de perte réellement transmises au calcul du taux (`perte` dans
   les lignes `observation réseau`) allant jusqu'à 6,4 % pendant cette
   session — largement de quoi déclencher `set_packet_loss_perc` avec une
   valeur non nulle côté encodeur Opus. Combiné à la preuve **décisive** déjà
   apportée par la tâche 8 au niveau décodeur (reconstruction LBRR mesurée
   par énergie : 0,00 sans perte déclarée contre 585,02 avec, sur un
   décodeur neuf), il y a de bonnes raisons de penser que la redondance a
   réellement été codée pendant cette session — mais ce n'est pas une preuve
   directe **de cette session précise**, seulement une inférence à partir
   d'une preuve mécanistique établie ailleurs.

**Verdict** : le critère précis demandé par le brief (débit audio
sensiblement supérieur au nominal) n'est **pas** observé, pour une raison
déjà connue du projet et non liée à un défaut de cette tâche. La propriété
n'est ni clairement tenue ni clairement fausse au vu des preuves
disponibles : **partielle**, avec le détail ci-dessus plutôt qu'un verdict
binaire qui masquerait la nuance.

## §4 Non-régression LAN — chiffres avant et après

**Référence pré-chantier** (`plans/2026-07-28-debit-latence.md`) :
**62,6 i/s** et médiane **47,9 ms**.

**Après ce chantier** (agent avec `DELAI_REMONTEE` = 20 s, mesuré sur cette
même VM) :

| Mesure | Valeurs relevées | Seuil | Verdict |
| --- | --- | --- | --- |
| Débit (images décodées), 6 passages indépendants, 20-25 s chacun | 50,75 · 54,63 · 55,53 · 55,68 · 56,60 · 62,70 i/s — **moyenne 56,0, médiane 55,6** | ≥ 55 i/s | Tendance centrale au-dessus du seuil ; 2 des 6 passages individuels en dessous |
| Latence médiane (tactile→photon), 12 essais, 8 exploitables | **48,2 ms** (min 43,2, max 73,1) | < 50 ms | Franchi |

**Verdict, sans arrondir** : la tendance centrale (moyenne et médiane) des
six mesures de débit franchit le seuil, tout comme la latence. Mais le
débit n'est **pas** uniformément ≥55 i/s : deux passages sur six (50,75 et
54,63 i/s) sont en dessous. Deux constats permettent de ne pas imputer cet
écart au chantier :
- **Aucun changement de barreau n'a été observé dans aucune des sessions LAN
  mesurées** (0 ligne `taille d'encodage changée` sur l'ensemble des
  journaux LAN capturés) — le contrôleur de congestion, `DELAI_REMONTEE`
  compris, n'est **jamais engagé** sous ce profil : l'estimation BWE reste
  systématiquement bien au-dessus du plafond de 12 Mb/s (11,6–20,5 Mb/s
  observés), donc aucun mécanisme de ce chantier ne peut expliquer la
  variance mesurée. **Preuve directe**, vérifiable dans les journaux cités.
- **La machine hôte était sous forte charge** pendant la recette : `uptime`
  a relevé une charge moyenne d'environ **15,5 sur 8 cœurs physiques** (la
  VM Windows réclame à elle seule 14 vCPU), ce qui est cohérent avec des
  passages parfois sous le seuil sans corrélation avec un changement de
  code. **Preuve indirecte, à pondérer en conséquence** : il s'agit d'une
  **seule lecture `uptime`**, prise à un instant de la séance et non
  horodatée précisément, jamais mise en regard de l'instant exact de chacun
  des deux passages faibles (50,75 et 54,63 i/s) pour confirmer que la
  charge était effectivement plus haute *pendant ces passages-là* que
  pendant les quatre autres. Cet argument reste plausible (la charge
  observée est structurellement présente tout du long, la VM seule dépassant
  déjà la capacité physique de la machine) mais **n'est pas, contrairement
  au premier constat, une démonstration** — seulement une explication
  compatible avec les chiffres.

Le seuil `ESTIMATION_INITIALE_BPS` (2 500 000 b/s) n'a **pas** été ajusté :
dans les journaux LAN capturés, l'estimation atteint 8,7–17,7 Mb/s dès la
première ou la deuxième seconde après connexion et dépasse durablement les
10 Mb/s en moins de 3 s — largement dans la fenêtre de mesure (20-25 s), ce
qui exclut une convergence trop lente comme explication de la variance
observée.

**Note de méthode** : la mesure de latence (48,2 ms) a été prise **avant**
l'ajustement de `DELAI_REMONTEE` et n'a pas été rejouée après. Elle reste
valide sans nouvelle mesure : comme démontré ci-dessus, aucun changement de
barreau — et donc aucun usage de `DELAI_REMONTEE` — n'est jamais survenu
sous LAN, avant ou après l'ajustement.

## §5 Réglages ajustés

| Réglage | Valeur avant | Valeur après | Justification |
| --- | --- | --- | --- |
| `DELAI_REMONTEE` (`agent/src/congestion.rs`) | 10 s | **20 s** | Oscillation mesurée sous `adsl` (4 changements de barreau en 49 s), tracée à un effondrement ×10 de l'estimation BWE une seconde après une remontée au barreau plein — cohérent avec le coût de l'image clé que la remontée déclenche elle-même. Doublé pour exiger deux fois plus de temps de confiance avant de reprendre la pleine résolution. Remesuré : oscillation réduite (2 changements dans les 60 premières s au lieu de 4) mais pas éliminée (un 3ᵉ survient à +67 s) — voir §3.2. Test `remonter_exige_dix_secondes_et_non_deux` renommé `remonter_exige_vingt_secondes_et_non_deux` et ses bornes ajustées (19 999 ms → None, 20 000 ms → Some) ; 137/137 tests agent toujours verts, `cargo clippy` propre, recompilation VM vérifiée sans erreur. |
| `ESTIMATION_INITIALE_BPS` (`agent/src/transport.rs`) | 2 500 000 b/s | **inchangé** | Seuils de non-régression LAN franchis sur la tendance centrale (§4) ; la clause du brief qui déclenche cet ajustement (« en dessous, ne pas continuer ») ne s'applique donc pas. Vérifié tout de même par prudence : l'estimation atteint 8,7–17,7 Mb/s en 1-3 s après connexion sous LAN, loin d'expliquer une quelconque variance de mesure. |
| `BPP_MIN` (`agent/src/congestion.rs`) | 0,05 | **inchangé** | Aucune preuve contraire mesurée : sous `adsl` (8 Mb/s), le seuil du barreau plein pour la source captée (764×484 à 60 fps) vaut ≈1,11 Mb/s — très en dessous du débit du lien, donc pas la cause des descentes observées (celles-ci proviennent de l'instabilité de l'estimation BWE traitée ci-dessus par `DELAI_REMONTEE`, pas d'un seuil mal calibré). **Réserve honnête** : le critère du brief pour ajuster `BPP_MIN` (« l'image en pleine résolution était visiblement acceptable/dégradée ») suppose un jugement visuel qui n'a pas été fait dans cette tâche — aucune capture d'écran n'a été comparée à l'œil. La valeur est donc reconduite faute de preuve du contraire, pas confirmée par une inspection visuelle positive. `cargo test -p agent congestion` : 15/15, inchangé. |

## §6 Ce que la mesure a coûté

- **Faux départ (≈15 min)** : le serveur de développement Vite tournait
  depuis 10:28, bien avant que `proto/ts/control.ts` ne soit modifié à
  17:46 (passage à `CONTROL_VERSION = 3`, tâche 10). La première tentative
  de vérification a donc échoué avec « version de contrôle non supportée :
  3 » — un faux négatif, le client servait un bundle figé avec l'ancienne
  version. Résolu en redémarrant le serveur Vite (`pkill` puis relance sur
  le port 5173, qui s'était décalé sur 5174 lors d'une première tentative
  de redémarrage incomplète).
- **Piège de connexion découvert et documenté** : une tentative de connexion
  sur un agent déjà utilisé une fois reste bloquée en `connectionState=new`
  sans jamais échouer explicitement ni réussir. Le brief documentait déjà
  qu'il fallait un agent neuf par profil ; cette tâche a établi qu'il en
  faut en réalité un neuf par **tentative de connexion**, y compris entre un
  passage `probe-link` et un passage `harness latency` sur le même profil.
  A coûté plusieurs allers-retours avant d'être identifié avec certitude.
- **La VM s'est arrêtée spontanément une fois**, pendant la mesure du
  profil `congestionné` — symptôme déjà documenté dans plusieurs rapports
  de tâches antérieures (7, 9, 12/jalon1) comme récurrent et non
  investigué. A coûté une session de mesure ratée (deux tentatives de
  connexion ont échoué en `ICE failed` avant que le diagnostic ne pointe
  vers `virsh list --all` → « fermé »). Redémarrée sans perte : le profil
  `netem` posé côté hôte (`internalBridge`/`ifb0`) a survécu au redémarrage
  de la VM, confirmé par `tc qdisc show`.
- **Outils jetables créés pour cette tâche**, non requis par le brief mais
  nécessaires pour obtenir les grandeurs demandées :
  - `client/recette/probe-link.mjs` — échantillonne `#status` et `#stats`
    dans le temps (le harnais existant ne lisait que `#stats`, pas le texte
    de l'indicateur nécessaire à la propriété 3).
  - `client/recette/fec-check.mjs` — lit directement
    `fecPacketsReceived`/`concealedSamples`/`concealmentEvents` de l'entrée
    `inbound-rtp` audio (a permis de découvrir que `fecPacketsReceived` est
    la mauvaise métrique pour le FEC in-band Opus, voir §3.4).
  - `audio-loop.ps1` (sur la VM) — lecture en boucle d'un fichier `.wav`
    système, nécessaire car le contenu vidéo (`anim.html`) ne produit aucun
    son : sans cette source, le flux audio capturé restait quasi silencieux
    (~1 kb/s) et rendait toute mesure de la propriété FEC vide de sens.
    Lancé comme tâche planifiée séparée (`guacamole-audio-loop`),
    indépendante du cycle agent/Firefox.
  - `launch-scroll.ps1` (sur la VM) — tenté en premier pour le mode `stats`
    du harnais existant (défilement à la molette). **Sans effet observé** :
    0 image décodée sur 15 s malgré 51 messages de molette envoyés,
    cohérent avec une réserve déjà consignée dans une recette antérieure
    (`2026-07-27-jalon1-recette.md` : « la molette n'a pas reproduit d'effet
    visible dans le harnais »). Abandonné au profit de `anim.html`
    (animation autonome, déjà vérifiée fonctionnelle) pour toutes les
    mesures de débit d'images de cette tâche — écart au brief qui citait
    le harnais du chantier 0 sans préciser le contenu, assumé et documenté
    ici plutôt que silencieusement.
- **Limite de méthode découverte sur `harness.mjs latency`** : sous les
  profils dégradés (`4g`, `congestionné`), le tampon de gigue dépasse
  couramment 200-400 ms, parfois plusieurs secondes. Le harnais suppose une
  pause de 1,5 s entre essais suffisante pour que l'état se stabilise avant
  le prochain essai — hypothèse violée sous ces profils : au moins un essai
  par profil dégradé a produit une latence **négative** (artefact où
  l'échantillon « avant » d'un essai reflétait encore une transition de
  couleur de l'essai précédent, encore en vol dans le tampon). Signalé dans
  les tableaux ci-dessus avec la médiane recalculée hors artefact ; **non
  corrigé** dans le budget de cette tâche — le harnais reste fiable sous
  LAN et pour des profils où le tampon reste court.
- **Détour d'investigation (~20 min)** sur `fecPacketsReceived` avant de
  comprendre qu'il s'agit d'un compteur RED/ulpfec et non du FEC in-band
  Opus — voir §3.4.
- **Étape 5 (passage sur lien réel) non réalisée** : aucun dispositif
  externe (ChromeOS via Pomerium, partage de connexion mobile) n'était
  disponible dans cet environnement d'exécution pour cet agent — **non
  mesuré**, faute d'accès matériel, pas par choix.
- **Mesures non refaites faute de budget, deux lacunes distinctes** (voir
  §2, cellules marquées « non mesuré ») :
  - **Débit d'images en régime continu** (`harness.mjs stats`, seule
    mesure produisant une moyenne soutenue sur plusieurs secondes plutôt
    que des lectures ponctuelles) : **non mesuré sous aucun des quatre
    profils dégradés**, pas seulement sous `adsl`. Notable précisément
    parce que l'instrument fonctionnait de façon démontrée : les six
    passages LAN de §4, avec le même contenu (`anim.html`), en sont la
    preuve directe. La lacune vient donc du budget de temps consacré à
    cette tâche, pas d'une limite technique du harnais ou du contenu de
    test — seules des lectures ponctuelles de l'overlay (colonne « Débit
    d'images » de §2) comblent partiellement ce trou, avec une variance
    bien plus grande qu'une moyenne soutenue ne l'aurait montré.
  - **Latence tactile→photon dédiée** (`harness.mjs latency`) : celle-ci a
    bien été mesurée sous `4g`, `congestionné` et `effondrement` (voir §2
    et les sorties brutes du rapport de tâche) — seul `adsl` en est
    dépourvu, faute de temps, avec pour seul repère le proxy « ≈ » de
    l'overlay (non équivalent, voir §2).

## §7 Preuve comportementale de C1 (redimensionnement × adaptation)

**Contexte de la mesure.** C1 (revue finale de branche, corrigé au commit
`3f02545`) n'était vérifié que par lecture de code côté intégration : la
re-revue de la vague de correction a établi que le câblage de la branche
`a1` (`Session::act_on_timeout` dans `agent/src/transport.rs`) n'est exercé
par aucun test automatisé — `VideoSource::resize` est un no-op par défaut
dans toutes les sources factices des tests, et rien n'y positionne
`pending_resize`. Cette section est donc la seule preuve comportementale que
la correction fonctionne réellement, sur la VM, de bout en bout.

**Méthode employée.** Chrome headless piloté par CDP (script jetable dérivé
du même patron que `client/verify-webrtc.mjs`), utilisant
`Emulation.setDeviceMetricsOverride` pour imposer une taille de viewport
précise. `#remote` est stylé `width:100vw;height:100vh`
(`client/src/style.css`) : changer le viewport par CDP redimensionne donc
réellement l'élément vidéo observé par le `ResizeObserver` de
`client/src/main.ts`, qui envoie alors un vrai message de contrôle `Resize`
sur le canal de données — exactement le chemin qu'emprunterait un
redimensionnement de fenêtre de navigateur réel. C'est la méthode la plus
directe disponible : elle exerce le code de production sans le modifier,
contrairement à un appel direct à l'émission du message de contrôle depuis
la console de la page. Agent compilé en `release` (`scripts/build-agent.sh`,
compilation VM propre), lancé avec `RUST_LOG=agent=debug`
(`scripts/run-agent.sh`), profil réseau `lan` (aucune dégradation posée,
`tc qdisc show dev internalBridge` → `noqueue`, aucune qdisc active).
Fenêtre capturée : `firefox` (titre par défaut), session `demo`. Une seule
session WebRTC continue a porté les trois étapes ci-dessous (connexion,
puis HAUSSE, puis BAISSE), pour rester dans le cas exact que C1 corrige :
une adaptation qui doit suivre plusieurs redimensionnements successifs sans
se figer.

**Relevé, les quatre tailles à chaque étape** (taille demandée = message
`Resize` reçu par l'agent, `agent: contrôle reçu Resize {...}` ; taille de
capture agent = `agent::windows_source: chaîne d'encodage reconstruite
self.width=… self.height=…`, qui est aussi la taille d'encodage initiale
après un `resize` puisque l'encodeur est reconstruit à la taille pleine de
la capture ; taille d'encodage agent = la même, sauf si une ligne
`agent::transport: taille d'encodage changée largeur=… hauteur=…` apparaît
ensuite — **aucune n'est apparue à aucune étape**, le contrôleur de
congestion gardant le barreau plein sur ce lien non dégradé (estimation BWE
observée 16,5–18,8 Mb/s, très au-dessus du plafond de 12 Mb/s) ; taille
reçue navigateur = `frameWidth`/`frameHeight` de l'entrée `inbound-rtp`
vidéo de `RTCPeerConnection.getStats()`) :

| Étape | Taille demandée | Taille de capture agent | Taille d'encodage agent | Taille reçue navigateur |
| --- | --- | --- | --- | --- |
| État initial | 640×480 | 624×472 | 624×472 (aucun changement séparé) | 624×472 |
| **HAUSSE** | 1600×900 | 1584×892 | 1584×892 (aucun changement séparé) | 1584×892 |
| **BAISSE** | 480×360 | 500×352 | 500×352 (aucun changement séparé) | 500×352 |

Les trois colonnes agent/navigateur concordent exactement à chaque étape,
horodatage à l'appui :

```
2026-07-29T20:50:39.838652Z  INFO agent: contrôle reçu Resize { version: 3, width: 640, height: 480 }
2026-07-29T20:50:39.992084Z  INFO agent::windows_source: chaîne d'encodage reconstruite self.width=624 self.height=472
  → getStats() : frameWidth=624 frameHeight=472, framesDecoded=5

2026-07-29T20:50:46.874668Z  INFO agent: contrôle reçu Resize { version: 3, width: 1600, height: 900 }
2026-07-29T20:50:47.032184Z  INFO agent::windows_source: chaîne d'encodage reconstruite self.width=1584 self.height=892
  → getStats() : frameWidth=1584 frameHeight=892, framesDecoded=6

2026-07-29T20:50:50.909983Z  INFO agent: contrôle reçu Resize { version: 3, width: 480, height: 360 }
2026-07-29T20:50:51.068447Z  INFO agent::windows_source: chaîne d'encodage reconstruite self.width=500 self.height=352
  → getStats() : frameWidth=500 frameHeight=352, framesDecoded=7
```

**Écart demandé/obtenu** : ~16 px en largeur et ~8 px en hauteur de moins
que la taille demandée à chaque étape (640→624, 1600→1584, 480→500 fait
exception dans l'autre sens — voir ci-dessous). Cohérent avec les bordures
de la fenêtre Firefox capturée (`window::client_rect_on_screen` lit la zone
CLIENTE, sous la barre de titre/bordures du système, pas la taille externe
demandée à `resize_window`) et avec l'alignement pair imposé par
`WindowsSource::resize` (`width.max(160) & !1`) — sans rapport avec C1, un
effet de méthode déjà documenté dans le commentaire de `resize()`. Pour
480×360, `500` dépasse la demande : la fenêtre a une largeur minimale
propre à Firefox/Windows en dessous de laquelle `resize_window` ne peut pas
descendre malgré la demande, plancher indépendant du plancher applicatif
`width.max(160)`.

**Verdict, sans arrondir** : **C1 est comportementalement fermé.** La
taille encodée suit le redimensionnement du viewport dans les deux sens —
HAUSSE (624×472 → 1584×892) et BAISSE (1584×892 → 500×352) — sur la même
session WebRTC continue, ce qui est précisément le cas que corrige `3f02545`
et que l'ancien code (taille conservée en valeur absolue) aurait échoué à
la deuxième étape. Aucune ligne `taille d'encodage changée` n'étant apparue,
la mesure ne dit rien sur l'interaction C1 × congestion (barreau réduit
puis redimensionnement) — ce cas n'a pas été exercé ici, faute de lien
dégradé pendant cette session ; il reste couvert uniquement par la lecture
de code de la re-revue (voir le raisonnement tracé dans le ledger de
progression, section « RE-REVUE DE LA VAGUE »).

**Moyen employé pour déclencher le redimensionnement** : CDP
`Emulation.setDeviceMetricsOverride` sur le viewport de la page (pas un
redimensionnement de fenêtre de navigateur physique — Chrome headless n'a
pas de fenêtre réelle à redimensionner). Choisi plutôt qu'un appel direct à
l'émission du message de contrôle depuis la console de la page : cette
méthode exerce le vrai `ResizeObserver` de production sur le vrai élément
`#remote`, pas une simulation du message qu'il produit.
