//! Injection des entrées du navigateur dans la session Windows.
//!
//! Le mapping de coordonnées (fenêtre → bureau virtuel) est indépendant de
//! toute API Windows et vit dans `crate::geometry::to_virtual_desktop`,
//! testé sur Linux (tâche 12 : le brief l'attendait ici, mais `Rect` a été
//! introduit à la tâche 9 dans `geometry.rs`, aux côtés de `crop_region` —
//! le mapping l'y rejoint plutôt que de dupliquer `Rect`). Ce module ne
//! porte donc que l'appel système `SendInput`, propre à Windows.

/// Les quatre événements clavier d'un collage : `Ctrl`↓, `V`↓, `V`↑, `Ctrl`↑.
///
/// **Hors du `#[cfg(windows)]`, à dessein** : c'est une table, pas un appel
/// système, et l'appelant (`transport/boucle.rs`) n'est pas gaté.
///
/// 🔴 **AUTO-SUFFISANTE EN MODIFICATEURS, et ce n'est pas de la prudence.**
/// Le canal d'entrées du client est `ordered: false, maxRetransmits: 0`
/// (`client/src/webrtc.ts`) : l'état des modificateurs côté VM au moment de
/// l'injection **n'est pas connaissable** — le `Ctrl`↓ que le client a envoyé
/// peut être arrivé, avoir été perdu, ou arriver après. Poser soi-même les
/// quatre événements rend le geste indépendant de tout cela ; un `Ctrl`↑ de
/// trop est inoffensif, un `Ctrl`↓ manquant ne collerait rien.
///
/// 🔴 **LES QUATRE, ET DANS CET ORDRE.** Omettre le `Ctrl`↑ final laisserait
/// l'application avec un modificateur ENFONCÉ, et **toute frappe suivante
/// deviendrait un raccourci** — le défaut le plus insidieux de ce chemin, et
/// celui qu'un test garde rouge.
///
/// Scancodes relevés sur `client/src/scancodes.ts`, la table que le client
/// emploie déjà : `ControlLeft` = `0x1d`, `KeyV` = `0x2f`, `extended: false`
/// aux deux. Les reprendre de là plutôt que de les redécouvrir garantit que
/// la VM reçoit exactement ce qu'elle reçoit d'une frappe humaine.
///
/// ⚠️ **La portée du dépôt sur `SetForegroundWindow` n'est PAS élargie par
/// cette table.** Le bras `InputMessage::Key` de `InputInjector` appelle déjà
/// `au_premier_plan()`, ce dont ce chemin hérite gratuitement — mais ce qui
/// est MESURÉ (D2) est « une frappe par fenêtre, sonde séquentielle, aucune
/// frappe concurrente », et `SendInput` reste GLOBAL à la session Windows.
/// **Deux collages simultanés depuis deux fenêtres restent hors de ce qui est
/// établi** ; P3 est le sous-bloc qui les rencontrera.
pub const TOUCHES_COLLAGE: [proto::input::InputMessage; 4] = [
    proto::input::InputMessage::Key { scancode: 0x1d, pressed: true, extended: false },
    proto::input::InputMessage::Key { scancode: 0x2f, pressed: true, extended: false },
    proto::input::InputMessage::Key { scancode: 0x2f, pressed: false, extended: false },
    proto::input::InputMessage::Key { scancode: 0x1d, pressed: false, extended: false },
];

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
        /// 🔴 SUR QUOI LES COORDONNÉES SE DÉMAPPENT — voir `crate::entrees`.
        /// Dérivée de `config.sortie_dxgi`, **le même discriminant que le mode
        /// de capture** : il n'y a pas deux descriptions à tenir d'accord.
        reference: crate::entrees::Reference,
        /// Le rectangle de la sortie capturée, et l'instant de son relevé.
        ///
        /// ⚠️ **Mis en cache, et il le faut** : `move_mouse` court à la cadence
        /// des mouvements de souris, et énumérer DXGI à chaque événement
        /// coûterait des appels COM par dizaines par seconde. ⚠️ **Mais pas
        /// figé non plus** : la disposition du bureau virtuel change quand une
        /// sortie naît ou meurt, et une origine périmée redonnerait exactement
        /// le défaut qu'on corrige. D'où la péremption ci-dessous.
        sortie: Option<(Rect, std::time::Instant)>,
        /// Renseigné par le fil de sondage du curseur (`cursor.rs`).
        /// L'agent est SEUL décideur du mode : le client n'a rien à savoir,
        /// et il n'existe qu'une source de vérité — la seule construction
        /// correcte sur un canal non ordonné.
        mode_relatif: Arc<AtomicBool>,
        /// Dernier résultat connu de `SetForegroundWindow`, pour ne journaliser
        /// qu'au basculement et non à chaque frappe. `None` à la construction
        /// — un sentinelle à DEUX états (`bool` initialisé à `false`) rendrait
        /// muet le tout premier échec, car `false == false` ne bascule rien :
        /// exactement le cas que cette trace existe pour révéler. `None` ne
        /// collisionne avec aucun résultat réel de `SetForegroundWindow`, donc
        /// le premier appel trace toujours, qu'il réussisse ou échoue.
        premier_plan_obtenu: Option<bool>,
    }

    impl InputInjector {
        /// Durée de validité du rectangle de la sortie.
        ///
        /// ⚠️ **Non calibrée** : une seconde est courte devant la fréquence à
        /// laquelle une sortie naît ou meurt (de l'ordre de l'ouverture d'une
        /// fenêtre) et longue devant la cadence des mouvements de souris. Ce
        /// n'est pas une constante mesurée, et elle est déclarée telle.
        ///
        /// ⚠️ **CE QU'ELLE COÛTE, écrit ici pour que personne n'ait à le
        /// redériver** : si la sortie capturée change d'origine ou de taille,
        /// le démappage reste faux **au pire une seconde**, puis se corrige
        /// tout seul au prochain relevé. Le décalage est alors borné par le
        /// déplacement qu'a subi la sortie pendant cette seconde — jamais
        /// cumulatif, jamais permanent.
        const PEREMPTION_SORTIE: std::time::Duration = std::time::Duration::from_secs(1);

        pub fn new(
            hwnd: HWND,
            sortie_dxgi: Option<&str>,
            mode_relatif: Arc<AtomicBool>,
        ) -> Self {
            let reference = crate::entrees::reference(sortie_dxgi);
            tracing::info!(
                ?reference,
                "reference des entrees retenue (elle DERIVE de config.sortie_dxgi, \
                 le meme discriminant que le mode de capture)"
            );
            Self { hwnd, reference, sortie: None, mode_relatif, premier_plan_obtenu: None }
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
            // Sur la région RÉELLEMENT capturée : le navigateur normalise ses
            // coordonnées sur l'image qu'il reçoit.
            //
            // 🔴 **CE COMMENTAIRE A ÉTÉ FAUX, AU PRÉSENT, DU SOUS-BLOC D10 AU
            // LOT 32M.** Il affirmait : « cette image est l'intersection de la
            // fenêtre avec l'écran ». **C'était vrai avant D10** — la capture
            // recadrait alors la fenêtre. Depuis, le chemin multi-fenêtres
            // capture la SORTIE ENTIÈRE (`ModeCapture::SortieEntiere`), fond
            // d'écran et barre des tâches compris, et cette phrase a cessé
            // d'être vraie sous elle sans que personne ne la relise. Le
            // démappage sur la zone client de la fenêtre produisait alors une
            // erreur à deux termes — origine + échelle —, mesurée à +1288 px
            // en x et +51 px en y sur la machine du propriétaire.
            //
            // **Ce qui est vrai aujourd'hui** : la référence est
            // `rectangle_de_reference()`, qui DÉRIVE de `config.sortie_dxgi` —
            // le même discriminant que le mode de capture. Voir
            // `crate::entrees`.
            //
            // **Les commandes qui l'établissent**, pour que le prochain
            // lecteur refasse le contrôle sans croire personne :
            //
            // ```text
            // grep -n 'normalisées sur 0..65535' client/src/input.ts
            // grep -rn 'ModeCapture::SortieEntiere' agent/src/windows_source/
            // cargo test --workspace entrees::
            // ```
            //
            // ⚠️ C'est le second commentaire de ce dépôt à mentir au présent
            // après `superviseur/placement.rs`, et pour la même raison : exact
            // à l'écriture, jamais relu après le changement qui l'a défait.
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

        /// 🔴 LE RECTANGLE SUR LEQUEL LE NAVIGATEUR A NORMALISÉ SES
        /// COORDONNÉES — c'est-à-dire **ce qui a été capturé**, jamais autre
        /// chose. Voir `crate::entrees` pour le défaut que cette indirection
        /// ferme et pour l'erreur qu'elle annule, terme par terme.
        fn rectangle_de_reference(&mut self) -> Result<Rect> {
            let nom = match &self.reference {
                crate::entrees::Reference::ZoneClientDeLaFenetre => {
                    return self.client_rect_on_screen()
                }
                crate::entrees::Reference::SortieCapturee(nom) => nom.clone(),
            };
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
                // ⚠️ **On NE retombe PAS sur la fenêtre.** Ce serait réintroduire
                // en silence le décalage que ce chemin existe pour supprimer, et
                // le symptôme redeviendrait « la souris clique à côté » sans
                // qu'aucune trace ne le dise. Mieux vaut une erreur nommée.
                None => Err(anyhow!(
                    "sortie capturée {nom} introuvable dans la topologie DXGI : \
                     aucune référence fiable pour démapper les entrées"
                )),
            }
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
