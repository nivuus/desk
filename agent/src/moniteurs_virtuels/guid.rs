//! Le GUID que nous attribuons à chaque sortie virtuelle, et sa lecture
//! inverse.
//!
//! Extrait de `pilote.rs` à la revue finale de branche : ce fichier-là était à
//! 493 lignes pour un plafond de 500 (voir `CLAUDE.md`), et le correctif I1 y
//! ajoutait le recyclage des numéros. L'addition s'accompagne donc de son
//! extraction. Rien n'a changé de valeur au passage.
//!
//! Sous `#[cfg(windows)]` non par choix mais par nécessité : `windows::core::GUID`
//! n'existe pas hors de cette cible (`Cargo.toml` gate le crate `windows` sur
//! `cfg(windows)`). La logique qui, elle, est pure — l'attribution et le
//! recyclage des numéros — vit dans le module frère `numeros`, hors de tout
//! `cfg`, et c'est elle qui porte les tests.

use windows::core::GUID;

/// Gabarit du GUID que nous attribuons à chaque sortie créée : les 16 bits de
/// poids faible portent un numéro, le reste est une constante arbitraire
/// choisie ici. Le GUID n'a besoin que d'être unique et reconnaissable — s'il
/// traîne un jour dans l'état du pilote, on saura d'où il vient.
///
/// Les numéros repartent de zéro à chaque exécution, et c'est un choix
/// assumé : deux exécutions attribuent donc les mêmes GUID. C'est ce
/// déterminisme qui donne à `purge::purger` un motif reconnaissable pour
/// retrouver nos sorties orphelines, qu'aucune table en mémoire ne peut plus
/// désigner.
const GABARIT_GUID_MONITEUR: u128 = 0x9c4a_1f6e_2b73_4d51_9e08_6775_4143_0000;

/// GUID attribué au moniteur portant ce `numero`.
///
/// `pub(crate)` : `purge.rs` calcule EXACTEMENT la même suite sans état
/// vivant — c'est ce déterminisme qui permet à une purge inter-processus de
/// retrouver les GUID d'une exécution tuée net.
pub(crate) fn guid_pour(numero: u16) -> GUID {
    GUID::from_u128(GABARIT_GUID_MONITEUR | u128::from(numero))
}

/// Numéro porté par un GUID que `guid_pour` a fabriqué, ou `None` si ce GUID
/// ne vient pas de ce gabarit.
///
/// Sert au recyclage (correctif I1) : `pilote::oublier` rend au distributeur
/// le numéro d'une sortie réellement retirée, et il n'a que le GUID sous la
/// main.
///
/// **La lecture est vérifiée, pas supposée.** Les 16 bits de poids faible d'un
/// `u128` atterrissent dans les deux derniers octets de `data4`
/// (`GUID::from_u128` y pose `(uuid as u64).to_be_bytes()`), mais on ne s'en
/// remet pas à ce raisonnement : le numéro relu est réinjecté dans `guid_pour`
/// et le GUID reconstruit doit être identique. Si la disposition supposée était
/// fausse, la fonction rend `None` — donc pas de recyclage, c'est-à-dire le
/// comportement d'avant ce correctif — jamais un numéro erroné qui ferait
/// réattribuer le GUID d'une sortie vivante.
pub(crate) fn numero_de(guid: GUID) -> Option<u16> {
    let numero = u16::from_be_bytes([guid.data4[6], guid.data4[7]]);
    (guid_pour(numero) == guid).then_some(numero)
}
