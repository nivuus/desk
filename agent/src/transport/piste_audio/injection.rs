//! L'injection de fautes de RECONSTRUCTION audio — variable de banc
//! `AUDIO_FAUTE_RECONSTRUCTION`, jamais une configuration livrée.
//!
//! Elle rend atteignable le repli sur la promotion d'une voisine, que le
//! sous-bloc D10 n'avait AUCUN moyen d'exercer : son seul déclencheur — tuer
//! l'arbre de processus cible — tue la fenêtre avant l'audio, la cible du
//! *process loopback* étant liée au PID propriétaire du HWND. C'est le leg 5
//! de D10.

use crate::audio::AudioSource;

/// Le budget global au processus de fautes de reconstruction injectées.
///
/// ⚠️ **Variable de BANC, jamais une configuration livrée** — même statut que
/// `PART_SONDAGE` (`transport/part.rs`). Absente : désarmée, et le binaire se
/// comporte exactement comme celui du sous-bloc D10.
///
/// **Global au processus, jamais par appel**, et c'est la leçon que D10 a
/// payée sur `AUDIO_FAUTE_LECTURE` à son deuxième passage de recette : un
/// budget relu à chaque tentative se réapprovisionne indéfiniment — chaque
/// capture reconstruite recevrait un budget neuf et remourrait —, et le
/// chiffre-juge qu'il sert devient structurellement incapable de quitter zéro.
/// Un `static` décrémenté par `fetch_update` s'épuise UNE FOIS pour tout le
/// processus.
///
/// `pub(super)` et non privé : les tests de la chaîne complète
/// « injection → refus → épuisement → `AudioMort` » vivent dans
/// `transport/tick/tests/audio.rs`, et **seedent l'atomique directement**
/// plutôt que l'environnement — un `OnceLock` déjà initialisé ne relirait de
/// toute façon plus `std::env`.
pub(in crate::transport) fn budget_faute_reconstruction() -> &'static std::sync::atomic::AtomicU32 {
    static BUDGET: std::sync::OnceLock<std::sync::atomic::AtomicU32> =
        std::sync::OnceLock::new();
    BUDGET.get_or_init(|| {
        let n: u32 = std::env::var("AUDIO_FAUTE_RECONSTRUCTION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if n > 0 {
            tracing::warn!(
                fautes_a_injecter = n,
                "injection de fautes de RECONSTRUCTION audio ARMEE : banc, jamais une configuration livrée"
            );
        }
        std::sync::atomic::AtomicU32::new(n)
    })
}

/// Interpose l'injection devant le reconstructeur réel.
///
/// `fetch_update` avec `checked_sub(1)` décrémente atomiquement SI le budget
/// global n'est pas déjà à zéro, et rend `Err` sans y toucher sinon : un
/// budget épuisé — le cas nominal, la variable étant absente — laisse donc
/// passer l'appel réel dès le premier tour, sans surcoût mesurable.
pub(super) fn intercepter<F>(reconstruire: F) -> anyhow::Result<Box<dyn AudioSource + Send>>
where
    F: FnOnce() -> anyhow::Result<Box<dyn AudioSource + Send>>,
{
    if budget_faute_reconstruction()
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_sub(1),
        )
        .is_ok()
    {
        Err(anyhow::anyhow!("faute injectée (AUDIO_FAUTE_RECONSTRUCTION)"))
    } else {
        reconstruire()
    }
}
