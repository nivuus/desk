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

/// Nom réservé de la session de contrôle. Le superviseur s'y déclare en
/// `agent`, la page-shell en `client`.
///
/// ⚠️ **CE N'EST PLUS UN IDENTIFIANT DE SESSION À LUI SEUL** (sous-bloc P3) :
/// c'est le nom qui suit le préfixe de la VM. Passer par
/// [`session_de_controle`], jamais par cette constante nue — deux VMs qui
/// ouvriraient toutes deux `bureau` se disputeraient la même session sur la
/// plateforme, et la seconde serait refusée en « un agent est déjà connecté ».
pub const NOM_SESSION_DE_CONTROLE: &str = "bureau";

/// Le séparateur du préfixe, tel que la spec §3.4 l'écrit.
///
/// ⚠️ Il apparaît aussi dans l'identifiant TURN que la plateforme dérive
/// (`<expiration>:<session>`), qui devient donc à trois segments. Le préfixe
/// étant en `base64url` il ne peut pas en contenir : la première borne reste
/// non ambiguë. **Propriété non éprouvée contre un coturn vivant.**
pub const SEPARATEUR_PREFIXE: char = ':';

/// Compose un identifiant de session : `<préfixe>:<nom>`, ou `<nom>` seul
/// quand aucun préfixe n'est connu.
///
/// 🔴 **Le préfixe vide doit restituer EXACTEMENT le nom d'aujourd'hui.** Un
/// `":bureau"` silencieux n'est le nom d'aucune session existante, et rien ne
/// le signalerait.
pub fn composer(prefixe: &str, nom: &str) -> String {
    if prefixe.is_empty() {
        return nom.to_string();
    }
    format!("{prefixe}{SEPARATEUR_PREFIXE}{nom}")
}

/// L'identifiant complet de la session de contrôle pour un préfixe donné.
pub fn session_de_controle(prefixe: &str) -> String {
    composer(prefixe, NOM_SESSION_DE_CONTROLE)
}

/// Nom réservé de la session de signaling du **pont fichiers**.
///
/// DISTINCT de [`NOM_SESSION_DE_CONTROLE`] : le relais n'accepte qu'un `agent`
/// et un `client` par identifiant (`plateforme/src/signaling/appariement.ts`),
/// et la page-shell occupe déjà le rôle `client` de `bureau`. Deux
/// `PeerConnection` vers la même VM exigent donc deux identifiants.
///
/// ⚠️ **CE N'EST PAS UN IDENTIFIANT DE SESSION À LUI SEUL**, exactement comme
/// son voisin depuis le sous-bloc P3 : passer par [`session_du_pont`], jamais
/// par cette constante nue. Deux VMs qui ouvriraient toutes deux `fichiers` se
/// disputeraient la même session sur la plateforme, et la seconde serait
/// refusée en « un agent est déjà connecté ».
///
/// *(Le plan de F1 écrivait `pub const SESSION_DU_PONT: &str = "fichiers"`,
/// employée telle quelle, et notait que l'identifiant « n'est pas namespacé
/// par utilisateur ». Il a été écrit avant que P3 ne pose le préfixe : la
/// remarque est donc CADUQUE — le préfixe est ce namespace — et la forme nue
/// aurait réintroduit le défaut que P3 venait de corriger, sur la seule
/// session qui l'aurait échappé.)*
pub const NOM_SESSION_DU_PONT: &str = "fichiers";

/// L'identifiant complet de la session du pont fichiers pour un préfixe donné.
pub fn session_du_pont(prefixe: &str) -> String {
    composer(prefixe, NOM_SESSION_DU_PONT)
}

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

    #[test]
    fn un_prefixe_vide_restitue_exactement_le_nom_d_aujourd_hui() {
        // 🔴 LE TEST LE PLUS IMPORTANT DE CE FICHIER. Sans lui, poser le
        // séparateur inconditionnellement donnerait `":bureau"` et `":w-1"` —
        // qui ne sont le nom d'AUCUNE session existante, et rien ne le
        // signalerait : la page-shell attendrait une fenêtre qui ne vient pas.
        assert_eq!(composer("", NOM_SESSION_DE_CONTROLE), "bureau");
        assert_eq!(composer("", "w-1"), "w-1");
        assert_eq!(session_de_controle(""), "bureau");
    }

    #[test]
    fn la_session_du_pont_suit_le_prefixe_comme_celle_de_controle() {
        // 🔴 Le pont a sa PROPRE session parce que le relais n'accepte qu'un
        // `agent` et un `client` par identifiant, et que la page-shell occupe
        // déjà le rôle `client` de `bureau`.
        //
        // Elle doit suivre le préfixe exactement comme sa voisine : le plan de
        // F1, écrit avant le sous-bloc P3, prescrivait une constante NUE
        // employée telle quelle. Deux VMs auraient alors ouvert toutes deux
        // `fichiers`, et la seconde aurait été refusée en « un agent est déjà
        // connecté » — le défaut même que P3 venait de corriger, réintroduit
        // sur la seule session qui l'aurait échappé.
        assert_eq!(session_du_pont(""), "fichiers");
        assert_eq!(session_du_pont("Zm9vYmFy"), "Zm9vYmFy:fichiers");
        // …et les deux sessions d'une même VM restent DISTINCTES, ce qui est
        // toute la raison d'être de cette constante.
        assert_ne!(session_du_pont("Zm9vYmFy"), session_de_controle("Zm9vYmFy"));
        assert_ne!(session_du_pont(""), session_de_controle(""));
    }

    #[test]
    fn un_prefixe_pose_precede_le_nom_et_le_separe_par_deux_points() {
        assert_eq!(composer("Zm9vYmFy", "w-1"), "Zm9vYmFy:w-1");
        assert_eq!(session_de_controle("Zm9vYmFy"), "Zm9vYmFy:bureau");
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
