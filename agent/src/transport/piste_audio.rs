//! La piste audio : négociation du payload type Opus, et écriture des
//! paquets vers str0m. L'audio passe AVANT la vidéo dans la liste de
//! priorités de `tick` — une coupure sonore s'entend, une image en retard
//! de 10 ms ne se voit pas.

use std::time::Duration;

use str0m::format::Codec;
use str0m::media::{Frequency, MediaTime, Mid, Pt};

use super::tick::Tick;
use super::Session;
use crate::audio::{AudioPacket, AudioSource};

/// L'injection de fautes de reconstruction (variable de banc
/// `AUDIO_FAUTE_RECONSTRUCTION`), extraite ici parce que son addition portait
/// `piste_audio.rs` à 513 lignes — au-dessus du plafond de 500 du dépôt. La
/// règle est sans exception : **extraction, jamais compression**.
///
/// Pas de frontière `#[cfg(windows)]` ici, donc la convention `#[path]` de
/// `CLAUDE.md` ne s'applique pas : un `mod` ordinaire suffit.
pub(in crate::transport) mod injection;

/// Plafond d'attente quand une piste audio est négociée.
///
/// Les paquets audio arrivent d'un fil de capture indépendant : cette boucle
/// n'a aucun moyen de prévoir leur instant d'arrivée, elle ne peut que se
/// réveiller assez souvent pour ne pas les laisser vieillir. 2 ms pour une
/// cadence de trames de 10 ms — un cinquième de trame de retard au pire.
pub(super) const AUDIO_POLL_INTERVAL: Duration = Duration::from_millis(2);

impl Session {
    /// Fournit la source audio. Sans appel, la session reste muette et la
    /// vidéo fonctionne normalement.
    pub fn set_audio_source(&mut self, source: Box<dyn AudioSource + Send>) {
        self.audio_source = Some(source);
    }

    /// Plafond d'attente de la branche `c` : uniquement quand une source ET
    /// une piste audio existent, sinon rien ne justifie de se réveiller plus
    /// souvent.
    pub(super) fn audio_wait_cap(&self) -> Option<Duration> {
        (self.audio_source.is_some() && self.audio_mid.is_some() && !self.ending)
            .then_some(AUDIO_POLL_INTERVAL)
    }

    /// Branche `a3` de la liste de priorités (voir `tick`) : émet un paquet
    /// audio si la piste est négociée et qu'un paquet attend.
    ///
    /// Pas d'échéance à surveiller ici : le fil de capture dépose dans un
    /// tampon, il suffit de regarder s'il y a quelque chose. Le réveil
    /// régulier vient d'`AUDIO_POLL_INTERVAL`, appliqué en branche `c`.
    ///
    /// Rend `Some(Tick::Continue)` quand elle a conclu le tour — un paquet
    /// écrit est une mutation de `Rtc`, qui doit être suivie du drainage
    /// différé de la branche `a0`. Rend `None` quand il n'y avait rien à
    /// émettre, et n'a alors rien muté : la liste de priorités peut passer à
    /// la branche suivante sans rompre l'invariant de drainage.
    pub(super) fn brancher_audio(&mut self) -> Option<Tick> {
        let (Some(mid), false) = (self.audio_mid, self.ending) else {
            return None;
        };
        let paquet = self
            .audio_source
            .as_mut()
            .and_then(|source| source.next_packet())?;
        // Le leg 6 de D9 : la remise à zéro du compteur de réarmements se fait
        // sur une PREUVE de son — ce paquet-ci —, jamais sur la décision
        // d'arbitrage qui, elle, ne peut pas mordre dans le cas majoritaire
        // (`sommeil/porteurs.rs`, une fenêtre seule de son groupe de PID
        // redevient porteuse automatiquement à la sortie de répit).
        if self.audio_reconstruit_sans_preuve {
            self.audio_reconstruit_sans_preuve = false;
            self.audio_vivant_a_annoncer = true;
        }
        if self.write_audio(mid, paquet) {
            self.audio_write_pending_drain = true;
        }
        Some(Tick::Continue)
    }

    /// Sélectionne le type de charge utile Opus négocié pour `mid`.
    ///
    /// Appel séparé de `write_audio` pour que l'emprunt sur `self` via
    /// `Rtc::writer` se termine avant tout appel `&mut self` ultérieur — même
    /// raison que `select_negotiated_h264_pt`.
    fn select_negotiated_opus_pt(&mut self, mid: Mid) -> Option<Pt> {
        let writer = self.rtc.writer(mid)?;
        // Lié à une variable plutôt que renvoyé directement : le type anonyme
        // rendu par `payload_params()` (capturant la durée de vie de
        // `writer`, voir sa signature) resterait sinon un temporaire vivant
        // jusqu'à la fin du bloc, après la destruction de `writer` — rejeté
        // par l'emprunteur (« `writer` does not live long enough ») alors que
        // la valeur finale (`Option<Pt>`, `Copy`) n'emprunte plus rien.
        let pt = writer
            .payload_params()
            .find(|p| p.spec().codec == Codec::Opus)
            .map(|p| p.pt());
        pt
    }

    /// Écrit un paquet Opus sur la piste audio.
    ///
    /// Renvoie `true` si `writer.write()` a réellement empilé le paquet — donc
    /// qu'un drainage différé est nécessaire.
    ///
    /// Contrairement à `write_frame`, un échec d'écriture ne clôt **pas** la
    /// session : un défaut audio ne doit jamais tuer une session vidéo qui
    /// fonctionne.
    pub(super) fn write_audio(&mut self, mid: Mid, packet: AudioPacket) -> bool {
        let Some(pt) = self.select_negotiated_opus_pt(mid) else {
            self.warn_audio_negotiation_once();
            return false;
        };
        let Some(writer) = self.rtc.writer(mid) else {
            self.warn_audio_negotiation_once();
            return false;
        };

        // `captured_at` est l'instant réel correspondant à `pts_48k` : c'est
        // lui qui part dans les RTCP Sender Reports et porte la synchro A/V.
        let rtp_time = MediaTime::new(packet.pts_48k, Frequency::FORTY_EIGHT_KHZ);
        match writer.write(pt, packet.captured_at, rtp_time, packet.data) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(erreur = %e, "échec d'écriture audio, paquet abandonné");
                false
            }
        }
    }

    /// Applique l'arbitrage audio du capteur : porter le son, ou se taire.
    ///
    /// **Deux effets, et le second est facile à oublier** : la source cesse
    /// d'émettre, ET le budget audio retenu par le contrôleur de congestion
    /// tombe à zéro. Sans le second, une fenêtre muette continuerait d'amputer
    /// son budget vidéo de 128 kb/s pour une piste qui n'émet rien — c'est le
    /// défaut préexistant que D7 corrige (spec §5).
    ///
    /// ⚠️ **Le budget se conditionne à l'EXISTENCE d'une source, pas au seul
    /// ordre** (F4, revue finale de branche du sous-bloc D7). `actif` seul
    /// ratait les deux chemins où la session n'a aucune source audio alors que
    /// le capteur l'élit porteuse : l'échec d'ouverture du *process loopback*,
    /// dont la spec §6 fait explicitement un repli silencieux, et `AUDIO=0` —
    /// où, à une fenêtre par PID, **toutes** les fenêtres sont porteuses et le
    /// défaut préexistant revenait intact.
    pub(super) fn appliquer_audio(&mut self, actif: bool) {
        // Correction apportée en revue de la tâche 12 (sous-bloc D10) : le
        // budget de reconstruction (`reconstructions_restantes`) n'était posé
        // qu'UNE FOIS, à la construction de la `Session`, et jamais
        // réapprovisionné — le cycle mort → reconstruit → prouvé ne pouvait
        // donc tourner qu'une seule fois par session (voir la doc du champ
        // `audio_porteuse`, `transport.rs`).
        //
        // Une RÉÉLECTION — une TRANSITION vers `actif: true` — est
        // littéralement le capteur qui dit « retente » : c'est le seul point
        // de réapprovisionnement retenu. **Une transition, pas la seule
        // présence d'un ordre `actif: true`** : le capteur ne réémet déjà que
        // sur changement (`sommeil::porteurs::distribuer_l_audio`), mais s'y
        // fier seul reporterait cette garantie sur un module distant, sur
        // lequel ce fichier n'a aucune prise ; `audio_porteuse` la rend locale
        // et vérifiable ici, sans dépendre de cette discipline distante. Sans
        // cette restriction à la seule transition, un flot d'ordres `actif:
        // true` identiques rendrait le budget infini.
        if actif && !self.audio_porteuse {
            self.reconstructions_restantes = crate::audio::RECONSTRUCTIONS_MAX;
            // ⚠️ **Résidu trouvé en re-revue (sous-bloc D10), documenté et non
            // corrigé : cette remise à `None` ANNULE l'espacement
            // `REPIT_RECONSTRUCTION` à chaque réélection.** Pour une session
            // déjà latchée morte, chaque transition `false → true` achète
            // donc une tentative de reconstruction IMMÉDIATE au prochain tour
            // — c'est-à-dire une ouverture *process loopback* bloquante sur
            // le fil de `Session::run`, exactement le coût que le répit
            // existe pour espacer. `REARMEMENTS_MAX` ne le borne pas : il ne
            // compte que les cycles qui atteignent `AudioMort`, jamais les
            // réélections elles-mêmes — un groupe qui bascule entre deux
            // fenêtres du même PID peut donc réélire plus vite qu'un budget
            // ne s'épuise. **Ce n'est pas une régression** : le débit reste
            // borné par `PERIODE_REARBITRAGE` (250 ms, `capteur/sommeil.rs`),
            // qui borne la fréquence à laquelle le registre peut faire
            // basculer `actif`. Mais c'est un couplage NEUF entre le
            // va-et-vient de l'arbitrage audio et du travail bloquant sur le
            // fil de drainage, que rien n'empêchait avant que cette remise à
            // zéro n'existe.
            self.prochaine_reconstruction = None;
            // Lève le verrou qui, sinon, empêcherait `act_on_timeout`
            // (branche a1sexies) de rappeler `reconstruire_ou_signaler` :
            // sans cette ligne, le réapprovisionnement du budget ci-dessus
            // serait sans effet, puisque la porte d'entrée resterait fermée.
            self.audio_mort_signale = false;
        }
        self.audio_porteuse = actif;

        let mut capture_morte = false;
        if let Some(source) = self.audio_source.as_mut() {
            source.set_actif(actif);
            capture_morte = source.capture_morte();
        }
        self.congestion
            .changer_audio_bps(if actif && self.audio_source.is_some() {
                crate::opus::BITRATE_BPS as u32
            } else {
                0
            });
        // `session` : sans ce champ la trace n'est PAS attribuable — tous les
        // enfants héritent le même `agent.log` depuis D4. Même motif et même
        // champ que « part de budget appliquee ».
        //
        // `capture_morte` : sans lui cette ligne MENTIRAIT (F3). Un fil de
        // capture qui a définitivement abandonné laisse `set_actif` réussir —
        // il n'écrit qu'un atomique que plus personne ne lit —, et la trace
        // annonçait alors `actif=true` pour une fenêtre qui ne produira plus
        // jamais un paquet. ~~C'est le seul endroit du produit où cet état
        // devienne observable ; le capteur, lui, ne le voit pas.~~
        //
        // ❌ **Les deux clauses barrées sont fausses depuis le sous-bloc
        // D10** (revue transverse). `capture_morte` est relu à CHAQUE tour par
        // `capture_audio_morte` → `reconstruire_ou_signaler` (branche
        // a1sexies), qui journalise « capture audio reconstruite » ou
        // « reconstruction de la capture audio refusée » et pousse `AudioMort`
        // en repli — le capteur le voit donc, l'inscrit dans ses `inaptes` et
        // le journalise à son tour. Cette trace-ci n'est plus ni le seul
        // observatoire ni la seule voie ; elle reste utile pour ce qu'elle
        // est, un état lu au point d'application de l'ordre. Le renvoi au
        // « commentaire d'abandon dans `windows_audio.rs` » a en outre suivi
        // l'extraction de la tâche 3 : il vit dans `windows_audio/fil.rs`.
        tracing::info!(
            session = %self.session_id,
            actif,
            capture_morte,
            "ordre audio applique"
        );
    }

    /// Déclare que cette session porte le son **sans qu'aucun capteur ne le
    /// lui dise**. Réservé au mode MONO-FENÊTRE.
    ///
    /// ⚠️ **Ne JAMAIS appeler depuis une session servie par un capteur.** Le
    /// capteur arbitre qui porte le son entre les fenêtres d'un même groupe de
    /// PID, et `appliquer_audio` est le seul chemin légitime dans ce mode.
    /// Poser `true` ici sur une session arbitrée ferait parler une fenêtre qui
    /// doit se taire — deux fenêtres joueraient alors le même mix
    /// désynchronisé, l'écho audible que le défaut F2 du sous-bloc D7 décrit.
    /// `une_session_non_porteuse_reconstruite_reste_muette` le garde rouge.
    ///
    /// Son unique appelant de production est `demarrage/audio.rs::brancher`,
    /// dans sa SEULE branche `config.fenetre_hwnd == None`.
    pub fn set_audio_porteuse(&mut self, porteuse: bool) {
        self.audio_porteuse = porteuse;
    }

    /// Confie de quoi refabriquer la source audio après la mort de sa capture.
    pub fn set_audio_reconstructeur(&mut self, r: crate::audio::Reconstructeur) {
        self.audio_reconstructeur = Some(r);
    }

    /// Rend `true` s'il faut signaler `AudioMort` au capteur — c'est-à-dire
    /// quand il n'y a plus rien à reconstruire.
    ///
    /// **La reconstruction passe AVANT le signalement**, et c'est l'inversion
    /// que D10 apporte : le signal au capteur cesse d'être le premier geste
    /// pour devenir le repli. La promotion d'une voisine (la seule moitié de
    /// D9 qui fonctionnait) garde alors son rôle exact — celui du cas où
    /// l'arbre de processus a réellement disparu.
    ///
    /// ⚠️ **Une reconstruction réussie RÉARME aussi la source, sur
    /// `audio_porteuse`** (défaut trouvé en recette VM, corrigé dans le corps
    /// ci-dessous) : `WindowsAudioSource::pour_processus` naît toujours
    /// MUETTE, et sans ce réarmement une session porteuse dont la capture
    /// vient d'être reconstruite ne produirait plus jamais aucun paquet, donc
    /// aucune PREUVE, donc aucune réélection : un état ABSORBANT.
    ///
    /// ❌ **« Le seul chemin qu'emprunte un reconstructeur » était écrit ici,
    /// et c'est FAUX — relevé par la revue transverse de fin de branche, et
    /// c'est le défaut le plus lourd qu'elle ait trouvé, parce qu'il a une
    /// conséquence de comportement.** `demarrage/audio.rs::brancher` pose un
    /// reconstructeur dans les DEUX modes : sa branche `None`
    /// (`config.fenetre_hwnd` absent — le chemin MONO-FENÊTRE) appelle
    /// `WindowsAudioSource::new`, qui s'auto-émet.
    ///
    /// ✅ **La conséquence que D10 léguait ici — « en mono-fenêtre, le remède
    /// de reconstruction est INERTE » — est CORRIGÉE (sous-bloc D11, leg 4).**
    /// Elle tenait à ce qu'`audio_porteuse` naisse `false` (`transport.rs`)
    /// avec `appliquer_audio` pour unique écrivain, c'est-à-dire un ordre
    /// `Audio` du capteur qu'un agent mono-fenêtre ne reçoit jamais : une
    /// capture reconstruite y était auto-émise à `true` par `new()`, puis
    /// **remise à `false`** par la ligne de réarmement ci-dessous.
    /// `demarrage/audio.rs::brancher` appelle désormais `set_audio_porteuse`
    /// dans sa seule branche mono-fenêtre, et le champ a donc un second
    /// écrivain — hors de ce module.
    ///
    /// ⚠️ **Le remède ne force PAS `true` sans arbitrage**, et c'est la seule
    /// forme sûre : forcer inconditionnellement ici réintroduirait le défaut
    /// PIRE que le passage de `audio_porteuse` évite en multi-fenêtres (une
    /// fuite de son vers une fenêtre qui doit se taire), et
    /// `une_session_non_porteuse_reconstruite_reste_muette` l'y garde rouge.
    /// Le correctif distingue les deux modes **au branchement**, là où le mode
    /// est connu, jamais ici où il ne l'est pas.
    ///
    /// ⚠️ **Cette méthode court sur le fil de `Session::run`**, et ouvrir une
    /// source WASAPI y est un appel bloquant de durée non bornée. D'où le
    /// répit : au plus une tentative par `REPIT_RECONSTRUCTION`. Si la mesure
    /// montre qu'elle retarde le drainage, elle passera sur un fil — même
    /// risque que `Drop for H264Encoder` porte déjà sur ce fil.
    pub(super) fn reconstruire_ou_signaler(&mut self, maintenant: std::time::Instant) -> bool {
        if !self.capture_audio_morte() {
            return false;
        }
        let Some(reconstructeur) = self.audio_reconstructeur.as_ref() else {
            return true;
        };
        if self.reconstructions_restantes == 0 {
            return true;
        }
        if self.prochaine_reconstruction.is_some_and(|t| maintenant < t) {
            return false;
        }
        self.reconstructions_restantes -= 1;
        self.prochaine_reconstruction = Some(maintenant + crate::audio::REPIT_RECONSTRUCTION);
        // L'injection de faute de banc est interposée devant le
        // reconstructeur — voir `injection`, qui porte le budget, sa raison
        // d'être globale au processus, et le contrat de ce point d'appel.
        //
        // ⚠️ Le bras `Err` qui s'ensuit est CELUI DU LEG 6 (`{erreur:#}`) : la
        // faute injectée emprunte le même `warn!`, et le journal de recette
        // porte donc `erreur=faute injectée (AUDIO_FAUTE_RECONSTRUCTION)`.
        // C'est le contrôle d'ATTEIGNABILITÉ de ce leg — si cette chaîne
        // n'apparaît pas alors que l'injection est armée, c'est le format qui
        // ne marche pas, pas la cause qui manque.
        let tentative = injection::intercepter(|| reconstructeur());
        match tentative {
            Ok(mut source) => {
                tracing::info!(
                    restantes = self.reconstructions_restantes,
                    "capture audio reconstruite"
                );
                // Trouvé en recette VM (deux exécutions : `capture audio
                // reconstruite` = 2, `compteurs_audio_actif_true` = 0 aux
                // deux) : une source reconstruite par
                // `WindowsAudioSource::pour_processus` NAÎT MUETTE
                // (`windows_audio.rs::demarrer`) — à la différence du mode
                // mono-fenêtre `new()`, qui s'émet lui-même. Sans cette
                // ligne, RIEN ne réarme la source reconstruite : elle ne
                // produit aucun paquet, donc aucune PREUVE
                // (`audio_vivant_a_annoncer`), donc aucune réélection —
                // muette pour toujours. Un état ABSORBANT, pas un retard.
                //
                // `audio_porteuse` — jamais l'ordre `actif` du dernier appel
                // à `appliquer_audio`, capturé AVANT que cette fonction n'ait
                // pu le modifier — est le miroir LOCAL du dernier ordre reçu
                // du capteur (voir `appliquer_audio`) : la garantie ne dépend
                // ainsi d'aucune discipline distante. Appliqué SANS
                // condition, aussi bien pour une session porteuse (`true`,
                // qui réarme) que pour une session muette (`false`, qui
                // confirme explicitement le silence plutôt que de le
                // supposer) — voir
                // `une_session_non_porteuse_reconstruite_reste_muette`.
                source.set_actif(self.audio_porteuse);
                self.audio_source = Some(source);
                self.audio_reconstruit_sans_preuve = true;
                false
            }
            Err(erreur) => {
                // `{erreur:#}` et non `%erreur` : le `Display` simple
                // d'`anyhow` ne rend que le contexte le plus EXTERNE, et
                // `windows_audio.rs` en pose justement un
                // (« ouverture du process loopback du PID … »). Le HRESULT —
                // seule donnée qui réponde au leg 6 de D10, « la cause du
                // refus de reconstruction n'est pas identifiée » — restait
                // donc dans les causes, jetée à l'écriture. La pièce est
                // versée : `journaux-multifenetres-d10/agent-critere-2-1-plat.log`
                // l. 97 porte `erreur=ouverture du process loopback du PID
                // 27544` et rien d'autre. Même doctrine que
                // `diagnostics/multifenetre/plafond/sonde.rs`, qui l'explique
                // mot pour mot sur un HRESULT perdu de la même façon.
                tracing::warn!(
                    erreur = format!("{erreur:#}"),
                    restantes = self.reconstructions_restantes,
                    "reconstruction de la capture audio refusée"
                );
                false
            }
        }
    }

    /// Vrai si la capture audio de cette fenêtre a définitivement abandonné
    /// (`AudioSource::capture_morte`, posé après
    /// `crate::audio::LECTURES_ECHOUEES_MAX` erreurs de lecture WASAPI
    /// consécutives, `windows_audio.rs`).
    ///
    /// Sans source (pas de piste audio pour cette session, ou `AUDIO=0`),
    /// jamais morte : il n'y a rien à signaler.
    pub(super) fn capture_audio_morte(&self) -> bool {
        self.audio_source.as_deref().is_some_and(|source| source.capture_morte())
    }

    fn warn_audio_negotiation_once(&mut self) {
        if !self.warned_audio_negotiation {
            self.warned_audio_negotiation = true;
            tracing::warn!(
                "aucun type de charge utile Opus négocié : paquets audio jetés (avertissement unique)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- horloge RTP audio --------------------------------------------------
    //
    // `write_audio` construit `MediaTime::new(pts_48k, Frequency::FORTY_EIGHT_KHZ)` :
    // la fréquence d'horloge RTP du type de charge utile Opus est câblée en
    // dur ici, séparément de `opus::SAMPLE_RATE_HZ`, qui documente pourtant
    // explicitement être « la fréquence d'horloge RTP du type de charge
    // utile Opus ». Rien ne lie ces deux constantes : modifier l'une sans
    // l'autre compilerait sans avertissement et produirait des horodatages
    // RTP faux d'un facteur constant — un défaut de synchronisation
    // silencieux. Ce test échoue si elles divergent.
    #[test]
    fn la_frequence_rtp_audio_correspond_au_taux_d_echantillonnage_opus() {
        assert_eq!(Frequency::FORTY_EIGHT_KHZ.get(), crate::opus::SAMPLE_RATE_HZ);
    }

    #[test]
    fn borne_l_attente_quand_l_audio_est_negocie() {
        // Sans ce plafond, la branche d'attente dormirait jusqu'à l'échéance
        // que réclame `Rtc` — jusqu'à la seconde entière — et traverserait
        // ainsi une centaine de paquets audio dus. C'est le même défaut que
        // C1 côté vidéo, transposé.
        use std::time::Instant;

        use crate::transport::socket::bounded_wait;

        let maintenant = Instant::now();
        let echeance_rtc = maintenant + Duration::from_secs(1);

        let sans_audio = bounded_wait(maintenant, echeance_rtc, None, None);
        assert_eq!(sans_audio, Duration::from_secs(1));

        let avec_audio = bounded_wait(maintenant, echeance_rtc, None, Some(AUDIO_POLL_INTERVAL));
        assert_eq!(avec_audio, AUDIO_POLL_INTERVAL);

        // Le plafond ne doit jamais ALLONGER une attente déjà plus courte.
        let echeance_proche = maintenant + Duration::from_micros(200);
        let court = bounded_wait(
            maintenant,
            echeance_proche,
            None,
            Some(AUDIO_POLL_INTERVAL),
        );
        assert_eq!(court, Duration::from_micros(200));
    }

    /// Éprouve le FORMAT, pas le site d'appel : `{erreur:#}` rend la chaîne de
    /// causes là où `{erreur}` ne rend que le contexte le plus externe. Le site
    /// lui-même n'est pas observable sur l'hôte (c'est un `warn!` de
    /// `tracing`) ; sa preuve est le journal de recette, pas ce test.
    #[test]
    fn le_format_diese_rend_la_chaine_de_causes() {
        use anyhow::Context;

        let cause = anyhow::anyhow!("0x88890004");
        let e = Err::<(), _>(cause)
            .context("ouverture du process loopback du PID 42")
            .unwrap_err();
        assert!(!format!("{e}").contains("0x88890004"), "le Display simple perd la cause");
        assert!(format!("{e:#}").contains("0x88890004"), "{{:#}} doit la rendre");
    }
}
