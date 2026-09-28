// F4 — the TEMPLATE, and nothing else.
//
// 🔴 THIS FILE IS CONCATENATED AFTER `injection-f2.js`, NEVER IN ITS PLACE.
// The driver reads BOTH files from their original directories and
// joins them: "a copy would test the copy, not the instrument" (F3). Everything
// F2 sets — the token, `showDirectoryPicker`, `__relire`, `__arbre`,
// `__compteur`, the capture of `RTCPeerConnection` — therefore holds here without a
// copied line.
//
// 🔴 THE TEMPLATE IS POPULATED ON THE DRIVER'S REQUEST, NOT AT LOAD.
// Writing 100 MiB and 10,000 entries at each navigation would cost minutes at
// each run, and a measurement launched on a HALF-WRITTEN template would return
// a figure that means nothing. The driver calls, then waits for the FACT —
// `__compteEntrees` — never a duration.
//
// ⚠️ F2'S `EST_SHELL` GUARD IS KEPT BY CONSTRUCTION: this file
// only acts on the driver's call, and the driver only calls the shell page's
// session. In M1 there is NO application window (plan §0.7), so
// nothing to guard — and the guard stays, because M2 has some.
//
// ⚠️ THE DRIVER SUBSTITUTES THROUGH `replaceAll`, AND THIS COMMENT NAMES NONE OF ITS
// MARKERS.
(() => {
    window.__f4 = { notes: [], errors: [] };
    const noter = (m) => {
        window.__f4.notes.push({ t: Date.now(), m: String(m).slice(0, 300) });
        if (window.__f4.notes.length > 400) window.__f4.notes.shift();
    };

    const racine = async () => {
        const r = await navigator.storage.getDirectory();
        return r.getDirectoryHandle('Mes documents', { create: true });
    };

    /** Walks down (and creates) a path `a/b/c` under the fixture's documents folder. */
    const dossier = async (chemin) => {
        let ici = await racine();
        for (const p of String(chemin).split('/').filter((s) => s.length > 0)) {
            ici = await ici.getDirectoryHandle(p, { create: true });
        }
        return ici;
    };

    // ⚠️ A NAME OF **EIGHTEEN** CHARACTERS, AND IT IS A MEASUREMENT CONSTRAINT,
    // NOT A MATTER OF TASTE. §0.1 of the plan computes an entry's weight on the wire
    // (85 B) with names of this length; a set of longer names
    // would move the listing wall DOWN, a shorter set up,
    // and the measured rank would no longer be the one the computation predicts.
    //   e n t r e e - 0 0 0 0 0 - f . t x t   =  18
    const nomEntree = (i) => 'entree-' + String(i).padStart(5, '0') + '-f.txt';

    /** Pseudo-random content with a FIXED SEED: two runs write the
     *  same bytes, hence the same digest, hence a possible comparison. */
    const octets = (size, graine) => {
        const u = new Uint8Array(size);
        let x = (graine >>> 0) || 1;
        for (let i = 0; i < size; i += 1) {
            x ^= x << 13; x >>>= 0;
            x ^= x >> 17;
            x ^= x << 5; x >>>= 0;
            u[i] = x & 0xff;
        }
        return u;
    };

    /**
     * Populates `listage/<N>/` with N files of ZERO bytes.
     *
     * ⚠️ Zero bytes ON PURPOSE: we measure ENUMERATION, not hydration. A
     * set of non-empty files would make Explorer's first `Get-ChildItem`
     * also trigger reads, and the two would get mixed up.
     */
    window.__gabaritListage = async (n) => {
        try {
            const d = await dossier('listage/' + n);
            for await (const nom of d.keys()) await d.removeEntry(nom, { recursive: true });
            for (let i = 0; i < n; i += 1) await d.getFileHandle(nomEntree(i), { create: true });
            let vus = 0;
            for await (const _ of d.keys()) vus += 1;
            noter('gabarit listage/' + n + ' : ' + vus + ' entrees');
            return JSON.stringify({ demande: n, presentes: vus });
        } catch (e) {
            window.__f4.errors.push('gabaritListage ' + n + ' : ' + String(e).slice(0, 300));
            return JSON.stringify({ demande: n, error: String(e).slice(0, 300) });
        }
    };

    /** Populates `<subfolder>/<nom>` with `size` bytes. */
    window.__fileTemplate = async (subfolder, nom, size, graine) => {
        try {
            const d = await dossier(subfolder);
            const fh = await d.getFileHandle(nom, { create: true });
            const w = await fh.createWritable();
            // In slices of one MiB: a 100 MiB Uint8Array in one go works,
            // but generating it byte by byte at once freezes the tab.
            const TRANCHE = 1 << 20;
            let written = 0;
            let g = graine;
            while (written < size) {
                const n = Math.min(TRANCHE, size - written);
                await w.write(octets(n, g));
                written += n;
                g = (g * 1664525 + 1013904223) >>> 0;
            }
            await w.close();
            const f = await (await d.getFileHandle(nom)).getFile();
            noter('gabarit ' + subfolder + '/' + nom + ' : ' + f.size + ' octets');
            return JSON.stringify({ chemin: subfolder + '/' + nom, taille: f.size });
        } catch (e) {
            window.__f4.errors.push('gabaritFichier ' + nom + ' : ' + String(e).slice(0, 300));
            return JSON.stringify({ chemin: subfolder + '/' + nom, error: String(e).slice(0, 300) });
        }
    };

    /** The FACT the driver waits for: how many entries `chemin` REALLY carries. */
    window.__compteEntrees = async (chemin) => {
        try {
            const d = await dossier(chemin);
            let n = 0;
            for await (const _ of d.keys()) n += 1;
            return JSON.stringify({ chemin, entrees: n });
        } catch (e) {
            return JSON.stringify({ chemin, error: String(e).slice(0, 200) });
        }
    };

    // 🔴 NEUTRALISING `move` IS THE INSTRUMENT, NOT THE PRODUCT.
    //
    // F3's rename-by-copy fallback (`client/src/fichiers/copie.ts`)
    // HAS NEVER RUN: `mutation.ts` tests `typeof poignee.move === 'function'`
    // AT CALL TIME, and `move()` exists on an OPFS file (F3's probe S2). The
    // only way to measure the fallback's cost is therefore to remove `move` — which
    // is a FORCED measurement, and the report says so. The CONTROL arm is the
    // same gesture WITHOUT this call.
    window.__neutraliserMove = () => {
        const sauve = [];
        for (const P of [FileSystemFileHandle, FileSystemDirectoryHandle]) {
            if (P && P.prototype && typeof P.prototype.move === 'function') {
                sauve.push(P.name);
                delete P.prototype.move;
            }
        }
        noter('move neutralise sur : ' + sauve.join(','));
        return JSON.stringify({ neutralise: sauve, restant: typeof FileSystemFileHandle.prototype.move });
    };

    /** What the injection layer noted, and what escaped it. */
    window.__f4Etat = () => JSON.stringify({
        notes: window.__f4.notes.slice(-40),
        errors: window.__f4.errors,
        move: typeof FileSystemFileHandle.prototype.move,
    });
})();
