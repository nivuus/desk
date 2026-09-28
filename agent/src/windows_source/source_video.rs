//! `impl VideoSource for WindowsSource`: one capture attempt per call, and the
//! short bounded retry kept for the encoder's very first start.

use super::*;

/// Budget accordé à l'attente de la sortie d'une image **qui vient d'être
/// soumise** à l'encodeur, **uniquement tant qu'il n'a encore rien produit**
/// (voir `WindowsSource::encoder_warmed_up`) — jamais à l'attente d'une
/// nouvelle capture, et jamais non plus une fois l'encodeur établi comme
/// capable de répondre.
///
/// L'encodeur matériel est asynchrone (voir `encode.rs`) : après le tout
/// premier `submit()`, `poll_output()` n'a presque jamais encore de résultat
/// au premier essai, l'événement `METransformHaveOutput` mettant un ou deux
/// cycles à arriver. Sans ce court réessai, la toute première image (donc le
/// premier keyframe) n'était récupérée qu'au tour suivant de `Session::run`
/// (~16,7 ms plus tard) au mieux. Borné à quelques dizaines de millisecondes :
/// largement suffisant pour ce démarrage, sans jamais s'approcher de la
/// seconde qui affamait `Session::run` (ronde de correction 1).
///
/// **Ronde de diagnostic (débit plafonné ~25-30 im/s) :** repéré en relecture
/// que l'inverse de la valeur alors en vigueur (40 ms) tombait exactement sur
/// le plafond observé — hypothèse d'un plafond ARTIFICIEL si ce réessai
/// bloquait `act_on_timeout` (donc tout `Session::run`, capture ET
/// transport) sur la quasi-totalité des tours en régime établi. Mesuré
/// directement (instrumentation temporaire, retirée) : FAUX sur cette VM. Le
/// réessai résolvait systématiquement en 3-5 ms (jamais le budget de 40 ms
/// atteint, sur des centaines d'images), et réduire le budget de 40 ms à
/// 2 ms (vingt fois moins) n'a strictement rien changé au débit mesuré côté
/// navigateur (297/10 s dans les deux cas, contenu identique). À ce stade du
/// diagnostic, le plafond réel était attribué en amont : `DesktopCapture::next_frame`
/// (donc `AcquireNextFrame`, non bloquant) ne signalait une image neuve qu'à
/// ~30 Hz, alors que la boucle l'interroge, elle, à 60 Hz exact (mesuré par
/// comptage — voir aussi `docs/superpowers/plans/fix-debit-socket-report.md`),
/// ce qui avait fait suspecter la cadence de composition/duplication du
/// bureau elle-même comme vraie limite.
///
/// **Hypothèse écartée depuis**, par la recette du jalon 1
/// (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`, critère 2) :
/// mesurée isolément (`CAPTURE_TEST`), la capture soutient ~90 im/s sur cette
/// même VM — la composition/duplication du bureau n'est pas le goulot. Le
/// plafond réel se situe côté encodeur matériel : `H264Encoder::submit`, dans
/// `encode.rs`, ne reçoit de nouvelles demandes d'entrée
/// (`METransformNeedInput`) qu'à ~30 Hz, alors que le même encodeur, sollicité
/// en boucle serrée (`ENCODE_TEST`), soutient ~80 im/s — une interaction non
/// résolue entre le rythme de soumission fixe (16,7 ms) et le rythme propre
/// du MFT matériel, pas une limite de la capture ni, en tant que telle, du
/// GPU/pilote NVIDIA (détail des essais qui écartent successivement les
/// hypothèses concurrentes dans
/// `docs/superpowers/plans/diagnostic-plafond-debit.md` et
/// `docs/superpowers/plans/remesure-debit.md`).
///
/// `SUBMIT_POLL_BUDGET` n'y est pour rien — mais
/// comme il ne coûtait donc jamais rien qu'au tout premier démarrage
/// (jamais revérifié une fois l'encodeur chaud), il est désormais borné à
/// ce seul cas : sur du matériel où l'encodeur répondrait plus lentement en
/// régime établi, l'ancienne version aurait pu réellement brider `run()`
/// jusqu'à ce budget à chaque image — ce que cette restriction élimine
/// structurellement, sans rien changer au débit mesuré ici.
const SUBMIT_POLL_BUDGET: std::time::Duration = std::time::Duration::from_millis(40);
const SUBMIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

impl VideoSource for WindowsSource {
    /// Un seul essai de capture par appel, jamais d'attente pour une
    /// nouvelle image — avec un court réessai borné, réservé au tout
    /// premier démarrage de l'encodeur, pour en récupérer la sortie.
    ///
    /// **Ronde de correction 1 (revue), 1er correctif :** une première
    /// version de cette méthode retentait en boucle (sommeil de 1 ms)
    /// jusqu'à DEUX SECONDES avant d'abandonner, **que quelque chose ait été
    /// capturé ou non**. Deux défauts en découlaient, tous deux mesurés par
    /// la relecture : (1) elle ne distinguait pas « l'encodeur démarre » (le
    /// vrai bug visé) de « rien n'a bougé à l'écran » (le cas nominal d'une
    /// capture en direct — `DesktopCapture::next_frame` documente elle-même
    /// ce cas comme « courant et normal ») ; toute page statique ou tout
    /// instant sans mouvement de plus de deux secondes coupait donc le flux ;
    /// (2) pendant qu'elle bouclait, le fil unique de `Session::run` ne
    /// traitait plus ni ICE, ni RTCP, ni les canaux de données — observé en
    /// pratique par une déconnexion ICE spontanée ~20 s après la
    /// négociation.
    ///
    /// **2e correctif, après mesure :** supprimer TOUT réessai (un essai
    /// unique, quoi qu'il arrive) réglait bien les deux défauts ci-dessus,
    /// mais dégradait fortement le débit observé côté navigateur au tout
    /// démarrage : la cadence externe de `Session::run` (~16,7 ms) est trop
    /// grossière pour rattraper à temps la sortie de la toute première image
    /// soumise, avant que l'encodeur ait prouvé qu'il répond vite. Le
    /// réessai réapparaît donc, mais borné à `SUBMIT_POLL_BUDGET`.
    ///
    /// **3e correctif, après diagnostic du plafond de débit (voir
    /// `SUBMIT_POLL_BUDGET`) :** ce réessai avait fini par s'appliquer à
    /// *chaque* image soumise, pas seulement à la première — sans
    /// conséquence mesurée sur cette VM (il ne consommait jamais son budget
    /// en régime établi) mais restant un risque latent sur du matériel plus
    /// lent, où il aurait réellement bridé `run()` à `1/SUBMIT_POLL_BUDGET`.
    /// Désormais réservé à la phase de démarrage (`encoder_warmed_up`) :
    /// une fois l'encodeur prouvé capable de répondre, chaque soumission ne
    /// fait plus qu'un seul essai immédiat, exactement comme le cas « rien
    /// de neuf à capturer » ci-dessous — une sortie non encore prête sort au
    /// tour suivant, 16,7 ms plus tard, sans jamais bloquer celui-ci.
    ///
    /// Quand rien n'a été capturé (cas normal, bureau immobile), retour
    /// immédiat, sans boucle ni attente, comme l'exige la revue. `None` ne
    /// signifie donc jamais « rien cette fois » ; il reste possible pour
    /// deux causes réellement définitives (`is_exhausted` en informe
    /// l'appelant) : la fenêtre a disparu, ou une erreur de capture non
    /// récupérable s'est produite.
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.telemetrie.tick();
        if self.fatal {
            // Une reconstruction de la chaîne par `resize` a pu échouer au
            // point de ne laisser aucune capture de secours valide non plus
            // (voir `RebuildOutcome::Fatal` dans `resize`) : `self.capture`
            // vaut alors `None` pour de bon. Ne JAMAIS appeler
            // `capture_mut()` dans ce cas — `is_exhausted()` (déjà vraie via
            // `self.fatal`) fera clore la session proprement au tour
            // suivant, plutôt qu'un panic sur le champ vide.
            return None;
        }

        // Alimenter l'encodeur avec l'image la plus récente, si le bureau a
        // changé depuis le dernier appel (Desktop Duplication ne rend une
        // image que sur changement — cas courant et normal, voir
        // capture.rs).
        let mut submitted = false;
        let region = self.region;
        let t_capture = std::time::Instant::now();
        let captured = self.capture_mut().next_frame(region);
        CAPTURE_NS.fetch_add(
            t_capture.elapsed().as_nanos() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        match captured {
            Ok(Some(frame)) => {
                self.telemetrie.capturee();
                // Horodatage lu sur l'horloge réelle AVANT la soumission :
                // c'est l'instant de la capture qui date l'image, pas celui
                // où l'encodeur voudra bien l'accepter (voir `next_pts_90k`).
                let pts = self.next_pts_90k();
                let t_submit = std::time::Instant::now();
                let fed = self
                    .encoder_mut()
                    .and_then(|encoder| encoder.submit(&frame, pts));
                SUBMIT_NS.fetch_add(
                    t_submit.elapsed().as_nanos() as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
                if let Err(e) = fed {
                    tracing::warn!(erreur = %crate::cause::chaine(&e), "soumission à l'encodeur échouée");
                } else {
                    submitted = true;
                }
            }
            Ok(None) => {}
            Err(e) => {
                // Une perte d'accès est déjà passée par les reprises de
                // `next_frame` : la recevoir ici signifie qu'elles n'ont pas
                // suffi. Fin légitime dans les deux cas.
                tracing::error!(erreur = %e, "capture interrompue, source déclarée épuisée");
                self.fatal = true;
                return None;
            }
        }

        if !submitted || self.encoder_warmed_up {
            // Soit rien de neuf à capturer ce tour-ci (cas normal), soit
            // l'encodeur a déjà prouvé qu'il répond vite (voir la doc de
            // `SUBMIT_POLL_BUDGET`) : dans les deux cas, aucune attente —
            // mais on draine tout ce qui est DÉJÀ prêt, sans jamais dormir.
            return self.drain_ready_output();
        }

        // Encodeur pas encore chaud : sa toute première sortie peut mettre
        // un peu plus d'un tour à arriver (voir la doc de
        // `SUBMIT_POLL_BUDGET`) — on l'attend brièvement plutôt que de
        // retarder le tout premier keyframe.
        let deadline = std::time::Instant::now() + SUBMIT_POLL_BUDGET;
        loop {
            match self.drain_ready_output() {
                Some(unit) => {
                    return Some(unit);
                }
                None => {
                    if std::time::Instant::now() >= deadline {
                        // Pas encore prête : elle sortira à un appel
                        // suivant. Pas un échec, juste une latence un peu
                        // plus longue que la normale à ce tout premier tour.
                        return None;
                    }
                    std::thread::sleep(SUBMIT_POLL_INTERVAL);
                }
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Épuisée pour de bon seulement si la fenêtre a disparu ou qu'une
    /// erreur de capture non récupérable a été observée — jamais pour un
    /// simple bureau immobile (voir `next_frame`).
    fn is_exhausted(&self) -> bool {
        self.fatal || !self.is_alive()
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        WindowsSource::resize(self, width, height)
    }

    fn is_alive(&self) -> bool {
        WindowsSource::is_alive(self)
    }

    /// Relaie vers l'encodeur matériel (voir `WindowsSource::request_keyframe`
    /// et le commentaire de `VideoSource::request_keyframe`).
    fn request_keyframe(&mut self) -> Result<()> {
        WindowsSource::request_keyframe(self)
    }

    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        // Mémorisé même en cas d'échec : c'est ce débit-là qu'une
        // reconstruction ultérieure de l'encodeur devra reprendre.
        self.bitrate = bitrate;
        self.encoder_mut()?.set_bitrate(bitrate)
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        WindowsSource::set_encode_size(self, width, height)
    }
}
