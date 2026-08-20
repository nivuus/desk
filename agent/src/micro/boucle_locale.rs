//! **PUR — aucun `cfg`.** La garde de boucle locale : le point de terminaison
//! sur lequel E2 va écrire est-il celui que le loopback du chantier A capte ?
//!
//! 🔴 **Mesuré nécessaire le 20 août 2026, et la spec dit le contraire.** Son
//! §3 affirme « CABLE Input n'est pas le périphérique de rendu par défaut,
//! donc le loopback du chantier A ne le capture pas. […] Aucune boucle locale
//! n'est créée par construction » — **faux sur les trois phrases** : le relevé
//! du 20 août 2026 rend `Haut-parleurs (VB-Audio Virtual Cable)` comme rendu
//! par défaut sur les **trois** rôles, et `LoopbackCapture::open` capte le
//! défaut de Windows quand `AUDIO_PERIPHERIQUE` est absente. En mode
//! mono-fenêtre, l'agent capterait donc la voix que E2 vient d'écrire et la
//! renverrait au navigateur, avec la latence du tour complet ; sans casque, la
//! boucle acoustique se refermerait par les haut-parleurs.
//!
//! **C'est le MICRO qui cède, et jamais le son** (Décision 3) : le son est un
//! chantier livré depuis A, le micro est ce qu'on ajoute.
//!
//! ⚠️ **Ne pas confondre avec l'écho de la Décision 7 de E1** — celui-là est
//! acoustique, multi-fenêtres et hors de notre portée ; celui-ci est
//! intra-agent, mono-fenêtre, et de notre fait.
//!
//! ⚠️ **Le multi-fenêtres est structurellement à l'abri** :
//! `pour_processus` n'a jamais résolu d'endpoint —
//! `ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK)` vise
//! un **arbre de processus**. La boucle n'existe que dans le mode
//! mono-fenêtre, c'est-à-dire dans le mode où toutes les recettes audio de ce
//! dépôt se jouent.

/// Ce que la garde conclut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boucle {
    /// Rien à craindre : process loopback, audio coupé, ou deux endpoints
    /// distincts.
    Aucune,
    /// Le loopback capterait le câble sur lequel on va écrire.
    Risque,
}

/// `capte` est l'identifiant d'endpoint que le loopback de CE processus
/// capterait, ou `None` quand il n'en capte aucun (process loopback, ou
/// `AUDIO=0`). `cable` est l'identifiant du câble.
///
/// ⚠️ **Comparaison sur l'IDENTIFIANT, jamais sur le nom** : deux périphériques
/// peuvent porter le même nom convivial — la VM en porte deux commençant par
/// « Haut-parleurs (…) » —, et le dépôt a payé en D1 pour avoir désigné une
/// sortie par un rang plutôt que par un identifiant stable.
pub fn evaluer(capte: Option<&str>, cable: &str) -> Boucle {
    let Some(capte) = capte else {
        return Boucle::Aucune;
    };
    // ⚠️ Le vide n'est PAS un identifiant : `decrire` rend `String::new()`
    // sur un périphérique indescriptible, et deux vides seraient égaux. Sans
    // cet écart, un agent dont le périphérique capté est indescriptible
    // perdrait son micro sans qu'aucune boucle n'existe.
    if capte.is_empty() || cable.is_empty() {
        return Boucle::Aucune;
    }
    if capte.eq_ignore_ascii_case(cable) {
        Boucle::Risque
    } else {
        Boucle::Aucune
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les identifiants sont ceux **relevés sur la VM le 20 août 2026** par
    /// `micro-format-e1.ps1` : le câble et les haut-parleurs Steam.
    const CABLE: &str = "{0.0.0.00000000}.{deec1914-6490-47ee-9475-091b9a2ea537}";
    const AUTRE: &str = "{0.0.0.00000000}.{8695a111-abf2-4199-88c0-fe4a9176f3e9}";

    /// 🔴 **Le défaut mesuré le 20 août 2026.**
    #[test]
    #[allow(non_snake_case)]
    fn le_cable_capte_par_le_loopback_est_un_RISQUE() {
        assert_eq!(evaluer(Some(CABLE), CABLE), Boucle::Risque);
    }

    #[test]
    fn deux_endpoints_distincts_ne_bouclent_pas() {
        assert_eq!(evaluer(Some(AUTRE), CABLE), Boucle::Aucune);
    }

    #[test]
    fn sans_capture_il_n_y_a_pas_de_boucle() {
        assert_eq!(evaluer(None, CABLE), Boucle::Aucune);
    }

    /// Windows rend ses GUID d'endpoint en minuscules, mais rien ne l'y
    /// oblige. Les identifiants sont de l'ASCII pur — accolades, points,
    /// chiffres hexadécimaux —, d'où `eq_ignore_ascii_case`.
    #[test]
    #[allow(non_snake_case)]
    fn la_comparaison_est_INSENSIBLE_a_la_casse_de_l_identifiant() {
        assert_eq!(evaluer(Some(&CABLE.to_uppercase()), CABLE), Boucle::Risque);
    }

    /// 🔴 Ce test ne se déduit PAS de l'énoncé : il vient d'une propriété
    /// **écrite** de `wasapi::rendu::decrire`, « ne rend jamais d'erreur […]
    /// une chaîne vide ». Deux vides seraient égaux et déclencheraient un faux
    /// `Risque` — un agent dont le périphérique capté est indescriptible
    /// perdrait son micro sans raison.
    #[test]
    #[allow(non_snake_case)]
    fn un_identifiant_VIDE_ne_boucle_pas() {
        assert_eq!(evaluer(Some(""), ""), Boucle::Aucune);
        assert_eq!(evaluer(Some(""), CABLE), Boucle::Aucune);
        assert_eq!(evaluer(Some(CABLE), ""), Boucle::Aucune);
    }
}
