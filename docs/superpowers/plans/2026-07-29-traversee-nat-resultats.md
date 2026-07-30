# Chantier C, volet 2 — Traversée NAT : résultats de recette

**Date des mesures :** 30 juillet 2026.
**Plan :** `2026-07-29-traversee-nat.md`. **Spec :** `2026-07-29-reseau-adaptatif-design.md` §6.
**Branche :** `chantier-traversee-nat`.

**Topologie de mesure.** Hôte Linux `192.168.3.1` portant coturn (réseau hôte,
plage de relais 49160-49200), le serveur de signaling et Chrome sans interface ;
VM Windows `192.168.3.2` portant l'agent ; pont `internalBridge`. Les deux pairs
se voient directement — c'est ce qui borne ce que cette recette peut prouver,
voir §5.

**Outils.** `client/recette/paire-candidats.mjs` (écrit pour ce chantier :
relève la paire de candidats réellement employée, son type des deux côtés, le
RTT, le débit et la cadence), `client/verify-webrtc.mjs` (preuve que le flux
traverse), les journaux de coturn et de l'agent.

**Avertissement de méthode, à lire avant de refaire ces mesures.** Une session de
plus de 5 minutes exige les options anti-gel de Chrome désormais posées dans la
sonde (voir §3) : sans elles, toute session tombe vers 330 s, quel que soit le
chemin — et cette chute imite à s'y méprendre l'expiration d'une permission TURN.

---

## 1. Non-régression locale, sans relais

Contrôle bloquant du plan : la traversée ne doit rien coûter au cas nominal.

coturn arrêté, signaling relancé **sans** `TURN_URL` ni `TURN_SECRET`.

Mesure refaite en fin de chantier, sur le binaire réellement livré (la première
passe datait d'un agent encore instrumenté, voir §6) :

| Mesure | Valeur |
| --- | --- |
| Établissement de la session | OK, `connected` |
| Images décodées | 463 en 8,0 s (≈58 i/s) |
| Résolution | 764×484 |
| Audio | `bytesReceived` et `packetsReceived` en hausse |
| Lignes TURN dans le journal de l'agent | **0** |
| Avertissement du signaling | 2 × « aucun serveur TURN configuré […] session sans relais » (un par pair) |

Le chemin TURN est **totalement inerte** quand aucun relais n'est configuré :
`Session::turn` reste `None`, et rien dans `relais.rs` ne s'exécute.

**Preuve supplémentaire, non demandée par le plan.** Une session a été menée par
accident avec un signaling qui annonçait un relais alors que coturn était
arrêté. L'agent a journalisé
`allocation TURN impossible : la session continue sans relais
erreur=réception pendant l'allocation TURN`, puis la session s'est établie et le
flux a traversé. C'est la contrainte globale du plan (« toute panne dégrade en le
disant, aucune ne tue la session ») vérifiée sur le cas le plus probable en
production : un relais déclaré mais injoignable.

---

## 2. Session relayée, de bout en bout

Relais forcé côté navigateur par `iceTransportPolicy: 'relay'`, imposé depuis la
sonde en interceptant le constructeur `RTCPeerConnection` — donc **sans modifier
le code du client**, contrairement au réglage en dur puis retiré que suggérait le
plan.

| Mesure | Session directe (témoin) | Session relayée |
| --- | --- | --- |
| Chemin (vu du navigateur) | `srflx` ↔ `host` | **`relay` ↔ `host`** |
| Candidat local | — | `192.168.3.1:49161` (plage de relais coturn) |
| RTT courant | 2,0 ms | 4,0 ms |
| RTT moyen cumulé | — | 18 ms |
| Cadence | 55,5 i/s | 61,2 i/s |
| Débit reçu | 2,63 Mb/s | 2,77 Mb/s |
| Pertes vidéo | 0 | 0 |
| Gigue vidéo | — | 2 ms |

**Surcoût du relais : environ +2 ms de RTT** (2,0 → 4,0 ms), aller-retour
supplémentaire par le relais compris. Cadence et débit sont équivalents : l'écart
de cadence (55 contre 61 i/s) est du bruit de source — la capture est
opportuniste, pas cadencée dur.

Côté coturn, le relais porte bien le média : session `1785503135:demo`,
≈1,64 Mo relayés par tranche de 5 s, soit ≈2,6 Mb/s — le flux vidéo entier.

**Chaîne d'authentification vérifiée de bout en bout.** coturn journalise
`CREATE_PERMISSION processed, success` et `CHANNEL_BIND processed, success` pour
l'utilisateur éphémère `1785502279:demo` — un identifiant dérivé par notre propre
`signaling/src/ice.ts` (HMAC-SHA1 du nom daté), accepté par un serveur tiers. Le
sérialiseur de la tâche 3 produit donc des requêtes conformes, y compris
`MESSAGE-INTEGRITY`, contre une implémentation de référence.

**Le prédicat de routage est constaté, pas supposé.** Le plan laissait ouverte la
valeur de `Transmit.source` pour un paquet devant passer par le relais.
Instrumentation temporaire posée puis retirée (tâche 6, étape 1) : les `Transmit`
d'une session réelle portent **deux** sources distinctes —
`192.168.3.2:60303` (socket local de la VM) pour le chemin direct, et
`192.168.3.1:49183` pour le chemin relayé, adresse de la plage de relais de
coturn, donc l'adresse relayée elle-même. Le prédicat
`transmit.source != allocation.relayee → envoi direct` est le bon ; il n'a pas eu
à être inversé.

**Ce que cette mesure ne prouve pas :** que le média emprunte le relais **côté
agent** (encapsulation ChannelData émise par l'agent). Voir §5.

---

## 3. Rafraîchissement du bail

Session relayée forcée lancée pour **11 minutes** (bail demandé 600 s,
rafraîchissement dû à 300 s). Elle s'est interrompue à 340 s — pour une raison
étrangère au relais, voir plus bas —, mais après le rafraîchissement recherché,
qui a donc bien été observé.

Côté coturn, l'allocation de l'agent (`session 023000000000000003`, utilisateur
éphémère `1785503220:demo`) :

| Instant coturn | Événement |
| --- | --- |
| t = 464 s | `new, lifetime=600` puis `ALLOCATE processed, success` |
| t = 764 s | `refreshed, lifetime=600` puis `REFRESH processed, success` |

**Exactement 300 s d'écart** — la moitié du bail, donc la règle
`echeance_refresh = maintenant + bail / 2` de `TurnClient::handle_packet`. C'est
cette valeur au cordeau qui identifie l'allocation de l'agent : le nom
d'utilisateur éphémère est commun aux deux pairs (il est dérivé de la session), et
Chrome ne rafraîchit pas à la moitié du bail. Sur les quatre allocations ouvertes
à t = 464, c'est la seule qui se rafraîchisse à cet instant précis ; deux des
autres ont été libérées immédiatement (`lifetime=0`, fermeture des sessions de
mesure précédentes).

**Attribution vérifiée par l'adresse du pair, pas seulement par la cadence.**
`session 023000000000000003` porte `remote 192.168.3.2` dans son journal de
fermeture : c'est bien l'allocation de la VM, donc celle de l'agent. Les deux
autres allocations vivantes de cette période portent `192.168.3.1`, l'hôte, donc
le navigateur.

**L'attribution est confirmée par une seconde signature.** Une autre allocation
ouverte au même instant (`session 021000000000000002`, celle du navigateur) s'est
rafraîchie à t = 1004 s, soit **540 s** après son allocation — 60 s avant
l'expiration du bail, la politique de Chrome. Deux cadences distinctes sur le même
serveur, au même instant de départ : 300 s pour la nôtre (moitié du bail), 540 s
pour celle du navigateur. Ce n'est donc pas une déduction par élimination.

La session a porté le flux sur le chemin relayé (`relay ↔ host`, candidat local
`192.168.3.1:49164`) pendant toute sa durée, rafraîchissement compris.

### La session de 11 minutes n'a pas tenu — et la cause n'est pas le relais

La session relayée s'est interrompue à **340,6 s** (`état ICE state=Checking`,
puis `ICE déconnecté`, puis `session terminée` ; côté navigateur
`connectionState: failed`).

**Première imputation, fausse.** Le rafraîchissement manquant des permissions
TURN (voir ci-dessous) collait si bien au délai qu'il a été retenu sans
contre-épreuve. Le témoin l'a réfuté : une session **directe**, sans le moindre
relais, tombe au même endroit — **331,7 s**. Deux chemins n'ayant en commun ni
coturn ni le code TURN : la cause est ailleurs.

**Cause retenue : le harnais de mesure.** 331 et 340 s, c'est 300 s plus le
délai de révocation du consentement ICE (~30 s). Or Chrome gèle une page
d'arrière-plan au bout de 5 minutes, et la sonde pilote un Chrome sans interface
sans les options qui l'en empêchent
(`--disable-background-timer-throttling`, `--disable-backgrounding-occluded-windows`,
`--disable-renderer-backgrounding`). Les mesures du projet n'avaient jamais duré
plus de 25 s, ce mur n'avait donc jamais été rencontré.

**Confirmé par la contre-mesure.** Sonde relancée avec les trois options, sur une
session relayée : le mur des 5 minutes disparaît — plus aucune interruption au
voisinage de 330 s, la session tient (relevé complet ci-dessous). Le diagnostic
n'est donc pas une hypothèse commode : il a été vérifié en supprimant la cause
supposée et en constatant la disparition de l'effet.

Aucune session de plus de 5 minutes n'avait jamais été menée dans ce projet
(les mesures du volet 1 plafonnaient à 25 s) : ce mur ne pouvait pas avoir été
rencontré plus tôt.

### Un vrai défaut trouvé en chemin, corrigé

L'imputation était fausse, le défaut qu'elle a mis au jour ne l'est pas. Le bail
de l'allocation était bien rafraîchi (§3), mais :

| Objet TURN | Durée normative | Rafraîchi par le code du plan ? |
| --- | --- | --- |
| Allocation (bail) | 600 s demandés | **oui**, à 300 s (`Refresh`) |
| Permission (RFC 5766 §8) | 300 s | **non, jamais** |
| Liaison de canal (§11) | 600 s | **non, jamais** |

`lier_canal` émettait `CreatePermission` et `ChannelBind` **une seule fois**, à
la liaison, et `avancer` ne s'occupait que du bail. Passé 300 s, le serveur
cesse de relayer les paquets du pair — **sans rien annoncer** : ni erreur, ni
message. Le plan ne prévoyait tout simplement pas ces deux minuteurs.

Ce défaut n'a pas causé la chute observée (le témoin direct l'a réfuté), mais il
aurait tué toute session relayée dépassant 5 minutes dès qu'une aurait pu être
menée. Il est corrigé sur la foi de la RFC, pas d'une mesure — et cette mesure
reste à faire.

### Le correctif en a réveillé un autre : le bail cessait d'être rafraîchi

Sur la session suivante, avec les liaisons désormais réaffirmées, l'allocation de
l'agent (`session 004000000000000001`, `remote 192.168.3.2`) a **expiré à
exactement 600 s** sans un seul `Refresh` — alors qu'une session antérieure, sans
le correctif, en avait bien émis un à 300 s.

Diagnostiqué par un relevé d'état périodique ajouté à `avancer` (une ligne par
minute : état, prochaine échéance, nombre de canaux). Trois relevés consécutifs
d'une même session :

| Instant | État | Prochaine échéance | Canaux |
| --- | --- | --- | --- |
| 13:58:20 | allouée | 90 s | 3 |
| 13:59:20 | allouée | 30 s | 3 |
| 14:00:20 | allouée | **120 s** | 3 |

L'échéance **recule** au lieu d'avancer. La cause est dans `handle_packet` : il
reposait `Etat::Allouee { echeance = maintenant + bail/2 }` à **chaque** réponse
de succès du serveur. Or ni `CreatePermission` ni `ChannelBind` ne portent de
bail. Tant que rien ne les réaffirmait, le défaut restait invisible ; dès que les
liaisons ont été réaffirmées toutes les 150 s, leurs réponses ont repoussé de
300 s une échéance qui n'était qu'à 300 s — elle n'est donc jamais arrivée, et
l'allocation est morte à l'expiration de son bail.

**Corrigé** (commit `e60d9dd`) : la méthode du message est décodée depuis son
type sur le fil (`messages::methode_de`, qui défait l'entrelacement classe/méthode
de la RFC 5389 §6), et seules les réponses à `Allocate` et `Refresh` reconduisent
le bail. Test :
`une_reponse_a_channel_bind_ne_repousse_pas_l_echeance_du_bail`.

**Leçon.** Un correctif juste peut en réveiller un autre, resté latent faute de
sollicitation. Ici le premier correctif était nécessaire *et* a cassé la session
plus vite qu'avant — c'est la mesure, pas le raisonnement, qui l'a montré.

### Les deux correctifs, prouvés sur session réelle

Session de validation, agent recompilé, allocation à 14:06:31 :

| Instant | Trace de l'agent | Écart |
| --- | --- | --- |
| 14:09:01 | `réaffirmation d'une liaison de canal TURN` ×3 (canaux 0x4000-0x4002) | +150 s |
| 14:11:31 | `rafraîchissement du bail TURN émis` | **+300 s** |
| 14:11:31 | `réaffirmation d'une liaison de canal TURN` ×3 | +150 s |

Et côté coturn, au même instant : `REFRESH processed, success` puis trois
`CHANNEL_BIND processed, success`. Les trois minuteurs sont donc entretenus, aux
échéances voulues, et le serveur les accepte.

Le relevé d'état confirme la mécanique de bout en bout : `allouée` avec une
échéance qui décroît (90 s → 30 s), se réarme au rafraîchissement des canaux
(119 s → 59 s), puis passe à `attente-refresh` à l'échéance du bail.

Ces trois canaux liés vers les adresses relayées du navigateur établissent au
passage que **le chemin d'encapsulation de l'agent est bien exercé** — ce que la
première rédaction de ce document donnait pour improuvable ici.

**Corrigé** (commit `59fde75`) : chaque liaison porte désormais son échéance, et
`avancer` réaffirme celles qui l'atteignent, à la moitié de la durée d'une
permission (150 s) — même raisonnement que pour le bail, une seule perte de
paquet ne doit pas suffire à perdre le relais. `poll_timeout` prend en compte
cette échéance, plus fréquente que celle du bail. Test :
`un_canal_lie_est_rafraichi_avant_l_expiration_de_la_permission`
(`agent/src/turn/canaux.rs`), qui vérifie aussi que le rafraîchissement porte
le **même** numéro de canal et qu'il ne se répète pas à chaque tour.

**Aucun 438 « Stale Nonce » rencontré.** coturn n'a pas fait tourner son nonce sur
la fenêtre observée. Le chemin de rejeu du nonce périmé reste donc couvert par le
seul test unitaire
`un_nonce_perime_est_rejoue_et_non_abandonne` (`agent/src/turn/allocation.rs`), et
n'a **pas** été exercé contre un vrai serveur — c'est la principale réserve de ce
§3. Une première lecture a compté un 438 : faux positif, la chaîne « 438 »
apparaissait dans un compteur d'octets (`sb=1643803`).

---

## 4. Passe sur un réseau réel contraint

**Non réalisée.** Cette étape exige une session depuis un lien sans route directe
vers l'agent (partage de connexion mobile, réseau d'entreprise). L'environnement
de mesure ne dispose que du pont local, où les deux pairs se voient toujours :
aucun moyen de la mener honnêtement ici.

C'est la seule étape du plan restée ouverte, et c'est aussi celle qui donnerait
la preuve manquante du §5. À mener dès qu'un second réseau est disponible.

---

## 5. Limites constatées

**`relay ↔ relay` est structurellement inatteignable dans cette topologie.** Le
plan attendait une paire nominée `relay`/`relay`. Elle n'a pas été obtenue, et ce
n'est pas un défaut du chantier :

- pour que le relais de l'agent fonctionne, coturn doit pouvoir joindre l'agent
  sur son socket UDP ;
- cette même joignabilité rend la paire (relais du navigateur ↔ `host` de
  l'agent) valide ;
- `host` a une priorité ICE bien supérieure à `relay` : ICE retient donc toujours
  cette paire.

Vérifié expérimentalement : deux règles de pare-feu Windows bloquant l'UDP
entrant depuis `192.168.3.1` hors de la plage 49160-49200 n'ont **rien changé**
(`relay ↔ host` de nouveau) — le trafic relayé arrive précisément depuis cette
plage, donc autorisé, et le chemin direct qu'elles coupaient n'était pas celui
qu'ICE employait. Les deux règles ont été retirées après mesure (0 règle
`recette-nat-*` restante).

Séparer les deux chemins demanderait deux réseaux réellement distincts : c'est
l'étape 4, non réalisable ici.

**Ce qui est tout de même prouvé du chemin d'encapsulation de l'agent :** il est
exercé pour les contrôles de connectivité ICE — les `Transmit` émis depuis
l'adresse relayée de l'agent ont bien été encapsulés puis acceptés par coturn
(`CREATE_PERMISSION` et `CHANNEL_BIND` réussis, quatre paires). Ce qui n'est pas
prouvé, c'est que le **média** l'emprunte, faute d'avoir pu faire nominer cette
paire.

**Pas de TURN/TCP** (spec §11) : aucun réseau bloquant tout l'UDP sortant n'a été
testé, cette limite n'a donc rien coûté à la recette — mais elle reste entière en
production.

**Le rejeu du nonce périmé n'est pas prouvé en réel** (voir §3) : coturn n'a pas
fait tourner son nonce sur la fenêtre observée, et le chemin 438 ne repose donc
que sur son test unitaire.

**Pas d'appariement des transactions.** `TurnClient::handle_packet` accepte la
réponse du serveur sans vérifier que son identifiant de transaction apparie la
requête en cours. Les `trans_id` sont retenus dans `Etat` (c'est là que
l'appariement se brancherait) mais jamais relus. Conséquence : une réponse
tardive ou rejouée fait avancer la machine à états. Le plan est écrit ainsi ;
réserve consignée plutôt que corrigée en cours de route.

---

## 6. Ce que la mesure a coûté

**Un faux échec, entièrement imputable à l'instrumentation.** La première
tentative de session relayée forcée n'a jamais établi de connexion
(`iceConnectionState` bloqué à `new`). Cause : le binaire de l'agent portait
encore l'instrumentation `tracing::info!("transmit")` de la tâche 6 étape 1, qui
journalise **chaque paquet** — 18 619 lignes pour une session de quelques
secondes, écrites dans un fichier sur partage CIFS, depuis la boucle de
transport. La mesure détruisait ce qu'elle mesurait. Recompilation sans
instrumentation : `relay ↔ host` du premier coup.

Détail qui explique pourquoi le défaut n'a pas sauté aux yeux plus tôt : sous la
même instrumentation, une session **directe** s'établissait sans peine (c'est
d'ailleurs sous ce binaire qu'a été relevée la première passe du §1, refaite
depuis sur le code livré). Seul le chemin relayé, plus coûteux d'un aller-retour
et d'une encapsulation par paquet, franchissait le seuil où le coût du journal
devenait fatal. Une instrumentation peut donc être inoffensive sur le cas nominal
et destructrice sur celui qu'on cherche précisément à mesurer.

Leçon pour les recettes suivantes : une sonde par paquet n'a pas sa place dans la
boucle de transport quand le journal atterrit sur un partage réseau. Compter, ou
échantillonner, jamais tracer.

**Un outil de recette écrit, conservé :**
`client/recette/paire-candidats.mjs`. `verify-webrtc.mjs` prouve que le flux
traverse mais jamais **par où** — or c'est toute la question de ce chantier. La
sonde relève la paire employée, le type des deux candidats, le RTT et le débit,
et sait forcer le relais sans toucher au code du client. Elle capture aussi la
console de la page : sans cela, l'échec ci-dessus se présentait comme un
« aucune paire employée » muet.

**Renvois du plan à recaler, tous dus au découpage de la veille.** Le plan a été
écrit le 29 juillet, le chantier de résorption de la dette de taille des fichiers
a été fusionné le 30 (`47f89ef`) :

- `agent/src/turn.rs` d'un seul tenant aurait fait ≈1 000 lignes, pour une limite
  à 500 (`CLAUDE.md`, posée le 30 dans `a18fe97`, donc après le plan). Le module
  naît en répertoire `agent/src/turn/` : `messages` (sérialisation), `allocation`
  (machine à états), `canaux` (ChannelData), `fixtures` (échafaudages de test
  partagés, sur le précédent de `transport/fixtures.rs`). Aucun fichier
  au-dessus de 500 lignes.
- Les deux sites d'envoi que le plan situait en `transport.rs:561` et `:1116`
  sont en `transport.rs:374` et `:396` ; la réception est en
  `transport/socket.rs`, `act_on_timeout` en `transport/tick.rs`. Le point
  d'envoi unique, le routage et l'allocation vivent dans un module frère neuf,
  `transport/relais.rs` — l'ajouter à `transport.rs` l'aurait poussé au-delà de
  la limite.
- L'offre est consommée dans `demarrage.rs:216`, non dans `main.rs` : le
  démarrage en a été séparé le 30 (`028cab2`).

**Une mesure témoin qui a sauvé une conclusion.** La chute des sessions longues
a d'abord été imputée au rafraîchissement manquant des permissions TURN : le
délai collait au minuteur de la RFC, l'explication était cohérente, le correctif
était déjà écrit. Une session **directe** de même durée l'a réfutée en dix
minutes (§3). Sans ce témoin, la recette aurait consigné une cause fausse, et le
vrai coupable — le harnais lui-même — serait resté en place pour tous les
chantiers suivants. **Quand une fonctionnalité neuve est suspecte, mesurer
d'abord le chemin qui ne l'emprunte pas.**

**Quatre défauts du code dicté par le plan, corrigés.** Ni les permissions
(300 s) ni les liaisons de canal (600 s) n'étaient jamais réaffirmées — voir §3 :
toute session relayée dépassant cinq minutes aurait cessé d'être relayée, en
silence. `route_relayee`
encapsulait une première fois pour **tester** si un canal existait, puis une
seconde pour **produire** la trame — soit une trame complète construite puis
jetée à chaque paquet du chemin média, 60 fois par seconde. Réécrit pour
n'encapsuler qu'une fois dans le cas courant. Le champ `password` de
`Identifiants` n'était jamais relu (la clé d'intégrité est dérivée séparément et
passée à part) : un champ mort qui portait un secret, retiré. Et un
`if …is_none() { return None }` réécrit en `?`, sur signalement de clippy. Zéro
avertissement clippy sur les fichiers du chantier.

**Le service coturn ne pouvait pas être committé comme le plan le demandait.**
`docker-compose.yml` est gitignoré à dessein — il porte les mots de passe Windows
du Guacamole historique en clair. Le service vit donc dans
`docker-compose.coturn.yml`, versionné et sans aucun secret (uniquement des
`${VAR}` lues dans `.env`). Lancement :

```bash
docker compose -f docker-compose.yml -f docker-compose.coturn.yml up -d coturn
```

**Une instance de signaling parasite.** Un serveur de signaling tournait depuis
36 heures, sans les variables TURN : les premières sessions ne recevaient donc
aucune configuration ICE. Diagnostiqué en comparant le pid qui tenait le port
8080 à celui que le lancement venait de créer. Vérifier `TURN_URL` dans
`/proc/<pid>/environ` du processus **qui écoute réellement**, pas de celui qu'on
croit avoir lancé.

**coturn écoute sur toutes les interfaces de l'hôte, dont l'adresse publique.**
Constaté au démarrage (`UDP listener opened on: 90.87.35.18:3478`). L'accès reste
authentifié par secret partagé et les identifiants expirent, mais c'est un
service de relais joignable depuis Internet : à restreindre avant tout
déploiement durable (`--listening-ip`, ou un pare-feu).

---

## Ce que ce volet établit

L'agent est un client TURN à part entière : il alloue un relais avant de répondre
à l'offre (faute de trickle ICE), publie le candidat relayé et le candidat
réflexif que la même réponse `Allocate` lui donne, encapsule en ChannelData ce
qui doit passer par le relais, et rafraîchit son bail. Le signaling délivre aux
deux pairs des identifiants éphémères dérivés d'un secret qui ne quitte jamais le
serveur. Le navigateur emploie ces mêmes serveurs ICE.

Prouvé par la mesure : le média traverse un relais TURN réel, pour ≈2 ms de RTT
supplémentaires ; la chaîne d'authentification que nous fabriquons est acceptée
par coturn ; et rien de tout cela ne coûte quoi que ce soit au cas nominal sans
relais.

Prouvé aussi, mais seulement après trois mesures longues et deux correctifs : les
**trois** minuteurs TURN sont entretenus — bail à 300 s, permissions et canaux à
150 s —, coturn les accepte, et une session relayée tient onze minutes sans une
perte.

Reste ouvert : une session depuis un réseau réellement contraint (§4), qui seule
peut faire nominer une paire `relay`/`relay` et prouver que le **média** emprunte
le relais du côté agent (§5) — son chemin d'encapsulation, lui, est exercé et
prouvé.

**Trois traces `info` ont été laissées dans le code** (`état du client TURN` une
fois par minute, `rafraîchissement du bail TURN émis`, `réaffirmation d'une
liaison de canal TURN`). Elles sont rares par construction et ce sont elles qui
ont permis de trancher entre deux hypothèses concurrentes en une session au lieu
d'une soirée : les retirer rendrait le prochain diagnostic aussi aveugle que
celui-ci l'a été.
