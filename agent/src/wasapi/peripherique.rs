//! Choice of the audio render endpoint the session loopback
//! must capture — **PURE rule, without `#[cfg(windows)]`, tested on the host**.
//!
//! ## Why this module exists (fix "A-bis", August 19th, 2026)
//!
//! Workstream A captured the VM's sound through `GetDefaultAudioEndpoint(eRender,
//! eConsole)`: Windows' **default render device**, whatever it is. This
//! implicit dependency turned around the day installing VB-Cable
//! (preparation of workstream E, microphone) switched that default to the
//! virtual cable — a device nothing feeds. The product then captured
//! silence, without any log line saying why.
//!
//! The remedy chosen by the repository owner is **not** "put the
//! speakers back as default": that would fix the occurrence while leaving the
//! whole failure class, and any future audio installation would
//! replay it. The remedy is **explicit choice**.
//!
//! ## Designate by a NAME, never by a rank
//!
//! This repository paid for this lesson on DXGI outputs: `(index_adaptateur,
//! index_sortie)` is positional and changes as soon as an output appears or
//! disappears (sub-block D1, fixed in D2 by `DesktopCapture::sur_sortie`,
//! which resolves by name). An audio enumeration rank has exactly the same
//! defect, and for the same reason: `IMMDeviceCollection` orders nothing
//! stable, and plugging in a headset renumbers everything.
//!
//! **Two designations are therefore accepted, and the arbitration between them is
//! written here rather than left to the caller**:
//!
//! - **the endpoint identifier** (`IMMDevice::GetId`, of the form
//!   `{0.0.0.00000000}.{guid}`) — *stable*: it survives reboot,
//!   default change and renaming the device in the control
//!   panel; but *opaque* — nobody types it from memory in a
//!   shell, and it cannot be read in a log;
//! - **the friendly name** (`PKEY_Device_FriendlyName`, e.g. "Haut-parleurs
//!   (Steam Streaming Speakers)") — *readable*: it is exactly what
//!   the operator sees in their Windows panel and in our own
//!   traces; but it *can change* with the driver, and two devices
//!   can carry similar names.
//!
//! Neither dominates the other, hence the choice to accept both:
//! the identifier wins when provided (it is the stable
//! designation, and nobody writes it by accident), the name serves by default because
//! it is the one a human writes. **Neither is a rank.**
//!
//! ## Partial matching is accepted, ambiguity is not
//!
//! A Windows friendly name often carries a parenthesised suffix that
//! the operator does not want to copy ("Haut-parleurs (Steam Streaming
//! Speakers)"). The rule therefore accepts a case-insensitive
//! **substring** — but **only if it designates a single device**. If
//! several match, we **refuse to decide** (`Choix::Ambigu`) instead
//! of taking the first: taking the first would fall back on an
//! enumeration rank through the back door, that is, exactly what
//! this module exists to forbid.

/// An audio endpoint as Windows' enumeration returns it.
///
/// **Deliberately free of any `windows` type**: that is what makes the
/// rule below testable on the Linux host. The caller
/// (`agent/src/wasapi.rs`) translates `IMMDevice` into this structure, and nothing
/// else crosses the boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peripherique {
    /// `PKEY_Device_FriendlyName`.
    pub nom: String,
    /// `IMMDevice::GetId`.
    pub identifiant: String,
}

/// What a device was recognised by. Logged: without it, we do not know
/// whether the elected one was elected on an exact match or on a substring,
/// so we do not know how fragile the choice is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Critere {
    /// The endpoint identifier, compared in full.
    Identifiant,
    /// The friendly name, equal in full (case and edge whitespace ignored).
    NomExact,
    /// The friendly name, containing the request — and only one contained it.
    NomPartiel,
}

impl Critere {
    /// Short label for the log.
    pub fn libelle(self) -> &'static str {
        match self {
            Critere::Identifiant => "identifiant",
            Critere::NomExact => "nom exact",
            Critere::NomPartiel => "nom partiel",
        }
    }
}

/// What the rule returns. **No variant is silent**: each carries
/// what is needed to write a log line saying what was requested, what was
/// found and what is retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choix<'a> {
    /// No request was made: Windows' default render device, as
    /// before the fix. **It is not a fallback** — it is the nominal case
    /// of an agent launched without the variable.
    Defaut,
    /// A device was elected.
    Elu {
        peripherique: &'a Peripherique,
        critere: Critere,
    },
    /// Nothing matches the request. **Fallback to the default, to be logged
    /// as `warn!`**: it is precisely the situation where the product starts
    /// capturing something other than what it was asked for.
    Introuvable { demande: String },
    /// Several devices match, and the rule refuses to decide.
    /// Same fallback, same noise — and the candidates are named so that
    /// the operator knows what to write instead.
    Ambigu {
        demande: String,
        candidats: Vec<String>,
    },
}

impl Choix<'_> {
    /// True when the request did not succeed and we fall back to Windows'
    /// default. `Defaut` returns **false**: there was nothing to honour.
    pub fn est_repli(&self) -> bool {
        matches!(self, Choix::Introuvable { .. } | Choix::Ambigu { .. })
    }
}

/// The **BUILT-IN** designation of the virtual cable, used when
/// `MICRO_PERIPHERIQUE` is absent or empty.
///
/// ⚠️ **It is NOT "CABLE Input".** The `PKEY_Device_FriendlyName` of the cable's
/// **RENDER** endpoint is "Haut-parleurs (VB-Audio Virtual
/// Cable)" — noted on the VM on August 20th, 2026, and it is **this exact
/// property** that `wasapi::rendu::decrire` reads, so it is this one and no other
/// that the rule below will compare. "CABLE Output" is the name of the other
/// end, the CAPTURE one, which we never open.
///
/// The short substring is chosen rather than "VB-Audio Virtual Cable"
/// because it is enough and it is **unique among the three active render devices**
/// of this VM. On a machine carrying two VB-Audio cables it would become
/// **ambiguous**, and the rule **refuses**: better no microphone than a microphone
/// in the wrong pipe.
pub const DESIGNATION_CABLE: &str = "VB-Audio";

/// The designation to pass to [`choisir`] to find the cable, from the
/// value of `MICRO_PERIPHERIQUE`.
///
/// 🔴 **It is NEVER `None`, and that is the whole purpose of this function.**
/// A-bis falls back to Windows' default because "sound, perhaps the
/// wrong one, and a `warn!` that says so" is better than "no sound". Here
/// the arbitration is **inverted**: "the user's voice, perhaps in the
/// wrong device" is not a lesser evil, it is a **leak** — on
/// a machine where the default is the sound card, the voice would come out of the
/// speakers. `Choix::Defaut` is therefore made unreachable through this path,
/// and a test guards it.
pub fn demande_cable(variable: Option<&str>) -> &str {
    match variable {
        Some(v) if !v.trim().is_empty() => v,
        _ => DESIGNATION_CABLE,
    }
}

/// Normalises a designation for comparison: edges trimmed, case
/// lowered. Trimming matters — an environment variable passed by a
/// PowerShell script readily arrives with a trailing space.
fn normaliser(texte: &str) -> String {
    texte.trim().to_lowercase()
}

/// Elects the render device to capture, or says why it cannot.
///
/// `demande` is what the operator wrote (environment variable):
/// `None`, or an empty / whitespace string, mean "no request".
///
/// The order of the three criteria is described at the head of the module; it goes from the most
/// specific to the most permissive, and **each only returns an elected one if it is
/// unique**.
pub fn choisir<'a>(disponibles: &'a [Peripherique], demande: Option<&str>) -> Choix<'a> {
    let demande = match demande {
        Some(texte) if !texte.trim().is_empty() => texte,
        _ => return Choix::Defaut,
    };
    let cible = normaliser(demande);

    for critere in [Critere::Identifiant, Critere::NomExact, Critere::NomPartiel] {
        let retenus: Vec<&Peripherique> = disponibles
            .iter()
            .filter(|p| correspond(p, &cible, critere))
            .collect();
        match retenus.len() {
            0 => continue,
            1 => {
                return Choix::Elu {
                    peripherique: retenus[0],
                    critere,
                }
            }
            _ => {
                return Choix::Ambigu {
                    demande: demande.trim().to_string(),
                    candidats: retenus.iter().map(|p| p.nom.clone()).collect(),
                }
            }
        }
    }

    Choix::Introuvable {
        demande: demande.trim().to_string(),
    }
}

/// Does a device match the NORMALISED target, by this criterion?
fn correspond(peripherique: &Peripherique, cible: &str, critere: Critere) -> bool {
    match critere {
        Critere::Identifiant => normaliser(&peripherique.identifiant) == cible,
        Critere::NomExact => normaliser(&peripherique.nom) == cible,
        Critere::NomPartiel => normaliser(&peripherique.nom).contains(cible),
    }
}

/// The available names, as written to the log when a request
/// does not succeed. **Always attached to a fallback**: saying "not found" without
/// saying what existed forces the operator into a second run.
pub fn inventaire(disponibles: &[Peripherique]) -> String {
    if disponibles.is_empty() {
        return "(none)".to_string();
    }
    disponibles
        .iter()
        .map(|p| format!("«{}» [{}]", p.nom, p.identifiant))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The REAL listing of the VM as of August 19th, 2026 — it is what motivates this
    /// fix, and testing it on anything else would be testing a fiction.
    /// The virtual cable comes first because it became the Windows default.
    fn vm() -> Vec<Peripherique> {
        vec![
            Peripherique {
                nom: "Haut-parleurs (VB-Audio Virtual Cable)".into(),
                identifiant: "{0.0.0.00000000}.{cable-input}".into(),
            },
            Peripherique {
                nom: "Haut-parleurs (Steam Streaming Speakers)".into(),
                identifiant: "{0.0.0.00000000}.{steam}".into(),
            },
            Peripherique {
                nom: "HDP-V104 (NVIDIA High Definition Audio)".into(),
                identifiant: "{0.0.0.00000000}.{nvidia}".into(),
            },
        ]
    }

    #[test]
    fn without_a_request_the_windows_default_is_kept() {
        assert_eq!(choisir(&vm(), None), Choix::Defaut);
        assert_eq!(choisir(&vm(), Some("")), Choix::Defaut);
        assert_eq!(choisir(&vm(), Some("   ")), Choix::Defaut);
    }

    /// `Defaut` is NOT a fallback: nothing was requested, so nothing
    /// failed. The distinction governs the log level on the caller side.
    #[test]
    fn the_default_is_not_a_fallback_but_the_failure_is_one() {
        assert!(!choisir(&vm(), None).est_repli());
        assert!(!choisir(&vm(), Some("Steam")).est_repli());
        assert!(choisir(&vm(), Some("Casque Bluetooth")).est_repli());
        assert!(choisir(&vm(), Some("Haut-parleurs")).est_repli());
    }

    /// THE FIX'S CASE: the Windows default is the cable, we request
    /// the speakers through a substring, and they are the ones elected.
    #[test]
    fn a_unique_substring_elects_the_speakers_despite_the_windows_default() {
        let disponibles = vm();
        match choisir(&disponibles, Some("Steam Streaming")) {
            Choix::Elu {
                peripherique,
                critere,
            } => {
                assert_eq!(peripherique.nom, "Haut-parleurs (Steam Streaming Speakers)");
                assert_eq!(peripherique.identifiant, "{0.0.0.00000000}.{steam}");
                assert_eq!(critere, Critere::NomPartiel);
            }
            autre => panic!("expected an elected one, got {autre:?}"),
        }
    }

    #[test]
    fn the_substring_ignores_case_and_edge_spaces() {
        let disponibles = vm();
        match choisir(&disponibles, Some("  sTeAm sTrEaMiNg  ")) {
            Choix::Elu { peripherique, .. } => {
                assert_eq!(peripherique.identifiant, "{0.0.0.00000000}.{steam}")
            }
            autre => panic!("expected an elected one, got {autre:?}"),
        }
    }

    #[test]
    fn the_full_name_is_recognised_as_exact_not_partial() {
        let disponibles = vm();
        match choisir(
            &disponibles,
            Some("Haut-parleurs (Steam Streaming Speakers)"),
        ) {
            Choix::Elu { critere, .. } => assert_eq!(critere, Critere::NomExact),
            autre => panic!("expected an elected one, got {autre:?}"),
        }
    }

    /// The identifier wins over the name, and it settles a case the name
    /// cannot settle: two devices with the same name.
    #[test]
    fn the_identifier_decides_where_the_name_is_ambiguous() {
        let jumeaux = vec![
            Peripherique {
                nom: "Haut-parleurs".into(),
                identifiant: "{0.0.0.00000000}.{a}".into(),
            },
            Peripherique {
                nom: "Haut-parleurs".into(),
                identifiant: "{0.0.0.00000000}.{b}".into(),
            },
        ];
        assert!(matches!(
            choisir(&jumeaux, Some("Haut-parleurs")),
            Choix::Ambigu { .. }
        ));
        match choisir(&jumeaux, Some("{0.0.0.00000000}.{b}")) {
            Choix::Elu {
                peripherique,
                critere,
            } => {
                assert_eq!(peripherique.identifiant, "{0.0.0.00000000}.{b}");
                assert_eq!(critere, Critere::Identifiant);
            }
            autre => panic!("expected one elected by identifier, got {autre:?}"),
        }
    }

    /// The identifier's priority is not theoretical: here a same string
    /// is the identifier of one AND a substring of the other's name. Without
    /// the order of criteria, the name would win.
    #[test]
    fn the_identifier_is_tried_before_the_name() {
        let piege = vec![
            Peripherique {
                nom: "Sortie ligne".into(),
                identifiant: "cable".into(),
            },
            Peripherique {
                nom: "Haut-parleurs (cable virtuel)".into(),
                identifiant: "{0.0.0.00000000}.{autre}".into(),
            },
        ];
        match choisir(&piege, Some("cable")) {
            Choix::Elu {
                peripherique,
                critere,
            } => {
                assert_eq!(peripherique.nom, "Sortie ligne");
                assert_eq!(critere, Critere::Identifiant);
            }
            autre => panic!("expected the one elected by identifier, got {autre:?}"),
        }
    }

    /// The EXACT name is tried before the substring: without this order, a name
    /// that is the prefix of another would be declared ambiguous whereas it
    /// designates exactly one device.
    #[test]
    fn the_exact_name_is_tried_before_the_substring() {
        let prefixe = vec![
            Peripherique {
                nom: "Haut-parleurs".into(),
                identifiant: "{a}".into(),
            },
            Peripherique {
                nom: "Haut-parleurs (Steam)".into(),
                identifiant: "{b}".into(),
            },
        ];
        match choisir(&prefixe, Some("Haut-parleurs")) {
            Choix::Elu {
                peripherique,
                critere,
            } => {
                assert_eq!(peripherique.identifiant, "{a}");
                assert_eq!(critere, Critere::NomExact);
            }
            autre => panic!("expected the one elected by exact name, got {autre:?}"),
        }
    }

    /// **Refusing to decide is the heart of the rule**: two "Haut-parleurs"
    /// in the VM's listing, so taking the first would amount to choosing
    /// by enumeration rank — what this module exists to forbid.
    #[test]
    fn an_ambiguous_substring_refuses_to_decide_and_names_the_candidates() {
        let disponibles = vm();
        match choisir(&disponibles, Some("Haut-parleurs")) {
            Choix::Ambigu { demande, candidats } => {
                assert_eq!(demande, "Haut-parleurs");
                assert_eq!(
                    candidats,
                    vec![
                        "Haut-parleurs (VB-Audio Virtual Cable)".to_string(),
                        "Haut-parleurs (Steam Streaming Speakers)".to_string(),
                    ]
                );
            }
            autre => panic!("expected an ambiguity, got {autre:?}"),
        }
    }

    #[test]
    fn a_missing_name_is_not_found_and_reports_the_trimmed_request() {
        match choisir(&vm(), Some("  Casque Bluetooth  ")) {
            Choix::Introuvable { demande } => assert_eq!(demande, "Casque Bluetooth"),
            autre => panic!("expected not found, got {autre:?}"),
        }
    }

    #[test]
    fn a_request_on_an_empty_list_is_not_found() {
        assert_eq!(
            choisir(&[], Some("Steam")),
            Choix::Introuvable {
                demande: "Steam".into()
            }
        );
    }

    #[test]
    fn the_inventory_names_each_device_with_its_identifier() {
        let texte = inventaire(&vm());
        assert!(texte.contains("«Haut-parleurs (Steam Streaming Speakers)»"));
        assert!(texte.contains("{0.0.0.00000000}.{cable-input}"));
        assert!(texte.contains("{0.0.0.00000000}.{nvidia}"));
        assert_eq!(inventaire(&[]), "(none)");
    }

    #[test]
    fn the_criterion_label_tells_the_three_cases_apart() {
        assert_eq!(Critere::Identifiant.libelle(), "identifiant");
        assert_eq!(Critere::NomExact.libelle(), "nom exact");
        assert_eq!(Critere::NomPartiel.libelle(), "nom partiel");
    }
}

#[cfg(test)]
#[path = "peripherique/tests_cable.rs"]
mod tests_cable;
