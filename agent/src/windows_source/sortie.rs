//! Construction d'une `WindowsSource` sur une sortie DXGI, et le discriminant
//! de mode qui distingue ce cas de l'autre.
//!
//! C'est le mode du sous-bloc D1 : une fenêtre par sortie virtuelle.
//!
//! ❌ **Ce paragraphe disait « donc plus rien à recadrer — la sortie *est* la
//! fenêtre », et le sous-bloc D10 l'a réfuté** (tâches 4 à 9). Une sortie
//! virtuelle **ne naît pas à la taille demandée** mais à la dernière taille
//! laissée au registre : elle peut donc être PLUS GRANDE que la fenêtre. Le
//! superviseur l'accepte désormais au lieu de la rendre au pilote, y pose la
//! fenêtre à la **taille retenue**, et `sur_sortie` ci-dessous **recadre ce
//! rectangle dans la duplication de la sortie**. Il reste donc bien un
//! recadrage, et la sortie n'est la fenêtre que dans le cas — non garanti —
//! où elle naît à la taille demandée.
//!
//! ❌ **La phrase qui suivait — « Ce qui n'a pas changé : `resize` ne retaille
//! toujours pas la fenêtre en ce mode » — est FAUSSE depuis le lot 33.**
//! `resize` retaille désormais la fenêtre et refait le recadrage pour suivre
//! le viewport, **sans jamais toucher au mode d'affichage de la sortie** :
//! voir `ModeCapture::suit_le_viewport` et `taille_pour_viewport` ci-dessous.
//!
//! Deux moitiés, séparées par un `#[cfg(windows)]` en milieu de fichier :
//! au-dessus, le calcul pur de région et `ModeCapture`, tous deux testables sur
//! l'hôte Linux ; en dessous, le constructeur réel, qui manipule des types COM.

use crate::geometry::Rect;

/// Ce que la source capture — donc ce qu'un redimensionnement demandé par le
/// navigateur doit faire.
///
/// **Le discriminant manquait, et son absence était un défaut de sûreté.**
/// `WindowsSource::new` et `WindowsSource::sur_sortie` convergeaient tous deux
/// sur le même assemblage, sans rien retenir de leur différence : `resize`
/// appliquait donc le chemin « recadrage de fenêtre » y compris à une source
/// qui capture une sortie entière. Sur ce chemin-là, la fenêtre était
/// rétrécie (elle quittait sa sortie virtuelle, que le contrôle périodique du
/// superviseur tentait aussitôt de rattraper), la duplication de la sortie
/// était relâchée, et `DesktopCapture::new()` dupliquait **le bureau physique
/// primaire** — après quoi le repli de secours installait cette duplication-là
/// avec une région calculée pour la sortie virtuelle. La fenêtre du navigateur
/// affichait alors un coin du bureau réel de la VM à la place de son
/// application, pour un seul `warn!`. En mono-fenêtre ce repli était
/// acceptable ; en multi-fenêtres il fait fuir le contenu d'un moniteur vers
/// la session d'autrui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeCapture {
    /// Le bureau entier est capturé, puis recadré sur la fenêtre. La fenêtre
    /// est redimensionnable et la région de recadrage suit sa taille : c'est
    /// le mode mono-fenêtre historique.
    FenetreRecadree,
    /// Une sortie DXGI est capturée, sans fenêtre Windows à suivre : la
    /// fenêtre occupe sa propre sortie virtuelle. Mode du sous-bloc D1.
    ///
    /// ❌ **Le nom de cette variante, et le texte qu'elle portait — « la
    /// sortie **est** la fenêtre » —, ne décrivent plus le comportement
    /// depuis le sous-bloc D10.** Une sortie peut naître PLUS GRANDE que la
    /// taille demandée (registre pollué), auquel cas `sur_sortie` recadre la
    /// **taille retenue** à l'origine de la sortie. Le nom est conservé
    /// plutôt que renommé : ce qu'il discrimine réellement — et la seule
    /// chose dont dépende `recapture_le_bureau` ci-dessous — est **l'absence
    /// de bureau physique à recapturer**, qui reste vraie.
    ///
    /// ❌ **Ce texte disait « l'absence de fenêtre Windows à retailler », et
    /// le lot 33 l'a réfuté** : il y a bien une fenêtre Windows, et elle est
    /// désormais retaillée pour suivre le viewport. Ce que ce mode discrimine
    /// vraiment, et a toujours discriminé, est qu'on capture **une sortie
    /// DXGI** et non le bureau — d'où le renommage du prédicat.
    SortieEntiere,
}

impl ModeCapture {
    /// Vrai si un redimensionnement doit **relâcher la duplication courante et
    /// recapturer le bureau** — le chemin mono-fenêtre historique.
    ///
    /// ❌ **CETTE MÉTHODE S'APPELAIT `redimensionne_la_fenetre`, ET CE NOM EST
    /// DEVENU UN MENSONGE AU LOT 33.** Depuis ce lot, `SortieEntiere` retaille
    /// bel et bien la fenêtre Windows (voir `suit_le_viewport` ci-dessous) :
    /// un prédicat nommé « redimensionne la fenêtre » qui rend `false` pour un
    /// mode qui la redimensionne est exactement le patron que `CLAUDE.md`
    /// nomme « le naufrage du 487 ». Le nom dit désormais ce que le prédicat
    /// discrimine RÉELLEMENT, et ce qu'il a toujours discriminé : **relâcher
    /// la duplication pour en ouvrir une du bureau physique**.
    ///
    /// Faux en `SortieEntiere`, et c'est le correctif C1 de D1 : sur ce
    /// chemin-là, relâcher la duplication de la sortie virtuelle pour
    /// `DesktopCapture::new()` faisait diffuser **le coin du bureau physique
    /// de la VM** dans la fenêtre du navigateur — en multi-fenêtres, une fuite
    /// du contenu d'un moniteur vers la session d'autrui.
    pub fn recapture_le_bureau(self) -> bool {
        matches!(self, ModeCapture::FenetreRecadree)
    }

    /// Vrai si un redimensionnement doit **suivre le viewport à l'intérieur
    /// d'une sortie qui ne bouge pas** : retailler la fenêtre, refaire le
    /// recadrage et l'encodeur, **sans jamais toucher à la duplication**.
    ///
    /// 🔴 **CE BRAS EST NEUF AU LOT 33, ET IL REMPLACE UN `Ok(())` QUI NE
    /// FAISAIT RIEN.** Mesuré sur le produit en production le 31 août 2026
    /// (`C:\nivuus\agent.log`, 34 demandes) : le navigateur a demandé des
    /// rapports d'aspect allant de **1,105 à 3,559** — dont un `5118x1438` —
    /// pendant que la fenêtre restait servie à **1428×1080, soit 1,3222**, et
    /// **les 34 ont été jetées**. Le `#remote` du client étant
    /// `width:100vw; height:100vh; object-fit:contain;
    /// background:var(--video-letterbox)` (`client/src/style.css`), l'écart se
    /// peint littéralement en `#000` de chaque côté de l'image : ce sont les
    /// « bordures noires » rapportées par le propriétaire.
    ///
    /// ⚠️ **CE N'EST PAS LA RÉSURRECTION DU CHEMIN QUE D9 A RETIRÉ.** D8
    /// faisait suivre **la SORTIE** au viewport par `ChangeDisplaySettingsExW`
    /// ; D9 l'a mesuré et retiré (le changement ne survit pas à l'ouverture de
    /// la fenêtre suivante, et `CDS_UPDATEREGISTRY` pollue le registre au
    /// point de bloquer le produit — constat en tête de
    /// `capteur/plein_ecran.rs`). **Rien ici ne change de mode d'affichage** :
    /// la sortie garde la taille à laquelle elle est née, et seuls **le
    /// recadrage** et **la fenêtre Windows** bougent à l'intérieur.
    ///
    /// ⚠️ **Ce que ce bras NE PEUT PAS FAIRE** : grandir au-delà de la sortie.
    /// `taille_pour_viewport` borne par un `min` axe par axe ; un viewport
    /// plus large que la sortie (le `5118x1438` mesuré) reste servi à la
    /// taille de la sortie, et les bandes noires demeurent. Sans changement de
    /// mode — que D9 interdit —, il n'y a pas d'autre issue.
    pub fn suit_le_viewport(self) -> bool {
        matches!(self, ModeCapture::SortieEntiere)
    }
}

/// La borne à laquelle une fenêtre servie, et le recadrage qui la suit,
/// doivent se tenir : **la ZONE DE TRAVAIL de la sortie, pas son rectangle**.
///
/// 🔴 **RIEN, DANS TOUT LE DÉPÔT, NE CONSULTAIT LA ZONE DE TRAVAIL AVANT CE
/// LOT** — `grep -rni 'rcWork\|SPI_GETWORKAREA\|zone_de_travail'` sur
/// `agent/ client/ plateforme/ proto/` rendait **zéro**. La distinction
/// moniteur / zone de travail n'existait pas dans ce produit, et c'est la
/// cause d'un défaut mesuré le 31 août 2026 en session 1 (voir
/// `docs/superpowers/plans/2026-08-31-barre-des-taches-diagnostic.md`) :
///
/// ```text
/// device=\\.\DISPLAY8 mon=(1280,0)-(2708,1080) 1428x1080 work=1428x1032
/// hwnd=0x201DC cls=Shell_SecondaryTrayWnd rect=(1280,1032)-(2708,1080) 1428x48
/// hwnd=0x60242 cls=Notepad                rect=(1280,0)-(2708,1080) 1428x1080
/// ```
///
/// **Une barre des tâches SECONDAIRE de 48 px est collée en bas de CHACUNE des
/// sorties servies** (le défaut Windows, `MMTaskbarEnabled` absente), et la
/// fenêtre est posée au rectangle du MONITEUR, pas à sa zone de travail. Trois
/// conséquences, dont deux que le symptôme ne disait pas :
///   ① la barre est **dans le recadrage** — 48 des 1080 rangées, 4,4 % de
///      l'image ;
///   ② l'application **perd 48 px de contenu** : sa fenêtre fait bien 1080, et
///      ses dernières rangées sont **recouvertes** — ce n'est pas une bande
///      ajoutée sous elle ;
///   ③ **un clic dans les 4,4 % bas de la vidéo atteint la barre des tâches**,
///      pas l'application (`input.rs::move_mouse` démappe sur la taille
///      d'image, donc `y = 65535` tombe dans la barre).
///
/// ⚠️ **CE BORNAGE NE DOIT PAS ÊTRE LIVRÉ SEUL.** Pris isolément, recadrer sur
/// la zone de travail **ajoute** une bande de letterbox de 48 px sous l'image
/// — c'est-à-dire précisément ce dont le propriétaire se plaint. Il ne vaut
/// qu'accompagné du suivi de viewport (`taille_pour_viewport` ci-dessous), qui
/// fait épouser à l'image le rapport d'aspect demandé.
///
/// `travail` est un `Option` parce que `GetMonitorInfoW` peut échouer : le
/// repli est **le rectangle du moniteur, c'est-à-dire le comportement exact
/// d'avant ce lot**. Un repli qui rendrait autre chose ferait dépendre le
/// cadrage d'un appel qui échoue silencieusement.
///
/// Une zone de travail **dégénérée** (nulle sur un axe) est refusée pour la
/// même raison : Windows la rend ainsi le temps d'une transition, et s'y fier
/// donnerait une fenêtre de deux pixels.
pub fn borne_de_la_sortie(moniteur: (u32, u32), travail: Option<(u32, u32)>) -> (u32, u32) {
    match travail {
        Some((l, h)) if l >= 2 && h >= 2 => (l.min(moniteur.0), h.min(moniteur.1)),
        _ => moniteur,
    }
}

/// La taille à laquelle une source en mode `SortieEntiere` doit se recadrer,
/// et à laquelle sa fenêtre Windows doit être posée, pour un viewport donné.
///
/// 🔴 **C'EST LA FONCTION QUI EMPÊCHE LES DEUX PROCESSUS DE SE BATTRE, ET
/// C'EST TOUTE LA CONCEPTION DE CE LOT.** La fenêtre Windows a **deux**
/// prétendants : le CAPTEUR, qui reçoit le `Resize` du navigateur, et le
/// SUPERVISEUR, qui repose la fenêtre **chaque seconde** sur la taille que sa
/// table retient (`boucle::placement_periodique::replacer_si_besoin`, cible
/// lue dans `Table::taille_sortie_de`). Un remède qui ne vivrait que dans le
/// capteur serait **défait une seconde plus tard**, et le symptôme serait
/// « ça marche, puis ça revient ».
///
/// La parade n'est ni un verrou ni un message de plus : c'est que **les deux
/// processus calculent la MÊME valeur par CETTE fonction**, à partir des mêmes
/// deux entrées — le viewport annoncé par le navigateur, et la borne rendue
/// par `borne_de_la_sortie` pour le MÊME moniteur (le superviseur l'interroge
/// par l'origine de la sortie, le capteur par sa fenêtre, qui est dessus :
/// même `HMONITOR`, même réponse). Quel que soit celui qui agit le premier, le
/// geste de l'autre est alors un `no-op` : `doit_etre_replacee` ne voit aucun
/// écart, et le court-circuit de `suivre_le_viewport` retourne sans rien
/// reconstruire.
///
/// ⚠️ **Et si les deux divergeaient quand même**, le désaccord serait **borné
/// et s'auto-résout** : le superviseur repose la fenêtre au tour suivant, donc
/// en une seconde au plus (`PERIODE_PLACEMENT`), et le capteur suit au
/// `Resize` suivant. Une divergence coûte une image mal cadrée, jamais une
/// oscillation sans fin.
///
/// Composition de deux règles qui existaient déjà, **réutilisées et non
/// recopiées** :
///   ① `borner_a_la_taille_max` — le viewport arrive en pixels périphériques
///      depuis D9, et un client à `devicePixelRatio > 1` s'engouffrerait sans
///      limite ; c'est le bornage que `creer_sortie` et `viewport_recu`
///      appliquent déjà, aux deux points d'entrée de la création ;
///   ② `superviseur::placement::taille_retenue` — `min` axe par axe, pair et
///      jamais nul : on **recadre** une texture, on ne la met pas à l'échelle,
///      donc aucun rapport d'aspect à préserver ici.
pub fn taille_pour_viewport(demande: (u32, u32), borne: (u32, u32)) -> (u32, u32) {
    // ⑵ **FIT À RAPPORT D'ASPECT PRÉSERVÉ, ET NON UN `min` AXE PAR AXE.**
    //
    // 🔴 **CE FUT UN DÉFAUT RÉEL, MESURÉ SUR LE PRODUIT EN PRODUCTION, ET IL
    // ÉTAIT DE MON FAIT.** La première rédaction composait
    // `borner_a_la_taille_max` (qui préserve l'aspect) avec
    // `placement::taille_retenue` (un `min` axe par axe, qui ne le préserve
    // pas) — si bien que **le second détruisait ce que le premier venait de
    // garantir**. Capture réseau du 31 août 2026, 28 trames `viewport`
    // relayées pendant que le propriétaire tirait les bords de sa fenêtre :
    //
    // ```text
    // demandé 1723x1303 (1,322) -> servi 1428x1032 (1,384) : ecart 4,6 %
    // demandé 2058x851  (2,418) -> servi 1860x794  (2,343) : ecart 3,1 %
    // demandé 1922x1092 (1,760) -> servi 1860x1032 (1,802) : ecart 2,4 %
    // ```
    //
    // Un écart d'aspect de quelques pour cent est **exactement** une bande de
    // `--video-letterbox` (`#000`) le long d'une paire de bords, dont
    // l'épaisseur **varie avec le rapport demandé** — ce que le propriétaire a
    // décrit mot pour mot (« la taille des bordures est différente selon la
    // taille/ratio »).
    //
    // ⚠️ **CE N'ÉTAIT PAS LA LIMITE DÉCLARÉE, ET LES CONFONDRE A COÛTÉ UN
    // ALLER-RETOUR.** La limite dit « on ne peut pas grandir AU-DELÀ de la
    // borne » ; ce défaut-ci servait **une forme fausse À L'INTÉRIEUR de la
    // borne**. `1723x1303` tient parfaitement en `1364x1032` — sous la borne
    // sur les deux axes, et au rapport exact.
    //
    // ⚠️ **LE `min` AXE PAR AXE RESTE JUSTE LÀ D'OÙ IL VIENT** :
    // `placement::taille_retenue` recadre une texture dans une sortie née trop
    // grande, où il n'y a aucun rapport d'aspect à honorer. Ici on choisit la
    // FORME du rectangle servi, et cette forme doit être celle que le
    // navigateur demande. Deux besoins, deux règles ; c'est de les avoir
    // confondus que venait le défaut.
    //
    // ⚠️ **LE PLAFOND À `1.0` N'AGRANDIT JAMAIS UNE PETITE DEMANDE**, et
    // ❌ **ce n'est PAS lui qui garde la barre des tâches hors du cadre —
    // la première rédaction de ce commentaire l'affirmait, et la mutation l'a
    // réfutée.** Retirer le plafond laisse le résultat DANS la borne (il ne
    // fait que monter jusqu'à elle) : la barre ne rentre pas pour autant, et
    // le test qui prétendait garder cette propriété est resté VERT sous la
    // mutation. C'est `borne_de_la_sortie`, et elle seule, qui sort la barre
    // du cadre.
    //
    // Ce que le plafond fait réellement : **ne jamais servir une image PLUS
    // GRANDE que ce que le navigateur a demandé.** Sans lui, un viewport de
    // 900×500 serait encodé en 1854×1030 — quatre fois les macroblocs, pour
    // des pixels que la page ne peut pas afficher.
    let (dl, dh) = borner_a_la_taille_max(demande);
    let f = f64::min(
        f64::min(borne.0 as f64 / dl as f64, borne.1 as f64 / dh as f64),
        1.0,
    );
    // `.max(2)` puis `& !1` : l'encodeur NV12 exige des dimensions paires, et
    // une boîte vidéo repliée émet `(0, 0)` (cas réel de D8).
    let mets = |x: u32| (((x as f64 * f).round() as u32).max(2)) & !1;
    // ⚠️ L'arrondi peut, sur un seul axe, rendre un pixel de plus que la
    // borne (`round` monte). Le `min` final est un filet : `region_de_sortie`
    // doit rester DANS la texture, et une région qui déborde ferait échouer
    // le recadrage.
    (mets(dl).min(borne.0 & !1).max(2), mets(dh).min(borne.1 & !1).max(2))
}

/// Taille maximale qu'une sortie virtuelle prendra sur demande de viewport.
///
/// ⚠️ **NON CALIBRÉE.** C'est un garde-fou posé par prudence, sans qu'aucun
/// jugement visuel ne l'ait jugée — exactement la lacune que `BPP_MIN` traîne
/// depuis le chantier C volet 1. Sa raison est mesurée, elle : D6 a relevé le
/// décodeur du navigateur saturé dès huit fenêtres de 1280×720 (18,03 %
/// d'images jetées au barreau plein, une exécution), et un écran 4K
/// demanderait 9× les pixels d'une seule de ces fenêtres.
pub const TAILLE_MAX_SORTIE: (u32, u32) = (1920, 1080);

/// Ramène une taille demandée sous `TAILLE_MAX_SORTIE`, à rapport d'aspect
/// préservé, en dimensions paires, et jamais nulle.
///
/// ⚠️ **IMPORTANT 4 (revue de la tâche 9) : la branche rapide ci-dessous
/// n'appliquait aucun plancher**, contrairement à la branche d'échelle
/// (`.max(2)` déjà présent dessus). `(0, 0)` — une boîte vidéo réduite à
/// rien, transitoirement vraie pendant une fenêtre repliée ou une transition
/// de plein écran — y passait tel quel. Ce `.max(2)` reste un filet minimal.
///
/// ❌ **CETTE FONCTION A RETROUVÉ SES APPELANTS, et c'est FAUX de dire ici
/// qu'elle n'en a aucun — corrigé à la tâche 8 du sous-bloc D10.** Elle est
/// restée sans appelant du retrait du changement de mode de sortie (D9,
/// tâche 3) jusqu'au leg 5 de D9, fermé par les tâches 6 et 7 de CE
/// sous-bloc : `superviseur::boucle::creation_sortie::creer_sortie` la
/// borne à la CRÉATION d'une sortie, et `superviseur::table::attribution::viewport_recu`
/// la borne à la RÉUTILISATION d'une sortie retenue, par le même bornage et
/// pour la même raison — le viewport annoncé par le navigateur arrive en
/// pixels périphériques depuis D9, et sans ce plafond une demande à
/// `devicePixelRatio > 1` s'y engouffrerait sans limite. Son rôle propre
/// reste borné au PLAFOND (`TAILLE_MAX_SORTIE`) ; le plancher applicatif de
/// 160×120 qu'appliquait l'ancien appelant (le changement de mode retiré par
/// D9) n'existe toujours nulle part.
///
/// ✅ **La TAILLE DE CRÉATION d'une sortie EST DÉSORMAIS BORNÉE**, aux deux
/// points d'entrée ci-dessus. **Aucun client HiDPI réel n'a été mesuré pour
/// autant** : le montage de recette reste un Chrome sans interface, et rien
/// n'a exercé `deviceScaleFactor > 1` sur le COÛT (huit fenêtres à
/// `TAILLE_MAX_SORTIE` plutôt qu'à 720p, plafond d'encodeurs jamais mesuré
/// au-delà de 720p) — seule la symétrie d'unité l'a été (D9, tâche 5). Cette
/// dernière phrase est la même réserve que D9 laissait ; le bornage qui la
/// suivait manquait, et c'est lui qui est corrigé ici, pas la réserve
/// elle-même.
pub fn borner_a_la_taille_max((l, h): (u32, u32)) -> (u32, u32) {
    let (max_l, max_h) = TAILLE_MAX_SORTIE;
    if l <= max_l && h <= max_h {
        return (l.max(2) & !1, h.max(2) & !1);
    }
    // Le facteur le plus contraignant des deux axes : borner chaque axe
    // séparément déformerait l'image.
    let facteur = f64::min(max_l as f64 / l as f64, max_h as f64 / h as f64);
    let borne = |x: u32| (((x as f64 * facteur).round() as u32).max(2)) & !1;
    (borne(l), borne(h))
}

/// Région à capturer dans la texture d'une sortie dupliquée.
///
/// **Relative à la sortie, pas au bureau virtuel.** `DesktopCapture::sur_sortie`
/// rend une texture qui couvre cette sortie seule ; son origine dans l'espace
/// du bureau virtuel (par exemple x=2400 pour une sortie posée à droite du
/// bureau physique) n'y a aucun sens. Passer les coordonnées de bureau
/// donnerait une image décalée ou vide.
///
/// Les dimensions sont alignées sur des valeurs paires : l'encodeur NV12 les
/// exige, et une sortie virtuelle créée à une taille impaire par un viewport
/// impair est un cas réel.
///
/// ⚠️ **Depuis le sous-bloc D10, l'appelant lui passe la taille RETENUE, pas
/// celle de la sortie** : une sortie née trop grande (registre pollué) est
/// acceptée et recadrée à l'origine. La fonction elle-même est inchangée —
/// c'est son argument qui a changé de sens.
pub fn region_de_sortie(largeur: u32, hauteur: u32) -> Option<Rect> {
    let largeur = largeur & !1;
    let hauteur = hauteur & !1;
    if largeur < 2 || hauteur < 2 {
        return None;
    }
    Some(Rect { x: 0, y: 0, width: largeur, height: hauteur })
}

// Les tests d'hôte de ce module vivent dans `sortie/tests.rs`, extrait là
// dans une tâche DÉDIÉE et AVANT l'addition du lot 33, qui aurait porté ce
// fichier au-delà du plafond de 500 lignes. `#[path]` plutôt qu'un
// sous-répertoire de module : précédent de `superviseur/table.rs`.
#[cfg(test)]
#[path = "sortie/tests.rs"]
mod tests;

// Le bloc ci-dessous ne compile que sous Windows : il construit une
// `WindowsSource` réelle (types COM `HWND`/`DesktopCapture`/`H264Encoder`,
// tous eux-mêmes gated `#[cfg(windows)]`). `region_de_sortie` et ses tests
// restent AU-DESSUS de ce `cfg`, à portée du module, pour continuer de
// tourner sur l'hôte — voir la déclaration `#[path]` de ce fichier comme
// module `windows_source_sortie`, hors de tout `#[cfg(windows)]`, dans
// `main.rs`.
//
// Ce module (`windows_source_sortie`) est un FRÈRE de `windows_source`, pas
// un descendant (tous deux déclarés séparément à la racine du crate, voir
// `main.rs`) : la visibilité privée par défaut de Rust ne donnerait donc PAS
// accès aux champs de `WindowsSource` depuis ici.
//
// **C'est pourquoi `sur_sortie` n'assemble PAS le littéral `Self { … }`
// lui-même** et passe par `WindowsSource::depuis_pieces`, restée dans
// `windows_source.rs` en `pub(crate) fn`. Une première version avait fait
// l'inverse — migrer aussi `depuis_pieces` — ce qui obligeait à ouvrir les
// TREIZE champs de `WindowsSource` en `pub(crate)`, dont `capture` et `fatal`
// qui portent un invariant inter-champs documenté comme fragile (voir leurs
// commentaires). Un appel `pub(crate)` à une fonction unique coûte une ligne
// et n'ouvre rien.
#[cfg(windows)]
use anyhow::{Context, Result};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

#[cfg(windows)]
use crate::capture::DesktopCapture;
#[cfg(windows)]
use crate::encode::H264Encoder;
#[cfg(windows)]
use crate::windows_source::WindowsSource;

#[cfg(windows)]
impl WindowsSource {
    /// Construit une source capturant une sortie DXGI, recadrée à la taille
    /// RETENUE.
    ///
    /// Mode du sous-bloc D1 : la fenêtre a sa propre sortie virtuelle, il n'y
    /// a donc aucune fenêtre Windows à suivre — `resize` continue de ne rien
    /// faire (`ModeCapture::SortieEntiere`). ⚠️ **Mais depuis le sous-bloc
    /// D10, la sortie elle-même peut naître PLUS GRANDE que ce que le
    /// superviseur a demandé** (registre pollué, voir le constat de mesure en
    /// tête de `capteur/plein_ecran.rs`) : `taille` porte ce que le
    /// superviseur a retenu, et la région capturée est recadrée à l'origine
    /// de la sortie sur cette taille-là — jamais sur la sortie entière si
    /// elle déborde. `hwnd` reste renseigné — l'injection d'entrée et le
    /// contrôle de vie en ont besoin — mais il ne sert toujours pas au calcul
    /// de la région.
    pub fn sur_sortie(
        hwnd: HWND,
        nom_sortie: &str,
        taille: (u32, u32),
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
    ) -> Result<Self> {
        let capture = DesktopCapture::sur_sortie(nom_sortie)?;
        let (dw, dh) = capture.desktop_size();
        // La sortie peut être PLUS GRANDE que la fenêtre depuis le sous-bloc
        // D10 : on recadre à l'origine de la sortie, là où le superviseur a
        // posé la fenêtre.
        //
        // 🔴 **LA BORNE EST LA ZONE DE TRAVAIL DEPUIS LE LOT 33, PLUS LA
        // TEXTURE SEULE** — c'est ce qui sort les 48 rangées de la barre des
        // tâches secondaire du recadrage dès la PREMIÈRE image, et non
        // seulement au premier redimensionnement. La texture reste une borne
        // (`min` ci-dessous) : elle est en pixels de TEXTURE quand `rcWork`
        // est en coordonnées de BUREAU, et les deux ne coïncident pas sur
        // cette machine (1860 contre 1428 en largeur, lot 32T). Le repli d'un
        // `GetMonitorInfoW` en échec est le comportement d'avant ce lot.
        let borne = match crate::window::zones_du_moniteur_de(hwnd) {
            Ok((moniteur, travail)) => {
                let b = borne_de_la_sortie(
                    (moniteur.width, moniteur.height),
                    Some((travail.width, travail.height)),
                );
                (b.0.min(dw), b.1.min(dh))
            }
            Err(erreur) => {
                tracing::warn!(%erreur, "zone de travail illisible : recadrage borné par la texture");
                (dw, dh)
            }
        };
        let (rl, rh) = taille_pour_viewport(taille, borne);
        let region = region_de_sortie(rl, rh).with_context(|| {
            format!("sortie {nom_sortie} de dimensions inexploitables ({dw}x{dh})")
        })?;
        let (width, height) = (region.width, region.height);

        let mut encoder =
            H264Encoder::new(capture.device(), (width, height), (width, height), fps, bitrate)?;
        encoder.request_keyframe()?;

        Ok(Self::depuis_pieces(
            hwnd,
            capture,
            encoder,
            region,
            width,
            height,
            fps,
            bitrate,
            clock_origin,
            // Le discriminant qui manquait : sans lui, `resize` retaillerait
            // cette fenêtre-ci et lui substituerait le bureau physique.
            ModeCapture::SortieEntiere,
        ))
    }
}
