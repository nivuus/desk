//! Résolution d'un point de terminaison audio de **rendu**, pour DEUX
//! consommateurs aux politiques opposées.
//!
//! | Consommateur | Ce qu'il cherche | Ce qu'il fait d'un échec |
//! | --- | --- | --- |
//! | [`resoudre`] — le loopback (correction « A-bis », `wasapi.rs`) | le rendu que la machine JOUE, à capter | **se replie** sur le défaut de Windows, en `warn!` |
//! | [`resoudre_cable`] — le micro (bloc E2, `windows_micro.rs`) | le rendu du CÂBLE, sur lequel écrire | **refuse**, et il n'y a pas de micro |
//!
//! 🔴 **Cette asymétrie de repli est le fait de conception de ce module, et
//! elle n'est pas une inconséquence.** A-bis se replie parce que « du son,
//! peut-être le mauvais, et un `warn!` qui le dit » vaut mieux que « aucun
//! son ». Pour le micro l'arbitrage **s'inverse** : « la voix de l'utilisateur,
//! peut-être dans le mauvais périphérique » n'est pas un moindre mal, c'est une
//! **fuite** — sur une machine où le défaut de Windows est la carte son, cette
//! voix sortirait des haut-parleurs de la VM. Mieux vaut pas de micro qu'un
//! micro dans le mauvais tuyau.
//!
//! ⚠️ **Le module n'a jamais eu qu'un seul sens : RÉSOUDRE.** Ce sont ses
//! appelants qui ont des sens contraires — l'un capte, l'autre écrit — et
//! chacun garde le sien. C'est ce qui a fait préférer, au bloc E2, un second
//! point d'entrée ici plutôt qu'une centaine de lignes de COM recopiées
//! ailleurs : le jumeau du chemin de production que la tâche 8 de E1 avait
//! précisément dû supprimer.
//!
//! ⚠️ **[`defaut`] n'est atteignable que par [`resoudre`].** Le chemin du câble
//! ne l'appelle jamais, et un test garde le prédicat pur qui le lui interdit
//! (`wasapi_peripherique::demande_cable`, qui ne rend jamais `None`).
//!
//! La règle de sélection, elle, est **pure** et vit dans
//! `agent/src/wasapi/peripherique.rs` (hissée à la racine du crate par
//! `#[path]`, cf. `main.rs`) : ce module-ci ne fait que traduire l'énumération
//! COM vers ses types, l'interroger, et **journaliser ce qui est retenu**.
//!
//! ## Le défaut de Windows n'est plus une dépendance implicite
//!
//! Jusqu'ici `LoopbackCapture::open` appelait directement
//! `GetDefaultAudioEndpoint(eRender, eConsole)`. Le 19 août 2026,
//! l'installation de VB-Cable sur la VM (préparation du chantier E) a fait
//! basculer ce défaut sur le câble virtuel : le produit s'est mis à capter du
//! silence, **sans qu'aucune ligne de journal ne le dise**. La correction
//! n'est pas de remettre le bon périphérique par défaut — cela corrigerait
//! l'occurrence et laisserait la classe de panne — mais de laisser
//! l'exploitant **désigner explicitement** son périphérique, et de tracer
//! celui qui est réellement retenu à chaque ouverture.
//!
//! ## `AUDIO_PERIPHERIQUE` — convention VALUÉE, absence = comportement d'avant
//!
//! Ce dépôt a trois conventions de variable, et il fallait choisir :
//!
//! - `PLEIN_ECRAN=0` **désarme** un mécanisme livré, et une simple présence
//!   n'arme pas (sans quoi écrire `PLEIN_ECRAN=0` l'activerait) ;
//! - `SOURCE_TRACE` s'active par **simple présence** ;
//! - `MULTIFENETRE_SORTIE=<\\.\DISPLAYn>` et `BUDGET_BPS=<bps>` portent une
//!   **valeur** qui désigne ou règle.
//!
//! `AUDIO_PERIPHERIQUE` suit la **troisième**, et c'est
//! `MULTIFENETRE_SORTIE` qui est le précédent exact : comme elle, elle
//! désigne une cible par un **nom stable** plutôt que par un rang. Les deux
//! premières conventions sont hors sujet ici — il n'y a rien à armer ni à
//! désarmer, il y a une cible à nommer, et une cible n'a pas de valeur
//! booléenne. **Absente (ou vide), le comportement est exactement celui
//! d'avant la correction** : le rendu par défaut de Windows. Aucune
//! régression pour un agent lancé sans elle.

#![cfg(windows)]

use anyhow::{bail, Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IMMDevice, IMMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoTaskMemFree, STGM_READ};

use crate::micro::boucle_locale;
use crate::wasapi_peripherique::{choisir, demande_cable, inventaire, Choix, Peripherique};

/// Nom de la variable d'environnement du LOOPBACK. Voir la convention en tête
/// de module.
pub const VARIABLE: &str = "AUDIO_PERIPHERIQUE";

/// Nom de la variable d'environnement du CÂBLE. Convention **valuée** elle
/// aussi, et pour la même raison : il n'y a rien à armer ni à désarmer, il y a
/// une cible à nommer. Absente, la désignation INTÉGRÉE
/// (`wasapi_peripherique::DESIGNATION_CABLE`) s'applique — jamais le défaut de
/// Windows.
pub const VARIABLE_CABLE: &str = "MICRO_PERIPHERIQUE";

/// Élit le périphérique de rendu à capter et le rend, **après avoir tracé
/// lequel a été retenu**.
///
/// Le repli est explicite et bruyant : si la demande n'aboutit pas, on retombe
/// sur le défaut de Windows — mais en `warn!`, en nommant ce qui a été
/// demandé, ce qui existait, et ce qui est finalement retenu. **Jamais un
/// retour silencieux au défaut** : c'est exactement la panne muette que cette
/// correction existe pour supprimer, et la remplacer par une autre panne
/// muette n'aurait aucun sens.
///
/// Le choix de *ne pas échouer* quand la demande n'aboutit pas est délibéré et
/// tient au comportement établi de l'appelant : `demarrage/audio.rs::brancher`
/// journalise et laisse la session continuer **muette** si la source audio
/// refuse de s'ouvrir. Échouer ici échangerait « du son, peut-être le mauvais,
/// et un `warn!` qui le dit » contre « aucun son du tout » — un moins bon
/// marché pour l'exploitant, à information égale.
pub fn resoudre(enumerateur: &IMMDeviceEnumerator) -> Result<IMMDevice> {
    let demande = std::env::var(VARIABLE).ok();
    let disponibles = enumerer(enumerateur)?;
    let choix = choisir(&disponibles, demande.as_deref());

    let (peripherique, critere) = match &choix {
        Choix::Elu {
            peripherique,
            critere,
        } => (
            ouvrir_par_identifiant(enumerateur, &peripherique.identifiant)?,
            critere.libelle(),
        ),
        Choix::Defaut => (defaut(enumerateur)?, "défaut de Windows"),
        Choix::Introuvable { demande } => {
            tracing::warn!(
                variable = VARIABLE,
                demande = %demande,
                disponibles = %inventaire(&disponibles),
                "aucun périphérique audio de rendu ne correspond : REPLI sur le défaut de Windows, \
                 qui n'est pas forcément celui qu'on veut capter"
            );
            (defaut(enumerateur)?, "défaut de Windows (repli)")
        }
        Choix::Ambigu { demande, candidats } => {
            tracing::warn!(
                variable = VARIABLE,
                demande = %demande,
                candidats = %candidats.join(" | "),
                disponibles = %inventaire(&disponibles),
                "plusieurs périphériques audio de rendu correspondent, la désignation est trop \
                 large pour trancher : REPLI sur le défaut de Windows. Précisez la demande, ou \
                 donnez l'identifiant d'endpoint"
            );
            (defaut(enumerateur)?, "défaut de Windows (repli)")
        }
    };

    // La trace qui compte : elle nomme le périphérique RÉELLEMENT retenu, quel
    // que soit le chemin qui y a mené. Une recette peut ainsi vérifier ce qui
    // est capté sans le deviner — et sans elle, la correction ne serait pas
    // falsifiable.
    let retenu = decrire(&peripherique);
    tracing::info!(
        variable = VARIABLE,
        demande = demande.as_deref().unwrap_or("(aucune)"),
        retenu = %retenu.0,
        identifiant = %retenu.1,
        critere,
        repli = choix.est_repli(),
        "périphérique audio de rendu retenu"
    );

    Ok(peripherique)
}

/// Élit le **CÂBLE** sur lequel écrire le micro, et rend son `IMMDevice` avec
/// son identifiant d'endpoint. **AUCUN REPLI** (Décision 4 du plan E2).
///
/// `MICRO_PERIPHERIQUE` désigne la cible ; absente ou vide, c'est
/// `DESIGNATION_CABLE` qui s'applique. La demande n'est donc **jamais** `None`,
/// et `Choix::Defaut` est par construction inatteignable par ce chemin — c'est
/// ce qui garantit qu'on n'appellera jamais [`defaut`] ici. Un test pur garde
/// ce prédicat (`wasapi_peripherique::demande_cable`).
///
/// `Introuvable` **et** `Ambigu` valent échec, tous deux avec l'inventaire dans
/// le message : l'appelant journalise, ne pose aucun puits, et la session
/// continue sans micro. Sur une machine portant deux câbles VB-Audio, la
/// désignation intégrée devient ambiguë et le micro est indisponible — c'est le
/// comportement voulu, pas un défaut.
///
/// L'**identifiant** rendu n'est pas un ornement : c'est lui que la garde de
/// boucle locale (`micro/boucle_locale::evaluer`) compare à ce que le loopback
/// capterait, et une comparaison sur le nom convivial ne vaudrait rien — cette
/// VM porte deux rendus dont le nom commence par « Haut-parleurs ( ».
pub fn resoudre_cable(enumerateur: &IMMDeviceEnumerator) -> Result<(IMMDevice, String)> {
    let brut = std::env::var(VARIABLE_CABLE).ok();
    let demande = demande_cable(brut.as_deref());
    let disponibles = enumerer(enumerateur)?;

    let (peripherique, critere) = match choisir(&disponibles, Some(demande)) {
        Choix::Elu {
            peripherique,
            critere,
        } => (
            ouvrir_par_identifiant(enumerateur, &peripherique.identifiant)?,
            critere.libelle(),
        ),
        Choix::Introuvable { demande } => {
            bail!(
                "aucun peripherique de rendu ne correspond a « {demande} » ({VARIABLE_CABLE}) :                  pas de micro. Disponibles : {}. Le cable virtuel est-il installe ?",
                inventaire(&disponibles)
            );
        }
        Choix::Ambigu { demande, candidats } => {
            bail!(
                "« {demande} » ({VARIABLE_CABLE}) designe plusieurs peripheriques de rendu et la                  regle refuse de trancher : pas de micro, plutot qu'un micro dans le mauvais                  tuyau. Candidats : {}. Disponibles : {}. Precisez la demande, ou donnez                  l'identifiant d'endpoint",
                candidats.join(" | "),
                inventaire(&disponibles)
            );
        }
        // ⚠️ INATTEIGNABLE : `demande_cable` ne rend jamais de demande vide, et
        // `choisir` ne rend `Defaut` que pour une demande absente ou vide. Le
        // bras existe pour que le compilateur garde cette propriété si l'un des
        // deux changeait — et il ECHOUE plutôt que de retomber sur le défaut de
        // Windows, qui est exactement la fuite que ce chemin existe pour
        // empêcher.
        Choix::Defaut => bail!(
            "incoherence interne : la demande de cable ne peut pas etre vide              (voir wasapi_peripherique::demande_cable)"
        ),
    };

    // La trace qui rend la Décision 4 falsifiable : sans elle, on ne peut pas
    // savoir sur QUOI le micro a été écrit, ni si `MICRO_PERIPHERIQUE` a
    // seulement atteint le processus. Même patron que celle de `resoudre`.
    let (nom, identifiant) = decrire(&peripherique);
    tracing::info!(
        variable = VARIABLE_CABLE,
        demande = %demande,
        integree = brut.is_none(),
        retenu = %nom,
        identifiant = %identifiant,
        critere,
        "cable de rendu retenu pour l'ecriture du micro"
    );

    Ok((peripherique, identifiant))
}

/// L'identifiant d'endpoint que le loopback de session capterait, **sans rien
/// journaliser ni ouvrir de flux**.
///
/// La garde de boucle locale a besoin de le connaître **avant** que
/// `LoopbackCapture::open` ne soit appelé — le fil de rendu du micro démarre
/// avant, ou en même temps, et il ne doit pas ouvrir le câble si c'est lui que
/// l'agent capte.
///
/// ⚠️ **Elle ne journalise RIEN, et c'est délibéré.** [`resoudre`] émet déjà sa
/// ligne « périphérique audio de rendu retenu » ; une seconde, identique et
/// sans cause visible, ferait croire à deux ouvertures — et `agent.log` mêle
/// le superviseur et tous ses enfants depuis D4.
///
/// La décision elle-même est **pure** et vit dans
/// `micro::boucle_locale::identifiant_capte`, où les quatre branches sont
/// éprouvées sur l'hôte contre celles de [`resoudre`]. Ici on ne fait que
/// l'alimenter, et résoudre son `None` — « le défaut de Windows » — par le seul
/// appel COM qui puisse le nommer.
pub fn identifiant_capte(enumerateur: &IMMDeviceEnumerator) -> Result<String> {
    let demande = std::env::var(VARIABLE).ok();
    let disponibles = enumerer(enumerateur)?;
    match boucle_locale::identifiant_capte(&disponibles, demande.as_deref()) {
        Some(identifiant) => Ok(identifiant),
        None => {
            let peripherique = defaut(enumerateur)?;
            Ok(decrire(&peripherique).1)
        }
    }
}

/// Le rendu par défaut de la session — l'ancien comportement, désormais
/// atteint par un seul endroit.
fn defaut(enumerateur: &IMMDeviceEnumerator) -> Result<IMMDevice> {
    // SAFETY : l'énumérateur vient d'un `CoCreateInstance` réussi sur un fil
    // membre de la MTA (garanti par l'appelant, `LoopbackCapture::open`).
    unsafe { enumerateur.GetDefaultAudioEndpoint(eRender, eConsole) }
        .context("aucun périphérique de rendu audio par défaut")
}

/// Rouvre un périphérique par son identifiant d'endpoint.
///
/// On ne CONSERVE pas l'`IMMDevice` récolté à l'énumération : le passer à
/// travers la règle pure obligerait celle-ci à porter un type `windows`, ce
/// qui la rendrait ininéprouvable sur l'hôte — toute la raison d'être de la
/// séparation. Rouvrir par identifiant coûte un appel COM une fois par
/// session, et garde la frontière nette.
fn ouvrir_par_identifiant(
    enumerateur: &IMMDeviceEnumerator,
    identifiant: &str,
) -> Result<IMMDevice> {
    let large: Vec<u16> = identifiant
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY : `large` reste vivant pendant tout l'appel, et se termine par
    // le zéro qu'exige `PCWSTR`.
    unsafe { enumerateur.GetDevice(PCWSTR(large.as_ptr())) }
        .with_context(|| format!("ouverture du périphérique audio {identifiant}"))
}

/// Photographie les périphériques de rendu ACTIFS.
///
/// `DEVICE_STATE_ACTIVE` seul, à dessein : un périphérique débranché ou
/// désactivé ne peut rien rendre, et le proposer à la sélection ferait élire
/// une cible qui ne produira jamais un octet — la panne même qu'on corrige,
/// sous une autre forme.
fn enumerer(enumerateur: &IMMDeviceEnumerator) -> Result<Vec<Peripherique>> {
    // SAFETY : voir `defaut`. Chaque `Item` rend une référence comptée que le
    // `Drop` de `IMMDevice` relâche.
    unsafe {
        let collection = enumerateur
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .context("énumération des périphériques audio de rendu")?;
        let nombre = collection
            .GetCount()
            .context("comptage des périphériques audio de rendu")?;
        let mut peripheriques = Vec::with_capacity(nombre as usize);
        for index in 0..nombre {
            // Un périphérique qu'on n'arrive pas à décrire n'interrompt pas
            // l'énumération : il serait sinon impossible d'en élire un autre
            // à cause d'un voisin en mauvais état. `decrire` rend alors des
            // chaînes vides, qui ne correspondront à aucune demande non vide.
            let peripherique = collection
                .Item(index)
                .with_context(|| format!("lecture du périphérique audio n°{index}"))?;
            let (nom, identifiant) = decrire(&peripherique);
            peripheriques.push(Peripherique { nom, identifiant });
        }
        Ok(peripheriques)
    }
}

/// Nom convivial et identifiant d'endpoint d'un périphérique.
///
/// **Ne rend jamais d'erreur** : un périphérique indescriptible ne doit pas
/// faire échouer l'ouverture du son, il doit seulement être inéligible. Une
/// chaîne vide ne correspond à aucune demande non vide (la règle pure écarte
/// les demandes vides avant toute comparaison), donc l'inéligibilité est
/// acquise sans code de plus.
fn decrire(peripherique: &IMMDevice) -> (String, String) {
    // SAFETY : `GetId` alloue par `CoTaskMemAlloc` et nous en transfère la
    // propriété — d'où le `CoTaskMemFree` en regard, après copie. Le
    // `PROPVARIANT` rendu par `GetValue` implémente `Drop` et se libère seul.
    unsafe {
        let identifiant = match peripherique.GetId() {
            Ok(brut) => {
                let texte = brut.to_string().unwrap_or_default();
                CoTaskMemFree(Some(brut.0 as *const core::ffi::c_void));
                texte
            }
            Err(_) => String::new(),
        };
        let nom = peripherique
            .OpenPropertyStore(STGM_READ)
            .and_then(|magasin| magasin.GetValue(&PKEY_Device_FriendlyName))
            .map(|valeur| valeur.to_string())
            .unwrap_or_default();
        (nom, identifiant)
    }
}
