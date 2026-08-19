//! L'instrument de fréquence de la recette du micro : la fréquence dominante
//! d'un signal supposé périodique, par comptage des passages par zéro.
//!
//! **PUR, aucun `cfg`.** Extrait de `micro.rs` au titre de la règle des 500
//! lignes, et EXTRAIT PLUTÔT QUE COMPRIMÉ — le parent devait accueillir le
//! plafond de dissimulation (`micro/dissimulation.rs`) et n'avait plus la
//! marge. La doctrine du dépôt est de faire l'extraction AVANT l'addition, pas
//! après l'avoir franchie ; c'est ce qui est fait ici.
//!
//! Rien n'a changé de son contenu : il est déplacé mot pour mot, et
//! `micro.rs` le ré-exporte, de sorte qu'aucun site d'appel ne bouge.
//!
//! ⚠️ **Ce n'est PAS le seul instrument de fréquence du dépôt** : `spectre.rs`
//! en porte un autre, par filtre de Goertzel, qui cherche LA raie dominante
//! sur une grille connue d'avance. Les deux coexistent à dessein — celui-ci
//! ne suppose aucune grille et rend `None` sur ce qui n'est pas périodique,
//! ce qu'un Goertzel ne fait pas de lui-même.

/// Fraction de la crête sous laquelle un échantillon ne compte pas comme un
/// passage : la bande morte.
///
/// **Sans elle, l'instrument prendrait du bruit pour un ton.** Le bruit de
/// quantification autour de zéro multiplie les changements de signe, et c'est
/// exactement ce que sanctionne
/// `le_silence_et_le_bruit_ne_rendent_pas_une_frequence_credible`.
const BANDE_MORTE: f32 = 0.25;

/// Amplitude crête sous laquelle le signal n'a pas de fréquence du tout.
const CRETE_MINIMALE: f32 = 1.0 / 512.0;

/// Fréquence dominante d'un signal supposé PÉRIODIQUE, par passages par zéro.
///
/// ⚠️ **`pcm` est un signal MONO à `hz` échantillons par seconde.** Un tampon
/// stéréo entrelacé doit être désentrelacé par l'appelant (`step_by(2)`).
///
/// ❌ **CETTE DOC A PORTÉ UN FAUX, et c'est la MESURE qui l'a réfuté** (tâche 13,
/// chantier E). Elle disait que l'analyser tel quel « doublerait la cadence
/// apparente ». **C'est l'inverse : la fréquence est DIVISÉE PAR DEUX** —
/// relevé **219,5 Hz pour une tonalité de 440 Hz**, canaux identiques, en
/// retirant le `step_by(2)` de `demarrage::micro::Fenetre` et en relançant son
/// test. Le mécanisme est dans le calcul ci-dessous : `duree` vaut
/// `pcm.len() / hz`, et un tampon entrelacé porte deux fois plus de valeurs que
/// de trames — la durée calculée double, quand le nombre de passages par zéro
/// ne bouge pas (dupliquer chaque échantillon n'ajoute aucun changement de
/// signe). L'obligation de désentrelacer est INCHANGÉE ; seul le sens de
/// l'erreur qu'on commet en l'oubliant était faux, et un lecteur qui aurait
/// cherché un « x2 » dans un journal n'aurait rien trouvé. *(Le plan décrivait l'implémentation comme opérant « sur le canal
/// gauche » tout en écrivant ses tests sur un tampon mono : les deux ne peuvent
/// pas être vrais ensemble, et c'est la sémantique du test qui a été retenue,
/// parce que c'est elle qui rend la fonction utilisable des deux façons.)*
///
/// Rend `None` quand le signal est trop faible pour qu'un passage ait un sens :
/// **un signal trop faible n'a pas de fréquence**, et rendre un nombre pour le
/// silence ferait de cet instrument le compteur d'octets qu'il existe pour
/// remplacer (doctrine du dépôt, payée en D7).
pub fn frequence_par_passages_a_zero(pcm: &[f32], hz: u32) -> Option<f32> {
    if pcm.len() < 2 || hz == 0 {
        return None;
    }
    let crete = pcm.iter().fold(0.0f32, |m, e| m.max(e.abs()));
    if crete < CRETE_MINIMALE {
        return None;
    }
    let seuil = crete * BANDE_MORTE;

    let mut passages = 0u64;
    let mut signe: Option<bool> = None;
    for &e in pcm {
        if e.abs() <= seuil {
            continue;
        }
        let positif = e > 0.0;
        match signe {
            Some(precedent) if precedent != positif => {
                passages += 1;
                signe = Some(positif);
            }
            None => signe = Some(positif),
            _ => {}
        }
    }
    if passages == 0 {
        return None;
    }
    let duree = pcm.len() as f32 / hz as f32;
    Some(passages as f32 / (2.0 * duree))
}
