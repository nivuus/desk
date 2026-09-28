//! Reading an HTTP/1.1 response: its status, its length, and where the body starts.
//!
//! 🔴 THIS MODULE IS PURE: no `#[cfg]`, no socket, no read. It
//! receives a `&[u8]` and returns a decision; `installation::telechargement`
//! holds the connection. That is what lets the parsing be judged on the Linux
//! host, where `execution.rs` has no possible test.
//!
//! ⚠️ WHY THIS MODULE RATHER THAN AN HTTP CRATE. The agent has **no** HTTP
//! client and `tokio-tungstenite` is locked without TLS there: `reqwest`
//! would bring an entire TLS stack and break the invariant "no
//! production dependency" that G1 and G2 both hold. This repository has already
//! written its TURN client, its STUN codec and its SHA-256 for the same reason.
//!
//! 🔴 "NOT COMPLETE YET" IS A STATE IN ITS OWN RIGHT, NOT A REFUSAL. The
//! buffer of a caller reading a socket carries a **cut** header one time
//! in two: a parser unable to say so would conclude on
//! `Content-Len` and return "length missing" on a perfectly
//! valid response that was four bytes short. The refusal would be loud,
//! typed, and **wrong**.
//!
//! 🔴 TYPED REFUSALS, NEVER INTERPRETATION. *A named refusal is diagnosed in
//! one log line; a parser that guesses is diagnosed in a
//! campaign.* This module reassembles no chunk, follows no
//! redirect and decompresses nothing: it refuses, saying what.

/// The header cap, in bytes.
///
/// ⚠️ **NOT CALIBRATED**, it joins the list kept since `BPP_MIN`. It
/// is not fine tuning: without it, a server that **never** sent
/// its `\r\n\r\n` would make the caller's buffer grow without end, and the
/// only symptom would be rising memory.
pub const ENTETE_MAX_OCTETS: usize = 16 * 1024;

/// The only transfer coding we can read: none.
const TRANSFERT_ACCEPTE: &str = "identity";

/// The two statuses that carry a body we can write: `200` the full
/// response, `206` the one to a `Range`. **Both travel**, the caller
/// doing two opposite things with them — a `200` received in reply to a `Range` makes it
/// **start over from zero**, never concatenate, which would produce a file
/// longer than its size and a wrong fingerprint.
const STATUTS_RETENUS: &[u16] = &[200, 206];

/// What prevents keeping a response.
///
/// ⚠️ EACH VARIANT CARRIES WHAT IS NEEDED TO DIAGNOSE IT WITHOUT REOPENING THE PRODUCT:
/// the refused status, the refused coding, the unreadable value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refus {
    /// The first line is not an HTTP/1.x status line.
    LigneDeStatut(String),
    /// A status other than `200` and `206` — it travels as is.
    Statut(u16),
    /// A `Transfer-Encoding` we cannot read; the observed value
    /// travels, so that the log **names `chunked`**.
    TransfertCode(String),
    /// No `Content-Length`. The service sets one; that an intermediary
    /// might remove it **has not been measured**.
    MissingLength,
    /// A `Content-Length` that is not a number.
    UnreadableLength(String),
    /// Two `Content-Length` that disagree — the classic request
    /// smuggling vector. Keeping one would mean picking at random the
    /// reading of one of the two intermediaries.
    ConflictingLength { premiere: u64, seconde: u64 },
    /// The header is not UTF-8 — hence not ASCII, which the RFC mandates.
    EnteteIllisible,
    /// The header exceeds `ENTETE_MAX_OCTETS` without ever ending.
    EnteteTropLongue(usize),
}

/// What a kept response tells the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entete {
    /// `200` or `206`, and the caller needs both: see
    /// `STATUTS_RETENUS`.
    pub statut: u16,
    /// ⚠️ ON A `206`, IT IS THE LENGTH OF THE RANGE, NOT THAT OF THE FILE.
    /// A caller taking it for the final size would declare the
    /// download finished at the first byte of the resumption.
    pub length: u64,
    /// The index, in the parsed buffer, of the **first byte of the body** —
    /// header and start of body arriving in the same read. Without it,
    /// the caller would redo the separator search, hence one day redo it
    /// differently.
    pub debut_du_corps: usize,
}

/// The state of a parse, refusals aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    /// The `\r\n\r\n` separator has not arrived yet. **This is not a
    /// refusal**: the socket must be read again and this called again.
    Incomplet,
    Prete(Entete),
}

/// The whole rule, on the buffer as it comes from the socket.
pub fn analyser(tampon: &[u8]) -> Result<Etat, Refus> {
    // 🔴 COMPLETENESS IS DECIDED BEFORE EVERYTHING ELSE, and it is the only order
    // that holds: looking for the separator without checking it is there would
    // conclude on a partial header.
    let Some(fin) = position_du_separateur(tampon) else {
        return if tampon.len() > ENTETE_MAX_OCTETS {
            Err(Refus::EnteteTropLongue(tampon.len()))
        } else {
            Ok(Etat::Incomplet)
        };
    };

    let texte = std::str::from_utf8(&tampon[..fin]).map_err(|_| Refus::EnteteIllisible)?;
    let mut lignes = texte.split("\r\n");

    let statut = statut_de(lignes.next().unwrap_or_default())?;
    if !STATUTS_RETENUS.contains(&statut) {
        return Err(Refus::Statut(statut));
    }

    let mut length: Option<u64> = None;
    let mut transfert: Option<String> = None;
    for ligne in lignes {
        // A line without a colon is not a field: the folded continuations
        // of RFC 7230 are deprecated, and none of the three fields
        // we read uses them.
        let Some((nom, value)) = ligne.split_once(':') else {
            continue;
        };
        let value = value.trim();
        // The RFC mandates case insensitivity on the NAME; the value of a
        // transfer coding is a token, so it is folded too.
        match nom.trim().to_ascii_lowercase().as_str() {
            "content-length" => {
                let lue = value
                    .parse::<u64>()
                    .map_err(|_| Refus::UnreadableLength(value.to_string()))?;
                match length {
                    Some(premiere) if premiere != lue => {
                        return Err(Refus::ConflictingLength {
                            premiere,
                            seconde: lue,
                        });
                    }
                    _ => length = Some(lue),
                }
            }
            "transfer-encoding" => transfert = Some(value.to_ascii_lowercase()),
            _ => {}
        }
    }

    // 🔴 THE TRANSFER CODING IS JUDGED BEFORE THE LENGTH, and it is the RFC that
    // mandates it: when both are present, `Transfer-Encoding` wins and the
    // `Content-Length` must be ignored. Judging the length first would return
    // "length missing" on a chunked response, that is the wrong
    // reason — the one that sends you looking for a faulty intermediary instead of the
    // coding we cannot read.
    if let Some(code) = transfert {
        if code != TRANSFERT_ACCEPTE {
            return Err(Refus::TransfertCode(code));
        }
    }

    Ok(Etat::Prete(Entete {
        statut,
        length: length.ok_or(Refus::MissingLength)?,
        debut_du_corps: fin + 4,
    }))
}

/// The index of the `\r\n\r\n`, if it has arrived.
fn position_du_separateur(tampon: &[u8]) -> Option<usize> {
    tampon.windows(4).position(|f| f == b"\r\n\r\n")
}

/// `HTTP/1.1 200 OK` → `200`.
fn statut_de(ligne: &str) -> Result<u16, Refus> {
    let mut morceaux = ligne.split(' ');
    let version = morceaux.next().unwrap_or_default();
    let code = morceaux.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") || code.len() != 3 {
        return Err(Refus::LigneDeStatut(ligne.to_string()));
    }
    code.parse::<u16>()
        .map_err(|_| Refus::LigneDeStatut(ligne.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lire(brut: &str) -> Result<Etat, Refus> {
        analyser(brut.as_bytes())
    }

    fn prete(brut: &str) -> Entete {
        match lire(brut) {
            Ok(Etat::Prete(entete)) => entete,
            autre => panic!("attendu une réponse retenue, obtenu {autre:?}"),
        }
    }

    #[test]
    fn a_complete_response_returns_its_status_its_length_and_the_start_of_the_body() {
        let brut = "HTTP/1.1 200 OK\r\nContent-Length: 42\r\n\r\nabc";
        let entete = prete(brut);
        assert_eq!((entete.statut, entete.length), (200, 42));
        // The body starts AFTER the separator, and the buffer already carries
        // three bytes of it: that is the nominal case of a socket read.
        assert_eq!(&brut.as_bytes()[entete.debut_du_corps..], b"abc");
    }

    /// 🔴 THE RED OF THIS TASK. The buffer stops in the middle of the header
    /// name; the parser must say "not yet", **never** conclude.
    #[test]
    fn un_entete_coupe_en_deux_lectures_dit_incomplet_et_ne_conclut_pas() {
        assert_eq!(lire("HTTP/1.1 200 OK\r\nContent-Len"), Ok(Etat::Incomplet));
        assert_eq!(lire("HTTP/1.1 200 OK\r\n"), Ok(Etat::Incomplet));
        assert_eq!(lire(""), Ok(Etat::Incomplet));
        // And the second read does conclude — otherwise "incomplete"
        // would be returned by an entirely dead parser.
        assert_eq!(
            prete("HTTP/1.1 200 OK\r\nContent-Length: 42\r\n\r\n").length,
            42
        );
    }

    #[test]
    fn le_206_est_retenu_et_son_statut_voyage_pour_que_l_appelant_le_distingue() {
        assert_eq!(
            prete("HTTP/1.1 206 Partial Content\r\nContent-Length: 7\r\n\r\n").statut,
            206
        );
    }

    #[test]
    fn an_unexpected_status_is_refused_carrying_its_number() {
        for code in [302u16, 404, 500] {
            let brut = format!("HTTP/1.1 {code} X\r\nContent-Length: 0\r\n\r\n");
            assert_eq!(lire(&brut), Err(Refus::Statut(code)));
        }
    }

    /// The coding is judged BEFORE the length: without `Content-Length`, a
    /// chunked response must denounce the coding, not the missing length —
    /// the second reason would send you looking for a faulty intermediary.
    #[test]
    fn chunked_is_refused_the_reason_names_it_and_it_wins_over_the_length() {
        for brut in [
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 9\r\n\r\n",
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
        ] {
            assert_eq!(lire(brut), Err(Refus::TransfertCode("chunked".into())));
        }
        let gzip = "HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n";
        assert_eq!(lire(gzip), Err(Refus::TransfertCode("gzip".into())));
        // `identity` encodes nothing: it passes, and case does not matter.
        let brut = "HTTP/1.1 200 OK\r\nTransfer-Encoding: IDENTITY\r\nContent-Length: 3\r\n\r\n";
        assert_eq!(prete(brut).length, 3);
    }

    #[test]
    fn a_missing_length_is_a_named_refusal_not_a_zero_length() {
        assert_eq!(
            lire("HTTP/1.1 200 OK\r\nServer: x\r\n\r\n"),
            Err(Refus::MissingLength)
        );
        // A zero length is a kept response, an unreadable length
        // carries what was read: three reasons, never confused.
        assert_eq!(
            prete("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").length,
            0
        );
        let flou = "HTTP/1.1 200 OK\r\nContent-Length: beaucoup\r\n\r\n";
        assert_eq!(lire(flou), Err(Refus::UnreadableLength("beaucoup".into())));
    }

    #[test]
    fn names_are_case_insensitive_and_values_trimmed() {
        for champ in [
            "CONTENT-LENGTH:   42  ",
            "content-length:42",
            "Content-Length:\t42",
        ] {
            assert_eq!(
                prete(&format!("HTTP/1.1 200 OK\r\n{champ}\r\n\r\n")).length,
                42
            );
        }
    }

    #[test]
    fn two_conflicting_lengths_are_refused_and_two_identical_ones_pass() {
        let deux =
            |a, b| format!("HTTP/1.1 200 OK\r\nContent-Length: {a}\r\nContent-Length: {b}\r\n\r\n");
        assert_eq!(
            lire(&deux(42, 9)),
            Err(Refus::ConflictingLength {
                premiere: 42,
                seconde: 9
            })
        );
        assert_eq!(prete(&deux(42, 42)).length, 42);
    }

    #[test]
    fn une_premiere_ligne_qui_n_est_pas_du_http_est_refusee_en_la_citant() {
        for ligne in ["BONJOUR", "HTTP/1.1 OK"] {
            let brut = format!("{ligne}\r\nContent-Length: 1\r\n\r\n");
            assert_eq!(lire(&brut), Err(Refus::LigneDeStatut(ligne.into())));
        }
    }

    /// A malformed header is refused, not guessed — and without a bound, the
    /// caller's buffer would grow without end.
    #[test]
    fn un_entete_sans_fin_est_borne_et_un_entete_non_ascii_est_refuse() {
        let trop = "HTTP/1.1 200 OK\r\n".to_string() + &"X: y\r\n".repeat(ENTETE_MAX_OCTETS / 4);
        assert!(matches!(lire(&trop), Err(Refus::EnteteTropLongue(_))));
        // Under the cap, the same unfinished header stays "incomplete".
        assert_eq!(lire("HTTP/1.1 200 OK\r\nX: y\r\n"), Ok(Etat::Incomplet));
        let brut = b"HTTP/1.1 200 OK\r\nX: \xff\r\nContent-Length: 1\r\n\r\n";
        assert_eq!(analyser(brut), Err(Refus::EnteteIllisible));
    }
}
