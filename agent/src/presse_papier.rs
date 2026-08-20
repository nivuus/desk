//! Le presse-papier de la VM, sens **VM → navigateur** : détecter qu'il a
//! changé, en lire le texte, et décider ce qu'on annonce.
//!
//! **Pur, sans aucun `cfg`** — comme `capteur/plein_ecran.rs`,
//! `capteur/audio.rs` et `capteur/repartiteur.rs` avant lui. Toute la
//! décision vit ici et s'éprouve sur l'hôte Linux ; les deux appels Win32
//! vivent dans `presse_papier/win32.rs`, gaté, et **ne décident rien**.
//!
//! Le module est à la **racine nue** (`mod presse_papier;` dans `main.rs`) et
//! non sous `capteur/`, alors que le propriétaire est aujourd'hui le capteur
//! et lui seul. La raison n'est pas celle que la spec avance — la « Convention
//! de module enfant » de `CLAUDE.md` déclare elle-même sa portée et ne couvre
//! pas ce cas, ce module n'étant extrait de rien. C'est que la décision D1 pose
//! que le propriétaire est « le capteur quand il existe, l'enfant sinon » : un
//! module rangé sous `capteur/` porterait un nom faux le jour où le
//! propriétaire mono-fenêtre arrivera.
//!
//! **Ce que ce module ne fait PAS, et ne doit pas se mettre à faire** : il
//! n'écrit **jamais** le presse-papier Windows. Le sens navigateur → VM est le
//! sous-bloc P2, et c'est lui qui portera les gardes anti-écho de D5. Le seul
//! garde livré ici est le n°2, l'égalité de contenu — il ne ferme aucune
//! boucle (il n'y en a pas), il absorbe le faux positif du compteur : celui-ci
//! **bouge sur une réécriture identique**, mesuré (sonde P0, `q2="bouge"`, deux
//! exécutions du 20 août 2026).

use std::time::{Duration, Instant};

/// Taille maximale, en octets d'UTF-8 **après normalisation**, d'un contenu
/// que l'on accepte de pousser au navigateur.
///
/// ⚠️ **NON CALIBRÉE.** Elle rejoint `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS` et `TAILLE_MAX_SORTIE` : aucun jugement
/// d'usage n'a été porté sur sa valeur. 64 KiB tient un document texte
/// ordinaire et refuse un presse-papier chargé d'un fichier entier.
///
/// **Au-delà, on REFUSE — on ne tronque pas.** Un collage silencieusement
/// amputé est le pire résultat possible, et il est pire que pas de collage du
/// tout : l'utilisateur ne peut pas voir qu'il lui manque la fin.
pub const PRESSE_PAPIER_MAX: usize = 64 * 1024;

/// Période minimale entre deux lectures du compteur de séquence.
///
/// ⚠️ **Ce n'est PAS `PERIODE_REARBITRAGE`**, qui cadence le tour de roue du
/// registre de sommeil. La valeur est du même ordre, délibérément, mais la
/// constante est propre à ce module : les faire suivre l'une l'autre
/// coupleraient deux mécanismes que rien ne lie — c'est exactement l'argument
/// que `plein_ecran::PERIODE_STYLE` porte déjà pour la relecture du style.
///
/// `Sondeur::tour` porte donc son propre minuteur et rend `None` sans rien
/// lire tant qu'il n'est pas échu, **même si le tour de roue l'appelle plus
/// souvent**.
pub const PERIODE_PRESSE_PAPIER: Duration = Duration::from_millis(250);

/// Ce que le sondeur a décidé d'annoncer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Annonce {
    /// Le texte à pousser, **déjà normalisé et sous la borne**.
    Texte(String),
    /// Un contenu de `octets` octets d'UTF-8, **après normalisation**, a été
    /// REFUSÉ — jamais tronqué. Le compte sert au bandeau côté client, qui
    /// doit pouvoir dire *combien* plutôt que « trop grand ».
    Refus { octets: u32 },
}

/// `PRESSE_PAPIER=0` désarme le mécanisme entier.
///
/// **`=0` DÉSACTIVE, une simple présence n'active pas**, exactement comme
/// `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR` et `CAPTEUR` : tester `is_ok()`
/// armerait le mécanisme en écrivant `PRESSE_PAPIER=0` pour le couper.
///
/// `OnceLock` et non une lecture par appel : le sondage court à 4 Hz, et
/// l'environnement ne change pas en cours de processus.
pub fn actif() -> bool {
    static ACTIF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ACTIF.get_or_init(|| {
        let actif = std::env::var("PRESSE_PAPIER").as_deref() != Ok("0");
        if !actif {
            tracing::warn!(
                "presse-papier DESARME (PRESSE_PAPIER=0) : le contenu copie dans la VM \
                 n'est plus pousse au navigateur"
            );
        }
        actif
    })
}

/// Ramène toutes les fins de ligne à `\n`.
///
/// Windows écrit `\r\n` ; d'anciennes applications écrivent un `\r` **seul**.
/// Les deux doivent devenir `\n`, sans quoi l'aller-retour de P2 doublerait
/// les lignes à chaque tour. La fonction est **idempotente** : la rejouer sur
/// son propre résultat ne change rien.
pub fn normaliser(texte: &str) -> String {
    let mut sortie = String::with_capacity(texte.len());
    let mut precedent_cr = false;
    for c in texte.chars() {
        match c {
            '\r' => {
                sortie.push('\n');
                precedent_cr = true;
            }
            '\n' => {
                // Le `\n` d'un `\r\n` a déjà été rendu par le `\r`.
                if !precedent_cr {
                    sortie.push('\n');
                }
                precedent_cr = false;
            }
            autre => {
                sortie.push(autre);
                precedent_cr = false;
            }
        }
    }
    sortie
}

/// Observe le presse-papier et décide ce qu'il faut annoncer.
///
/// Il ne tient **aucune** ressource Windows : c'est l'appelant qui lui donne
/// le numéro de séquence et la fermeture de lecture.
#[derive(Debug, Default)]
pub struct Sondeur {
    /// Le dernier numéro de séquence pour lequel une lecture a **réussi**.
    ///
    /// `None` au premier tour : l'état lu alors fait **référence** et n'est
    /// **pas annoncé** — c'est le patron de `SuiviBordure`
    /// (`capteur/plein_ecran.rs`). Une fenêtre qui s'attache ne reçoit donc
    /// pas le contenu déjà présent ; elle reçoit la première copie **qui
    /// suit**.
    reference: Option<u32>,
    /// Le dernier contenu réellement annoncé — le **garde n°2 de D5**.
    dernier_emis: Option<String>,
    /// La dernière taille refusée, pour ne pas répéter le refus.
    dernier_refus: Option<u32>,
    /// Quand `tour()` a lu le compteur pour la dernière fois.
    dernier_tour: Option<Instant>,
}

impl Sondeur {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// **Le cœur, et il ne touche pas Windows.**
    ///
    /// `seq` est le numéro de séquence lu par l'appelant ; `lire` n'est
    /// appelée **que** si ce numéro a bougé. C'est ce qui rend éprouvable sur
    /// l'hôte, sans le moindre `cfg`, la propriété « on n'ouvre pas le
    /// presse-papier pour rien » — l'ouverture est une ressource contendue
    /// sous Windows, et l'ouvrir à chaque tour affamerait les applications.
    ///
    /// `lire` rend `None` quand la lecture a **échoué** (une autre application
    /// tient le presse-papier — cas NORMAL sous Windows, pas une panne) ou
    /// quand le presse-papier ne porte pas de texte. **La référence n'avance
    /// alors PAS** : sans cela le contenu correspondant serait perdu à jamais,
    /// le tour suivant voyant un compteur « inchangé » et ne retentant rien.
    pub fn observer(
        &mut self,
        seq: u32,
        lire: impl FnOnce() -> Option<String>,
    ) -> Option<Annonce> {
        if self.reference == Some(seq) {
            return None;
        }
        let brut = lire()?;
        // La référence n'avance qu'après une lecture RÉUSSIE (D-P1-5).
        let premier_tour = self.reference.is_none();
        self.reference = Some(seq);
        if premier_tour {
            // L'état lu à l'attache fait référence : on n'annonce rien.
            self.dernier_emis = Some(normaliser(&brut));
            return None;
        }
        // Normaliser d'abord, borner ensuite (D-P1-2) : la borne porte sur ce
        // qu'on ÉMET, jamais sur ce qu'on a lu. Un texte Windows de 65 000
        // lignes perd 65 000 octets à la normalisation ; borner d'abord
        // refuserait un texte qui, une fois normalisé, tiendrait.
        let texte = normaliser(&brut);
        let octets = texte.len();
        if octets > PRESSE_PAPIER_MAX {
            let octets = octets as u32;
            // Un refus répété à l'identique n'est annoncé qu'une fois : sinon
            // un contenu énorme laissé dans le presse-papier ferait clignoter
            // le bandeau à chaque copie voisine.
            if self.dernier_refus == Some(octets) {
                return None;
            }
            self.dernier_refus = Some(octets);
            return Some(Annonce::Refus { octets });
        }
        self.dernier_refus = None;
        // Garde n°2 de D5 : le compteur bouge sur une réécriture identique
        // (mesuré, sonde P0). Sans cette comparaison, un tel geste pousserait
        // un message pour rien.
        if self.dernier_emis.as_deref() == Some(texte.as_str()) {
            return None;
        }
        self.dernier_emis = Some(texte.clone());
        Some(Annonce::Texte(texte))
    }

    /// Un tour de sondage complet, **hors de tout verrou**.
    ///
    /// Rend `None` sans rien lire tant que `PERIODE_PRESSE_PAPIER` n'est pas
    /// échue, même si l'appelant vient plus souvent.
    pub fn tour(&mut self) -> Option<Annonce> {
        let maintenant = Instant::now();
        if let Some(dernier) = self.dernier_tour {
            if maintenant.duration_since(dernier) < PERIODE_PRESSE_PAPIER {
                return None;
            }
        }
        self.dernier_tour = Some(maintenant);
        if !actif() {
            return None;
        }
        self.lire_la_plateforme()
    }

    #[cfg(windows)]
    fn lire_la_plateforme(&mut self) -> Option<Annonce> {
        let seq = win32::numero_de_sequence();
        self.observer(seq, || win32::lire_texte().ok().flatten())
    }

    /// Repli non-Windows : il n'y a pas de presse-papier système à observer.
    ///
    /// **Ce stub est obligatoire, pas décoratif** : l'appelant
    /// (`capteur/sommeil/registre.rs`) n'est pas gaté et doit compiler sur
    /// l'hôte Linux — à la différence de `plein_ecran::lire_style`, dont
    /// l'unique appelant est lui-même `#[cfg(windows)]`.
    #[cfg(not(windows))]
    fn lire_la_plateforme(&mut self) -> Option<Annonce> {
        None
    }
}

#[cfg(windows)]
mod win32;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Un sondeur qui a déjà pris sa référence : c'est l'état nominal après
    /// le premier tour, et celui dans lequel toutes les propriétés ci-dessous
    /// se jugent.
    fn amorce(sondeur: &mut Sondeur) {
        assert_eq!(sondeur.observer(1, || Some(String::from("etat-initial"))), None);
    }

    /// ROUGE si `normaliser` laisse passer `\r\n` : l'aller-retour de P2
    /// doublerait alors les lignes à chaque tour.
    #[test]
    fn normaliser_ramene_crlf_a_lf() {
        assert_eq!(normaliser("a\r\nb"), "a\nb");
    }

    /// ROUGE si l'on ne traite que `\r\n` : les fins de ligne Mac classiques
    /// passeraient telles quelles.
    #[test]
    fn normaliser_ramene_un_cr_seul_a_lf() {
        assert_eq!(normaliser("a\rb"), "a\nb");
    }

    /// ROUGE si `normaliser` remplaçait `\n` par `\r\n` : la fonction ne
    /// serait plus idempotente et l'aller-retour de P2 divergerait.
    #[test]
    fn normaliser_est_idempotente() {
        let une = normaliser("a\r\nb\rc\nd");
        assert_eq!(une, "a\nb\nc\nd");
        assert_eq!(normaliser(&une), une);
    }

    /// 🔴 « On n'ouvre pas le presse-papier pour rien », et c'est vérifiable
    /// SANS Windows : le témoin est un `Cell<bool>`.
    ///
    /// ROUGE si `observer` appelle `lire` inconditionnellement.
    #[test]
    fn un_numero_inchange_n_ouvre_pas_le_presse_papier() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        let appele = Cell::new(false);
        let annonce = sondeur.observer(1, || {
            appele.set(true);
            Some(String::from("bonjour"))
        });
        assert_eq!(annonce, None);
        assert!(!appele.get(), "lire() ne doit pas être appelée à numéro inchangé");
    }

    #[test]
    fn un_numero_neuf_annonce_le_texte() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        assert_eq!(
            sondeur.observer(2, || Some(String::from("bonjour"))),
            Some(Annonce::Texte(String::from("bonjour")))
        );
    }

    /// 🔴 Le garde n°2 de D5, et la rouge du critère ② de la recette.
    ///
    /// Le compteur BOUGE sur une réécriture identique — mesuré par la sonde
    /// P0 (`q2="bouge"`, deux exécutions). Sans la comparaison de contenu, ce
    /// geste pousserait un message pour rien.
    ///
    /// ROUGE si l'on retire la comparaison : `Some` serait rendu deux fois.
    #[test]
    fn un_meme_texte_a_un_numero_different_n_est_annonce_qu_une_fois() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        assert_eq!(
            sondeur.observer(2, || Some(String::from("bonjour"))),
            Some(Annonce::Texte(String::from("bonjour")))
        );
        assert_eq!(sondeur.observer(3, || Some(String::from("bonjour"))), None);
    }

    /// 🔴 Au-delà de la borne on REFUSE, on ne tronque JAMAIS : un collage
    /// silencieusement amputé est le pire résultat possible.
    ///
    /// ROUGE si l'implémentation tronque — l'assertion sur la variante tombe.
    #[test]
    fn un_texte_trop_grand_est_refuse_jamais_tronque() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        let gros = "a".repeat(PRESSE_PAPIER_MAX + 1);
        let annonce = sondeur.observer(2, || Some(gros));
        assert_eq!(annonce, Some(Annonce::Refus { octets: (PRESSE_PAPIER_MAX + 1) as u32 }));
    }

    /// ROUGE si la borne est écrite `>=` au lieu de `>`.
    #[test]
    fn un_texte_de_la_taille_exacte_de_la_borne_passe() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        let pile = "a".repeat(PRESSE_PAPIER_MAX);
        assert_eq!(sondeur.observer(2, || Some(pile.clone())), Some(Annonce::Texte(pile)));
    }

    /// 🔴 D-P1-2 : on normalise D'ABORD, on borne ENSUITE.
    ///
    /// Le texte pèse `PRESSE_PAPIER_MAX + 8` octets bruts et porte 12 `\r`
    /// appariés à autant de `\n` : la normalisation lui en retire 12, donc il
    /// tient. ROUGE si l'on borne avant de normaliser — il serait refusé.
    #[test]
    fn on_normalise_avant_de_borner() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        let corps = "a".repeat(PRESSE_PAPIER_MAX + 8 - 24);
        let brut = format!("{corps}{}", "\r\n".repeat(12));
        assert_eq!(brut.len(), PRESSE_PAPIER_MAX + 8);
        let attendu = normaliser(&brut);
        assert_eq!(attendu.len(), PRESSE_PAPIER_MAX - 4);
        assert_eq!(sondeur.observer(2, || Some(brut)), Some(Annonce::Texte(attendu)));
    }

    /// 🔴 Le bornage compte des OCTETS d'UTF-8, pas des `char`.
    ///
    /// ROUGE si l'on borne sur `.chars().count()` : ce texte fait
    /// `PRESSE_PAPIER_MAX / 4 + 1` caractères, très en dessous de la borne
    /// comptée ainsi, et passerait alors qu'il pèse plus de 64 KiB.
    #[test]
    fn le_bornage_compte_des_octets_pas_des_caracteres() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        let emoji = "🙂".repeat(PRESSE_PAPIER_MAX / 4 + 1);
        assert_eq!(emoji.chars().count(), PRESSE_PAPIER_MAX / 4 + 1);
        assert!(emoji.len() > PRESSE_PAPIER_MAX);
        assert!(matches!(
            sondeur.observer(2, || Some(emoji)),
            Some(Annonce::Refus { .. })
        ));
    }

    /// 🔴 D-P1-5 : une lecture qui ÉCHOUE n'avance pas la référence.
    ///
    /// Sinon le contenu correspondant serait perdu à jamais : le tour suivant
    /// verrait un compteur « inchangé » et ne retenterait rien.
    ///
    /// ROUGE si l'on mémorise le numéro avant la lecture — le second appel,
    /// au MÊME numéro, n'appellerait plus `lire`.
    #[test]
    fn une_lecture_echouee_n_avance_pas_la_reference() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        assert_eq!(sondeur.observer(2, || None), None);
        let rappelee = Cell::new(false);
        let annonce = sondeur.observer(2, || {
            rappelee.set(true);
            Some(String::from("rattrape"))
        });
        assert!(rappelee.get(), "le même numéro doit être retenté après un échec");
        assert_eq!(annonce, Some(Annonce::Texte(String::from("rattrape"))));
    }

    /// ROUGE si le refus n'est pas mémorisé : le bandeau clignoterait à
    /// chaque copie voisine tant que le contenu énorme reste en place.
    #[test]
    fn un_refus_repete_a_l_identique_n_est_annonce_qu_une_fois() {
        let mut sondeur = Sondeur::nouveau();
        amorce(&mut sondeur);
        let gros = "a".repeat(PRESSE_PAPIER_MAX + 1);
        assert!(sondeur.observer(2, || Some(gros.clone())).is_some());
        assert_eq!(sondeur.observer(3, || Some(gros)), None);
    }

    /// L'état lu au PREMIER tour fait référence, et n'est pas annoncé : une
    /// fenêtre qui s'attache ne reçoit pas le contenu déjà présent, elle
    /// reçoit la première copie QUI SUIT (D-P1-4, patron de `SuiviBordure`).
    ///
    /// ROUGE si le premier tour annonce — ce qui ferait recevoir à chaque
    /// attache un contenu que l'utilisateur n'a pas copié pour elle.
    #[test]
    fn le_premier_tour_prend_reference_et_n_annonce_rien() {
        let mut sondeur = Sondeur::nouveau();
        assert_eq!(sondeur.observer(7, || Some(String::from("deja-la"))), None);
        // Et ce contenu-là est bien retenu : le recopier ne relance rien.
        assert_eq!(sondeur.observer(8, || Some(String::from("deja-la"))), None);
        assert_eq!(
            sondeur.observer(9, || Some(String::from("neuf"))),
            Some(Annonce::Texte(String::from("neuf")))
        );
    }
}
