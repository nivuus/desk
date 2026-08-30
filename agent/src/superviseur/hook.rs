//! Détection des fenêtres par `SetWinEventHook`, et sa pompe de messages.
//!
//! **Le hook `WINEVENT_OUTOFCONTEXT` n'appelle son rappel que depuis un fil
//! qui pompe des messages.** Sans `GetMessageW` en boucle, le hook se pose
//! sans erreur et ne se déclenche jamais — c'est le mode de défaillance
//! muet de cette API, et la raison du fil dédié.
//!
//! Le rappel ne fait qu'une chose : traduire et envoyer. Aucune décision n'est
//! prise ici (voir `fenetres`), aucun état n'y est tenu (voir `table`) : un
//! rappel de hook global s'exécute dans un contexte contraint, et tout ce
//! qu'on peut y faire de long retarde tout le bureau.

#![cfg(windows)]

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EnumWindows, GetMessageW, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, PostThreadMessageW,
    TranslateMessage, EVENT_OBJECT_DESTROY,
    EVENT_OBJECT_HIDE, EVENT_OBJECT_SHOW, GWL_EXSTYLE, GW_OWNER, MSG, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT,
    WM_QUIT, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use super::fenetres::{ecartee_pour_non_appartenance, merite_une_fenetre, DescriptionFenetre};
use super::table::IdFenetre;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvenementFenetre {
    Apparue { fenetre: IdFenetre, titre: String },
    Disparue { fenetre: IdFenetre },
}

/// Émetteur global du rappel.
///
/// Un rappel `extern "system"` ne porte aucune donnée utilisateur : Windows ne
/// passe rien qui nous appartienne. C'est la raison de ce global, et non un
/// choix de commodité. Il est écrit une fois par `poser` et lu par le rappel.
static EMETTEUR: Mutex<Option<Sender<EvenementFenetre>>> = Mutex::new(None);

/// Garde : retire le hook et arrête la pompe à la destruction.
pub struct Hook {
    hook: HWINEVENTHOOK,
    fil: Option<std::thread::JoinHandle<()>>,
    fil_id: u32,
}

impl Drop for Hook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWinEvent(self.hook);
            // Réveiller la pompe pour qu'elle sorte de `GetMessageW`, sans
            // quoi le fil ne se termine jamais et la jointure ci-dessous
            // bloquerait indéfiniment.
            let _ = PostThreadMessageW(self.fil_id, WM_QUIT, Default::default(), Default::default());
        }
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
        *EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Relève l'état d'une fenêtre, pour le soumettre au critère de `fenetres`.
///
/// `None` si la fenêtre a déjà disparu entre l'événement et cet appel — cas
/// courant et normal, pas une erreur.
pub fn decrire(hwnd: HWND) -> Option<DescriptionFenetre> {
    unsafe {
        let longueur = GetWindowTextLengthW(hwnd);
        let titre = if longueur > 0 {
            let mut tampon = vec![0u16; longueur as usize + 1];
            let ecrits = GetWindowTextW(hwnd, &mut tampon);
            if ecrits > 0 {
                String::from_utf16_lossy(&tampon[..ecrits as usize])
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let styles = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut masquee: u32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut masquee as *mut _ as *mut _,
            std::mem::size_of::<u32>() as u32,
        );

        Some(DescriptionFenetre {
            visible: IsWindowVisible(hwnd).as_bool(),
            a_un_proprietaire: GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid()),
            tool_window: styles & WS_EX_TOOLWINDOW.0 != 0,
            app_window: styles & WS_EX_APPWINDOW.0 != 0,
            masquee_dwm: masquee != 0,
            titre,
        })
    }
}

unsafe extern "system" fn rappel(
    _hook: HWINEVENTHOOK,
    evenement: u32,
    hwnd: HWND,
    id_objet: i32,
    _id_enfant: i32,
    _fil: u32,
    _instant: u32,
) {
    // `OBJID_WINDOW` seul : sans ce filtre, chaque contrôle enfant, chaque
    // barre de défilement et chaque curseur remontent ici.
    if id_objet != OBJID_WINDOW.0 || hwnd.is_invalid() {
        return;
    }
    let message = match evenement {
        EVENT_OBJECT_SHOW => {
            let description = match decrire(hwnd) {
                Some(d) if merite_une_fenetre(&d) => d,
                _ => return,
            };
            // La porte d'appartenance est CONSULTÉE ICI et dans l'énumération
            // initiale — les deux chemins d'entrée, jamais un seul. Ce dépôt a
            // déjà payé un garde qui ne mordait que sur l'un des deux.
            if refusee_pour_appartenance(hwnd, &description.titre) {
                return;
            }
            EvenementFenetre::Apparue {
                fenetre: IdFenetre(hwnd.0 as u64),
                titre: description.titre,
            }
        }
        // `HIDE` autant que `DESTROY` : une fenêtre masquée ne se distingue
        // pas d'une fenêtre fermée du point de vue de l'utilisateur, et une
        // application qui masque sa fenêtre principale au lieu de la détruire
        // (barre de notification) laisserait sinon un flux vivant sur une
        // fenêtre invisible.
        EVENT_OBJECT_HIDE | EVENT_OBJECT_DESTROY => {
            EvenementFenetre::Disparue { fenetre: IdFenetre(hwnd.0 as u64) }
        }
        _ => return,
    };
    if let Some(tx) = EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        // L'échec d'envoi signifie que le superviseur s'arrête : rien à
        // journaliser depuis un rappel de hook global.
        let _ = tx.send(message);
    }
}

/// Énumère les fenêtres déjà ouvertes au démarrage du superviseur.
///
/// Le hook ne rapporte que les changements : sans cette énumération, les
/// fenêtres antérieures au superviseur n'existeraient jamais pour lui.
pub fn enumerer_existantes() -> Vec<(IdFenetre, String)> {
    let mut trouvees: Vec<(IdFenetre, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(rappel_enumeration),
            LPARAM(&mut trouvees as *mut Vec<(IdFenetre, String)> as isize),
        );
    }
    trouvees
}

unsafe extern "system" fn rappel_enumeration(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let trouvees = &mut *(lparam.0 as *mut Vec<(IdFenetre, String)>);
    if let Some(d) = decrire(hwnd) {
        if merite_une_fenetre(&d) && !refusee_pour_appartenance(hwnd, &d.titre) {
            trouvees.push((IdFenetre(hwnd.0 as u64), d.titre));
        }
    }
    TRUE
}

/// La porte d'APPARTENANCE, et **sa trace**.
///
/// 🔴 **CETTE TRACE EST UNE EXIGENCE, PAS UN CONFORT.** Sans elle, une
/// application que `desk` n'a pas pu adopter serait **muette** : elle ne
/// paraîtrait jamais, et rien nulle part ne dirait pourquoi. Ce dépôt paie une
/// panne muette plus cher qu'un défaut bruyant, et le cas est RÉEL — une
/// application du **Windows Store** paraît sous un intermédiaire du système
/// (`ApplicationFrameHost`) qui ne descend pas de nous. Aucune du catalogue
/// n'est dans ce cas aujourd'hui (mesuré, 41 raccourcis Win32) ; **le risque
/// est repoussé, pas supprimé.**
///
/// ⚠️ **`info!`, jamais `error!`** : écarter une fenêtre qui n'est pas à nous
/// est le fonctionnement NORMAL de la règle, pas une panne. Ce lot vient de
/// corriger une fausse alerte pour cette raison exacte
/// (`moniteurs_virtuels::verdict_purge`), et un `error!` qui crie à chaque
/// fenêtre de Steam serait la même faute.
fn refusee_pour_appartenance(hwnd: HWND, titre: &str) -> bool {
    let armee = crate::appartenance::armee();
    let mut pid = 0u32;
    let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let appartient = crate::appartenance::est_des_notres(pid);
    if !ecartee_pour_non_appartenance(appartient, armee) {
        return false;
    }
    tracing::info!(
        titre,
        pid,
        processus = %nom_du_processus(pid).unwrap_or_else(|| "?".into()),
        "fenêtre ÉCARTÉE : desk ne l'a pas lancée (règle d'appartenance). \
         Désarmer par APPARTENANCE=0 pour retrouver le comportement d'avant"
    );
    true
}

/// Le nom du processus, pour que la trace ci-dessus soit lisible sans une
/// seconde enquête. `None` si on ne peut pas l'obtenir — la trace le dit
/// plutôt que de taire la ligne entière.
fn nom_du_processus(pid: u32) -> Option<String> {
    // ⚠️ `QueryFullProcessImageNameW` et non `GetModuleBaseNameW` : la seconde
    // vit dans `Win32_System_ProcessStatus`, une feature que ce crate n'active
    // pas. La première est dans `Win32_System_Threading`, déjà active — et
    // ajouter une feature pour un nom de journal serait payer cher un confort.
    use windows::core::PWSTR;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let processus =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut tampon = [0u16; 260];
    let mut taille = tampon.len() as u32;
    let issue = unsafe {
        QueryFullProcessImageNameW(
            processus,
            PROCESS_NAME_WIN32,
            PWSTR(tampon.as_mut_ptr()),
            &mut taille,
        )
    };
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(processus) };
    issue.ok()?;
    let chemin = String::from_utf16_lossy(&tampon[..taille as usize]);
    Some(chemin.rsplit(['\\', '/']).next().unwrap_or(&chemin).to_string())
}

/// Pose le hook global et lance sa pompe de messages sur un fil dédié.
pub fn poser(tx: Sender<EvenementFenetre>) -> Result<Hook> {
    *EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

    let (prete, attendre) = std::sync::mpsc::channel::<Result<(isize, u32), String>>();
    let fil = std::thread::spawn(move || {
        let hook = unsafe {
            SetWinEventHook(
                // Bornes basse et haute : DESTROY=0x8001, SHOW=0x8002,
                // HIDE=0x8003. `(DESTROY, SHOW)` — l'ordre du brief d'origine
                // — exclurait HIDE, situé juste au-dessus de la borne haute.
                // `(DESTROY, HIDE)` couvre les trois, SHOW tombant entre les
                // deux : c'est la correction faite ici, pas un choix
                // arbitraire de bornes plus larges.
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_HIDE,
                None,
                Some(rappel),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_invalid() {
            let _ = prete.send(Err("SetWinEventHook a échoué".into()));
            return;
        }
        let id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
        let _ = prete.send(Ok((hook.0 as isize, id)));

        // La pompe. `GetMessageW` rend 0 sur `WM_QUIT` : c'est ainsi que
        // `Hook::drop` fait sortir ce fil.
        let mut message = MSG::default();
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    });

    match attendre.recv() {
        Ok(Ok((hook, fil_id))) => Ok(Hook {
            hook: HWINEVENTHOOK(hook as *mut core::ffi::c_void),
            fil: Some(fil),
            fil_id,
        }),
        Ok(Err(e)) => Err(anyhow!(e)),
        Err(_) => Err(anyhow!("le fil du hook s'est terminé avant de rendre son état")),
    }
}
