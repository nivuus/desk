//! Window detection through `SetWinEventHook`, and its message pump.
//!
//! **The `WINEVENT_OUTOFCONTEXT` hook only calls its callback from a thread
//! that pumps messages.** Without `GetMessageW` in a loop, the hook is set
//! without error and never fires — it is this API's mute failure
//! mode, and the reason for the dedicated thread.
//!
//! The callback does only one thing: translate and send. No decision is
//! made here (see `fenetres`), no state is held here (see `table`): a
//! global hook callback runs in a constrained context, and anything
//! long done there delays the whole desktop.

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
    GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, PostThreadMessageW,
    TranslateMessage, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_SHOW, GWL_EXSTYLE,
    GW_OWNER, MSG, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT, WM_QUIT, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use super::fenetres::{ecartee_pour_non_appartenance, merite_une_fenetre, DescriptionFenetre};
use super::table::IdFenetre;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvenementFenetre {
    Apparue { fenetre: IdFenetre, titre: String },
    Disparue { fenetre: IdFenetre },
}

/// Global sender of the callback.
///
/// An `extern "system"` callback carries no user data: Windows
/// passes nothing that belongs to us. That is the reason for this global, and not a
/// choice of convenience. It is written once by `poser` and read by the callback.
static EMETTEUR: Mutex<Option<Sender<EvenementFenetre>>> = Mutex::new(None);

/// Guard: removes the hook and stops the pump on destruction.
pub struct Hook {
    hook: HWINEVENTHOOK,
    fil: Option<std::thread::JoinHandle<()>>,
    fil_id: u32,
}

impl Drop for Hook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWinEvent(self.hook);
            // Wake the pump so it leaves `GetMessageW`, otherwise
            // the thread never ends and the join below
            // would block indefinitely.
            let _ =
                PostThreadMessageW(self.fil_id, WM_QUIT, Default::default(), Default::default());
        }
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
        *EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Records a window's state, to submit it to the `fenetres` criterion.
///
/// `None` if the window has already disappeared between the event and this call — a
/// common and normal case, not an error.
/// Does the window STILL deserve a tab, when its probation expires?
///
/// 🔴 **IT IS THE SECOND HALF OF THE DEBOUNCE, AND WITHOUT IT PROBATION WOULD
/// ONLY BE A DELAY.** `superviseur::sursis` establishes that a window has
/// LASTED; this one establishes that it is still presentable. A window can
/// perfectly survive 500 ms and meanwhile have lost its title, been
/// cloaked by DWM, or received an owner — it is the case of splash
/// screens that turn into a child dialog.
///
/// `IsWindow` first: `decrire` on a dead `HWND` would return a description
/// of default values, which `merite_une_fenetre` could judge acceptable.
pub fn merite_encore(fenetre: IdFenetre) -> bool {
    let hwnd = HWND(fenetre.0 as *mut std::ffi::c_void);
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return false;
    }
    decrire(hwnd).is_some_and(|d| super::fenetres::merite_une_fenetre(&d))
}

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
    // `OBJID_WINDOW` alone: without this filter, each child control, each
    // scroll bar and each cursor comes up here.
    if id_objet != OBJID_WINDOW.0 || hwnd.is_invalid() {
        return;
    }
    let message = match evenement {
        EVENT_OBJECT_SHOW => {
            let description = match decrire(hwnd) {
                Some(d) if merite_une_fenetre(&d) => d,
                _ => return,
            };
            // The ownership gate is CONSULTED HERE and in the initial
            // enumeration — both entry paths, never just one. This repository has
            // already paid for a guard that only bit on one of the two.
            if refusee_pour_appartenance(hwnd, &description.titre) {
                return;
            }
            EvenementFenetre::Apparue {
                fenetre: IdFenetre(hwnd.0 as u64),
                titre: description.titre,
            }
        }
        // `HIDE` as much as `DESTROY`: a hidden window cannot be told apart
        // from a closed window from the user's point of view, and an
        // application hiding its main window instead of destroying it
        // (notification area) would otherwise leave a live stream on an
        // invisible window.
        EVENT_OBJECT_HIDE | EVENT_OBJECT_DESTROY => EvenementFenetre::Disparue {
            fenetre: IdFenetre(hwnd.0 as u64),
        },
        _ => return,
    };
    if let Some(tx) = EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        // A send failure means the supervisor is stopping: nothing to
        // log from a global hook callback.
        let _ = tx.send(message);
    }
}

/// Enumerates the windows already open at supervisor startup.
///
/// The hook only reports changes: without this enumeration, the
/// windows predating the supervisor would never exist for it.
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

/// The OWNERSHIP gate, and **its trace**.
///
/// 🔴 **THIS TRACE IS A REQUIREMENT, NOT A COMFORT.** Without it, an
/// application `desk` could not adopt would be **mute**: it would never
/// appear, and nothing anywhere would say why. This repository pays for a
/// mute failure more dearly than a loud defect, and the case is REAL — a
/// **Windows Store** application appears under a system intermediary
/// (`ApplicationFrameHost`) that does not descend from us. None in the catalogue
/// is in that case today (measured, 41 Win32 shortcuts); **the risk
/// is pushed back, not removed.**
///
/// ⚠️ **`info!`, never `error!`**: setting aside a window that is not ours
/// is the rule's NORMAL operation, not a failure. This batch has just
/// fixed a false alarm for this exact reason
/// (`moniteurs_virtuels::verdict_purge`), and an `error!` shouting at each
/// Steam window would be the same mistake.
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

/// The process name, so that the trace above is readable without a
/// second investigation. `None` if it cannot be obtained — the trace says so
/// rather than dropping the whole line.
fn nom_du_processus(pid: u32) -> Option<String> {
    // ⚠️ `QueryFullProcessImageNameW` and not `GetModuleBaseNameW`: the latter
    // lives in `Win32_System_ProcessStatus`, a feature this crate does not
    // enable. The former is in `Win32_System_Threading`, already enabled — and
    // adding a feature for a log name would be paying dearly for a comfort.
    use windows::core::PWSTR;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let processus = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
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
    Some(
        chemin
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&chemin)
            .to_string(),
    )
}

/// Sets the global hook and starts its message pump on a dedicated thread.
pub fn poser(tx: Sender<EvenementFenetre>) -> Result<Hook> {
    *EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

    let (prete, attendre) = std::sync::mpsc::channel::<Result<(isize, u32), String>>();
    let fil = std::thread::spawn(move || {
        let hook = unsafe {
            SetWinEventHook(
                // Low and high bounds: DESTROY=0x8001, SHOW=0x8002,
                // HIDE=0x8003. `(DESTROY, SHOW)` — the original brief's order
                // — would exclude HIDE, located just above the high bound.
                // `(DESTROY, HIDE)` covers all three, SHOW falling between
                // them: it is the fix made here, not an arbitrary choice
                // of wider bounds.
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

        // The pump. `GetMessageW` returns 0 on `WM_QUIT`: that is how
        // `Hook::drop` makes this thread exit.
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
        Err(_) => Err(anyhow!(
            "le fil du hook s'est terminé avant de rendre son état"
        )),
    }
}
