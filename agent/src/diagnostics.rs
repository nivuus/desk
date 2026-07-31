//! Aiguillage des modes diagnostic, tous activés par variable
//! d'environnement. Aucun n'ouvre de session WebRTC : ils observent et
//! consignent, ils ne construisent rien.
//!
//! « Observer » ne veut pas dire « sans effet » pour autant : deux sondes
//! perturbent délibérément l'état de la session Windows, parce que c'est le
//! seul moyen d'obtenir la mesure. `INPUT_LINEARITY_PROBE` injecte de vrais
//! déplacements de souris et repositionne le curseur (`SetCursorPos`), et
//! `CAPTURE_TEST` déplace la fenêtre observée d'un pixel en boucle
//! (`SetWindowPos`) pour forcer la recomposition du bureau. Ne pas les
//! lancer sur une session dont on veut préserver l'état.

use anyhow::Result;

#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod capture;
mod entree;
#[cfg(windows)]
pub(crate) mod exceptions;
#[cfg(windows)]
mod multifenetre;
#[cfg(windows)]
pub(crate) mod pixels;

/// Renvoie `true` si une sonde a tourné — `main` doit alors s'arrêter là.
///
/// L'ordre des cinq sondes reproduit exactement celui qu'avait `main()`
/// avant le découpage, et il n'est pas indifférent : `INPUT_LINEARITY_PROBE`
/// (la dernière) lit `INPUT_LINEARITY_NEUTRALISER`, dont l'effet dépend de la
/// neutralisation SPI que `main()` a déjà — ou non — appliquée avant
/// d'appeler cette fonction. Voir le commentaire de `main()` à ce sujet.
pub(crate) fn aiguiller() -> Result<bool> {
    // Mode diagnostic : CAPTURE_TEST=firefox vérifie le repérage et la capture.
    #[cfg(windows)]
    if let Ok(fragment) = std::env::var("CAPTURE_TEST") {
        capture::executer(&fragment)?;
        return Ok(true);
    }

    // Sonde audio (`AUDIO_PROBE=1`) : répond aux questions n°1 et n°2 de la
    // spécification du chantier A — quel est le périphérique de rendu par
    // défaut de CETTE session, quel est son format de mixage, et un loopback
    // y capte-t-il bien ce que jouent les applications.
    #[cfg(windows)]
    if std::env::var("AUDIO_PROBE").is_ok() {
        audio::executer_sonde_audio()?;
        return Ok(true);
    }

    // Sonde n°4 de la spec du chantier A : le *process loopback*
    // (Windows 10 build 19041+) isole l'audio d'un seul processus, ce
    // qu'exige le modèle multi-fenêtres du chantier D. La VM est en build
    // 20348, donc éligible sur le papier. RIEN N'EST CONSTRUIT DESSUS ici :
    // on observe seulement si l'activation réussit, et le résultat est
    // consigné pour le chantier D.
    #[cfg(windows)]
    if let Ok(pid_texte) = std::env::var("PROCESS_LOOPBACK_PROBE") {
        audio::executer_process_loopback(&pid_texte)?;
        return Ok(true);
    }

    // Sonde du chantier B (§11, inconnues n°1 et n°2) : ViGEmBus accepte-t-il
    // de brancher une manette Xbox 360 virtuelle, et son rappel de
    // notification restitue-t-il bien les magnitudes de vibration qu'un jeu
    // demande ? RIEN N'EST CONSTRUIT DESSUS ici : on observe, et le résultat
    // fige l'API réellement disponible pour la tâche 10.
    #[cfg(windows)]
    if std::env::var("VIGEM_PROBE").is_ok() {
        entree::executer_vigem()?;
        return Ok(true);
    }

    // Sonde n°1 de la recette du chantier B : la visée est-elle linéaire
    // 1:1 ? On injecte une somme connue de déplacements relatifs et on
    // compare au déplacement réel du curseur.
    //
    // `INPUT_LINEARITY_NEUTRALISER=0` saute la neutralisation SPI — ici ET
    // au tout début de `main()` (voir le commentaire là-bas) : c'est ce qui
    // rend la mesure démonstrative plutôt que rassurante — l'écart observé
    // sans neutralisation chiffre ce que la neutralisation apporte. Sauter
    // seulement l'appel ci-dessous, sans toucher à celui du démarrage,
    // aurait laissé ce dernier neutraliser la session avant même que la
    // sonde ne s'exécute (bogue réel de la première version de cette
    // tâche, corrigé en ronde de revue 1).
    #[cfg(windows)]
    if std::env::var("INPUT_LINEARITY_PROBE").is_ok() {
        entree::executer_linearite()?;
        return Ok(true);
    }

    // Sondes du chantier D (capture multi-fenêtres). Placées en dernier :
    // elles créent leurs propres fenêtres et n'interfèrent avec aucune des
    // sondes ci-dessus, mais elles perturbent la disposition du bureau.
    #[cfg(windows)]
    if multifenetre::aiguiller()? {
        return Ok(true);
    }

    Ok(false)
}
