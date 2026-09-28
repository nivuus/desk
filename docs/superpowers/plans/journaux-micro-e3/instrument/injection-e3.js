// Injected BEFORE any script of the page (`Page.addScriptToEvaluateOnNewDocument`).
//
// Two things, and nothing else: the session token (otherwise `shell-page.ts`
// redirects to `connexion.html` and everything that follows would measure the sign-in
// screen), and **the wire witness** of block E3.
//
// 🔴 THE WIRE WITNESS IS INDEPENDENT OF OUR CLIENT CODE, and that is what
// gives it its value. It records the `mic-state` messages as they ARRIVE
// on the control channel, with their timestamp — never what `micro.ts` does
// with them. Without it, "the banner shows" and "the message arrived"
// read the same, and a banner set by our own code on an assumption
// would pass for a measurement.
//
// 🔵 IT ALSO SHOUTS WHEN THE MESSAGE GOES OUT TOO EARLY. Each entry carries its
// instant; the driver compares it with the instant of the click on the mic button. A
// `mic-state` earlier than the click would be a message emitted before any upstream
// packet exists — it is the defect a neighbouring workstream has just paid for, and
// that only its own witness denounced.
//
// ⚠️ THE DRIVER SUBSTITUTES THROUGH `replaceAll`, AND THIS COMMENT NAMES NO
// MARKER: a first version of F1 quoted them in full, and the
// substitution hit the COMMENT while leaving the real marker intact.
(() => {
    try {
        localStorage.setItem('guac.jeton.acces', '__JETON_ACCES__');
        localStorage.setItem('guac.jeton.rafraichissement', '__JETON_RAFRAICHISSEMENT__');
        localStorage.setItem('guac.prefixe', '__PREFIXE__');
    } catch (e) { /* page sans localStorage */ }

    window.__e3 = { micState: [], canaux: 0, erreurs: [], ouvertures: [] };

    // 🔴 THE `signaling` PARAMETER IS ADDED TO THE WINDOWS THE SHELL OPENS,
    // AND IT IS A SETUP COMPENSATION, NOT A PRODUCT FIX.
    //
    // `shell-page.ts` opens `/?session=<id>` WITHOUT `signaling`, and
    // `adresseSignaling` then falls back to `ws://${location.host}` —
    // that is, in the acceptance run, to the `vite` development server
    // (127.0.0.1:5173) and not to the platform (127.0.0.1:8080). The application
    // pages stayed at "Connecting…" indefinitely, without a single console
    // line: `createDataChannel` was never called.
    //
    // ⚠️ **IT IS NOT A PRODUCT DEFECT.** Since the platform's sub-block P5,
    // the page and the API are served by the SAME origin behind
    // nginx, and falling back to `location.host` is then exactly right. It is
    // the acceptance setup — two servers, two ports — that separates them.
    //
    // ⚠️ **The window name is PRESERVED**: `shell-page.ts` passes
    // `guac-<session>`, and losing it would reopen a window at each call
    // instead of reusing its own.
    try {
        const SIGNALING = '__SIGNALING__';
        const natif = window.open;
        window.open = function (u, ...reste) {
            let cible = u;
            try {
                if (typeof u === 'string' && u.indexOf('session=') !== -1 && u.indexOf('signaling=') === -1) {
                    cible = u + (u.indexOf('?') === -1 ? '?' : '&') + 'signaling=' + encodeURIComponent(SIGNALING);
                }
            } catch (e) { /* on ouvre l'original */ }
            window.__e3.ouvertures.push({ t: Date.now(), demande: String(u).slice(0, 200), ouverte: String(cible).slice(0, 200) });
            return natif.call(window, cible, ...reste);
        };
    } catch (e) { window.__e3.erreurs.push(String(e).slice(0, 200)); }

    // The control channel is created BY THE CLIENT (`createDataChannel`), so
    // that is where the witness attaches. A PASSIVE `addEventListener`: it takes
    // nothing away from `main.ts`, which receives the same event.
    try {
        const P = window.RTCPeerConnection;
        if (P && P.prototype && P.prototype.createDataChannel) {
            const natif = P.prototype.createDataChannel;
            P.prototype.createDataChannel = function (...a) {
                const c = natif.apply(this, a);
                window.__e3.canaux += 1;
                try {
                    c.addEventListener('message', (ev) => {
                        const t = String(ev.data ?? '');
                        if (t.indexOf('mic-state') === -1) return;
                        window.__e3.micState.push({ t: Date.now(), brut: t.slice(0, 200) });
                    });
                } catch (e) { window.__e3.erreurs.push(String(e).slice(0, 200)); }
                return c;
            };
        }
    } catch (e) { window.__e3.erreurs.push(String(e).slice(0, 200)); }
})();
