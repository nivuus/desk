//! Transcription de l'ABI NVENC : versions de structures, et les constantes
//! d'énumération dont le chemin d'encodage a besoin.
//!
//! 🔴 **CE MODULE EST LA FRONTIÈRE D'ATTRIBUTION, ET C'EST DÉLIBÉRÉ.** Tout
//! ce qui est transcrit de l'en-tête amont vit ICI et nulle part ailleurs,
//! avec la notice ci-dessous ; le reste du dépôt est de nous. La licence de
//! l'en-tête l'exige, et regrouper la transcription rend la frontière
//! vérifiable d'un coup d'œil plutôt que de la disperser.
//!
//! ```text
//! /*
//!  * This copyright notice applies to this header file only:
//!  *
//!  * Copyright (c) 2010-2024 NVIDIA Corporation
//!  *
//!  * Permission is hereby granted, free of charge, to any person
//!  * obtaining a copy of this software and associated documentation
//!  * files (the "Software"), to deal in the Software without
//!  * restriction, including without limitation the rights to use,
//!  * copy, modify, merge, publish, distribute, sublicense, and/or sell
//!  * copies of the software, and to permit persons to whom the
//!  * software is furnished to do so, subject to the following
//!  * conditions:
//!  *
//!  * The above copyright notice and this permission notice shall be
//!  * included in all copies or substantial portions of the Software.
//!  *
//!  * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//!  * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES
//!  * OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//!  * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
//!  * HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
//!  * WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//!  * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
//!  * OTHER DEALINGS IN THE SOFTWARE.
//!  */
//! ```
//!
//! ⚠️ **Deux précisions qui ne se devinent pas.** ① Le texte ci-dessus **est**
//! celui de la licence MIT, mais l'en-tête ne le nomme jamais « MIT » et le
//! titulaire est **NVIDIA Corporation**, pas FFmpeg ; sa première ligne borne
//! elle-même la portée : *« applies to this header file only »*. ② Le dépôt
//! `nv-codec-headers` **ne porte AUCUN fichier `LICENSE`** — vérifié à ce
//! tag : la notice par en-tête EST la licence, et c'est elle qu'on cite.
//! 🔵 **Nous ne redistribuons pas `nvEncodeAPI64.dll`** : elle vient du
//! pilote installé. Ce qui est transcrit ici, c'est l'ABI. ⚠️ **L'usage de
//! NVENC à l'exécution relève de la licence du pilote NVIDIA, qui n'est PAS
//! celle-ci et qui n'a pas été lue** — question distincte, non tranchée ici.
//!
//! ## Provenance, et comment la relire
//!
//! | | |
//! | --- | --- |
//! | dépôt | `FFmpeg/nv-codec-headers` |
//! | tag | `n12.2.72.0` |
//! | commit | `c69278340ab1d5559c7d7bf0edf615dc33ddbba7` |
//! | fichier | `include/ffnvcodec/nvEncodeAPI.h` |
//! | sha256 | `4677a397e3ec5300a6b38bf49cba42bb63a922ab26f24bc63a05ed08857cba16` |
//!
//! **Pourquoi ce tag et pas le plus récent** : son `README` annonce un
//! plancher de pilote **Windows 551.76**, quand celui de `n13.1.15.0` exige
//! **610.0**, qui n'existe pas sur la VM cible. Les dispositions des
//! structures dont ce chemin dépend sont par ailleurs **identiques** entre
//! les deux tags.
//!
//! 🔴 **AUCUNE VALEUR DE CE FICHIER N'A ÉTÉ RECOPIÉE DE MÉMOIRE.** Chacune a
//! été obtenue en **compilant l'en-tête réel** et en imprimant la macro.
//! Refaire la dérivation, et c'est le contrôle qui vaut :
//!
//! ```text
//! curl -sL https://raw.githubusercontent.com/FFmpeg/nv-codec-headers/n12.2.72.0/include/ffnvcodec/nvEncodeAPI.h -o nvEncodeAPI.h
//! sha256sum nvEncodeAPI.h   # doit rendre le sha256 ci-dessus
//! printf '#include <stdio.h>\n#include <stdint.h>\n#include "nvEncodeAPI.h"\nint main(void){printf("%%08X\\n",(unsigned)NV_ENC_CONFIG_VER);}\n' > v.c
//! gcc v.c -o v && ./v        # doit rendre F209000C
//! ```
//!
//! 🔴 **POURQUOI CE FICHIER EXISTE SÉPARÉMENT, ET POURQUOI IL EST TESTÉ.**
//! Une version de structure fausse ne plante pas et ne dit rien : NVENC
//! **REFUSE**, avec `NV_ENC_ERR_INVALID_VERSION`. C'est-à-dire exactement le
//! symptôme qu'on essaie de faire disparaître — un encodeur qui ne se crée
//! pas. Une constante dont l'erreur est muette doit être **relisible**, donc
//! recalculée par une `const fn` et éprouvée contre la valeur mesurée.

// 🔴 **POURQUOI CE `allow`, ET QUAND LE RETIRER.** Ce module transcrit une
// ABI, et une ABI se transcrit ENTIÈRE : n'en déclarer que la moitié
// aujourd'hui obligerait le prochain à rouvrir l'en-tête amont, donc à
// repayer la vérification de provenance. Tant que la session d'encodage
// n'est pas écrite, ces constantes n'ont donc aucun appelant — 31
// avertissements `dead_code` (compté au 30 août 2026), qui NOIERAIENT les 24
// avertissements préexistants du binaire et rendraient inutilisable la règle
// du dépôt « vérifier la NATURE des avertissements, jamais leur nombre ».
//
// ⚠️ **Il est posé sur CE MODULE SEUL, jamais sur le crate**, et il masque
// exactement une famille : `dead_code`. Une constante fausse resterait
// fausse ; c'est le rôle des tests plus bas, pas celui du compilateur.
// **À retirer dès que la session d'encodage consomme ces constantes** — et
// ce qui le rappellera est ce commentaire, pas une note ailleurs.
#![allow(dead_code)]

/// Version majeure de l'API transcrite.
pub const VERSION_MAJEURE: u32 = 12;
/// Version mineure de l'API transcrite.
pub const VERSION_MINEURE: u32 = 2;

/// `NVENCAPI_VERSION` — ⚠️ **empaquetage `majeure | (mineure << 24)`**, qui
/// n'est PAS celui que rend le pilote (voir `version_pilote_attendue`).
pub const VERSION_API: u32 = VERSION_MAJEURE | (VERSION_MINEURE << 24);

/// `NVENCAPI_STRUCT_VERSION(ver)` de l'en-tête, réimplémentée.
///
/// Le `0x7 << 28` est une étiquette que porte toute version de structure.
pub const fn version_de_structure(revision: u32) -> u32 {
    VERSION_API | (revision << 16) | (0x7 << 28)
}

/// Les structures dont l'en-tête ajoute `1u << 31` à leur version.
///
/// 🔴 **Omettre ce bit rend `NV_ENC_ERR_INVALID_VERSION`**, et rien d'autre
/// ne le signale. C'est pour cela que les deux formes sont deux fonctions
/// distinctes plutôt qu'un booléen à ne pas oublier.
pub const fn version_de_structure_marquee(revision: u32) -> u32 {
    version_de_structure(revision) | (1 << 31)
}

/// Ce que `NvEncodeAPIGetMaxSupportedVersion` doit rendre au minimum.
///
/// 🔴 **L'EMPAQUETAGE EST `(majeure << 4) | mineure`, ET IL DIFFÈRE DE
/// `VERSION_API`.** L'en-tête le dit en toutes lettres : « the 4 least
/// significant bits […] indicate the minor version and the rest of the bits
/// indicate the major version ». Confondre les deux fait comparer 0x0200000C
/// à 0xC2 et rejeter tous les pilotes du monde.
pub const fn version_pilote_attendue() -> u32 {
    (VERSION_MAJEURE << 4) | VERSION_MINEURE
}

/// Le pilote installé sait-il parler la version qu'on a transcrite ?
///
/// À appeler **avant** `NvEncodeAPICreateInstance` : sinon l'échec arrive
/// plus tard et se lit moins bien.
pub const fn pilote_compatible(rendu_par_le_pilote: u32) -> bool {
    rendu_par_le_pilote >= version_pilote_attendue()
}

// --- Les versions de structures, chacune avec sa révision d'en-tête. ---
pub const OPEN_ENCODE_SESSION_EX_PARAMS_VER: u32 = version_de_structure(1);
pub const INITIALIZE_PARAMS_VER: u32 = version_de_structure_marquee(7);
pub const CONFIG_VER: u32 = version_de_structure_marquee(9);
pub const RC_PARAMS_VER: u32 = version_de_structure(1);
pub const PRESET_CONFIG_VER: u32 = version_de_structure_marquee(5);
pub const REGISTER_RESOURCE_VER: u32 = version_de_structure(5);
pub const MAP_INPUT_RESOURCE_VER: u32 = version_de_structure(4);
pub const CREATE_BITSTREAM_BUFFER_VER: u32 = version_de_structure(1);
pub const PIC_PARAMS_VER: u32 = version_de_structure_marquee(7);
pub const LOCK_BITSTREAM_VER: u32 = version_de_structure_marquee(2);
pub const FUNCTION_LIST_VER: u32 = version_de_structure(2);

// --- Les constantes d'énumération du chemin d'encodage. ---

/// `NV_ENC_DEVICE_TYPE_DIRECTX`. ⚠️ Le commentaire de l'en-tête dit
/// « directx9 » ; c'est **aussi** la valeur pour D3D11 et D3D12, il n'existe
/// pas de constante D3D11 séparée.
pub const DEVICE_TYPE_DIRECTX: u32 = 0;
/// `NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX`.
pub const INPUT_RESOURCE_TYPE_DIRECTX: u32 = 0;
/// `NV_ENC_INPUT_IMAGE`, l'usage d'un tampon d'entrée.
pub const BUFFER_USAGE_INPUT_IMAGE: u32 = 0;

/// `NV_ENC_BUFFER_FORMAT_NV12`.
pub const BUFFER_FORMAT_NV12: u32 = 0x0000_0001;

/// `NV_ENC_BUFFER_FORMAT_ARGB`.
///
/// 🔴 **C'EST LE FORMAT DE CE QUE LA CAPTURE NOUS DONNE, MALGRÉ SON NOM.**
/// L'en-tête le définit comme *word-ordered* avec **B dans les 8 bits de
/// poids faible** — c'est-à-dire, en mémoire petit-boutiste, l'ordre d'octets
/// B, G, R, A : exactement `DXGI_FORMAT_B8G8R8A8_UNORM`, ce que rend
/// Desktop Duplication. **Prendre `ABGR` à la place intervertit le rouge et
/// le bleu SANS AUCUNE ERREUR** — l'image sort, simplement fausse.
pub const BUFFER_FORMAT_ARGB: u32 = 0x0100_0000;
/// `NV_ENC_BUFFER_FORMAT_ABGR` — **le piège voisin**, déclaré pour qu'un
/// test puisse établir qu'on ne l'a pas pris par mégarde.
pub const BUFFER_FORMAT_ABGR: u32 = 0x1000_0000;

/// `NV_ENC_PARAMS_RC_CBR` — débit constant, ce que l'interactif exige.
pub const RC_MODE_CBR: u32 = 2;

/// `NV_ENC_PIC_STRUCT_FRAME`. ⚠️ **Vaut 1, pas 0** : mettre la structure à
/// zéro et oublier ce champ est une erreur, pas un défaut inoffensif.
pub const PIC_STRUCT_FRAME: u32 = 1;

/// `NV_ENC_PIC_FLAG_FORCEIDR`.
pub const PIC_FLAG_FORCEIDR: u32 = 0x2;
/// `NV_ENC_PIC_FLAG_OUTPUT_SPSPPS`.
pub const PIC_FLAG_OUTPUT_SPSPPS: u32 = 0x4;

/// `NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY`.
pub const TUNING_ULTRA_LOW_LATENCY: u32 = 3;

/// `NV_ENC_ERR_INVALID_VERSION` — le code que rend une version fausse.
pub const ERR_INVALID_VERSION: u32 = 15;
/// `NV_ENC_SUCCESS`.
pub const SUCCESS: u32 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 **LES VALEURS ATTENDUES NE SONT PAS RECOPIÉES : ELLES SONT
    /// MESURÉES.** Chacune a été imprimée en compilant l'en-tête réel
    /// (commande dans l'en-tête de ce module). Ce test compare notre
    /// arithmétique à ce relevé — c'est ce qui rend relisible une constante
    /// dont l'erreur serait muette.
    #[test]
    fn les_versions_de_structures_valent_celles_de_l_entete() {
        assert_eq!(VERSION_API, 0x0200_000C, "NVENCAPI_VERSION");
        assert_eq!(OPEN_ENCODE_SESSION_EX_PARAMS_VER, 0x7201_000C);
        assert_eq!(INITIALIZE_PARAMS_VER, 0xF207_000C);
        assert_eq!(CONFIG_VER, 0xF209_000C);
        assert_eq!(RC_PARAMS_VER, 0x7201_000C);
        assert_eq!(PRESET_CONFIG_VER, 0xF205_000C);
        assert_eq!(REGISTER_RESOURCE_VER, 0x7205_000C);
        assert_eq!(MAP_INPUT_RESOURCE_VER, 0x7204_000C);
        assert_eq!(CREATE_BITSTREAM_BUFFER_VER, 0x7201_000C);
        assert_eq!(PIC_PARAMS_VER, 0xF207_000C);
        assert_eq!(LOCK_BITSTREAM_VER, 0xF202_000C);
        assert_eq!(FUNCTION_LIST_VER, 0x7202_000C);
    }

    /// 🔴 Le bit `1 << 31` sépare les deux familles. S'il disparaissait des
    /// cinq structures qui le portent, NVENC les refuserait **en silence**.
    #[test]
    fn le_bit_de_poids_fort_distingue_les_deux_familles() {
        for (nom, v) in [
            ("INITIALIZE_PARAMS", INITIALIZE_PARAMS_VER),
            ("CONFIG", CONFIG_VER),
            ("PRESET_CONFIG", PRESET_CONFIG_VER),
            ("PIC_PARAMS", PIC_PARAMS_VER),
            ("LOCK_BITSTREAM", LOCK_BITSTREAM_VER),
        ] {
            assert_ne!(v & (1 << 31), 0, "{nom} doit porter le bit 31");
        }
        for (nom, v) in [
            ("OPEN_ENCODE_SESSION_EX_PARAMS", OPEN_ENCODE_SESSION_EX_PARAMS_VER),
            ("RC_PARAMS", RC_PARAMS_VER),
            ("REGISTER_RESOURCE", REGISTER_RESOURCE_VER),
            ("MAP_INPUT_RESOURCE", MAP_INPUT_RESOURCE_VER),
            ("CREATE_BITSTREAM_BUFFER", CREATE_BITSTREAM_BUFFER_VER),
            ("FUNCTION_LIST", FUNCTION_LIST_VER),
        ] {
            assert_eq!(v & (1 << 31), 0, "{nom} ne doit PAS porter le bit 31");
        }
    }

    /// 🔴 **LES DEUX EMPAQUETAGES DE VERSION SONT DIFFÉRENTS**, et les
    /// confondre rejetterait tous les pilotes. Ce test fige l'écart.
    #[test]
    fn la_version_attendue_du_pilote_n_est_pas_celle_de_l_api() {
        assert_eq!(version_pilote_attendue(), 0xC2, "(12 << 4) | 2");
        assert_ne!(version_pilote_attendue(), VERSION_API);
    }

    #[test]
    fn un_pilote_trop_ancien_est_refuse_et_un_plus_recent_accepte() {
        assert!(pilote_compatible(0xC2), "12.2 exactement");
        assert!(pilote_compatible((13 << 4) | 1), "13.1, plus récent");
        assert!(!pilote_compatible((12 << 4) | 1), "12.1, trop ancien");
        assert!(!pilote_compatible(0), "pilote muet");
    }

    /// 🔴 Le piège rouge/bleu, figé : ce que la capture produit
    /// (`DXGI_FORMAT_B8G8R8A8_UNORM`) se déclare `ARGB` à NVENC, jamais
    /// `ABGR` — l'inverse sortirait une image sans la moindre erreur.
    #[test]
    fn le_format_de_la_capture_est_argb_et_non_abgr() {
        assert_eq!(BUFFER_FORMAT_ARGB, 0x0100_0000);
        assert_eq!(BUFFER_FORMAT_ABGR, 0x1000_0000);
        assert_ne!(BUFFER_FORMAT_ARGB, BUFFER_FORMAT_ABGR);
    }

    #[test]
    fn les_constantes_d_enumeration_valent_celles_de_l_entete() {
        assert_eq!(DEVICE_TYPE_DIRECTX, 0);
        assert_eq!(INPUT_RESOURCE_TYPE_DIRECTX, 0);
        assert_eq!(BUFFER_FORMAT_NV12, 1);
        assert_eq!(RC_MODE_CBR, 2);
        assert_eq!(PIC_STRUCT_FRAME, 1, "vaut 1, pas 0");
        assert_eq!(PIC_FLAG_FORCEIDR, 2);
        assert_eq!(PIC_FLAG_OUTPUT_SPSPPS, 4);
        assert_eq!(TUNING_ULTRA_LOW_LATENCY, 3);
        assert_eq!(ERR_INVALID_VERSION, 15);
        assert_eq!(SUCCESS, 0);
    }
}
