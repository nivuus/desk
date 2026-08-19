//! Messages de la session de contrôle, entre le superviseur et la page-shell.
//!
//! Le signaling ne fait que relayer : c'est ici que la forme des messages est
//! décidée, et elle doit correspondre exactement à ce que `client/src/shell.ts`
//! attend — et à ce que `plateforme/src/signaling/relais.ts` accepte de
//! relayer (`TYPES_RELAYES`).
//!
//! ❌ **Ce chemin disait `signaling/src/server.ts`, et ce fichier n'existe
//! plus** : le paquet `signaling/` a été absorbé par `plateforme/` au
//! sous-bloc P1 du sous-projet ⑤. **La propriété énoncée, elle, reste
//! VRAIE** — `TYPES_RELAYES` porte toujours les mêmes six types (`offer`,
//! `answer`, `fenetre-ouverte`, `fenetre-fermee`, `refus`, `viewport`),
//! relus le 19 août 2026. Dette d'une ligne, laissée par P1 parce que
//! `agent/` était alors le périmètre d'un travail concurrent, et soldée
//! ici.
//!
//! **Pas de `#[cfg(windows)]`** : ces messages sont de la sérialisation pure,
//! et c'est justement le genre de contrat qui doit être éprouvé sur l'hôte —
//! un nom de champ qui dérive du côté agent ne se voit autrement qu'en session
//! réelle, sur la VM.

use serde::{Deserialize, Serialize};

/// Identifiant réservé de la session de contrôle. Le superviseur s'y déclare
/// en `agent`, la page-shell en `client`.
pub const SESSION_DE_CONTROLE: &str = "bureau";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum VersLaShell {
    #[serde(rename = "fenetre-ouverte")]
    FenetreOuverte { session: String, titre: String },
    #[serde(rename = "fenetre-fermee")]
    FenetreFermee { session: String },
    #[serde(rename = "refus")]
    Refus { titre: String, motif: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum DepuisLaShell {
    #[serde(rename = "viewport")]
    Viewport { session: String, largeur: u32, hauteur: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_ouverture_se_serialise_comme_la_shell_l_attend() {
        let json = serde_json::to_string(&VersLaShell::FenetreOuverte {
            session: "w-1".into(),
            titre: "Bloc-notes".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"type":"fenetre-ouverte","session":"w-1","titre":"Bloc-notes"}"#
        );
    }

    #[test]
    fn une_fermeture_et_un_refus_se_serialisent_comme_la_shell_les_attend() {
        // `shell-page.ts` lit `message.session` sur une fermeture et
        // `message.titre`/`message.motif` sur un refus : ces noms-là sont le
        // contrat, pas une commodité de nommage côté Rust.
        assert_eq!(
            serde_json::to_string(&VersLaShell::FenetreFermee { session: "w-2".into() }).unwrap(),
            r#"{"type":"fenetre-fermee","session":"w-2"}"#
        );
        assert_eq!(
            serde_json::to_string(&VersLaShell::Refus {
                titre: "Bloc-notes".into(),
                motif: "plus aucune sortie".into(),
            })
            .unwrap(),
            r#"{"type":"refus","titre":"Bloc-notes","motif":"plus aucune sortie"}"#
        );
    }

    #[test]
    fn un_viewport_de_la_shell_se_lit() {
        let message: DepuisLaShell = serde_json::from_str(
            r#"{"type":"viewport","session":"w-1","largeur":1600,"hauteur":900}"#,
        )
        .unwrap();
        let DepuisLaShell::Viewport { session, largeur, hauteur } = message;
        assert_eq!((session.as_str(), largeur, hauteur), ("w-1", 1600, 900));
    }

    #[test]
    fn un_message_inconnu_de_la_shell_est_refuse_plutot_qu_ignore() {
        let resultat: Result<DepuisLaShell, _> =
            serde_json::from_str(r#"{"type":"autre-chose"}"#);
        assert!(resultat.is_err());
    }

    /// Le signaling relaie aussi `ice-config` et `peer-gone` sur cette
    /// connexion : ils doivent tomber du côté « refusé » de cette frontière,
    /// pour que `signalisation.rs` les ignore sans les prendre pour un
    /// viewport.
    #[test]
    fn les_messages_de_service_du_signaling_ne_sont_pas_des_viewports() {
        assert!(serde_json::from_str::<DepuisLaShell>(r#"{"type":"peer-gone"}"#).is_err());
        assert!(
            serde_json::from_str::<DepuisLaShell>(r#"{"type":"ice-config","urls":[]}"#).is_err()
        );
    }
}
