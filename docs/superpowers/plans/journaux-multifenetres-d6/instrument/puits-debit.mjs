#!/usr/bin/env node
// Puits de débit brut, côté HÔTE (192.168.3.1). Compte les octets qui
// arrivent de la VM par le pont `internalBridge`, en TCP et en UDP.
//
// Pourquoi cet instrument existe : la question de la tâche 1 est « le pont
// porte-t-il assez pour que 8 × 12 Mb/s l'aient saturé ? ». Une mesure prise
// à travers WebRTC ne peut pas y répondre seule — elle est bornée par
// l'encodeur, par le sondage BWE et par le navigateur autant que par le lien.
// Ce puits mesure le CHEMIN, sans encodeur ni WebRTC.
//
// Deux transports parce qu'ils ne disent pas la même chose : TCP donne la
// capacité utile du chemin (fenêtre glissante, pas de perte), UDP donne ce
// qui traverse sans contrôle de flux — c'est le transport réel du média.
//
// Usage : node puits-debit.mjs <tcp|udp> [port] [duree_s_max]
// Sortie : une ligne JSON par bilan, sur stdout.

import net from 'node:net';
import dgram from 'node:dgram';

const mode = process.argv[2] ?? 'tcp';
const port = Number(process.argv[3] ?? 9999);
const limiteS = Number(process.argv[4] ?? 60);

const bilan = (etiquette, octets, ms, extra = {}) => {
    const s = ms / 1000;
    console.log(JSON.stringify({
        etiquette, transport: mode, octets, secondes: Number(s.toFixed(3)),
        // CALCULÉ à partir des deux relevés ci-dessus, jamais mesuré tel quel.
        mbps_calcule: Number(((octets * 8) / s / 1e6).toFixed(2)),
        ...extra,
    }));
};

if (mode === 'tcp') {
    const srv = net.createServer((sock) => {
        sock.setNoDelay(true);
        let octets = 0;
        const t0 = process.hrtime.bigint();
        sock.on('data', (b) => { octets += b.length; });
        sock.on('close', () => {
            const ms = Number(process.hrtime.bigint() - t0) / 1e6;
            bilan('connexion close', octets, ms, { pair: `${sock.remoteAddress}` });
            srv.close();
        });
        sock.on('error', () => { });
    });
    srv.listen(port, '0.0.0.0', () => console.error(`puits TCP prêt sur ${port}`));
    setTimeout(() => { console.error('limite atteinte'); process.exit(0); }, limiteS * 1000);
} else {
    const srv = dgram.createSocket('udp4');
    let octets = 0; let paquets = 0; let t0 = null;
    // Un gros tampon de réception : sans lui, c'est la file du socket qui
    // borne, pas le pont — et l'on mesurerait notre propre instrument.
    srv.on('listening', () => {
        try { srv.setRecvBufferSize(16 * 1024 * 1024); } catch { }
        console.error(`puits UDP prêt sur ${port} recvbuf=${srv.getRecvBufferSize()}`);
    });
    srv.on('message', (m) => {
        if (t0 === null) t0 = process.hrtime.bigint();
        octets += m.length; paquets += 1;
    });
    srv.bind(port, '0.0.0.0');
    let dernier = 0;
    const fin = () => {
        const ms = t0 === null ? 0 : Number(process.hrtime.bigint() - t0) / 1e6;
        bilan('fin', octets, ms || 1, { paquets });
        process.exit(0);
    };
    // On s'arrête sur le SILENCE (1,5 s sans rien), pas sur une durée : la
    // durée réelle de l'émission est ce qu'on veut mesurer.
    const veille = setInterval(() => {
        if (t0 !== null && octets === dernier) { clearInterval(veille); fin(); }
        dernier = octets;
    }, 1500);
    setTimeout(fin, limiteS * 1000);
}
