//! La liste de priorités d'un tour de boucle.
//!
//! `act_on_timeout` décide de la SEULE action entreprise par tour. L'ordre
//! n'est pas arbitraire :
//!
//! - le drainage dû passe en priorité absolue : c'est la seule façon de
//!   garantir qu'aucune mutation ne s'enchaîne sans un passage complet par
//!   `poll_output()` entre les deux, quel que soit l'état des autres files ;
//! - le contrôle et l'adaptation passent avant les médias : reconfigurer
//!   l'encodeur avec une image en vol coûterait cette image ;
//! - l'audio passe avant la vidéo : une coupure sonore s'entend, une image
//!   en retard de 10 ms ne se voit pas ;
//! - l'attente sur le socket ne vient qu'en dernier, quand il n'y a rien à
//!   émettre.
//!
//! Le corps de chaque branche vit dans son module thématique ; ce fichier ne
//! porte que l'ordre.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use proto::control::AgentControl;
use str0m::Input;

use super::Session;

/// Résultat du traitement d'un événement ou d'un tour de boucle interne.
pub(super) enum Tick {
    Continue,
    Disconnected,
}

/// Intervalle minimal entre deux vérifications de `source.is_alive()` dans
/// `act_on_timeout`. Cet appel coûte un appel système à chaque tour côté
/// Windows (recherche de la fenêtre) ; une fenêtre fermée le reste, inutile
/// de le revérifier à 60 Hz.
const ALIVE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

impl Session {
    /// Réagit à `Output::Timeout` : décide et effectue AU PLUS UNE mutation
    /// de `Rtc` (drainage différé d'une image déjà écrite, message de
    /// contrôle en attente, image vidéo due, ou traitement d'un paquet
    /// entrant / échéance str0m), puis rend la main à `run()`, qui rappelle
    /// immédiatement `poll_output` — c'est cette structure qui garantit le
    /// drainage avant toute mutation suivante (C2 de la revue) : il
    /// n'existe aucun chemin de code qui mute `Rtc` sans que `run()` ne
    /// rappelle `poll_output` juste après. La priorité donnée au drainage
    /// différé (voir `video_write_pending_drain`) est ce qui rend cette
    /// garantie vraie même juste après l'écriture d'une image : sans elle,
    /// `write_frame` (une mutation) suivi directement de `handle_input`
    /// (une seconde) violerait la même règle.
    ///
    /// Onze branches supplémentaires (a0bis : drainage d'un message de
    /// contrôle produit hors boucle vers `pending_control` ; a0ter :
    /// décision d'adaptation en attente ; a1 : redimensionnement en attente ;
    /// a1bis : visibilité en attente ; a1ter : annonce d'un changement de
    /// sommeil ; a1ter-bis : annonce d'un changement de plein écran
    /// (sous-bloc D8) ; a1quater : part de budget accordée par le capteur
    /// (sous-bloc D6) ; a1quinquies : ordre audio décidé par le capteur
    /// (sous-bloc D7) ; a1sexies : reconstruction d'une capture audio morte
    /// détectée localement, `AudioMort` en repli si le budget de tentatives
    /// est épuisé, et annonce d'une reprise PROUVÉE par un paquet réel
    /// (sous-bloc D9, remède complet apporté par D10) ; a1septies : annonce
    /// d'un changement du presse-papier de la VM (sous-bloc P1) ; a2 :
    /// vérification de la fenêtre) ne mettent JAMAIS en file, avant de rendre
    /// la main, une écriture qui resterait à drainer — c'est l'invariant que
    /// cette énumération existe pour auditer. **Dix d'entre elles (toutes sauf
    /// a1quater) ne touchent même pas `self.rtc`** : seulement `self.source`,
    /// `self.audio_source`, le budget de reconstruction audio (a1sexies
    /// seule) et/ou `self.pending_control`, au plus en y mettant en file un
    /// message de contrôle (`queue_control`, qui n'empile qu'un `VecDeque`,
    /// sans effet sur `Rtc` avant le tour suivant).
    ///
    /// **a1quater fait exception, et il faut le dire précisément** :
    /// `rtc.bwe().set_desired_bitrate` (corps dans `part`) MUTE bien un champ
    /// interne de `Rtc` — et reconfigure le pacer de str0m
    /// (`configure_pacer`) si une estimation de bande passante existe déjà.
    /// Mais cet appel ne met AUCUN paquet en file : l'effet qu'il programme
    /// côté sondage (`ProbeControl` de str0m, qui peut avancer l'échéance de
    /// la prochaine sonde et faire émettre du bourrage) n'est évalué qu'au
    /// PROCHAIN traitement de `Input::Timeout`, jamais pendant cet appel-ci.
    /// C'est cette absence de mise en file — pas l'absence de mutation de
    /// `Rtc` — qui préserve l'invariant de drainage pour cette branche.
    ///
    /// Chacune rend quand même la main immédiatement après son action plutôt
    /// que d'enchaîner sur la branche suivante dans le même appel : le
    /// redimensionnement reconstruit une chaîne d'encodage entière
    /// (potentiellement long, voir `WindowsSource::resize`), et le traiter
    /// comme une étape à part entière — au même titre que les branches qui,
    /// elles, écrivent ou lisent réellement des paquets sur `Rtc` (a0, a3, b,
    /// c…) — garde cette fonction lisible comme une seule liste de priorités
    /// plutôt que de mêler deux styles différents.
    ///
    /// **À qui lira ceci après une dixième branche** : ce compte et cette
    /// énumération sont le point d'audit de l'invariant « aucune de ces
    /// branches ne met en file, avant de rendre la main, une écriture qui
    /// resterait à drainer » — **PAS** « aucune de ces branches ne mute
    /// `Rtc` » : a1quater en mute bien un champ (voir plus haut, et ne pas
    /// laisser cette formulation-ci se recopier dans une future addition
    /// sans revérifier ce distinguo). Une addition qui oublie de se confronter
    /// à cet invariant se vérifie sur une liste incomplète. Mets-les à jour
    /// dans le même geste que la branche.
    ///
    /// Ne prend pas `on_input`/`on_control` : `handle_input` ne produit
    /// jamais d'événement applicatif directement (les événements qui en
    /// résultent ne sortent que via un futur `poll_output`, donc via
    /// `run()`, qui les dispatche lui-même).
    pub(super) fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick> {
        // a0) Drainage dû après la dernière image vidéo ou le dernier paquet
        // audio écrit. Vérifié en priorité absolue, avant tout le reste :
        // c'est la seule façon de garantir qu'aucune mutation ne s'enchaîne
        // jamais sans un passage complet par `poll_output()` entre les deux,
        // quel que soit l'état des autres files (voir le commentaire du
        // champ et la ronde de correction 1 de la tâche 11).
        if self.video_write_pending_drain || self.audio_write_pending_drain {
            // Un seul `handle_input(Timeout)` dépile `to_payload` pour TOUTES
            // les pistes : les deux drapeaux retombent donc ensemble. Les
            // garder séparés reste nécessaire en amont — c'est ce qui permet
            // à `write_audio` et `write_frame` de signaler indépendamment
            // qu'une écriture a bien eu lieu.
            self.video_write_pending_drain = false;
            self.audio_write_pending_drain = false;
            self.rtc
                .handle_input(Input::Timeout(Instant::now()))
                .map_err(|e| anyhow!("handle_input timeout (drainage média) : {e}"))?;
            return Ok(Tick::Continue);
        }

        // a0bis) Un message de contrôle produit hors de la boucle attend.
        //        Corps dans `controle`.
        if let Some(tick) = self.drainer_controle_externe() {
            return Ok(tick);
        }

        // a) Un message de contrôle est en attente. Corps dans `controle`,
        //    test de vacuité compris : la branche ne laisse passer (`None`)
        //    que sans avoir muté `Rtc` — file vide, ou canal pas encore
        //    ouvert alors que la session n'est pas en clôture, auquel cas le
        //    message reste en file.
        if let Some(tick) = self.brancher_controle_en_file()? {
            return Ok(tick);
        }

        if self.ending {
            // Message de fin envoyé (file vidée ci-dessus) : terminé.
            return Ok(Tick::Disconnected);
        }

        // a0ter) Décision d'adaptation en attente. Traitée avant la branche
        //        vidéo et avant le redimensionnement : reconfigurer
        //        l'encodeur avec une image en vol coûterait cette image.
        //        Ne mute jamais `Rtc`. Corps dans `adaptation`.
        if let Some(decision) = self.pending_decision.take() {
            self.appliquer_decision(decision);
            return Ok(Tick::Continue);
        }

        // a1) Redimensionnement en attente, à traiter avant la branche
        //     vidéo. Ne mute jamais `Rtc` non plus, mais reste une opération
        //     potentiellement longue — fenêtre ET périphérique D3D11 neufs,
        //     voir `WindowsSource::resize` — traitée ici comme une étape à
        //     part entière plutôt que mêlée à d'autres dans le même appel, à
        //     l'image des autres branches. Corps dans `redimensionnement`.
        if let Some((width, height)) = self.pending_resize.take() {
            self.appliquer_redimensionnement(width, height);
            return Ok(Tick::Continue);
        }

        // a1bis) Visibilité en attente. Après le redimensionnement et avant la
        //        vidéo, pour la même raison que lui : la décision peut
        //        relâcher un encodeur côté capteur, ce qui est long, et ne
        //        mute jamais `Rtc`.
        if let Some((visible, focalisee)) = self.pending_visibility.take() {
            if let Err(erreur) = self.source.set_awake(visible, focalisee) {
                // Non fatal : perdre l'arbitrage n'est pas perdre la session.
                tracing::warn!(%erreur, visible, focalisee, "visibilité refusée par le capteur");
            }
            return Ok(Tick::Continue);
        }

        // a1ter) Un changement de sommeil à annoncer au navigateur. Interrogée
        //        à chaque tour où a1bis ne s'est pas déclenchée (sinon celle-ci
        //        est déjà sortie par un retour anticipé) ; mais
        //        `sommeil_a_annoncer` consomme : aucun message n'est jamais
        //        réémis, donc cette branche ne peut pas inonder le canal de
        //        contrôle même à ~100 Hz.
        if let Some((endormie, raison)) = self.source.sommeil_a_annoncer() {
            self.queue_control(AgentControl::asleep(endormie, &raison));
            return Ok(Tick::Continue);
        }

        // a1ter-bis) Un changement de plein écran à annoncer au navigateur.
        //            Même régime que a1ter juste au-dessus :
        //            `plein_ecran_a_annoncer` CONSOMME, donc aucun message
        //            n'est jamais réémis et cette branche ne peut pas inonder
        //            le canal de contrôle même à ~100 Hz.
        if let Some(actif) = self.source.plein_ecran_a_annoncer() {
            self.queue_control(AgentControl::fullscreen(actif));
            return Ok(Tick::Continue);
        }

        // a1quater) Une part de budget accordée par le capteur. Après a1ter
        //           (qui n'endort rien : elle ANNONCE au navigateur un
        //           sommeil déjà décidé côté capteur — l'endormissement
        //           réel, lui, a lieu côté capteur, pas ici). La cohérence
        //           entre un sommeil et la part qui en découle ne se joue PAS
        //           dans cet ordre local : elle se joue en AMONT, côté
        //           capteur, où `distribuer` (les ordres de sommeil) précède
        //           `distribuer_les_parts` sur tous les chemins d'entrée du
        //           registre (`inscrire`, `retirer`, `signaler`,
        //           `echec_de_reveil`, tour de roue — voir
        //           `capteur/sommeil.rs`).
        //
        //           ⚠️ **Tous SAUF UN, et il ne faut pas le taire** (I2, revue
        //           finale de branche). Le chemin `rompus` de
        //           `distribuer_les_parts` envoie les parts D'ABORD, puis
        //           détecte les canaux rompus, retire leurs sessions du
        //           vivier, et relaie seulement ensuite les ordres que ce
        //           retrait engendre. Une session RÉVEILLÉE par la place
        //           qu'un mort libère reçoit donc son `Reveiller` APRÈS la
        //           part d'endormie calculée juste avant, et n'obtient sa part
        //           d'éveillée qu'au tour de roue suivant.
        //           **Borne : `PERIODE_REARBITRAGE`, soit 250 ms**, pendant
        //           lesquelles cette fenêtre encode au plancher
        //           `PART_DORMANTE_BPS`. Le canal unique garantit l'ordre de
        //           LIVRAISON, jamais l'ordre de CALCUL — c'est cette
        //           distinction que la rédaction précédente manquait.
        //
        //           Traiter a1quater juste après a1ter reste le choix le plus
        //           lisible : il respecte l'ordre d'arrivée plutôt que de
        //           l'inverser sans raison.
        //           Ne met aucun paquet en file — voir la doc de tête de
        //           cette fonction sur ce que `set_desired_bitrate` mute
        //           réellement — mais pose une décision que la branche
        //           a0ter appliquera au tour suivant.
        //           `part_a_appliquer` CONSOMME : aucune réémission, donc
        //           aucune reconfiguration en boucle à ~100 Hz.
        if let Some(bps) = self.source.part_a_appliquer() {
            self.appliquer_part(bps);
            return Ok(Tick::Continue);
        }

        // a1quinquies) Un ordre audio décidé par le capteur. Après a1quater,
        //              pour la même raison de lisibilité : on respecte l'ordre
        //              d'arrivée plutôt que de l'inverser sans raison.
        //
        //              Ne met AUCUN paquet en file : `appliquer_audio` (tâche
        //              7, voir `piste_audio.rs`) ne touche que la source
        //              audio (`AudioSource::set_actif`, qui écrit un booléen
        //              lu par le fil de capture) et le budget du contrôleur
        //              (`Controleur::changer_audio_bps`, qui n'écrit qu'un
        //              champ de `Config`) — ni l'un ni l'autre ne met de
        //              paquet en file. L'invariant de drainage de cette
        //              fonction est donc préservé.
        //
        //              `audio_a_appliquer` CONSOMME : un `Start()`/`Stop()` par
        //              tour à ~100 Hz est exactement ce que cette consommation
        //              empêche.
        if let Some(actif) = self.source.audio_a_appliquer() {
            self.appliquer_audio(actif);
            return Ok(Tick::Continue);
        }

        // a1sexies) Deux transitions détectées LOCALEMENT, jamais poussées par
        //           le capteur : la capture audio de cette fenêtre vient de
        //           mourir définitivement (`crate::audio::LECTURES_ECHOUEES_MAX`
        //           erreurs de lecture WASAPI consécutives, `windows_audio.rs`),
        //           ou elle vient d'apporter la PREUVE qu'elle est repartie
        //           (un paquet réel, posé par `brancher_audio` — voir
        //           `piste_audio`).
        //
        //           **D10 inverse l'ordre du remède** (D9 ne savait que
        //           signaler `AudioMort`, jamais reconstruire).
        //           `reconstruire_ou_signaler` (corps dans `piste_audio`)
        //           TENTE D'ABORD de refabriquer la source ; `AudioMort` n'est
        //           plus le premier geste mais le REPLI — celui du cas où le
        //           budget de tentatives est épuisé, ou où il n'existe aucun
        //           reconstructeur — et où seule la promotion d'une voisine
        //           par le capteur peut encore rendre du son au groupe.
        //
        //           ❌ **« Chemin mono-fenêtre » figurait dans cette
        //           parenthèse et c'est FAUX** (revue transverse de fin de
        //           branche, second tour) : `demarrage/audio.rs::brancher`
        //           pose un reconstructeur INCONDITIONNELLEMENT dans son bras
        //           `Ok`, branche `None` COMPRISE. Les seuls cas réellement
        //           sans reconstructeur sont `AUDIO=0`, `TEST_FILE`, et un
        //           échec d'ouverture initiale.
        //
        //           🔴 **Et l'erreur portait à conséquence ICI plus
        //           qu'ailleurs, parce que ce fichier est celui qui APPELLE
        //           `reconstruire_ou_signaler`** : elle donnait à qui
        //           reprendra le legs n°1 le modèle mental exactement
        //           INVERSE du vrai. En mono-fenêtre la source EST
        //           reconstruite — puis le réarmement la RENDAIT MUETTE,
        //           `audio_porteuse` valant alors toujours `false` faute de
        //           capteur pour l'écrire.
        //
        //           ✅ **CORRIGÉ AU SOUS-BLOC D11 (leg 4), et mesuré** :
        //           `demarrage/audio.rs::brancher` pose
        //           `audio_porteuse = true` dans sa seule branche
        //           mono-fenêtre, et la recette ① relève 441 Hz reçus au
        //           vert contre la sentinelle au rouge. Voir
        //           `Session::reconstruire_ou_signaler` (`piste_audio.rs`).
        //
        //           `appliquer_audio` (a1quinquies juste au-dessus) ne court
        //           qu'à l'ARRIVÉE d'un ordre, jamais périodiquement : sans ce
        //           contrôle au tick, une capture qui meurt (ou qui reprend)
        //           entre deux ordres ne serait jamais signalée. Un `load`
        //           atomique ou une tentative de reconstruction bornée par son
        //           propre répit sont, l'un comme l'autre, bon marché par
        //           tour.
        //
        //           Le verrou `audio_mort_signale` est ce qui empêche
        //           d'inonder le capteur d'`AudioMort` : une fois posé, il ne
        //           retombe QUE sur deux transitions — un rattachement (voir
        //           plus bas), ou une RÉÉLECTION par le capteur
        //           (`appliquer_audio`, a1quinquies), qui réapprovisionne
        //           aussi le budget de tentatives. Sans ce second point de
        //           chute, trouvé en revue de la tâche 12, le budget posé une
        //           fois à la construction de la `Session` n'aurait permis
        //           qu'un seul cycle mort → reconstruit → prouvé par session,
        //           jamais plusieurs échecs CONSÉCUTIFS — l'inverse de ce que
        //           `REARMEMENTS_MAX` (`capteur/sommeil.rs`) est censé
        //           compter. `audio_vivant_a_annoncer`, lui, se CONSOMME dès
        //           sa lecture (même régime que `sommeil_a_annoncer` /
        //           `part_a_appliquer`), donc ne peut pas non plus réémettre
        //           `AudioVivant` en boucle.
        //
        //           La remise à zéro du verrou (`rattachement_survenu`) N'EST
        //           PAS elle-même une action : elle ne mute ni `Rtc` ni la
        //           source, ne met rien en file, et ne casse donc pas
        //           l'invariant de drainage même sans `return` — même régime
        //           que `last_alive_check` en a2. Un rattachement (capteur
        //           relancé) fait perdre au capteur la mémoire de tout
        //           `AudioMort` signalé avant la rupture : sans cette remise à
        //           zéro, cette fenêtre ne le réinformerait jamais.
        if self.source.rattachement_survenu() {
            self.audio_mort_signale = false;
        }
        // D10 : on tente d'abord de RECONSTRUIRE. `AudioMort` n'est plus le
        // premier geste mais le repli — celui du cas où l'arbre de processus a
        // disparu, et où seule la promotion d'une voisine peut encore rendre
        // du son au groupe.
        if !self.audio_mort_signale && self.reconstruire_ou_signaler(Instant::now()) {
            self.audio_mort_signale = true;
            self.source.signaler_audio_mort();
            return Ok(Tick::Continue);
        }
        if self.audio_vivant_a_annoncer {
            self.audio_vivant_a_annoncer = false;
            self.source.signaler_audio_vivant();
            return Ok(Tick::Continue);
        }

        // a1septies) Le presse-papier de la VM a changé (sous-bloc P1). Même
        //            régime qu'a1ter-bis : `presse_papier_a_annoncer` CONSOMME,
        //            donc aucun message n'est jamais réémis et cette branche ne
        //            peut pas inonder le canal de contrôle même à ~100 Hz. Ce
        //            point compte davantage ici qu'ailleurs : le texte peut
        //            peser jusqu'à `presse_papier::PRESSE_PAPIER_MAX` (64 Kio),
        //            là où un `Asleep` ou un `Fullscreen` pèse quelques octets.
        //
        //            `texte` à `None` n'est PAS « rien à annoncer » : c'est un
        //            REFUS de taille, que le navigateur doit dire à
        //            l'utilisateur (D-P1-1). C'est le `Option` EXTÉRIEUR, celui
        //            que rend la méthode, qui porte « rien à annoncer ».
        if let Some((texte, octets)) = self.source.presse_papier_a_annoncer() {
            self.queue_control(AgentControl::clipboard(texte, octets));
            return Ok(Tick::Continue);
        }

        // a2) La fenêtre capturée a-t-elle disparu ? Coûte un appel système
        // côté Windows (recherche de la fenêtre) : espacé par
        // `ALIVE_CHECK_INTERVAL` plutôt que vérifié à chaque tour de
        // boucle — une fenêtre fermée le reste.
        let now = Instant::now();
        if now.saturating_duration_since(self.last_alive_check) >= ALIVE_CHECK_INTERVAL {
            self.last_alive_check = now;
            if !self.source.is_alive() {
                self.begin_ending("fenêtre fermée");
                return Ok(Tick::Continue);
            }
        }

        // a3) Un paquet audio, si la piste est négociée et qu'un paquet
        //     attend. AVANT la vidéo : une coupure sonore s'entend, une image
        //     en retard de 10 ms ne se voit pas. L'audio a de plus une
        //     cadence dure de 10 ms, quand la vidéo est opportuniste par
        //     nature. Corps dans `piste_audio`.
        if let Some(tick) = self.brancher_audio() {
            return Ok(tick);
        }

        // b) Une image vidéo, si son échéance est atteinte et la piste
        //    négociée. Corps dans `piste_video`.
        if let Some(tick) = self.brancher_video() {
            return Ok(tick);
        }

        // b0) Requête TURN en attente d'émission (allocation, rafraîchissement
        //     du bail, permission, liaison de canal). Ne mute jamais `Rtc` :
        //     c'est un échange avec le serveur de relais, invisible de str0m.
        //     Placée juste avant l'attente pour que le rafraîchissement du
        //     bail ne dépende pas de l'arrivée d'un paquet. Corps dans
        //     `relais`.
        if let Some(tick) = self.emettre_requete_turn() {
            return Ok(tick);
        }

        // c) Rien à émettre : attendre un paquet entrant, borné à la fois
        //    par l'échéance de `Rtc` et par les prochaines échéances de
        //    média. Corps dans `socket`.
        self.brancher_attente(deadline)
    }
}

// Les tests vivent dans un fichier voisin : ce fichier-ci a franchi 500
// lignes en ajoutant la couverture des branches a1bis/a1ter (tâche 8,
// sous-bloc D5). Voir l'en-tête de `tick/tests.rs`.
#[cfg(test)]
mod tests;
