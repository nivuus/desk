//! Les deux types purs de `capture.rs` qui ne touchent à aucun champ privé de
//! `DesktopCapture` : pourquoi une acquisition échoue, et ce qu'une
//! duplication couvre.
//!
//! Extrait à la tâche 6 quater du sous-bloc D2 : `EchecAcquisition` portant
//! désormais le HRESULT nu de l'échec qui a motivé la réouverture (et sa
//! documentation associée), le fichier parent dépassait le plafond de 500
//! lignes (`CLAUDE.md`). Même raison d'extraction que `capture/ouverture.rs` :
//! aucun de ces deux types n'a besoin d'être dans le fichier qui définit
//! `DesktopCapture`.

/// Pourquoi une acquisition d'image a échoué, une fois les reprises épuisées.
///
/// `AccesPerdu` ne remonte pas à la première perte : `next_frame` tente de se
/// rouvrir d'abord (voir `FenetreDeReprise`). Le recevoir signifie « je n'ai
/// pas pu revenir dans le délai imparti », pas « l'accès vient d'être perdu ».
///
/// Porte le HRESULT nu (`e.code().0`) qui a motivé la réouverture : sans lui,
/// diagnostiquer un épuisement de la fenêtre de reprise oblige à relire le
/// code pour deviner le code d'erreur, comme l'a dû faire le rapport de la
/// mesure du 1ᵉʳ août 2026.
pub enum EchecAcquisition {
    AccesPerdu(i32),
    Panne(anyhow::Error),
}

impl std::fmt::Display for EchecAcquisition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccesPerdu(code) => write!(
                f,
                "accès à la duplication perdu (hresult={code:#010x}) et non repris en {:?}",
                crate::capture_reprise::DUREE_FENETRE_REPRISE
            ),
            Self::Panne(e) => write!(f, "{e:#}"),
        }
    }
}

/// Ce que cette duplication couvre — et donc ce qu'il faut rouvrir après une
/// perte d'accès.
///
/// **Un nom de sortie, jamais un index.** `(index_adaptateur, index_sortie)`
/// est positionnel : il change dès qu'une sortie apparaît ou disparaît. Or
/// c'est exactement ce qui vient de se produire quand on rouvre. `\\.\DISPLAYn`
/// est stable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CibleCapture {
    /// La sortie qui compose le bureau, quelle qu'elle soit — comportement de
    /// `DesktopCapture::new()`. Résolue à chaque ouverture, donc une
    /// réouverture peut légitimement tomber sur une autre sortie.
    Bureau,
    /// Une sortie précise, désignée par son nom.
    Sortie(String),
}
