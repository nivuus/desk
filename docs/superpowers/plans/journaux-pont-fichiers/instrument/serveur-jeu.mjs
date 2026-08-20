// Le serveur du jeu de donnees et de la mire animee.
//
// 🔴 POURQUOI IL EXISTE : la page-shell tourne sur l'hote, dans un Chrome sans
// interface, et doit servir au pont fichiers les octets d'un VRAI repertoire du
// disque de l'hote. Le seul chemin qu'un navigateur offre pour lire un fichier
// local SANS selecteur est OPFS, qui est vide a la naissance. On l'y peuple
// donc par `fetch`, et ce serveur est ce que la page fetche.
//
// Les octets vont ainsi : disque de l'hote -> HTTP -> `Blob` reel dans la page
// -> OPFS (vrai `FileSystemFileHandle`) -> `adaptateur.ts` -> WebRTC -> agent
// -> ProjFS -> fichier sur la VM. La chaine sous test est traversee entiere.
//
// ⚠️ CORS OUVERT : la page est servie par Vite sur 5173, ce serveur ecoute
// ailleurs. Sans `Access-Control-Allow-Origin`, le `fetch` echouerait et le
// jeu serait vide -- une racine vide se lirait comme un pont casse.

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const RACINE_JEU = process.env.JEU
    ?? path.join(process.env.HOME ?? '/tmp', 'jeu-f1');
const PORT = Number(process.env.PORT_JEU ?? 5399);

/** La mire : cadence AFFICHEE PAR LA MIRE ELLE-MEME (piege D4). */
const MIRE = `<!doctype html><meta charset="utf-8"><title>mire f1</title>
<style>html,body{margin:0;background:#101018;overflow:hidden}
canvas{display:block}#hud{position:fixed;top:8px;left:8px;color:#0f0;
font:16px monospace;text-shadow:0 0 4px #000}</style>
<canvas id="c" width="1280" height="720"></canvas><div id="hud">…</div>
<script>
const c=document.getElementById('c'),g=c.getContext('2d');
let n=0,t0=performance.now(),raf=0,dernier=t0;
function boucle(t){
  n++;
  // Fond qui derive : Desktop Duplication n'emet QU'AU CHANGEMENT du bureau.
  g.fillStyle='hsl('+((n*2)%360)+',70%,45%)';g.fillRect(0,0,c.width,c.height);
  g.fillStyle='#fff';g.font='bold 96px monospace';
  g.fillText('F1 '+n,60,c.height/2);
  if(t-dernier>=500){raf=Math.round(n/((t-t0)/1000));dernier=t;
    document.getElementById('hud').textContent='rAF '+raf+' Hz  trames '+n;}
  requestAnimationFrame(boucle);
}
requestAnimationFrame(boucle);
</script>`;

function typeDe(p) {
    if (p.endsWith('.html')) return 'text/html; charset=utf-8';
    if (p.endsWith('.txt')) return 'text/plain; charset=utf-8';
    return 'application/octet-stream';
}

const serveur = http.createServer((req, rep) => {
    const cors = {
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Headers': '*',
    };
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/mire.html') {
        rep.writeHead(200, { ...cors, 'Content-Type': 'text/html; charset=utf-8' });
        rep.end(MIRE);
        return;
    }
    // `/liste` : l'arborescence, pour que la page sache quoi fetcher. Les
    // chemins sont rendus TELS QUELS, casse comprise -- c'est ce qui permet au
    // legs de casse d'etre OBSERVE et non conjecture.
    if (url.pathname === '/liste') {
        const sortie = [];
        const descendre = (rel) => {
            const abs = path.join(RACINE_JEU, rel);
            for (const e of fs.readdirSync(abs, { withFileTypes: true })) {
                const r = rel ? `${rel}/${e.name}` : e.name;
                if (e.isDirectory()) { sortie.push({ chemin: r, type: 'directory' }); descendre(r); }
                else sortie.push({ chemin: r, type: 'file', taille: fs.statSync(path.join(RACINE_JEU, r)).size });
            }
        };
        descendre('');
        rep.writeHead(200, { ...cors, 'Content-Type': 'application/json; charset=utf-8' });
        rep.end(JSON.stringify(sortie));
        return;
    }
    if (url.pathname.startsWith('/jeu/')) {
        const rel = decodeURIComponent(url.pathname.slice('/jeu/'.length));
        const abs = path.join(RACINE_JEU, rel);
        // Garde de traversee : le serveur ne sert que sous RACINE_JEU.
        if (!abs.startsWith(RACINE_JEU)) { rep.writeHead(403, cors); rep.end(); return; }
        if (!fs.existsSync(abs) || !fs.statSync(abs).isFile()) {
            rep.writeHead(404, cors); rep.end(); return;
        }
        rep.writeHead(200, { ...cors, 'Content-Type': typeDe(abs) });
        fs.createReadStream(abs).pipe(rep);
        return;
    }
    rep.writeHead(404, cors); rep.end();
});

serveur.listen(PORT, '0.0.0.0', () => {
    console.log(`serveur du jeu : http://0.0.0.0:${PORT} (racine ${RACINE_JEU})`);
});
