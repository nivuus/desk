// The script of criterion ⑤ — EXTERNAL, to survive `script-src 'self'`.
//
// 🔴 IT SETS A MARKER BEFORE ANYTHING: without it, "no switch" would be
// indistinguishable from "no script executed" (a trap characterised at task 14).
const etat = {
    SCRIPT_OK: true,
    protocole: location.protocol,
    origine: location.origin,
    violationsCsp: [],
    ws: 'attente',
    wsUrl: null,
    fetchStatut: null,
    fetchLisible: null,
    fetchChamps: null,
    fetchError: null,
};
window.__p5 = etat;

// The CSP violations, caught IN the page (the standard event).
document.addEventListener('securitypolicyviolation', (e) => {
    etat.violationsCsp.push({
        directive: e.violatedDirective,
        bloque: e.blockedURI,
        source: e.sourceFile || null,
    });
});

function peindre() {
    document.getElementById('sortie').textContent = JSON.stringify(etat, null, 2);
}

// (c) the wss:// upgrade to the SAME origin — hence no more mixed content.
// 🔴 RED: the EXACT line from before task 16 (git show 7566b12^:client/src/main.ts)
const url = `ws://${window.location.hostname}:8080`;
etat.wsUrl = url;
try {
    const s = new WebSocket(url);
    s.onopen = () => { etat.ws = 'WS-OUVERT'; peindre(); };
    s.onerror = () => { etat.ws = 'WS-REFUSE'; peindre(); };
    s.onclose = (ev) => {
        if (etat.ws === 'attente') { etat.ws = 'WS-REFUSE'; }
        etat.wsFermeture = { code: ev.code, propre: ev.wasClean };
        peindre();
    };
} catch (e) {
    // A CSP refusing `connect-src` THROWS synchronously: it is the
    // path that makes the red run insensitive to the virtual time budget.
    etat.ws = 'WS-REFUSE';
    etat.wsLeve = String(e && e.name);
    peindre();
}

// (d) the POST /auth/connexion request, and above all: IS ITS BODY READABLE
// BY THE PAGE? A 200 whose body was unreadable (CORS badly set) would
// read as a success on the network side and a failure on the product side.
// ⚠️ THE CREDENTIALS COME FROM A FILE THE DRIVER WRITES INTO
// `client/dist/` (GITIGNORED) and DELETES after the measurement. They are neither in
// this instrument, nor in any filed piece.
const ident = window.__ident ?? null;
fetch(location.origin + '/auth/connexion', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(ident ?? { email: 'inconnu@p5.local', motdepasse: 'x' }),
})
    .then(async (r) => {
        etat.fetchStatut = r.status;
        const j = await r.json();
        etat.fetchLisible = true;
        // 🔴 WE NEVER COPY THE TOKENS: only THE FIELD NAMES.
        etat.fetchChamps = Object.keys(j).sort();
        peindre();
    })
    .catch((e) => {
        etat.fetchLisible = false;
        etat.fetchError = String(e);
        peindre();
    });

peindre();
