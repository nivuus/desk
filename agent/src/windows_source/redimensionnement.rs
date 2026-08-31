//! `WindowsSource::resize` : retailler la fenêtre et reconstruire la chaîne
//! d'encodage — par l'UN de deux chemins, selon le mode de capture.
//!
//! ❌ **Ce titre disait « — ou ne rien faire du tout », et le lot 33 l'a rendu
//! faux.** Il n'y a plus de chemin qui ne fait rien : `FenetreRecadree`
//! recapture le bureau (chemin mono-fenêtre historique), `SortieEntiere` suit
//! le viewport **à l'intérieur d'une sortie qui ne bouge pas**
//! (`suivre_le_viewport`, en pied de fichier). Voir
//! `ModeCapture::suit_le_viewport` pour la mesure qui l'a motivé et pour la
//! distinction d'avec le chemin `ChangeDisplaySettingsExW` que D9 a retiré.
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
//! ⚠️ **Le sous-bloc D8 avait donné à ce module un enfant, `mode_sortie`**, qui
//! faisait suivre à la sortie virtuelle le mode du viewport. Le sous-bloc D9
//! l'a mesuré — le changement ne survit pas à l'ouverture de la fenêtre
//! suivante, et `CDS_UPDATEREGISTRY` pollue le registre au point de bloquer le
//! produit — et l'a **retiré**. Voir le constat de mesure en tête de
//! `capteur/plein_ecran.rs`. **Ce qui reste actif et livré du plein écran,
//! c'est la DÉTECTION et l'ANNONCE** (`capteur/fenetre.rs` →
//! `AgentControl::Fullscreen`), qui ne passent pas par ici.

use anyhow::{Context, Result};

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
        // chaque connexion, avec une taille qui n'avait alors aucune raison
        // d'égaler celle de la sortie (elle vaut `clientWidth × devicePixelRatio`,
        // là où la sortie était créée sur `innerWidth` SEUL, sans le facteur
        // dpr), si bien que le court-circuit « taille inchangée » plus bas ne
        // la retenait pas. ⚠️ **Ce désaccord d'unité est celui que la tâche 5
        // du sous-bloc D9 a précisément fermé** (`client/src/main.ts`, l'annonce
        // de viewport multiplie désormais par `devicePixelRatio`) : les deux
        // unités concordent aujourd'hui, ce qui ne change rien à ce garde —
        // il reste nécessaire en mode `SortieEntiere` quelle que soit l'unité.
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
        // Ne rien faire est le comportement JUSTE, pas un pis-aller : la spec
        // §3.3 acte que le redimensionnement d'une fenêtre déjà ouverte est
        // hors périmètre de D1 (le pilote SudoVDA n'expose aucun `SET_MODE`,
        // la sortie ne peut donc pas suivre — ⚠️ **et c'est toujours vrai de
        // la SORTIE, que le lot 33 ne fait pas bouger non plus ; ce qui a
        // changé est que le RECADRAGE et la FENÊTRE, eux, la suivent à
        // l'intérieur**). `Ok(())` et non `Err` : rien n'a
        // échoué, et une erreur ferait journaliser un incident à chaque
        // connexion. L'adaptation réseau, elle, passe par `set_encode_size` et
        // n'est pas concernée.
        //
        // ⚠️ **Le sous-bloc D8 avait établi que la PRÉMISSE ci-dessus était
        // exacte, mais la CONCLUSION réfutable** : une autre voie
        // (`ChangeDisplaySettingsExW`, hors du canal du pilote SudoVDA) fait
        // bien suivre la sortie. Le sous-bloc D9 l'a mesurée en conditions de
        // produit et l'a **retirée** : le changement ne survit pas à
        // l'ouverture de la fenêtre suivante, et il pollue le registre au
        // point de bloquer le produit. Voir le constat de mesure en tête de
        // `capteur/plein_ecran.rs`. **Ne rien faire est donc redevenu, à
        // nouveau, le comportement juste — cette fois sur la foi d'une
        // mesure, et non d'une limite seulement supposée du pilote.**
        if self.mode.suit_le_viewport() {
            return self.suivre_le_viewport(width, height);
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

    /// Fait suivre au recadrage — et à la fenêtre Windows — le viewport
    /// annoncé par le navigateur, **à l'intérieur d'une sortie qui ne bouge
    /// pas**.
    ///
    /// 🔴 **CE CHEMIN REMPLACE UN `Ok(())` QUI NE FAISAIT RIEN**, et ce n'est
    /// pas une régression de D9 : voir `ModeCapture::suit_le_viewport`, qui
    /// porte la mesure du 31 août 2026 (34 demandes jetées, rapports d'aspect
    /// de 1,105 à 3,559 servis à 1,3222) et la distinction d'avec le chemin
    /// `ChangeDisplaySettingsExW` que D9 a retiré. **Aucun mode d'affichage
    /// n'est changé ici**, et le registre n'est pas touché.
    ///
    /// 🔴 **LA DUPLICATION N'EST JAMAIS RELÂCHÉE, ET C'EST LE CORRECTIF C1 DE
    /// D1 QU'IL NE FAUT PAS DÉFAIRE.** `resize` en mode `FenetreRecadree` pose
    /// `self.capture = None` puis rouvre `DesktopCapture::new()`, qui duplique
    /// **le bureau physique primaire** — sur cette VM, le VGA QEMU
    /// `\\.\DISPLAY1`, que nulle session ne sert. Ici on garde la
    /// duplication de NOTRE sortie et on ne reconstruit que la région et
    /// l'encodeur, comme `set_encode_size` (voir `encodage.rs`) : aucune
    /// contrainte de duplication DXGI, donc aucun besoin de
    /// `rebuild_or_recover`.
    fn suivre_le_viewport(&mut self, width: u32, height: u32) -> Result<()> {
        // Même garde que `set_encode_size`, et pour la même raison : sur une
        // source définitivement épuisée, `capture_mut()` paniquerait, et une
        // panique traverse `spawn_blocking` et emporte TOUT le processus —
        // donc les huit autres fenêtres du capteur avec.
        if self.fatal {
            anyhow::bail!("source épuisée : recadrage inchangé");
        }

        // La borne vient de la ZONE DE TRAVAIL du moniteur qui porte la
        // fenêtre — c'est ce qui sort la barre des tâches du recadrage. Le
        // superviseur interroge le même moniteur par l'origine de la sortie :
        // même `HMONITOR`, même borne, donc aucune bataille entre les deux
        // processus (voir `taille_pour_viewport`).
        //
        // ⚠️ **Le repli est le comportement d'avant ce lot** : quand
        // `GetMonitorInfoW` refuse, on borne par la texture de la duplication,
        // exactement comme `sur_sortie` le faisait seul.
        let texture = self.capture_mut().desktop_size();
        let borne = match crate::window::zones_du_moniteur_de(self.hwnd) {
            Ok((moniteur, travail)) => {
                let borne = crate::windows_source_sortie::borne_de_la_sortie(
                    (moniteur.width, moniteur.height),
                    Some((travail.width, travail.height)),
                );
                // 🔴 ET LA TEXTURE RESTE UNE BORNE, PAS UNE INFORMATION.
                // `rcMonitor` est en coordonnées de BUREAU, la région de
                // recadrage en pixels de TEXTURE, et les deux ne coïncident
                // pas sur cette machine : la duplication rend 1860×1080 là où
                // `GetDesc().DesktopCoordinates` rend 1428×1080 (écart mesuré
                // par le lot 32T, reconfirmé le 31 août 2026). Sans ce `min`,
                // une zone de travail plus large que la texture ferait sortir
                // la région de l'image et `crop_region` échouerait.
                (borne.0.min(texture.0), borne.1.min(texture.1))
            }
            Err(erreur) => {
                tracing::warn!(%erreur, "zone de travail illisible : recadrage borné par la texture");
                texture
            }
        };

        let (l, h) = crate::windows_source_sortie::taille_pour_viewport((width, height), borne);
        // 🔴 **TRACE INCONDITIONNELLE — elle remplace celle que ce lot avait
        // RETIRÉE.** `redimensionnement ignoré` sortait à CHAQUE demande, et
        // c'est elle qui a rendu le diagnostic du lot 33 possible (34 demandes
        // relevées, rapports d'aspect de 1,105 à 3,559). Sa remplaçante ne
        // sortait qu'en cas de CHANGEMENT : un `0` au journal ne distinguait
        // donc plus « aucun `Resize` n'arrive » de « il arrive et sature la
        // borne », c'est-à-dire un défaut de la limite déclarée.
        //
        // **Une trace qui ne peut sortir qu'en cas de succès ne peut pas
        // diagnostiquer un échec.**
        tracing::info!(
            demande = format!("{width}x{height}"),
            borne = format!("{}x{}", borne.0, borne.1),
            texture = format!("{}x{}", texture.0, texture.1),
            retenue = format!("{l}x{h}"),
            courante = format!("{}x{}", self.width, self.height),
            change = (l, h) != (self.width, self.height),
            "Resize recu par le capteur"
        );
        // Court-circuit AVANT toute destruction, et il n'est pas cosmétique :
        // le `ResizeObserver` du client émet toutes les 200 ms pendant qu'on
        // tire un bord, et chaque passage reconstruirait sinon un encodeur.
        if (l, h) == (self.width, self.height) {
            return Ok(());
        }

        // La fenêtre d'abord : `resize_window` porte `SWP_NOMOVE`, donc
        // l'origine — celle de la sortie, posée par le superviseur — ne bouge
        // pas, et le recadrage à l'origine reste juste.
        //
        // ⚠️ `resize_window` impose un plancher de 160×120 que
        // `taille_pour_viewport` n'a pas (le sien est 2) : sous 160×120 la
        // fenêtre reste plus grande que la région, et l'image montre alors un
        // coin de l'application. Cas dégénéré, non corrigé, dit ici.
        crate::window::resize_window(self.hwnd, l, h)?;

        let region = crate::windows_source_sortie::region_de_sortie(l, h)
            .ok_or_else(|| anyhow::anyhow!("recadrage inexploitable ({l}x{h})"))?;

        let device = self.capture_mut().device().clone();
        // Détruire AVANT de construire : à `vivier::PLAFOND_EVEIL` encodeurs
        // vivants, le transitoire à N+1 est refusé par la MFT NVIDIA
        // (`MF_E_UNSUPPORTED_D3D_TYPE`, 18 refus sur 18 en D4). Même remède,
        // même prix, que `set_encode_size`.
        drop(self.encoder.take());
        let neuf = H264Encoder::new(&device, (l, h), (l, h), self.fps, self.bitrate);
        let mut encoder = match neuf {
            Ok(encoder) => encoder,
            Err(erreur) => {
                self.fatal = true;
                return Err(erreur).context(
                    "encodeur neuf refusé après destruction de l'ancien : source épuisée",
                );
            }
        };
        if let Err(erreur) = encoder.request_keyframe() {
            self.fatal = true;
            return Err(erreur).context("image clé refusée par l'encodeur neuf : source épuisée");
        }

        self.region = region;
        self.width = l;
        self.height = h;
        self.encoder = Some(encoder);
        // L'encodeur neuf n'a rien produit : le budget de sondage de démarrage
        // repart, comme après `resize` et après `set_encode_size`.
        self.encoder_warmed_up = false;
        tracing::info!(
            demande = format!("{width}x{height}"),
            borne = format!("{}x{}", borne.0, borne.1),
            retenue = format!("{l}x{h}"),
            "recadrage et fenêtre alignés sur le viewport (la sortie, elle, n'a pas bougé)"
        );
        Ok(())
    }
}
