//! Le fil du tour de roue : la cadence du ré-arbitrage, et le sondage du
//! presse-papier HORS du verrou global.
//!
//! **Extrait de `registre.rs` dans le round de correction 1 (25 août 2026),
//! parce que ses correctifs y ont porté le fichier à 508 lignes pour un
//! plafond de projet à 500.** Extraire, jamais comprimer — même motif et même
//! montage que `parts.rs`, `porteurs.rs` et `presse_papier.rs`, les trois
//! voisins déjà extraits de `sommeil.rs` pour cette raison.
//!
//! **Ce qui a guidé la COUPE, et non la seule arithmétique** : `registre.rs`
//! garde *le registre lui-même* — l'état, son point d'accès, et les quatre
//! opérations qui le touchent (`distribuer`, `oublier`, `inscrire`,
//! `retirer`). Ce fichier-ci est *le FIL qui les appelle en cadence*, ce qui
//! n'est pas la même responsabilité : il porte une horloge et une E/S Win32,
//! le registre n'en a aucune.
//!
//! **Transposition, pas réécriture** : le bloc est déplacé à l'identique,
//! aucune valeur, aucun ordre d'opération, aucune signature n'a changé — seule
//! la visibilité de `demarrer_le_tour_de_roue` passe à `pub(super)` pour
//! rester atteignable depuis `registre.rs`, qui l'appelle.

use std::time::Instant;

use super::{distribuer, etat, parts, porteurs, presse_papier, purger_les_inaptitudes, PERIODE_REARBITRAGE};

/// **Un seul fil pour tout le processus**, démarré à la première inscription.
///
/// **La sûreté ne tient pas au `sleep` ci-dessous.** Ce fil est lancé DEPUIS la
/// fermeture d'initialisation de `ETAT.get_or_init` ; c'est
/// `OnceLock::get_or_init` lui-même qui garantit qu'un second fil appelant
/// `etat()` pendant que cette fermeture tourne encore **bloque** jusqu'à ce
/// qu'elle se termine — la réentrance qui paniquerait serait celle du *même*
/// fil, qui n'a pas lieu ici. Le `sleep` n'est qu'une cadence, pas une garde.
pub(super) fn demarrer_le_tour_de_roue() {
    std::thread::spawn(|| {
        let mut sondeur = crate::presse_papier::Sondeur::nouveau();
        loop {
            std::thread::sleep(PERIODE_REARBITRAGE);

            // 🔴 **HORS DU VERROU, ET C'EST TOUT L'INTÉRÊT DE CETTE LIGNE.**
            // `sondeur.tour()` fait une E/S Win32 — `GetClipboardSequenceNumber`,
            // puis `OpenClipboard`/`GetClipboardData` quand le compteur a bougé.
            // `OpenClipboard` est une ressource CONTENDUE de la station de
            // fenêtres : il échoue, ou attend, dès qu'une autre application la
            // tient. Placée sous `etat()` — le verrou GLOBAL du registre, un
            // unique `Mutex<Etat>` pour tout le processus —, elle bloquerait
            // pendant tout ce temps `inscrire`, `retirer`, `signaler` et
            // `echec_de_reveil`, c'est-à-dire l'attache et le retrait de TOUTES
            // les fenêtres, et le retour d'un réveil refusé.
            //
            // La spécification place le sondage « sur le tour de roue » sans
            // dire de quel côté du verrou ; c'est le plan (D-P1-3, divergence
            // E3) qui a tranché, et c'est un défaut corrigé avant d'exister.
            //
            // Seul le RÉSULTAT — une `Annonce` déjà normalisée, bornée et
            // dédupliquée — entre sous le verrou, plus bas.
            //
            // Le garde d'armement `presse_papier::actif()` vit à l'intérieur de
            // `tour()`, AVANT toute lecture : `PRESSE_PAPIER=0` empêche donc
            // jusqu'à la lecture du compteur, pas seulement l'envoi. Le
            // dupliquer ici doublerait une décision déjà prise au bon endroit.
            //
            // ⚠️ **Le sens INVERSE le teste une seconde fois, et ce n'est PAS
            // le doublon que la phrase ci-dessus interdit** : `actif()` y garde
            // un autre point de décision — l'ÉCRITURE, servie depuis un fil de
            // fenêtre (`sommeil::presse_papier::ecrire_avec`). Sans lui,
            // `PRESSE_PAPIER=0` couperait la lecture et laisserait l'écriture,
            // et « le mécanisme entier est désarmé » serait une demi-vérité.
            // 🔴 **AVANT `tour()`, et l'ordre EST le mécanisme** (sous-bloc
            // P2). Consomme l'écriture que le fil de FENÊTRE a posée dans
            // `Etat` en servant un collage, et arme sur elle les gardes n°1 et
            // n°2 de D5. Placée après `tour()`, elle arriverait trop tard : le
            // tour aurait déjà relu notre propre texte et l'aurait renvoyé aux
            // fenêtres.
            presse_papier::armer_les_gardes(&mut sondeur);

            let annonce = sondeur.tour();

            // 🔴 **LA SECONDE PRISE (D-P3-6), APRÈS `tour()` ET AVANT
            // `distribuer`.** `armer_les_gardes` ci-dessus a consommé
            // l'écriture qui EXISTAIT avant le tour ; celle-ci consomme celle
            // qui est ARRIVÉE PENDANT. Sans elle, un second collage survenu
            // entre l'armement et la lecture passe les DEUX gardes de D5 — le
            // n°1 parce que le compteur a rebougé, le n°2 parce que le texte
            // mémorisé est celui du collage PRÉCÉDENT — et son propre texte
            // repart vers les N fenêtres.
            //
            // La course a été MESURÉE avant d'être fermée, par un test rouge
            // sur l'arbre intact et sans aucune mutation ; sa démonstration et
            // le résidu qui subsiste vivent auprès de
            // `Sondeur::ecarter_notre_ecriture`.
            let annonce = presse_papier::filtrer_nos_ecritures_tardives(&mut sondeur, annonce);

            let mut garde = etat();
            let maintenant = Instant::now();
            let ordres = garde.vivier.rearbitrer(maintenant);
            distribuer(&mut garde, ordres);
            parts::distribuer_les_parts(&mut garde);
            purger_les_inaptitudes(&mut garde.inaptes, Instant::now());
            porteurs::distribuer_l_audio(&mut garde);
            if let Some(annonce) = annonce {
                presse_papier::distribuer(&mut garde, annonce);
            }
        }
    });
}
