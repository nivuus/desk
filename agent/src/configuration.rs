//! La configuration de l'agent, lue depuis l'environnement.
//!
//! **Extrait de `main.rs` VERBATIM le 20 août 2026**, avant les additions du
//! sous-bloc P1 du presse-papier, parce que `main.rs` était à **513 lignes** —
//! au-dessus du plafond de 500 de `CLAUDE.md`, et absent de son tableau de
//! dette. Le commit `264c275` (« un seul canal `/agent` par VM ») l'avait porté
//! de 470 à 505 sans le déclarer, et la tâche 4 du presse-papier y avait ajouté
//! 8 lignes de plus. La règle du dépôt est d'**extraire avant d'ajouter**,
//! jamais de comprimer un commentaire pour repasser sous la ligne.
//!
//! Le découpage suit la responsabilité : `main.rs` ne garde que la déclaration
//! des modules et l'aiguillage des modes (capteur, pont, superviseur,
//! mono-fenêtre) ; la lecture et la validation de l'environnement vivent ici.
//! Aucun site d'appel n'a bougé — `Config` reste `crate::Config`, réexporté par
//! `main.rs`. Le seul changement au texte déplacé est la **visibilité** :
//! `pub(crate)` sur le type, ses champs et les deux fonctions que `main.rs`
//! appelle encore, sans quoi elles seraient invisibles depuis leur propre
//! crate.

use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};

// Importé plutôt que qualifié `crate::demarrage::…` sur son site d'appel : le
// texte déplacé reste ainsi VERBATIM, à la seule visibilité près.
use crate::demarrage;

/// Configuration de l'agent, lue depuis l'environnement.
pub(crate) struct Config {
    pub(crate) signaling_url: String,
    pub(crate) session_id: String,
    pub(crate) local_ip: IpAddr,
    pub(crate) test_file: Option<PathBuf>,
    /// Vrai en mode superviseur : ce processus ne capture rien, il détecte les
    /// fenêtres et lance un enfant par fenêtre.
    pub(crate) superviseur: bool,
    /// `HWND` de la fenêtre à capturer, en décimal ou hexadécimal préfixé
    /// `0x`. Posé par le superviseur sur ses enfants ; absent, l'agent
    /// retombe sur la recherche par titre (`WINDOW_TITLE`), c'est-à-dire sur
    /// le comportement mono-fenêtre d'avant ce sous-bloc.
    ///
    /// Les quatre champs qui suivent ne sont lus que par la branche Windows de
    /// `demarrage` : sur l'hôte Linux ils sont morts par construction, et
    /// l'`allow` le dit plutôt que de laisser un avertissement s'installer.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fenetre_hwnd: Option<u64>,
    /// Sortie DXGI à capturer, désignée par son nom (`\\.\DISPLAYn`). Absente,
    /// l'agent capture le bureau et recadre la fenêtre.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) sortie_dxgi: Option<String>,
    /// La taille RETENUE (`superviseur::placement::taille_retenue`) à
    /// laquelle le superviseur a posé cette fenêtre — posée par lui seul
    /// (`lanceur.rs`), jamais par un opérateur. Absente — le chemin
    /// mono-fenêtre, où `sortie_dxgi` l'est aussi —, la taille demandée au
    /// capteur à l'attache reste `(u32::MAX, u32::MAX)`
    /// (`capteur::tube::connecter`), et `taille_retenue` la ramène telle
    /// quelle à la taille de la sortie : le comportement d'avant ce
    /// sous-bloc, inchangé.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) taille_fenetre: Option<(u32, u32)>,
    /// Faux quand `AUDIO=0` coupe le son de cet agent.
    ///
    /// **Interrupteur GLOBAL, plus une consigne par fenêtre.** Jusqu'au
    /// sous-bloc D7, le superviseur posait `AUDIO=0` sur tous les enfants sauf
    /// un, parce qu'un unique loopback de session aurait été capté huit fois.
    /// Chaque enfant capte désormais le son de son PROPRE processus, et c'est
    /// le capteur qui arbitre entre les fenêtres d'un même processus : il n'y a
    /// plus rien à réserver.
    ///
    /// **Un agent lancé à la main coupe son son via `AUDIO=0
    /// scripts/run-agent.sh`**, qui transmet la variable en `$env:AUDIO` dans
    /// le script d'amorçage PowerShell (`schtasks` ne transmet pas
    /// l'environnement directement). Ce transport a manqué un temps : retirer
    /// le poseur du superviseur a d'abord laissé `AUDIO` sans aucun chemin
    /// vers l'enfant, `run-agent.sh` ne la portant pas — corrigé dans la
    /// même tâche 9, avant que ce commentaire ne soit lu par personne.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) audio: bool,
    /// Vrai quand `MICRO_MESURE=1` arme le puits de mesure du micro
    /// (chantier E, `demarrage/micro.rs`).
    ///
    /// ⚠️ **CONVENTION INVERSE de `AUDIO`, `SUPERVISEUR`, `PLEIN_ECRAN` et
    /// `CAPTEUR`, et à dessein** : on désarme sur `=0` ce qui est LIVRÉ, on
    /// **arme sur `=1`** ce qui ne l'est pas. Ce puits est un instrument de
    /// banc — il consomme le flux montant pour le journaliser, il ne le joue
    /// nulle part —, et une simple présence de la variable ne suffit donc pas :
    /// il faut la valeur `1`. C'est la convention qu'avait
    /// `PLEIN_ECRAN_MODE_SORTIE`, variable retirée par le sous-bloc D9 et qu'il
    /// est donc inutile de chercher dans le code.
    pub(crate) micro_mesure: bool,
    /// Le nom de la VM enrôlée auprès de la plateforme, et son secret
    /// d'enrôlement (sous-bloc P3). **Les deux ou aucun** : c'est le couple
    /// que le canal `/agent` présente.
    ///
    /// ⚠️ **ABSENTS = aucun jeton d'agent, donc AUCUNE session.** Depuis P3 la
    /// garde de la plateforme refuse un `{"role":"agent"}` anonyme, et il n'y
    /// a pas d'interrupteur permissif. Leur absence n'est donc PAS un mode de
    /// repli : c'est une panne, annoncée par un `warn!` qui la nomme, et non
    /// un échec de démarrage — les sondes de `diagnostics` (`MULTIFENETRE_*`)
    /// n'ouvrent aucun signaling et doivent continuer de tourner sans elles.
    pub(crate) agent_vm: Option<String>,
    pub(crate) agent_secret: Option<String>,
    /// Le préfixe de session délivré à l'enrôlement, et le jeton d'agent qui
    /// ouvre les deux poignées de main. Remplis par `main` APRÈS le
    /// démarrage, jamais par `config()` : les obtenir demande un socket.
    pub(crate) prefixe: String,
    pub(crate) jeton: Option<String>,
}

pub(crate) fn config() -> Result<Config> {
    Ok(Config {
        signaling_url: std::env::var("SIGNALING_URL")
            .unwrap_or_else(|_| "ws://127.0.0.1:8080".into()),
        session_id: std::env::var("SESSION_ID").unwrap_or_else(|_| "demo".into()),
        local_ip: std::env::var("LOCAL_IP")
            .unwrap_or_else(|_| "127.0.0.1".into())
            .parse()
            .context("LOCAL_IP n'est pas une adresse IP valide")?,
        test_file: std::env::var("TEST_FILE").ok().map(PathBuf::from),
        // `SUPERVISEUR=0` DÉSACTIVE le mode, comme `AUDIO=0` désactive le son.
        // Une simple présence (`is_ok()`) ferait qu'écrire `SUPERVISEUR=0`
        // pour le couper l'activerait — piège d'exploitation d'autant plus
        // sûr que la variable voisine, elle, se lit bien ainsi.
        superviseur: matches!(std::env::var("SUPERVISEUR").as_deref(), Ok(v) if v != "0"),
        // ABSENTE : mode mono-fenêtre légitime, aucun bruit. PRÉSENTE MAIS MAL
        // FORMÉE : échec du démarrage, jamais un repli muet — voir
        // `analyser_hwnd`.
        fenetre_hwnd: match std::env::var("FENETRE_HWND") {
            Ok(brut) => Some(analyser_hwnd(&brut)?),
            Err(_) => None,
        },
        // ABSENTE : mode mono-fenêtre légitime. PRÉSENTE MAIS VIDE : échec du
        // démarrage, jamais un repli muet — même règle que `FENETRE_HWND`.
        // Plus d'analyse `adaptateur:sortie` : c'est un NOM de sortie DXGI
        // (`\\.\DISPLAYn`), stable là où les index sont positionnels.
        sortie_dxgi: match std::env::var("SORTIE_DXGI") {
            Ok(brut) => {
                let nom = brut.trim().to_string();
                anyhow::ensure!(!nom.is_empty(), "SORTIE_DXGI est vide");
                Some(nom)
            }
            Err(_) => None,
        },
        // ABSENTE : mode mono-fenêtre légitime, aucun bruit — même cas que
        // `SORTIE_DXGI`. PRÉSENTE MAIS MAL FORMÉE : échec du démarrage, même
        // règle que `FENETRE_HWND` et `SORTIE_DXGI` — cette variable n'est
        // posée QUE par le superviseur (`lanceur.rs`, forme `LxH`), donc une
        // valeur illisible signale un bug du superviseur, pas une entrée
        // d'opérateur à tolérer en silence.
        taille_fenetre: match std::env::var("TAILLE_FENETRE") {
            Ok(brut) => {
                let (l, h) = brut
                    .split_once('x')
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : format attendu LxH"))?;
                let largeur: u32 = l
                    .parse()
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : largeur illisible"))?;
                let hauteur: u32 = h
                    .parse()
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : hauteur illisible"))?;
                Some((largeur, hauteur))
            }
            Err(_) => None,
        },
        // Le son est actif par défaut : un agent lancé à la main doit
        // retrouver ce comportement. `AUDIO=0` DÉSACTIVE, comme
        // `SUPERVISEUR=0` : une simple présence (`is_ok()`) activerait le son
        // en écrivant `AUDIO=0` pour le couper — même piège que documenté
        // au-dessus pour `SUPERVISEUR`. Depuis la tâche 9 du sous-bloc D7, le
        // superviseur ne pose plus cette variable sur ses enfants : chacun
        // capte le son de SON PROPRE processus (tâche 7), et c'est le capteur
        // qui arbitre entre les fenêtres qui en partagent un.
        audio: std::env::var("AUDIO").as_deref() != Ok("0"),
        // La décision vit dans `demarrage::micro::arme`, où un test la garde :
        // une variable posée à `0`, à vide, ou à quoi que ce soit d'autre
        // laisse le puits DÉSARMÉ, comme son absence.
        micro_mesure: demarrage::micro::arme(std::env::var("MICRO_MESURE").ok().as_deref()),
        // Une valeur vide vaut absence : `run-agent.sh` n'écrit la ligne que
        // si la variable est définie, mais un `AGENT_VM=` posé à la main
        // donnerait sinon un enrôlement au nom vide, que la plateforme
        // refuserait sans qu'on sache pourquoi.
        agent_vm: variable_non_vide("AGENT_VM"),
        agent_secret: variable_non_vide("AGENT_SECRET"),
        prefixe: String::new(),
        jeton: None,
    })
}

/// Lit une variable d'environnement, en traitant la chaîne vide comme une
/// absence.
pub(crate) fn variable_non_vide(nom: &str) -> Option<String> {
    let valeur = std::env::var(nom).ok()?;
    let valeur = valeur.trim().to_string();
    (!valeur.is_empty()).then_some(valeur)
}

/// Analyse un `HWND` tel que le superviseur le pose sur ses enfants :
/// hexadécimal préfixé `0x` (la forme que produit `lanceur.rs`), ou décimal.
///
/// **Échoue bruyamment plutôt que de rendre `None`**, et c'est le correctif I5
/// de la revue finale. La version précédente enchaînait `.ok().and_then(…)` :
/// toute valeur mal formée — un `0x` oublié, un espace, un débordement —
/// devenait indiscernable d'une variable absente, et `demarrage::source`
/// basculait alors sur la capture du bureau entier avec recadrage, sans un mot.
/// Un enfant du superviseur diffuserait ainsi le bureau de la VM en croyant
/// montrer sa fenêtre. Une variable ABSENTE garde son sens (mode mono-fenêtre,
/// recherche par titre) ; une variable PRÉSENTE doit être honorée ou refusée.
fn analyser_hwnd(brut: &str) -> Result<u64> {
    let texte = brut.trim();
    let valeur = match texte.strip_prefix("0x").or_else(|| texte.strip_prefix("0X")) {
        Some(hexa) => u64::from_str_radix(hexa, 16)
            .with_context(|| format!("FENETRE_HWND « {texte} » : hexadécimal illisible"))?,
        None => texte
            .parse()
            .with_context(|| format!("FENETRE_HWND « {texte} » : décimal illisible"))?,
    };
    anyhow::ensure!(valeur != 0, "FENETRE_HWND vaut 0 : aucune fenêtre ne porte ce handle");
    Ok(valeur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_hwnd_hexadecimal_ou_decimal_est_accepte() {
        assert_eq!(analyser_hwnd("0x1a2b").unwrap(), 0x1a2b);
        assert_eq!(analyser_hwnd(" 0x1A2B ").unwrap(), 0x1a2b);
        assert_eq!(analyser_hwnd("6699").unwrap(), 6699);
    }

    /// Le cœur de I5 : chacune de ces valeurs rendait `None` — donc « pas de
    /// fenêtre imposée », donc le repli silencieux sur la capture du bureau.
    #[test]
    fn un_hwnd_mal_forme_fait_echouer_le_demarrage() {
        assert!(analyser_hwnd("").is_err(), "vide");
        assert!(analyser_hwnd("0x").is_err(), "préfixe seul");
        assert!(analyser_hwnd("0xzz").is_err(), "pas de l'hexadécimal");
        assert!(analyser_hwnd("1a2b").is_err(), "hexadécimal sans préfixe");
        assert!(analyser_hwnd("-1").is_err(), "négatif");
        assert!(analyser_hwnd("0").is_err(), "handle nul");
    }
}
