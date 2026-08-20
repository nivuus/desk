//! Le collage venu du navigateur : écrire le presse-papier de la VM, puis
//! injecter `Ctrl+V` — **dans cet ordre, et jamais l'inverse**.
//!
//! **Extrait de `tick.rs` AVANT que la branche `a1octies` n'y tienne**
//! (sous-bloc P2) : ce fichier-là était à 421 lignes pour un plafond de 500, et
//! le corps de la branche en fait plus de soixante-dix — il porte tout le
//! raisonnement de D6. Le mettre en ligne l'aurait porté à 493, marge 7, sur un
//! fichier qui compte déjà onze branches et soixante lignes d'audit
//! d'invariant. La règle du dépôt est d'extraire AVANT d'ajouter.
//!
//! 🔴 **LES DEUX MOITIÉS DE L'ORDRE DE D6 VIVENT ICI, et c'est le point.**
//! `traiter_le_collage` écrit et arme ; `injecter_le_collage` consomme et
//! frappe. Elles sont appelées depuis deux fichiers différents —
//! `tick::act_on_timeout` pour la première, `boucle::run` pour la seconde —
//! parce que seule `run` reçoit `on_input` (D-P2-1) ; les lire à deux fichiers
//! d'écart rendrait l'ordre invisible.

use proto::input::InputMessage;

use super::Session;

impl Session {
    /// Écrit `texte` dans le presse-papier de la VM, puis **arme** l'injection.
    ///
    /// Trois pas, dans cet ordre exact et pour cette raison : écrire d'abord —
    /// **SYNCHRONE**, `ecrire_le_presse_papier` attend le `Fait` du capteur —,
    /// n'armer qu'ENSUITE, et seulement si l'écriture a **réussi**. Aucun
    /// ordonnancement de canal n'entre là-dedans : l'ordre est garanti par
    /// construction.
    ///
    /// 🔴 **Sur `Err`, on n'arme PAS**, et c'est tout le sujet : un `Ctrl+V` sur
    /// un presse-papier inchangé collerait le contenu **PRÉCÉDENT**, sans que
    /// rien ne le dise à l'utilisateur. D6 prescrit littéralement l'inverse —
    /// « si le presse-papier ne peut pas être écrit, la touche `V` est PERDUE,
    /// pas reportée ».
    ///
    /// **Le texte est normalisé, borné, puis dénormalisé, dans CET ordre** — le
    /// même que le sens sortant (D-P1-2). Borner d'abord refuserait un texte
    /// qui, une fois les `\r\n` ramenés à `\n`, tiendrait ; borner APRÈS la
    /// dénormalisation refuserait un texte que la VM venait d'accepter dans
    /// l'autre sens, un aller-retour l'ayant gonflé d'un `\r` par ligne.
    ///
    /// **Un refus ne remonte AUCUN message au navigateur** : le client a déjà
    /// sa propre borne et son bandeau, et un second refus pour le même geste
    /// serait du bruit. Il est journalisé, jamais tu.
    pub(super) fn traiter_le_collage(&mut self, texte: &str) {
        let normalise = crate::presse_papier::normaliser(texte);
        match crate::presse_papier::borner_entrant(&normalise) {
            None => tracing::warn!(
                session = %self.session_id,
                octets = normalise.len(),
                borne = crate::presse_papier::PRESSE_PAPIER_MAX,
                "collage refusé : au-dessus de la borne, il n'est ni tronqué ni écrit"
            ),
            Some(borne) => {
                let pour_windows = crate::presse_papier::denormaliser(&borne);
                match self.source.ecrire_le_presse_papier(&pour_windows) {
                    // ⚠️ **JAMAIS LE TEXTE AU JOURNAL** (D-P1-7) : le
                    // contenu du presse-papier est une ressource privée, et
                    // un journal versé dans git est public au dépôt. Seule
                    // sa TAILLE est journalisée, et une seule ligne — deux
                    // traces au même instant se comptent comme deux
                    // événements (piège maison de D6).
                    Ok(()) => {
                        tracing::debug!(
                            session = %self.session_id,
                            octets = borne.len(),
                            "collage écrit dans le presse-papier de la VM"
                        );
                        self.collage_a_injecter = true;
                    }
                    Err(erreur) => tracing::warn!(
                        session = %self.session_id,
                        erreur = %format!("{erreur:#}"),
                        "collage NON écrit : la touche V est perdue, pas reportée"
                    ),
                }
            }
        }
    }

    /// Injecte les quatre touches d'un collage si `traiter_le_collage` les a
    /// armées. **Consomme le drapeau** : sans cela, `Ctrl+V` partirait à CHAQUE
    /// tour de boucle, c'est-à-dire à la cadence vidéo.
    ///
    /// Aucune méthode `coller()` n'est ajoutée à `InputInjector` : son bras
    /// `InputMessage::Key` appelle **déjà** `au_premier_plan()`, qui vérifie le
    /// retour de `SetForegroundWindow` et le journalise une fois par
    /// basculement. Réutiliser `InputMessage::Key` en hérite gratuitement ;
    /// écrire un second chemin le dupliquerait.
    pub(super) fn injecter_le_collage(&mut self, on_input: &mut impl FnMut(InputMessage)) {
        if !self.collage_a_injecter {
            return;
        }
        self.collage_a_injecter = false;
        for touche in crate::input::TOUCHES_COLLAGE {
            on_input(touche);
        }
    }
}

#[cfg(test)]
#[path = "collage/tests.rs"]
mod tests;
