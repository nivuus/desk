//! Messages du canal plateforme <-> agent (`/agent`).
//!
//! Format JSON versionné : `{"type":"...","v":1,...}` (`type` sert de tag
//! interne à l'enum et est toujours émis en premier par serde). Le champ `v`
//! est obligatoire et vérifié à la désérialisation : un message sans `v`, ou
//! avec un `v` différent de [`PLATEFORME_VERSION`], est rejeté.
//!
//! ⚠️ CE MODULE EST DISTINCT DE [`crate::control`], ET CE N'EST PAS UN HASARD.
//! `control` versionne le canal de données **agent <-> navigateur** ; celui-ci
//! versionne le canal **agent <-> plateforme**. Les deux évoluent pour des
//! raisons sans rapport, et une constante partagée forcerait chacun à bouger
//! quand l'autre change — ce qui rendrait tout bump illisible.
//!
//! 🔴 CE QUI SE PASSE QUAND LES VERSIONS DIVERGENT N'EST PAS SYMÉTRIQUE, parce
//! que les deux bouts n'ont pas le même pouvoir :
//!   - l'agent envoie une version que la plateforme ne connaît pas : elle
//!     répond `{v, type:"refus", motif:"version"}`, journalise AVEC la version
//!     reçue, et ferme le socket. Aucune négociation à la baisse : il n'y a
//!     qu'une version ;
//!   - la plateforme envoie une version que l'agent ne connaît pas :
//!     `serde_json::from_str` échoue, l'agent journalise avec le texte de
//!     l'erreur et ferme le canal.
//!
//! 🔴 UNE DIVERGENCE DE VERSION NE DOIT JAMAIS SE LIRE COMME UNE PANNE RÉSEAU.
//! Un refus `version` NE SE RÉESSAIE PAS ; une chute de socket, si. Deux
//! comportements, deux traces distinctes — sans quoi une incompatibilité de
//! version se déguiserait en boucle de reconnexion infinie, qui est le mode de
//! panne le plus coûteux à diagnostiquer.

use serde::{Deserialize, Serialize};

/// Version du protocole du canal plateforme <-> agent. Incrémenter à tout
/// changement de format.
///
/// v1 (sous-bloc P3) : enrôlement, battement de cœur, jeton d'agent.
pub const PLATEFORME_VERSION: u8 = 1;

// Note : pas de `default` sur le champ `v` — un message sans champ `v` doit être
// rejeté (champ obligatoire), pas silencieusement complété avec la version
// courante. `default` court-circuiterait `deserialize_with` quand le champ est
// absent, ce qui romprait la vérification.
fn verifie_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
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

/// Pourquoi la plateforme refuse.
///
/// ⚠️ `Enrolement` NE DISTINGUE PAS « VM inconnue » de « secret faux », et
/// c'est délibéré : les distinguer donnerait à quiconque ouvre le canal un
/// oracle d'énumération des VMs enrôlées. Le diagnostic vit dans le journal de
/// la plateforme, jamais sur le fil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

/// Message de l'agent vers la plateforme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum VersLaPlateforme {
    /// S'enrôler : présenter le nom de VM et le secret d'enrôlement.
    Enroler {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        vm: String,
        secret: String,
    },
    /// Battre le cœur. Fait avancer `vu_a`, et rend un jeton frais.
    Battement {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
    },
}

impl VersLaPlateforme {
    pub fn enroler(vm: impl Into<String>, secret: impl Into<String>) -> Self {
        Self::Enroler {
            version: PLATEFORME_VERSION,
            vm: vm.into(),
            secret: secret.into(),
        }
    }

    pub fn battement() -> Self {
        Self::Battement {
            version: PLATEFORME_VERSION,
        }
    }
}

/// Message de la plateforme vers l'agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DepuisLaPlateforme {
    /// L'enrôlement est accepté : voici le préfixe de session et le jeton.
    Enrole {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        prefixe: String,
        jeton: String,
        /// En MILLISECONDES, comme tout horodatage de ce service.
        expire_a: i64,
    },
    /// Le battement est enregistré : voici un jeton FRAIS.
    ///
    /// ⚠️ Un jeton frais À CHAQUE battement, et non le même : le jeton d'accès
    /// dure dix minutes, et un agent qui garderait le premier tomberait à son
    /// expiration sans le voir venir.
    BattementRecu {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        jeton: String,
        expire_a: i64,
    },
    /// Refus, avec son motif. Le socket se ferme ensuite.
    Refus {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        motif: MotifCanal,
    },
}

impl DepuisLaPlateforme {
    pub fn enrole(prefixe: impl Into<String>, jeton: impl Into<String>, expire_a: i64) -> Self {
        Self::Enrole {
            version: PLATEFORME_VERSION,
            prefixe: prefixe.into(),
            jeton: jeton.into(),
            expire_a,
        }
    }

    pub fn battement_recu(jeton: impl Into<String>, expire_a: i64) -> Self {
        Self::BattementRecu {
            version: PLATEFORME_VERSION,
            jeton: jeton.into(),
            expire_a,
        }
    }

    pub fn refus(motif: MotifCanal) -> Self {
        Self::Refus {
            version: PLATEFORME_VERSION,
            motif,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialise_l_enrolement_en_kebab_case() {
        let json = serde_json::to_string(&VersLaPlateforme::enroler("w1", "chut")).expect("sér.");
        assert_eq!(json, r#"{"type":"enroler","v":1,"vm":"w1","secret":"chut"}"#);
    }

    #[test]
    fn serialise_le_battement() {
        let json = serde_json::to_string(&VersLaPlateforme::battement()).expect("sér.");
        assert_eq!(json, r#"{"type":"battement","v":1}"#);
    }

    #[test]
    fn serialise_le_battement_recu_en_kebab_case() {
        // 🔴 `battement-recu` EST LA SEULE VARIANTE A DEUX MOTS DU MODULE, donc
        // la seule dont `kebab-case` et `snake_case` diffèrent. Sans ce test,
        // passer `rename_all` en `snake_case` ne rougissait RIEN — MESURE : la
        // mutation restait verte sur les 50 tests. Rust émettrait alors
        // `battement_recu` là où le miroir TypeScript lit `battement-recu`, et
        // les deux bouts divergeraient EN SILENCE sur le message que l'agent
        // reçoit le plus souvent.
        let json = serde_json::to_string(&DepuisLaPlateforme::battement_recu("kkk", 1_787_136_774_000))
            .expect("sér.");
        assert_eq!(
            json,
            r#"{"type":"battement-recu","v":1,"jeton":"kkk","expire_a":1787136774000}"#
        );
    }

    #[test]
    fn serialise_l_enrole_et_le_refus() {
        let json = serde_json::to_string(&DepuisLaPlateforme::enrole("PPP", "jjj", 1_787_136_773_742))
            .expect("sér.");
        assert_eq!(
            json,
            r#"{"type":"enrole","v":1,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}"#
        );
        let json = serde_json::to_string(&DepuisLaPlateforme::refus(MotifCanal::Enrolement))
            .expect("sér.");
        assert_eq!(json, r#"{"type":"refus","v":1,"motif":"enrolement"}"#);
    }

    // 🔴 UN TEST DE VERSION PAR VARIANTE ENTRANTE, jamais un seul pour toutes.
    // `verifie_version` est branchée variante par variante : l'omettre sur UNE
    // seule laisserait ce trou-là ouvert, et un test unique ne le verrait pas.

    #[test]
    fn rejette_une_version_absente_sur_enroler() {
        assert!(
            serde_json::from_str::<VersLaPlateforme>(r#"{"type":"enroler","vm":"w","secret":"s"}"#)
                .is_err()
        );
    }

    #[test]
    fn rejette_une_version_absente_sur_battement() {
        assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"battement"}"#).is_err());
    }

    #[test]
    fn rejette_une_version_absente_sur_enrole() {
        assert!(serde_json::from_str::<DepuisLaPlateforme>(
            r#"{"type":"enrole","prefixe":"P","jeton":"j","expire_a":1}"#
        )
        .is_err());
    }

    #[test]
    fn rejette_une_version_absente_sur_battement_recu() {
        assert!(serde_json::from_str::<DepuisLaPlateforme>(
            r#"{"type":"battement-recu","jeton":"j","expire_a":1}"#
        )
        .is_err());
    }

    #[test]
    fn rejette_une_version_absente_sur_refus() {
        assert!(
            serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"refus","motif":"version"}"#)
                .is_err()
        );
    }

    #[test]
    fn rejette_la_version_suivante_sur_enroler() {
        assert!(serde_json::from_str::<VersLaPlateforme>(
            r#"{"type":"enroler","v":2,"vm":"w","secret":"s"}"#
        )
        .is_err());
    }

    #[test]
    fn rejette_la_version_suivante_sur_battement() {
        assert!(
            serde_json::from_str::<VersLaPlateforme>(r#"{"type":"battement","v":2}"#).is_err()
        );
    }

    #[test]
    fn rejette_la_version_suivante_sur_enrole() {
        assert!(serde_json::from_str::<DepuisLaPlateforme>(
            r#"{"type":"enrole","v":2,"prefixe":"P","jeton":"j","expire_a":1}"#
        )
        .is_err());
    }

    #[test]
    fn rejette_la_version_suivante_sur_battement_recu() {
        assert!(serde_json::from_str::<DepuisLaPlateforme>(
            r#"{"type":"battement-recu","v":2,"jeton":"j","expire_a":1}"#
        )
        .is_err());
    }

    #[test]
    fn rejette_la_version_suivante_sur_refus() {
        assert!(serde_json::from_str::<DepuisLaPlateforme>(
            r#"{"type":"refus","v":2,"motif":"version"}"#
        )
        .is_err());
    }

    #[test]
    fn rejette_un_type_inconnu() {
        assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"vol","v":1}"#).is_err());
        assert!(serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"vol","v":1}"#).is_err());
    }

    #[test]
    fn rejette_un_champ_inconnu() {
        // `deny_unknown_fields` : un champ de trop est une divergence de
        // format, pas une extension tolérable — le canal n'a qu'une version.
        assert!(serde_json::from_str::<VersLaPlateforme>(
            r#"{"type":"battement","v":1,"bonus":1}"#
        )
        .is_err());
    }

    /// Conformité aux vecteurs partagés.
    ///
    /// 🔴 IL VÉRIFIE `doc["version"]`, ET C'EST LA LACUNE D'`input.rs` CORRIGÉE
    /// POUR CE FICHIER-CI : `input.rs::conformite_aux_vecteurs_partages` lit
    /// `vectors.json` sans jamais contrôler sa clé `version`, et le SEUL
    /// endroit du dépôt qui la contrôle est `ts/input.test.ts`. Un vecteur dont
    /// la version aurait dérivé passerait donc le Rust en silence — MESURÉ :
    /// en retirant l'assertion ci-dessous et en portant le fichier à
    /// `"version": 2`, les 52 tests restaient VERTS. Ici, les DEUX côtés la
    /// vérifient.
    #[test]
    fn conformite_aux_vecteurs_partages() {
        let raw = include_str!("../plateforme-vectors.json");
        let doc: serde_json::Value = serde_json::from_str(raw).expect("vecteurs valides");

        // 🔴 La version du fichier EST celle du protocole. Sans cette
        // assertion, un bump d'un seul côté ne se verrait nulle part.
        assert_eq!(
            doc["version"].as_u64().expect("clé version"),
            u64::from(PLATEFORME_VERSION),
            "la version des vecteurs a dérivé de PLATEFORME_VERSION"
        );

        let cases = doc["cases"].as_array().expect("tableau de cas");
        // 🔴 ANTI-TAUTOLOGIE : un fichier de vecteurs VIDE ferait passer toute
        // la boucle sans rien éprouver. Même garde qu'`input.rs:326` et que
        // `sous-ensemble.test.ts`.
        assert!(!cases.is_empty(), "au moins un vecteur attendu");

        let mut vus = 0;
        for case in cases {
            let name = case["name"].as_str().expect("nom");
            let attendu = case["json"].as_str().expect("json attendu");

            match case["sens"].as_str().expect("sens") {
                "vers" => {
                    let msg = match case["kind"].as_str().expect("kind") {
                        "enroler" => VersLaPlateforme::enroler(
                            case["vm"].as_str().unwrap(),
                            case["secret"].as_str().unwrap(),
                        ),
                        "battement" => VersLaPlateforme::battement(),
                        autre => panic!("kind inconnu dans le sens vers : {autre}"),
                    };
                    assert_eq!(
                        serde_json::to_string(&msg).expect("sér."),
                        attendu,
                        "sérialisation du vecteur « {name} »"
                    );
                    let relu: VersLaPlateforme =
                        serde_json::from_str(attendu).expect("désér.");
                    assert_eq!(relu, msg, "désérialisation du vecteur « {name} »");
                }
                "depuis" => {
                    let msg = match case["kind"].as_str().expect("kind") {
                        "enrole" => DepuisLaPlateforme::enrole(
                            case["prefixe"].as_str().unwrap(),
                            case["jeton"].as_str().unwrap(),
                            case["expire_a"].as_i64().unwrap(),
                        ),
                        "battement-recu" => DepuisLaPlateforme::battement_recu(
                            case["jeton"].as_str().unwrap(),
                            case["expire_a"].as_i64().unwrap(),
                        ),
                        "refus" => DepuisLaPlateforme::refus(
                            serde_json::from_value(case["motif"].clone()).expect("motif"),
                        ),
                        autre => panic!("kind inconnu dans le sens depuis : {autre}"),
                    };
                    assert_eq!(
                        serde_json::to_string(&msg).expect("sér."),
                        attendu,
                        "sérialisation du vecteur « {name} »"
                    );
                    let relu: DepuisLaPlateforme =
                        serde_json::from_str(attendu).expect("désér.");
                    assert_eq!(relu, msg, "désérialisation du vecteur « {name} »");
                }
                autre => panic!("sens inconnu : {autre}"),
            }
            vus += 1;
        }
        // 🔴 Le compte est ÉCRIT EN DUR : sans lui, un `sens` mal orthographié
        // ferait sauter des cas en silence — le `panic!` ne les verrait pas,
        // puisqu'il n'est atteint que par une valeur PRÉSENTE et inconnue, pas
        // par un cas qu'une future refonte de la boucle sauterait.
        assert_eq!(vus, cases.len(), "tous les cas doivent être exercés");
    }

    #[test]
    fn round_trip_des_trois_reponses() {
        for message in [
            DepuisLaPlateforme::enrole("PPP", "jjj", 1_787_136_773_742),
            DepuisLaPlateforme::battement_recu("kkk", 1_787_136_774_000),
            DepuisLaPlateforme::refus(MotifCanal::Sequence),
        ] {
            let json = serde_json::to_string(&message).expect("sér.");
            let relu: DepuisLaPlateforme = serde_json::from_str(&json).expect("désér.");
            assert_eq!(message, relu);
        }
    }
}
