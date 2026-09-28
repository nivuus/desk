// Injected BEFORE any script of the page (Page.addScriptToEvaluateOnNewDocument).
//
// Taken from `journaux-pont-fichiers/instrument/injection-f1.js`, with what F2
// adds: a REREAD of OPFS and its digest, and reading the counter
// of owed writes.
//
// 🔴 THE RETURNED HANDLE IS A REAL `FileSystemDirectoryHandle` (OPFS), so
// `createWritable()` is the REAL one there, with its swap file and its
// commit on `close()`. That is what makes criterion ⑤ measurable.
//
// ⚠️ WHAT OPFS DOES NOT COVER, and it is declared: `showDirectoryPicker()` is
// never called, the permission model (`queryPermission` /
// `requestPermission`) is not exercised, and the `readwrite` mode F2 sets
// is not tested — OPFS has no permission model.
//
// ⚠️ THE DRIVER SUBSTITUTES THROUGH `replaceAll`, AND THIS COMMENT NAMES NONE OF ITS
// MARKERS: a first version of F1 quoted them in full here, and the
// substitution hit the COMMENT while leaving the real marker intact.
(() => {
    try {
        localStorage.setItem('guac.jeton.acces', '__JETON_ACCES__');
        localStorage.setItem('guac.jeton.rafraichissement', '__JETON_RAFRAICHISSEMENT__');
        localStorage.setItem('guac.prefixe', '__PREFIXE__');
    } catch (e) { /* page sans localStorage */ }

    // Capture of the `RTCPeerConnection`, for criterion ⑥ (`framesDecoded`).
    try {
        const N = window.RTCPeerConnection;
        if (N) {
            window.__pc = null;
            window.RTCPeerConnection = function (...a) {
                const p = new N(...a);
                window.__pc = p;
                return p;
            };
            window.RTCPeerConnection.prototype = N.prototype;
        }
    } catch (e) { /* criterion ⑥ will say */ }

    const BASE = '__BASE_JEU__';
    window.__f2 = { etapes: [], erreurs: [] };
    const noter = (m) => {
        window.__f2.etapes.push({ t: Date.now(), m: String(m).slice(0, 300) });
        if (window.__f2.etapes.length > 300) window.__f2.etapes.shift();
    };

    // 🔴 OPFS IS ONLY POPULATED BY THE SHELL PAGE, AND IT IS AN INSTRUMENT
    // DEFECT PAID FOR ON THE SPOT.
    //
    // The injection is set through `Target.setAutoAttach`, hence on ALL
    // pages — including the application windows opened by
    // `window.open('/?session=…')`. Each of them then ran the OPFS PURGE and
    // repopulated it, **while the bridge was writing there**.
    //
    // ⚠️ **THE SYMPTOM READS AS A PRODUCT DEFECT**: at run
    // `arme-2`, `Casse.txt`, `gros-lecture.bin` and `sous-dossier` had
    // DISAPPEARED from OPFS, and `CASSE.TXT` — which the case guard refuses in normal
    // times — was found created there. **The guard had not failed: the namesake
    // it looks for had been erased under it by another page.**
    const EST_SHELL = location.pathname.endsWith('/shell.html');
    const pret = (async () => {
        if (!EST_SHELL) throw new Error('OPFS n est peuple que par la page-shell');
        const racineOpfs = await navigator.storage.getDirectory();
        const dossier = await racineOpfs.getDirectoryHandle('Mes documents', { create: true });
        // Purge: OPFS persists in the profile, and a second run would read
        // the first one's data set.
        for await (const nom of dossier.keys()) {
            await dossier.removeEntry(nom, { recursive: true });
        }
        const liste = await (await fetch(BASE + '/liste')).json();
        for (const e of liste.filter((x) => x.type === 'directory')) {
            let ici = dossier;
            for (const p of e.chemin.split('/')) ici = await ici.getDirectoryHandle(p, { create: true });
        }
        for (const e of liste.filter((x) => x.type === 'file')) {
            const parts = e.chemin.split('/');
            const nom = parts.pop();
            let ici = dossier;
            for (const p of parts) ici = await ici.getDirectoryHandle(p, { create: true });
            const octets = await (await fetch(BASE + '/jeu/' + e.chemin.split('/').map(encodeURIComponent).join('/'))).arrayBuffer();
            if (octets.byteLength !== e.taille) {
                throw new Error('taille recue ' + octets.byteLength + ' != annoncee ' + e.taille + ' pour ' + e.chemin);
            }
            const fh = await ici.getFileHandle(nom, { create: true });
            const w = await fh.createWritable();
            await w.write(octets);
            await w.close();
        }
        noter('OPFS peuple : ' + liste.length + ' entrees');
        window.__f2.peuple = liste.length;
        window.__f2.racine = dossier;
        return dossier;
    })().catch((e) => {
        window.__f2.erreurs.push('peuplement OPFS : ' + String(e).slice(0, 300));
        throw e;
    });

    window.showDirectoryPicker = async (options) => {
        noter('showDirectoryPicker appele, mode=' + (options && options.mode));
        const d = await pret;
        noter('poignee OPFS rendue, name=' + d.name + ', kind=' + d.kind);
        return d;
    };

    // 🔴 THE REREAD, AND IT IS CRITERION ①. It reads OPFS — that is, what
    // the LOCAL MACHINE really carries — and returns size and digest. A
    // criterion "the file appears" would be satisfied by a truncated file,
    // by chunks out of order and by a missing last chunk:
    // all THREE are defects `pont/decoupe.rs` and the write thread
    // can really produce, and NONE of them shows to the eye.
    window.__relire = async (chemin) => {
        try {
            const racine = await navigator.storage.getDirectory();
            let ici = await racine.getDirectoryHandle('Mes documents');
            const parts = chemin.split('/').filter((p) => p.length > 0);
            const nom = parts.pop();
            for (const p of parts) ici = await ici.getDirectoryHandle(p);
            const f = await (await ici.getFileHandle(nom)).getFile();
            const octets = await f.arrayBuffer();
            const d = await crypto.subtle.digest('SHA-256', octets);
            return JSON.stringify({
                present: true,
                taille: f.size,
                sha256: Array.from(new Uint8Array(d)).map((b) => b.toString(16).padStart(2, '0')).join(''),
            });
        } catch (e) {
            return JSON.stringify({ present: false, erreur: String(e).slice(0, 200) });
        }
    };

    /** The complete OPFS tree, to see what arrived and what did not. */
    window.__arbre = async () => {
        const sortie = [];
        const descendre = async (poignee, prefixe) => {
            for await (const [nom, enfant] of poignee.entries()) {
                const chemin = prefixe ? prefixe + '/' + nom : nom;
                if (enfant.kind === 'directory') { sortie.push({ chemin, type: 'd' }); await descendre(enfant, chemin); }
                else sortie.push({ chemin, type: 'f', taille: (await enfant.getFile()).size });
            }
        };
        try {
            const racine = await navigator.storage.getDirectory();
            await descendre(await racine.getDirectoryHandle('Mes documents'), '');
        } catch (e) { sortie.push({ erreur: String(e).slice(0, 200) }); }
        return JSON.stringify(sortie);
    };

    // 🔴 THE COUNTER IS READ FROM ATTRIBUTES, NEVER FROM THE TEXT.
    // It is the remedy for the trap F1 paid nine minutes for: two messages
    // sharing a substring. And `vues` is CUMULATIVE: `dues=0` ALONE is
    // indistinguishable from a MEASUREMENT NOT TAKEN.
    window.__compteur = () => {
        const e = document.querySelector('#ecritures-dues');
        if (!e) return JSON.stringify({ absent: true });
        return JSON.stringify({
            dues: Number(e.dataset.dues),
            vues: Number(e.dataset.vues),
            texte: e.textContent,
        });
    };
})();
