# Chantier C — Adaptation réseau et traversée NAT

**Date** : 29 juillet 2026
**Statut** : Spécification validée, prête à planifier
**Portée** : Rendre le pipeline utilisable sur un lien quelconque — asservir le
débit et la résolution à ce que le réseau porte réellement, et joindre le pair
même derrière NAT

---

## 1. Objectif

Le chantier 0 a porté le pipeline à ses cibles **sur réseau local** : 62,6 i/s
et une latence médiane de 47,9 ms (`plans/2026-07-28-debit-latence.md`). Ces
chiffres sont obtenus sur gigabit, avec un débit d'encodage **fixé à 12 Mb/s par
la variable d'environnement `BITRATE`**, et une connectivité directe par
candidats hôtes sur le même sous-réseau.

Aucune de ces trois conditions ne tient sur internet. Ce chantier lève les
trois :

1. **Le débit ne s'adapte pas.** Rien ne lit la rétroaction du pair ; l'encodeur
   émet à 12 Mb/s quoi qu'il arrive. Sur un lien qui n'en porte pas 12, la file
   du goulot enfle jusqu'à rendre l'image et la latence inexploitables.
2. **La résolution ne s'adapte pas.** Sous contrainte, il n'existe aucun moyen
   de sacrifier des pixels plutôt que de la qualité.
3. **Le pair n'est pas joignable hors du réseau local.** `webrtc.ts:161`
   construit la `RTCPeerConnection` avec `iceServers: []`, et l'agent ne déclare
   qu'un candidat hôte (`transport.rs:477`). Deux machines qui ne se voient pas
   directement ne se connectent jamais.

Ce chantier reprend le §5 C du cadrage jeu
(`2026-07-28-support-jeux-design.md`), amendé par ce que la lecture du code a
établi (§3 ci-dessous).

---

## 2. Décisions actées

1. **Périmètre large, TURN compris.** L'adaptation et la traversée NAT sont
   traitées dans le même chantier. Elles restent deux volets séparables, et le
   plan les séquencera comme tels.
2. **Banc reproductible d'abord, lien réel ensuite.** Le réglage et la recette
   se font sur `netem` posé sur l'hôte, à profils nommés. Une passe finale sur
   un lien réel confirme, sans servir de base de réglage.
3. **TURN des deux côtés.** L'agent devient client TURN, pas seulement le
   navigateur. Le déploiement ne dépend donc pas d'une redirection de port ni
   d'une IP publique côté agent.
4. **La cadence n'est jamais dégradée automatiquement.** Sous contrainte, on
   baisse le débit, puis la résolution. La cadence est ce qui rend un jeu
   jouable ; sous le dernier barreau, on annonce l'insuffisance du lien au lieu
   de continuer à dégrader.
5. **Toute panne dégrade en le disant, aucune ne tue la session.** Règle déjà
   suivie par le transport (`échec d'envoi UDP, ignoré`) et par le chantier
   audio.
6. **La logique décidable va dans des modules purs**, sans Windows, sans socket,
   sans str0m — testables sur Linux. Même patron que `rebuild.rs`,
   `geometry.rs`, `clock.rs`.

---

## 3. État de départ réellement constaté

Relevé par lecture du code et des sources des dépendances le 29/07/2026. Cette
section **amende le §5 C du cadrage jeu**, qui décrivait plusieurs de ces points
à l'envers.

### 3.1 Ce qui est déjà acquis

| Item du cadrage | État réel |
| --- | --- |
| Keyframe sur demande (PLI/FIR → IDR) | **Fait.** `Event::KeyframeRequest` → `VideoSource::request_keyframe` → `CODECAPI_AVEncVideoForceKeyFrame` (`transport.rs:1175`, `encode.rs:952`), avec test d'intégration contre un vrai pair str0m |
| Pacing des PTS sur horloge réelle | **Fait** hors chantier (commit `857af65`) |
| `wallclock` des Sender Reports | **Fait** au chantier A (commit `733558e`) |
| NACK / RTX | **Probablement déjà négocié** : str0m écrit `a=rtcp-fb nack`, `nack pli` et `a=rtpmap … rtx/90000` dans sa réponse, et Chromium les offre. **À constater par mesure avant de construire quoi que ce soit** — c'est peut-être un non-travail |
| FEC in-band Opus | **Activé mais inerte** — et le diagnostic de cette ligne était INCOMPLET, voir l'amendement du §5.6 : la cause première n'était pas le pourcentage de perte à zéro, mais `RESTRICTED_LOWDELAY` qui force CELT seul, où LBRR n'existe pas |
| Support BWE de str0m | **Disponible**, contrairement à ce que le cadrage donnait pour incertain : `RtcConfig::enable_bwe(Option<Bitrate>)`, `Event::EgressBitrateEstimate(BweKind::Twcc \| Remb)`, `Rtc::bwe().set_desired_bitrate()`. Rien n'est câblé (`transport.rs:467` ne l'appelle pas) |
| Statistiques RTCP | **Déjà émises et jetées.** `set_stats_interval(Some(1 s))` est configuré (`transport.rs:471`), donc `Event::MediaEgressStats` — qui porte `rtt` et la fraction de perte des Receiver Reports — tombe chaque seconde dans le `_ => {}` de `handle_event` (`transport.rs:1191`) |

### 3.2 Deux erreurs de conception évitées par la lecture

**`WindowsSource::resize` ne convient pas à la résolution adaptative.** Cette
méthode redimensionne la *vraie fenêtre Windows* (`window::resize_window`,
`windows_source.rs:178`) : c'est le chemin du redimensionnement de viewport
demandé par l'utilisateur. L'employer sous contrainte réseau rétrécirait la
fenêtre du jeu parce que le lien faiblit.

Le bon point d'insertion existe déjà : la chaîne d'encodage contient un **Video
Processor MFT** (`create_color_converter`, `encode.rs:1128`) qui convertit
BGRA→NV12 à taille constante — le recadrage amont se fait par
`CopySubresourceRegion` (`capture.rs:288`), qui ne met pas à l'échelle. Cette
MFT, elle, sait le faire. Réduire la résolution d'encodage revient à lui donner
un type de sortie plus petit : la fenêtre garde sa taille, seul le flux
transporté maigrit. **Aucun composant nouveau.**

**`is::stun` ne suffit pas à émettre une requête Allocate.** Le crate ICE de
str0m (`is 0.10`) porte presque tout le vocabulaire TURN — builder `.allocate()`,
`.refresh()`, `.create_permission()`, `.channel_bind()`, `.send()`, `.data()`, et
lectures `xor_relayed_address()`, `lifetime()`, `channel_number()`, `realm()`,
`nonce()`, `error_code()`. Mais **l'attribut `REQUESTED-TRANSPORT` (0x0019) est
absent de sa table** (elle s'arrête à `NETWORK-COST`), et une requête Allocate
sans lui est rejetée en 400 par tout serveur conforme. Ses tests `Allocate`
bouclent sur son propre sérialiseur et ne prouvent donc rien sur
l'interopérabilité — la requête de test ne contient elle-même aucun
`REQUESTED-TRANSPORT`.

### 3.3 Trois propriétés de `is::stun` qui décident du partage

- **Les attributs inconnus sont ignorés silencieusement** (`_ => {}` dans
  `Attributes::parse`) : lire une réponse coturn est sûr même si elle porte des
  attributs hors de sa table.
- **Une réponse `Binding` sans MESSAGE-INTEGRITY est rejetée** (« No message
  integrity in incoming STUN binding reply ») : c'est un parseur taillé pour les
  contrôles ICE. Un serveur STUN public ordinaire lui serait illisible.
- **Cette restriction ne s'applique qu'à `Binding`.** La réponse Allocate de
  coturn porte *à la fois* `XOR-RELAYED-ADDRESS` et `XOR-MAPPED-ADDRESS`, elle
  est authentifiée, et sa méthode est `Allocate`. **Un seul échange donne donc
  les deux candidats — relayé et réflexif — sans serveur STUN séparé.**

### 3.4 Contrainte de séquence : pas de trickle ICE

`webrtc.ts:213` attend la fin de la collecte ICE et envoie **une offre unique** ;
la réponse est une chaîne SDP unique. Il n'existe aucun mécanisme de candidat
tardif. Conséquence directe : **l'agent doit avoir alloué son relais avant de
répondre à l'offre**, sinon le candidat relayé n'apparaît dans aucun SDP.

---

## 4. Architecture

Deux modules purs, trois câblages.

```
  observations                décisions                 application
  ───────────                 ─────────                 ───────────
  Event::EgressBitrateEstimate ┐
  Event::MediaEgressStats      ├─► congestion.rs ──► débit ──► ICodecAPI (à chaud)
    (rtt, fraction de perte)   │   (pur, sans IO)    résolution ► VideoProcessor MFT
                               ┘                     perte % ──► opus set_packet_loss_perc

  socket UDP ◄──► turn.rs (pur, sans IO) ◄──► transport.rs ──► str0m
                  Allocate/Refresh/Permission          Transmit.source décide
                  ChannelData (en-tête 4 o.)           direct ou relayé
```

`congestion.rs` et `turn.rs` ne touchent ni socket, ni Windows, ni str0m : ils
consomment des observations et rendent des décisions ou des paquets à émettre.
C'est ce qui les rend testables sur Linux, sans VM et sans réseau.

`Transmit.source` porte déjà l'adresse locale d'émission
(`str0m_proto::net::Transmit`, documentée « The IP could come from a local socket
or relayed over a TURN server ») : c'est la clé de routage direct/relayé, sans
rien inventer.

---

## 5. Volet 1 — Le contrôleur de congestion

### 5.1 Interface

Module `agent/src/congestion.rs`, sans `unsafe`, sans dépendance Windows.

```rust
pub struct Observation {
    pub estimate: Option<Bitrate>,  // Event::EgressBitrateEstimate (TWCC ou REMB)
    pub rtt: Option<Duration>,      // MediaEgressStats.rtt
    pub loss: Option<f32>,          // MediaEgressStats, fraction 0..1
    pub at: Instant,
}

pub struct Decision {
    pub video_bitrate: u32,
    pub encode_size: (u32, u32),
    pub opus_loss_perc: i32,
}

impl Controller {
    pub fn observe(&mut self, o: Observation) -> Option<Decision>;
}
```

`observe` rend `None` tant que rien ne change — le cas courant. Toute la
politique tient dans cette fonction, et tous ses cas se testent avec des
`Instant` fabriqués.

### 5.2 Ordre des sacrifices

1. **Le débit d'abord.** Réglage continu, sans coût :
   `ICodecAPI::SetValue(CODECAPI_AVEncCommonMeanBitRate)` à chaud. Que
   `SetValue` fonctionne à chaud sur ce pilote est déjà établi par
   `request_keyframe`, qui écrit `AVEncVideoForceKeyFrame` sur le même objet en
   cours de session.
2. **La résolution ensuite**, quand le débit disponible tombe sous ce que la
   résolution courante exige pour rester regardable. Coûte un type de sortie
   neuf sur le Video Processor MFT et une image clé.
3. **La cadence, jamais automatiquement** (décision 4 du §2). Sous le dernier
   barreau, le contrôleur cesse de dégrader et déclare le lien insuffisant ;
   l'indicateur le dit à l'utilisateur. C'est la dégradation gracieuse
   **annoncée** du §3 du cadrage jeu.

### 5.3 Échelle de résolutions

Quatre barreaux dérivés de la taille source, par diviseurs **1 · 1,25 · 1,5 ·
2**, arrondis au pixel pair (la contrainte `& !1` existe déjà dans
`windows_source.rs:151`). Chaque barreau porte un débit minimal en dessous
duquel il devient laid ; on retient le barreau le plus haut que le débit
disponible finance.

Les valeurs de ces minimums sont un **réglage à établir sur le banc**, pas une
constante devinée : le plan les fixera à partir des profils netem, et la spec
n'y met pas de chiffre qu'aucune mesure ne soutiendrait.

### 5.4 Hystérésis

Asymétrique, et bornée par un temps de séjour :

| Transition | Condition |
| --- | --- |
| Descendre d'un barreau | 2 s consécutives sous le minimum du barreau courant |
| Remonter d'un barreau | 10 s consécutives au-dessus du minimum du barreau supérieur **majoré de 20 %** |
| Quelle que soit la transition | 5 s minimum depuis le dernier changement de résolution |

Descendre vite, remonter lentement. Sans cela, une estimation qui oscille fait
battre l'encodeur, et chaque battement coûte une reconstruction du type de
sortie et une image clé.

Le débit, lui, s'applique dès qu'il s'écarte de plus de 10 % de la valeur
appliquée, au plus une fois par seconde.

### 5.5 Partage du tuyau

```
video = estimate × 0,9 − audio(128 kb/s) − surcoût de relais si le chemin est relayé
```

La marge de 10 % laisse la place aux retransmissions RTX et aux paquets de
sondage que le sous-système BWE émet pour tester à la hausse.

### 5.6 Perte → Opus

La fraction de perte lissée, en pourcentage, bornée à **[0, 25]**, écrite dans
`set_packet_loss_perc`. C'est ce qui réveille le FEC in-band aujourd'hui inerte
(§3.1). Au-delà de 25 %, la redondance coûte plus de débit qu'elle n'en sauve.

> **AMENDEMENT DU 29/07/2026 — ce paragraphe était insuffisant, et le plafond
> à 25 juste pour la bonne raison.**
>
> Alimenter `set_packet_loss_perc` ne suffisait PAS à réveiller le FEC. La cause
> première était `Application::LowDelay`, choisi au chantier A pour la latence,
> qui force `MODE_CELT_ONLY` (`opus_encoder.c:1349`) où `decide_fec` retourne 0
> sans condition (`opus_encoder.c:721`) : LBRR n'existe que dans SILK. Mesuré, la
> sortie encodée était **bit à bit identique** avec et sans perte déclarée.
> L'application a été basculée en `Application::Audio` — arbitrage assumé de
> 4 ms de pré-délai contre la résilience, voir l'amendement de
> `2026-07-28-audio-design.md`.
>
> Le plafond à 25 se trouve confirmé par libopus lui-même :
> `opus_encoder.c:734` borne l'effet utile par `silk_min(PacketLoss_perc, 25)`.
> Au-delà, la bibliothèque ignore la valeur.
>
> **Et le critère de vérification que ce document sous-entendait était faux** :
> à débit cible fixe, LBRR ne s'ajoute pas aux octets, il les redistribue
> (`opus_encoder.c:751`, tables de débit distinctes selon que le FEC est codé).
> La preuve valable est un décodage, pas une taille : sur un décodeur neuf sans
> historique, `decode(paquet, sortie, fec=true)` rend 0,00 d'énergie sans perte
> déclarée contre 585,02 avec.

### 5.7 Effet de bord du BWE au démarrage — à mesurer, pas à supposer

Activer le BWE change le démarrage. `enable_bwe(Some(initial))` part d'une
estimation modeste et sonde à la hausse vers `set_desired_bitrate(plafond)`.
Aujourd'hui, l'encodeur démarre directement à `BITRATE` (12 Mb/s), et c'est dans
ces conditions que la recette relève 62,6 i/s.

La recette inscrira donc une **non-régression LAN explicite** : après activation
du BWE, le débit doit rejoindre le plafond en un délai borné sur gigabit, et les
chiffres de la recette doivent tenir. Si la rampe coûte trop cher au démarrage,
l'estimation initiale devient un réglage mesuré, pas une valeur devinée.

`BITRATE` conserve son rôle mais change de sens : il devient le **plafond**
passé à `set_desired_bitrate`, plus le débit de repli quand aucune estimation
n'arrive (§8).

---

## 6. Volet 2 — Client TURN et routage

### 6.1 Partage du travail

`turn.rs` **sérialise lui-même** les quatre requêtes dont il a besoin (Allocate,
Refresh, CreatePermission, ChannelBind) : jeu d'attributs connu,
MESSAGE-INTEGRITY en HMAC-SHA1, FINGERPRINT omis (facultatif en TURN). Il **lit**
les réponses avec `is::stun::StunMessage::parse`, qui les couvre entièrement
(§3.3).

Écrire un sérialiseur de quatre messages est petit et testable ; réécrire un
parseur d'attributs, le XOR des adresses et la vérification d'intégrité serait
dupliquer du code éprouvé déjà présent dans l'arbre de dépendances.

**Deux dépendances nouvelles, minimales** :

- `is = "0.10"` en dépendance directe, avec ses fonctionnalités par défaut (qui
  fournissent `DefaultSha1HmacProvider`). En direct plutôt que par le module
  `str0m::ice`, qui est `#[doc(hidden)]`. Version alignée sur celle que
  `Cargo.lock` verrouille déjà comme dépendance transitive de `str0m` — même
  précédent que `windows-core` dans `agent/Cargo.toml`.
- Un MD5 : la clé longue durée vaut `MD5(user:realm:pass)` (RFC 5766) et aucun
  MD5 n'est dans l'arbre. MD5 n'est ici qu'une dérivation de clé imposée par la
  norme, pas une primitive de sécurité.

### 6.2 Machine à états

Sans entrées-sorties, sur le patron de `rebuild.rs` :

```rust
impl TurnClient {
    fn handle_packet(&mut self, from: SocketAddr, data: &[u8]) -> Option<Relayed<'_>>;
    fn poll_transmit(&mut self) -> Option<(SocketAddr, Vec<u8>)>;
    fn poll_timeout(&self) -> Option<Instant>;
    fn allocation(&self) -> Option<Allocation>;  // adresse relayée + réflexive
}
```

`handle_packet` rend `Some(Relayed { peer, data })` quand le paquet était une
donnée relayée à désencapsuler — l'appelant la présente alors à str0m comme
venant de `peer` — et `None` quand c'était un message de service TURN, déjà
absorbé par la machine à états.

Repos → Allocate nu → **401** porteur du realm et du nonce → Allocate signé →
Allouée. Rafraîchissement à la moitié du bail. Le **438 « stale nonce »** réémet
avec le nouveau nonce plutôt que d'échouer — c'est le cas d'erreur réellement
rencontré, coturn faisant tourner ses nonces.

Chaque pair découvert obtient un **ChannelBind** ; l'encapsulation ChannelData
coûte 4 octets par paquet, contre 36 pour une Send indication.

### 6.3 Routage

`transport.rs` n'a que trois points de contact avec le socket (`:561`, `:1116`,
et la réception). Ils passent derrière un `Transport` qui possède le socket et,
éventuellement, le client TURN :

- **Sortant** : `transmit.source == adresse relayée` → encapsuler en ChannelData
  et envoyer au serveur ; sinon `send_to` direct, exactement comme aujourd'hui.
- **Entrant** : provenance ≠ serveur TURN → chemin actuel inchangé. Provenance =
  serveur TURN → **les deux bits de poids fort du premier octet** départagent
  STUN (`00`) de ChannelData (`01`) ; le premier va au client TURN, le second est
  désencapsulé et présenté à str0m comme venant du pair.

### 6.4 Séquence de démarrage

Conséquence de l'absence de trickle (§3.4) : **l'allocation précède la réponse à
l'offre**, comme une étape de démarrage bornée par un délai. Si elle échoue ou
traîne au-delà de ce délai, l'agent répond avec les candidats disponibles plutôt
que de refuser la session.

Ce délai est fixé à **2 s** au départ. Il doit rester très inférieur aux 15 s
au bout desquelles le client abandonne l'attente de la réponse
(`ANSWER_TIMEOUT_MS`, `webrtc.ts:27`), tout en laissant place aux deux
aller-retours qu'exige une allocation authentifiée (Allocate nu → 401 → Allocate
signé). À confirmer contre un coturn réel, hors boucle locale.

Ajouter le trickle ICE serait plus élégant — il supprimerait ce blocage et
permettrait de rattraper une allocation tardive — mais il touche le protocole de
signaling et le client. **Hors périmètre**, noté ici pour ne pas être redécouvert
comme un oubli.

### 6.5 Identifiants et déploiement

coturn en mode `use-auth-secret` : le serveur de signaling délivre à chaque
session un couple éphémère
(`username = expiration:session`, `password = HMAC-SHA1(secret, username)`).
Aucun mot de passe statique ne descend dans le client, et les identifiants
expirent avec la session.

Les deux pairs sont déjà connectés au signaling par WebSocket et s'y annoncent
par un message de rôle (`{ role: 'client' | 'agent', session }`,
`webrtc.ts:209`). C'est la réponse à cette annonce qui porte désormais la
configuration ICE — même canal, même instant, pour les deux côtés :

- **navigateur** : `webrtc.ts:161` cesse de construire la `RTCPeerConnection`
  avec `iceServers: []` et emploie ce qu'il a reçu ;
- **agent** : `signaling.rs` transmet l'adresse du serveur et les identifiants à
  `Session::new`, qui les passe au client TURN.

Le secret partagé ne quitte jamais le serveur de signaling : ni le navigateur ni
l'agent ne le voient, seulement le mot de passe dérivé et daté.

---

## 7. Volet 3 — Client web

### 7.1 Latence de restitution

`playoutDelayHint = 0` sur le `RTCRtpReceiver` vidéo, dans `webrtc.ts`.

Ce n'est pas gratuit. La mesure actuelle décompose la latence en `RTT/2 +
tampon de gigue` (`stats.ts:90`), et c'est ce tampon qu'on rabote : sur un lien
stable il rend des millisecondes, sur un lien qui gigue il rend du saccadement.
**On le mesure sous chaque profil netem** plutôt que de l'activer par foi.
Chromium seulement ; ailleurs l'attribut est ignoré, sans dommage.

### 7.2 Indicateur de qualité

Ce que `stats.ts` affiche aujourd'hui, c'est ce que le *navigateur* observe. Il
ne sait rien de ce que l'agent a décidé — ni qu'il est au dernier barreau, ni
qu'il n'a jamais reçu la moindre estimation. L'agent doit donc le dire, par un
nouveau message de contrôle :

```
AgentControl::Link {
    bitrate,
    encode_size,
    quality: Bonne | Degradee | Insuffisante,
    adaptation: Active | Indisponible,
}
```

`quality` est une énumération à trois valeurs, pas un booléen : les trois états
du tableau ci-dessous sont distincts, et « dégradé » ne se déduit pas de
« insuffisant » par une négation. `adaptation` en est indépendant — une session
sans estimation BWE peut très bien tourner en `Bonne` sur un lien large.

`CONTROL_VERSION` passe de **2 à 3**, avec les trois côtés à tenir alignés :
`proto/src/control.rs`, `proto/ts/control.ts`, `vectors.json`.

L'affichage se résume à trois états :

| État | Signification |
| --- | --- |
| **Bon** | Barreau le plus haut, perte et RTT sous seuils |
| **Dégradé** | Résolution réduite — l'utilisateur doit savoir pourquoi l'image a molli |
| **Insuffisant pour le jeu nerveux** | Plancher atteint : avertissement explicite exigé par le §3 du cadrage jeu |

Un jeu qui devient injouable sans que rien ne l'annonce est un défaut ; un jeu
qui devient injouable **en le disant** est un lien saturé.

---

## 8. Gestion des erreurs

Toutes sur le même principe : **dégrader en le disant, jamais tuer la session.**

| Panne | Conduite |
| --- | --- |
| Aucune estimation BWE (TWCC non négocié) | Débit fixe `BITRATE`, journal **une seule fois**, indicateur « adaptation indisponible » — surtout pas un silence qui ressemble à « tout va bien » |
| L'encodeur refuse le débit à chaud | On garde le débit courant, journal une fois, l'adaptation continue par la résolution |
| Le Video Processor refuse le nouveau type de sortie | On reste au barreau courant ; la session vit |
| L'allocation TURN échoue ou dépasse son délai | On répond avec les candidats restants : sans relais plutôt que sans session |
| Le bail TURN meurt en cours de session | ICE bascule sur une autre paire si elle existe ; journal explicite sinon |
| Le serveur de signaling ne délivre pas d'identifiants TURN | Session sans relais, journalisée ; le cas local continue de fonctionner |

---

## 9. Banc d'essai

Un script `scripts/netem.sh` sur l'hôte, à **profils nommés**, qui pose la
dégradation **dans les deux sens**. Le sens agent→navigateur — celui qui porte la
vidéo — exige une redirection `ifb` avec `tc mirred` : l'egress seul ne le
couvre pas.

| Profil | Débit | Latence | Gigue | Perte | Rôle |
| --- | --- | --- | --- | --- | --- |
| `lan` | — | — | — | — | Témoin, aucune dégradation — sert la non-régression du §5.7 |
| `adsl` | 8 Mb/s | 30 ms | 5 ms | 0 % | Débit confortable mais borné, sous le plafond de 12 Mb/s |
| `4g` | 10 Mb/s | 60 ms | 20 ms | 1 % | Gigue et perte : le cas qui exerce le FEC et le tampon |
| `congestionné` | 3 Mb/s | 100 ms | 20 ms | 3 % | Force au moins une descente de barreau |
| `effondrement` | 500 kb/s | 150 ms | 30 ms | 5 % | Sous le plancher : doit déclencher l'annonce d'insuffisance |

Ces valeurs sont des **points de départ**, choisis pour exercer chacun un
mécanisme distinct — pas des mesures. Le plan les ajuste si un profil n'exerce
pas ce qu'il vise, et consigne l'ajustement. Un profil est une ligne de commande,
pas une manipulation à refaire de mémoire : c'est ce qui rend deux réglages
comparables.

Une **passe finale sur un lien réel** (ChromeOS via Pomerium depuis l'extérieur,
ou partage de connexion mobile) confirme. Elle ne sert pas de base de réglage :
elle n'est pas reproductible.

---

## 10. Stratégie de test

### Unitaires — sur Linux, sans VM ni réseau

C'est ce que le découpage en modules purs achète.

- **`congestion.rs`** : chaque barreau ; l'hystérésis dans les deux sens ; le
  temps de séjour ; la déclaration de plancher ; la conversion perte→Opus et son
  plafond à 25 % ; le cas « aucune estimation reçue ».
- **`turn.rs`** : la machine à états complète ; le 401 et la réémission signée ;
  le 438 nonce périmé ; le rafraîchissement à mi-bail ; l'encapsulation
  ChannelData ; le démultiplexage STUN/ChannelData sur les deux bits de poids
  fort.

### Intégration — dans `transport.rs`

Le fichier a déjà le patron : un vrai pair str0m en boucle locale (voir les
tests `write_frame_annonce_l_instant_de_capture…` et celui du PLI). On y ajoute :

- le routage — un `Transmit` dont la source est l'adresse relayée part
  encapsulé, les autres non ;
- la présence du candidat relayé dans la réponse SDP.

`turn.rs` étant sans entrées-sorties, **un faux serveur TURN en mémoire suffit**
— pas de coturn dans la boucle de test.

### Recette — sur la VM, par profil netem

Pour chaque profil : le débit suit-il, la résolution descend-elle et remonte-t-elle
sans battre, l'indicateur dit-il vrai, la latence tient-elle. Instrumentée comme
les recettes précédentes (`client/verify-webrtc.mjs`, `client/recette/harness.mjs`,
compteurs `getStats()` réels d'un Chrome piloté par CDP).

### Non-régression, qui prime sur le reste

Sur le profil `lan`, la recette doit continuer à relever **62,6 i/s et une
médiane à 47,9 ms**. Un chantier d'adaptation réseau qui abîme le cas nominal a
échoué, quelle que soit son élégance sur les liens dégradés.

### Mesure préalable, avant toute construction

Constater ce que NACK et RTX font **déjà** (§3.1), sous profil `4g` ou
`congestionné` : compteurs `nackCount` / `retransmittedPacketsSent` côté
`getStats()`, et `MediaEgressStats.nacks` côté agent. Si la résilience est déjà
là, ne rien construire et l'écrire.

---

## 11. Hors périmètre

- **Trickle ICE** (§6.4) — supprimerait le blocage au démarrage, mais touche le
  protocole de signaling et le client.
- **TURN sur TCP ou TLS.** Seul UDP est couvert. Les réseaux qui bloquent tout
  UDP sortant — le cas qui motive souvent TURN/TCP sur 443 — ne le seront pas.
  La boucle de transport est UDP par construction ; y greffer un transport en
  flux est un chantier à part. Limite **explicite, pas un oubli**.
- **Simulcast et couches temporelles.** Une seule piste, un seul encodage.
- **WebCodecs** en remplacement du décodeur du navigateur — option mentionnée par
  la spec produit §5②, non traitée ici.
- **Adaptation de la cadence** (décision 4 du §2).
- **Coexistence multi-fenêtres.** Le chantier D changera la topologie WebRTC (une
  `RTCPeerConnection` par fenêtre) ; le contrôleur devra alors partager une
  estimation entre plusieurs flux. Le présent chantier traite une session à un
  flux vidéo, et `congestion.rs` reste un module à instancier par session — ce
  qui laisse la porte ouverte sans la franchir.

---

## 12. Répercussions sur les documents existants

`2026-07-28-support-jeux-design.md` §5 C doit être amendé sur quatre points, tous
établis au §3 ci-dessus :

1. Le support BWE de str0m n'est plus « à vérifier, prérequis pas acquis » : il
   est disponible et l'API est nommée.
2. La keyframe sur demande n'est plus à faire : elle est livrée et testée.
3. Le FEC in-band Opus est activé mais inerte — ce qui reste à faire est de
   l'alimenter en pourcentage de perte, pas de l'activer.
4. « Réduire la résolution d'encodage » ne passe pas par `WindowsSource::resize`,
   qui redimensionne la fenêtre réelle, mais par le type de sortie du Video
   Processor MFT.
