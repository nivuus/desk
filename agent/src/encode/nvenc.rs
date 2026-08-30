//! La part **PURE** du chemin NVENC : le choix de la voie, la traduction de
//! nos réglages vers ceux de NVENC, et l'arithmétique de version des
//! structures. Aucun `cfg`, aucun appel Windows, **testée sur l'hôte Linux**.
//!
//! ⚠️ **`#[path]` chez le parent, et c'est vérifié contre la convention**
//! (§ « Convention de module enfant », tête de `CLAUDE.md`) : ce module est
//! extrait d'`encode.rs`, qui est `#![cfg(windows)]`, précisément pour que sa
//! logique pure compile et se teste sur l'hôte. Son nom, `encode_nvenc`,
//! porte le préfixe `encode_` d'un module de premier niveau existant
//! (`mod encode;`, `main.rs`), donc la règle le range **chez son parent** :
//! fichier `encode/nvenc.rs`, déclaration
//! `#[path = "encode/nvenc.rs"] mod encode_nvenc;` dans `main.rs`.
//! Aucun autre module de premier niveau n'en est un préfixe — la clause du
//! « préfixe le plus long » ne change donc rien ici. Même précédent que
//! `wasapi_format`, hissé pour exactement la même raison.
//!
//! 🔴 **POURQUOI NVENC AVANT LA MFT, ET POURQUOI LA MFT RESTE.** Ce n'est
//! pas une préférence, c'est une mesure, et les commandes qui l'établissent
//! sont données pour qu'on puisse la refaire sans croire personne — le
//! précédent que ce dépôt paie en ce moment même est un commentaire faux qui
//! a fait concevoir un défaut (`placement.rs`).
//!
//! - Sur la VM cible, le 30 août 2026, la MFT `NVIDIA H.264 Encoder MFT`
//!   s'active en **session 0** et rend `0x8000FFFF` en **session 1**, sur
//!   les quatre arrangements que Media Foundation permet d'essayer. Deux
//!   témoins verts posés dans la même exécution — l'encodeur H.264
//!   **logiciel** et le processeur vidéo **logiciel** s'activent, eux, dans
//!   les deux sessions — établissent que la machinerie n'est pas en cause.
//! - Apollo, sur la MÊME machine, dans la MÊME session 1, fabrique six
//!   encodeurs NVENC par la porte **native** : son processus vivant ne porte
//!   **aucun** module Media Foundation.
//!
//! **Refaire la mesure** (détail et relevés bruts :
//! `docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md`) :
//!
//! ```text
//! # les modules d'Apollo pendant qu'il encode, en session 1 :
//! (Get-Process sunshine).Modules | ? { $_.ModuleName -match 'mfplat|nvEnc' }
//! # les encodeurs qu'il a fabriqués :
//! Select-String 'NvEnc: created encoder' 'C:\Program Files\Apollo\config\sunshine.log'
//! ```
//!
//! 🔴 **ET LA MFT NE DOIT PAS ÊTRE RETIRÉE.** `MFTEnumEx` n'énumère pas
//! « l'encodeur NVIDIA » : il énumère **les encodeurs H.264 matériels**,
//! Intel Quick Sync et AMD VCE compris. Une machine sans NVIDIA n'a aucun
//! NVENC ; lui retirer la MFT la priverait de **tout** encodeur matériel.
//! La MFT est donc le repli **générique**, et elle reste inchangée.

/// La transcription de l'ABI amont, isolée dans son propre fichier parce
/// qu'elle porte une notice de licence qui ne s'applique qu'à elle.
///
/// ⚠️ **Le `#[path]` ci-dessous N'EST PAS celui de la convention du dépôt,
/// et les confondre embrouillerait le prochain lecteur.** La convention vise
/// les modules qu'on extrait d'un parent **non portable** pour les compiler
/// sur l'hôte ; `abi` n'a rien à fuir, son parent est déjà pur. Ce `#[path]`
/// est imposé par une règle de **rustc** : quand un module est lui-même
/// chargé par `#[path = "encode/nvenc.rs"]`, ses enfants sont cherchés dans
/// le répertoire de CE fichier — `encode/` — et non dans un `encode/nvenc/`
/// homonyme. Sans la ligne explicite, rustc réclame `encode/abi.rs`
/// (mesuré : `error[E0583]: file not found for module 'abi'`). C'est le même
/// mécanisme employé pour une autre raison, exactement comme `table.rs` s'en
/// sert pour scinder ses tests.
#[path = "nvenc/abi.rs"]
pub mod abi;

/// Les dispositions de structures, même frontière d'attribution qu'`abi`.
/// Même raison pour le `#[path]` — c'est rustc qui l'impose, pas la
/// convention de nommage du dépôt.
#[path = "nvenc/structures.rs"]
pub mod structures;

/// Les dispositions qui s'echangent par image. Meme frontiere
/// d'attribution, meme raison de `#[path]`.
#[path = "nvenc/tampons.rs"]
pub mod tampons;

/// La table de fonctions du pilote. Meme frontiere, meme raison de `#[path]`.
#[path = "nvenc/fonctions.rs"]
pub mod fonctions;

/// La session d'encodage elle-meme. **Le SEUL fichier du sous-arbre NVENC
/// qui ait besoin de Windows** : tout le reste -- regle de choix, ABI,
/// dispositions -- se teste sur l'hote. Il vit ici plutot que sous
/// `encode.rs` pour que la frontiere d'attribution de la notice de licence
/// reste UN seul sous-arbre.
#[cfg(windows)]
#[path = "nvenc/porte.rs"]
pub mod porte;

#[cfg(windows)]
#[path = "nvenc/session.rs"]
pub mod session;

/// L'identifiant de vendeur PCI de NVIDIA.
///
/// Relevé sur la VM cible plutôt que recopié d'une liste : la sonde du lot 31
/// a lu `vendeur=0x10DE peripherique=0x2786` sur `NVIDIA GeForce RTX 4070`,
/// et `0x1414` (Microsoft) sur le `Basic Render Driver` du VGA QEMU.
pub const VENDEUR_NVIDIA: u32 = 0x10DE;

/// Un adaptateur graphique, réduit à ce dont la décision a besoin.
///
/// Volontairement **sans type Windows** : c'est ce qui permet à la règle
/// ci-dessous d'être éprouvée sur l'hôte. L'appelant `#[cfg(windows)]`
/// remplit ces champs depuis `IDXGIAdapter1::GetDesc1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adaptateur {
    pub nom: String,
    pub vendeur: u32,
    /// Le LUID DXGI, à plat. NVENC ne s'en sert pas pour choisir — c'est le
    /// périphérique D3D11 qui porte le choix — mais le tracer permet de dire
    /// **lequel** des adaptateurs homonymes a été retenu, et cette VM en
    /// présente deux qui portent le même nom et le même identifiant de
    /// périphérique.
    pub luid: (i32, u32),
}

/// La voie d'encodage retenue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voie {
    /// L'API NVENC native (`nvEncodeAPI64.dll`), sur l'adaptateur d'indice
    /// donné.
    Nvenc(usize),
    /// La MFT Media Foundation — le repli **générique**, pour Intel, AMD, et
    /// pour la session 0 où la MFT NVIDIA fonctionne.
    Mft,
}

/// Choisit la voie à partir des seuls adaptateurs présents.
///
/// **Le premier adaptateur NVIDIA l'emporte, et l'ordre d'énumération DXGI
/// fait foi.** ⚠️ Ce n'est PAS le piège des index positionnels payé en D1 :
/// on ne mémorise ni ne transporte cet indice d'une exécution à l'autre, il
/// n'est qu'un renvoi dans la liste qu'on vient de lire, dans le même appel.
/// Le nom et le LUID sont rendus avec, pour que la trace dise **lequel**.
///
/// **Aucun adaptateur NVIDIA ⇒ `Mft`**, et c'est le cas nominal d'une machine
/// Intel ou AMD : voir le commentaire de module, la MFT est le repli
/// générique et non un pis-aller.
pub fn choisir_voie(adaptateurs: &[Adaptateur]) -> Voie {
    match adaptateurs
        .iter()
        .position(|a| a.vendeur == VENDEUR_NVIDIA)
    {
        Some(index) => Voie::Nvenc(index),
        None => Voie::Mft,
    }
}

/// Fabrique d'appui partagée par les deux modules de tests de ce fichier.
#[cfg(test)]
mod tests_appui {
    use super::Adaptateur;
    pub fn adaptateur(nom: &str, vendeur: u32) -> Adaptateur {
        Adaptateur {
            nom: nom.to_string(),
            vendeur,
            luid: (0, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_appui::adaptateur;
    use super::*;

    #[test]
    fn une_machine_nvidia_prend_la_voie_native() {
        let vus = vec![adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA)];
        assert_eq!(choisir_voie(&vus), Voie::Nvenc(0));
    }

    /// 🔴 Le cas qui interdit de retirer la MFT : une machine sans NVIDIA.
    /// Si cette assertion tombait à `Nvenc`, la machine perdrait TOUT
    /// encodeur matériel — c'est la régression que ce test fige.
    #[test]
    fn une_machine_sans_nvidia_garde_la_mft() {
        let vus = vec![
            adaptateur("Intel(R) UHD Graphics 770", 0x8086),
            adaptateur("Microsoft Basic Render Driver", 0x1414),
        ];
        assert_eq!(choisir_voie(&vus), Voie::Mft);
    }

    #[test]
    fn aucun_adaptateur_du_tout_garde_la_mft() {
        assert_eq!(choisir_voie(&[]), Voie::Mft);
    }

    /// La topologie EXACTE de la VM cible, relevée par la sonde du lot 31 en
    /// session 1 : le VGA QEMU est l'adaptateur **0** et porte le seul
    /// affichage attaché ; les deux NVIDIA n'en portent aucun. La voie doit
    /// néanmoins être NVENC, et viser le premier NVIDIA — c'est-à-dire
    /// l'indice **1**, pas l'indice 0.
    #[test]
    fn la_topologie_mesuree_de_la_vm_vise_le_premier_nvidia() {
        let vus = vec![
            adaptateur("Microsoft Basic Render Driver", 0x1414),
            adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA),
            adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA),
            adaptateur("Microsoft Basic Render Driver", 0x1414),
        ];
        assert_eq!(choisir_voie(&vus), Voie::Nvenc(1));
    }

    /// ⚠️ Le nom ne décide de RIEN : c'est le vendeur. Un adaptateur qui se
    /// nommerait « NVIDIA … » sans porter `0x10DE` ne doit pas emmener vers
    /// une DLL que sa machine n'a pas.
    #[test]
    fn le_nom_ne_decide_pas_le_vendeur_decide() {
        let vus = vec![adaptateur("NVIDIA GeForce RTX 4070", 0x1414)];
        assert_eq!(choisir_voie(&vus), Voie::Mft);
    }
}

/// `E_UNEXPECTED` — « Catastrophic failure ». Le code que la MFT NVIDIA rend
/// en session 1 sur la VM cible, mesuré le 30 août 2026.
pub const ECHEC_CATASTROPHIQUE: i32 = 0x8000_FFFFu32 as i32;

/// **Étage ③ des trois du lot 31 : rendre l'échec LISIBLE.**
///
/// 🔴 **CECI NE FAIT PAS MARCHER LE PRODUIT, et n'est pas écrit comme si ça
/// le faisait.** Quand les deux étages utiles ont échoué, il reste à ne pas
/// laisser remonter un `0x8000FFFF` nu : les lots 30 et 31 ont coûté deux
/// journées à établir ce que ce message dit en quelques lignes, et sans lui
/// le prochain lecteur les repaierait.
///
/// Le message nomme trois choses, dans cet ordre : le code rendu, ce que la
/// machine porte comme adaptateurs (c'est la variable qui décide), et le
/// document qui porte la mesure.
///
/// ⚠️ **Le paragraphe de cause connue n'est ajouté QUE si les deux
/// conditions mesurées sont réunies** — le code exact ET un adaptateur
/// NVIDIA. Sur une machine Intel ou AMD, le même code voudrait dire autre
/// chose, et affirmer notre diagnostic y serait une affirmation fausse
/// présentée comme un fait.
pub fn diagnostic_activation(code: i32, erreur: &str, adaptateurs: &[Adaptateur]) -> String {
    let noms: Vec<&str> = adaptateurs.iter().map(|a| a.nom.as_str()).collect();
    let mut message = format!(
        "activation de l'encodeur H.264 matériel (ActivateObject) : {erreur} \
         — adaptateurs vus : [{}]",
        noms.join(" | ")
    );
    if code == ECHEC_CATASTROPHIQUE && matches!(choisir_voie(adaptateurs), Voie::Nvenc(_)) {
        message.push_str(
            " — CAUSE CONNUE, MESUREE LE 30 AOUT 2026 (lot 31) : la MFT \
             « NVIDIA H.264 Encoder MFT » rend 0x8000FFFF en SESSION 1 sur cette \
             machine, alors qu'elle s'active en session 0. Ce n'est ni le pilote \
             absent, ni Media Foundation en panne : dans la meme execution, \
             l'encodeur H.264 LOGICIEL et le processeur video LOGICIEL s'activent \
             tous deux. Poser MFT_ENUM_ADAPTER_LUID, tenir un peripherique D3D11 \
             NVIDIA vivant, ou lier l'affichage virtuel au GPU NVIDIA sont TROIS \
             remedes deja REFUTES PAR LA MESURE — ne pas les reessayer. La voie \
             qui fonctionne ici est l'API NVENC native. Detail, releves bruts et \
             remedes refutes : \
             docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md",
        );
    }
    message
}

#[cfg(test)]
mod tests_diagnostic {
    use super::tests_appui::adaptateur;
    use super::*;

    const AUTRE_CODE: i32 = 0x8007_0057u32 as i32; // E_INVALIDARG

    #[test]
    fn nomme_les_adaptateurs_vus() {
        let vus = vec![
            adaptateur("Microsoft Basic Render Driver", 0x1414),
            adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA),
        ];
        let m = diagnostic_activation(ECHEC_CATASTROPHIQUE, "Catastrophic failure", &vus);
        assert!(m.contains("Microsoft Basic Render Driver | NVIDIA GeForce RTX 4070"), "{m}");
    }

    #[test]
    fn ajoute_la_cause_connue_quand_les_deux_conditions_sont_reunies() {
        let vus = vec![adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA)];
        let m = diagnostic_activation(ECHEC_CATASTROPHIQUE, "Catastrophic failure", &vus);
        assert!(m.contains("CAUSE CONNUE"), "{m}");
        assert!(m.contains("2026-08-30-encodeur-porte-apollo-resultats.md"), "{m}");
    }

    /// 🔴 Le bras qui empêche d'affirmer notre diagnostic là où il ne
    /// s'applique pas : même code, machine SANS NVIDIA.
    #[test]
    fn se_tait_sur_la_cause_quand_aucun_nvidia_n_est_present() {
        let vus = vec![adaptateur("Intel(R) UHD Graphics 770", 0x8086)];
        let m = diagnostic_activation(ECHEC_CATASTROPHIQUE, "Catastrophic failure", &vus);
        assert!(!m.contains("CAUSE CONNUE"), "{m}");
        assert!(m.contains("Intel(R) UHD Graphics 770"), "{m}");
    }

    /// L'autre bras : machine NVIDIA, mais un AUTRE code d'erreur.
    #[test]
    fn se_tait_sur_la_cause_pour_un_autre_code() {
        let vus = vec![adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA)];
        let m = diagnostic_activation(AUTRE_CODE, "Paramètre incorrect", &vus);
        assert!(!m.contains("CAUSE CONNUE"), "{m}");
    }

    #[test]
    fn sans_aucun_adaptateur_le_message_reste_lisible() {
        let m = diagnostic_activation(ECHEC_CATASTROPHIQUE, "Catastrophic failure", &[]);
        assert!(m.contains("adaptateurs vus : []"), "{m}");
        assert!(!m.contains("CAUSE CONNUE"), "{m}");
    }
}
