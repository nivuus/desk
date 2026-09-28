//! Where an agent process holds its platform identity from — the rule, PURE.
//!
//! 🔴 **A SINGLE `/agent` CHANNEL PER VM, AND THAT IS THE WHOLE SUBJECT OF THIS FILE.**
//! The platform keeps its table of reachable agents by `vm_id`
//! (`plateforme/src/agents/registre.ts`): at most ONE socket per VM, and
//! **the last registered wins, the old one being closed**. It is the right
//! rule — two sockets for the same VM would raise the question "which one receives
//! the launch order?", which the primary key of `agent_enrole`
//! already answers. What was wrong is that the agent presented several.
//!
//! **The defect, MEASURED on the VM on 20 August 2026 (one run, 64 s):**
//! `lancer_pont` removed `SUPERVISEUR`, `CAPTEUR`, `TEST_FILE` and
//! `WINDOW_TITLE` from the file bridge's environment, **but not `AGENT_VM`
//! nor `AGENT_SECRET`**. The bridge therefore enrolled under the same identity as its
//! father, each evicted the other, the evicted one reconnected immediately — the
//! exponential backoff restarts from zero after any successful enrolment —, and the cycle
//! had no end: **95 enrolments and 94 evictions in 64 s**, at ~1.5 Hz.
//! What it cost, noted in acceptance run G1: the complete catalogue resent at
//! each cycle, the incremental messages lost, the launch responses
//! lost (the `504 delai`), and **two discovery loops** instead of one.
//!
//! ⚠️ **THE BRIDGE WAS NOT THE ONLY ONE AT FAULT, and that is what the mapping
//! shows**: `lancer` (the window children) did not remove them either,
//! and a child also goes through the enrolment of `main.rs`. Acceptance run G1
//! had no window open, hence no child: the defect was
//! invisible there on that half, and it would have bitten at the first window.
//! The **sensor**, for its part, is not at fault — `main.rs` returns control to it BEFORE
//! enrolment — but its variables are removed anyway, by the
//! rule that `lanceur.rs` already imposes on itself for `SUPERVISEUR`, `CAPTEUR` and
//! `PONT`: *a test order is a property that changes, an `env_remove`
//! is not.*
//!
//! 🔴 **WHAT IS PASSED ON IS THE TOKEN, NEVER THE SECRET.** A child and the
//! bridge need an identity — they each open their own
//! `PeerConnection`, and the platform's guard refuses a handshake
//! without an agent token since sub-block P3 —, but they need
//! NO channel: they do not beat the VM's heart, push no
//! catalogue and receive no launch order. The supervisor therefore
//! passes them `AGENT_JETON`, and withholds `AGENT_VM`/`AGENT_SECRET`. The enrolment
//! secret no longer leaves the process that holds the channel.

/// Where this process gets its agent token from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceIdentite {
    /// `AGENT_JETON`: the identity comes from the father, which holds the channel. This
    /// process opens NO `/agent` channel.
    Heritee(String),
    /// `AGENT_VM` + `AGENT_SECRET`: this process enrols itself, and
    /// becomes the VM's reachable socket.
    Enrolement { vm: String, secret: String },
    /// Neither one nor the other: no token, hence no session will be established.
    /// It is not a fallback mode, it is an announced failure.
    Aucune,
}

/// Decides the identity source of this process.
///
/// 🔴 **THE INHERITED TOKEN WINS, EVEN IF THE ENROLMENT PAIR IS THERE**, and
/// this precedence is the heart of the remedy rather than a detail. `lanceur.rs`
/// removes `AGENT_VM` and `AGENT_SECRET` from any child, so the case should
/// not happen — but "should not" is exactly what was said about
/// inheritance itself before measuring it. If both arrive anyway,
/// through a path not foreseen, the precedence guarantees that the process
/// does NOT open a second channel: the defect would at worst become an
/// ageing identity, never an endless mutual eviction.
///
/// ⚠️ **AN INCOMPLETE PAIR RETURNS `Aucune`, never a half-enrolment.** It is
/// the contract `main.rs` already carried ("both or none"): presenting a
/// VM name without a secret can only be refused, and doing it anyway would
/// only produce an `enrolement` refusal indistinguishable from a wrong secret.
pub fn source(
    jeton_herite: Option<&str>,
    vm: Option<&str>,
    secret: Option<&str>,
) -> SourceIdentite {
    if let Some(jeton) = jeton_herite {
        return SourceIdentite::Heritee(jeton.to_string());
    }
    match (vm, secret) {
        (Some(vm), Some(secret)) => SourceIdentite::Enrolement {
            vm: vm.to_string(),
            secret: secret.to_string(),
        },
        _ => SourceIdentite::Aucune,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_jeton_herite_dispense_de_tout_enrolement() {
        assert_eq!(
            source(Some("jwt.h"), None, None),
            SourceIdentite::Heritee("jwt.h".into())
        );
    }

    /// 🔴 THE PRECEDENCE, AND IT IS WHAT HOLDS THE "A SINGLE CHANNEL
    /// PER VM" INVARIANT. Mutation that turns it red: test the enrolment pair
    /// first. A process receiving the three variables would then open its
    /// own channel, and we would fall back on the 95 enrolments / 94 evictions
    /// measured on 20 August 2026.
    #[test]
    fn le_jeton_herite_l_emporte_sur_un_couple_d_enrolement_present() {
        assert_eq!(
            source(Some("jwt.h"), Some("vm-1"), Some("chut")),
            SourceIdentite::Heritee("jwt.h".into())
        );
    }

    #[test]
    fn sans_jeton_le_couple_complet_fait_s_enroler() {
        assert_eq!(
            source(None, Some("vm-1"), Some("chut")),
            SourceIdentite::Enrolement {
                vm: "vm-1".into(),
                secret: "chut".into()
            }
        );
    }

    /// The two halves of the incomplete pair, and the empty case — one test per
    /// form, because an `assert!` that stops at the first failure would leave
    /// the following ones untested.
    #[test]
    fn un_couple_incomplet_ne_produit_aucune_identite() {
        assert_eq!(source(None, Some("vm-1"), None), SourceIdentite::Aucune);
        assert_eq!(source(None, None, Some("chut")), SourceIdentite::Aucune);
        assert_eq!(source(None, None, None), SourceIdentite::Aucune);
    }
}
