//! Classer un échec d'acquisition DXGI, et borner les reprises.
//!
//! **Pur à dessein.** `capture.rs` est `#![cfg(windows)]` dans son ensemble :
//! un module ENFANT n'y serait pas compilable sur l'hôte Linux, donc pas
//! testable. Ce fichier est donc déclaré en module FRÈRE dans `main.rs`
//! (`#[path = "capture/reprise.rs"] mod capture_reprise;`), hors de tout
//! `cfg` — le même montage que `windows_source/sortie.rs`, et pour la même
//! raison.
//!
//! Il ne connaît que des `i32` : les codes DXGI nus. Aucun type `windows-rs`
//! ne franchit cette frontière, sans quoi elle ne tiendrait pas.

/// `DXGI_ERROR_ACCESS_LOST`. DXGI révoque l'accès à une duplication quand la
/// topologie d'affichage change — et **la création d'une sortie virtuelle en
/// est un cas**, relevé par le sous-bloc D1 sur trois exécutions sur trois.
/// La documentation Desktop Duplication décrit cet état comme récupérable :
/// relâcher l'`IDXGIOutputDuplication` et en créer une nouvelle.
pub const ACCES_PERDU: i32 = 0x887A0026u32 as i32;

/// `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`. Rendu par `DuplicateOutput` quand la
/// sortie ne peut pas être dupliquée **à cet instant**. Deux causes très
/// différentes se présentent sous ce même code, et le code ne les distingue
/// pas : une reconfiguration de topologie en cours — passagère —, et un plafond
/// de duplications concurrentes — durable. C'est pourquoi la fenêtre de
/// réessai est courte et son abandon bruyant.
pub const NON_DISPONIBLE: i32 = 0x887A0022u32 as i32;

/// `DXGI_ERROR_DEVICE_REMOVED`. Le périphérique lui-même est perdu : rouvrir
/// la seule duplication ne servirait à rien. Reste définitif.
pub const DEVICE_REMOVED: i32 = 0x887A0005u32 as i32;

/// `DXGI_ERROR_WAIT_TIMEOUT`. Pas un échec : le bureau n'a simplement pas
/// changé. Traité en amont de la classification, mais nommé ici pour que le
/// test puisse vérifier qu'il n'est PAS pris pour une perte d'accès.
pub const ATTENTE_EXPIREE: i32 = 0x887A0027u32 as i32;

/// Durée pendant laquelle une perte d'accès est retentée avant d'être déclarée
/// définitive.
///
/// **Majorante et non calibrée, et il faut le dire.** La mesure du
/// 1ᵉʳ août 2026 (`plans/journaux-multifenetres-d2/`) établit deux points et
/// deux seulement : trois tentatives enchaînées sans délai, soit 14 à 21 ms,
/// **ne suffisent pas** ; et une réouverture tentée 3 s après le remaniement
/// **réussit**, sur 7 sondes sur 7. Le seuil réel est quelque part entre les
/// deux et n'a pas été cherché. Huit secondes le couvrent largement.
///
/// Ce qui borne le coût d'une valeur trop grande : la fenêtre ne bloque rien
/// (voir `Tentative::Patienter`), elle ne fait que retarder l'aveu d'échec
/// d'une source qui, de toute façon, ne rendrait plus d'image.
pub const DUREE_FENETRE_REPRISE: std::time::Duration = std::time::Duration::from_secs(8);

/// Intervalle minimal entre deux tentatives de réouverture.
///
/// Petit devant la fenêtre, pour ne pas retarder la reprise réelle ; assez
/// grand pour que la trace `info!` de chaque tentative reste rare — au plus
/// ~7 tentatives par seconde et par source (1000 ms / 150 ms), soit jusqu'à
/// ~13 lignes par seconde quand chaque réouverture échoue (une ligne de
/// tentative, une ligne d'échec), contre une par appel de `next_frame`
/// (~90/s) si le pas n'existait pas. Le dépôt a déjà payé deux fois pour une
/// trace émise à la cadence de la boucle de capture.
pub const PAS_REPRISE: std::time::Duration = std::time::Duration::from_millis(150);

/// Durée pendant laquelle l'ouverture d'une duplication est retentée.
///
/// **Plus courte que `DUREE_FENETRE_REPRISE`**, et pour une raison de
/// diagnostic : quand la cause est un plafond de concurrence, patienter
/// davantage ne change pas le résultat et retarde la lecture. Trois secondes
/// couvrent la reconfiguration de topologie que le dépôt admet par ailleurs
/// (`DELAI_TOPOLOGIE`).
///
/// **Majorante et non calibrée**, comme `DUREE_FENETRE_REPRISE`.
pub const DUREE_FENETRE_OUVERTURE: std::time::Duration = std::time::Duration::from_secs(3);

pub fn est_acces_perdu(code: i32) -> bool {
    code == ACCES_PERDU
}

/// Vrai si un échec d'ouverture de duplication mérite d'être retenté.
pub fn est_ouverture_retentable(code: i32) -> bool {
    code == NON_DISPONIBLE || code == ACCES_PERDU
}

/// Ce que la fenêtre demande à l'appelant de faire, maintenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tentative {
    /// Retenter la réouverture tout de suite.
    Rouvrir,
    /// Ne rien faire de ce tour-ci. L'appelant rend « rien de neuf » — **sans
    /// dormir** : c'est ce qui distingue cette forme d'une boucle de reprise
    /// bloquante, et ce qui laisse la boucle de session continuer de tourner.
    Patienter,
    /// La fenêtre est close : la perte d'accès est définitive.
    Expiree,
}

/// Fenêtre de reprise, ouverte à la première perte d'accès et refermée par le
/// premier succès.
///
/// **Elle ne lit aucune horloge** : l'instant lui est passé. C'est ce qui la
/// rend testable sur l'hôte Linux, où tout le reste de ce chemin est invisible.
///
/// **Elle a remplacé un budget en nombre de tentatives**, que la mesure a
/// réfuté : créer une sortie virtuelle rend `ACCESS_LOST`, la réouverture
/// réussit, et la duplication rouverte rend **aussitôt** `ACCESS_LOST` à son
/// tour tant que Windows n'a pas fini de reconfigurer sa topologie. Trois
/// tentatives sans délai étaient donc brûlées avant que le phénomène ne se
/// termine. Le compte de tentatives ne mesurait pas la bonne grandeur.
pub struct FenetreDeReprise {
    ouverte_a: Option<std::time::Instant>,
    derniere_tentative: Option<std::time::Instant>,
    tentatives: u32,
}

impl FenetreDeReprise {
    pub fn nouvelle() -> Self {
        Self { ouverte_a: None, derniere_tentative: None, tentatives: 0 }
    }

    pub fn tenter(&mut self, maintenant: std::time::Instant) -> Tentative {
        let ouverte_a = *self.ouverte_a.get_or_insert(maintenant);
        if maintenant.duration_since(ouverte_a) > DUREE_FENETRE_REPRISE {
            return Tentative::Expiree;
        }
        if let Some(derniere) = self.derniere_tentative {
            if maintenant.duration_since(derniere) < PAS_REPRISE {
                return Tentative::Patienter;
            }
        }
        self.derniere_tentative = Some(maintenant);
        self.tentatives += 1;
        Tentative::Rouvrir
    }

    /// Referme la fenêtre. Appelée sur tout succès d'acquisition, `Ok(None)`
    /// compris : dès que DXGI cesse de refuser, le remaniement est terminé.
    pub fn succes(&mut self) {
        self.ouverte_a = None;
        self.derniere_tentative = None;
        self.tentatives = 0;
    }

    pub fn tentatives(&self) -> u32 {
        self.tentatives
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seule_la_perte_d_acces_est_recuperable() {
        assert!(est_acces_perdu(ACCES_PERDU));
        assert!(!est_acces_perdu(DEVICE_REMOVED), "périphérique perdu : rouvrir ne sert à rien");
        assert!(!est_acces_perdu(ATTENTE_EXPIREE), "attente expirée n'est même pas un échec");
        assert!(!est_acces_perdu(0), "S_OK");
        assert!(!est_acces_perdu(0x80070057u32 as i32), "E_INVALIDARG");
    }

    /// `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE` dit dans son propre libellé que la
    /// ressource « pourra l'être ultérieurement ». C'est ce que rend une
    /// `DuplicateOutput` tentée pendant que Windows reconfigure sa topologie —
    /// le cas nominal quand une autre fenêtre s'ouvre au même instant.
    #[test]
    fn une_ouverture_est_retentable_sur_indisponibilite_ou_perte_d_acces() {
        assert!(est_ouverture_retentable(NON_DISPONIBLE));
        assert!(est_ouverture_retentable(ACCES_PERDU));
    }

    /// Un périphérique perdu ne reviendra pas, et un argument invalide n'est
    /// pas une question de patience : les retenter ne ferait que retarder le
    /// diagnostic de trois secondes.
    #[test]
    fn une_ouverture_n_est_pas_retentable_sur_une_panne_franche() {
        assert!(!est_ouverture_retentable(DEVICE_REMOVED));
        assert!(!est_ouverture_retentable(0x80070057u32 as i32), "E_INVALIDARG");
        assert!(!est_ouverture_retentable(0), "S_OK");
    }

    /// La fenêtre d'ouverture est plus COURTE que celle de la capture, et c'est
    /// délibéré : un échec durable à l'ouverture doit se lire vite, la vraie
    /// cause pouvant être un plafond de concurrence que nulle patience ne
    /// franchit.
    #[test]
    fn la_fenetre_d_ouverture_est_plus_courte_que_celle_de_la_capture() {
        assert!(DUREE_FENETRE_OUVERTURE < DUREE_FENETRE_REPRISE);
        assert!(DUREE_FENETRE_OUVERTURE >= std::time::Duration::from_secs(2));
    }

    /// Une base d'instants qui ne lit pas l'horloge du système : le type sous
    /// test n'en lit aucune, c'est tout l'intérêt.
    fn t(base: std::time::Instant, ms: u64) -> std::time::Instant {
        base + std::time::Duration::from_millis(ms)
    }

    #[test]
    fn la_premiere_perte_fait_rouvrir_tout_de_suite() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        assert_eq!(fenetre.tenter(t(base, 0)), Tentative::Rouvrir);
        assert_eq!(fenetre.tentatives(), 1);
    }

    /// Le défaut que la mesure a relevé : trois tentatives sans délai étaient
    /// brûlées en 14 à 21 ms, alors que la topologie met jusqu'à 3 s à se
    /// stabiliser. Le pas d'attente est ce qui empêche cela.
    #[test]
    fn une_seconde_tentative_trop_proche_fait_patienter() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        fenetre.tenter(t(base, 0));
        assert_eq!(fenetre.tenter(t(base, 5)), Tentative::Patienter);
        assert_eq!(fenetre.tenter(t(base, 20)), Tentative::Patienter);
        assert_eq!(
            fenetre.tentatives(),
            1,
            "patienter n'est pas une tentative"
        );
    }

    #[test]
    fn le_pas_ecoule_fait_rouvrir_a_nouveau() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        fenetre.tenter(t(base, 0));
        let apres_le_pas = PAS_REPRISE.as_millis() as u64;
        assert_eq!(fenetre.tenter(t(base, apres_le_pas)), Tentative::Rouvrir);
        assert_eq!(fenetre.tentatives(), 2);
    }

    /// La fenêtre doit couvrir largement les 3 s que ce dépôt admet déjà pour
    /// qu'une topologie se stabilise (`DELAI_TOPOLOGIE`).
    #[test]
    fn la_fenetre_couvre_largement_la_stabilisation_de_la_topologie() {
        assert!(
            DUREE_FENETRE_REPRISE >= std::time::Duration::from_secs(6),
            "la sonde post-mortem a réussi à 3 s ; une fenêtre qui ne les \
             couvrirait pas au double reproduirait le défaut mesuré"
        );
        assert!(
            PAS_REPRISE < DUREE_FENETRE_REPRISE / 10,
            "un pas trop grand devant la fenêtre retarderait la reprise réelle"
        );
    }

    #[test]
    fn la_fenetre_expire_au_bout_de_sa_duree() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        fenetre.tenter(t(base, 0));
        let apres = DUREE_FENETRE_REPRISE.as_millis() as u64 + 1;
        assert_eq!(fenetre.tenter(t(base, apres)), Tentative::Expiree);
    }

    /// L'expiration se compte depuis l'OUVERTURE de la fenêtre, pas depuis la
    /// dernière tentative : sans quoi une reprise qui échoue indéfiniment ne
    /// finirait jamais.
    #[test]
    fn l_expiration_se_compte_depuis_l_ouverture_et_non_depuis_la_derniere_tentative() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        let pas = PAS_REPRISE.as_millis() as u64;
        let mut instant = 0;
        while instant < DUREE_FENETRE_REPRISE.as_millis() as u64 {
            fenetre.tenter(t(base, instant));
            instant += pas;
        }
        assert_eq!(fenetre.tenter(t(base, instant)), Tentative::Expiree);
    }

    /// La fenêtre se referme sur un succès d'acquisition, `Ok(None)` compris —
    /// c'est-à-dire dès que DXGI cesse de refuser, même sans image neuve. Un
    /// bureau immobile ne produit aucune image pendant de longues périodes, et
    /// une fenêtre qui ne se refermerait que sur une image livrée
    /// transformerait des pertes rares et sans rapport en une usure.
    #[test]
    fn un_succes_referme_la_fenetre_qui_rouvre_alors_pleine() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        fenetre.tenter(t(base, 0));
        fenetre.succes();
        assert_eq!(fenetre.tentatives(), 0);

        let tard = DUREE_FENETRE_REPRISE.as_millis() as u64 * 3;
        assert_eq!(
            fenetre.tenter(t(base, tard)),
            Tentative::Rouvrir,
            "une perte d'accès bien plus tard ouvre une fenêtre NEUVE"
        );
        assert_eq!(
            fenetre.tenter(t(base, tard + DUREE_FENETRE_REPRISE.as_millis() as u64 + 1)),
            Tentative::Expiree,
            "et cette fenêtre neuve court depuis SA propre ouverture"
        );
    }
}
