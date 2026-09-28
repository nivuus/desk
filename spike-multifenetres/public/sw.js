// Service worker of the spike. Two roles:
//   1. make the PWA installable — Chromium requires a `fetch` handler;
//   2. carry variant 4: open a window from notificationclick, which
//      is a valid user activation even with the window in the background.
//
// The fetch is deliberately a plain pass-through, with no cache at all: caching
// the spike pages would test stale code between two reruns.

// `skipWaiting` is the counterpart of this absence of cache: without it, a changed
// version of the service worker would stay waiting behind the old one, and
// variant 4 would be measured on the code of the previous rerun.
self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (evenement) => {
    evenement.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', (evenement) => {
    evenement.respondWith(fetch(evenement.request));
});

// Reports to the server that the opening did not happen. Without this signal, a refused
// opening and a window gone off to the identity provider would be
// indistinguishable — both resulting in 60 s of silence — whereas
// that is precisely the confusion the classification was built to avoid, and
// on the variant that is the documented fallback of the product framing.
async function signalerBlocage(url, raison) {
    try {
        const parametres = new URL(url, self.location.origin).searchParams;
        const variante = parametres.get('variant') ?? '4';
        const nonce = parametres.get('nonce') ?? '';
        await fetch(`/bloque?variant=${encodeURIComponent(variante)}&nonce=${encodeURIComponent(nonce)}`);
    } catch (erreur) {
        console.error('[spike] blocage non rapporté', raison, erreur);
    }
}

self.addEventListener('notificationclick', (evenement) => {
    evenement.notification.close();
    const url = evenement.notification.data?.url ?? '/opened.html?variant=4';
    // waitUntil is mandatory: without it the service worker may be stopped
    // before the window opens, and the failure would be blamed on the browser.
    evenement.waitUntil((async () => {
        try {
            // `openWindow` returns `null` when the browser refuses the opening:
            // a mute value that must be turned into a verdict, otherwise the
            // page would only see a delay elapsing with nothing.
            const fenetre = await self.clients.openWindow(url);
            if (!fenetre) await signalerBlocage(url, 'poignée nulle');
        } catch (erreur) {
            await signalerBlocage(url, erreur);
        }
    })());
});
