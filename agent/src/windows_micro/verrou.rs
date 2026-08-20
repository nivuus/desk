//! Le mutex nommé Windows derrière le trait `micro::exclusivite::Verrou`.
//!
//! 🔴 **C'est la SEULE ligne du mécanisme d'exclusivité qui ne soit pas testée
//! sur l'hôte** — la politique, elle (qui gagne, quand on réessaie, ce qui se
//! journalise), vit dans `micro/exclusivite.rs`, pure et éprouvée sous Linux.
//! Ce fichier ne fait que tenir un handle.
//!
//! ## Pourquoi un mutex NOMMÉ, et pas un drapeau
//!
//! Depuis le sous-bloc D1, N fenêtres sont N **processus** : un booléen
//! atomique ne garde rien entre eux. Et il n'y a qu'un câble — à deux
//! écrivains, Windows mixerait deux copies décalées de la même voix, un filtre
//! en peigne (spec §9).
//!
//! ## `Global\` d'abord, `Local\` en repli, et le repli est DIT
//!
//! Le câble est une ressource **de la machine**, pas d'une session : l'espace
//! de nommage voulu est `Global\`. Sa création exige cependant
//! `SeCreateGlobalPrivilege`, que l'utilisateur interactif ne détient pas
//! toujours ; sur refus (`ERROR_ACCESS_DENIED`) on retombe sur `Local\`.
//!
//! ✅ **MESURÉ (recette E2, tâche 12, 20 août 2026) : c'est `Global\` qui est
//! obtenu, aux CINQ exécutions vertes** — `espace_mutex="Global"`, sans une
//! seule occurrence du repli. L'utilisateur interactif de cette VM détient donc
//! bien `SeCreateGlobalPrivilege`. ⚠️ **Le repli ci-dessous est par conséquent
//! du code LIVRÉ ET JAMAIS COURU**, et il ne faut pas le lire comme un chemin
//! employé : sa `warn!` n'a jamais été émise, sur aucune machine.
//!
//! ⚠️ **Ce repli rétrécit la portée de la garantie à UNE session Windows, et il
//! est donc JOURNALISÉ, jamais silencieux.** Un repli muet sur une garantie
//! d'exclusivité est exactement la classe de panne que la correction « A-bis »
//! existe pour supprimer : le produit ferait quelque chose de plus faible que
//! ce qu'il annonce, sans qu'aucune ligne ne le dise.
//!
//! ## `WAIT_ABANDONED` est un SUCCÈS
//!
//! C'est la sémantique de libération que la spec §9 demande : à la mort du
//! propriétaire, Windows abandonne le mutex et le suivant l'obtient. Le
//! traiter en échec condamnerait la fenêtre B à rester sans micro pour la vie
//! de son processus après la mort de la fenêtre A — le défaut latent que la
//! Décision 2 du plan E2 corrige.

#![cfg(windows)]

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};

use crate::micro::exclusivite::Verrou;

/// Le nom de l'objet, sans son préfixe d'espace de nommage.
///
/// ⚠️ **Il désigne le CÂBLE, pas l'agent** : c'est la ressource qui est unique,
/// et deux agents de VM différentes ne se voient de toute façon pas. Un nom
/// portant le numéro de session serait un mutex par session, c'est-à-dire
/// exactement l'exclusivité que ce module n'apporte pas.
const NOM: &str = "guacamole-agent-micro-cable";

/// Un mutex nommé, acquis paresseusement et **jamais relâché explicitement**.
///
/// La libération est celle de la mort du processus : Windows abandonne alors
/// le mutex, et le prochain `WaitForSingleObject` d'un autre processus rend
/// `WAIT_ABANDONED`, donc un succès. Il n'y a rien à défaire à la main, et
/// c'est délibéré — un `ReleaseMutex` doit venir du fil PROPRIÉTAIRE, or le
/// propriétaire ici est le fil de transport, qui ne se termine qu'avec la
/// session.
pub struct MutexNomme {
    handle: HANDLE,
    /// L'espace de nommage réellement obtenu, pour le journal.
    espace: &'static str,
    /// ⚠️ **Le court-circuit qu'exige la doc du trait `Verrou`.** `tenter` est
    /// appelé à CHAQUE dépôt, soit ~50 fois par seconde, y compris quand le
    /// mutex est déjà possédé : un `WaitForSingleObject` sur un mutex déjà
    /// détenu en incrémente le compte de récursion, et il en faudrait autant de
    /// `ReleaseMutex`. Ce drapeau évite ce compte, et c'est à cette moitié-ci
    /// de le porter, pas à `Exclusivite`.
    tenu: bool,
}

// SÉCURITÉ : `MutexNomme` porte un `HANDLE`, c'est-à-dire un `*mut c_void`,
// que Rust ne marque pas `Send` par défaut. Le marquer ici est **nécessaire** —
// `Session::set_puits_micro` exige `Box<dyn PuitsMicro + Send>`, la session
// étant prise par valeur par `run()` sur un fil bloquant — et il est **correct**
// pour trois raisons, dans cet ordre :
//
// 1. **Un handle de mutex nommé est un objet du NOYAU, valide pour tout le
//    processus**, pas une référence liée à un appartement COM ni à un fil. Ce
//    n'est pas le cas de `LoopbackCapture`, dont l'`unsafe impl Send` doit
//    s'appuyer sur une vérification d'appartement à l'ouverture : ici il n'y a
//    aucun appartement en jeu.
// 2. **Rien n'est POSSÉDÉ au moment du transfert.** `creer` passe
//    `bInitialOwner = false` : l'acquisition est paresseuse, au premier dépôt,
//    donc sur le fil de transport — celui-là même qui appellera `tenter`
//    ensuite. La propriété d'un mutex Windows est per-fil ; ce qui traverse la
//    frontière n'est qu'un handle sans propriétaire.
// 3. **`Send` et non `Sync`**, et l'écart est le fond de l'argument : le puits
//    est déplacé UNE fois, de la construction vers le fil de transport, et n'est
//    ensuite touché que par lui. `Send` autorise exactement ce transfert, et
//    rien de plus — deux fils appelant `tenter` concurremment resteraient
//    interdits par le typage, et c'est bien ainsi : le drapeau `tenu`
//    ci-dessous suppose un seul appelant.
//
// **Ce qui invaliderait cette promesse**, à vérifier avant d'y toucher :
// partager le puits derrière un `Arc` (il faudrait alors `Sync`, que rien
// n'établit), ou ajouter un `ReleaseMutex` — qui doit venir du fil
// PROPRIÉTAIRE, et rendrait donc le fil d'appel significatif.
unsafe impl Send for MutexNomme {}

impl MutexNomme {
    /// Crée (ou ouvre) le mutex. **N'acquiert rien** : l'acquisition est
    /// paresseuse, au premier dépôt.
    pub fn creer() -> Result<Self> {
        match Self::creer_dans("Global\\") {
            Ok((handle, espace)) => Ok(Self { handle, espace, tenu: false }),
            Err(e) if e == ERROR_ACCESS_DENIED.into() => {
                tracing::warn!(
                    erreur = %e,
                    "micro : espace de nommage Global refuse (SeCreateGlobalPrivilege absent), \
                     REPLI sur Local. L'exclusivite du cable ne vaut plus que pour CETTE session \
                     Windows"
                );
                let (handle, espace) = Self::creer_dans("Local\\")
                    .context("creation du mutex du cable, y compris dans l'espace Local")?;
                Ok(Self { handle, espace, tenu: false })
            }
            Err(e) => Err(anyhow::Error::from(e).context("creation du mutex du cable micro")),
        }
    }

    fn creer_dans(prefixe: &str) -> windows::core::Result<(HANDLE, &'static str)> {
        let nom: Vec<u16> = format!("{prefixe}{NOM}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY : `nom` reste vivant pendant tout l'appel et se termine par le
        // zéro qu'exige `PCWSTR`. `bInitialOwner = false` : on ne veut PAS
        // posséder à la création — l'acquisition est paresseuse, et un
        // propriétaire à la création tiendrait le câble dès le démarrage même
        // si aucun paquet ne montait jamais.
        let handle = unsafe { CreateMutexW(None, false, PCWSTR(nom.as_ptr())) }?;
        let espace = if prefixe.starts_with("Global") { "Global" } else { "Local" };
        Ok((handle, espace))
    }

    /// L'espace de nommage obtenu — `Global` ou `Local`. Journalisé une fois à
    /// l'ouverture.
    pub fn espace(&self) -> &'static str {
        self.espace
    }
}

impl Verrou for MutexNomme {
    fn tenter(&mut self) -> bool {
        if self.tenu {
            return true;
        }
        // SAFETY : `handle` vient d'un `CreateMutexW` réussi. Délai NUL : la
        // doc du trait exige que cet appel ne bloque pas, et il est sur le
        // chemin de la boucle de transport.
        let issue = unsafe { WaitForSingleObject(self.handle, 0) };
        // `WAIT_ABANDONED` : le propriétaire précédent est mort sans relâcher.
        // Windows nous donne quand même la propriété — c'est le cas NOMINAL de
        // la reprise après la mort d'une fenêtre voisine, pas une anomalie.
        //
        // ✅ OBSERVÉ, et ce n'est plus un raisonnement (recette E2, tâche 12) :
        // un processus tiers acquiert le mutex, est TUÉ par `Stop-Process
        // -Force` — donc sans jamais appeler `ReleaseMutex` —, et l'agent
        // obtient le câble 46 s après son refus (`cable acquis apres un
        // refus`), le juge repassant de 0,000000 à 440,0 Hz sur CABLE Output.
        // C'est le seul chemin par lequel il pouvait l'obtenir.
        self.tenu = issue == WAIT_OBJECT_0 || issue == WAIT_ABANDONED;
        self.tenu
    }
}

// ⚠️ **Pas de `Drop` qui ferme le handle, et c'est un choix.** Ce verrou vit
// aussi longtemps que le puits, donc que la session ; le fermer à la fin du
// processus est ce que Windows fait de toute façon. Un `CloseHandle` posé ici
// s'exécuterait sur le fil qui détruit le puits, qui n'est pas forcément le fil
// PROPRIÉTAIRE du mutex — et fermer le handle d'un mutex possédé ne le relâche
// pas, il le laisse abandonné, ce qui est déjà le comportement obtenu sans rien
// écrire.
