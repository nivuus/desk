// Injectée AVANT tout script de la page (`Page.addScriptToEvaluateOnNewDocument`).
//
// Deux choses, et rien d'autre : le jeton de session (sans quoi `shell-page.ts`
// redirige vers `connexion.html` et tout ce qui suit mesurerait l'écran de
// connexion), et **le témoin de fil** du bloc E3.
//
// 🔴 LE TÉMOIN DE FIL EST INDÉPENDANT DE NOTRE CODE CLIENT, et c'est ce qui
// fait sa valeur. Il enregistre les messages `mic-state` tels qu'ils ARRIVENT
// sur le canal de contrôle, avec leur horodatage — jamais ce que `micro.ts` en
// fait. Sans lui, « le bandeau s'affiche » et « le message est arrivé » se
// lisent pareil, et un bandeau posé par notre propre code sur une supposition
// passerait pour une mesure.
//
// 🔵 IL CRIE AUSSI QUAND LE MESSAGE PART TROP TÔT. Chaque entrée porte son
// instant ; le pilote le compare à l'instant du clic sur le bouton micro. Un
// `mic-state` antérieur au clic serait un message émis avant qu'aucun paquet
// montant n'existe — c'est le défaut qu'un chantier voisin vient de payer, et
// que seul son propre témoin a dénoncé.
//
// ⚠️ LE PILOTE SUBSTITUE PAR `replaceAll`, ET CE COMMENTAIRE NE NOMME AUCUN
// MARQUEUR : une première version de F1 les citait en toutes lettres, et la
// substitution frappait le COMMENTAIRE en laissant le vrai marqueur intact.
(() => {
    try {
        localStorage.setItem('guac.jeton.acces', '__JETON_ACCES__');
        localStorage.setItem('guac.jeton.rafraichissement', '__JETON_RAFRAICHISSEMENT__');
        localStorage.setItem('guac.prefixe', '__PREFIXE__');
    } catch (e) { /* page sans localStorage */ }

    window.__e3 = { micState: [], canaux: 0, erreurs: [], ouvertures: [] };

    // 🔴 LE PARAMÈTRE `signaling` EST AJOUTÉ AUX FENÊTRES QUE LA SHELL OUVRE,
    // ET C'EST UNE COMPENSATION DE MONTAGE, PAS UN CORRECTIF DE PRODUIT.
    //
    // `shell-page.ts` ouvre `/?session=<id>` SANS `signaling`, et
    // `adresseSignaling` retombe alors sur `ws://${location.host}` —
    // c'est-à-dire, en recette, sur le serveur de développement `vite`
    // (127.0.0.1:5173) et non sur la plateforme (127.0.0.1:8080). Les pages
    // d'application restaient à « Connexion… » indéfiniment, sans une ligne de
    // console : `createDataChannel` n'était jamais appelé.
    //
    // ⚠️ **CE N'EST PAS UN DÉFAUT DU PRODUIT.** Depuis le sous-bloc P5 de la
    // plateforme, la page et l'API sont servies par la MÊME origine derrière
    // nginx, et le repli sur `location.host` est alors exactement juste. C'est
    // le montage de recette — deux serveurs, deux ports — qui les sépare.
    //
    // ⚠️ **Le nom de fenêtre est PRÉSERVÉ** : `shell-page.ts` passe
    // `guac-<session>`, et le perdre ferait rouvrir une fenêtre à chaque appel
    // au lieu de réutiliser la sienne.
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

    // Le canal de contrôle est créé PAR LE CLIENT (`createDataChannel`), donc
    // c'est là que le témoin se pose. Un `addEventListener` PASSIF : il n'ôte
    // rien à `main.ts`, qui reçoit le même événement.
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
