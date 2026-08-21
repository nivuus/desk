//! La piste MONTANTE : le micro du navigateur vers l'agent (chantier E).
//!
//! **Ce module DÉPOSE, et rien d'autre.** C'est l'invariant du chantier A pris
//! en miroir (spec §7) : la boucle de transport dépose, un fil dédié travaille.
//! `handle_event` s'exécute PENDANT le drainage de `poll_output` et ne doit
//! muter aucun `Rtc` — un dépôt dans un puits n'en mute aucun, et
//! `TamponGigue::deposer` ne rend rien, donc ne peut structurellement pas faire
//! attendre la boucle.
//!
//! **Aucun mutex ici, et c'est délibéré.** L'exclusivité — « il n'y a qu'un
//! câble » (spec §9) — vit dans le PUITS, au bloc E2, sous la forme d'un mutex
//! nommé à l'échelle de la machine : depuis D1, N fenêtres sont N PROCESSUS, et
//! un drapeau atomique ne garde rien entre eux. Ce module ne connaît de cette
//! exclusivité que sa couture : `PuitsMicro::deposer` rend `false` quand le
//! puits refuse, on le journalise UNE fois, et on n'insiste pas.
//!
//! ⚠️ **« On n'insiste pas » vaut du JOURNAL, plus du navigateur.** Depuis le
//! bloc E3, chaque CHANGEMENT du verdict du puits met en file un
//! `AgentControl::MicState` : le journal reste unique, l'annonce au client
//! suit les transitions. Voir `deposer_trame_micro`.

use str0m::media::MediaData;

use crate::micro::{PuitsMicro, TrameMicro};
use crate::opus::{echantillons_de, SAMPLE_RATE_HZ};

use super::Session;

// Les quatre champs que ce module ajoute à `Session` (`transport.rs`) n'y
// portent qu'une ligne de doc chacun ; leur raisonnement est ici.
//
// - `puits_micro` — absent tant qu'aucun puits n'a été installé : une session
//   sans micro reste une session vidéo parfaitement normale, et c'est ce qui
//   permet au chantier E de ne rien coûter aux sessions qui l'ignorent.
// - `warned_micro_negotiation` — une piste négociée sans puits, ou une horloge
//   RTP qui n'est pas 48 kHz, sont des conditions PERMANENTES : elles se disent
//   une fois, pas à chaque paquet. Calqué sur `warned_audio_negotiation`.
// - `refus_micro_signale` — même raison, pour le refus d'exclusivité.
//   ❌ **Ce commentaire disait « une autre fenêtre tient le câble POUR LA VIE
//   DE SON PROCESSUS », et c'était FAUX depuis la Décision 2 du bloc E2** :
//   la tentative d'acquisition y est devenue NON COLLANTE, refaite à chaque
//   dépôt, de sorte qu'un câble libéré est repris. Seul le JOURNAL est unique,
//   et c'est tout ce que ce drapeau garde. L'énoncé faux a survécu à E2 —
//   `git log` ne rend qu'un seul commit sur ce fichier, `784f1fc` (E1), et la
//   revue transverse de E2 ne le liste pas. Corrigé par E3.
// - `exclusivite_annoncee` — le dernier verdict DIT au navigateur. Distinct de
//   `refus_micro_signale`, et il faut les deux : l'un borne le journal à une
//   ligne, l'autre suit les transitions dans les DEUX sens.
// - `journaux_micro` — le compte des lignes RÉELLEMENT émises, incrémenté au
//   point d'émission et jamais à l'appel. C'est lui qui rend « l'avertissement
//   ne sort qu'une fois » assertable, donc capable de tomber : un compteur
//   d'appels aurait rendu le test vacueux.

impl Session {
    /// Installe le puits qui recevra les trames montantes.
    ///
    /// À poser AVANT `run()`, qui prend la session par valeur.
    pub fn set_puits_micro(&mut self, puits: Box<dyn PuitsMicro + Send>) {
        self.puits_micro = Some(puits);
    }

    /// Vrai quand une piste micro a été négociée ET qu'un puits est là pour la
    /// recevoir. Les deux conditions sont nécessaires : une piste sans puits ne
    /// mène nulle part, un puits sans piste ne recevra jamais rien.
    pub fn micro_disponible(&self) -> bool {
        self.mic_mid.is_some() && self.puits_micro.is_some()
    }

    /// Dépose un paquet Opus montant. **Ne rend rien, n'échoue jamais, et ne
    /// tue jamais la session** : un défaut du micro ne doit pas compromettre
    /// une session vidéo qui fonctionne (spec §10) — même règle que
    /// `write_audio`, pour la même raison.
    pub(super) fn deposer_micro(&mut self, data: &MediaData) {
        if self.puits_micro.is_none() {
            // Spec §10 : « piste montante non négociée → aucun paquet attendu,
            // avertissement UNIQUE ». Calqué sur `warn_audio_negotiation_once`.
            self.avertir_micro_une_fois(
                "piste micro négociée mais aucun puits installé, paquet abandonné",
            );
            return;
        }

        // ⚠️ L'horloge RTP est VÉRIFIÉE, pas supposée. Tout `micro.rs` raisonne
        // en échantillons à 48 kHz : un pair qui négocierait une autre horloge
        // ferait dériver la ligne de temps sans qu'aucune erreur ne le dise.
        if data.time.denom() != SAMPLE_RATE_HZ {
            self.avertir_micro_une_fois(
                "horloge RTP de la piste micro différente de 48 kHz, paquets abandonnés",
            );
            return;
        }

        // La durée est LUE du paquet, jamais supposée (spec §7).
        let echantillons = match echantillons_de(&data.data) {
            Ok(n) if n > 0 => n,
            _ => {
                self.avertir_micro_une_fois(
                    "paquet micro dont la durée Opus est illisible, paquets abandonnés",
                );
                return;
            }
        };

        self.deposer_trame_micro(TrameMicro {
            opus: data.data.to_vec(),
            rtp_48k: data.time.numer(),
            echantillons,
        });
    }

    /// Le dépôt lui-même, séparé de l'extraction depuis `MediaData`.
    ///
    /// ⚠️ **La séparation n'est pas cosmétique.** str0m interdit délibérément
    /// la construction d'un `MediaData` hors de son crate : sans cette
    /// fonction, les tests unitaires devaient passer par un point d'entrée
    /// `#[cfg(test)]` qui RECOPIAIT la logique — et ils exerçaient alors une
    /// copie, pas le chemin de production. La mutation « la session se termine
    /// quand le micro refuse » passait au vert sous ce montage, parce qu'elle
    /// frappait un code que les tests n'empruntaient pas (tâche 8, step 3).
    pub(super) fn deposer_trame_micro(&mut self, trame: TrameMicro) {
        // L'emprunt du puits se termine AVANT toute autre lecture de `self` :
        // sans cette liaison, l'emprunteur refuserait le `self.refus_micro_signale`
        // qui suit — même précaution que `select_negotiated_opus_pt`.
        let accepte = match self.puits_micro.as_mut() {
            Some(puits) => puits.deposer(trame),
            None => {
                self.avertir_micro_une_fois(
                    "piste micro négociée mais aucun puits installé, paquet abandonné",
                );
                return;
            }
        };

        if !accepte && !self.refus_micro_signale {
            self.refus_micro_signale = true;
            self.journaux_micro += 1;
            // UNE fois — mais **pas parce que le refus serait permanent**.
            //
            // ❌ Cette phrase disait « le refus est une condition PERMANENTE,
            // une autre fenêtre tient le câble pour la vie de son processus
            // (spec §9) ». **Faux depuis la Décision 2 du bloc E2** : la
            // tentative d'acquisition y est devenue NON COLLANTE, refaite à
            // chaque dépôt, si bien qu'une fenêtre qui meurt rend le câble et
            // que la suivante l'acquiert. Ce qui reste vrai de la spec §9 est
            // « journalisée UNE fois » ; « refusée » comme ÉTAT DÉFINITIF, non.
            //
            // Ce qui justifie l'unicité est donc plus étroit, et suffit : une
            // ligne par paquet ferait cinquante lignes par seconde, et la
            // reprise, elle, a sa propre ligne côté puits
            // (`micro : cable acquis apres un refus`).
            tracing::warn!(
                "le puits micro refuse les trames (exclusivité non acquise) : \
                 une autre fenêtre porte déjà le micro"
            );
        }

        // ✅ **Le refus EST dit au client depuis le bloc E3**, et cette moitié
        // du commentaire d'origine est devenue fausse à son tour — les deux
        // sont corrigées, pas l'une des deux.
        //
        // 🔴 **SUR TRANSITION, jamais à chaque dépôt.** Le micro dépose une
        // trame toutes les 20 ms ; annoncer à chaque dépôt mettrait cinquante
        // messages par seconde dans une file bornée à 32
        // (`PLAFOND_CONTROLE_EN_FILE`), qui déborderait en moins d'une seconde
        // et **noierait le curseur, la vibration et le presse-papier**.
        //
        // ⚠️ **`None` compte comme une transition, et c'est voulu** : le tout
        // premier dépôt annonce son verdict. Sans cela, une fenêtre qui perd
        // le câble dès son premier paquet n'apprendrait jamais rien —
        // `Ready.mic` a déjà été émis, et il dit `true`. C'est le patron
        // d'`Accent` : « au changement seulement, et sa PREMIÈRE lecture
        // comprise ».
        //
        // ⚠️ **La transition se dérive du BOOLÉEN, pas d'une `Issue` du puits.**
        // `PuitsCable::deposer` connaît ses quatre `Issue` mais ignore le canal
        // de contrôle ; ce module connaît le canal et ne voit qu'un booléen.
        // Enrichir le trait `PuitsMicro` pour transporter l'`Issue` jusqu'ici
        // aurait fait traverser la frontière à un vocabulaire dont ce module
        // n'a aucun usage : **la transition d'un booléen EST une transition**,
        // et les deux `Issue` de transition d'E2 (`AccepteApresRefus`,
        // `RefusePremierement`) sont exactement les deux changements de ce
        // booléen. Le trait ne bouge pas.
        if self.exclusivite_annoncee != Some(accepte) {
            self.exclusivite_annoncee = Some(accepte);
            self.queue_control(proto::control::AgentControl::mic_state(accepte));
        }
    }

    /// Point d'entrée `#[cfg(test)]` qui court-circuite `MediaData` — str0m
    /// interdit délibérément sa construction hors du crate.
    ///
    /// **Il DÉLÈGUE, il ne recopie pas** : c'est ce qui garantit que les tests
    /// unitaires exercent le chemin de production et non un jumeau. Même parti
    /// que `dispatch_controle_de_test` pour `ChannelData`.
    #[cfg(test)]
    pub(super) fn deposer_trame_micro_de_test(&mut self, trame: TrameMicro) {
        self.deposer_trame_micro(trame);
    }

    /// Journalise une fois, pas à chaque paquet.
    fn avertir_micro_une_fois(&mut self, message: &'static str) {
        if self.warned_micro_negotiation {
            return;
        }
        self.warned_micro_negotiation = true;
        self.journaux_micro += 1;
        tracing::warn!("{message}");
    }
}

#[cfg(test)]
#[path = "piste_micro/tests.rs"]
mod tests;
