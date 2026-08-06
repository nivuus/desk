//! `SourceDistante` — la source vidéo d'un enfant, alimentée par le capteur.
//!
//! **Pas de `#[cfg(windows)]`** : le tube réel est gaté (`capteur/tube.rs`),
//! mais la décision de clore ou non une session vit ici, et c'est la pièce la
//! plus coûteuse à se tromper. Elle est donc écrite contre un `Canal`
//! injecté et un `Receiver`, tous deux triviaux à simuler sur l'hôte.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use anyhow::{bail, Result};

use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::capteur::reprise::FenetreCanal;
use crate::h264::AccessUnit;
use crate::source::VideoSource;

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
            endormie: true,
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

impl VideoSource for SourceDistante {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        loop {
            match self.images.try_recv() {
                Ok(Recu::Image(unite)) => {
                    self.fenetre.succes();
                    return Some(unite);
                }
                Ok(Recu::Etat { vivante, epuisee, largeur, hauteur }) => {
                    self.vivante = vivante;
                    self.epuisee = epuisee;
                    self.largeur = largeur;
                    self.hauteur = hauteur;
                }
                Ok(Recu::Sommeil { endormie, raison }) => {
                    // Deux écritures, deux durées de vie : l'état courant, qui
                    // survit à sa lecture, et l'annonce, qui ne s'y survit pas.
                    self.endormie = endormie;
                    self.sommeil = Some((endormie, raison));
                }
                Ok(Recu::Part { bps }) => {
                    self.part = Some(bps);
                }
                Ok(Recu::Audio { actif }) => {
                    self.audio = Some(actif);
                }
                Ok(Recu::PleinEcran { actif }) => {
                    self.plein_ecran = Some(actif);
                }
                // Le cas COURANT et normal : rien de neuf ce tour-ci. La
                // boucle de transport interroge à 100 Hz une source qui
                // produit à ~90 i/s.
                Err(TryRecvError::Empty) => {
                    self.fenetre.succes();
                    return None;
                }
                // Le canal est rompu. La fenêtre de reprise borne combien de
                // temps la source reste vivante en attendant un rattachement.
                Err(TryRecvError::Disconnected) => {
                    // **Un épuisement AUTORITAIRE ne se retente pas** (I2,
                    // revue finale de branche du sous-bloc D4). À chaque
                    // fermeture NORMALE d'une fenêtre, le capteur pousse un
                    // `Etat { epuisee: true }` puis ferme le tube : l'enfant
                    // consomme cet état, reboucle, et voit `Disconnected` dans
                    // le même tour. Sans cette garde il rattacherait
                    // aussitôt — un tube de commandes neuf, un tube média
                    // neuf, et côté capteur une duplication DXGI et un
                    // `H264Encoder` neufs sur une sortie que le superviseur
                    // est justement en train de détruire. Pire, un
                    // rattachement qui aboutirait remettrait `epuisee` à faux
                    // (plus bas) : la session qui devait se clore ne se
                    // clorait pas, et le résultat dépendrait d'une course.
                    if self.epuisee {
                        return None;
                    }
                    let maintenant = Instant::now();
                    if self.fenetre.rupture(maintenant) {
                        // La fenêtre est expirée : l'épuisement est acquis.
                        self.epuisee = true;
                        return None;
                    }
                    if self.fenetre.peut_reessayer(maintenant) {
                        match self.canal.rattacher() {
                            Ok(Rattachee { images, largeur, hauteur }) => {
                                tracing::info!(largeur, hauteur, "canal rattaché au capteur");
                                self.images = images;
                                self.largeur = largeur;
                                self.hauteur = hauteur;
                                self.vivante = true;
                                self.epuisee = false;
                                // Un rattachement passe par `VersCapteur::Attache`,
                                // donc par une `Fenetre` NEUVE côté capteur — et
                                // une fenêtre naît endormie. Garder ici l'état
                                // d'avant la rupture ferait appliquer la part
                                // plancher de cette renaissance comme un plafond
                                // d'encodage, pour toute la durée qui sépare
                                // l'attache du premier `Ordre::Reveiller`.
                                self.endormie = true;
                                // ⚠️ **`Some(false)`, PAS `None` : au
                                // rattachement, l'enfant REDEVIENT MUET**
                                // (conception §4.4 ; F2, revue finale de
                                // branche du sous-bloc D7). `None` ne veut
                                // pas dire « muet », il veut dire « rien à
                                // changer » : le drapeau `emet` de la
                                // `WindowsAudioSource` gardait alors sa valeur
                                // d'AVANT la rupture, et une fenêtre qui
                                // portait le son continuait de le porter.
                                //
                                // Le cas qui mord : deux fenêtres A et B d'un
                                // même PID, A porteuse, le capteur redémarre.
                                // B se rattache la première, son groupe est
                                // vide côté capteur — donc elle est élue et
                                // démarre. A se rattache quelques dizaines à
                                // quelques centaines de millisecondes plus
                                // tard (D4 a mesuré 538 à 689 ms pour le seul
                                // rattachement, et rien ne synchronise les deux
                                // enfants) en émettant TOUJOURS : les deux
                                // jouent le même mix du même PID, désynchronisé
                                // — un écho audible — jusqu'à ce que le
                                // `Audio { actif: false }` destiné à A arrive.
                                //
                                // Le défaut inverse n'existe pas : un
                                // rattachement passe par `VersCapteur::Attache`,
                                // donc par une `Fenetre` NEUVE côté capteur,
                                // dont `sommeil::inscrire` purge
                                // `derniers_audio` — un ordre neuf arrive donc
                                // TOUJOURS, et sans la borne du tour de roue.
                                // `inscrire` appelle `distribuer_l_audio`
                                // SYNCHRONEMENT, avant même que `boucler` ne
                                // démarre sa boucle (`fenetre.rs`, `servir`),
                                // laquelle sonde `ordres` en tête de chaque
                                // tour, sans délai. Ce n'est PAS le résidu de
                                // `sommeil/porteurs.rs` (un ordre différé au
                                // TOUR DE ROUE SUIVANT, borné
                                // `PERIODE_REARBITRAGE` = 250 ms) : ce
                                // résidu-là ne joue que quand le canal d'une
                                // fenêtre VOISINE casse pendant la MÊME passe
                                // d'arbitrage. Le silence d'une porteuse qui se
                                // rattache est donc borné par l'acheminement du
                                // message sur le fil, et c'est l'arbitrage que
                                // la conception a choisi : un blanc bref plutôt
                                // qu'un écho.
                                self.audio = Some(false);
                                self.fenetre.succes();
                            }
                            // Journalisé en `debug!` et non `info!` : au pas
                            // de 250 ms sur une fenêtre de 15 s, un capteur
                            // durablement absent produirait 60 lignes par
                            // fenêtre et par session.
                            Err(erreur) => tracing::debug!(%erreur, "rattachement refusé"),
                        }
                    }
                    return None;
                }
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    fn is_exhausted(&self) -> bool {
        self.epuisee
    }

    fn is_alive(&self) -> bool {
        self.vivante
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        match self
            .canal
            .commander(VersCapteur::Redimensionner { largeur: width, hauteur: height })?
        {
            // La taille RETENUE est celle obtenue, jamais celle demandée : le
            // pilote quantifie, et une fenêtre Windows impose des dimensions
            // paires. Même règle qu'en mono-fenêtre.
            DepuisCapteur::Taille { largeur, hauteur } => {
                self.largeur = largeur;
                self.hauteur = hauteur;
                Ok(())
            }
            DepuisCapteur::Erreur { motif } => bail!("le capteur a refusé : {motif}"),
            autre => bail!("réponse inattendue du capteur : {autre:?}"),
        }
    }

    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.commander_simple(VersCapteur::Debit { bps: bitrate })
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        self.commander_simple(VersCapteur::TailleEncodage { largeur: width, hauteur: height })
    }

    fn request_keyframe(&mut self) -> Result<()> {
        self.commander_simple(VersCapteur::ImageCle)
    }

    fn set_awake(&mut self, visible: bool, focalisee: bool) -> Result<()> {
        self.commander_simple(VersCapteur::Visibilite { visible, focalisee })
    }

    /// Rend le changement de sommeil en attente, et le consomme.
    ///
    /// **Une annonce ne se répète pas** : la boucle de transport l'interroge à
    /// chaque tour, et réémettre le même message inonderait le canal de
    /// contrôle.
    fn sommeil_a_annoncer(&mut self) -> Option<(bool, String)> {
        self.sommeil.take()
    }

    /// Rend la part en attente, et la consomme.
    fn part_a_appliquer(&mut self) -> Option<u32> {
        self.part.take()
    }

    /// Rend l'ordre audio en attente, et le consomme.
    fn audio_a_appliquer(&mut self) -> Option<bool> {
        self.audio.take()
    }

    /// Rend le changement de plein écran en attente, et le consomme.
    fn plein_ecran_a_annoncer(&mut self) -> Option<bool> {
        self.plein_ecran.take()
    }

    /// Rend l'état de sommeil courant, sans le consommer.
    fn est_endormie(&self) -> bool {
        self.endormie
    }
}

// Les tests vivent dans un fichier voisin : ce fichier-ci était à 487 lignes
// pour un plafond de projet à 500. Voir l'en-tête de `distante/tests.rs`.
#[cfg(test)]
mod tests;
