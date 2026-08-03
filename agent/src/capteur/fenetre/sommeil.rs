//! Les deux transitions du fil de fenêtre : relâcher sa source, la
//! reconstruire.
//!
//! ⚠️ **À ne pas confondre avec `crate::capteur::sommeil`**, qui est le registre
//! GLOBAL du processus : celui-là *décide* qui dort, celui-ci *exécute* la
//! décision pour une fenêtre. Ils portent le même nom parce qu'ils parlent de
//! la même chose de part et d'autre d'un canal `mpsc`, et le module parent
//! nomme donc toujours le registre par son chemin complet.
//!
//! **Extrait de `fenetre.rs` et non ajouté dedans** : le sous-bloc D5 y aurait
//! porté le fichier au-delà du plafond de 500 lignes du projet. Le dépôt a le
//! précédent (`vivier.rs` et `vivier/tests.rs`), et la règle qui l'impose est
//! « extraction, jamais compression ».

use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::time::Instant;

use anyhow::{Context, Result};

use crate::capteur::protocole::DepuisCapteur;
use crate::capteur::vivier::Ordre;
use crate::source::VideoSource;
use crate::windows_source::WindowsSource;

use super::commandes::deposer;
use super::{AEcrire, Contexte, Fenetre, Fin};

impl Fenetre {
    /// Relâche l'encodeur et la duplication. **Sur CE fil**, jamais ailleurs :
    /// `Drop for H264Encoder` peut geler (risque observé, non attribué), et
    /// ici il ne gèlerait que cette fenêtre.
    fn dormir(&mut self) {
        if self.source.take().is_some() {
            tracing::info!(
                session = %self.session,
                "fenêtre endormie, encodeur et duplication relâchés"
            );
        }
    }

    /// Reconstruit la source à l'identique, puis force une image clé.
    ///
    /// **Peut échouer, et c'est le cas NOMINAL** quand le plafond matériel
    /// d'encodeurs est atteint : l'appelant doit alors le dire au vivier (voir
    /// `appliquer_les_ordres`).
    ///
    /// `sur_sortie` retente la duplication pendant `DUREE_FENETRE_OUVERTURE` :
    /// c'est le chemin de reprise du sous-bloc D2, et le réveil l'emprunte donc
    /// sans avoir à réapprendre la même leçon — une sortie créée par le
    /// superviseur au même instant fait abandonner le mutex des duplications
    /// voisines.
    ///
    /// `duree_ms` est journalisée parce que le délai de réveil est l'un des
    /// relevés attendus de la recette, et qu'il n'existe aucun autre endroit
    /// où le prendre côté agent.
    fn reveiller(&mut self) -> Result<()> {
        if self.source.is_some() {
            return Ok(());
        }
        let debut = Instant::now();
        let p = &self.parametres;
        let mut source =
            WindowsSource::sur_sortie(p.hwnd, &p.sortie, p.fps, p.debit, p.clock_origin)
                .with_context(|| format!("réveil de la session {}", self.session))?;
        // `sur_sortie` en demande déjà une à la construction. Ce second appel
        // est une ceinture : sans image clé, le décodeur du navigateur n'aurait
        // aucun point d'entrée dans le flux neuf et rendrait un écran gris
        // jusqu'à la prochaine — le groupe d'images de l'encodeur matériel est
        // ouvert. Le réveil ne dépend ainsi d'aucun détail de `sur_sortie`.
        source.request_keyframe().context("image clé au réveil")?;
        let (largeur, hauteur) = source.dimensions();
        self.largeur = largeur;
        self.hauteur = hauteur;
        self.source = Some(source);
        tracing::info!(
            session = %self.session,
            largeur,
            hauteur,
            duree_ms = debut.elapsed().as_millis() as u64,
            "fenêtre réveillée"
        );
        Ok(())
    }

    /// Applique les ordres du vivier en attente.
    ///
    /// **Appelée en tête de tour, avant tout le reste** : dormir libère un
    /// encodeur, et il n'y a aucune raison d'en solliciter un de plus quand
    /// l'ordre de le rendre est déjà là.
    ///
    /// ⚠️ Le contrat « tous les `Dormir` précèdent tout `Reveiller` » du vivier
    /// ne vaut qu'à l'ÉMISSION : les fils de fenêtre sont indépendants et
    /// consomment des canaux distincts, donc rien n'ordonne le TRAITEMENT entre
    /// deux fenêtres. Aucune décision ici ne suppose qu'un sommeil voisin a
    /// déjà eu lieu ; le filet est `echec_de_reveil`, qui fait reproposer un
    /// réveil arrivé trop tôt.
    pub(super) fn appliquer_les_ordres(
        &mut self,
        ordres: &Receiver<Ordre>,
        ecritures: &SyncSender<AEcrire>,
        ctx: &Contexte,
    ) -> Fin {
        loop {
            match ordres.try_recv() {
                Ok(Ordre::Dormir(raison)) => {
                    self.dormir();
                    let raison = crate::capteur::sommeil::raison_en_texte(raison);
                    let etat = DepuisCapteur::Sommeil { endormie: true, raison: raison.into() };
                    // `deposer` et non un `send` bloquant : la file peut être
                    // pleine, et attendre dessus sans servir les commandes
                    // recréerait l'interblocage à six maillons de la tâche 10
                    // du sous-bloc D4.
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(etat), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
                Ok(Ordre::Reveiller) => {
                    if let Err(erreur) = self.reveiller() {
                        tracing::warn!(
                            session = %ctx.session,
                            %erreur,
                            "réveil refusé, la fenêtre reste endormie"
                        );
                        // **Indispensable, et rien d'autre ne le remplace.** Le
                        // vivier pose `eveillee = true` AVANT que le réveil ait
                        // lieu : sans ce chemin de retour il croirait la fenêtre
                        // éveillée pour toujours, ne rendrait jamais sa place et
                        // ne la reproposerait jamais — fenêtre perdue
                        // définitivement, pour un refus qui est le cas nominal
                        // quand le plafond matériel est atteint. L'appel est
                        // court et hors de tout emprunt sur `self.source` : le
                        // registre prend un verrou global.
                        crate::capteur::sommeil::echec_de_reveil(ctx.session);
                        continue;
                    }
                    let etat = DepuisCapteur::Sommeil { endormie: false, raison: String::new() };
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(etat), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
                Err(TryRecvError::Empty) => return Fin::Continuer,
                // Le registre a laissé tomber notre émetteur : la fenêtre n'est
                // plus arbitrée. On continue de servir plutôt que de clore —
                // perdre l'arbitrage n'est pas perdre la session.
                Err(TryRecvError::Disconnected) => return Fin::Continuer,
            }
        }
    }
}
