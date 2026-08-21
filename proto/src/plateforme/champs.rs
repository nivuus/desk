//! Les trois lecteurs de champ que `serde` appelle par `deserialize_with`, et
//! ce que chacun refuse.
//!
//! 🔴 **EXTRAIT AVANT L'ADDITION, JAMAIS APRÈS.** Le sous-bloc G3 ajoute cinq
//! variantes au protocole ; `plateforme.rs` était à 468 lignes, marge 32, et
//! les aurait fait franchir 500. La doctrine de `CLAUDE.md` est de rendre la
//! marge par une **extraction jouée d'avance**, jamais par une compression —
//! et le sous-bloc G2 a payé, quelques heures plus tôt, de ne pas l'avoir vue
//! venir sur ce fichier même (il l'a franchi à 588 puis extrait `apps`).
//!
//! ⚠️ Ce n'est PAS la « Convention de module enfant » de `CLAUDE.md`, qui vise
//! les modules extraits d'un parent `#[cfg(windows)]` : c'est le même mécanisme
//! employé pour l'autre raison — la règle des 500 lignes.
//!
//! 🔴 **AUCUNE LIGNE DE COMPORTEMENT N'A CHANGÉ**, et les trois fonctions sont
//! transposées mot pour mot. Le parent les réimporte par un `use`, ce qui fait
//! que les attributs `deserialize_with = "verifie_version"` des structures
//! **n'ont pas bougé d'un caractère** : `serde` résout le chemin dans la portée
//! du module qui porte l'attribut, et un `use` suffit à l'y remettre.

use serde::Deserialize;

use super::PLATEFORME_VERSION;

// Note : pas de `default` sur le champ `v` — un message sans champ `v` doit être
// rejeté (champ obligatoire), pas silencieusement complété avec la version
// courante. `default` court-circuiterait `deserialize_with` quand le champ est
// absent, ce qui romprait la vérification.
pub(super) fn verifie_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = u8::deserialize(deserializer)?;
    if v != PLATEFORME_VERSION {
        return Err(serde::de::Error::custom(format!(
            "version de plateforme non supportée : {v}"
        )));
    }
    Ok(v)
}

/// Lit le champ `v` d'un REFUS **sans le vérifier** — voir la clause 1 de
/// l'en-tête de ce module.
///
/// 🔴 CE N'EST PAS « SANS `v` » : le champ reste obligatoire et reste un
/// entier. Le rendre facultatif rouvrirait le trou que
/// [`verifie_version`] refuse — un `v: null`, ou un `v` absent, deviendrait
/// acceptable — et priverait le journal de la seule information qui dise
/// QUELLE version nous refuse.
pub(super) fn version_toleree<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    u8::deserialize(deserializer)
}

/// Lit une `Option<String>` en la gardant **OBLIGATOIRE sur le fil**.
///
/// 🔴 SANS CETTE FONCTION, LE CHAMP SERAIT SILENCIEUSEMENT FACULTATIF, ET LA
/// PLANIFICATION DE G2 SE TROMPAIT SUR CE POINT PRÉCIS. `serde_derive` traite
/// tout champ de type `Option<T>` comme portant un `#[serde(default)]`
/// IMPLICITE : un champ absent devient `None` sans qu'aucun `default` n'ait
/// été écrit, et `deny_unknown_fields` n'y change rien — il regarde les champs
/// EN TROP, jamais ceux qui manquent.
///
/// **Mesuré le 20 août 2026**, deux structures identiques à ce détail près,
/// `deny_unknown_fields` sur les deux :
/// ```text
/// Option<String> nue                          -> `{"a":1}` ACCEPTÉ
/// Option<String> + deserialize_with           -> `{"a":1}` REFUSÉ
/// Option<String> + deserialize_with, `b:null` -> Ok(None), et sérialise `"b":null`
/// ```
/// La seule chose que `deserialize_with` change est donc l'implicite : il
/// coupe le défaut, et le champ redevient exigé.
///
/// **Ce qui serait perdu sans elle** : le catalogue d'un agent v2 — six champs,
/// sans `icone` — serait accepté par une plateforme v3, avec une icône
/// silencieusement absente. C'est exactement le déguisement que le bump de
/// version existe pour empêcher, et la règle que ce module s'impose déjà pour
/// le champ `v` : **un champ absent se refuse, il ne se complète pas**.
pub(super) fn icone_obligatoire<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // ⚠️ DÉLÈGUE À LA FORME GÉNÉRIQUE CI-DESSOUS depuis le sous-bloc G3, qui en
    // avait besoin pour un `Option<i32>` et pour un second `Option<String>`.
    // Le nom d'origine est CONSERVÉ parce qu'il est cité par l'attribut
    // `deserialize_with = "icone_obligatoire"` du champ qu'il garde, et que le
    // renommer aurait été une addition de risque pour un gain de style.
    option_obligatoire(deserializer)
}

/// La forme GÉNÉRIQUE de la fonction ci-dessus : rend un champ de type
/// `Option<T>` **obligatoire sur le fil**, quel que soit `T`.
///
/// 🔴 SANS ELLE, LE CHAMP SERAIT SILENCIEUSEMENT FACULTATIF. Le raisonnement
/// entier est écrit au-dessus, pour `icone` — il ne dépend en rien du type, et
/// il vaut mot pour mot pour le `motif` et le `code_sortie` du sous-bloc G3 :
/// un `termine` sans `motif` serait accepté avec un motif absent, ce qui est
/// exactement le déguisement que le bump de version existe pour empêcher.
///
/// ⚠️ `null` RESTE ACCEPTÉ, ET C'EST VOULU : « le champ est là et il ne porte
/// rien » est un fait, « le champ manque » en est un autre. C'est cette
/// distinction seule que cette fonction rétablit.
pub(super) fn option_obligatoire<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
