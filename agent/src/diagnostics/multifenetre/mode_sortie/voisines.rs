//! Duplications DXGI BRUTES sur des sorties voisines, pour compter les pertes
//! d'accès qu'un changement de mode leur inflige — l'inconnue annexe n°2 de
//! D8 (« combien de pertes d'accès `0x887a0026` un changement de mode
//! inflige-t-il aux voisines ? »).
//!
//! Extrait de `mode_sortie.rs` à la tâche 1 du sous-bloc D9, pour le plafond
//! de 500 lignes (`CLAUDE.md`) — et séparé de
//! `crate::capture::DesktopCapture` par NÉCESSITÉ, pas par convenance : sa
//! fenêtre de reprise (`capture_reprise::FenetreDeReprise`) ABSORBE une perte
//! d'accès transitoire en la rouvrant en silence, sans jamais la remonter à
//! l'appelant tant que la reprise réussit dans les 8 s de sa fenêtre — voir
//! `next_frame` dans `capture.rs`. C'est précisément ce que cette mesure
//! annexe doit observer et compter, pas ce que la production a intérêt à
//! masquer. `crate::capture::ouverture` porte la même logique d'ouverture par
//! sortie, mais ses fonctions sont `pub(super)` du module `capture` : hors de
//! portée d'un module de diagnostic, et les rendre plus visibles toucherait
//! du code de production pour un besoin qui n'est pas le sien.

use std::collections::HashSet;

use anyhow::{anyhow, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
    DXGI_OUTDUPL_FRAME_INFO,
};

use super::super::capture_virtuelle::designer_sortie_neuve;
use super::super::montee::{attendre_en_pinguant, relever_topologie, DELAI_TOPOLOGIE};
use crate::capture::SortieDxgi;
use crate::moniteurs_virtuels::pilote::PiloteParIoctl;
use crate::moniteurs_virtuels::Sorties;

/// Une duplication brute, tenue le temps du tour, sur une sortie qui n'est
/// PAS celle sous test.
pub(super) struct DuplicationVoisine {
    nom: String,
    device: ID3D11Device,
    output: IDXGIOutput1,
    duplication: IDXGIOutputDuplication,
    /// Posé quand une réouverture après perte d'accès a elle-même échoué :
    /// plus rien à sonder sur cette voisine, voir `sonder`.
    morte: bool,
}

impl DuplicationVoisine {
    /// Ouvre par INDEX (`sortie.index_adaptateur`/`index_sortie`) plutôt que
    /// par nom, à la différence de `DesktopCapture::sur_sortie` : `sortie`
    /// vient d'un relevé de topologie que l'appelant vient tout juste de
    /// prendre, et rien ne s'intercale entre ce relevé et cette ouverture qui
    /// puisse décaler ces indices positionnels (voir la doctrine du nom dans
    /// `capture_virtuelle.rs` — elle vaut pour un index conservé À TRAVERS une
    /// mutation, pas pour un index relu et consommé sur-le-champ).
    ///
    /// **⚠️ Cette garantie repose ENTIÈREMENT sur l'appelant : il doit ouvrir
    /// CETTE voisine avant de créer la sortie suivante** — la revue de la
    /// tâche 1 (Critique 1) a trouvé `mode_sortie::executer` en défaut sur ce
    /// point exact (la voisine 1 était ouverte après la création de la
    /// voisine 2, donc sur un index déjà décalé). C'est pourquoi `ouvrir`
    /// reste privée à ce module : la seule voie de construction publique est
    /// désormais `creer_deux`, qui applique l'ordre strict au lieu d'en
    /// dépendre.
    ///
    /// **`SetMultithreadProtected(TRUE)` est posé, et la tension avec
    /// `CLAUDE.md` est tranchée ici plutôt que laissée implicite (Important 1
    /// de la revue de la tâche 1).** Deux affirmations coexistent dans ce
    /// dépôt : `capture::ouverture::creer_peripherique` explique le mécanisme
    /// — SANS cet appel, un périphérique D3D11 sollicité depuis DEUX FILS À
    /// LA FOIS (capture ET Media Foundation, chacun depuis ses propres fils)
    /// peut se bloquer indéfiniment DANS le pilote, sans erreur — quand
    /// `CLAUDE.md` (sous-bloc D3, épreuve de la « sonde minimale ») l'énonce
    /// SANS cette condition : « elle porte inévitablement un `ID3D11Device`
    /// avec `SetMultithreadProtected(true)`, `DuplicateOutput` L'EXIGEANT ».
    /// Cette voisine ne remplit AUCUNE des deux conditions du mécanisme de
    /// `creer_peripherique` (jamais confiée à Media Foundation, jamais
    /// sollicitée que depuis le fil unique de la sonde) — un raisonnement
    /// PLAUSIBLE pour s'en passer. Mais la mesure de D3 dit « `DuplicateOutput`
    /// l'exigeant », sans réserve de partage ni de concurrence, et je n'ai
    /// aucune mesure à moi qui contredise cette formulation plus large.
    /// **Faute de pouvoir départager les deux lectures, je pose l'appel** :
    /// son coût est nul (un drapeau sur le contexte immédiat), et la doctrine
    /// déjà consignée, prise à la lettre, l'exige.
    fn ouvrir(sortie: &SortieDxgi) -> Result<Self> {
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
            .context("fabrique DXGI (duplication d'une voisine)")?;
        let adapter = unsafe { factory.EnumAdapters1(sortie.index_adaptateur) }.with_context(
            || format!("adaptateur introuvable pour la voisine {}", sortie.nom_sortie),
        )?;
        let output: IDXGIOutput1 = unsafe { adapter.EnumOutputs(sortie.index_sortie) }
            .with_context(|| format!("sortie introuvable pour la voisine {}", sortie.nom_sortie))?
            .cast()
            .with_context(|| format!("IDXGIOutput1 pour la voisine {}", sortie.nom_sortie))?;

        let mut device: Option<ID3D11Device> = None;
        let mut contexte = None;
        unsafe {
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                Default::default(),
                // Aucun drapeau : cette voisine ne lit ni ne convertit jamais
                // de pixel, `D3D11_CREATE_DEVICE_BGRA_SUPPORT` n'a donc rien
                // à y faire.
                Default::default(),
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut contexte),
            )
        }
        .with_context(|| format!("périphérique D3D11 pour la voisine {}", sortie.nom_sortie))?;
        let device = device
            .ok_or_else(|| anyhow!("périphérique D3D11 absent (voisine {})", sortie.nom_sortie))?;
        let contexte: ID3D11DeviceContext = contexte
            .ok_or_else(|| anyhow!("contexte D3D11 absent (voisine {})", sortie.nom_sortie))?;

        // Voir le commentaire de tête de cette fonction : posé par doctrine
        // consignée (`CLAUDE.md`, sous-bloc D3), pas parce qu'un mécanisme
        // connu l'exigerait pour CE périphérique-ci. Le contexte lui-même
        // n'est conservé que le temps de cet appel -- rien ensuite ne s'en
        // sert, le périphérique porte l'état posé.
        let multithread: ID3D11Multithread = contexte
            .cast()
            .with_context(|| format!("ID3D11Multithread pour la voisine {}", sortie.nom_sortie))?;
        // Rend l'état PRÉCÉDENT (`BOOL` qu'il faut consommer) : jamais lu
        // ailleurs, mais journalisé plutôt qu'ignoré par un `let _`, même
        // discipline que `capture::ouverture::creer_peripherique`.
        let protection_precedente = unsafe { multithread.SetMultithreadProtected(true) };
        tracing::info!(
            voisine = %sortie.nom_sortie,
            protection_precedente = protection_precedente.as_bool(),
            "protection multi-fils activée sur le contexte immédiat D3D11 (voisine)"
        );

        let duplication = unsafe { output.DuplicateOutput(&device) }
            .with_context(|| format!("duplication de la voisine {}", sortie.nom_sortie))?;
        tracing::info!(voisine = %sortie.nom_sortie, "duplication brute ouverte sur une voisine");

        Ok(Self { nom: sortie.nom_sortie.clone(), device, output, duplication, morte: false })
    }

    /// Sonde une fois, sans bloquer (`AcquireNextFrame(0, ..)`, même
    /// convention que `capture.rs::tenter_acquisition`).
    ///
    /// Rend `true` si CETTE sollicitation détecte une perte d'accès —
    /// auquel cas la duplication est immédiatement rouverte pour que la
    /// sollicitation SUIVANTE puisse en détecter une AUTRE. Sans cette
    /// réouverture, `AcquireNextFrame` rendrait indéfiniment le même refus
    /// sur l'instance périmée : une seule coupure compterait pour autant de
    /// sollicitations qu'il en reste dans le tour, ce qui fausserait
    /// `pertes_acces_voisines` dans le sens dangereux (une majoration muette).
    ///
    /// Ne détecte donc, par construction, qu'AU PLUS une perte par appel —
    /// si plusieurs coupures distinctes survenaient entre deux sollicitations,
    /// elles se compteraient pour une seule. Ce n'est pas un défaut caché :
    /// c'est la granularité du tour, qui ne sonde qu'une fois par tentative
    /// de changement de mode (voir son appelant).
    pub(super) fn sonder(&mut self) -> bool {
        if self.morte {
            return false;
        }
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        match unsafe { self.duplication.AcquireNextFrame(0, &mut info, &mut resource) } {
            Ok(()) => {
                let _ = unsafe { self.duplication.ReleaseFrame() };
                false
            }
            // Rien de neuf, ou une panne étrangère à la question posée ici :
            // dans les deux cas, rien à compter.
            Err(e) if !crate::capture_reprise::est_acces_perdu(e.code().0) => false,
            Err(e) => {
                tracing::info!(
                    voisine = %self.nom,
                    hresult = format!("{:#010x}", e.code().0),
                    "perte d'acces detectee sur une voisine -- reouverture pour continuer a compter"
                );
                match unsafe { self.output.DuplicateOutput(&self.device) } {
                    Ok(fraiche) => self.duplication = fraiche,
                    Err(erreur) => {
                        // Cette coupure-CI compte tout de même : elle a bien
                        // eu lieu. Ce sont les SUIVANTES, sur cette voisine,
                        // qui ne seront plus comptées -- `morte` l'empêche de
                        // relire indéfiniment la même instance périmée.
                        self.morte = true;
                        tracing::warn!(
                            voisine = %self.nom,
                            %erreur,
                            "reouverture de la voisine apres perte d'acces : echouee -- les \
                             pertes suivantes ne seront plus comptees sur cette voisine"
                        );
                    }
                }
                true
            }
        }
    }
}

/// Crée, désigne ET OUVRE les deux voisines — dans cet ordre STRICT, pour
/// CHACUNE, sans qu'aucune création ne s'intercale entre le relevé d'une
/// voisine et l'ouverture de SA duplication (voir la doctrine de `ouvrir`
/// ci-dessus, et Critique 1 de la revue de la tâche 1). C'est pour rendre
/// cet ordre STRUCTUREL, plutôt qu'une discipline que l'appelant devrait
/// respecter de lui-même, que `ouvrir` est privée et que ceci est l'unique
/// point d'entrée public de ce module.
///
/// Met à jour `connues_a_ce_point` au passage : l'appelant en a encore
/// besoin pour désigner la sortie témoin ensuite. Rend les NOMS des deux
/// voisines et non leurs `SortieDxgi` entières — tout ce dont l'appelant se
/// sert au-delà de cette fonction.
pub(super) fn creer_deux(
    pilote: &PiloteParIoctl,
    sorties: &mut Sorties<'_>,
    connues_a_ce_point: &mut HashSet<String>,
    largeur: u32,
    hauteur: u32,
    hertz: u32,
) -> Result<(DuplicationVoisine, DuplicationVoisine, String, String)> {
    let id_v1 = sorties.creer(largeur, hauteur, hertz)?;
    attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
    let apres_v1 = relever_topologie("après création (voisine 1)")?;
    let sortie_v1 = designer_sortie_neuve(&apres_v1, connues_a_ce_point, id_v1)?.clone();
    connues_a_ce_point.insert(sortie_v1.nom_sortie.clone());
    let voisine1 = DuplicationVoisine::ouvrir(&sortie_v1)?;

    let id_v2 = sorties.creer(largeur, hauteur, hertz)?;
    attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
    let apres_v2 = relever_topologie("après création (voisine 2)")?;
    let sortie_v2 = designer_sortie_neuve(&apres_v2, connues_a_ce_point, id_v2)?.clone();
    connues_a_ce_point.insert(sortie_v2.nom_sortie.clone());
    let voisine2 = DuplicationVoisine::ouvrir(&sortie_v2)?;

    Ok((voisine1, voisine2, sortie_v1.nom_sortie, sortie_v2.nom_sortie))
}
