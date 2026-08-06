//! `WindowsSource::resize` : retailler la fenêtre et reconstruire la chaîne
//! d'encodage — ou ne rien faire du tout.
//!
//! **Module ENFANT de `windows_source`**, et non frère : c'est ce qui lui donne
//! accès aux champs privés de `WindowsSource` sans qu'aucun ait à être ouvert
//! en `pub(crate)` (voir le commentaire des champs dans `windows_source.rs`).
//! Extrait de ce fichier-là parce qu'il est en dette de taille (`CLAUDE.md`) et
//! que le correctif C1 de la revue finale y ajoutait par ailleurs
//! `depuis_pieces` et le champ `mode` : l'addition s'accompagne de son
//! extraction, comme la règle l'exige.
//!
//! Aucune valeur, aucun ordre d'opération n'a changé au déplacement ; la seule
//! addition est le garde de mode en tête de `resize`.
//!
//! **Le sous-bloc D8 lui a donné un enfant, `mode_sortie`**, qui porte le
//! chemin `SortieEntiere` : la sortie virtuelle y change de mode pour suivre le
//! viewport. Il vit sous ce module-ci, et non sous `windows_source`, pour ne
//! pas ajouter une ligne à un fichier en dette de taille gelée — voir son
//! commentaire de tête.

// Petit-fils de `windows_source` : il voit les champs privés de
// `WindowsSource` comme ce module-ci, la visibilité privée de Rust s'étendant
// à tous les descendants du module définissant.
mod mode_sortie;

use anyhow::Result;

use super::WindowsSource;
use crate::capture::DesktopCapture;
use crate::encode::H264Encoder;
use crate::geometry::{borner_au_bureau, crop_region, Rect};
use crate::rebuild::{rebuild_or_recover, RebuildOutcome};
use crate::window;

impl WindowsSource {
    /// Redimensionne la fenêtre et reconstruit la chaîne d'encodage.
    ///
    /// Media Foundation n'autorise pas le changement de résolution en cours de
    /// route : il faut repartir d'un encodeur neuf. L'horodatage, lui, reste
    /// continu — le décodeur du navigateur rejetterait un retour en arrière
    /// (`next_pts_90k` n'est jamais réinitialisé ici).
    ///
    /// **Sans effet quand la source capture une sortie DXGI entière** — voir le
    /// garde en tête de fonction, et `ModeCapture` pour ce qui se produisait
    /// avant lui.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        // CORRECTIF C1 (revue finale de la branche multi-fenêtres D1). Tout ce
        // qui suit suppose que
        // la fenêtre est libre d'être retaillée et que la capture est celle du
        // bureau. Les deux sont faux en mode `SortieEntiere`, et le chemin
        // était pourtant emprunté SYSTÉMATIQUEMENT : le `ResizeObserver` du
        // client émet une fois à l'observation initiale, donc ~200 ms après
        // chaque connexion, avec une taille qui n'a aucune raison d'égaler
        // celle de la sortie (elle vaut `clientWidth × devicePixelRatio`, là où
        // la sortie a été créée sur `innerWidth`), si bien que le court-circuit
        // « taille inchangée » plus bas ne la retenait pas.
        //
        // La suite produisait alors, dans l'ordre : une fenêtre rétrécie qui
        // quitte sa sortie virtuelle (que le contrôle à 1 Hz du superviseur
        // tente aussitôt de rattraper, les deux se battant), la duplication de
        // la sortie relâchée, un `DesktopCapture::new()` qui duplique le BUREAU
        // PHYSIQUE primaire, un `crop_region` en échec, et le repli
        // `Recovered` installant cette duplication-là avec la région calculée
        // pour la sortie virtuelle — c'est-à-dire le coin haut-gauche du bureau
        // réel de la VM diffusé dans la fenêtre du navigateur, pour un seul
        // `warn!`.
        //
        // Ne rien faire ÉTAIT le comportement juste, et ne l'est plus. La spec
        // §3.3 actait que le redimensionnement d'une fenêtre déjà ouverte est
        // hors périmètre de D1 « le pilote SudoVDA n'expose aucun `SET_MODE`,
        // la sortie ne peut donc pas suivre ».
        //
        // ⚠️ **La PRÉMISSE reste vraie, la CONCLUSION est réfutée** (sous-bloc
        // D8, 4 août 2026). Le pilote SudoVDA n'a effectivement toujours aucun
        // `SET_MODE` parmi ses six IOCTL — ce n'était pas une erreur de D1. Ce
        // qui a changé est le CHEMIN employé : l'API d'affichage de WINDOWS
        // (`ChangeDisplaySettingsExW`), et non le canal du pilote. La sortie
        // suit donc désormais le viewport, et `changer_mode_de_sortie` s'en
        // charge.
        //
        // **Ce qui l'établit, et comment le refaire sans croire personne** :
        // la sonde P1 `agent/src/diagnostics/multifenetre/mode_sortie.rs`
        // (`MULTIFENETRE_MODE_SORTIE=1920x1080`) crée une sortie virtuelle à
        // 1280×720 par le chemin de production, la fait passer à 1920×1080, et
        // **relit par DXGI** (`GetDesc`/`DesktopCoordinates`) — jamais par WMI,
        // dont le champ a été vu périmé de 68 s sur ce terrain. Elle a rendu
        // « P1 RECU » avec `CDS_UPDATEREGISTRY` seul, du premier coup. ⚠️ **Ses
        // deux combinaisons de repli ONT été exercées** (correction I5 de la
        // revue finale de branche — une rédaction antérieure les disait
        // « jamais exercées ») : `mode-sortie-1728x1080.log` les joue toutes
        // deux, aucune ne fait bouger la sortie, et la troisième annonce un
        // SUCCÈS d'API sur une sortie inchangée. Voir le commentaire de
        // `mode_sortie.rs`, auprès du `CDS_UPDATEREGISTRY` seul du produit.
        //
        // `Ok(())` et non `Err` reste vrai, et pour la même raison : le
        // `ResizeObserver` du client émet une fois à l'observation initiale, et
        // un refus du pilote ferait journaliser un incident à chaque connexion.
        // L'adaptation réseau, elle, passe par `set_encode_size` et n'est
        // toujours pas concernée.
        //
        // ⚠️ **IMPORTANT 2 (revue de la tâche 9), écart banc/produit jamais
        // mesuré.** La sonde P1 crée sa sortie, change son mode, puis relit —
        // elle n'ouvre JAMAIS `DuplicateOutput` dessus. En production,
        // `changer_mode_de_sortie` retaille une sortie dont la duplication
        // DXGI est ouverte et détenue pendant l'attente (jusqu'à 3,1 s). P1 ne
        // dit donc rien de ce que fait `ChangeDisplaySettingsExW` sur une
        // sortie EN COURS de capture — la même classe d'écart banc/produit
        // que le chantier « N duplications de front » a payée en D1. **Non
        // corrigeable par du code** : les tâches 10 et 11 (recette) en sont
        // la première mesure réelle.
        if !self.mode.redimensionne_la_fenetre() {
            if !crate::capteur::plein_ecran::actif() {
                tracing::info!(
                    width,
                    height,
                    mode = ?self.mode,
                    "redimensionnement ignoré : PLEIN_ECRAN=0 désarme le changement de mode \
                     de sortie (comportement D1)"
                );
                return Ok(());
            }
            // ⚠️ **DÉSARMÉ PAR DÉFAUT — décision de la revue finale de branche
            // de D8 (5 août 2026), pas une prudence vague.** Les trois raisons
            // et leurs pièces vivent auprès du garde lui-même
            // (`capteur::plein_ecran::changement_de_mode_arme`) : C1, la
            // pollution du registre qui bloque les ouvertures de fenêtre
            // ultérieures, de portée inconnue puisque cinq GUID SudoVDA
            // distincts apparaissent au journal de recette ; C2, la reprise sur
            // perte d'accès de D2 court-circuitée par une `Err` sur un échec
            // transitoire de réouverture ; et le fait que ce chemin n'ait
            // JAMAIS tourné en conditions de produit — critère ② NON EXERCÉ,
            // `mode_sortie_demande=0` aux deux exécutions de la recette.
            //
            // **Ce qui reste livré et actif** : la détection du style, son
            // annonce au navigateur, et l'armement client. C'est le repli que
            // le §4 de la conception a écrit — « ①, ③, ④ et ⑤ tiennent sans ② ».
            //
            // Les deux Critiques sont donc INATTEIGNABLES par défaut et
            // délibérément NON CORRIGÉES : les traiter est le premier travail
            // de la recette qui armera `PLEIN_ECRAN_MODE_SORTIE=1`.
            if !crate::capteur::plein_ecran::changement_de_mode_arme() {
                tracing::info!(
                    width,
                    height,
                    mode = ?self.mode,
                    "redimensionnement ignoré : changement de mode de sortie DÉSARMÉ \
                     (poser PLEIN_ECRAN_MODE_SORTIE=1 pour l'armer) — la détection et \
                     l'annonce du plein écran restent actives"
                );
                return Ok(());
            }
            // IMPORTANT 4 (revue de la tâche 9) : même plancher que le chemin
            // `FenetreRecadree` quelques lignes plus bas (`width.max(160)`,
            // `height.max(120)`), appliqué ICI et non délégué à
            // `borner_a_la_taille_max` — dont le rôle reste borné au PLAFOND
            // (`TAILLE_MAX_SORTIE`), pas au plancher. Sans lui, toute
            // réduction à zéro de la boîte vidéo (fenêtre repliée, transition
            // de plein écran) émettrait `0×0`, que `borner_a_la_taille_max`
            // laissait passer tel quel par sa branche rapide (`l <= max_l &&
            // h <= max_h`, aucun `.max` avant D8) : le pilote aurait été
            // sollicité pour un mode quasi nul, brûlant les 3,1 s
            // d'`attendre_la_sortie` pour rien.
            let (width, height) = (width.max(160), height.max(120));
            let (largeur, hauteur) = crate::windows_source_sortie::borner_a_la_taille_max((
                width, height,
            ));
            return self.changer_mode_de_sortie(largeur, hauteur);
        }

        let (width, height) = (width.max(160) & !1, height.max(120) & !1);

        // Borner à ce que le bureau peut réellement afficher. Un viewport
        // client plus grand que le bureau de la VM produirait sinon une
        // fenêtre qui dépasse : `crop_region` la rognerait à la capture,
        // l'image prendrait un rapport d'aspect que le conteneur du navigateur
        // n'a pas — d'où des bandes noires — et la partie hors écran de
        // l'application deviendrait inatteignable. Constaté le 29/07/2026 :
        // 1187 px demandés pour un bureau de 1080.
        //
        // Sans capture vivante ou sans position lisible, on laisse passer la
        // taille demandée : `crop_region` reste le filet, et un
        // redimensionnement imparfait vaut mieux qu'un échec.
        let (width, height) = match (self.capture.as_ref(), window::client_rect_on_screen(self.hwnd))
        {
            (Some(capture), Ok(actuel)) => {
                let (dw, dh) = capture.desktop_size();
                let (w, h) = borner_au_bureau(actuel.x, actuel.y, width, height, dw, dh);
                (w & !1, h & !1)
            }
            _ => (width, height),
        };

        if (width, height) == (self.width, self.height) {
            return Ok(());
        }

        window::resize_window(self.hwnd, width, height)?;
        // Laisser la fenêtre atteindre sa nouvelle taille avant de recapturer.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let window_rect = window::client_rect_on_screen(self.hwnd)?;

        // Relâche explicitement l'ancienne capture (donc son
        // `IDXGIOutputDuplication`) AVANT d'en créer une nouvelle. DXGI
        // n'autorise qu'une seule instance vivante de la duplication pour une
        // sortie donnée, dans un même processus : une simple réaffectation
        // (`self.capture = Some(DesktopCapture::new()?)`) évaluerait le
        // membre droit — donc `DuplicateOutput` — avant de remplacer
        // l'ancien `Some`, laissant les deux exister en même temps le temps
        // de l'appel. `DuplicateOutput` échoue alors avec « duplication de
        // la sortie écran » — observé lors du premier essai bout en bout de
        // la tâche 13.
        //
        // Cette libération anticipée ouvre en retour une fenêtre où
        // `self.capture` peut rester `None` si la reconstruction échoue : on
        // ne la referme jamais avec un simple `?` (voir la ronde de
        // correction 1 au commentaire du champ `capture`). `rebuild_or_recover`
        // (module `rebuild`, testé sans dépendance Windows) porte cette
        // logique : tenter la reconstruction complète, et si elle échoue,
        // retenter EXPLICITEMENT une capture de secours — avec les anciens
        // `region`/`encoder`/dimensions, encore valides puisqu'eux n'ont pas
        // été touchés — avant de renvoyer l'erreur à l'appelant.
        self.capture = None;
        let fps = self.fps;
        let bitrate = self.bitrate;
        // **Correctif C1 de la revue finale de la branche RÉSEAU ADAPTATIF**
        // (29/07/2026, commit `3f02545`) — homonyme du C1 multi-fenêtres
        // ci-dessus, et sans rapport avec lui. Une première version de
        // ce chantier conservait ici la taille d'encodage courante
        // (`self.encoder.encode_size()`) au lieu de repartir de la taille de
        // capture, dans l'intention de ne pas effacer une réduction de
        // résolution appliquée pour cause de lien dégradé (tâche 9). C'était
        // faux : au démarrage, encode == capture, donc dès le PREMIER
        // redimensionnement de fenêtre, la taille encodée se figeait pour
        // toute la session — agrandir la fenêtre n'agrandissait plus jamais
        // le flux, et le contrôleur (dont l'échelle n'était, elle, jamais
        // reconstruite) pouvait même finir par viser une taille supérieure à
        // la nouvelle capture. La taille encodée doit donc à nouveau suivre
        // la fenêtre inconditionnellement ; c'est `Session::act_on_timeout`
        // (branche a1, `transport/tick.rs`) qui a désormais la charge de
        // rappliquer, juste après, la réduction que le contrôleur jugerait
        // encore nécessaire pour la NOUVELLE taille (voir
        // `congestion::Controleur::changer_source`) — au lieu de la préserver
        // ici à l'aveugle.

        // Un NOUVEAU périphérique D3D11 est créé par l'ouverture appelée juste
        // en dessous — `DesktopCapture::new_sans_attente`, et non plus
        // `DesktopCapture::new` : ce chemin court sur le fil bloquant de
        // `Session::run`, où la fenêtre de réessai de trois secondes
        // suspendrait du même coup les demandes de keyframe et l'adaptation
        // réseau. Les deux passent par `DesktopCapture::ouvrir`, qui pose
        // `SetMultithreadProtected(TRUE)` sur CE périphérique à chaque appel
        // (`capture/ouverture.rs::creer_peripherique`, local `multithread`) — la
        // protection est donc reconstruite avec lui, pas seulement héritée de
        // l'ancien périphérique qui vient d'être libéré. Sans cela le
        // blocage intermittent d'`AcquireNextFrame` documenté à la tâche 10
        // réapparaîtrait après tout redimensionnement.
        let outcome = rebuild_or_recover(
            || -> Result<(DesktopCapture, Rect, H264Encoder)> {
                let new_capture = DesktopCapture::new_sans_attente()?;
                let (dw, dh) = new_capture.desktop_size();
                let region = crop_region(window_rect, dw, dh)
                    .ok_or_else(|| anyhow::anyhow!("la fenêtre est hors de l'écran"))?;
                let mut encoder = H264Encoder::new(
                    new_capture.device(),
                    (region.width, region.height),
                    (region.width, region.height),
                    fps,
                    bitrate,
                )?;
                encoder.request_keyframe()?;
                Ok((new_capture, region, encoder))
            },
            // Fabrique de secours : juste une capture valide, pour ne jamais
            // laisser `self.capture` à `None` sans `fatal` à vrai en retour.
            // `region`/`encoder`/`width`/`height` restent ceux d'avant :
            // seule la capture avait dû être relâchée, pas les paramètres qui
            // en dépendent, qui n'ont jamais cessé d'être valides.
            //
            // `new_sans_attente` comme la fabrique principale ci-dessus : ces
            // deux appels courent sur le fil bloquant de `Session::run` (voir
            // `transport/redimensionnement.rs`), où le réessai d'ouverture
            // ajouté à la tâche 11 bis gèlerait la boucle de session jusqu'à
            // DEUX fenêtres pleines. Le droit de bloquer se décide ici, pas
            // dans `capture::ouvrir`.
            DesktopCapture::new_sans_attente,
        );

        match outcome {
            RebuildOutcome::Rebuilt((new_capture, region, encoder)) => {
                self.capture = Some(new_capture);
                self.region = region;
                self.encoder = Some(encoder);
                self.width = region.width;
                self.height = region.height;
                // Nouvel encodeur : sa toute première sortie retombe dans le
                // même cas que le démarrage initial (voir `SUBMIT_POLL_BUDGET`).
                self.encoder_warmed_up = false;
                tracing::info!(self.width, self.height, "chaîne d'encodage reconstruite");
                Ok(())
            }
            RebuildOutcome::Recovered(new_capture, primary_error) => {
                // État exploitable restauré (anciens région/encodeur/
                // dimensions, nouvelle capture) : la session continue, comme
                // l'exige le brief pour un échec de redimensionnement. La
                // fenêtre OS, elle, a déjà changé de taille
                // (`resize_window` ci-dessus a réussi) : un décalage
                // transitoire entre la fenêtre réelle et la région capturée
                // est possible jusqu'au prochain redimensionnement réussi —
                // préférable, de loin, à un agent qui plante.
                self.capture = Some(new_capture);
                tracing::warn!(erreur = %primary_error, "reconstruction de la chaîne d'encodage échouée, capture de secours restaurée");
                Err(primary_error)
            }
            RebuildOutcome::Fatal(primary_error) => {
                // Ni la chaîne complète, ni une simple capture de secours
                // n'ont pu être obtenues : `self.capture` reste `None`.
                // `fatal` le signale pour de bon — `next_frame` s'arrête
                // avant de toucher `capture` (voir son garde), et
                // `is_exhausted()` fera clore la session proprement au tour
                // suivant, plutôt qu'un panic sur le champ vide.
                self.fatal = true;
                tracing::error!(erreur = %primary_error, "reconstruction de la chaîne d'encodage et capture de secours toutes deux échouées, source déclarée épuisée");
                Err(primary_error)
            }
        }
    }
}
