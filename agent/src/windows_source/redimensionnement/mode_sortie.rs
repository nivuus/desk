//! Faire suivre à la sortie virtuelle la taille du viewport — le passage en
//! plein écran du sous-bloc D8.
//!
//! **Module ENFANT de `redimensionnement`, donc PETIT-FILS de
//! `windows_source`** : la visibilité privée de Rust s'étend au module
//! définissant et à tous ses descendants, si bien que ces méthodes touchent les
//! champs privés de `WindowsSource` exactement comme le fait leur parent, sans
//! qu'aucun n'ait à être ouvert en `pub(crate)`.
//!
//! **Enfant de `redimensionnement` et non de `windows_source` à dessein** :
//! déclarer un module de plus dans `windows_source.rs` serait une addition à un
//! fichier en dette de taille GELÉE (638 lignes), dont `CLAUDE.md` exige
//! qu'elle s'accompagne de sa propre extraction. Ici la déclaration `mod` vit
//! dans `redimensionnement.rs`, que cette extraction fait justement maigrir.
//!
//! Extrait pour cette raison même : le changement de mode portait
//! `redimensionnement.rs` de 232 à 520 lignes, au-dessus du plafond de 500.
//! **Aucune valeur, aucun ordre d'opération n'a changé au déplacement.**

use anyhow::Result;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, CDS_UPDATEREGISTRY, DEVMODEW, DISP_CHANGE_SUCCESSFUL, DM_PELSHEIGHT,
    DM_PELSWIDTH,
};

use crate::capture::{CibleCapture, DesktopCapture};
use crate::encode::H264Encoder;
use crate::geometry::Rect;
use crate::rebuild::{rebuild_or_recover, RebuildOutcome};
use crate::windows_source::WindowsSource;

/// Pas et budget de l'attente qui suit un changement de mode.
///
/// **On attend le FAIT — la taille relue par DXGI — et jamais une durée.**
/// Windows reconfigure sa topologie d'affichage de façon asynchrone : la sonde
/// P1 observait pour cela un délai de grâce de 3 s, plat. Ici le budget vaut
/// autant, mais il est rendu dès que la sortie a bougé, parce que ce chemin
/// court sur le fil de fenêtre du capteur (`capteur/fenetre/commandes.rs`) —
/// l'immobiliser trois secondes pleines gèlerait la capture de CETTE fenêtre à
/// chaque entrée en plein écran. C'est la doctrine du sous-bloc D3 : « attendre
/// le fait, jamais une durée », où un `sleep 45` prescrit couvrait un fait
/// acquis en 6,4 s.
const BUDGET_TOPOLOGIE: std::time::Duration = std::time::Duration::from_secs(3);
const PAS_TOPOLOGIE: std::time::Duration = std::time::Duration::from_millis(100);

impl WindowsSource {
    /// Change le mode de la sortie virtuelle, puis y repose la fenêtre.
    ///
    /// Rend `Ok(())` même quand le pilote refuse : rien n'a échoué du point de
    /// vue de la session, l'image reste simplement mise à l'échelle. Un `Err`
    /// ferait journaliser un incident à chaque connexion (le `ResizeObserver`
    /// émet une fois à l'observation initiale).
    ///
    /// ✅ Le superviseur ne combattra pas ce placement, et c'est vérifié :
    /// `replacer_si_besoin` (`superviseur/boucle/placement_periodique.rs`)
    /// réénumère les sorties DXGI à chaque contrôle et les résout par NOM, puis
    /// pose sur le `rect` fraîchement lu. Il n'y a aucun rectangle mémorisé qui
    /// deviendrait périmé.
    ///
    /// **La sortie GARDE SON NOM** : son attache capteur→enfant, sa place dans
    /// la table du superviseur et la cible retenue par `DesktopCapture`
    /// survivent au changement de mode. Seule la duplication perd son accès —
    /// c'est l'abandon de mutex `0x887A0026` que D2 a appris à encaisser — et
    /// elle est ici rouverte explicitement, à la taille neuve.
    pub(super) fn changer_mode_de_sortie(&mut self, largeur: u32, hauteur: u32) -> Result<()> {
        // Le nom de la sortie n'est PAS un champ de `WindowsSource` — et il n'a
        // pas eu à le devenir : `DesktopCapture` retient déjà sa `CibleCapture`
        // (il en a besoin pour se rouvrir après une perte d'accès) et l'expose
        // par `cible()`. Un champ de plus l'aurait dupliquée, et
        // `windows_source.rs` est en dette de taille gelée (`CLAUDE.md`).
        //
        // **Par le NOM, jamais par un index** : les index d'énumération DXGI
        // sont positionnels et changent dès qu'une sortie paraît ou disparaît,
        // ce qui est le cas nominal en multi-fenêtres. Le dépôt l'a payé en D1
        // (« aucune sortie DXGI à l'index adaptateur 0, sortie 5 »).
        let nom_sortie = match self.capture.as_ref().map(DesktopCapture::cible) {
            Some(CibleCapture::Sortie(nom)) => nom.clone(),
            // `Bureau` en mode `SortieEntiere` ne devrait pas exister, et
            // l'absence de capture est le transitoire de `resize` ou l'état
            // `fatal`. Dans les deux cas il n'y a pas de sortie à retailler :
            // on ne fait rien, et on le dit.
            autre => {
                tracing::warn!(
                    largeur,
                    hauteur,
                    cible = ?autre,
                    "changement de mode impossible : la source ne vise aucune sortie nommée"
                );
                return Ok(());
            }
        };

        // Court-circuit AVANT tout appel Windows, et il n'est pas cosmétique.
        // Sans lui, l'observation initiale du `ResizeObserver` — qui arrive à
        // CHAQUE connexion, ~200 ms après, avec une taille qui n'a aucune
        // raison d'égaler celle de la sortie — déclencherait un changement de
        // mode complet, donc un abandon du mutex DXGI de TOUTES les sorties
        // voisines. C'est exactement la cascade que D1 a subie et que D2 a
        // appris à encaisser : l'encaisser ne dispense pas de ne pas la
        // provoquer.
        //
        // `taille_compatible` (4 px) et non l'égalité stricte : c'est le
        // prédicat que le superviseur emploie déjà pour juger qu'une sortie
        // existante fait l'affaire, et les bordures invisibles de DWM comme
        // l'alignement pair produisent couramment cet écart-là. ⚠️ Il ne
        // protège PAS de la quantification du pilote, d'un tout autre ordre
        // (1280×632 demandé rendu 1280×720, soit 88 px, sous-bloc D2) : c'est
        // la taille OBTENUE qui est retenue plus bas, et c'est elle qui ferme
        // la boucle.
        if crate::superviseur::placement::taille_compatible(
            (largeur, hauteur),
            (self.width, self.height),
        ) {
            return Ok(());
        }

        // `CDS_UPDATEREGISTRY` SEUL : c'est littéralement la combinaison que la
        // sonde P1 a vue tenir du premier coup. Ses deux replis
        // (`|CDS_RESET`, puis `NORESET` + `RESET` séparé) n'ont jamais été
        // exercés — les reprendre ici serait du code jamais couru.
        // `CDS_SET_PRIMARY` est délibérément absent : on ne touche pas au
        // moniteur primaire.
        let code = changer_mode(&nom_sortie, largeur, hauteur);
        tracing::info!(
            sortie = %nom_sortie,
            largeur,
            hauteur,
            code,
            api_annonce_succes = (code == DISP_CHANGE_SUCCESSFUL.0),
            "changement de mode de la sortie virtuelle demandé"
        );

        // **Le verdict est ce que DXGI relit, jamais ce que l'API retourne** —
        // doctrine de la sonde P1 : un `DISP_CHANGE_SUCCESSFUL` sur une sortie
        // dont les dimensions n'ont pas bougé est un refus déguisé.
        let Some(rect) = attendre_la_sortie(&nom_sortie, (largeur, hauteur)) else {
            tracing::warn!(
                sortie = %nom_sortie,
                largeur,
                hauteur,
                code,
                "la sortie n'a pas pris le mode demandé : le flux reste mis à l'échelle"
            );
            return Ok(());
        };

        // La fenêtre est reposée sur le rectangle FRAÎCHEMENT RELU, jamais sur
        // celui demandé — le pilote quantifie.
        if let Err(erreur) = crate::superviseur::placement::poser(self.hwnd, &rect) {
            // Non bloquant : le contrôle périodique du superviseur repose la
            // fenêtre à 1 Hz de toute façon. Le journal, lui, doit le dire.
            tracing::warn!(erreur = %erreur, sortie = %nom_sortie, "replacement de la fenêtre sur sa sortie retaillée échoué");
        }

        self.reconstruire_sur_la_sortie(&nom_sortie, rect)
    }

    /// Rouvre la duplication de la sortie retaillée et refait l'encodeur à sa
    /// nouvelle taille.
    ///
    /// Même structure que la seconde moitié de `resize`, et pour les mêmes
    /// raisons : l'ancienne capture est relâchée AVANT que la neuve ne soit
    /// demandée (DXGI n'autorise qu'une duplication par sortie et par
    /// processus), et `rebuild_or_recover` garantit qu'un échec ne laisse
    /// jamais `self.capture` à `None` sans que `fatal` ne le dise.
    fn reconstruire_sur_la_sortie(&mut self, nom_sortie: &str, rect: Rect) -> Result<()> {
        self.capture = None;
        let fps = self.fps;
        let bitrate = self.bitrate;
        let nom = nom_sortie.to_string();
        let nom_secours = nom.clone();

        // `sur_sortie` et non `new_sans_attente` : la cible est cette
        // sortie-ci, pas le bureau — substituer le bureau physique est
        // exactement la fuite d'image entre sessions que `ModeCapture` existe
        // pour empêcher. Sa fenêtre de réessai pleine est ici un ACQUIS et non
        // un coût : on vient de reconfigurer la topologie, donc `DuplicateOutput`
        // a toutes les raisons de répondre « pas maintenant » au premier essai.
        let outcome = rebuild_or_recover(
            || -> Result<(DesktopCapture, Rect, H264Encoder)> {
                let new_capture = DesktopCapture::sur_sortie(&nom)?;
                let (dw, dh) = new_capture.desktop_size();
                let region = crate::windows_source_sortie::region_de_sortie(dw, dh)
                    .ok_or_else(|| anyhow::anyhow!("sortie {nom} de dimensions inexploitables ({dw}x{dh})"))?;
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
            || DesktopCapture::sur_sortie(&nom_secours),
        );

        match outcome {
            RebuildOutcome::Rebuilt((new_capture, region, encoder)) => {
                self.capture = Some(new_capture);
                self.region = region;
                self.encoder = Some(encoder);
                // La taille OBTENUE, jamais la demandée. `region` vient de
                // `desktop_size()`, c'est-à-dire du `ModeDesc` de la
                // duplication neuve : c'est la seule valeur qui décrit la
                // texture que l'encodeur recevra.
                self.width = region.width;
                self.height = region.height;
                self.encoder_warmed_up = false;
                tracing::info!(
                    sortie = %nom_sortie,
                    self.width,
                    self.height,
                    rect_largeur = rect.width,
                    rect_hauteur = rect.height,
                    "sortie retaillée : chaîne d'encodage reconstruite à la taille obtenue"
                );
                Ok(())
            }
            RebuildOutcome::Recovered(new_capture, primary_error) => {
                self.capture = Some(new_capture);
                tracing::warn!(erreur = %primary_error, sortie = %nom_sortie, "reconstruction après changement de mode échouée, capture de secours restaurée");
                Err(primary_error)
            }
            RebuildOutcome::Fatal(primary_error) => {
                self.fatal = true;
                tracing::error!(erreur = %primary_error, sortie = %nom_sortie, "reconstruction après changement de mode et capture de secours toutes deux échouées, source déclarée épuisée");
                Err(primary_error)
            }
        }
    }
}

/// `ChangeDisplaySettingsExW` sur une sortie nommée, avec le seul jeu de
/// drapeaux que la sonde P1 a vu tenir. Rend le code brut, qui **ne prouve
/// rien par lui-même** — voir l'appelant.
fn changer_mode(nom_sortie: &str, largeur: u32, hauteur: u32) -> i32 {
    let nom: Vec<u16> = nom_sortie.encode_utf16().chain(std::iter::once(0)).collect();
    let dm = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        dmFields: DM_PELSWIDTH | DM_PELSHEIGHT,
        dmPelsWidth: largeur,
        dmPelsHeight: hauteur,
        ..Default::default()
    };
    unsafe {
        ChangeDisplaySettingsExW(
            PCWSTR(nom.as_ptr()),
            Some(&dm as *const DEVMODEW),
            None,
            CDS_UPDATEREGISTRY,
            None,
        )
        .0
    }
}

/// Attend que la sortie nommée porte la taille demandée, et rend son rectangle
/// relu. `None` si elle ne l'a pas prise dans le budget.
///
/// **Relecture par `GetDesc`/`DesktopCoordinates`** (`enumerer_sorties_silencieux`),
/// jamais par WMI : le champ WMI a été vu périmé de 68 s sur ce terrain.
/// Silencieuse, parce que cette boucle interroge jusqu'à trente fois.
///
/// Le critère est « la taille demandée est atteinte, à `taille_compatible`
/// près » : le pilote quantifie, donc exiger l'égalité stricte ferait conclure
/// à un refus sur un succès de 4 px d'écart. Une quantification plus large
/// (88 px relevés en D2) épuise le budget et se lit comme un refus — c'est le
/// comportement voulu : le flux reste alors mis à l'échelle, et le journal le
/// dit.
fn attendre_la_sortie(nom_sortie: &str, cible: (u32, u32)) -> Option<Rect> {
    let debut = std::time::Instant::now();
    loop {
        if let Ok(sorties) = crate::capture::enumerer_sorties_silencieux() {
            if let Some(sortie) = sorties.iter().find(|s| s.nom_sortie == nom_sortie) {
                if crate::superviseur::placement::taille_compatible(
                    (sortie.rect.width, sortie.rect.height),
                    cible,
                ) {
                    return Some(sortie.rect);
                }
            }
        }
        if debut.elapsed() >= BUDGET_TOPOLOGIE {
            return None;
        }
        std::thread::sleep(PAS_TOPOLOGIE);
    }
}
