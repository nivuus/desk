//! `Sondeur` — l'observation du presse-papier de la VM, et la décision de ce
//! qu'on annonce.
//!
//! **Extrait VERBATIM de `presse_papier.rs` au sous-bloc P3, AVANT l'addition
//! qui l'a rendu nécessaire** (la seconde prise de D-P3-6, tâche 5). Le parent
//! était à 441 lignes pour un plafond de 500, et la documentation d'un garde
//! y coûte trente lignes pour une fonction de six — `apres_notre_ecriture` en
//! porte vingt-neuf. La règle du dépôt est d'extraire AVANT d'ajouter, jamais
//! de comprimer après : D9 a payé deux compressions pour l'avoir oublié.
//!
//! ⚠️ **Ce module se déclare par un `mod sondeur;` ORDINAIRE chez son parent,
//! et la « Convention de module enfant » de `CLAUDE.md` ne s'applique pas** :
//! elle ne vise que les modules extraits d'un parent `#[cfg(windows)]` pour
//! que leur logique pure compile sur l'hôte, et `presse_papier.rs` n'est pas
//! gaté. Rien n'est hissé à la racine du crate.
//!
//! **Pur, sans aucun `cfg`** — hormis le seul `lire_la_plateforme`, qui est
//! l'aiguillage de plateforme et ne décide rien.

use std::time::Instant;

use super::{actif, gardes_armes, normaliser, Annonce, PERIODE_PRESSE_PAPIER, PRESSE_PAPIER_MAX};
#[cfg(windows)]
use super::win32;

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
    /// (`capteur/plein_ecran.rs`).
    ///
    /// ❌ **CETTE DOC AJOUTAIT « une fenêtre qui s'attache ne reçoit donc pas
    /// le contenu déjà présent ; elle reçoit la première copie QUI SUIT », ET
    /// LE SOUS-BLOC P3 L'A RÉFUTÉE.** C'était le legs n°3 de P1, et il est
    /// fermé : le registre mémorise la dernière annonce
    /// (`capteur/sommeil/registre.rs::Etat::dernier_presse_papier`) et
    /// l'émet à l'inscription sur le seul canal neuf.
    ///
    /// ⚠️ **Ce qui reste VRAI est la propriété de CE champ**, et elle est
    /// inchangée : le `Sondeur` n'annonce toujours rien à son premier tour.
    /// Ce qui a changé est ailleurs — c'est le REGISTRE qui rejoue, pas lui.
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
    ///
    /// ❌ **CETTE RÉSERVE ÉTAIT INCOMPLÈTE, ET LE SOUS-BLOC P3 L'A MESURÉ.**
    /// Elle ne traite que la copie **TIERCE**, qu'elle déclare voulue. Le cas
    /// de **NOTRE PROPRE SECONDE ÉCRITURE** — une deuxième fenêtre qui colle
    /// après cet armement et avant le tour — n'était déclaré NULLE PART, et il
    /// franchit les DEUX gardes : le n°1 parce que le compteur a rebougé, le
    /// n°2 parce que le texte mémorisé est celui du collage PRÉCÉDENT. Un test
    /// l'a vu ROUGE sur l'arbre intact, sans aucune mutation. Le remède est
    /// `ecarter_notre_ecriture`, plus bas dans ce même fichier.
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

    /// Écarte l'annonce que NOTRE PROPRE écriture vient de produire, et arme
    /// les gardes sur elle. **La SECONDE PRISE de D-P3-6.**
    ///
    /// 🔴 **LA COURSE QUE CETTE MÉTHODE FERME, ET ELLE A ÉTÉ MESURÉE AVANT
    /// D'ÊTRE FERMÉE** (sous-bloc P3, rouge sur l'arbre intact, sans aucune
    /// mutation — `journaux-presse-papier-p3/rouge-t5-d-p3-6-arbre-intact.log`).
    /// L'entrelacement, à deux fenêtres :
    ///
    /// 1. la fenêtre A colle → l'écriture pose `notre_ecriture = (seqA, textA)` ;
    /// 2. le tour de roue appelle `armer_les_gardes` : il PREND ce couple et
    ///    arme `reference = seqA`, `dernier_emis = textA` ;
    /// 3. la fenêtre B colle → `notre_ecriture = (seqB, textB)`, et le
    ///    presse-papier Windows porte désormais `textB` ;
    /// 4. `tour()` lit `seqB ≠ seqA` — le garde n°1 ne mord pas — puis lit
    ///    `textB ≠ textA` — le garde n°2 ne mord pas non plus — et
    ///    **`Annonce::Texte(textB)` part vers les N fenêtres** ;
    /// 5. au tour suivant, `armer_les_gardes` prend `(seqB, textB)` : trop tard.
    ///
    /// C'est exactement l'aller-retour par collage que les gardes de D5
    /// existent pour supprimer, et `apres_notre_ecriture` ne le couvre pas :
    /// sa réserve écrite traite le cas d'une copie **TIERCE** intercalée,
    /// qu'elle déclare voulu. Le cas ci-dessus est **notre propre seconde
    /// écriture**, et il n'était déclaré nulle part.
    ///
    /// ⚠️ **CE REMÈDE RÉTRÉCIT LA FENÊTRE, IL NE LA FERME PAS.**
    /// `capteur/sommeil/presse_papier::ecrire_avec` écrit le presse-papier
    /// **PUIS** pose `notre_ecriture` — le verrou y est délibérément pris
    /// APRÈS l'E/S Win32, parce que le tenir autour d'`OpenClipboard`
    /// bloquerait l'attache et le retrait de TOUTES les fenêtres. Si `tour()`
    /// lit le texte dans ce court intervalle, la seconde prise ne trouvera
    /// rien. Le résidu est de l'ordre d'une acquisition de mutex, et il est du
    /// **même genre** que celui qu'`apres_notre_ecriture` déclare déjà accepté.
    ///
    /// ⚠️ **Le filtre porte sur le TEXTE, jamais sur le seul `seq`**, et c'est
    /// un garde-fou, pas un détail : une copie TIERCE survenue après notre
    /// écriture porte elle aussi un `seq` postérieur, et filtrer sur le numéro
    /// ferait taire une vraie copie. Un test le tient.
    ///
    /// ⚠️ **Un `Annonce::Refus` n'est jamais écarté** : il ne porte pas de
    /// texte à comparer, et le refuser reviendrait à priver l'utilisateur du
    /// bandeau qui lui dit pourquoi rien n'est arrivé.
    pub fn ecarter_notre_ecriture(
        &mut self,
        notre: Option<(u32, String)>,
        annonce: Option<Annonce>,
    ) -> Option<Annonce> {
        self.ecarter(gardes_armes(), notre, annonce)
    }

    /// Le cœur d'`ecarter_notre_ecriture`, avec l'état du garde **injecté** —
    /// même patron, et pour la même raison, qu'`armer` face à
    /// `apres_notre_ecriture` : `gardes_armes()` est un `OnceLock` qu'un test
    /// ne peut ni piloter ni réinitialiser, et un contrôle écrit contre lui ne
    /// pourrait donc pas rendre l'autre valeur, c'est-à-dire pas échouer.
    ///
    /// 🔵 **`PRESSE_PAPIER_GARDE=0` désarme AUSSI cette prise**, et il le faut :
    /// cette variable de banc existe pour rendre atteignable la rouge du
    /// critère ④ de P2, qui compte les messages revenant vers la fenêtre après
    /// un collage. Une seconde prise qui écarterait quand même viderait ce
    /// bras de son sens.
    pub fn ecarter(
        &mut self,
        armes: bool,
        notre: Option<(u32, String)>,
        annonce: Option<Annonce>,
    ) -> Option<Annonce> {
        let Some((seq, texte)) = notre else { return annonce };
        if !armes {
            return annonce;
        }
        let notre_texte = normaliser(&texte);
        self.armer(armes, seq, &texte);
        match annonce {
            Some(Annonce::Texte(t)) if t == notre_texte => None,
            autre => autre,
        }
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
