//! Injection of the browser's inputs into the Windows session.
//!
//! The coordinate mapping (window → virtual desktop) is independent of
//! any Windows API and lives in `crate::geometry::to_virtual_desktop`,
//! tested on Linux (task 12: the brief expected it here, but `Rect` was
//! introduced at task 9 in `geometry.rs`, next to `crop_region` —
//! the mapping joins it there rather than duplicating `Rect`). This module therefore
//! only carries the `SendInput` system call, specific to Windows.

/// The four keyboard events of a paste: `Ctrl`↓, `V`↓, `V`↑, `Ctrl`↑.
///
/// **Outside the `#[cfg(windows)]`, on purpose**: it is a table, not a system
/// call, and the caller (`transport/boucle.rs`) is not gated.
///
/// 🔴 **SELF-SUFFICIENT IN MODIFIERS, and it is not over-caution.**
/// The client's input channel is `ordered: false, maxRetransmits: 0`
/// (`client/src/webrtc.ts`): the state of modifiers on the VM side at the moment of
/// injection **cannot be known** — the `Ctrl`↓ the client sent
/// may have arrived, been lost, or arrive afterwards. Setting the
/// four events ourselves makes the gesture independent of all that; one `Ctrl`↑ too
/// many is harmless, a missing `Ctrl`↓ would paste nothing.
///
/// 🔴 **ALL FOUR, AND IN THIS ORDER.** Omitting the final `Ctrl`↑ would leave
/// the application with a modifier PRESSED, and **any following keystroke
/// would become a shortcut** — the most insidious defect of this path, and
/// the one a test keeps red.
///
/// Scancodes taken from `client/src/scancodes.ts`, the table the client
/// already uses: `ControlLeft` = `0x1d`, `KeyV` = `0x2f`, `extended: false`
/// for both. Taking them from there rather than rediscovering them guarantees that
/// the VM receives exactly what it receives from a human keystroke.
///
/// ⚠️ **The repository's scope on `SetForegroundWindow` is NOT widened by
/// this table.** The `InputMessage::Key` arm of `InputInjector` already calls
/// `au_premier_plan()`, which this path inherits for free — but what
/// is MEASURED (D2) is "one keystroke per window, sequential probe, no
/// concurrent keystroke", and `SendInput` remains GLOBAL to the Windows session.
/// **Two simultaneous pastes from two windows remain outside what is
/// established**; P3 is the sub-block that will meet them.
pub const TOUCHES_COLLAGE: [proto::input::InputMessage; 4] = [
    proto::input::InputMessage::Key {
        scancode: 0x1d,
        pressed: true,
        extended: false,
    },
    proto::input::InputMessage::Key {
        scancode: 0x2f,
        pressed: true,
        extended: false,
    },
    proto::input::InputMessage::Key {
        scancode: 0x2f,
        pressed: false,
        extended: false,
    },
    proto::input::InputMessage::Key {
        scancode: 0x1d,
        pressed: false,
        extended: false,
    },
];

#[cfg(windows)]
mod win {
    use crate::geometry::{to_virtual_desktop_visible, Rect};
    use anyhow::{anyhow, Context, Result};
    use proto::input::{InputMessage, MouseButton};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    // windows-rs 0.62 API gap, already met in `window.rs`:
    // `ClientToScreen` lives in `Win32::Graphics::Gdi` (gdi32 module), not
    // in `WindowsAndMessaging` (user32) where one would expect it by analogy
    // with `GetClientRect`.
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

    /// Injects the input messages received from the browser into the current Windows
    /// session, through `SendInput`.
    pub struct InputInjector {
        hwnd: HWND,
        /// 🔴 WHAT THE COORDINATES ARE UNMAPPED ONTO — see `crate::entrees`.
        /// Derived from `config.sortie_dxgi`, **the same discriminant as the capture
        /// mode**: there are not two descriptions to keep in agreement.
        reference: crate::entrees::Reference,
        /// The rectangle of the captured output, and the instant it was surveyed.
        ///
        /// ⚠️ **Cached, and it must be**: `move_mouse` runs at the cadence
        /// of mouse movements, and enumerating DXGI at each event
        /// would cost COM calls by the dozen per second. ⚠️ **But not
        /// frozen either**: the layout of the virtual desktop changes when an
        /// output is born or dies, and a stale origin would give back exactly
        /// the defect being fixed. Hence the expiry below.
        sortie: Option<(Rect, std::time::Instant)>,
        /// Filled by the cursor polling thread (`cursor.rs`).
        /// The agent is the SOLE decider of the mode: the client has nothing to know,
        /// and there is only one source of truth — the only correct
        /// construction on an unordered channel.
        mode_relatif: Arc<AtomicBool>,
        /// Last known result of `SetForegroundWindow`, to log only
        /// on a toggle and not at each keystroke. `None` at construction
        /// — a TWO-state sentinel (`bool` initialised to `false`) would silence
        /// the very first failure, since `false == false` toggles nothing:
        /// exactly the case this trace exists to reveal. `None` does not
        /// collide with any real result of `SetForegroundWindow`, so
        /// the first call always traces, whether it succeeds or fails.
        premier_plan_obtenu: Option<bool>,
    }

    impl InputInjector {
        /// Validity duration of the output's rectangle.
        ///
        /// ⚠️ **Not calibrated**: one second is short compared with the frequency at
        /// which an output is born or dies (of the order of opening a
        /// window) and long compared with the cadence of mouse movements. It
        /// is not a measured constant, and it is declared as such.
        ///
        /// ⚠️ **WHAT IT COSTS, written here so that no one has to
        /// re-derive it**: if the captured output changes origin or size,
        /// the unmapping stays wrong **for at most one second**, then corrects
        /// itself at the next survey. The offset is then bounded by the
        /// displacement the output underwent during that second — never
        /// cumulative, never permanent.
        const PEREMPTION_SORTIE: std::time::Duration = std::time::Duration::from_secs(1);

        /// 🔴 **THE REFERENCE ARRIVES BUILT, IT IS NO LONGER DEDUCED HERE.**
        /// It is built by the `match` of `demarrage::source::construire`
        /// — the SAME `match` that chooses the capture mode and which, in the
        /// multi-window arm, holds the source's size cell. A
        /// second computation here, even correct the day it is written, is
        /// exactly what produced the defects of batches 32M and 32Q.
        pub fn new(
            hwnd: HWND,
            reference: crate::entrees::Reference,
            mode_relatif: Arc<AtomicBool>,
        ) -> Self {
            tracing::info!(
                ?reference,
                "reference des entrees retenue (batie par le match qui choisit \
                 le mode de capture ; la taille de l'image y est PARTAGEE avec \
                 la source, jamais recalculee)"
            );
            Self {
                hwnd,
                reference,
                sortie: None,
                mode_relatif,
                premier_plan_obtenu: None,
            }
        }

        pub fn inject(&mut self, message: InputMessage) -> Result<()> {
            match message {
                InputMessage::MouseMove { x, y } => self.move_mouse(x, y),
                InputMessage::MouseButton {
                    button,
                    pressed,
                    x,
                    y,
                } => {
                    // In absolute: always position before clicking, the
                    // channel not being ordered, the corresponding movement
                    // may have been lost.
                    //
                    // In RELATIVE: absolutely not. The coordinates carried by
                    // the message no longer mean anything under Pointer Lock, and an
                    // absolute repositioning would teleport the cursor at
                    // each shot. Reread at EACH call (not captured once at
                    // construction): the mode can toggle during the
                    // session, at the whim of the cursor polling thread.
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
                    send_mouse(MOUSEINPUT {
                        dwFlags: flags,
                        ..Default::default()
                    })
                }
                InputMessage::MouseMoveRelative { dx, dy } => send_mouse(MOUSEINPUT {
                    dx: dx as i32,
                    dy: dy as i32,
                    dwFlags: MOUSEEVENTF_MOVE,
                    ..Default::default()
                }),
                InputMessage::Gamepad(_) => {
                    // Ignored here, on purpose: `demarrage.rs` intercepts this
                    // variant BEFORE calling the injector (task 10, ViGEmBus
                    // virtual gamepad) — the keyboard/mouse injector has
                    // nothing to do with it. A silent arm, without this comment,
                    // would be a trap for later: one would believe the message
                    // handled while it never was by this module.
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
                InputMessage::Key {
                    scancode,
                    pressed,
                    extended,
                } => {
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

        /// Brings this session's window to the foreground before injecting
        /// keyboard input.
        ///
        /// **Necessary and probably not sufficient.** `SendInput` is global
        /// to the Windows session: it addresses no one, it feeds the input queue
        /// of the active window. Without this call, all sessions
        /// type into the same window — the one in the foreground.
        /// With it, two sessions typing at the same time fight over it. The
        /// structural answer lies elsewhere (targeted injection through messages, or
        /// a driver) and remains out of scope of sub-block D2.
        ///
        /// **The return is checked.** `SetForegroundWindow` fails
        /// silently when the calling process is not allowed to
        /// steal focus: without this trace, one could not distinguish "the
        /// foreground was not enough" from "the foreground was never
        /// given". Logged once per toggle and not per keystroke — one
        /// log per key would drown the channel.
        fn au_premier_plan(&mut self) {
            if unsafe { GetForegroundWindow() } == self.hwnd {
                return;
            }
            let obtenu = unsafe { SetForegroundWindow(self.hwnd) }.as_bool();
            if self.premier_plan_obtenu != Some(obtenu) {
                if obtenu {
                    tracing::info!(hwnd = ?self.hwnd, "premier plan obtenu avant injection clavier");
                } else {
                    tracing::warn!(
                        hwnd = ?self.hwnd,
                        "SetForegroundWindow refusé — le clavier ira à la fenêtre active"
                    );
                }
                self.premier_plan_obtenu = Some(obtenu);
            }
        }

        fn move_mouse(&mut self, x: u16, y: u16) -> Result<()> {
            let window = self.rectangle_de_reference()?;
            let desktop = virtual_desktop();
            // On the region ACTUALLY captured: the browser normalises its
            // coordinates on the image it receives.
            //
            // 🔴 **THIS COMMENT WAS WRONG, IN THE PRESENT TENSE, FROM SUB-BLOCK D10 TO
            // BATCH 32M.** It asserted: "this image is the intersection of the
            // window with the screen". **That was true before D10** — the capture
            // then cropped the window. Since then, the multi-window path
            // captures the WHOLE OUTPUT (`ModeCapture::SortieEntiere`), wallpaper
            // and taskbar included, and this sentence stopped
            // being true underneath it without anyone rereading it. The
            // unmapping on the window's client area then produced an
            // error with two terms — origin + scale —, measured at +1288 px
            // in x and +51 px in y on the owner's machine.
            //
            // **What is true today**: the reference is
            // `rectangle_de_reference()` — the ORIGIN of the captured output and
            // the SIZE OF THE IMAGE, the latter SHARED with the source and
            // not recomputed. See `crate::entrees`.
            //
            // ⚠️ **Batch 32Q fixed the origin and left the size**, hence
            // a purely proportional residual drift: +432 px at the right
            // edge on the owner's machine (output 1860, image 1428),
            // nil on the left, nil in y. That is what E1 closes.
            //
            // **The commands that establish it**, so that the next
            // reader redoes the check without believing anyone:
            //
            // ```text
            // grep -n 'normalised on 0..65535' client/src/input.ts
            // grep -rn 'ModeCapture::SortieEntiere' agent/src/windows_source/
            // grep -rn 'TailleImage' agent/src/
            // cargo test --workspace entrees::
            // ```
            //
            // ⚠️ It is the second comment of this repository to lie in the present tense
            // after `superviseur/placement.rs`, and for the same reason: accurate
            // when written, never reread after the change that undid it.
            let Some((absolute_x, absolute_y)) = to_virtual_desktop_visible(x, y, window, desktop)
            else {
                // Window entirely off screen: no image is sent,
                // there is therefore no point to aim at. Ignore rather than
                // inject at random.
                return Ok(());
            };
            send_mouse(MOUSEINPUT {
                dx: absolute_x,
                dy: absolute_y,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                ..Default::default()
            })
        }

        /// 🔴 THE RECTANGLE ON WHICH THE BROWSER NORMALISED ITS
        /// COORDINATES — that is **what was captured**, never anything
        /// else. See `crate::entrees` for the defect this indirection
        /// closes and for the error it cancels, term by term.
        fn rectangle_de_reference(&mut self) -> Result<Rect> {
            let (nom, image) = match &self.reference {
                crate::entrees::Reference::ZoneClientDeLaFenetre => {
                    return self.client_rect_on_screen()
                }
                crate::entrees::Reference::SortieCapturee { nom, image } => {
                    // Reread at EACH call, never cached with the
                    // output's rectangle: it is the only half of the two
                    // that changes without any output being born or dying
                    // (a browser `resize`), and reading it only costs
                    // one atomic load — nothing to save.
                    (nom.clone(), image.lire())
                }
            };
            // The ORIGIN comes from the DXGI output (cached, see
            // `PEREMPTION_SORTIE`); the SIZE comes from the image. The two
            // halves have different sources because they have different
            // lifetimes — and it is `crate::entrees` that
            // assembles them, in a single place.
            let sortie = self.rectangle_de_la_sortie(&nom)?;
            crate::entrees::rectangle_capture(sortie, image).ok_or_else(|| {
                anyhow!(
                    "taille de l'image capturée encore inconnue ({}x{}) : aucune \
                     référence fiable pour démapper les entrées",
                    image.0,
                    image.1
                )
            })
        }

        /// The rectangle of the named DXGI output — **only its ORIGIN is
        /// used** (`crate::entrees::rectangle_capture`); its size is
        /// that of the OUTPUT, which is not that of the image.
        fn rectangle_de_la_sortie(&mut self, nom: &str) -> Result<Rect> {
            if let Some((rect, releve)) = self.sortie {
                if releve.elapsed() < Self::PEREMPTION_SORTIE {
                    return Ok(rect);
                }
            }
            let sorties = crate::capture::enumerer_sorties_silencieux()
                .context("énumération DXGI pour la référence des entrées")?;
            let trouvee = sorties.iter().find(|s| s.nom_sortie == nom).map(|s| s.rect);
            match trouvee {
                Some(rect) => {
                    self.sortie = Some((rect, std::time::Instant::now()));
                    Ok(rect)
                }
                // ⚠️ **We do NOT fall back on the window.** That would silently
                // reintroduce the offset this path exists to remove, and
                // the symptom would again become "the mouse clicks beside" without
                // any trace saying so. Better a named error.
                None => Err(anyhow!(
                    "sortie capturée {nom} introuvable dans la topologie DXGI : \
                     aucune référence fiable pour démapper les entrées"
                )),
            }
        }

        /// Client area of the window, expressed in screen coordinates.
        fn client_rect_on_screen(&self) -> Result<Rect> {
            let mut rect = RECT::default();
            unsafe { GetClientRect(self.hwnd, &mut rect)? };
            let mut origin = POINT {
                x: rect.left,
                y: rect.top,
            };
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
            return Err(anyhow!(
                "SendInput a refusé l'entrée (session verrouillée ?)"
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use win::InputInjector;
