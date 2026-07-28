// Service worker du spike. Deux rôles :
//   1. rendre la PWA installable — Chromium exige un gestionnaire `fetch` ;
//   2. porter la variante 4 : ouvrir une fenêtre depuis notificationclick, qui
//      est une activation utilisateur valide même fenêtre en arrière-plan.
//
// Le fetch est délibérément un simple passe-plat, sans aucun cache : mettre en
// cache les pages du spike ferait tester du code périmé entre deux relances.

// `skipWaiting` est le pendant de cette absence de cache : sans lui, une version
// modifiée du service worker resterait en attente derrière l'ancienne, et la
// variante 4 serait mesurée sur le code de la relance précédente.
self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (evenement) => {
    evenement.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', (evenement) => {
    evenement.respondWith(fetch(evenement.request));
});

// Rapporte au serveur que l'ouverture n'a pas eu lieu. Sans ce signal, un refus
// d'ouverture et une fenêtre partie sur le fournisseur d'identité seraient
// indiscernables — les deux se traduisant par un silence de 60 s — alors que
// c'est précisément la confusion que le classement a été bâti pour éviter, et
// sur la variante qui est le repli documenté du cadrage produit.
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
    // waitUntil est obligatoire : sans lui le service worker peut être arrêté
    // avant que la fenêtre ne s'ouvre, et l'échec serait imputé au navigateur.
    evenement.waitUntil((async () => {
        try {
            // `openWindow` rend `null` quand le navigateur refuse l'ouverture :
            // une valeur muette qu'il faut convertir en verdict, faute de quoi la
            // page ne verrait qu'un délai écoulé sans rien.
            const fenetre = await self.clients.openWindow(url);
            if (!fenetre) await signalerBlocage(url, 'poignée nulle');
        } catch (erreur) {
            await signalerBlocage(url, erreur);
        }
    })());
});
