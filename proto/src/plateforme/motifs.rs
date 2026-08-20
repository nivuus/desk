//! La table des motifs de refus du canal, et la correspondance mot ↔ variante.
//!
//! 🔴 **EXTRAIT AVANT L'ADDITION** — même raison que `champs.rs`, écrite là.
//! **Aucune ligne de comportement n'a changé** : l'enum et son `impl` sont
//! transposés mot pour mot, et le parent les réexporte par un `pub use`, de
//! sorte qu'aucun appelant, dans aucun paquet, n'a eu à bouger.

/// Pourquoi la plateforme refuse.
///
/// ⚠️ `Enrolement` NE DISTINGUE PAS « VM inconnue » de « secret faux », et
/// c'est délibéré : les distinguer donnerait à quiconque ouvre le canal un
/// oracle d'énumération des VMs enrôlées. Le diagnostic vit dans le journal de
/// la plateforme, jamais sur le fil.
/// ⚠️ **PLUS DE `Serialize`/`Deserialize` DEPUIS LA CORRECTION DU 20 AOÛT
/// 2026, ET C'EST DÉLIBÉRÉ.** Le motif voyage en MOT LIBRE dans
/// [`DepuisLaPlateforme::Refus`] (clause 2 de l'en-tête) : cet enum n'est plus
/// une forme de fil, c'est la table des motifs que NOUS savons interpréter.
/// La correspondance mot ↔ variante est écrite une seule fois, dans
/// [`MotifCanal::mot`] et [`MotifCanal::depuis_mot`], et un test la parcourt
/// dans les deux sens sur les quatre variantes — ce qu'un `rename_all` ne
/// permettait pas de faire rougir tant qu'aucune variante n'a deux mots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotifCanal {
    /// La version du message reçu n'est pas [`PLATEFORME_VERSION`].
    /// 🔴 CELUI-CI NE SE RÉESSAIE PAS.
    Version,
    /// Le message n'a pas la forme attendue.
    Forme,
    /// L'enrôlement est refusé. Indistinct par construction (voir ci-dessus).
    Enrolement,
    /// Un `battement` est arrivé avant tout `enroler`.
    Sequence,
}

impl MotifCanal {
    /// Les quatre variantes, dans l'ordre où un test les parcourt.
    ///
    /// 🔴 ANTI-OUBLI : une variante ajoutée sans sa ligne ici serait absente
    /// du test de correspondance, qui compare cette liste à un `match`
    /// EXHAUSTIF — le compilateur exige la branche, et le test exige l'entrée.
    pub const TOUS: [Self; 4] =
        [Self::Version, Self::Forme, Self::Enrolement, Self::Sequence];

    /// Le mot exact qui voyage sur le fil.
    pub fn mot(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Forme => "forme",
            Self::Enrolement => "enrolement",
            Self::Sequence => "sequence",
        }
    }

    /// Le motif que ce mot désigne, ou `None` si nous ne le connaissons pas.
    ///
    /// 🔴 `None` N'EST PAS UNE ERREUR : c'est un motif d'une version qui nous
    /// dépasse, et l'appelant doit le journaliser tel quel plutôt que de le
    /// perdre. C'est la clause 2 de l'en-tête de ce module.
    pub fn depuis_mot(mot: &str) -> Option<Self> {
        Self::TOUS.into_iter().find(|candidat| candidat.mot() == mot)
    }
}
