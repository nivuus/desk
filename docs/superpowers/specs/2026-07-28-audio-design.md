# Chantier A — Audio

**Date** : 28 juillet 2026
**Statut** : Spécification validée, prête à planifier
**Portée** : Porter le son de la VM Windows jusqu'au navigateur, en synchronisme
avec la vidéo

---

## 1. Objectif

Le produit est muet. Toute la chaîne — capture DXGI, encodage NVENC, transport
str0m, rendu navigateur — ne transporte que de l'image. Ce chantier ajoute
l'audio de bout en bout : capture du son que produit la VM, encodage Opus,
transport sur la même connexion WebRTC, restitution dans le navigateur.

Le cadrage jeu (`2026-07-28-support-jeux-design.md` §5) classe ce chantier comme
bénéficiant à **tout** le produit, pas seulement au jeu : un traitement de texte
qui bipe, une vidéo lue dans un navigateur distant, une visioconférence — rien de
tout cela ne fonctionne aujourd'hui.

Le risque n°1 du chantier D (popup blocker) ayant été levé le 28/07
(`2026-07-28-spike-multifenetres-resultats.md`), le modèle « une fenêtre Windows
= une fenêtre navigateur » tient. Ce chantier est donc bâti sur cette hypothèse.

## 2. Ce que le relevé sur la VM a déjà établi

Le cadrage désignait un risque susceptible de bloquer le chantier :

> **Risque à lever** : la VM dispose-t-elle d'un périphérique de rendu audio ?
> [...] Sans périphérique de rendu actif, WASAPI loopback ne produit rien. Un
> pilote audio virtuel peut être nécessaire.

**Ce risque est levé.** Relevé du 28 juillet 2026 (VM démarrée pour l'occasion,
consigné dans `CLAUDE.md`) :

| Périphérique | État |
| --- | --- |
| Haut-parleurs (Steam Streaming Speakers) | `OK` — endpoint actif, **virtuel**, aucun matériel requis |
| HDP-V104 (NVIDIA High Definition Audio) | `OK` — endpoint actif |
| NVIDIA Virtual Audio Device (Wave Extensible) (WDM) | pilote présent |
| « Sortie audio de l'ordinateur distant » (×11) | `Unknown` — endpoints RDP résiduels de sessions mortes |

`Audiosrv` tourne, démarrage `Automatic`. **Aucun pilote audio virtuel n'est à
installer.**

La chaîne de compilation a été vérifiée dans la même passe, libopus étant une
bibliothèque C alors que l'agent ne dépendait jusqu'ici d'aucun code C :

- `rustc 1.97.1`, hôte `x86_64-pc-windows-msvc` ;
- Visual Studio 2022 Build Tools présent, `cl.exe` trouvé ;
- `winget` disponible ; **`cmake` absent** ;
- l'agent se compile **sur la VM** via WinRM (`scripts/build-agent.sh`), pas en
  compilation croisée : toute dépendance native doit donc être satisfaite là-bas.

## 3. Décisions actées

| Décision | Choix | Justification |
| --- | --- | --- |
| Périmètre de capture | Loopback **système** (périphérique de rendu par défaut), plus une **sonde** du process loopback | Le son marche vite ; la sonde lève le risque du multi-fenêtres pour quelques lignes, sans construire l'isolation par processus dont aucun consommateur n'existe avant le chantier D |
| Déblocage du son côté navigateur | Muet au départ, réactivé au **premier geste** utilisateur | Troisième usage du même ressort d'armement, après la fenêtre (variante 3 du spike) et le plein écran (cadrage jeu §4.1). Coût utilisateur nul : le geste survient de toute façon |
| Codec | Opus, 48 kHz stéréo, trames de 10 ms, ~128 kbps, `RESTRICTED_LOWDELAY`, FEC in-band, DTX | Media Foundation n'expose pas d'encodeur Opus ; aucun codec que Chrome accepte en WebRTC n'est disponible côté Windows nativement. Valeurs reprises du cadrage jeu §5 A |
| Microphone (navigateur → VM) | **Hors périmètre** | Le cadrage ne le mentionne pas. Il exigerait une capture navigateur, un décodeur côté agent et un périphérique d'entrée virtuel sous Windows — un chantier à part entière |
| Horodatage de capture porté jusqu'à `write()` | **Dans le périmètre** | Sans lui la synchro A/V est fausse par construction. Voir §6 |

## 4. Découpage en modules

Le dépôt est plat (`capture.rs`, `encode.rs`, `h264.rs`, `source.rs`,
`windows_source.rs`). L'audio suit la même symétrie, module pour module, et la
même ligne de partage : ce qui est testable sous Linux est séparé de ce qui
exige Windows.

| Fichier | Responsabilité | Portable |
| --- | --- | --- |
| `agent/src/audio.rs` (créé) | Trait `AudioSource`, type `AudioPacket`, tampon circulaire borné `PacketRing`, source de test `ToneSource` (sinusoïde) | oui |
| `agent/src/opus.rs` (créé) | Enveloppe de l'encodeur libopus : configuration, encodage d'une trame de 10 ms | oui |
| `agent/src/wasapi.rs` (créé) | Capture loopback brute : `IMMDeviceEnumerator`, `IAudioClient`, `IAudioCaptureClient`. Rend des blocs PCM | Windows |
| `agent/src/windows_audio.rs` (créé) | Assemble wasapi + opus + fil dédié + canal ; implémente `AudioSource` | Windows |
| `agent/src/transport.rs` (modifié) | Branche audio de la boucle, `audio_mid`, sélection du PT Opus, horodatage de capture |  |
| `agent/src/h264.rs` (modifié) | `AccessUnit` porte l'instant de capture |  |
| `agent/src/windows_source.rs` (modifié) | Renseigne cet instant ; accepte une origine d'horloge imposée |  |
| `agent/src/main.rs` (modifié) | Construit l'horloge de session et la source audio |  |
| `client/src/webrtc.ts` (modifié) | Transceiver audio, flux unique à deux pistes, déblocage au geste |  |
| `client/src/stats.ts` (modifié) | Métriques audio dans l'incrustation de mesure |  |

`audio.rs` et `opus.rs` ne référencent jamais `windows` : ils se compilent et se
testent sous Linux, comme `source.rs` et `h264.rs` aujourd'hui.

## 5. Flux de données

```
[fil dédié]   WASAPI loopback, sondé toutes les ~5 ms
                → PCM (format de mixage de l'endpoint)
                → conversion en i16 entrelacé, 48 kHz, stéréo
                → complément de silence si le bloc est court ou absent
                → libopus (RESTRICTED_LOWDELAY, FEC, DTX)
                → AudioPacket { data, pts_48k, captured_at }
                → PacketRing borné (~10 paquets), dépôt NON bloquant
[boucle transport]  retrait non bloquant dans act_on_timeout
                → writer(audio_mid).write(pt, captured_at,
                                          MediaTime::new(pts_48k, FORTY_EIGHT_KHZ),
                                          data)
```

### Pourquoi un fil dédié

La boucle de transport ne doit jamais bloquer — c'est l'invariant que tout
`transport.rs` protège, jusqu'à remplacer `set_read_timeout` par un sondage en
petites tranches parce que le délai débordait de 12,7 ms en moyenne sous
Windows. Une capture audio dans cette boucle y réintroduirait exactement le
défaut qui vient d'en être chassé.

### Pourquoi un sondage, et non un réveil par événement

`AUDCLNT_STREAMFLAGS_EVENTCALLBACK` **n'est pas supporté** en combinaison avec
`AUDCLNT_STREAMFLAGS_LOOPBACK` : Microsoft documente la capture loopback comme
devant être pilotée par minuterie. Un flux de rendu inactif ne signale d'ailleurs
aucun événement, ce qui est précisément le cas fréquent ici (rien ne joue).

Le sondage n'est donc pas un pis-aller : c'est la seule forme correcte, et elle
sert directement le complément de silence décrit ci-dessous.

### Pourquoi un tampon circulaire borné, et non un canal

Si la boucle de transport prend du retard, l'audio en attente est déjà périmé :
mieux vaut le jeter que laisser la file grossir et la latence s'accumuler
irréversiblement. C'est le raisonnement qui a déjà fait déclarer le canal
d'entrées `ordered: false, maxRetransmits: 0`.

Le dépôt est **non bloquant** : un dépôt bloquant ferait de la boucle de
transport la contrainte du fil de capture, et un blocage côté transport gèlerait
la capture WASAPI, dont le tampon interne déborderait à son tour.

À saturation, c'est le paquet **le plus ancien** qui est rejeté, pas le plus
récent — sur une piste temps réel, garder le frais et jeter le périmé est le seul
arbitrage qui ne fait pas croître la latence. C'est la raison pour laquelle un
`std::sync::mpsc::sync_channel` ne convient pas : son `try_send` échoue à
saturation, donc rejette le **nouveau** paquet, exactement le mauvais bout. D'où
un `PacketRing` — un `Arc<Mutex<VecDeque<AudioPacket>>>` borné, `push_back` avec
`pop_front` quand il est plein. Aucune dépendance supplémentaire, et il vit dans
`audio.rs`, donc testable sous Linux.

Chaque rejet incrémente un compteur, journalisé périodiquement et jamais
silencieux.

### Silence

Quand aucune application ne joue, WASAPI loopback ne rend aucune donnée (ou des
tampons marqués `AUDCLNT_BUFFERFLAGS_SILENT`). La cadence de 10 ms est **malgré
tout maintenue**, en complétant par du silence.

Deux raisons, et la seconde est la vraie :

1. DTX activé, l'encodeur émet alors des trames de quelques octets — le coût est
   négligeable ;
2. surtout, la ligne de temps RTP reste **continue et sans trou**. C'est cette
   continuité qui rend les Sender Reports exploitables et empêche le tampon de
   gigue du navigateur de s'affamer puis de resynchroniser brutalement — la
   signature exacte du défaut relevé en recette du jalon 1 sur la vidéo (641,8 →
   877,6 → 1164,0 → 1449,9 ms puis retour à 83,0 ms).

### Format d'entrée

En mode partagé, `IAudioClient::GetMixFormat` impose le format : typiquement
`WAVEFORMATEXTENSIBLE`, flottant 32 bits, 48 kHz, 2 canaux. La conversion prise
en charge est :

- flottant 32 bits → entier signé 16 bits (format d'entrée de libopus retenu) ;
- mono → stéréo par duplication, plus de 2 canaux → sous-mixage sur les deux
  premiers.

**Une fréquence d'échantillonnage autre que 48 kHz est refusée** avec un message
explicite nommant la fréquence rencontrée, plutôt que rééchantillonnée. Écrire un
rééchantillonneur pour un cas dont on ignore s'il se produit serait du travail
spéculatif ; le format de mixage réel est relevé par la sonde de la tâche 1, et
si le besoin apparaît il sera traité sur pièces. Opus accepte nativement 48, 24,
16, 12 et 8 kHz : le jour où le besoin se présente, l'ajout est borné.

## 6. Synchronisation A/V — un écart existant qu'il faut corriger ici

C'est le point de conception le moins évident du chantier, et celui qu'on ne peut
pas remettre à plus tard.

`Writer::write(pt, wallclock, rtp_time, data)` : le `wallclock` est ce que str0m
utilise pour bâtir les **RTCP Sender Reports**, seul mécanisme par lequel un
récepteur WebRTC met deux pistes en correspondance. La documentation de str0m est
sans ambiguïté :

> the wallclock is the real world time that corresponds to the `MediaTime`

Or `write_frame` passe aujourd'hui `Instant::now()` **au moment de l'écriture**
(`agent/src/transport.rs:838`), et non l'instant auquel l'image a été capturée.
Tout le délai de capture puis d'encodage matériel se trouve donc encapsulé dans
la correspondance annoncée.

Tant qu'il n'y avait que la vidéo, c'était invisible : un décalage uniforme, sans
second flux pour le révéler, ne se voit pas. Dès qu'on ajoute l'audio — dont le
chemin est bien plus court — **l'audio devancerait la vidéo de tout ce délai**.
La synchro labiale serait fausse par construction, et aucun réglage ultérieur ne
la rattraperait.

### La correction

`WindowsSource` horodate déjà ses images depuis une origine réelle
(`clock_origin.elapsed()`, corrigé le 28/07 — le cadrage jeu §5 C, qui range
encore le pacing des PTS parmi les correctifs à venir, est sur ce point périmé).
L'instant de capture est donc **déjà connu ; il est simplement jeté**.

Trois changements, tous locaux :

1. `AccessUnit` gagne un champ `captured_at: Instant` ;
2. les deux sources reçoivent la **même** origine d'horloge, créée une fois dans
   `main.rs` et passée à la construction ;
3. `write_frame` transmet `unit.captured_at` au lieu de `Instant::now()`, et le
   chemin audio transmet `packet.captured_at`.

L'audio, lui, est exact par nature : son PTS est un **compte d'échantillons**
depuis l'origine, à 48 kHz — pas une lecture d'horloge. `captured_at` s'en
déduit, ce qui garantit que les deux quantités restent cohérentes entre elles.

Ce n'est pas un élargissement de périmètre : sans cette correction, le livrable
du chantier serait un son désynchronisé, c'est-à-dire un défaut à la place d'une
fonctionnalité.

## 7. Négociation et boucle de transport

### Négociation

Le navigateur est l'offrant (`client/src/webrtc.ts`) : c'est **lui** qui doit
déclarer la piste audio. L'agent ne fait que répondre.

- Client : `pc.addTransceiver('audio', { direction: 'recvonly' })` avant
  `createOffer`.
- Agent : le PT 111 Opus figure déjà dans la table de candidats
  (`agent/src/transport.rs:1083`) — **rien à ajouter au SDP**. Il manque
  seulement de quoi retenir le `mid` : `Event::MediaAdded` ne conserve
  aujourd'hui que la vidéo (`if media.kind == MediaKind::Video`), ce `if` devient
  un `match` sur les deux genres.
- Un `select_negotiated_opus_pt` calqué sur `select_negotiated_h264_pt`, avec le
  même emprunt séparé de `self` pour la même raison (le journal d'avertissement
  qui suit exige `&mut self`).

### Boucle

Une branche `a1` est insérée dans `act_on_timeout`, **entre** le drainage différé
(`a0`) et la vidéo (`b`) :

```
a0) drainage différé en attente          (existant, priorité absolue)
a1) paquet audio disponible ?            (nouveau)
b)  échéance d'image vidéo atteinte ?    (existant)
c)  attente bornée sur le socket         (existant)
```

Deux propriétés à préserver :

- **Une seule mutation de `Rtc` par appel.** La branche audio a son propre
  drapeau `audio_write_pending_drain`, distinct de celui de la vidéo, traité en
  `a0` au même titre. Sans drapeau distinct, une écriture audio suivie d'une
  écriture vidéo au tour suivant perdrait un drainage.
- **L'audio passe avant la vidéo.** Une coupure sonore s'entend ; une image en
  retard de 10 ms ne se voit pas. L'audio a par ailleurs une cadence dure de
  10 ms, là où la vidéo est opportuniste par nature (`next_frame` rend `None` sur
  un bureau immobile, cas courant et normal).

`bounded_wait` doit en outre être plafonné court dès que l'audio est négocié
— sans quoi la branche `c` dormirait au travers de paquets audio qui arrivent,
exactement le défaut C1 que `bounded_wait` avait été introduit pour corriger côté
vidéo.

## 8. Côté navigateur

### Déblocage du son

Chrome bloque la lecture audio sans activation utilisateur, et l'activation
obtenue sur la page d'accueil **ne franchit pas** l'ouverture d'une nouvelle
fenêtre — c'est précisément ce que le spike multi-fenêtres a mesuré.

- L'élément vidéo démarre `muted = true` ;
- un écouteur `pointerdown` et `keydown` en `{ once: true }` repasse `muted` à
  `false` au premier geste, quel qu'il soit ;
- un bandeau discret « cliquez pour activer le son » n'apparaît **que** si aucun
  geste n'est survenu au bout de quelques secondes, et disparaît au premier.

Aucun clic n'est donc imposé : celui qui sert à jouer suffit.

### Flux à deux pistes

Le gestionnaire `track` actuel réassigne `srcObject` à chaque piste reçue. Avec
deux pistes, il faut un `MediaStream` unique qui les porte toutes les deux, sans
quoi la seconde piste chasse la première.

### Métriques

`stats.ts` gagne, pour l'audio : débit, paquets perdus, gigue. Sans ces
chiffres, la recette n'aurait rien à lire et « ça a l'air de marcher » tiendrait
lieu de résultat.

## 9. Erreurs et dégradation

Le principe est celui déjà appliqué au reste de l'agent : **l'audio ne doit
jamais tuer une session vidéo qui fonctionne.**

| Situation | Comportement |
| --- | --- |
| Aucun périphérique de rendu par défaut | Journal d'erreur explicite, session **sans audio**, vidéo intacte |
| Format de mixage non 48 kHz | Idem, avec la fréquence rencontrée nommée |
| Échec d'initialisation WASAPI ou libopus | Idem |
| Piste audio non négociée par le client | Aucun paquet écrit, avertissement unique (calqué sur `warn_negotiation_once`) |
| Canal saturé | Paquet le plus ancien rejeté, compteur agrégé journalisé périodiquement |
| Échec d'écriture sur la piste audio | Journalisé ; la session **continue** — contrairement au chemin vidéo, où l'échec d'écriture clôt la session |

Un échec audio silencieux serait le pire cas : c'est exactement l'écran noir muet
que `warn_negotiation_once` a été ajouté pour rendre diagnosticable côté vidéo.
Chaque chemin de dégradation journalise donc une fois, explicitement.

## 10. Tests

### Sous Linux, sans Windows

- **`opus.rs`** : encodage d'une sinusoïde connue, puis **décodage en retour**
  avec libopus, et vérification que le signal restitué corrèle avec l'original.
  Vérifier seulement que l'encodeur rend des octets prouverait qu'il rend du
  bruit tout aussi bien.
- **`opus.rs`** : une trame de 10 ms à 48 kHz stéréo consomme exactement 480
  échantillons par canal ; un compte différent est refusé.
- **`audio.rs`** : `ToneSource` produit des PTS strictement croissants, espacés
  de 480, sans trou — y compris à travers un intervalle de silence.
- **`audio.rs`** : à saturation, `PacketRing` rejette le plus **ancien**, ne
  bloque jamais, et incrémente son compteur. Un test nomme explicitement quel
  bout est jeté — c'est l'arbitrage du §5, et l'inverser passerait sinon
  inaperçu.
- **`transport.rs`** : le test de bouclage str0m existant (`transport.rs:1320`)
  étendu à une piste audio négociée — un paquet écrit côté agent est reçu côté
  pair, avec le PT attendu.
- **`transport.rs`** : `write_frame` transmet bien `unit.captured_at` et non
  l'instant courant (non-régression sur §6).

### Côté client

- L'offre contient un `m=audio` en `recvonly` ;
- deux pistes reçues aboutissent dans **un seul** `MediaStream` ;
- le premier geste démute, et un second geste ne fait rien.

La logique de démutage est testée par **injection** de l'élément et des
écouteurs, sans toucher aux objets globaux — la technique retenue pour
`reset-origin.js` dans le spike, qui a permis de tester le même module sous
Vitest et dans le navigateur sans build.

### Sur la VM — recette

Automatiser ceci honnêtement n'est pas possible : il faut entendre.

1. Jouer un son connu sur la VM ; le percevoir dans le navigateur.
2. Relever débit, perte et gigue sur une minute.
3. Mesurer le décalage A/V : un événement simultanément visible et audible, dont
   on compare les instants de restitution.
4. Vérifier qu'un silence prolongé (60 s) ne provoque ni coupure, ni dérive, ni
   resynchronisation brutale à la reprise.

## 11. Risques et sondes

Le chantier ouvre par une tâche de sonde, dont le seul livrable est un relevé
consigné. Rien n'est construit avant.

| # | Question | Comment on la tranche |
| --- | --- | --- |
| 1 | Quel est le périphérique de rendu **par défaut** de la session interactive, et quel est son format de mixage ? | `IMMDeviceEnumerator::GetDefaultAudioEndpoint(eRender, eConsole)` puis `GetMixFormat`, exécuté **en session interactive** — la session 0 de WinRM peut avoir des valeurs par défaut différentes |
| 2 | Un loopback sur « Steam Streaming Speakers » capture-t-il bien ce que jouent les applications, ou le son part-il vers un flux Steam ? | Jouer un son connu, capturer, vérifier que le tampon n'est pas silencieux |
| 3 | Quel crate libopus se construit sur la chaîne de la VM, et exige-t-il `cmake` ? | Essai de compilation. `cmake` s'installe par `winget install Kitware.CMake` si nécessaire |
| 4 | Le **process loopback** fonctionne-t-il sur cette VM (build 20348) ? | Sonde jetable : activation via `ActivateAudioInterfaceAsync` avec `AUDIOCLIENT_ACTIVATION_PARAMS` / `PROCESS_LOOPBACK` sur un PID connu. On observe si l'activation réussit et si des données arrivent. **Rien n'est construit dessus** — le résultat est consigné pour le chantier D |

Les sondes 1, 2 et 4 exigent la VM démarrée : `virsh start Windows` (voir
`CLAUDE.md`).

## 12. Critère de fin

- Un son joué sur la VM est **entendu** dans le navigateur, à travers Pomerium.
- Le décalage A/V mesuré est inférieur à la perception courante — l'ITU-R BT.1359
  situe le seuil de gêne à environ 45 ms d'avance de l'audio et 125 ms de retard.
- Un silence de 60 s ne provoque ni coupure ni resynchronisation brutale.
- La suite de tests est verte sous Linux, sans VM.
- Les quatre sondes du §11 sont consignées, y compris celle du process loopback,
  dont le résultat conditionne le chantier D.

## 13. Hors périmètre

- **Le microphone** (navigateur → VM). Chantier distinct : capture navigateur,
  décodage côté agent, périphérique d'entrée virtuel sous Windows.
- **L'isolation audio par processus.** Sondée (§11 n°4), pas construite. Son
  consommateur est le chantier D, qui n'existe pas.
- **Le débit audio adaptatif.** Relève du chantier C, qui traitera l'adaptation
  réseau pour les deux médias à la fois.
- **Le rééchantillonnage.** Refusé explicitement plutôt qu'écrit à l'aveugle
  (§5). À rouvrir sur pièces si la sonde n°1 rend autre chose que 48 kHz.
- **La sélection du périphérique par l'utilisateur.** Le périphérique par défaut
  suffit ; rien dans le produit ne permet aujourd'hui d'en choisir un autre.
