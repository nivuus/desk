//! L'icône d'une application : l'extraire, l'encoder, et PROUVER d'où elle
//! vient.
//!
//! ⚠️ CE MODULE EST DÉCLARÉ SANS `cfg` DANS `apps.rs`, ET CE SONT SES PARTIES
//! WINDOWS QUI PORTENT LE LEUR. C'est la figure exacte d'`apps.rs` lui-même,
//! un cran plus bas, et c'est ce qui fait exister `apps::icone::ressource`,
//! `::magasin` et `::source` sur l'hôte Linux, où leurs tests courent.
//!
//! 🔴 **DIVERGENCE AVEC LE PLAN DE G2, RELEVÉE ET TRANCHÉE PLUTÔT QUE
//! RECOPIÉE.** Le plan écrit deux choses qui ne tiennent pas ensemble : que ce
//! fichier-ci est `#[cfg(windows)]`, ET que ses enfants purs sont déclarés
//! « dans `apps.rs` … et cela évite le `#[path]` ». Or on ne peut pas déclarer
//! un PETIT-fils depuis le grand-parent sans `#[path]` : si `icone` était gaté,
//! `apps::icone::ressource` n'existerait pas sur l'hôte, et l'y faire vivre
//! demanderait précisément le `#[path]` que le plan dit éviter. Ce qui est
//! retenu est donc son INTENTION — les modules purs existent sur l'hôte, et
//! aucun `#[path]` n'est employé — par la seule construction qui la serve : le
//! parent est libre de `cfg`, ses parties Windows sont gatées à l'intérieur.
//!
//! La « Convention de module enfant » de `CLAUDE.md` n'est **pas** mobilisée :
//! aucun module ne franchit ici de frontière `#[cfg(windows)]`.

/// 🔴 LA PREUVE DU SOUS-BLOC, ET ELLE EST PURE : lire un `GRPICONDIR` ou un
/// `ICONDIR` depuis un `&[u8]`.
pub mod ressource;

/// Le magasin en mémoire, adressé par contenu. **PUR.**
pub mod magasin;
/// D'où vient l'icône d'un raccourci. **PUR.**
pub mod source;

// ---------------------------------------------------------------------------
// Ce qui suit est `#[cfg(windows)]` : l'extraction par le Shell, l'encodage
// PNG par WIC, et la lecture de la ressource. Rien n'y DÉCIDE — les décisions
// vivent dans les trois modules purs ci-dessus.
// ---------------------------------------------------------------------------

/// Les octets bruts d'un `GRPICONDIR`. **`#[cfg(windows)]`.**
#[cfg(windows)]
pub mod lecture_pe;

#[cfg(windows)]
mod extraction;

#[cfg(windows)]
pub use extraction::{armee, extraire, provenance_de};

/// Le téléversement HTTP des icônes vers la plateforme.
///
/// ⚠️ SANS `cfg` : ses deux parties PURES — la dérivation de l'autorité et la
/// lecture du statut — se testent sur l'hôte, et ce sont elles qui décident du
/// refus de TLS.
pub mod televersement;
