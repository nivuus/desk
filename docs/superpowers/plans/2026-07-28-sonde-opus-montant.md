# Sonde n°1 du chantier E — Opus montant

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Établir, sans la VM et sans libopus, si str0m 0.21 dépaquetise l'Opus entrant et l'expose via `Event::MediaData` — le seul risque capable de remettre en cause l'architecture du chantier E.

**Architecture:** Deux `Rtc` str0m en bouclage UDP sur `127.0.0.1`, dans le module de tests de `agent/src/transport.rs`. L'un joue le navigateur (offrant, piste audio `sendonly`), l'autre l'agent (répondeur). Aucun code de production n'est modifié : la sonde interroge str0m et la configuration du `Rtc`, pas notre boucle de transport.

**Tech Stack:** Rust, str0m 0.21, `cargo test`. Aucune dépendance ajoutée.

## Global Constraints

- **Aucune modification du code de production.** Ce plan n'ajoute que des tests. Si une tâche semble exiger de toucher `Session`, c'est que la sonde a débordé de son périmètre — s'arrêter et le signaler.
- **Aucune dépendance nouvelle.** Ni `opus`, ni `audiopus`, ni rien. Le dépaquetiseur Opus de str0m (`src/packet/opus.rs`) est un passe-plat : il recopie la charge utile sans la valider. Des octets arbitraires suffisent donc à exercer le chemin.
- **Tout tourne sous Linux, sans la VM Windows.** Ne jamais lancer `virsh` dans ce plan.
- Commande de test du dépôt : `cargo test -p agent --bins`. Le paquet `agent` n'a **pas** de cible bibliothèque — `--lib` échoue avec « no library targets found ».
- Les tests du dépôt sont nommés en français, en snake_case, et portent un commentaire expliquant **ce que leur échec prouverait**. Suivre cette convention.
- La spec de référence est `docs/superpowers/specs/2026-07-28-micro-design.md` §12.

---

### Task 1: Prouver que str0m expose l'Opus entrant via `Event::MediaData`

C'est le risque bloquant. Si ce test ne peut pas passer, le chantier E change de forme et la spec doit être révisée avant toute autre chose.

**Files:**
- Modify: `agent/src/transport.rs` — dans le module `mod tests` existant, après le test `relaie_une_demande_d_image_cle_du_pair_vers_la_source` (fin de fichier, ~ligne 1458)

**Interfaces:**
- Consumes: rien (aucune tâche antérieure)
- Produces: la fonction d'aide `pompe_une_fois(rtc: &mut Rtc, socket: &UdpSocket, addr: SocketAddr, events: &mut Vec<Event>)`, réutilisée par la tâche 2

- [ ] **Step 1: Écrire le test qui échoue**

À ajouter à la fin du module `mod tests` de `agent/src/transport.rs`, avant l'accolade fermante du module.

Note sur `pompe_une_fois` : elle draine **toutes** les sorties de `Rtc` jusqu'au prochain `Timeout`, puis émet **une seule** mutation. C'est la règle de drainage de str0m que tout `transport.rs` protège (voir le commentaire d'en-tête du fichier) ; enchaîner deux `handle_input` sans redrainer entre eux la violerait.

```rust
    /// Fait avancer un `Rtc` d'un tour : draine toutes ses sorties (les
    /// transmissions partent sur le socket, les événements sont collectés),
    /// puis émet **une seule** mutation — un datagramme s'il y en a un en
    /// attente, sinon l'échéance. Cette forme respecte la règle de drainage
    /// de str0m rappelée en tête de ce fichier : jamais deux mutations sans
    /// drainage entre elles.
    fn pompe_une_fois(
        rtc: &mut Rtc,
        socket: &UdpSocket,
        addr: SocketAddr,
        events: &mut Vec<Event>,
    ) {
        loop {
            match rtc.poll_output().expect("poll_output") {
                Output::Timeout(_) => break,
                Output::Transmit(t) => {
                    let _ = socket.send_to(&t.contents, t.destination);
                }
                Output::Event(e) => events.push(e),
            }
        }

        let mut buffer = vec![0u8; 2000];
        match socket.recv_from(&mut buffer) {
            Ok((n, source_addr)) => {
                if let Ok(contents) = DatagramRecv::try_from(&buffer[..n]) {
                    let receive = Receive {
                        proto: Protocol::Udp,
                        source: source_addr,
                        destination: addr,
                        contents,
                    };
                    let _ = rtc.handle_input(Input::Receive(Instant::now(), receive));
                }
            }
            Err(_) => {
                let _ = rtc.handle_input(Input::Timeout(Instant::now()));
            }
        }
    }

    /// Sonde n°1 du chantier E
    /// (`docs/superpowers/specs/2026-07-28-micro-design.md` §12).
    ///
    /// Le sens montant — navigateur vers agent — n'a jamais été exercé dans
    /// ce dépôt : `transport.rs` n'écrit que de la vidéo et ne traite
    /// `Event::MediaData` que côté pair, dans un test. Toute la conception du
    /// chantier E repose sur l'hypothèse que str0m dépaquetise l'Opus entrant
    /// et le remet au répondeur. Ce test l'établit, ou la réfute.
    ///
    /// Son échec prouverait que le transport du micro ne peut pas s'appuyer
    /// sur une piste média str0m telle quelle : il faudrait alors écrire la
    /// dépaquetisation, ou replier le micro sur un canal de données — dans
    /// les deux cas, la spec de E est à réviser avant d'écrire une ligne
    /// d'implémentation.
    ///
    /// N'exerce ni libopus ni WASAPI : le dépaquetiseur Opus de str0m
    /// (`src/packet/opus.rs`) recopie la charge utile sans la valider, donc
    /// des octets arbitraires suffisent à prouver le chemin de bout en bout.
    #[test]
    fn str0m_remet_l_opus_entrant_via_media_data() {
        use str0m::media::{Direction, MediaKind};
        use str0m::rtp::Frequency;

        str0m::crypto::from_feature_flags().install_process_default();

        let local_ip: IpAddr = "127.0.0.1".parse().unwrap();

        // Le « navigateur » : offrant, une piste audio en émission seule —
        // exactement la topologie décidée par la spec E §5.
        let offrant_socket = UdpSocket::bind(SocketAddr::new(local_ip, 0)).expect("socket offrant");
        let offrant_addr = offrant_socket.local_addr().unwrap();
        offrant_socket.set_nonblocking(true).unwrap();
        let mut offrant = Rtc::builder().clear_codecs().enable_opus(true).build(Instant::now());
        offrant.add_local_candidate(Candidate::host(offrant_addr, "udp").unwrap());

        // L'« agent » : répondeur. Opus activé — la tâche 2 établit
        // séparément que la configuration actuelle de `Session` ne l'active
        // pas, ce qui est précisément ce que le chantier E devra corriger.
        let repondeur_socket =
            UdpSocket::bind(SocketAddr::new(local_ip, 0)).expect("socket répondeur");
        let repondeur_addr = repondeur_socket.local_addr().unwrap();
        repondeur_socket.set_nonblocking(true).unwrap();
        let mut repondeur = Rtc::builder().clear_codecs().enable_opus(true).build(Instant::now());
        repondeur.add_local_candidate(Candidate::host(repondeur_addr, "udp").unwrap());

        let mut api = offrant.sdp_api();
        let mid = api.add_media(MediaKind::Audio, Direction::SendOnly, None, None, None);
        let (offre, en_attente) = api.apply().expect("offre non vide");

        let reponse = repondeur
            .sdp_api()
            .accept_offer(offre)
            .expect("le répondeur accepte une offre audio quand Opus est activé");
        offrant
            .sdp_api()
            .accept_answer(en_attente, reponse)
            .expect("réponse acceptée par l'offrant");

        // Charge utile arbitraire : un premier octet plausible comme TOC Opus
        // suivi de trois octets quelconques. Le contenu n'est jamais
        // interprété, ni par str0m ni par ce test — seule son intégrité de
        // bout en bout est vérifiée.
        const CHARGE: &[u8] = &[0x78, 0x01, 0x02, 0x03];
        const TRAMES_VISEES: usize = 3;
        // 20 ms à 48 kHz : la durée de trame que Chrome émet par défaut.
        const PAS_RTP: u64 = 960;

        let echeance = Instant::now() + Duration::from_secs(15);
        let mut connecte = false;
        let mut ecrites = 0usize;
        let mut rtp = 0u64;
        let mut recues: Vec<Arc<[u8]>> = Vec::new();

        while Instant::now() < echeance && recues.len() < TRAMES_VISEES {
            let mut evenements_offrant = Vec::new();
            pompe_une_fois(
                &mut offrant,
                &offrant_socket,
                offrant_addr,
                &mut evenements_offrant,
            );
            if evenements_offrant.iter().any(|e| matches!(e, Event::Connected)) {
                connecte = true;
            }

            let mut evenements_repondeur = Vec::new();
            pompe_une_fois(
                &mut repondeur,
                &repondeur_socket,
                repondeur_addr,
                &mut evenements_repondeur,
            );
            for evenement in &evenements_repondeur {
                if let Event::MediaData(data) = evenement {
                    recues.push(data.data.clone());
                }
            }

            // L'écriture est une mutation : le tour suivant de `pompe_une_fois`
            // la draine, conformément à la règle de str0m.
            //
            // Le PT est résolu dans une expression qui se termine avant
            // l'écriture : `Writer::write` consomme le writer, et conserver le
            // premier emprunt de `offrant` pendant qu'on en prend un second
            // ne compilerait pas. C'est la raison pour laquelle le code de
            // production sépare lui aussi `select_negotiated_h264_pt` de
            // `write_frame` (`agent/src/transport.rs:801-838`).
            if connecte && ecrites < TRAMES_VISEES {
                let pt = offrant.writer(mid).and_then(|w| {
                    w.payload_params()
                        .find(|p| p.spec().codec == Codec::Opus)
                        .map(|p| p.pt())
                });
                if let (Some(pt), Some(writer)) = (pt, offrant.writer(mid)) {
                    writer
                        .write(
                            pt,
                            Instant::now(),
                            MediaTime::new(rtp, Frequency::FORTY_EIGHT_KHZ),
                            CHARGE.to_vec(),
                        )
                        .expect("écriture de la trame Opus");
                    rtp += PAS_RTP;
                    ecrites += 1;
                }
            }
        }

        eprintln!(
            "sonde n°1 : {ecrites} trame(s) écrite(s), {} reçue(s) via Event::MediaData",
            recues.len()
        );

        assert!(
            connecte,
            "les deux pairs ne se sont jamais connectés : la sonde n'a rien pu établir"
        );
        assert_eq!(
            recues.len(),
            TRAMES_VISEES,
            "str0m n'a pas remis les trames Opus entrantes via Event::MediaData \
             ({} reçues sur {TRAMES_VISEES} écrites) — la spec du chantier E est à réviser",
            recues.len()
        );
        for (index, charge) in recues.iter().enumerate() {
            assert_eq!(
                charge.as_ref(),
                CHARGE,
                "la charge utile de la trame {index} est altérée : le dépaquetiseur \
                 ne restitue pas les octets écrits"
            );
        }
    }
```

- [ ] **Step 2: Ajouter l'import manquant**

`Arc` n'est importé nulle part au niveau du fichier : le test
`relaie_une_demande_d_image_cle_du_pair_vers_la_source` l'importe localement
(`agent/src/transport.rs:1317`). Suivre la même convention — ajouter cette ligne
**à l'intérieur** du test de l'étape 1, avec ses autres `use` :

```rust
        use std::sync::Arc;
```

Tout le reste (`Rtc`, `Candidate`, `Output`, `Input`, `Receive`, `Protocol`,
`DatagramRecv`, `MediaTime`, `Codec`, `UdpSocket`, `SocketAddr`, `IpAddr`,
`Duration`, `Instant`) est déjà visible via le `use super::*` du module de tests
(`agent/src/transport.rs:999`).

- [ ] **Step 3: Lancer le test et observer**

```bash
cargo test -p agent --bins str0m_remet_l_opus_entrant_via_media_data -- --nocapture
```

Attendu : **PASS**, avec la ligne `sonde n°1 : 3 trame(s) écrite(s), 3 reçue(s) via Event::MediaData`.

**Si le test échoue, ne pas le « réparer » pour le faire passer.** C'est une sonde : un échec est un résultat, pas un bug. Consigner le message exact et s'arrêter — la tâche 3 en tirera les conséquences sur la spec.

Deux causes d'échec à distinguer avant de conclure, car elles ne disent pas la même chose :

- `les deux pairs ne se sont jamais connectés` → la sonde n'a rien établi. C'est un défaut du harnais (pare-feu local, boucle trop lente), pas une réponse sur str0m. Augmenter l'échéance de 15 s et relancer.
- `str0m n'a pas remis les trames Opus entrantes` alors que la connexion a eu lieu → **c'est la réponse recherchée, et elle est négative.**

- [ ] **Step 4: Commit**

```bash
git add agent/src/transport.rs
git commit -m "test: sonde n°1 du chantier E — Opus entrant via Event::MediaData"
```

---

### Task 2: Constater ce que la configuration actuelle de `Session` fait d'une offre audio

`Session::new` construit `Rtc::builder().clear_codecs().enable_h264(true)` (`agent/src/transport.rs:410-414`) : **aucun codec audio n'est configuré**. La spec du chantier A affirme pourtant, en §7, que « le PT 111 Opus figure déjà dans la table de candidats (`agent/src/transport.rs:1083`) — rien à ajouter au SDP ». Or la ligne 1083 est à l'intérieur d'un **test**, où `CandidatePt` est fabriqué à la main ; elle ne dit rien de la configuration réelle du `Rtc`.

Cette tâche établit le fait, pour A comme pour E.

**Files:**
- Modify: `agent/src/transport.rs` — module `mod tests`, à la suite du test de la tâche 1

**Interfaces:**
- Consumes: rien de la tâche 1 (ce test n'utilise pas `pompe_une_fois` : il s'arrête à la négociation SDP, sans établir de connexion)
- Produces: rien

- [ ] **Step 1: Écrire le test-sonde**

```rust
    /// Sonde de configuration, complément de la sonde n°1
    /// (`docs/superpowers/specs/2026-07-28-micro-design.md` §12).
    ///
    /// `Session::new` construit son `Rtc` avec `clear_codecs().enable_h264(true)`
    /// et n'active aucun codec audio. La spec du chantier A §7 affirme
    /// l'inverse — « rien à ajouter au SDP » — en s'appuyant sur une ligne
    /// qui se trouve être à l'intérieur d'un test, pas dans la configuration.
    ///
    /// Ce test ne fige pas un comportement souhaitable : il **relève** ce que
    /// fait la configuration d'aujourd'hui face à une offre contenant une
    /// piste audio. Les chantiers A et E devront tous deux ajouter
    /// `.enable_opus(true)` ; ce test sera alors mis à jour par celui des deux
    /// qui arrivera le premier.
    #[test]
    fn sonde_la_configuration_actuelle_face_a_une_offre_audio() {
        use str0m::media::{Direction, MediaKind};

        let local_ip: IpAddr = "127.0.0.1".parse().unwrap();
        let source_path =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let source = Box::new(
            crate::source::FileSource::from_path(source_path, 1280, 720, 60)
                .expect("chargement du flux de test"),
        );
        let mut session = Session::new(source, local_ip).expect("session");

        // Offre d'un « navigateur » qui déclare vidéo en réception et audio en
        // émission : la topologie exacte que décrit la spec E §5.
        let peer_socket = UdpSocket::bind(SocketAddr::new(local_ip, 0)).expect("socket du pair");
        let peer_addr = peer_socket.local_addr().unwrap();
        let mut peer_rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            .enable_opus(true)
            .build(Instant::now());
        peer_rtc.add_local_candidate(Candidate::host(peer_addr, "udp").unwrap());

        let mut api = peer_rtc.sdp_api();
        api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        api.add_media(MediaKind::Audio, Direction::SendOnly, None, None, None);
        let (offre, _en_attente) = api.apply().expect("offre non vide");

        let reponse = session
            .accept_offer(&offre.to_sdp_string())
            .expect("l'offre est acceptée dans son ensemble");

        eprintln!("--- réponse SDP de l'agent à une offre vidéo + audio ---\n{reponse}\n---");

        // Relevé : la section audio de la réponse est-elle acceptée ou rejetée ?
        // Une m-line rejetée porte le port 0 (RFC 3264 §6).
        let section_audio_rejetee = reponse
            .lines()
            .filter(|l| l.starts_with("m=audio"))
            .any(|l| l.split_whitespace().nth(1) == Some("0"));
        let porte_un_codec_opus = reponse.to_lowercase().contains("opus");

        eprintln!(
            "sonde de configuration : section audio rejetée = {section_audio_rejetee}, \
             mention d'Opus dans la réponse = {porte_un_codec_opus}"
        );

        // La seule chose qu'on affirme ici, c'est que la présence d'une piste
        // audio dans l'offre ne fait pas échouer la négociation entière : une
        // section rejetée est une réponse valide, un `Err` serait un blocage.
        assert!(
            reponse.contains("m=video"),
            "la réponse ne contient plus de section vidéo : une offre audio \
             ne doit pas faire perdre la vidéo"
        );
    }
```

- [ ] **Step 2: Lancer le test et relever la sortie**

```bash
cargo test -p agent --bins sonde_la_configuration_actuelle_face_a_une_offre_audio -- --nocapture
```

Attendu : **PASS**. Copier intégralement la ligne `sonde de configuration : …` et la réponse SDP affichée — c'est le livrable de la tâche, il alimente la tâche 3.

Si le test échoue sur `accept_offer` (un `Err` au lieu d'une réponse), c'est un résultat encore plus fort : la configuration actuelle refuse en bloc une offre contenant de l'audio. Consigner le message d'erreur exact.

- [ ] **Step 3: Commit**

```bash
git add agent/src/transport.rs
git commit -m "test: relever ce que la configuration actuelle fait d'une offre audio"
```

---

### Task 3: Consigner les relevés et corriger ce qu'ils démentent

Une sonde dont le résultat n'est pas écrit quelque part n'a servi à rien. Cette tâche est le livrable réel du plan.

**Files:**
- Modify: `docs/superpowers/specs/2026-07-28-micro-design.md` — §12, ligne du risque n°1
- Modify: `docs/superpowers/specs/2026-07-28-audio-design.md` — §7, paragraphe « Négociation »
- Modify: `CLAUDE.md` — nouvelle section, à la suite de « 🔊 Audio de la VM — état constaté »

**Interfaces:**
- Consumes: les sorties `--nocapture` des tâches 1 et 2
- Produces: rien (tâche terminale)

- [ ] **Step 1: Inscrire le résultat de la sonde n°1 dans la spec E**

Dans `docs/superpowers/specs/2026-07-28-micro-design.md` §12, remplacer la cellule « Comment on la tranche » de la ligne n°1 par le résultat obtenu, daté, avec le nom du test qui l'établit. Si la sonde est **positive**, la formulation attendue est de cette forme :

```markdown
| 1 | str0m 0.21 dépaquetise-t-il l'Opus entrant et l'expose-t-il via `Event::MediaData` ? | **Levé le 28 juillet 2026** par `transport::tests::str0m_remet_l_opus_entrant_via_media_data` : trois trames écrites par un pair offrant `sendonly` sont remises intactes au répondeur. Le dépaquetiseur `OpusDepacketizer` de str0m recopie la charge utile sans la valider, donc la sonde n'a exigé ni libopus ni la VM |

(Dater du jour où les tâches 1 et 2 ont réellement tourné — `date +"%d %B %Y"` — et non du jour où ce plan a été écrit, si les deux diffèrent.)
```

Si elle est **négative**, ne pas la maquiller : consigner l'échec, et ajouter en §5 une note indiquant que la topologie retenue est remise en cause et que la spec doit être rouverte avant planification du chantier.

- [ ] **Step 2: Corriger la spec du chantier A**

Dans `docs/superpowers/specs/2026-07-28-audio-design.md` §7, sous « Négociation », l'affirmation suivante est fausse :

> - Agent : le PT 111 Opus figure déjà dans la table de candidats
>   (`agent/src/transport.rs:1083`) — **rien à ajouter au SDP**.

La ligne 1083 est à l'intérieur du test `selectionne_le_mode_de_paquetisation_1`, où `CandidatePt` est construit à la main. La configuration réelle est `clear_codecs().enable_h264(true)` (`agent/src/transport.rs:410-414`), sans aucun codec audio. Remplacer par :

```markdown
- Agent : contrairement à ce que laissait croire la présence d'un PT 111 dans
  les données d'un test, **aucun codec audio n'est configuré** : `Session::new`
  construit son `Rtc` avec `clear_codecs().enable_h264(true)`
  (`agent/src/transport.rs:410-414`). Il faut donc ajouter `.enable_opus(true)`
  au constructeur. Relevé par
  `transport::tests::sonde_la_configuration_actuelle_face_a_une_offre_audio`.
```

Ajuster la formulation au relevé réel de la tâche 2 : si la réponse SDP rejetait la section audio par un port 0, le dire ; si `accept_offer` échouait, le dire aussi.

- [ ] **Step 3: Consigner dans CLAUDE.md**

Ajouter une section après « 🔊 Audio de la VM — état constaté », sur le modèle de celle-ci — un relevé daté, sa commande, et ce qu'il permet de conclure :

```markdown
## 🎤 Sens montant WebRTC — état constaté (28 juillet 2026)

Relevé en ouvrant le chantier E (micro navigateur → VM), pour lever le risque
« str0m ne remet peut-être pas l'audio entrant ».

- **str0m 0.21 remet bien l'Opus entrant** via `Event::MediaData`, charge utile
  intacte. Son `OpusDepacketizer` est un passe-plat : il ne valide pas le
  contenu, donc une sonde n'a besoin ni de libopus ni de la VM.
- **Aucun codec audio n'est configuré** dans le `Rtc` de `Session`
  (`clear_codecs().enable_h264(true)`). Les chantiers A et E devront ajouter
  `.enable_opus(true)`.

```bash
cargo test -p agent --bins str0m_remet_l_opus_entrant_via_media_data -- --nocapture
cargo test -p agent --bins sonde_la_configuration_actuelle_face_a_une_offre_audio -- --nocapture
```
```

Adapter le contenu au résultat réel. **Ne rien écrire qui n'ait été observé.**

- [ ] **Step 4: Vérifier que la suite complète reste verte**

```bash
cargo test -p agent --bins
```

Attendu : 62 tests passants (60 avant ce plan, plus les deux sondes).

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-07-28-micro-design.md \
        docs/superpowers/specs/2026-07-28-audio-design.md \
        CLAUDE.md
git commit -m "docs: consigner le relevé du sens montant WebRTC"
```

---

## Ce que ce plan ne fait pas

Les sondes 2, 3 et 4 de la spec E §12 — installation de VB-Cable, format du câble, périphérique d'entrée par défaut, annulation d'écho — **exigent la VM** et ne sont pas dans ce plan. Elles seront traitées quand le chantier E sera planifié pour de bon, c'est-à-dire après la livraison du chantier A dont E réutilise `opus.rs` et la plomberie WASAPI.
