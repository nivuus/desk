//! The table of the channel's refusal reasons, and the word ↔ variant mapping.
//!
//! 🔴 **EXTRACTED BEFORE THE ADDITION** — same reason as `champs.rs`, written there.
//! **No line of behaviour has changed**: the enum and its `impl` are
//! transposed word for word, and the parent re-exports them through a `pub use`, so
//! that no caller, in any package, had to move.

/// Pourquoi la plateforme refuse.
///
/// ⚠️ `Enrolement` DOES NOT DISTINGUISH "unknown VM" from "wrong secret", and
/// it is deliberate: distinguishing them would give whoever opens the channel an
/// oracle for enumerating the enrolled VMs. The diagnosis lives in the log of
/// the platform, never on the wire.
/// ⚠️ **NO MORE `Serialize`/`Deserialize` SINCE THE FIX OF 20 AUGUST
/// 2026, AND IT IS DELIBERATE.** The reason travels as a FREE WORD in
/// [`DepuisLaPlateforme::Refus`] (clause 2 of the header): this enum is no longer
/// a wire shape, it is the table of reasons WE know how to interpret.
/// The word ↔ variant mapping is written only once, in
/// [`MotifCanal::mot`] and [`MotifCanal::depuis_mot`], and a test walks it
/// in both directions over the four variants — which a `rename_all` could not
/// make turn red as long as no variant has two words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotifCanal {
    /// The version of the received message is not [`PLATEFORME_VERSION`].
    /// 🔴 THIS ONE IS NOT RETRIED.
    Version,
    /// The message does not have the expected shape.
    Forme,
    /// The enrolment is refused. Indistinct by construction (see above).
    Enrolement,
    /// A `battement` arrived before any `enroler`.
    Sequence,
}

impl MotifCanal {
    /// The four variants, in the order a test walks them.
    ///
    /// 🔴 ANTI-OMISSION: a variant added without its line here would be absent
    /// from the mapping test, which compares this set to an EXHAUSTIVE
    /// `match` — the compiler requires the arm, and the test requires the entry.
    pub const TOUS: [Self; 4] = [Self::Version, Self::Forme, Self::Enrolement, Self::Sequence];

    /// The exact word that travels on the wire.
    pub fn mot(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Forme => "forme",
            Self::Enrolement => "enrolement",
            Self::Sequence => "sequence",
        }
    }

    /// The reason this word designates, or `None` if we do not know it.
    ///
    /// 🔴 `None` IS NOT AN ERROR: it is a reason from a version that is
    /// beyond us, and the caller must log it as is rather than
    /// lose it. It is clause 2 of the header of this module.
    pub fn depuis_mot(mot: &str) -> Option<Self> {
        Self::TOUS
            .into_iter()
            .find(|candidat| candidat.mot() == mot)
    }
}
