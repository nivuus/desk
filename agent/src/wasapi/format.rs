//! **PUR — aucun `cfg`.** Le format de mixage qu'exige l'écriture du micro sur
//! le câble, et le refus **nommé** quand il diffère.
//!
//! ⚠️ **Pourquoi cette règle vit ICI et non dans `wasapi/ecriture.rs`.** Le
//! plan E2 (tâche 7, step 1) demande une règle « PURE et testée sur l'hôte »,
//! et écrit vingt lignes plus bas que `wasapi/ecriture.rs` « ne se teste PAS
//! sur l'hôte » (`#![cfg(windows)]`). Les deux ne peuvent pas être vraies du
//! même fichier. Le précédent qui tranche est déjà dans ce répertoire :
//! `wasapi/peripherique.rs`, pur, hissé à la racine du crate par `#[path]`
//! (`main.rs`) précisément pour échapper au `#![cfg(windows)]` de `wasapi.rs`.
//! Cette règle-ci suit le même montage, sous le nom `wasapi_format`.
//!
//! ## Ce qui est accepté, et ce qui ne l'est PAS
//!
//! `LecteurMicro::remplir` rend du **stéréo entrelacé `f32` normalisé** à
//! 48 kHz. Le format de mixage relevé sur la VM le 20 août 2026 pour le point
//! de terminaison de rendu du câble est `48000 Hz, 2 canaux, 32 bits
//! flottant` : **aucune conversion n'est donc nécessaire sur cette machine**,
//! et l'on n'en écrit aucune. Écrire un convertisseur qu'aucune machine
//! n'exerce serait du travail posé avant d'avoir constaté le besoin — ce que
//! ce dépôt refuse, et ce qu'aucun test ne garderait honnête.
//!
//! Le prix de ce choix est qu'un autre format doit être **refusé
//! explicitement**, jamais subi : écrire des `f32` stéréo dans un tampon qui
//! attend autre chose ne produit pas un son dégradé, cela produit du bruit à
//! plein niveau dans l'oreille de quelqu'un. Le refus nomme la valeur
//! rencontrée, sur le patron du `bail!` de `LoopbackCapture::open`
//! (`agent/src/wasapi.rs`, contrôle de fréquence).

use anyhow::{bail, Result};

/// Le nombre de canaux que `LecteurMicro::remplir` produit, et le seul que
/// l'écriture sache poser tel quel.
pub const CANAUX: usize = crate::opus::CHANNELS;

/// La largeur d'échantillon attendue, en bits.
pub const BITS: u16 = 32;

/// Décrit un format de mixage comme le fait déjà `LoopbackCapture::open`, pour
/// que les deux moitiés du son se lisent de la même façon dans `agent.log`.
pub fn decrire(frequence: u32, canaux: usize, bits: u16, flottant: bool) -> String {
    format!(
        "{frequence} Hz, {canaux} canaux, {bits} bits, {}",
        if flottant { "flottant" } else { "entier" }
    )
}

/// Accepte le format de mixage, ou dit **précisément** ce qui cloche.
///
/// Les quatre grandeurs sont celles que `IAudioClient::GetMixFormat` rend, déjà
/// dépliées par l'appelant (le sous-format `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT`
/// d'un `WAVEFORMATEXTENSIBLE` est réduit au booléen `flottant`) : cette
/// fonction ne connaît aucun type Windows, c'est ce qui la rend éprouvable ici.
///
/// ⚠️ **Aucun repli, aucune conversion.** Voir l'en-tête de module : le refus
/// est le comportement voulu, et le message est la seule chose qui permette de
/// le corriger.
pub fn verifier(frequence: u32, canaux: usize, bits: u16, flottant: bool) -> Result<()> {
    let description = decrire(frequence, canaux, bits, flottant);
    let attendu = crate::opus::SAMPLE_RATE_HZ;
    if frequence != attendu {
        bail!(
            "format de mixage a {frequence} Hz : seul {attendu} Hz est supporte pour l'ecriture \
             du micro (aucun reechantillonneur n'est embarque, spec §4) [{description}]"
        );
    }
    if canaux != CANAUX {
        bail!(
            "format de mixage a {canaux} canaux : seul le stereo ({CANAUX} canaux) est supporte \
             pour l'ecriture du micro, `LecteurMicro::remplir` ne produisant que cela \
             [{description}]"
        );
    }
    if !flottant {
        bail!(
            "format de mixage entier {bits} bits non supporte pour l'ecriture du micro : \
             `LecteurMicro::remplir` rend des flottants normalises [{description}]"
        );
    }
    if bits != BITS {
        bail!(
            "format de mixage flottant {bits} bits non supporte pour l'ecriture du micro : \
             seul le {BITS} bits l'est [{description}]"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opus::SAMPLE_RATE_HZ;

    /// Le format RELEVÉ sur la VM le 20 août 2026 pour
    /// « Haut-parleurs (VB-Audio Virtual Cable) ».
    #[test]
    fn le_format_releve_sur_la_vm_est_accepte() {
        assert!(verifier(48_000, 2, 32, true).is_ok());
        assert_eq!(SAMPLE_RATE_HZ, 48_000, "la constante du codec a changé");
    }

    /// 🔴 Le cas de CABLE Output, l'autre bout : 44 100 Hz. Le message doit
    /// nommer la fréquence rencontrée — sans elle, une recette lit « format
    /// refusé » et ne sait pas quoi corriger.
    #[test]
    #[allow(non_snake_case)]
    fn une_frequence_autre_est_refusee_EN_LA_NOMMANT() {
        let e = verifier(44_100, 2, 32, true).unwrap_err().to_string();
        assert!(e.contains("44100"), "la fréquence rencontrée doit être nommée : {e}");
        assert!(e.contains("48000"), "la fréquence attendue doit être nommée : {e}");
        // ⚠️ La mutation M4 (« le motif ne nomme plus la fréquence
        // rencontrée ») a d'abord SURVÉCU : `[{description}]` la portait quand
        // même, et les deux `contains` ci-dessus passaient par elle. Le
        // relever ici plutôt que de durcir l'assertion, parce que
        // l'affirmation testée — « le message nomme la fréquence rencontrée »
        // — est vraie des deux façons, et que l'opérateur la lit dans les deux
        // cas. Ce qui NE serait pas vrai, c'est un message qui ne la porte
        // nulle part : c'est cet état-là que les deux `contains` interdisent,
        // et la mutation M4-bis (ni nom ni description) les tue.
    }

    /// Un mixage en entiers est un format WASAPI parfaitement légal, et il est
    /// refusé quand même : `remplir` rend des `f32`.
    ///
    /// ⚠️ **La largeur est 32 bits, et c'est TOUT L'INTÉRÊT du cas.** Une
    /// première rédaction éprouvait `(48 000, 2, 16, entier)` — refusé par
    /// DEUX gardes indépendantes, celle du flottant et celle des 32 bits — et
    /// la mutation M3 (« l'entier passe ») a **survécu** : la garde des bits
    /// rattrapait le cas, et le mot « entier » que le test cherchait venait de
    /// la description, pas du motif. `(48 000, 2, 32, entier)` ne peut être
    /// refusé que par la garde du flottant.
    #[test]
    fn un_format_entier_32_bits_est_refuse_par_la_SEULE_garde_du_flottant() {
        let e = verifier(48_000, 2, 32, false).unwrap_err().to_string();
        assert!(e.contains("entier"), "le motif doit nommer le format entier : {e}");
        assert!(
            !e.contains("flottant 32"),
            "ce n'est pas la garde des bits qui doit refuser ce cas : {e}"
        );
    }

    /// Et le 16 bits entier reste refusé, lui aussi.
    #[test]
    fn un_format_entier_16_bits_est_refuse() {
        assert!(verifier(48_000, 2, 16, false).is_err());
    }

    #[test]
    fn un_flottant_qui_n_est_pas_32_bits_est_refuse() {
        assert!(verifier(48_000, 2, 64, true).is_err());
    }

    /// 🔴 **Le mono est REFUSÉ, pas replié.** Poser un tampon stéréo entrelacé
    /// sur un point de terminaison mono jouerait un canal sur deux à double
    /// vitesse : pas un son dégradé, un son faux.
    #[test]
    fn le_mono_est_refuse_plutot_que_converti() {
        let e = verifier(48_000, 1, 32, true).unwrap_err().to_string();
        assert!(e.contains('1'), "le nombre de canaux rencontré doit être nommé : {e}");
    }

    #[test]
    fn le_multicanal_est_refuse() {
        assert!(verifier(48_000, 6, 32, true).is_err());
    }

    #[test]
    fn la_description_se_lit_comme_celle_du_loopback() {
        assert_eq!(decrire(48_000, 2, 32, true), "48000 Hz, 2 canaux, 32 bits, flottant");
        assert_eq!(decrire(44_100, 2, 16, false), "44100 Hz, 2 canaux, 16 bits, entier");
    }
}
