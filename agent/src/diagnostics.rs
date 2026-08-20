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
// `pub(crate)` et non privé : `moniteurs_virtuels::purge` (production, promue
// hors de cet arbre) lit encore `multifenetre::montee` pour la topologie DXGI
// et le plafond de recherche — seul lien restant dans ce sens, assumé, voir
// le commentaire de `moniteurs_virtuels/purge.rs`.
#[cfg(windows)]
pub(crate) mod multifenetre;
#[cfg(windows)]
pub(crate) mod pixels;
#[cfg(windows)]
mod presse_papier;

/// Renvoie `true` si une sonde a tourné — `main` doit alors s'arrêter là.
///
/// L'ordre des sondes reproduit exactement celui qu'avait `main()`
/// avant le découpage (à l'exception de `PROCESS_LOOPBACK_CAPTURE`, ajoutée
/// par le sous-bloc D7 directement avant sa voisine `PROCESS_LOOPBACK_PROBE`,
/// les deux variables partageant un préfixe), et il n'est pas indifférent :
/// `INPUT_LINEARITY_PROBE`
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
    // spécification du chantier A — quel est le périphérique de rendu RETENU
    // pour CETTE session, quel est son format de mixage, et un loopback y
    // capte-t-il bien ce que jouent les applications.
    //
    // ⚠️ « Retenu », plus « par défaut », depuis la correction « A-bis » : la
    // sonde passe par `LoopbackCapture::open`, donc par `AUDIO_PERIPHERIQUE`
    // quand elle est posée. Elle rend en outre la FRÉQUENCE DOMINANTE de ce
    // qu'elle capte, et pas seulement une crête — c'est ce qui en fait
    // l'instrument de mesure d'A-bis, lancée deux fois, avec et sans la
    // variable, sur la même machine.
    #[cfg(windows)]
    if std::env::var("AUDIO_PROBE").is_ok() {
        audio::executer_sonde_audio()?;
        return Ok(true);
    }

    // Mesure pivot du sous-bloc D7 : `PROCESS_LOOPBACK_CAPTURE=<pid>` va
    // jusqu'où `PROCESS_LOOPBACK_PROBE` (juste en dessous) s'arrête —
    // `Initialize`, `GetService`, `Start`, et une lecture réelle. Placée
    // AVANT ce bras : les deux variables partagent un préfixe
    // (`PROCESS_LOOPBACK_`), le piège exact de `MULTIFENETRE_NVENC_CYCLES` en
    // D5, où la variable la plus spécifique doit être testée en premier.
    #[cfg(windows)]
    if let Ok(pid_texte) = std::env::var("PROCESS_LOOPBACK_CAPTURE") {
        audio::executer_capture_process_loopback(&pid_texte)?;
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

    // Sonde P0 du sous-bloc P1 (presse-papier) : `PRESSE_PAPIER_SONDE=<secondes>`
    // mesure ce que la spécification (§8) déclare NON MESURÉ — le compteur
    // `GetClipboardSequenceNumber` existe-t-il, est-il stable au repos, bouge-t-il
    // sur une copie, et bouge-t-il sur une RÉÉCRITURE IDENTIQUE. C'est une porte
    // éliminatoire : trois de ses cinq verdicts rendent le mécanisme de détection
    // retenu non livrable en l'état.
    //
    // ⚠️ Cette sonde ÉCRIT le presse-papier de la VM (phases C et D) et le détruit
    // donc. Le produit, lui, ne l'écrit jamais en P1.
    //
    // 🔴 **Cette variable ne doit JAMAIS coexister avec `SUPERVISEUR`** — c'est la
    // divergence E10 du plan. `main()` appelle `diagnostics::aiguiller()` en
    // `main.rs:172`, AVANT la branche `CAPTEUR` (`:180`), avant l'enrôlement,
    // avant `PONT` (`:279`) et avant la branche superviseur : **quel que soit le
    // mode demandé**, un agent qui porte cette variable exécute la sonde et
    // s'arrête.
    //
    // ❌ **LE MÉCANISME QUE E10 DÉCRIT N'EST PAS ATTEIGNABLE, et l'écrire ici
    // vaut mieux que de laisser courir une menace fausse** (tâche 14, 20 août
    // 2026). E10 annonce que la sonde serait exécutée « par le processus
    // CAPTEUR, qui s'arrêterait aussitôt — et le superviseur le relancerait en
    // boucle ». Cela supposerait que l'ENFANT porte la variable et pas son PÈRE.
    // Or `superviseur/lanceur.rs` lance ses enfants par `std::process::Command`,
    // qui hérite de l'environnement du père : si le capteur la porte, le
    // superviseur la portait déjà — et il s'est donc arrêté à `main.rs:172`,
    // AVANT d'avoir lancé quoi que ce soit. Il n'existe aucun chemin, dans ce
    // dépôt, qui pose cette variable sur un enfant sans l'avoir posée sur son
    // père : `run-agent.sh` écrit un unique script d'amorçage, et `lanceur.rs`
    // n'ajoute jamais de variable de diagnostic.
    //
    // ✅ **CE QUI EST VRAI, ET SUFFIT À JUSTIFIER LA MÊME CONSIGNE** : la
    // variable dégénère TOUT lancement d'agent en sonde, superviseur compris.
    // Le symptôme n'est pas une boucle mais un silence — aucune session ne
    // s'établit, et la sortie de la sonde est le seul indice. **La sonde se
    // lance SEULE**, sans `SUPERVISEUR`.
    //
    // ⚠️ Le plan ne pose PAS d'`env_remove` pour cette variable, et sa raison
    // (« ce serait une convention que les huit `MULTIFENETRE_*` ne suivent
    // pas ») est faible au regard de la doctrine que `lanceur.rs` porte dans son
    // propre code — « un ordre de test est une propriété qui change, un
    // `env_remove` non », écrite le 20 août 2026 en retirant l'identité de
    // plateforme aux enfants. **Mais la conclusion tient pour une AUTRE raison,
    // et elle est décisive** : le père s'arrête avant d'atteindre `lanceur.rs`,
    // donc un `env_remove` posé là ne préviendrait RIEN. Le seul remède qui
    // mordrait serait de déplacer l'aiguillage des sondes après les branches de
    // mode, ce qui changerait le contrat de `diagnostics::aiguiller` pour ses
    // douze variables — hors périmètre de P1, et nommé ici plutôt que dormant.
    #[cfg(windows)]
    if let Ok(secondes) = std::env::var("PRESSE_PAPIER_SONDE") {
        presse_papier::executer(&secondes)?;
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
