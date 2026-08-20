//! Choix du point de terminaison audio de rendu que le loopback de session
//! doit capter — **règle PURE, sans `#[cfg(windows)]`, éprouvée sur l'hôte**.
//!
//! ## Pourquoi ce module existe (correction « A-bis », 19 août 2026)
//!
//! Le chantier A captait le son de la VM par `GetDefaultAudioEndpoint(eRender,
//! eConsole)` : le **rendu par défaut** de Windows, quel qu'il soit. Cette
//! dépendance implicite s'est retournée le jour où l'installation de VB-Cable
//! (préparation du chantier E, microphone) a fait basculer ce défaut sur le
//! câble virtuel — un périphérique que rien n'alimente. Le produit captait
//! alors du silence, sans qu'aucune ligne de journal ne dise pourquoi.
//!
//! Le remède retenu par le propriétaire du dépôt n'est **pas** « remettre les
//! haut-parleurs par défaut » : cela corrigerait l'occurrence en laissant la
//! classe de panne entière, et n'importe quelle installation audio future la
//! rejouerait. Le remède est le **choix explicite**.
//!
//! ## Désigner par un NOM, jamais par un rang
//!
//! Ce dépôt a payé cette leçon sur les sorties DXGI : `(index_adaptateur,
//! index_sortie)` est positionnel et change dès qu'une sortie apparaît ou
//! disparaît (sous-bloc D1, corrigé en D2 par `DesktopCapture::sur_sortie`,
//! qui résout par nom). Un rang d'énumération audio a exactement le même
//! défaut, et pour la même raison : `IMMDeviceCollection` n'ordonne rien de
//! stable, et brancher un casque renumérote tout.
//!
//! **Deux désignations sont donc acceptées, et l'arbitrage entre elles est
//! écrit ici plutôt que laissé à l'appelant** :
//!
//! - **l'identifiant d'endpoint** (`IMMDevice::GetId`, de la forme
//!   `{0.0.0.00000000}.{guid}`) — *stable* : il survit au redémarrage, au
//!   changement de défaut et au renommage du périphérique dans le panneau de
//!   configuration ; mais *opaque* — personne ne le tape de mémoire dans un
//!   shell, et il ne se lit pas dans un journal ;
//! - **le nom convivial** (`PKEY_Device_FriendlyName`, p. ex. « Haut-parleurs
//!   (Steam Streaming Speakers) ») — *lisible* : c'est exactement ce que
//!   l'exploitant voit dans le panneau de son Windows et dans nos propres
//!   traces ; mais il *peut changer* avec le pilote, et deux périphériques
//!   peuvent porter des noms voisins.
//!
//! Aucune des deux ne domine l'autre, d'où le choix de les accepter toutes
//! deux : l'identifiant l'emporte quand il est fourni (c'est la désignation
//! stable, et personne ne l'écrit par accident), le nom sert par défaut parce
//! que c'est celui qu'un humain écrit. **Ni l'une ni l'autre n'est un rang.**
//!
//! ## La correspondance partielle est acceptée, l'ambiguïté ne l'est pas
//!
//! Un nom convivial Windows porte souvent un suffixe entre parenthèses que
//! l'exploitant n'a pas envie de recopier (« Haut-parleurs (Steam Streaming
//! Speakers) »). La règle accepte donc une **sous-chaîne**, insensible à la
//! casse — mais **seulement si elle ne désigne qu'un seul périphérique**. Si
//! plusieurs correspondent, on **refuse de trancher** (`Choix::Ambigu`) au
//! lieu de prendre le premier : prendre le premier serait retomber sur un
//! rang d'énumération par la porte de derrière, c'est-à-dire exactement ce
//! que ce module existe pour interdire.

/// Un point de terminaison audio tel que l'énumération Windows le rend.
///
/// **Volontairement dépourvu de tout type `windows`** : c'est ce qui rend la
/// règle ci-dessous éprouvable sur l'hôte Linux. L'appelant
/// (`agent/src/wasapi.rs`) traduit `IMMDevice` vers cette structure, et rien
/// d'autre ne franchit la frontière.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peripherique {
    /// `PKEY_Device_FriendlyName`.
    pub nom: String,
    /// `IMMDevice::GetId`.
    pub identifiant: String,
}

/// Par quoi un périphérique a été reconnu. Journalisé : sans lui, on ne sait
/// pas si l'élu l'a été sur une correspondance exacte ou sur une sous-chaîne,
/// donc on ne sait pas à quel point le choix est fragile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Critere {
    /// L'identifiant d'endpoint, comparé en entier.
    Identifiant,
    /// Le nom convivial, égal en entier (casse et espaces de bord ignorés).
    NomExact,
    /// Le nom convivial, contenant la demande — et un seul le contenait.
    NomPartiel,
}

impl Critere {
    /// Libellé court pour le journal.
    pub fn libelle(self) -> &'static str {
        match self {
            Critere::Identifiant => "identifiant",
            Critere::NomExact => "nom exact",
            Critere::NomPartiel => "nom partiel",
        }
    }
}

/// Ce que la règle rend. **Aucune variante n'est silencieuse** : chacune porte
/// de quoi écrire une ligne de journal qui dit ce qui a été demandé, ce qui a
/// été trouvé et ce qui est retenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choix<'a> {
    /// Aucune demande n'a été formulée : le rendu par défaut de Windows, comme
    /// avant la correction. **Ce n'est pas un repli** — c'est le cas nominal
    /// d'un agent lancé sans la variable.
    Defaut,
    /// Un périphérique a été élu.
    Elu {
        peripherique: &'a Peripherique,
        critere: Critere,
    },
    /// Rien ne correspond à la demande. **Repli sur le défaut, à journaliser
    /// en `warn!`** : c'est précisément la situation où le produit se met à
    /// capter autre chose que ce qu'on lui a demandé.
    Introuvable { demande: String },
    /// Plusieurs périphériques correspondent, et la règle refuse de trancher.
    /// Même repli, même bruit — et les candidats sont nommés pour que
    /// l'exploitant sache quoi écrire à la place.
    Ambigu {
        demande: String,
        candidats: Vec<String>,
    },
}

impl Choix<'_> {
    /// Vrai quand la demande n'a pas abouti et qu'on retombe sur le défaut de
    /// Windows. `Defaut` rend **faux** : il n'y avait rien à honorer.
    pub fn est_repli(&self) -> bool {
        matches!(self, Choix::Introuvable { .. } | Choix::Ambigu { .. })
    }
}

/// La désignation **INTÉGRÉE** du câble virtuel, employée quand
/// `MICRO_PERIPHERIQUE` est absente ou vide.
///
/// ⚠️ **Ce n'est PAS « CABLE Input ».** Le `PKEY_Device_FriendlyName` du point
/// de terminaison de **RENDU** du câble vaut « Haut-parleurs (VB-Audio Virtual
/// Cable) » — relevé sur la VM le 20 août 2026, et c'est **cette propriété
/// exacte** que lit `wasapi::rendu::decrire`, donc c'est elle et pas une autre
/// que la règle ci-dessous comparera. « CABLE Output » est le nom de l'autre
/// bout, celui de CAPTURE, que nous n'ouvrons jamais.
///
/// La sous-chaîne courte est retenue plutôt que « VB-Audio Virtual Cable »
/// parce qu'elle suffit et qu'elle est **unique parmi les trois rendus actifs**
/// de cette VM. Sur une machine portant deux câbles VB-Audio elle deviendrait
/// **ambiguë**, et la règle **refuse** : mieux vaut pas de micro qu'un micro
/// dans le mauvais tuyau.
pub const DESIGNATION_CABLE: &str = "VB-Audio";

/// La désignation à passer à [`choisir`] pour trouver le câble, à partir de la
/// valeur de `MICRO_PERIPHERIQUE`.
///
/// 🔴 **Elle n'est JAMAIS `None`, et c'est tout l'objet de cette fonction.**
/// A-bis se replie sur le défaut de Windows parce que « du son, peut-être le
/// mauvais, et un `warn!` qui le dit » vaut mieux que « aucun son ». Ici
/// l'arbitrage s'**inverse** : « la voix de l'utilisateur, peut-être dans le
/// mauvais périphérique » n'est pas un moindre mal, c'est une **fuite** — sur
/// une machine où le défaut est la carte son, la voix sortirait des
/// haut-parleurs. `Choix::Defaut` est donc rendu inatteignable par ce chemin,
/// et un test le garde.
pub fn demande_cable(variable: Option<&str>) -> &str {
    match variable {
        Some(v) if !v.trim().is_empty() => v,
        _ => DESIGNATION_CABLE,
    }
}

/// Normalise une désignation pour la comparaison : bords rognés, casse
/// abaissée. Le rognage compte — une variable d'environnement passée par un
/// script PowerShell arrive volontiers avec une espace de fin.
fn normaliser(texte: &str) -> String {
    texte.trim().to_lowercase()
}

/// Élit le périphérique de rendu à capter, ou dit pourquoi il ne le peut pas.
///
/// `demande` est ce que l'exploitant a écrit (variable d'environnement) :
/// `None`, ou une chaîne vide / d'espaces, valent « aucune demande ».
///
/// L'ordre des trois critères est décrit en tête de module ; il va du plus
/// spécifique au plus permissif, et **chacun ne rend un élu que s'il est
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

/// Un périphérique correspond-il à la cible NORMALISÉE, selon ce critère ?
fn correspond(peripherique: &Peripherique, cible: &str, critere: Critere) -> bool {
    match critere {
        Critere::Identifiant => normaliser(&peripherique.identifiant) == cible,
        Critere::NomExact => normaliser(&peripherique.nom) == cible,
        Critere::NomPartiel => normaliser(&peripherique.nom).contains(cible),
    }
}

/// Les noms disponibles, tels qu'on les écrit au journal quand une demande
/// n'aboutit pas. **Toujours joints à un repli** : dire « introuvable » sans
/// dire ce qui existait oblige l'exploitant à une seconde exécution.
pub fn inventaire(disponibles: &[Peripherique]) -> String {
    if disponibles.is_empty() {
        return "(aucun)".to_string();
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

    /// Le relevé RÉEL de la VM au 19 août 2026 — c'est lui qui motive cette
    /// correction, et l'éprouver sur autre chose serait éprouver une fiction.
    /// Le câble virtuel est en tête parce qu'il est devenu le défaut Windows.
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
    fn sans_demande_on_garde_le_defaut_de_windows() {
        assert_eq!(choisir(&vm(), None), Choix::Defaut);
        assert_eq!(choisir(&vm(), Some("")), Choix::Defaut);
        assert_eq!(choisir(&vm(), Some("   ")), Choix::Defaut);
    }

    /// `Defaut` n'est PAS un repli : rien n'a été demandé, donc rien n'a
    /// échoué. La distinction gouverne le niveau de journal côté appelant.
    #[test]
    fn le_defaut_n_est_pas_un_repli_mais_l_echec_en_est_un() {
        assert!(!choisir(&vm(), None).est_repli());
        assert!(!choisir(&vm(), Some("Steam")).est_repli());
        assert!(choisir(&vm(), Some("Casque Bluetooth")).est_repli());
        assert!(choisir(&vm(), Some("Haut-parleurs")).est_repli());
    }

    /// LE CAS DE LA CORRECTION : le défaut Windows est le câble, on demande
    /// les haut-parleurs par une sous-chaîne, et ce sont eux qui sont élus.
    #[test]
    fn une_sous_chaine_unique_elit_les_haut_parleurs_malgre_le_defaut_windows() {
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
            autre => panic!("attendu un élu, obtenu {autre:?}"),
        }
    }

    #[test]
    fn la_sous_chaine_ignore_la_casse_et_les_espaces_de_bord() {
        let disponibles = vm();
        match choisir(&disponibles, Some("  sTeAm sTrEaMiNg  ")) {
            Choix::Elu { peripherique, .. } => {
                assert_eq!(peripherique.identifiant, "{0.0.0.00000000}.{steam}")
            }
            autre => panic!("attendu un élu, obtenu {autre:?}"),
        }
    }

    #[test]
    fn le_nom_complet_est_reconnu_comme_exact_et_non_comme_partiel() {
        let disponibles = vm();
        match choisir(&disponibles, Some("Haut-parleurs (Steam Streaming Speakers)")) {
            Choix::Elu { critere, .. } => assert_eq!(critere, Critere::NomExact),
            autre => panic!("attendu un élu, obtenu {autre:?}"),
        }
    }

    /// L'identifiant l'emporte sur le nom, et il tranche un cas que le nom ne
    /// peut pas trancher : deux périphériques homonymes.
    #[test]
    fn l_identifiant_tranche_la_ou_le_nom_est_ambigu() {
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
            autre => panic!("attendu un élu par identifiant, obtenu {autre:?}"),
        }
    }

    /// La priorité de l'identifiant n'est pas théorique : ici une même chaîne
    /// est l'identifiant de l'un ET une sous-chaîne du nom de l'autre. Sans
    /// l'ordre des critères, le nom l'emporterait.
    #[test]
    fn l_identifiant_est_essaye_avant_le_nom() {
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
            autre => panic!("attendu l'élu par identifiant, obtenu {autre:?}"),
        }
    }

    /// Le nom EXACT est essayé avant la sous-chaîne : sans cet ordre, un nom
    /// qui est le préfixe d'un autre serait déclaré ambigu alors qu'il
    /// désigne exactement un périphérique.
    #[test]
    fn le_nom_exact_est_essaye_avant_la_sous_chaine() {
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
            autre => panic!("attendu l'élu par nom exact, obtenu {autre:?}"),
        }
    }

    /// **Le refus de trancher est le cœur de la règle** : deux « Haut-parleurs »
    /// dans le relevé de la VM, donc prendre le premier reviendrait à choisir
    /// par rang d'énumération — ce que ce module existe pour interdire.
    #[test]
    fn une_sous_chaine_ambigue_refuse_de_trancher_et_nomme_les_candidats() {
        let disponibles = vm();
        match choisir(&disponibles, Some("Haut-parleurs")) {
            Choix::Ambigu {
                demande,
                candidats,
            } => {
                assert_eq!(demande, "Haut-parleurs");
                assert_eq!(
                    candidats,
                    vec![
                        "Haut-parleurs (VB-Audio Virtual Cable)".to_string(),
                        "Haut-parleurs (Steam Streaming Speakers)".to_string(),
                    ]
                );
            }
            autre => panic!("attendu une ambiguïté, obtenu {autre:?}"),
        }
    }

    #[test]
    fn un_nom_absent_est_introuvable_et_reporte_la_demande_rognee() {
        match choisir(&vm(), Some("  Casque Bluetooth  ")) {
            Choix::Introuvable { demande } => assert_eq!(demande, "Casque Bluetooth"),
            autre => panic!("attendu introuvable, obtenu {autre:?}"),
        }
    }

    #[test]
    fn une_demande_sur_une_liste_vide_est_introuvable() {
        assert_eq!(
            choisir(&[], Some("Steam")),
            Choix::Introuvable {
                demande: "Steam".into()
            }
        );
    }

    #[test]
    fn l_inventaire_nomme_chaque_peripherique_avec_son_identifiant() {
        let texte = inventaire(&vm());
        assert!(texte.contains("«Haut-parleurs (Steam Streaming Speakers)»"));
        assert!(texte.contains("{0.0.0.00000000}.{cable-input}"));
        assert!(texte.contains("{0.0.0.00000000}.{nvidia}"));
        assert_eq!(inventaire(&[]), "(aucun)");
    }

    #[test]
    fn le_libelle_du_critere_distingue_les_trois_cas() {
        assert_eq!(Critere::Identifiant.libelle(), "identifiant");
        assert_eq!(Critere::NomExact.libelle(), "nom exact");
        assert_eq!(Critere::NomPartiel.libelle(), "nom partiel");
    }
}

#[cfg(test)]
#[path = "peripherique/tests_cable.rs"]
mod tests_cable;
