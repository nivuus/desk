//! Injection des entrées du navigateur dans la session Windows.
//!
//! Le mapping de coordonnées (fenêtre → bureau virtuel) est indépendant de
//! toute API Windows et vit dans `crate::geometry::to_virtual_desktop`,
//! testé sur Linux (tâche 12 : le brief l'attendait ici, mais `Rect` a été
//! introduit à la tâche 9 dans `geometry.rs`, aux côtés de `crop_region` —
//! le mapping l'y rejoint plutôt que de dupliquer `Rect`). Ce module ne
//! porte donc que l'appel système `SendInput`, propre à Windows.

#[cfg(windows)]
mod win {
    use crate::geometry::{to_virtual_desktop_visible, Rect};
    use anyhow::{anyhow, Context, Result};
    use proto::input::{InputMessage, MouseButton};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    // écart d'API windows-rs 0.62, déjà rencontré dans `window.rs` :
    // `ClientToScreen` vit dans `Win32::Graphics::Gdi` (module gdi32), pas
    // dans `WindowsAndMessaging` (user32) où on l'attendrait par analogie
    // avec `GetClientRect`.
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL,
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
        MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK,
        MOUSEEVENTF_WHEEL, MOUSEINPUT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetForegroundWindow, GetSystemMetrics, SetForegroundWindow,
        SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };

    /// Injecte les messages d'entrée reçus du navigateur dans la session
    /// Windows courante, via `SendInput`.
    pub struct InputInjector {
        hwnd: HWND,
        /// Renseigné par le fil de sondage du curseur (`cursor.rs`).
        /// L'agent est SEUL décideur du mode : le client n'a rien à savoir,
        /// et il n'existe qu'une source de vérité — la seule construction
        /// correcte sur un canal non ordonné.
        mode_relatif: Arc<AtomicBool>,
        /// Dernier résultat connu de `SetForegroundWindow`, pour ne journaliser
        /// qu'au basculement et non à chaque frappe.
        premier_plan_obtenu: bool,
    }

    impl InputInjector {
        pub fn new(hwnd: HWND, mode_relatif: Arc<AtomicBool>) -> Self {
            Self { hwnd, mode_relatif, premier_plan_obtenu: false }
        }

        pub fn inject(&mut self, message: InputMessage) -> Result<()> {
            match message {
                InputMessage::MouseMove { x, y } => self.move_mouse(x, y),
                InputMessage::MouseButton { button, pressed, x, y } => {
                    // En absolu : toujours positionner avant de cliquer, le
                    // canal n'étant pas ordonné, le déplacement correspondant
                    // a pu se perdre.
                    //
                    // En RELATIF : surtout pas. Les coordonnées portées par
                    // le message n'ont plus de sens sous Pointer Lock, et un
                    // repositionnement absolu téléporterait le curseur à
                    // chaque tir. Relu à CHAQUE appel (pas capturé une fois à
                    // la construction) : le mode peut basculer en cours de
                    // session, au gré du fil de sondage du curseur.
                    if !self.mode_relatif.load(Ordering::Relaxed) {
                        self.move_mouse(x, y)?;
                    }
                    let flags = match (button, pressed) {
                        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
                        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                        (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                        (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                    };
                    send_mouse(MOUSEINPUT { dwFlags: flags, ..Default::default() })
                }
                InputMessage::MouseMoveRelative { dx, dy } => send_mouse(MOUSEINPUT {
                    dx: dx as i32,
                    dy: dy as i32,
                    dwFlags: MOUSEEVENTF_MOVE,
                    ..Default::default()
                }),
                InputMessage::Gamepad(_) => {
                    // Ignoré ici, volontairement : `demarrage.rs` intercepte cette
                    // variante AVANT d'appeler l'injecteur (tâche 10, manette
                    // virtuelle ViGEmBus) — l'injecteur clavier/souris n'a
                    // rien à en faire. Un bras muet, sans ce commentaire,
                    // serait un piège pour la suite : on croirait le message
                    // traité alors qu'il ne l'a jamais été par ce module.
                    Ok(())
                }
                InputMessage::Wheel { delta_x, delta_y } => {
                    if delta_y != 0 {
                        send_mouse(MOUSEINPUT {
                            mouseData: delta_y as i32 as u32,
                            dwFlags: MOUSEEVENTF_WHEEL,
                            ..Default::default()
                        })?;
                    }
                    if delta_x != 0 {
                        send_mouse(MOUSEINPUT {
                            mouseData: delta_x as i32 as u32,
                            dwFlags: MOUSEEVENTF_HWHEEL,
                            ..Default::default()
                        })?;
                    }
                    Ok(())
                }
                InputMessage::Key { scancode, pressed, extended } => {
                    self.au_premier_plan();
                    let mut flags = KEYEVENTF_SCANCODE;
                    if !pressed {
                        flags |= KEYEVENTF_KEYUP;
                    }
                    if extended {
                        flags |= KEYEVENTF_EXTENDEDKEY;
                    }
                    send_keyboard(KEYBDINPUT {
                        wVk: Default::default(),
                        wScan: scancode,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    })
                }
            }
        }

        /// Porte la fenêtre de cette session au premier plan avant d'injecter
        /// du clavier.
        ///
        /// **Nécessaire et probablement pas suffisant.** `SendInput` est global
        /// à la session Windows : il n'adresse personne, il alimente la file
        /// d'entrée de la fenêtre active. Sans cet appel, toutes les sessions
        /// tapent dans la même fenêtre — celle qui se trouve au premier plan.
        /// Avec, deux sessions qui tapent en même temps se le disputent. La
        /// réponse structurelle est ailleurs (injection ciblée par messages, ou
        /// un pilote) et reste hors périmètre du sous-bloc D2.
        ///
        /// **Le retour est vérifié.** `SetForegroundWindow` échoue
        /// silencieusement quand le processus appelant n'a pas le droit de
        /// voler le focus : sans cette trace, on ne saurait pas distinguer « le
        /// premier plan n'a pas suffi » de « le premier plan n'a jamais été
        /// donné ». Journalisé une fois par basculement et non par frappe — un
        /// journal par touche noierait le canal.
        fn au_premier_plan(&mut self) {
            if unsafe { GetForegroundWindow() } == self.hwnd {
                return;
            }
            let obtenu = unsafe { SetForegroundWindow(self.hwnd) }.as_bool();
            if obtenu != self.premier_plan_obtenu {
                if obtenu {
                    tracing::info!(hwnd = ?self.hwnd, "premier plan obtenu avant injection clavier");
                } else {
                    tracing::warn!(
                        hwnd = ?self.hwnd,
                        "SetForegroundWindow refusé — le clavier ira à la fenêtre active"
                    );
                }
                self.premier_plan_obtenu = obtenu;
            }
        }

        fn move_mouse(&self, x: u16, y: u16) -> Result<()> {
            let window = self.client_rect_on_screen()?;
            let desktop = virtual_desktop();
            // Sur la région RÉELLEMENT capturée, pas sur la zone client
            // entière : le navigateur normalise ses coordonnées sur l'image
            // qu'il reçoit, et cette image est l'intersection de la fenêtre
            // avec l'écran. Mapper sur la zone client complète fait dériver le
            // pointeur de tout ce qui dépasse.
            let Some((absolute_x, absolute_y)) = to_virtual_desktop_visible(x, y, window, desktop)
            else {
                // Fenêtre entièrement hors écran : aucune image n'est envoyée,
                // il n'y a donc aucun point à viser. Ignorer plutôt que
                // d'injecter au hasard.
                return Ok(());
            };
            send_mouse(MOUSEINPUT {
                dx: absolute_x,
                dy: absolute_y,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                ..Default::default()
            })
        }

        /// Zone client de la fenêtre, exprimée en coordonnées écran.
        fn client_rect_on_screen(&self) -> Result<Rect> {
            let mut rect = RECT::default();
            unsafe { GetClientRect(self.hwnd, &mut rect)? };
            let mut origin = POINT { x: rect.left, y: rect.top };
            unsafe { ClientToScreen(self.hwnd, &mut origin) }
                .ok()
                .context("ClientToScreen")?;
            Ok(Rect {
                x: origin.x,
                y: origin.y,
                width: (rect.right - rect.left).max(0) as u32,
                height: (rect.bottom - rect.top).max(0) as u32,
            })
        }
    }

    fn virtual_desktop() -> Rect {
        unsafe {
            Rect {
                x: GetSystemMetrics(SM_XVIRTUALSCREEN),
                y: GetSystemMetrics(SM_YVIRTUALSCREEN),
                width: GetSystemMetrics(SM_CXVIRTUALSCREEN).max(0) as u32,
                height: GetSystemMetrics(SM_CYVIRTUALSCREEN).max(0) as u32,
            }
        }
    }

    fn send_mouse(mouse: MOUSEINPUT) -> Result<()> {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { mi: mouse },
        };
        dispatch(&[input])
    }

    fn send_keyboard(keyboard: KEYBDINPUT) -> Result<()> {
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { ki: keyboard },
        };
        dispatch(&[input])
    }

    fn dispatch(inputs: &[INPUT]) -> Result<()> {
        let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err(anyhow!("SendInput a refusé l'entrée (session verrouillée ?)"));
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use win::InputInjector;
