//! Faire suivre à la sortie virtuelle la taille du viewport — le passage en
//! plein écran du sous-bloc D8.
//!
//! ⚠️ **CE MODULE NE S'EXÉCUTE PAS EN CONFIGURATION LIVRÉE.** La revue finale
//! de branche de D8 (5 août 2026) l'a **désarmé par défaut** : son unique
//! appelant (`redimensionnement.rs::resize`) est gardé par
//! `capteur::plein_ecran::changement_de_mode_arme()`, que seul
//! `PLEIN_ECRAN_MODE_SORTIE=1` rend vrai. **Raison** : ce chemin n'a JAMAIS
//! tourné en conditions de produit (critère ② de la recette NON EXERCÉ,
//! `mode_sortie_demande=0` aux deux exécutions) et porte deux Critiques
//! ouvertes — C1, la pollution du registre qui bloquerait les ouvertures de
//! fenêtre ultérieures ; C2, la reprise sur perte d'accès de D2
//! court-circuitée. Les deux sont documentées auprès du garde, et sont le
//! premier travail de qui armera ce module.
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
    /// Rend `Ok(())` même quand le pilote REFUSE TOTALEMENT (la sortie relue
    /// n'a pas bougé de `self.width`/`self.height`) : rien n'a échoué du point
    /// de vue de la session, le flux reste à sa taille courante. Un `Err`
    /// ferait journaliser un incident à chaque connexion (le `ResizeObserver`
    /// émet une fois à l'observation initiale). ⚠️ Ce n'est PAS le seul cas où
    /// `Ok(())` sort : un refus de RECONSTRUCTION après que la sortie a bel et
    /// bien bougé rend, lui, une `Err` (voir `reconstruire_sur_la_sortie`) —
    /// les deux ne doivent pas être confondus, IMPORTANT 1 de la revue de la
    /// tâche 9 l'a précisément corrigé pour cette raison.
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
    ///
    /// ⚠️ **IMPORTANT 2 (revue de la tâche 9), écart banc/produit jamais
    /// mesuré.** La sonde P1 (`diagnostics/multifenetre/mode_sortie.rs::essayer_les_modes`)
    /// crée la sortie, change son mode, puis relit — elle n'ouvre JAMAIS
    /// `DuplicateOutput` dessus. Ici, en production, cette fonction retaille
    /// une sortie dont la duplication DXGI est ouverte et détenue pendant
    /// toute l'attente (jusqu'à 3,1 s). P1 ne dit donc rien de ce que fait
    /// `ChangeDisplaySettingsExW` sur une sortie EN COURS de capture — la
    /// même classe d'écart banc/produit que le chantier « N duplications de
    /// front » a payée en D1. **Non corrigeable par du code** : les tâches
    /// 10 et 11 (recette) en sont la première mesure réelle.
    ///
    /// ⚠️ **IMPORTANT 4 (revue de la tâche 9) : aucun plancher n'est appliqué
    /// ICI.** Le plancher (160×120, même valeur que le chemin
    /// `FenetreRecadree`) est posé par l'appelant, `resize()`, avant
    /// `borner_a_la_taille_max` — voir son commentaire.
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
        //
        // ⚠️ **IMPORTANT 3 (revue de la tâche 9) : cette protection n'est
        // COMPLÈTE qu'à `devicePixelRatio == 1`, et l'affirmation ci-dessus
        // (« sans lui, [...] déclencherait ») ne le disait pas.** La sortie
        // est créée sur `viewportPair(window.innerWidth, window.innerHeight)`
        // (`client/src/main.ts`, SANS `devicePixelRatio`), alors que le
        // `ResizeObserver` qui suit émet
        // `round(video.clientWidth * window.devicePixelRatio)`
        // (`client/src/main.ts`, AVEC lui). À dpr = 1 les deux coïncident (0
        // ou 1 px d'écart, sous la tolérance de 4 px) ; à dpr = 1,25 / 1,5 / 2
        // l'écart vaut 25 à 100 % — largement au-dessus — et l'observation
        // initiale déclenche alors RÉELLEMENT la cascade que ce court-circuit
        // existe pour empêcher, sur tout client HiDPI, sans qu'aucun plein
        // écran n'ait été demandé. **Non corrigé ici** : les deux annonces
        // (création de sortie côté shell, `ResizeObserver` côté page de
        // session) partagent le même message `resize` que consomme AUSSI le
        // chemin `FenetreRecadree` (mono-fenêtre), où `devicePixelRatio` est
        // en revanche NÉCESSAIRE — la texture DXGI du bureau physique que ce
        // chemin recadre est en pixels PHYSIQUES, pas en pixels CSS. Unifier
        // les deux unités sans casser ce second chemin exige de vérifier ce
        // qu'il en coûte, ce qu'aucun test d'hôte ne peut faire ; signalé au
        // lieu d'être risqué. ✅ **Depuis la revue finale de branche (5 août
        // 2026), la parade est l'état PAR DÉFAUT** : ce chemin entier est
        // désarmé tant que `PLEIN_ECRAN_MODE_SORTIE=1` n'est pas posé (voir
        // `capteur::plein_ecran::changement_de_mode_arme`), et le défaut HiDPI
        // est donc inatteignable en configuration livrée. Il reste ouvert côté
        // client, et devient le premier travail de qui armera ce chemin —
        // `PLEIN_ECRAN=0` n'est plus la seule parade, elle est devenue la
        // parade du mécanisme entier.
        if crate::superviseur::placement::taille_compatible(
            (largeur, hauteur),
            (self.width, self.height),
        ) {
            return Ok(());
        }

        // `CDS_UPDATEREGISTRY` SEUL : c'est littéralement la combinaison que la
        // sonde P1 a vue tenir du premier coup.
        //
        // ⚠️ **Ses deux replis (`|CDS_RESET`, puis `NORESET` + `RESET` séparé)
        // ONT été exercés, et il ne faut pas les reprendre pour autant** —
        // correction de la revue finale de branche (I5) : une rédaction
        // antérieure les disait « jamais exercés », ce qui était faux.
        // `mode-sortie-1728x1080.log` (tâche 9) les joue tous les deux sur une
        // cible hors des neuf modes annoncés, et AUCUN ne fait bouger la
        // sortie. Le troisième est pire qu'un refus : il rend
        // `code_second_appel=0` et `api_annonce_succes=true` sur une sortie
        // relue INCHANGÉE (`largeur_relue=1920 hauteur_relue=1080`,
        // `conforme=false`) — un **refus déguisé en succès**, exactement ce
        // contre quoi ce dépôt a bâti sa doctrine « juger sur la relecture
        // DXGI, jamais sur le code de retour ». Les reprendre ici n'ajouterait
        // donc aucune chance de succès, et le troisième ferait croire à une
        // réussite là où rien n'a bougé.
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
        //
        // `None` signifie ici « aucune lecture de topologie n'a réussi dans le
        // budget » — PAS « la cible n'a pas été atteinte ». Voir
        // `attendre_la_sortie` : la distinction entre « refusé » et « atteint
        // une taille différente de la cible » se fait ci-dessous, contre
        // `self.width`/`self.height`, pas ici contre `None`.
        let Some(rect) = attendre_la_sortie(&nom_sortie, (largeur, hauteur)) else {
            tracing::warn!(
                sortie = %nom_sortie,
                largeur,
                hauteur,
                code,
                "topologie illisible apres le changement de mode : etat suppose inchange"
            );
            return Ok(());
        };

        // IMPORTANT 1 (revue de la tâche 9) : comparer à `self.width`/`self.height`
        // — la taille AVANT tentative —, pas seulement à `(largeur, hauteur)`
        // — la cible. Le pilote QUANTIFIE (1280×632 demandé → 1280×720 obtenu,
        // 88 px d'écart, sous-bloc D2) et n'annonce que NEUF modes discrets
        // (sonde P1), alors que `borner_a_la_taille_max` produit des tailles à
        // rapport d'aspect préservé qui n'en font presque jamais partie
        // (1920×1200 → 1728×1080, par exemple). Une première version de cette
        // tâche ne testait la sortie relue QUE contre la cible : un
        // changement PARTIEL — la sortie a bougé, mais pas jusqu'à la cible —
        // se lisait alors comme un refus total, et la chaîne de capture
        // n'était JAMAIS reconstruite alors que la texture, elle, avait
        // changé de taille : même corruption permanente que le Critique
        // (région périmée, recadrage de travers).
        if crate::superviseur::placement::taille_compatible(
            (rect.width, rect.height),
            (self.width, self.height),
        ) {
            // La sortie n'a PAS bougé : refus authentique. Rien à
            // reconstruire — le flux reste à sa taille courante, mise à
            // l'échelle par le lecteur vidéo du navigateur si le viewport
            // diffère.
            tracing::warn!(
                sortie = %nom_sortie,
                largeur,
                hauteur,
                code,
                largeur_relue = rect.width,
                hauteur_relue = rect.height,
                "la sortie n'a pas pris le mode demandé : le flux reste a sa taille courante, mis a l'echelle cote client"
            );
            return Ok(());
        }

        // La sortie A bougé — qu'elle ait atteint la cible exacte ou une
        // quantification différente — : la fenêtre est reposée sur le
        // rectangle FRAÎCHEMENT RELU, jamais sur celui demandé, et la chaîne
        // de capture est reconstruite sur CETTE taille.
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
        // CRITIQUE (revue de la tâche 9) : l'ancien encodeur DOIT être détruit
        // ICI, AVANT que la fabrique ci-dessous n'en demande un neuf au
        // matériel — même ordre que `set_encode_size` (`windows_source/encodage.rs`),
        // avec vingt lignes de commentaire dessus pour dire pourquoi. À
        // `vivier::PLAFOND_EVEIL` (8) encodeurs vivants, un instant où le
        // neuf coexiste avec l'ancien demande un NEUVIÈME encodeur au
        // matériel, refusé au `SetOutputType` (`MF_E_UNSUPPORTED_D3D_TYPE`,
        // `0xC00D6D76` — 18 refus sur 18 mesurés en D4). Une première version
        // de cette tâche ne détruisait que `self.capture` ici : sur la
        // configuration nominale (huit fenêtres éveillées), un changement de
        // mode aurait donc demandé ce neuvième encodeur, retombant sur le
        // secours `DesktopCapture::sur_sortie` — qui réussit, lui, puisqu'il
        // ne demande aucun encodeur — et **gardait alors `self.region` d'AVANT
        // sur une texture qui avait changé de taille** : recadrage périmé,
        // corruption permanente de la session (le bras `Recovered` ci-dessous
        // ne réessaie jamais).
        //
        // Le prix est le même qu'ailleurs dans ce fichier, et assumé de la
        // même façon : si la construction du neuf échoue, l'ancien n'est plus
        // là. Le bras `Recovered` plus bas en tire la conséquence honnête —
        // `fatal`, pas un faux repli.
        //
        // ⚠️ **`resize()` (parent de ce fichier) ne détruit PAS son propre
        // encodeur avant d'en construire un neuf** : sa fabrique appelle
        // `H264Encoder::new` pendant que `self.encoder` porte encore l'ancien.
        // C'est le même geste que le Critique ci-dessus décrivait AVANT ce
        // correctif — non exploré ni corrigé ici, hors du périmètre de cette
        // tâche, et nommé pour qu'un futur lecteur ne le redécouvre pas à ses
        // frais.
        self.encoder = None;
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
                // CRITIQUE (revue de la tâche 9) : contrairement au secours
                // de `resize()`, il n'y a ici PLUS d'encodeur de repli valide
                // — `self.encoder` a été détruit avant la tentative (voir le
                // commentaire au-dessus de `self.encoder = None`), justement
                // pour ne jamais demander un encodeur en trop au matériel.
                // `fatal` le dit pour de bon : `next_frame` s'arrête avant de
                // toucher `capture` ou `encoder` (voir leurs gardes), et la
                // session se clôt proprement au lieu de soumettre des images
                // à un encodeur qui n'existe plus.
                self.fatal = true;
                tracing::error!(erreur = %primary_error, sortie = %nom_sortie, "reconstruction après changement de mode échouée, aucun encodeur de repli (détruit avant la tentative) : source déclarée épuisée");
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

/// Attend que la sortie nommée se stabilise, et rend le DERNIER rectangle
/// relu — que la cible ait été atteinte ou non.
///
/// `None` signifie « aucune lecture de topologie n'a réussi dans le budget »
/// (pilote muet, sortie disparue) — PAS « la cible n'a pas été atteinte ».
/// **C'est l'appelant qui juge du refus**, en comparant ce rectangle à
/// `self.width`/`self.height` (la taille AVANT tentative), pas à `cible` :
/// voir l'IMPORTANT 1 de la revue de la tâche 9. Une première version de
/// cette fonction rendait `None` dès que la cible n'était pas atteinte,
/// confondant ainsi « refusé » et « atteint une taille différente de la
/// cible » — cette dernière laissait la sortie changée sans que la chaîne de
/// capture ne soit jamais reconstruite.
///
/// **Relecture par `GetDesc`/`DesktopCoordinates`** (`enumerer_sorties_silencieux`),
/// jamais par WMI : le champ WMI a été vu périmé de 68 s sur ce terrain.
/// Silencieuse, parce que cette boucle interroge jusqu'à trente fois.
///
/// Le critère de sortie ANTICIPÉE du budget est « la taille demandée est
/// atteinte, à `taille_compatible` près » : le pilote quantifie, donc exiger
/// l'égalité stricte ferait attendre inutilement les 3 s pleines sur un
/// succès à 4 px d'écart. Une quantification plus large (88 px relevés en D2)
/// épuise le budget — mais rend tout de même le dernier rectangle lu, que
/// l'appelant reconnaîtra comme « bougé, à reconstruire ».
fn attendre_la_sortie(nom_sortie: &str, cible: (u32, u32)) -> Option<Rect> {
    let debut = std::time::Instant::now();
    let mut dernier_lu: Option<Rect> = None;
    loop {
        if let Ok(sorties) = crate::capture::enumerer_sorties_silencieux() {
            if let Some(sortie) = sorties.iter().find(|s| s.nom_sortie == nom_sortie) {
                dernier_lu = Some(sortie.rect);
                if crate::superviseur::placement::taille_compatible(
                    (sortie.rect.width, sortie.rect.height),
                    cible,
                ) {
                    return dernier_lu;
                }
            }
        }
        if debut.elapsed() >= BUDGET_TOPOLOGIE {
            return dernier_lu;
        }
        std::thread::sleep(PAS_TOPOLOGIE);
    }
}
