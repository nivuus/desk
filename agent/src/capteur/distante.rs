//! `SourceDistante` — la source vidéo d'un enfant, alimentée par le capteur.
//!
//! **Pas de `#[cfg(windows)]`** : le tube réel est gaté (`capteur/tube.rs`),
//! mais la décision de clore ou non une session vit ici, et c'est la pièce la
//! plus coûteuse à se tromper. Elle est donc écrite contre un `Canal`
//! injecté et un `Receiver`, tous deux triviaux à simuler sur l'hôte.

use std::sync::mpsc::Receiver;

use anyhow::{bail, Result};

use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::capteur::reprise::FenetreCanal;
use crate::h264::AccessUnit;

/// Ce que l'enfant peut demander au capteur, et le moyen de s'y rattacher
/// quand le canal se rompt.
pub trait Canal {
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur>;
    /// Rouvre un canal vers le capteur et s'y réattache. L'implémentation
    /// remplace son propre état interne d'écriture ; elle rend la file
    /// d'images neuve et les dimensions annoncées à l'attache.
    fn rattacher(&mut self) -> Result<Rattachee>;
}

/// Le fruit d'un rattachement réussi.
pub struct Rattachee {
    pub images: Receiver<Recu>,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Ce que le capteur pousse, non sollicité.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recu {
    Image(AccessUnit),
    Etat { vivante: bool, epuisee: bool, largeur: u32, hauteur: u32 },
    /// Changement de sommeil poussé par le capteur, non sollicité. Retenu par
    /// `SourceDistante::sommeil` jusqu'à ce que `sommeil_a_annoncer` le
    /// consomme.
    Sommeil { endormie: bool, raison: String },
    /// Part du budget de débit accordée par le capteur, poussée non
    /// sollicitée. Retenue par `SourceDistante::part` jusqu'à ce que
    /// `part_a_appliquer` la consomme.
    Part { bps: u32 },
    /// Ordre audio poussé par le capteur, non sollicité. Retenu par
    /// `SourceDistante::audio` jusqu'à ce que `audio_a_appliquer` le consomme.
    Audio { actif: bool },
    /// Changement de plein écran poussé par le capteur, non sollicité. Retenu
    /// par `SourceDistante::plein_ecran` jusqu'à ce que
    /// `plein_ecran_a_annoncer` le consomme.
    PleinEcran { actif: bool },
    /// Le presse-papier de la VM a changé (sous-bloc P1). Poussé non
    /// sollicité, **au changement seulement** : c'est le capteur qui détient
    /// le presse-papier et qui sonde son numéro de séquence.
    ///
    /// `texte` vaut `None` sur un REFUS de taille (au-delà de
    /// `presse_papier::PRESSE_PAPIER_MAX`) — le contenu est refusé, jamais
    /// tronqué —, et `octets` porte alors la taille refusée, pour que le
    /// bandeau du navigateur puisse la dire. Retenu par
    /// `SourceDistante::presse_papier` jusqu'à ce que
    /// `presse_papier_a_annoncer` le consomme.
    PressePapier { texte: Option<String>, octets: u32 },
    /// La couleur d'accent de la fenêtre Windows, poussée par le capteur au
    /// changement — **première lecture comprise**. Retenue dans
    /// `SourceDistante::accent` jusqu'à ce que `accent_a_annoncer` la consomme.
    Accent { couleur: String },
}

pub struct SourceDistante {
    canal: Box<dyn Canal + Send>,
    images: Receiver<Recu>,
    largeur: u32,
    hauteur: u32,
    vivante: bool,
    epuisee: bool,
    /// Rupture du canal en cours. Une rupture n'épuise pas la source tant que
    /// cette fenêtre n'a pas expiré : c'est ce qui fait survivre les sessions
    /// à une relance du capteur.
    fenetre: FenetreCanal,
    /// Dernier changement de sommeil reçu du capteur, en attente d'être
    /// annoncé au navigateur. Consommé par `sommeil_a_annoncer`.
    ///
    /// **État courant, pas un historique** : deux `Sommeil` reçus avant
    /// qu'une lecture n'intervienne s'écrasent, seul le dernier survit — même
    /// régime que `Etat` juste au-dessus, dont les champs s'écrasent aussi
    /// sans accumulation.
    sommeil: Option<(bool, String)>,
    /// Dernière part de budget de débit reçue du capteur, en attente
    /// d'application. Consommée par `part_a_appliquer`. Même régime
    /// d'écrasement que `sommeil`.
    part: Option<u32>,
    /// Dernier ordre audio reçu du capteur, en attente d'application. Consommé
    /// par `audio_a_appliquer`. Même régime d'écrasement que `part`.
    ///
    /// ⚠️ **`None` à la naissance, et ce n'est pas « pas d'ordre » mais « rien
    /// à changer »** : l'enfant naît MUET (voir `demarrage.rs`), et le capteur
    /// lui envoie son premier ordre dès l'attache. Partir d'un `Some(true)`
    /// implicite ferait porter le son aux deux fenêtres d'un même processus
    /// pendant les millisecondes qui précèdent le premier arbitrage.
    audio: Option<bool>,
    /// Dernier changement de plein écran reçu du capteur, en attente d'être
    /// annoncé au navigateur. Consommé par `plein_ecran_a_annoncer`.
    ///
    /// **Même régime d'écrasement que `sommeil`** : deux changements arrivés
    /// entre deux lectures s'écrasent, seul le dernier survit. Il n'y a pas
    /// d'état courant jumeau ici, contrairement à `sommeil`/`endormie` : rien
    /// dans l'enfant n'a besoin de relire le plein écran hors de l'annonce.
    plein_ecran: Option<bool>,
    /// Dernier presse-papier reçu du capteur, en attente d'être annoncé au
    /// navigateur. Consommé par `presse_papier_a_annoncer`.
    ///
    /// **Même régime d'écrasement que `plein_ecran`** : deux copies arrivées
    /// entre deux lectures s'écrasent, seule la dernière survit. C'est correct
    /// — le presse-papier EST un état, pas un historique, et le navigateur
    /// n'aurait rien à faire d'une copie que l'utilisateur a déjà remplacée.
    presse_papier: Option<(Option<String>, u32)>,
    /// Dernière couleur d'accent reçue du capteur, en attente d'être annoncée
    /// au navigateur. Consommée par `accent_a_annoncer`.
    ///
    /// **Même régime d'écrasement que `presse_papier`** : deux changements
    /// arrivés entre deux lectures s'écrasent, seul le dernier survit. C'est
    /// correct — l'accent EST un état, pas un historique, et le navigateur
    /// n'aurait rien à faire d'une teinte que l'icône a déjà remplacée.
    accent: Option<String>,
    /// État de sommeil COURANT, tel que le capteur le décrit.
    ///
    /// **Distinct de `sommeil` juste au-dessus, et non redondant avec lui** :
    /// celui-là est l'annonce à faire au navigateur, rendue une seule fois ;
    /// celui-ci est l'état, relu à chaque part appliquée par
    /// `Session::appliquer_part` — qui ne doit pas propager le plancher d'une
    /// endormie au contrôleur de congestion. Les deux se posent au même
    /// endroit, sur le même message ; seule leur durée de vie diffère.
    ///
    /// ⚠️ **Vrai à la naissance, et ce n'est pas un choix prudent mais un
    /// fait** : depuis le sous-bloc D5 une fenêtre naît ENDORMIE côté capteur
    /// (`Fenetre::ouvrir` ne construit plus de `WindowsSource`, voir sa doc),
    /// et aucun `Sommeil { endormie: true }` n'est jamais poussé pour cette
    /// naissance — il n'y a pas de transition à annoncer. La toute première
    /// part reçue, envoyée par `sommeil::inscrire` dès l'attache, est donc le
    /// plancher `PART_DORMANTE_BPS`. Partir de `false` la ferait appliquer
    /// comme plafond d'encodage, précisément le défaut que ce champ existe
    /// pour éviter.
    endormie: bool,
    /// Vrai une seule fois, juste après un rattachement réussi. Consommé par
    /// `rattachement_survenu`, sur le même régime que `sommeil`/`part`/
    /// `audio`/`plein_ecran` : c'est ce qui permet à `Session` de remettre à
    /// zéro `audio_mort_signale` — un capteur relancé a perdu la mémoire de
    /// tout signalement antérieur.
    rattache: bool,
}

impl SourceDistante {
    pub fn nouvelle(
        canal: Box<dyn Canal + Send>,
        images: Receiver<Recu>,
        largeur: u32,
        hauteur: u32,
    ) -> Self {
        Self {
            canal,
            images,
            largeur,
            hauteur,
            vivante: true,
            epuisee: false,
            fenetre: FenetreCanal::nouvelle(),
            sommeil: None,
            part: None,
            audio: None,
            plein_ecran: None,
            presse_papier: None,
            accent: None,
            endormie: true,
            rattache: false,
        }
    }

    /// Émet une commande et n'accepte que `Fait` comme succès.
    fn commander_simple(&mut self, message: VersCapteur) -> Result<()> {
        match self.canal.commander(message)? {
            DepuisCapteur::Fait => Ok(()),
            DepuisCapteur::Erreur { motif } => bail!("le capteur a refusé : {motif}"),
            autre => bail!("réponse inattendue du capteur : {autre:?}"),
        }
    }

    /// Fait vieillir la fenêtre de reprise, pour les seuls tests : sans elle,
    /// éprouver l'expiration exigerait d'attendre réellement 15 secondes.
    #[cfg(test)]
    pub fn vieillir_pour_test(&mut self, ecart: std::time::Duration) {
        self.fenetre.vieillir_pour_test(ecart);
    }
}

// L'implémentation de `VideoSource` vit dans un fichier voisin : ce fichier-ci
// était à 472 lignes pour un plafond de projet à 500, et le sous-bloc A1 devait
// y greffer le patron `PressePapier`, qui pèse une trentaine de lignes.
// L'extraction est jouée AVANT l'addition qui la rend nécessaire, jamais après
// — et jamais par une compression. Voir l'en-tête de `distante/video_source.rs`.
#[path = "distante/video_source.rs"]
mod video_source;

// Les tests vivent dans un fichier voisin : ce fichier-ci était à 487 lignes
// pour un plafond de projet à 500. Voir l'en-tête de `distante/tests.rs`.
#[cfg(test)]
mod tests;

// Les tests des ÉTATS poussés par le capteur (visibilité, sommeil, part,
// audio, plein écran, presse-papier) vivent dans un TROISIÈME fichier :
// `distante/tests.rs` était à 474 lignes pour ce même plafond de 500, et le
// test du presse-papier (sous-bloc P1, tâche 10) en aurait entamé la marge.
// Voir l'en-tête de `distante/tests_etats.rs`.
#[cfg(test)]
#[path = "distante/tests_etats.rs"]
mod tests_etats;
