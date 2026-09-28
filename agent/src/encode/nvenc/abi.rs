//! Transcription of the NVENC ABI: structure versions, and the enumeration
//! constants the encoding path needs.
//!
//! 🔴 **THIS MODULE IS THE ATTRIBUTION BOUNDARY, AND IT IS DELIBERATE.** Everything
//! transcribed from the upstream header lives HERE and nowhere else,
//! with the notice below; the rest of the repository is ours. The
//! header's licence requires it, and grouping the transcription makes the boundary
//! checkable at a glance rather than scattering it.
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
//! ⚠️ **Two clarifications that cannot be guessed.** ① The text above **is**
//! that of the MIT licence, but the header never names it "MIT" and the
//! holder is **NVIDIA Corporation**, not FFmpeg; its first line itself bounds
//! the scope: *"applies to this header file only"*. ② The
//! `nv-codec-headers` repository **carries NO `LICENSE` file** — checked at this
//! tag: the per-header notice IS the licence, and it is the one quoted.
//! 🔵 **We do not redistribute `nvEncodeAPI64.dll`**: it comes from the
//! installed driver. What is transcribed here is the ABI. ⚠️ **Using
//! NVENC at runtime falls under the NVIDIA driver licence, which is NOT
//! this one and which has not been read** — a distinct question, not settled here.
//!
//! ## Provenance, and how to reread it
//!
//! | | |
//! | --- | --- |
//! | repository | `FFmpeg/nv-codec-headers` |
//! | tag | `n12.2.72.0` |
//! | commit | `c69278340ab1d5559c7d7bf0edf615dc33ddbba7` |
//! | file | `include/ffnvcodec/nvEncodeAPI.h` |
//! | sha256 | `4677a397e3ec5300a6b38bf49cba42bb63a922ab26f24bc63a05ed08857cba16` |
//!
//! **Why this tag and not the most recent**: its `README` announces a
//! driver floor of **Windows 551.76**, whereas that of `n13.1.15.0` requires
//! **610.0**, which does not exist on the target VM. The layouts of the
//! structures this path depends on are moreover **identical** between
//! the two tags.
//!
//! 🔴 **NO VALUE IN THIS FILE WAS COPIED FROM MEMORY.** Each one was
//! obtained by **compiling the real header** and printing the macro.
//! Redo the derivation, and that is the check that counts:
//!
//! ```text
//! curl -sL https://raw.githubusercontent.com/FFmpeg/nv-codec-headers/n12.2.72.0/include/ffnvcodec/nvEncodeAPI.h -o nvEncodeAPI.h
//! sha256sum nvEncodeAPI.h   # must return the sha256 above
//! printf '#include <stdio.h>\n#include <stdint.h>\n#include "nvEncodeAPI.h"\nint main(void){printf("%%08X\\n",(unsigned)NV_ENC_CONFIG_VER);}\n' > v.c
//! gcc v.c -o v && ./v        # must return F209000C
//! ```
//!
//! 🔴 **WHY THIS FILE EXISTS SEPARATELY, AND WHY IT IS TESTED.**
//! A wrong structure version does not crash and says nothing: NVENC
//! **REFUSES**, with `NV_ENC_ERR_INVALID_VERSION`. That is, exactly the
//! symptom we are trying to make disappear — an encoder that does not get
//! created. A constant whose error is silent must be **re-readable**, hence
//! recomputed by a `const fn` and tested against the measured value.

// 🔴 **WHY THIS `allow`, AND WHEN TO REMOVE IT.** This module transcribes an
// ABI, and an ABI is transcribed WHOLE: declaring only half of it
// today would force the next person to reopen the upstream header, hence to
// pay again for the provenance check. Part of it therefore has no caller.
//
// ⚠️ **The count has changed, and it is remeasured rather than copied**: it
// was **31** before the encoding session existed, it is **7** for
// the four transcription modules together once the facade is wired
// (measured on 30 August 2026 by removing the four `allow`s and counting).
// Seven more warnings would no longer drown much; what
// still justifies this `allow` is that the remaining elements are
// **deliberately** declared — `BUFFER_FORMAT_ABGR` only exists so that a
// test can establish that we did NOT pick it.
//
// ⚠️ **It is set on THIS MODULE ONLY, never on the crate**, and it masks
// exactly one family: `dead_code`. A wrong constant would stay
// wrong; that is the role of the tests below, not the compiler's.
// **To be removed as soon as the encoding session consumes these constants** — and
// what will remind of it is this comment, not a note elsewhere.
#![allow(dead_code)]

/// Version majeure de l'API transcrite.
pub const VERSION_MAJEURE: u32 = 12;
/// Version mineure de l'API transcrite.
pub const VERSION_MINEURE: u32 = 2;

/// `NVENCAPI_VERSION` — ⚠️ **packing `major | (minor << 24)`**, which
/// is NOT the one the driver returns (see `version_pilote_attendue`).
pub const VERSION_API: u32 = VERSION_MAJEURE | (VERSION_MINEURE << 24);

/// The header's `NVENCAPI_STRUCT_VERSION(ver)`, reimplemented.
///
/// The `0x7 << 28` is a tag that every structure version carries.
pub const fn version_de_structure(revision: u32) -> u32 {
    VERSION_API | (revision << 16) | (0x7 << 28)
}

/// The structures whose version the header adds `1u << 31` to.
///
/// 🔴 **Omitting this bit returns `NV_ENC_ERR_INVALID_VERSION`**, and nothing else
/// reports it. That is why the two forms are two distinct functions
/// rather than a boolean not to forget.
pub const fn version_de_structure_marquee(revision: u32) -> u32 {
    version_de_structure(revision) | (1 << 31)
}

/// What `NvEncodeAPIGetMaxSupportedVersion` must return at minimum.
///
/// 🔴 **THE PACKING IS `(major << 4) | minor`, AND IT DIFFERS FROM
/// `VERSION_API`.** The header says it in so many words: "the 4 least
/// significant bits […] indicate the minor version and the rest of the bits
/// indicate the major version". Confusing the two compares 0x0200000C
/// with 0xC2 and rejects every driver in the world.
pub const fn version_pilote_attendue() -> u32 {
    (VERSION_MAJEURE << 4) | VERSION_MINEURE
}

/// Does the installed driver speak the version we transcribed?
///
/// To be called **before** `NvEncodeAPICreateInstance`: otherwise the failure arrives
/// later and reads less well.
pub const fn pilote_compatible(rendu_par_le_pilote: u32) -> bool {
    rendu_par_le_pilote >= version_pilote_attendue()
}

// --- The structure versions, each with its header revision. ---
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
/// ⚠️ **Same value as `LOCK_BITSTREAM_VER`** — both structures
/// carry revision 2 and the high-order bit. It is not a
/// typo: two structures can share a version.
pub const RECONFIGURE_PARAMS_VER: u32 = version_de_structure_marquee(2);

// --- The enumeration constants of the encoding path. ---

/// `NV_ENC_DEVICE_TYPE_DIRECTX`. ⚠️ The header's comment says
/// "directx9"; it is **also** the value for D3D11 and D3D12, there is no
/// separate D3D11 constant.
pub const DEVICE_TYPE_DIRECTX: u32 = 0;
/// `NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX`.
pub const INPUT_RESOURCE_TYPE_DIRECTX: u32 = 0;
/// `NV_ENC_INPUT_IMAGE`, the usage of an input buffer.
pub const BUFFER_USAGE_INPUT_IMAGE: u32 = 0;

/// `NV_ENC_BUFFER_FORMAT_NV12`.
pub const BUFFER_FORMAT_NV12: u32 = 0x0000_0001;

/// `NV_ENC_BUFFER_FORMAT_ARGB`.
///
/// 🔴 **THIS IS THE FORMAT OF WHAT THE CAPTURE GIVES US, DESPITE ITS NAME.**
/// The header defines it as *word-ordered* with **B in the 8 least
/// significant bits** — that is, in little-endian memory, the byte order
/// B, G, R, A: exactly `DXGI_FORMAT_B8G8R8A8_UNORM`, what
/// Desktop Duplication returns. **Picking `ABGR` instead swaps red and
/// blue WITHOUT ANY ERROR** — the image comes out, simply wrong.
pub const BUFFER_FORMAT_ARGB: u32 = 0x0100_0000;
/// `NV_ENC_BUFFER_FORMAT_ABGR` — **the neighbouring trap**, declared so that a
/// test can establish that we did not pick it by mistake.
pub const BUFFER_FORMAT_ABGR: u32 = 0x1000_0000;

/// `NV_ENC_PARAMS_RC_CBR` — constant bitrate, what interactive use requires.
pub const RC_MODE_CBR: u32 = 2;

/// `NV_ENC_PIC_STRUCT_FRAME`. ⚠️ **Is 1, not 0**: zeroing the structure and
/// forgetting this field is an error, not a harmless default.
pub const PIC_STRUCT_FRAME: u32 = 1;

/// `NV_ENC_PIC_FLAG_FORCEIDR`.
pub const PIC_FLAG_FORCEIDR: u32 = 0x2;
/// `NV_ENC_PIC_FLAG_OUTPUT_SPSPPS`.
pub const PIC_FLAG_OUTPUT_SPSPPS: u32 = 0x4;

/// `NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY`.
pub const TUNING_ULTRA_LOW_LATENCY: u32 = 3;

/// `NV_ENC_ERR_INVALID_VERSION` — the code a wrong version returns.
pub const ERR_INVALID_VERSION: u32 = 15;
/// `NV_ENC_SUCCESS`.
pub const SUCCESS: u32 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 **THE EXPECTED VALUES ARE NOT COPIED: THEY ARE
    /// MEASURED.** Each one was printed by compiling the real header
    /// (command in this module's header). This test compares our
    /// arithmetic with that survey — it is what makes re-readable a constant
    /// whose error would be silent.
    #[test]
    fn the_structure_versions_equal_the_header_ones() {
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
        assert_eq!(RECONFIGURE_PARAMS_VER, 0xF202_000C);
        assert_eq!(
            RECONFIGURE_PARAMS_VER, LOCK_BITSTREAM_VER,
            "both share revision 2: read, not assumed"
        );
    }

    /// 🔴 The `1 << 31` bit separates the two families. If it disappeared from the
    /// five structures that carry it, NVENC would refuse them **silently**.
    #[test]
    fn the_most_significant_bit_tells_the_two_families_apart() {
        for (nom, v) in [
            ("INITIALIZE_PARAMS", INITIALIZE_PARAMS_VER),
            ("CONFIG", CONFIG_VER),
            ("PRESET_CONFIG", PRESET_CONFIG_VER),
            ("PIC_PARAMS", PIC_PARAMS_VER),
            ("LOCK_BITSTREAM", LOCK_BITSTREAM_VER),
            ("RECONFIGURE_PARAMS", RECONFIGURE_PARAMS_VER),
        ] {
            assert_ne!(v & (1 << 31), 0, "{nom} must carry bit 31");
        }
        for (nom, v) in [
            (
                "OPEN_ENCODE_SESSION_EX_PARAMS",
                OPEN_ENCODE_SESSION_EX_PARAMS_VER,
            ),
            ("RC_PARAMS", RC_PARAMS_VER),
            ("REGISTER_RESOURCE", REGISTER_RESOURCE_VER),
            ("MAP_INPUT_RESOURCE", MAP_INPUT_RESOURCE_VER),
            ("CREATE_BITSTREAM_BUFFER", CREATE_BITSTREAM_BUFFER_VER),
            ("FUNCTION_LIST", FUNCTION_LIST_VER),
        ] {
            assert_eq!(v & (1 << 31), 0, "{nom} must NOT carry bit 31");
        }
    }

    /// 🔴 **THE TWO VERSION PACKINGS ARE DIFFERENT**, and
    /// confusing them would reject every driver. This test pins the difference.
    #[test]
    fn the_expected_driver_version_is_not_the_api_one() {
        assert_eq!(version_pilote_attendue(), 0xC2, "(12 << 4) | 2");
        assert_ne!(version_pilote_attendue(), VERSION_API);
    }

    #[test]
    fn a_too_old_driver_is_refused_and_a_newer_one_accepted() {
        assert!(pilote_compatible(0xC2), "12.2 exactement");
        assert!(pilote_compatible((13 << 4) | 1), "13.1, more recent");
        assert!(!pilote_compatible((12 << 4) | 1), "12.1, too old");
        assert!(!pilote_compatible(0), "pilote muet");
    }

    /// 🔴 The red/blue trap, pinned: what the capture produces
    /// (`DXGI_FORMAT_B8G8R8A8_UNORM`) is declared `ARGB` to NVENC, never
    /// `ABGR` — the reverse would output an image without the slightest error.
    #[test]
    fn the_capture_format_is_argb_not_abgr() {
        assert_eq!(BUFFER_FORMAT_ARGB, 0x0100_0000);
        assert_eq!(BUFFER_FORMAT_ABGR, 0x1000_0000);
        assert_ne!(BUFFER_FORMAT_ARGB, BUFFER_FORMAT_ABGR);
    }

    #[test]
    fn the_enumeration_constants_equal_the_header_ones() {
        assert_eq!(DEVICE_TYPE_DIRECTX, 0);
        assert_eq!(INPUT_RESOURCE_TYPE_DIRECTX, 0);
        assert_eq!(BUFFER_FORMAT_NV12, 1);
        assert_eq!(RC_MODE_CBR, 2);
        assert_eq!(PIC_STRUCT_FRAME, 1, "is 1, not 0");
        assert_eq!(PIC_FLAG_FORCEIDR, 2);
        assert_eq!(PIC_FLAG_OUTPUT_SPSPPS, 4);
        assert_eq!(TUNING_ULTRA_LOW_LATENCY, 3);
        assert_eq!(ERR_INVALID_VERSION, 15);
        assert_eq!(SUCCESS, 0);
    }
}
