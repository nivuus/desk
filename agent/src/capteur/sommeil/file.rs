//! La file des messages d'une session, BORNÉE PAR COALESCENCE.
//!
//! 🔴 POURQUOI CE MODULE EXISTE. Le canal du registre était un
//! `std::sync::mpsc::channel()` NON BORNÉ, et quatre sous-blocs y ont ajouté
//! chacun une variante. Trois d'entre elles sont poussées « au changement
//! seulement » et n'ont de valeur que dans leur DERNIÈRE occurrence : sous
//! coalescence, elles cessent de faire croître la file en régime permanent,
//! quelle que soit la cadence d'arrivée.
//!
//! 🔴 LA COALESCENCE CONSERVE LA POSITION, ET C'EST L'INVARIANT DU CANAL.
//! `sommeil.rs` écrit qu'au sein d'une session, un canal unique garantit
//! l'ORDRE DE LIVRAISON entre les variantes. Remplacer en place préserve cet
//! ordre ; déplacer en queue ferait franchir à une part de débit un ordre de
//! dormir déposé entre-temps, et livrerait la part APRÈS l'ordre qui aurait dû
//! la rendre caduque.
//!
//! ⚠️ SÉPARER LES VARIANTES EN CANAUX DISTINCTS DÉTRUIRAIT CET INVARIANT.
//! C'est la solution qui vient d'abord à l'esprit, et elle est fausse.
//!
//! ⚠️ CE MODULE EST PUR : il ne connaît ni verrou, ni fil, ni Windows. Le
//! verrou et le réveil vivent chez son appelant.

use std::collections::VecDeque;

use super::Message;

/// La profondeur au-delà de laquelle un dépôt est REFUSÉ.
///
/// ⚠️ **NON CALIBRÉE.** Aucune constante de ce dépôt ne l'est. Elle est choisie
/// assez grande pour qu'un régime normal ne l'atteigne jamais — les variantes
/// coalescables n'y contribuent pas — et assez petite pour que la mémoire reste
/// bornée si un enfant cesse de lire.
pub(crate) const PROFONDEUR_MAX: usize = 64;

/// Ce qu'un dépôt a fait.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Depot {
    /// Ajouté en queue.
    Empilee,
    /// A remplacé, EN PLACE, un message de la même variante déjà en attente.
    Coalescee,
    /// La file était pleine. **Rien n'a été ajouté, rien n'a été retiré.**
    Refusee,
}

/// Cette variante peut-elle remplacer une occurrence en attente d'elle-même ?
///
/// 🔴 LA RÈGLE EST « SA PERTE COÛTE-T-ELLE QUELQUE CHOSE ? », PAS « EST-ELLE
/// FRÉQUENTE ? ». `Part` et `Audio` sont poussées au changement seulement et
/// n'ont de valeur que dans leur dernière occurrence. `Sommeil` porte un ORDRE,
/// `PressePapier` porte la DONNÉE DE L'UTILISATEUR : ni l'un ni l'autre ne se
/// remplace.
///
/// ⚠️ TOUTE VARIANTE NEUVE DOIT PASSER ICI, et le `match` est EXHAUSTIF pour
/// que le compilateur l'exige — jamais un `_ => false`, qui la classerait
/// « à conserver » en silence et laisserait la file recroître.
pub(crate) fn coalescable(m: &Message) -> bool {
    match m {
        Message::Part { .. } | Message::Audio { .. } => true,
        Message::Sommeil(_) | Message::PressePapier { .. } => false,
    }
}

/// Deux messages sont-ils de la même variante ?
fn meme_variante(a: &Message, b: &Message) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Dépose un message, en appliquant la politique de sa variante.
pub(crate) fn deposer(file: &mut VecDeque<Message>, message: Message) -> Depot {
    if coalescable(&message) {
        if let Some(place) = file.iter().position(|en_attente| meme_variante(en_attente, &message)) {
            file[place] = message;
            return Depot::Coalescee;
        }
    }
    if file.len() >= PROFONDEUR_MAX {
        return Depot::Refusee;
    }
    file.push_back(message);
    Depot::Empilee
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capteur::sommeil::{Message, Ordre};
    use std::collections::VecDeque;

    /// 🔴 LE CŒUR DE LA DÉCISION : deux `Part` sans lecture n'en laissent
    /// qu'UNE, et c'est la DERNIÈRE valeur qui survit.
    #[test]
    fn deux_parts_se_coalescent_en_une_seule() {
        let mut f = VecDeque::new();
        assert!(matches!(deposer(&mut f, Message::Part { bps: 1 }), Depot::Empilee));
        assert!(matches!(deposer(&mut f, Message::Part { bps: 2 }), Depot::Coalescee));
        assert_eq!(f.len(), 1);
        assert!(matches!(f[0], Message::Part { bps: 2 }));
    }

    /// 🔴 LE CRITÈRE QUI DISTINGUE LA COALESCENCE EN PLACE DE CELLE EN QUEUE,
    /// et c'est l'invariant que `sommeil.rs` écrit : au sein d'une session, le
    /// canal garantit l'ORDRE DE LIVRAISON entre les variantes. Coalescer en
    /// queue ferait franchir à la part un ordre de dormir déposé entre-temps.
    #[test]
    fn la_coalescence_conserve_la_position() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::Part { bps: 1 });
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        deposer(&mut f, Message::Part { bps: 2 });
        assert_eq!(f.len(), 2);
        assert!(matches!(f[0], Message::Part { bps: 2 }), "la part garde sa PLACE");
        assert!(matches!(f[1], Message::Sommeil(Ordre::Reveiller)));
    }

    /// Un ordre perdu laisse une fenêtre endormie ou éveillée à tort.
    #[test]
    fn un_ordre_de_sommeil_n_est_jamais_coalesce() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        assert!(matches!(deposer(&mut f, Message::Sommeil(Ordre::Reveiller)), Depot::Empilee));
        assert_eq!(f.len(), 2);
    }

    /// Un presse-papier perdu, c'est la donnée de l'utilisateur.
    #[test]
    fn un_presse_papier_n_est_jamais_coalesce() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::PressePapier { texte: Some("a".into()), octets: 1 });
        let d = deposer(&mut f, Message::PressePapier { texte: Some("b".into()), octets: 1 });
        assert!(matches!(d, Depot::Empilee));
        assert_eq!(f.len(), 2);
    }

    /// La borne dure REFUSE, elle ne tronque pas en silence.
    #[test]
    fn au_dela_de_la_borne_le_depot_est_refuse() {
        let mut f = VecDeque::new();
        for _ in 0..PROFONDEUR_MAX {
            deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        }
        assert!(matches!(deposer(&mut f, Message::Sommeil(Ordre::Reveiller)), Depot::Refusee));
        assert_eq!(f.len(), PROFONDEUR_MAX, "la file n'a pas grossi");
    }

    /// 🔴 LE TÉMOIN NÉGATIF : une variante coalescable ne bute JAMAIS sur la
    /// borne, quelle que soit la cadence. Sans lui, « refusée » au-dessus ne
    /// dirait pas que la coalescence borne réellement.
    #[test]
    fn une_variante_coalescable_ne_bute_jamais_sur_la_borne() {
        let mut f = VecDeque::new();
        for i in 0..(PROFONDEUR_MAX * 10) {
            let d = deposer(&mut f, Message::Part { bps: i as u32 });
            assert!(!matches!(d, Depot::Refusee));
        }
        assert_eq!(f.len(), 1);
    }
}
