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
    use crate::geometry::{to_virtual_desktop, Rect};
    use anyhow::{anyhow, Context, Result};
    use proto::input::{InputMessage, MouseButton};
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
        GetClientRect, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };

    /// Injecte les messages d'entrée reçus du navigateur dans la session
    /// Windows courante, via `SendInput`.
    pub struct InputInjector {
        hwnd: HWND,
    }

    impl InputInjector {
        pub fn new(hwnd: HWND) -> Self {
            Self { hwnd }
        }

        pub fn inject(&mut self, message: InputMessage) -> Result<()> {
            match message {
                InputMessage::MouseMove { x, y } => self.move_mouse(x, y),
                InputMessage::MouseButton { button, pressed, x, y } => {
                    // Toujours positionner avant de cliquer : le canal n'est pas
                    // ordonné, le déplacement correspondant a pu se perdre.
                    self.move_mouse(x, y)?;
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

        fn move_mouse(&self, x: u16, y: u16) -> Result<()> {
            let window = self.client_rect_on_screen()?;
            let desktop = virtual_desktop();
            let (absolute_x, absolute_y) = to_virtual_desktop(x, y, window, desktop);
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
