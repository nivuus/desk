// Le script du critere ⑤ — EXTERNE, pour survivre a `script-src 'self'`.
//
// 🔴 IL POSE UN MARQUEUR AVANT TOUT : sans lui, « aucune bascule » serait
// indiscernable d'« aucun script execute » (piege caracterise a la tache 14).
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
    fetchErreur: null,
};
window.__p5 = etat;

// Les violations CSP, captees DANS la page (l'evenement standard).
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

// (c) la montee wss:// vers la MEME origine — donc plus de contenu mixte.
// 🔴 ROUGE : la ligne EXACTE d'avant la tache 16 (git show 7566b12^:client/src/main.ts)
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
    // Une CSP qui refuse `connect-src` LEVE de facon synchrone : c'est le
    // chemin qui rend la rouge insensible au budget de temps virtuel.
    etat.ws = 'WS-REFUSE';
    etat.wsLeve = String(e && e.name);
    peindre();
}

// (d) la requete POST /auth/connexion, et surtout : SON CORPS EST-IL LISIBLE
// PAR LA PAGE ? Un 200 dont le corps serait illisible (CORS mal pose) se
// lirait comme un succes cote reseau et un echec cote produit.
// ⚠️ LES IDENTIFIANTS VIENNENT D'UN FICHIER QUE LE PILOTE ECRIT DANS
// `client/dist/` (GITIGNORE) et SUPPRIME apres la mesure. Ils ne sont ni dans
// cet instrument, ni dans aucune piece versee.
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
        // 🔴 ON NE RECOPIE JAMAIS LES JETONS : seulement LES NOMS DES CHAMPS.
        etat.fetchChamps = Object.keys(j).sort();
        peindre();
    })
    .catch((e) => {
        etat.fetchLisible = false;
        etat.fetchErreur = String(e);
        peindre();
    });

peindre();
