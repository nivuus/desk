//! L'injection de fautes de surveillance — variable de banc `APPS_FAUTE`,
//! **jamais une configuration livrée**.
//!
//! Elle rend atteignables trois chemins de code que la machine ne produit pas
//! d'elle-même : le débordement de tampon, la complétion **avalée**, et la
//! perte d'un handle. Patron **verbatim** de
//! `agent/src/transport/piste_audio/injection.rs`.
//!
//! 🔴 **CE QUE L'INJECTION ÉTABLIT, ET CE QU'ELLE N'ÉTABLIT PAS.** Elle établit
//! que le **REMÈDE** fonctionne, jamais qu'une **CAUSE** existe. C'est la phrase
//! que le sous-bloc D11 a écrite d'`AUDIO_FAUTE_RECONSTRUCTION`, et elle vaut
//! ici mot pour mot : une ligne `notifications perdues` produite par injection
//! **ne dit rien** de la probabilité qu'un débordement réel se produise sur
//! cette machine. Seule une rafale réelle le dit, et son verdict peut être NON
//! MESURABLE.
//!
//! ⚠️ **Convention `absente = désarmée`** — celle d'`AUDIO_FAUTE_LECTURE`,
//! d'`AUDIO_FAUTE_RECONSTRUCTION` et d'`INSTALLATION_FAUTE`. **Jamais** celle
//! de `PLEIN_ECRAN` : ici l'absence n'est pas un désarmement de mécanisme
//! livré, c'est l'état nominal d'un instrument qui n'existe que pour un banc.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::OnceLock;

/// Ce qu'on fait dire à la prochaine complétion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Famille {
    /// Les *n* prochaines complétions sont traitées comme des **débordements** :
    /// comptées, journalisées, **et déclenchantes**. Rend atteignable le chemin
    /// du critère ② quand la rafale réelle ne déborde pas.
    Debordement,
    /// 🔵 Les *n* prochaines complétions sont **AVALÉES** : ni comptées, ni
    /// journalisées, ni déclenchantes.
    ///
    /// 🔴 C'EST LE SEUL MONTAGE QUI RENDE LE CRITÈRE ③ DISCRIMINANT, et la
    /// raison tient en une phrase : dans cette conception, **un débordement est
    /// lui-même une complétion**, donc un déclencheur, et toute réconciliation
    /// relit le disque entier — un débordement se répare donc tout seul. La
    /// seule panne que la réconciliation périodique achète réellement est
    /// **une surveillance qui cesse de délivrer SANS ERREUR**, et c'est
    /// exactement ce que cette famille fabrique.
    Muette,
    /// Les *n* prochaines complétions rendent une erreur fatale de handle : la
    /// racine passe en échec, et son rétablissement devient observable.
    Perte,
}

impl Famille {
    /// Le code stocké dans l'atomique. **`0` est réservé à « aucune ».**
    fn code(self) -> u8 {
        match self {
            Famille::Debordement => 1,
            Famille::Muette => 2,
            Famille::Perte => 3,
        }
    }

    fn depuis_code(code: u8) -> Option<Famille> {
        match code {
            1 => Some(Famille::Debordement),
            2 => Some(Famille::Muette),
            3 => Some(Famille::Perte),
            _ => None,
        }
    }
}

/// Analyse `APPS_FAUTE`. **PURE** — elle ne lit ni l'environnement, ni l'état.
///
/// ⚠️ `"debordement:0"` REND `None`, ET C'EST TRANCHÉ ICI PLUTÔT QUE SUBI : un
/// budget de zéro est une injection qui ne tirera jamais, et la déclarer
/// « ARMÉE » au journal ferait lire un armement à qui n'en a aucun. Le
/// comportement est donc celui de l'absence, exactement.
///
/// ⚠️ UNE FAMILLE INCONNUE SE PLAINT, elle ne se tait pas : sans le `warn!`, une
/// coquille (`debordment:3`) désarmerait l'injection en silence et la recette
/// lirait un zéro qui ne veut rien dire.
pub fn lire(valeur: Option<&str>) -> Option<(Famille, u32)> {
    let valeur = valeur?;
    let (nom, compte) = valeur.split_once(':')?;
    let famille = match nom {
        "debordement" => Famille::Debordement,
        "muette" => Famille::Muette,
        "perte" => Famille::Perte,
        _ => {
            tracing::warn!(
                valeur,
                "APPS_FAUTE : famille inconnue, injection DESARMEE \
                 (attendu : debordement:<n>, muette:<n> ou perte:<n>)"
            );
            return None;
        }
    };
    let compte: u32 = compte.parse().ok()?;
    if compte == 0 {
        return None;
    }
    Some((famille, compte))
}

/// L'état d'injection, **GLOBAL AU PROCESSUS**.
///
/// 🔴 **GLOBAL, ET LA RAISON EST MESURÉE AILLEURS.** Le sous-bloc D10 a payé un
/// budget relu **par fil** sur `AUDIO_FAUTE_LECTURE` : chaque capture
/// reconstruite recevait un budget neuf, et **le chiffre-juge était
/// structurellement incapable de quitter zéro, sur un produit pourtant
/// corrigé**. Ici la surveillance **se rouvre** après une perte : un budget
/// relu à la réouverture se réarmerait à l'identique, et la même panne de
/// mesure se rejouerait.
///
/// Deux atomiques plutôt qu'un `Option<Famille>` : c'est ce qui permet aux
/// tests de **seeder l'état directement** plutôt que l'environnement — un
/// `OnceLock` déjà initialisé ne relirait de toute façon plus `std::env`, et
/// c'est la note que porte `piste_audio/injection.rs`.
fn etat() -> &'static (AtomicU8, AtomicU32) {
    static ETAT: OnceLock<(AtomicU8, AtomicU32)> = OnceLock::new();
    ETAT.get_or_init(|| {
        let brut = std::env::var("APPS_FAUTE").ok();
        match lire(brut.as_deref()) {
            Some((famille, compte)) => {
                tracing::warn!(
                    ?famille,
                    fautes_a_injecter = compte,
                    "faute de surveillance ARMEE (APPS_FAUTE) : banc, jamais une configuration livrée"
                );
                (AtomicU8::new(famille.code()), AtomicU32::new(compte))
            }
            None => (AtomicU8::new(0), AtomicU32::new(0)),
        }
    })
}

/// Consomme une faute de cette famille, ou rend `false`.
///
/// `fetch_update` avec `checked_sub(1)` décrémente atomiquement SI le budget
/// global n'est pas déjà à zéro, et rend `Err` sans y toucher sinon : un budget
/// épuisé — le cas nominal, la variable étant absente — laisse donc passer
/// l'appel réel dès le premier tour, sans surcoût mesurable.
pub fn consommer(famille: Famille) -> bool {
    let (armee, reste) = etat();
    if Famille::depuis_code(armee.load(Ordering::Relaxed)) != Some(famille) {
        return false;
    }
    reste
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lire_reconnait_les_trois_familles_et_leur_compte() {
        assert_eq!(lire(Some("debordement:3")), Some((Famille::Debordement, 3)));
        assert_eq!(lire(Some("muette:1")), Some((Famille::Muette, 1)));
        assert_eq!(lire(Some("perte:2")), Some((Famille::Perte, 2)));
    }

    #[test]
    fn lire_desarme_sur_tout_ce_qui_n_est_pas_une_famille_suivie_d_un_compte() {
        assert_eq!(lire(None), None, "absente = DÉSARMÉE");
        assert_eq!(lire(Some("debordement")), None, "sans compte : rien à tirer");
        assert_eq!(lire(Some("debordement:")), None);
        assert_eq!(lire(Some("debordement:x")), None);
        // ⚠️ TRANCHÉ : un budget de zéro vaut l'absence, et ne se déclare pas
        // « ARMÉE ». Un armement qui ne tirera jamais serait un armement faux
        // au journal.
        assert_eq!(lire(Some("debordement:0")), None);
        // Famille inconnue : `None`, ET un `warn!` qui la nomme.
        assert_eq!(lire(Some("debordment:3")), None);
        assert_eq!(lire(Some("muette")), None);
    }

    /// 🔴 LE TEST QUI FIXE QUE LE BUDGET EST GLOBAL AU PROCESSUS.
    ///
    /// Sa ROUGE est un budget **relu à chaque appel** : la panne que D10 a
    /// payée sur `AUDIO_FAUTE_LECTURE`. Sous elle, l'état n'étant pas seedé
    /// depuis l'environnement, le tout premier `consommer` rendrait déjà
    /// `false` — et un chiffre-juge bâti dessus ne pourrait jamais quitter
    /// zéro, sur un produit pourtant correct.
    ///
    /// ⚠️ L'état est SEEDÉ DIRECTEMENT, jamais par `std::env` : un `OnceLock`
    /// déjà initialisé ne relit plus l'environnement, et deux tests qui s'en
    /// remettraient à lui se voleraient leur budget l'un à l'autre.
    #[test]
    fn le_budget_est_global_au_processus_et_s_epuise_une_seule_fois() {
        let (armee, reste) = etat();
        armee.store(Famille::Perte.code(), Ordering::Relaxed);
        reste.store(1, Ordering::Relaxed);
        assert!(consommer(Famille::Perte), "le budget de 1 doit tirer une fois");
        assert!(
            !consommer(Famille::Perte),
            "et une seule : un budget global s'épuise pour tout le processus"
        );
        armee.store(0, Ordering::Relaxed);
        reste.store(0, Ordering::Relaxed);
    }

    /// Une famille armée n'en sert aucune autre : demander `Muette` quand
    /// `Debordement` est armé ne consomme rien, et ne ment pas.
    #[test]
    fn une_famille_ne_consomme_pas_le_budget_d_une_autre() {
        let (armee, reste) = etat();
        armee.store(Famille::Debordement.code(), Ordering::Relaxed);
        reste.store(5, Ordering::Relaxed);
        assert!(!consommer(Famille::Muette));
        assert!(!consommer(Famille::Perte));
        assert_eq!(reste.load(Ordering::Relaxed), 5, "le budget n'a pas bougé");
        assert!(consommer(Famille::Debordement));
        armee.store(0, Ordering::Relaxed);
        reste.store(0, Ordering::Relaxed);
    }

    #[test]
    fn un_etat_desarme_ne_consomme_rien() {
        let (armee, reste) = etat();
        armee.store(0, Ordering::Relaxed);
        reste.store(0, Ordering::Relaxed);
        for famille in [Famille::Debordement, Famille::Muette, Famille::Perte] {
            assert!(!consommer(famille), "{famille:?} sur un état désarmé");
        }
    }

    #[test]
    fn le_code_zero_est_reserve_a_aucune_famille() {
        assert_eq!(Famille::depuis_code(0), None);
        for famille in [Famille::Debordement, Famille::Muette, Famille::Perte] {
            assert_ne!(famille.code(), 0);
            assert_eq!(Famille::depuis_code(famille.code()), Some(famille));
        }
    }
}
