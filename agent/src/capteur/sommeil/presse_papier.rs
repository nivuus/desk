//! Distribution du presse-papier de la VM — la branche de
//! `crate::presse_papier::Sondeur` sur le registre de `capteur::sommeil`.
//!
//! **Extrait de `sommeil.rs` et non ajouté dedans**, exactement comme
//! `parts.rs` et `porteurs.rs` : ce fichier-là est proche de son plafond, et
//! la règle du dépôt veut qu'une addition substantielle s'accompagne d'une
//! extraction.
//!
//! Il ne s'appelle pas comme le module racine `crate::presse_papier` par
//! hasard, mais il n'en porte pas la même chose : celui-là porte la RÈGLE
//! pure (normaliser, dénormaliser, borner, comparer au dernier émis, armer les
//! gardes) ; celui-ci ne porte que sa BRANCHE sur ce registre. Même distinction
//! que `repartiteur` / `parts` et que `audio` / `porteurs`.
//!
//! **Les DEUX sens y passent depuis le sous-bloc P2** : `distribuer` pousse aux
//! fenêtres ce que la VM a copié, `ecrire` écrit dans la VM ce qu'une fenêtre a
//! collé. Le second est ici et pas ailleurs parce que **le propriétaire du
//! presse-papier est le capteur, et lui seul** (D1) : un enfant qui écrirait
//! lui-même mettrait N processus en concurrence sur une ressource dont Windows
//! ne donne l'accès qu'à un seul à la fois.

use std::sync::MutexGuard;

use anyhow::Result;

use crate::presse_papier::{Annonce, Sondeur};

use super::{distribuer as distribuer_les_ordres, etat, oublier, Etat, Message};

/// Pousse une annonce de presse-papier à **toutes** les fenêtres inscrites.
///
/// **Toutes, et non la seule focalisée** : le presse-papier est une ressource
/// GLOBALE à la session Windows, chaque fenêtre navigateur a son propre
/// presse-papier local à alimenter, et c'est le client qui décide s'il écrit
/// (`PressePapierLocal::aEcrire`, qui prend le focus en argument). Décider ici
/// priverait une fenêtre non focalisée d'un contenu qu'elle devra écrire dès
/// qu'elle reprendra le focus — le dépôt différé de D3.
///
/// **Aucun filtre d'écrasement ici**, à la différence de
/// `parts::distribuer_les_parts` : le `Sondeur` n'appelle cette fonction qu'au
/// CHANGEMENT — c'est lui qui porte le garde d'égalité de contenu (garde n°2
/// de D5) et le garde de refus répété. Refiltrer ici doublerait une décision
/// déjà prise, et la doublerait *mal* : le registre ne connaît pas le texte
/// précédemment émis, et un second garde par session divergerait du premier
/// dès qu'une fenêtre s'inscrit ou se retire.
///
/// Un canal rompu passe par `oublier` — **le point de passage unique du
/// registre**, jamais un `remove` direct : c'est la leçon de M1 (revue finale
/// de branche de D6), où `focalisee` avait été oublié par deux chemins qui
/// retiraient à la main.
pub(super) fn distribuer(garde: &mut MutexGuard<'static, Etat>, annonce: Annonce) {
    let (texte, octets) = match annonce {
        Annonce::Texte(texte) => {
            let octets = texte.len() as u32;
            (Some(texte), octets)
        }
        Annonce::Refus { octets } => (None, octets),
    };

    // ⚠️ **UNE SEULE TRACE, ET JAMAIS LE TEXTE** (D-P1-7). Le contenu du
    // presse-papier est une ressource privée, et un journal versé dans git est
    // public au dépôt : on ne journalise que sa TAILLE et le fait qu'il
    // s'agisse d'un refus. Une seule, parce que deux traces au même instant se
    // comptent comme deux événements — piège maison de D6, où chaque
    // changement de barreau produisait deux lignes au même horodatage et
    // faisait valoir le double à tous les compteurs de recette.
    tracing::info!(octets, refus = texte.is_none(), "presse-papier de la VM");

    let sessions: Vec<String> = garde.canaux.keys().cloned().collect();
    let mut rompus = Vec::new();
    for session in sessions {
        let envoye = match garde.canaux.get(&session) {
            Some(canal) => canal
                .send(Message::PressePapier { texte: texte.clone(), octets })
                .is_ok(),
            None => false,
        };
        if !envoye {
            rompus.push(session);
        }
    }

    let mut ordres_du_retrait = Vec::new();
    for session in rompus {
        ordres_du_retrait.extend(oublier(garde, &session));
    }
    if !ordres_du_retrait.is_empty() {
        distribuer_les_ordres(garde, ordres_du_retrait);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;

    use crate::capteur::sommeil::tests::{premier_ordre, verrouiller_pour_le_test};
    use crate::capteur::sommeil::{etat, inscrire, retirer, signaler, Message};
    use crate::capteur::vivier::Ordre;
    use crate::presse_papier::Annonce;
    use crate::presse_papier::Sondeur;

    /// Le dernier presse-papier reçu sur un canal, en vidant ce qui s'y
    /// trouve : parts et ordres de sommeil s'y intercalent librement.
    fn dernier_presse_papier(canal: &Receiver<Message>) -> Option<(Option<String>, u32)> {
        canal
            .try_iter()
            .filter_map(|m| match m {
                Message::PressePapier { texte, octets } => Some((texte, octets)),
                _ => None,
            })
            .last()
    }

    /// **Toutes les fenêtres reçoivent, pas seulement la focalisée** : chaque
    /// fenêtre navigateur a son propre presse-papier local, et c'est le client
    /// qui décide s'il écrit maintenant ou au retour du focus.
    #[test]
    fn une_annonce_de_texte_part_vers_toutes_les_sessions_inscrites() {
        let _verrou = verrouiller_pour_le_test();
        let (canal_a, generation_a) = inscrire("pp-a", 7100);
        let (canal_b, generation_b) = inscrire("pp-b", 7101);
        signaler("pp-a", true, true);

        super::distribuer(&mut etat(), Annonce::Texte("bonjour".to_string()));

        assert_eq!(
            dernier_presse_papier(&canal_a),
            Some((Some("bonjour".to_string()), 7)),
            "la fenêtre focalisée doit recevoir le texte"
        );
        assert_eq!(
            dernier_presse_papier(&canal_b),
            Some((Some("bonjour".to_string()), 7)),
            "la fenêtre NON focalisée aussi : c'est le client qui décide d'écrire"
        );

        retirer("pp-a", generation_a);
        retirer("pp-b", generation_b);
    }

    /// Un refus voyage par la même variante, `texte` à `None` et `octets`
    /// portant la taille refusée — c'est ce qui permet au bandeau du
    /// navigateur de dire *combien* plutôt que « trop grand ».
    #[test]
    fn un_refus_part_sans_texte_mais_avec_sa_taille() {
        let _verrou = verrouiller_pour_le_test();
        let (canal, generation) = inscrire("pp-refus", 7200);

        super::distribuer(&mut etat(), Annonce::Refus { octets: 100_000 });

        assert_eq!(
            dernier_presse_papier(&canal),
            Some((None, 100_000)),
            "un refus doit partir, et porter sa taille"
        );

        retirer("pp-refus", generation);
    }

    /// Même remède, et pour la même raison, que
    /// `un_canal_rompu_detecte_par_les_parts_est_retire_du_vivier` : un fil de
    /// fenêtre qui meurt sans passer par `retirer` (une panique court-circuite
    /// le point de passage unique de `Fenetre::servir`) laisse une entrée dans
    /// `canaux` ET dans le vivier, où elle occuperait une place d'encodeur
    /// pour toute la vie du processus. `oublier` — jamais un `remove` direct —
    /// est ce qui retire les deux.
    ///
    /// ⚠️ **Ce test observe le vivier DIRECTEMENT, et c'est ce qui le rend
    /// discriminant.** Une première rédaction jugeait sur « une session neuve
    /// arrive-t-elle à s'éveiller » — et elle passait AVEC UN DISTRIBUTEUR
    /// VIDE : `inscrire` et `signaler` appellent tous deux
    /// `parts::distribuer_les_parts`, qui détecte la même rupture par son
    /// propre chemin et libère la place à la place de celui-ci. Le contrôle ne
    /// pouvait donc pas échouer — exactement le patron que ce dépôt paie
    /// depuis D6. Ici, rien ne s'intercale entre la rupture et l'observation.
    #[test]
    fn un_canal_rompu_detecte_par_le_presse_papier_est_retire_du_vivier() {
        let _verrou = verrouiller_pour_le_test();
        let (canal_mort, generation_morte) = inscrire("pp-mort", 7300);
        signaler("pp-mort", true, false);
        assert_eq!(
            premier_ordre(&canal_mort),
            Some(Ordre::Reveiller),
            "pp-mort devrait s'éveiller avant qu'on ne tue son fil"
        );

        // Le fil « meurt » : son récepteur est jeté SANS passer par `retirer`.
        drop(canal_mort);

        // Aucun appel public entre la rupture et l'observation : ni `inscrire`
        // ni `signaler`, qui détecteraient la rupture par le chemin des parts.
        super::distribuer(&mut etat(), Annonce::Texte("bonjour".to_string()));

        assert!(
            !etat().vivier.eveillees().iter().any(|s| s == "pp-mort"),
            "la session dont le canal est rompu doit être retirée du VIVIER, \
             pas seulement de `canaux` : sa place d'encodeur resterait sinon \
             occupée pour toute la vie du processus"
        );

        retirer("pp-mort", generation_morte);
    }

    // -----------------------------------------------------------------------
    // Sous-bloc P2 — l'écriture par le propriétaire, et l'armement des gardes.
    // -----------------------------------------------------------------------

    /// Un `Sondeur` qui a déjà pris sa référence : c'est l'état nominal après
    /// le premier tour, et le seul dans lequel les gardes se jugent.
    fn sondeur_amorce() -> Sondeur {
        let mut sondeur = Sondeur::nouveau();
        assert_eq!(sondeur.observer(1, || Some(String::from("etat-initial"))), None);
        sondeur
    }

    /// Le fil de fenêtre pose notre écriture dans `Etat` ; `armer_les_gardes`
    /// la consomme et arme le `Sondeur`. Le témoin n'est pas que rien ne soit
    /// annoncé — ce serait aussi vrai du garde n°2 — mais que le presse-papier
    /// ne soit **même pas rouvert** : la fermeture de lecture PANIQUE si elle
    /// est appelée.
    ///
    /// ROUGE si `armer_les_gardes` ne consomme rien, ou ne pose pas
    /// `reference`.
    ///
    /// 🔴 **CE QUE CE TEST NE COUVRE PAS, ET C'EST MESURÉ, PAS SUPPOSÉ.** Il
    /// établit que le mécanisme est juste **quand on l'appelle avant
    /// `tour()`** ; il n'établit **pas** que le tour de roue l'appelle bien
    /// dans cet ordre. Vérifié par mutation le 21 août 2026 : intervertir les
    /// deux lignes de `registre.rs::demarrer_le_tour_de_roue` laisse **les sept
    /// tests de ce module VERTS**. Le corps du tour de roue est une boucle
    /// infinie dans un `thread::spawn`, qu'aucun test d'hôte n'atteint —
    /// l'ordre y est un fait de LECTURE, et sa seule épreuve est la recette
    /// (critère ④, qui compte les messages de retour).
    #[test]
    fn armer_les_gardes_empeche_de_relire_notre_ecriture() {
        let _verrou = verrouiller_pour_le_test();
        etat().notre_ecriture = Some((42, String::from("colle")));

        let mut sondeur = sondeur_amorce();
        super::armer_les_gardes(&mut sondeur);

        assert_eq!(
            sondeur.observer(42, || panic!("le garde n°1 a laissé rouvrir le presse-papier")),
            None
        );
        // Consommée : un second armement ne trouve plus rien.
        assert!(etat().notre_ecriture.is_none());
    }

    /// 🔴 **Le garde reste EXACT au sens de D5, et c'est ce que ce test
    /// mesure.** Poser `reference` sur *notre* `seq` ne masque pas une copie
    /// TIERCE survenue depuis : le compteur a encore bougé, et cette copie doit
    /// être annoncée.
    ///
    /// ROUGE si l'armement posait un état « on se tait désormais ». Sans ce
    /// test, un garde trop large passerait le précédent.
    #[test]
    fn une_copie_tierce_survenue_apres_notre_ecriture_est_quand_meme_annoncee() {
        let _verrou = verrouiller_pour_le_test();
        etat().notre_ecriture = Some((42, String::from("colle")));

        let mut sondeur = sondeur_amorce();
        super::armer_les_gardes(&mut sondeur);

        assert_eq!(
            sondeur.observer(43, || Some(String::from("autre chose"))),
            Some(Annonce::Texte(String::from("autre chose")))
        );
    }

    /// 🔴 **Une écriture ÉCHOUÉE n'arme AUCUN garde**, et c'est le cas le plus
    /// dangereux : armer avant de savoir ferait sortir de l'observation un
    /// contenu qui n'a jamais atteint le presse-papier. Ce contenu deviendrait
    /// alors invisible **à jamais** — le tour suivant ne le verrait pas comme
    /// un changement.
    ///
    /// L'écrivain est INJECTÉ, exactement comme la fermeture de lecture de
    /// `Sondeur::observer` : c'est ce qui rend ce chemin éprouvable sur l'hôte
    /// sans le moindre `cfg`.
    ///
    /// ROUGE si `ecrire_avec` posait `notre_ecriture` avant d'appeler
    /// l'écrivain, ou si elle ignorait son `Err`.
    #[test]
    fn une_ecriture_echouee_n_arme_aucun_garde() {
        let _verrou = verrouiller_pour_le_test();
        etat().notre_ecriture = None;

        let resultat = super::ecrire_avec("colle", |_| anyhow::bail!("OpenClipboard refusé"));

        assert!(resultat.is_err(), "l'échec doit remonter à l'appelant");
        assert!(
            etat().notre_ecriture.is_none(),
            "rien ne doit être posé quand l'écriture a échoué"
        );
    }

    /// Le pendant : une écriture RÉUSSIE pose bien le couple, avec le numéro
    /// que l'écrivain a rendu — celui relu APRÈS `CloseClipboard`.
    ///
    /// ROUGE si `ecrire_avec` posait un numéro fabriqué au lieu de celui de
    /// l'écrivain : le garde n°1 serait alors faux d'un cran, c'est-à-dire
    /// silencieusement inopérant.
    #[test]
    fn une_ecriture_reussie_pose_le_numero_rendu_par_l_ecrivain() {
        let _verrou = verrouiller_pour_le_test();
        etat().notre_ecriture = None;

        super::ecrire_avec("colle", |texte| {
            assert_eq!(texte, "colle", "le texte doit arriver tel quel à Win32");
            Ok(1234)
        })
        .expect("écriture");

        assert_eq!(
            etat().notre_ecriture.clone(),
            Some((1234, String::from("colle")))
        );
        etat().notre_ecriture = None;
    }
}

/// Écrit `texte` dans le presse-papier de la VM, et **arme les gardes dans le
/// même geste**.
///
/// Appelée depuis le fil de FENÊTRE qui sert `VersCapteur::PressePapierEcrire`.
/// Le texte arrive déjà normalisé, borné et dénormalisé par l'enfant.
///
/// 🔴 **`PRESSE_PAPIER=0` interdit AUSSI l'écriture**, et pas seulement la
/// lecture. Le garde est ici et non dans `crate::presse_papier` parce que la
/// symétrie qui compte est celle du PROPRIÉTAIRE : `Sondeur::tour` teste
/// `actif()` avant toute lecture, ce chemin le teste avant toute écriture, et
/// « le mécanisme entier est désarmé » cesse d'être une demi-vérité.
pub(super) fn ecrire(texte: &str) -> Result<()> {
    ecrire_avec(texte, crate::presse_papier::ecrire_la_plateforme)
}

/// Le cœur de `ecrire`, avec son écrivain **injecté** — c'est ce qui le rend
/// éprouvable sur l'hôte, exactement comme `Sondeur::observer` reçoit sa
/// fermeture de lecture.
///
/// 🔴 **Rien n'est posé quand l'écriture ÉCHOUE**, et c'est ce qui compte le
/// plus ici : armer avant de savoir ferait sortir de l'observation un contenu
/// qui n'a jamais atteint le presse-papier, et ce contenu deviendrait alors
/// invisible **à jamais** — le tour suivant ne le verrait pas comme un
/// changement.
pub(super) fn ecrire_avec(
    texte: &str,
    ecrivain: impl FnOnce(&str) -> Result<u32>,
) -> Result<()> {
    if !crate::presse_papier::actif() {
        anyhow::bail!("presse-papier desarme (PRESSE_PAPIER=0)");
    }
    let seq = ecrivain(texte)?;
    // ⚠️ Le verrou n'est pris QU'APRÈS l'E/S Win32, jamais autour d'elle :
    // `OpenClipboard` est une ressource contendue de la station de fenêtres,
    // et la tenir sous le verrou global du registre bloquerait l'attache et le
    // retrait de TOUTES les fenêtres pendant ce temps. Même raison, et même
    // discipline, que le sondage `hors du verrou` du tour de roue.
    etat().notre_ecriture = Some((seq, texte.to_owned()));
    Ok(())
}

/// Consomme NOTRE écriture en attente et arme les gardes du `Sondeur`.
///
/// 🔴 **À appeler AVANT `sondeur.tour()`, et l'ordre EST le mécanisme.**
/// Après, le tour aurait déjà lu le presse-papier, y aurait trouvé notre
/// propre texte, et l'aurait renvoyé aux fenêtres : un aller-retour par
/// collage, exactement ce que les gardes de D5 existent pour supprimer.
pub(super) fn armer_les_gardes(sondeur: &mut Sondeur) {
    // Le verrou est pris et rendu ici, avant l'E/S de `tour()` : il ne couvre
    // que la lecture d'un champ.
    let notre = etat().notre_ecriture.take();
    if let Some((seq, texte)) = notre {
        sondeur.apres_notre_ecriture(seq, &texte);
    }
}
