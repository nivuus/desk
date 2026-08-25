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
/// de D5) et le garde de refus répété. **Depuis P2, c'est AUSSI lui qui porte
/// le garde n°1** (`apres_notre_ecriture`, armé par `armer_les_gardes` juste
/// en dessous) : un texte que nous venons d'écrire nous-mêmes n'arrive donc
/// jamais jusqu'ici. Refiltrer ici doublerait une décision
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

    // La mémoire de l'état courant, pour les fenêtres qui s'attacheront
    // ENSUITE (D-P3-2, moitié agent). Posée à CHAQUE annonce, refus compris —
    // voir le champ `Etat::dernier_presse_papier`, qui porte la raison et dit
    // pourquoi la symétrie avec `dernieres_parts` est trompeuse.
    garde.dernier_presse_papier = Some(match &texte {
        Some(t) => Annonce::Texte(t.clone()),
        None => Annonce::Refus { octets },
    });

    let sessions: Vec<String> = garde.canaux.keys().cloned().collect();
    let mut rompus = Vec::new();
    for session in sessions {
        let envoye = match garde.canaux.get(&session) {
            Some(canal) => canal
                .envoyer(Message::PressePapier { texte: texte.clone(), octets })
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

/// Émet l'état courant du presse-papier **sur le SEUL canal de la session qui
/// vient de s'inscrire**.
///
/// 🔴 **C'est la moitié AGENT du legs n°3 de P1**, et elle ne suffit PAS à
/// elle seule : `client/src/main.ts` mémorise le dernier `clipboard` reçu
/// avant l'attache, parce que ce message-ci tombe précisément dans l'intervalle
/// où `pressePapier` n'est pas encore assigné. **Livrer une moitié sans
/// l'autre ferait PARAÎTRE le défaut corrigé alors qu'il resterait
/// intermittent — et c'est pire qu'un défaut connu.**
///
/// ⚠️ **JAMAIS UN FAN-OUT.** `distribuer` pousse à toutes les fenêtres ; celle-ci
/// n'écrit que sur le canal neuf. Rejouer le contenu à toutes les fenêtres à
/// chaque attache serait un aller-retour par attache, et le garde n°3 côté
/// page n'y pourrait rien : il ne ferme que le renvoi vers l'agent.
///
/// ⚠️ **Un canal rompu n'est PAS traité ici**, à la différence de `distribuer`.
/// Il ne peut pas l'être : ce canal vient d'être inséré dans la même fonction,
/// son receveur est encore sur la pile de `inscrire`, et `envoyer` ne rend
/// `Err` que si le receveur a été lâché — ce qui n'a pas encore pu arriver.
/// Appeler `oublier` ici retirerait une session qui vient de naître.
///
/// ⚠️ **Le REFUS de file pleine ne peut pas s'y produire non plus** (borne de
/// `file.rs`, 25 août 2026) : la file de ce canal vient d'être créée, elle est
/// vide. Le `let _ =` ci-dessous ignore donc une valeur dont les deux cas
/// d'échec sont structurellement inatteignables ici, et c'est pour cela qu'il
/// reste juste.
pub(super) fn emettre_l_etat_courant(garde: &mut MutexGuard<'static, Etat>, session: &str) {
    let Some(annonce) = garde.dernier_presse_papier.clone() else {
        return;
    };
    let (texte, octets) = match annonce {
        Annonce::Texte(texte) => {
            let octets = texte.len() as u32;
            (Some(texte), octets)
        }
        Annonce::Refus { octets } => (None, octets),
    };
    // ⚠️ **JAMAIS LE TEXTE AU JOURNAL** (D-P1-7) : le contenu du presse-papier
    // est une ressource privée, et un journal versé dans git est public au
    // dépôt. On ne journalise que sa TAILLE, et le fait qu'il s'agisse d'un
    // refus — exactement comme `distribuer`.
    tracing::info!(%session, octets, refus = texte.is_none(),
        "etat courant du presse-papier emis a l'inscription");
    if let Some(canal) = garde.canaux.get(session) {
        let _ = canal.envoyer(Message::PressePapier { texte, octets });
    }
}

/// La SECONDE PRISE de D-P3-6 : consomme l'écriture arrivée APRÈS
/// `armer_les_gardes`, arme les gardes sur elle, et écarte l'annonce si c'est
/// la nôtre.
///
/// 🔴 **À appeler ENTRE `sondeur.tour()` et `presse_papier::distribuer`, et
/// l'ordre EST le mécanisme** — exactement comme `armer_les_gardes` doit
/// précéder `tour()`. La démonstration de la course, sa portée exacte et le
/// résidu qui subsiste vivent auprès de `Sondeur::ecarter_notre_ecriture`, qui
/// porte la règle ; ce qui est ici est sa BRANCHE sur ce registre, et rien
/// d'autre — même distinction que `distribuer` face au `Sondeur`, et que
/// `parts` face à `repartiteur`.
///
/// ⚠️ **Le verrou est pris et rendu ici, et il ne couvre aucune E/S** : même
/// discipline qu'`armer_les_gardes` juste au-dessus. Il ne couvre que la
/// lecture d'un champ.
pub(super) fn filtrer_nos_ecritures_tardives(
    sondeur: &mut Sondeur,
    annonce: Option<Annonce>,
) -> Option<Annonce> {
    let notre = etat().notre_ecriture.take();
    sondeur.ecarter_notre_ecriture(notre, annonce)
}

// Les tests de ce module vivent à part depuis le sous-bloc P3 du chantier
// presse-papier : le fichier était à 379 lignes pour un plafond de 500, et P3 y
// ajoute la mémoire `dernier_presse_papier` (D-P3-2), la seconde prise
// `filtrer_nos_ecritures_tardives` (D-P3-6) et leurs tests. L'extraction
// précède l'addition, comme la règle du dépôt l'exige.
//
// ⚠️ La déclaration est posée EN FIN DE FICHIER, jamais à la place qu'occupait
// le bloc — qui était AU MILIEU, le code de production reprenant juste après.
// C'est la place de tous les autres `#[path]` de tests du dépôt, et un lecteur
// qui cherche le code de production ne doit pas buter dessus.
//
// ⚠️ Cet emploi de `#[path]` est HORS de la portée de la « Convention de module
// enfant » de `CLAUDE.md` : même mécanisme Rust, autre raison — la règle des
// 500 lignes. Ce module ne se hisse PAS à la racine du crate.
#[cfg(test)]
#[path = "presse_papier/tests.rs"]
mod tests;
