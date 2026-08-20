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

/// `PRESSE_PAPIER_GARDE=0` désarme les gardes anti-écho de `apres_notre_ecriture`.
///
/// ⚠️ **VARIABLE DE BANC, JAMAIS UNE CONFIGURATION LIVRÉE** — même statut que
/// `PART_SONDAGE`. Elle existe pour un seul usage : rendre ATTEIGNABLE la rouge
/// du critère ④ de P2, qui compte les messages `clipboard` revenant vers la
/// fenêtre après un collage.
///
/// **`=0` DÉSARME ; une simple présence n'arme pas.** Les gardes sont armés par
/// défaut, et tester `is_ok()` les désarmerait en écrivant
/// `PRESSE_PAPIER_GARDE=0` pour... les désarmer. Convention de `PLEIN_ECRAN`,
/// `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE` et `PRESSE_PAPIER`.
///
/// 🔴 **ELLE DÉSARME LES DEUX GARDES, PAS LE SEUL N°1, ET C'EST LE POINT.**
/// La spécification prescrivait de désarmer le n°1 et d'attendre un compte qui
/// « croît sans borne » ; **il reste à un, et la spec avait prévu ce cas**.
/// Sans armement du n°2, le `Sondeur` relit notre texte, l'annonce **une**
/// fois, puis pose lui-même `dernier_emis` et `reference` — au tour suivant
/// `observer` sort sur sa première ligne. Et rien ne relance : le client
/// n'émet vers l'agent que sur un `paste`, donc sur un GESTE HUMAIN, jamais à
/// la réception d'un `clipboard`. Désarmer le seul n°1 rendrait donc **zéro
/// message aussi**, et la rouge serait vacueuse une seconde fois.
///
/// 🔵 **Conséquence de conception, et elle contredit une phrase de D5** : dans
/// l'architecture livrée, **aucune oscillation auto-entretenue n'est
/// possible**, chaque tour exigeant un geste humain. Ce que les gardes
/// suppriment est **un aller-retour par collage**, pas une divergence.
/// ⚠️ Déduit du code, pas d'une mesure : `client/src/presse-papier-dom.ts`
/// n'écrit que localement à la réception et n'émet rien. La condition qui
/// rendrait la boucle réelle est nommée — un client qui réémettrait ce qu'il
/// reçoit —, et c'est précisément ce que le garde n°3 empêche côté page.
fn gardes_armes() -> bool {
    static ARMES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ARMES.get_or_init(|| {
        let armes = std::env::var("PRESSE_PAPIER_GARDE").as_deref() != Ok("0");
        if !armes {
            tracing::warn!(
                "garde anti-echo du presse-papier DESARME (PRESSE_PAPIER_GARDE=0) : \
                 bras de banc, jamais une configuration livree"
            );
        }
        armes
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

/// `\n` → `\r\n`, la réciproque de `normaliser`. **Windows attend `\r\n`.**
///
/// Elle n'est PAS un `replace("\n", "\r\n")` : le texte qui arrive du
/// navigateur peut porter DÉJÀ des `\r\n` — un copier depuis un éditeur
/// Windows local en porte —, et le remplacement naïf rendrait alors `\r\r\n`,
/// donc une ligne vide de plus à chaque collage. La fonction est **idempotente**
/// exactement comme `normaliser` l'est dans l'autre sens, et l'aller-retour
/// `normaliser(denormaliser(x)) == x` est ce qu'un test doit voir rouge en
/// premier (spec §7.1).
pub fn denormaliser(texte: &str) -> String {
    let mut sortie = String::with_capacity(texte.len() + texte.len() / 16);
    let mut precedent_cr = false;
    for c in texte.chars() {
        match c {
            '\r' => {
                sortie.push_str("\r\n");
                precedent_cr = true;
            }
            '\n' => {
                // Le `\n` d'un `\r\n` a déjà été rendu par le `\r`.
                if !precedent_cr {
                    sortie.push_str("\r\n");
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

/// Borne le texte ENTRANT, en octets d'UTF-8. **On REFUSE, on ne tronque pas.**
///
/// ⚠️ **Ce n'est pas la même borne que celle du sens sortant, et l'asymétrie
/// est voulue.** Côté sortant, `PRESSE_PAPIER_MAX` protège le **canal de
/// contrôle** (D4) : le texte n'y est pas encore passé. Côté entrant, le texte
/// a **déjà** traversé ce canal quand l'agent le voit — la borne y protège le
/// tube capteur↔enfant et la mémoire, pas le canal. C'est le client qui doit
/// appliquer la sienne AVANT d'émettre ; celle-ci est la ceinture.
///
/// **Le refus entrant se journalise et ne remonte aucun bandeau** : le client a
/// déjà refusé et dit pourquoi, et un second bandeau pour le même geste serait
/// du bruit.
///
/// La borne porte sur `len()`, c'est-à-dire des **octets**, jamais sur
/// `chars().count()` : c'est l'unité du canal, et un texte d'emojis dont le
/// compte de caractères tient déborde de quatre fois en octets.
pub fn borner_entrant(texte: &str) -> Option<String> {
    (texte.len() <= PRESSE_PAPIER_MAX).then(|| texte.to_owned())
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

    /// Arme les gardes n°1 et n°2 de D5 **sur NOTRE PROPRE écriture**.
    ///
    /// À appeler juste après avoir écrit le presse-papier Windows nous-mêmes
    /// (sens navigateur → VM, sous-bloc P2). `seq` est le numéro de séquence
    /// relu **APRÈS `CloseClipboard`** — le relire avant rendrait un compteur
    /// que la fermeture peut encore faire bouger, et le garde n°1 serait faux
    /// d'un cran, c'est-à-dire silencieusement inopérant.
    ///
    /// Les **deux** champs sont posés, et chacun est un garde distinct :
    ///
    /// - `reference` **est le garde n°1** : au tour suivant, `observer` sort
    ///   sur sa première ligne et **ne rouvre même pas** le presse-papier ;
    /// - `dernier_emis` **est le garde n°2 armé sur notre écriture** : il
    ///   rattrape le cas où une écriture TIERCE se serait intercalée entre
    ///   notre `SetClipboardData` et cette relecture du compteur. Le numéro
    ///   relu n'est alors déjà plus le courant, le garde n°1 ne mord pas, et
    ///   c'est la comparaison de contenu qui empêche l'aller-retour.
    ///
    /// Le texte est **normalisé** avant d'être mémorisé, comme l'est celui que
    /// lit `observer` : sans cela le garde n°2 comparerait un texte à `\r\n`
    /// (ce que Windows nous rendra, puisque c'est `denormaliser` qui les y met)
    /// à un texte à `\n`, et ne reconnaîtrait jamais notre propre écriture.
    ///
    /// ⚠️ **Ce que cette méthode ne peut PAS faire**, et il faut le dire : si
    /// une autre copie survient entre notre écriture et le tour de roue qui
    /// consomme ce couple, poser `reference` sur *notre* `seq` ne la masque
    /// pas — le compteur aura encore bougé, et la copie tierce sera annoncée.
    /// **C'est le comportement voulu** : le garde reste exact au sens de D5, et
    /// un test le vérifie.
    pub fn apres_notre_ecriture(&mut self, seq: u32, texte: &str) {
        self.armer(gardes_armes(), seq, texte);
    }

    /// Le cœur d'`apres_notre_ecriture`, avec l'état du garde **injecté**.
    ///
    /// 🔴 **C'est ce qui rend le bras désarmé ÉPROUVABLE SUR L'HÔTE.**
    /// `gardes_armes()` est un `OnceLock` : un test ne peut ni le piloter ni le
    /// réinitialiser, et un contrôle écrit contre lui ne pourrait donc **pas
    /// rendre l'autre valeur** — c'est-à-dire pas échouer. Même patron que la
    /// fermeture de lecture d'`observer`, et pour la même raison.
    pub fn armer(&mut self, armes: bool, seq: u32, texte: &str) {
        // Le bras désarmé du critère ④ : voir `gardes_armes`, qui dit pourquoi
        // il désarme les DEUX et non le seul n°1.
        if !armes {
            return;
        }
        self.reference = Some(seq);
        self.dernier_emis = Some(normaliser(texte));
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

/// Écrit le presse-papier de la VM, et rend le numéro de séquence relu APRÈS
/// la fermeture — celui qu'il faut passer à `Sondeur::apres_notre_ecriture`.
///
/// **Jumelle exacte de `Sondeur::lire_la_plateforme`**, et posée au même
/// endroit pour la même raison : l'appelant (`capteur/sommeil/presse_papier.rs`)
/// n'est pas gaté et doit compiler sur l'hôte Linux.
///
/// ⚠️ **Le texte doit arriver DÉJÀ dénormalisé** (`\r\n`) : cette fonction ne
/// décide rien, elle transmet.
#[cfg(windows)]
pub fn ecrire_la_plateforme(texte: &str) -> anyhow::Result<u32> {
    win32::ecrire_texte(texte)
}

/// Repli non-Windows. **Un `Err`, jamais un `Ok`** : rendre `Ok(0)` ferait
/// croire à un succès, et l'appelant injecterait `Ctrl+V` sur un
/// presse-papier inchangé — c'est-à-dire collerait le contenu PRÉCÉDENT, le
/// mode de défaillance silencieux que D6 existe entièrement pour éviter.
#[cfg(not(windows))]
pub fn ecrire_la_plateforme(_texte: &str) -> anyhow::Result<u32> {
    anyhow::bail!("le presse-papier de la VM n'existe pas hors de Windows")
}

#[cfg(windows)]
mod win32;

// Les tests de ce module vivent à part depuis le sous-bloc P2 du chantier
// presse-papier : le fichier était à 428 lignes pour un plafond de 500, et P2 y
// ajoute le garde n°1 de D5, la réciproque de `normaliser` et leurs tests.
// L'extraction précède l'addition, comme la règle du dépôt l'exige.
//
// ⚠️ Cet emploi de `#[path]` est HORS de la portée de la « Convention de module
// enfant » de `CLAUDE.md` : c'est le même mécanisme Rust employé pour une autre
// raison — la règle des 500 lignes —, exactement comme `superviseur/table.rs`.
// Ce module ne se hisse PAS à la racine du crate.
#[cfg(test)]
#[path = "presse_papier/tests.rs"]
mod tests;
