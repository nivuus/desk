// Service worker du spike. Deux rôles :
//   1. rendre la PWA installable — Chromium exige un gestionnaire `fetch` ;
//   2. porter la variante 4 : ouvrir une fenêtre depuis notificationclick, qui
//      est une activation utilisateur valide même fenêtre en arrière-plan.
//
// Le fetch est délibérément un simple passe-plat, sans aucun cache : mettre en
// cache les pages du spike ferait tester du code périmé entre deux relances.

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (evenement) => {
    evenement.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', (evenement) => {
    evenement.respondWith(fetch(evenement.request));
});

self.addEventListener('notificationclick', (evenement) => {
    evenement.notification.close();
    const url = evenement.notification.data?.url ?? '/opened.html?variant=4';
    // waitUntil est obligatoire : sans lui le service worker peut être arrêté
    // avant que la fenêtre ne s'ouvre, et l'échec serait imputé au navigateur.
    evenement.waitUntil(self.clients.openWindow(url));
});
