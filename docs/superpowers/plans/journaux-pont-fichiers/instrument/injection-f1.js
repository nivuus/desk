// Injected BEFORE any script of the page (Page.addScriptToEvaluateOnNewDocument).
//
// Three things, and nothing more:
//   1. the tokens and the prefix in `localStorage`;
//   2. an OPFS tree populated from the host's data set;
//   3. `window.showDirectoryPicker` overridden to return that tree.
//
// 🔴 THE RETURNED HANDLE IS A REAL `FileSystemDirectoryHandle`. It is not
// a fake object: OPFS returns the same class as the picker, with the same
// methods and real `File`s. See the header of `commun-f1.mjs` for what
// that covers and what it does not.
//
// The values between double underscores are substituted by the driver.
//
// ⚠️ THE DRIVER SUBSTITUTES THROUGH `replaceAll`, AND THIS COMMENT NO LONGER NAMES
// ANY OF THEM. A first version quoted the markers in full HERE, and
// `String.replace` -- which only replaces the FIRST occurrence -- substituted
// the COMMENT while leaving the real marker intact. The page then stored the
// literal marker as a token: not empty, hence no redirect to
// `connexion.html`, and the only symptom was a `token refused (shape)` in the
// service's log. The check could not see its own failure.
(() => {
    try {
        localStorage.setItem('guac.jeton.acces', '__JETON_ACCES__');
        localStorage.setItem('guac.jeton.rafraichissement', '__JETON_RAFRAICHISSEMENT__');
        localStorage.setItem('guac.prefixe', '__PREFIXE__');
    } catch (e) { /* page sans localStorage */ }

    // 🔴 CAPTURE OF THE `RTCPeerConnection`, for criterion 4.
    // There is no other way to reach `getStats()` from the
    // driver: neither `main.ts` nor `shell-page.ts` exposes its connection on
    // `window`. Technique taken AS IS from
    // `journaux-multifenetres-d11/instrument/pilote-cout-d11.mjs:81-83`.
    //
    // ⚠️ This injection must reach the windows opened by
    // `window.open`: `Page.addScriptToEvaluateOnNewDocument` DOES NOT RUN on
    // them (a trap measured in sub-block D5). That is why the driver goes through
    // `Target.setAutoAttach` with `waitForDebuggerOnStart`, and not through a
    // simple page target.
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
    } catch (e) { /* nothing to do: criterion 4 will say */ }

    const BASE = '__BASE_JEU__';
    // The driver's log: the page writes into it, the driver rereads it. It is
    // BOUNDED (200 entries) -- an unbounded buffer is the instrument defect
    // D8 left open on `window.__pleinEcran`.
    window.__f1 = { etapes: [], errors: [] };
    const noter = (m) => {
        window.__f1.etapes.push({ t: Date.now(), m: String(m).slice(0, 300) });
        if (window.__f1.etapes.length > 200) window.__f1.etapes.shift();
    };

    // The OPFS population is launched RIGHT AWAY: it must be finished when
    // the user (the driver) clicks. The overridden picker waits for it
    // anyway, so an early click breaks nothing -- it waits.
    const pret = (async () => {
        const racineOpfs = await navigator.storage.getDirectory();
        // A NAMED subdirectory: the OPFS root has an empty `name`, and
        // `choisirDossier` returns `poignee.name` to the page, which displays it.
        const dossier = await racineOpfs.getDirectoryHandle('Mes documents', { create: true });

        // Purge: OPFS persists in the profile. Without it, a second
        // run would read the first one's data set and a file removed from the set
        // would survive -- a check that could not see the difference.
        for await (const nom of dossier.keys()) {
            await dossier.removeEntry(nom, { recursive: true });
        }

        const list = await (await fetch(BASE + '/liste')).json();
        // Directories first, so that files have their parent.
        for (const e of list.filter((x) => x.type === 'directory')) {
            const parts = e.chemin.split('/');
            let ici = dossier;
            for (const p of parts) ici = await ici.getDirectoryHandle(p, { create: true });
        }
        for (const e of list.filter((x) => x.type === 'file')) {
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
        // 🔴 THE DIGEST OF WHAT THE PAGE REALLY HOLDS.
        // Without it, a wrong digest on the VM side would be unattributable: one would not
        // know whether the bridge carried the bytes badly, or whether the page
        // did not have the right ones in the first place. It is compared with `sha256-origine.txt`,
        // computed on the host's disk: three points on the same chain.
        try {
            const gh = await dossier.getFileHandle('gros.bin');
            const f = await gh.getFile();
            const d = await crypto.subtle.digest('SHA-256', await f.arrayBuffer());
            window.__f1.sha_opfs = Array.from(new Uint8Array(d))
                .map((b) => b.toString(16).padStart(2, '0')).join('');
            window.__f1.opfs_size = f.size;
            noter('sha256 OPFS de gros.bin : ' + window.__f1.sha_opfs);
        } catch (e) {
            window.__f1.errors.push('condensat OPFS : ' + String(e).slice(0, 200));
        }
        noter('OPFS peuple : ' + list.length + ' entrees');
        window.__f1.peuple = list.length;
        return dossier;
    })().catch((e) => {
        window.__f1.errors.push('peuplement OPFS : ' + String(e).slice(0, 300));
        throw e;
    });

    // 🔴 THE OVERRIDE. `choisirDossier()` reads `globalThis.showDirectoryPicker`
    // AT CALL TIME, never at module load: setting it here is enough, and all
    // the product code runs unchanged behind it.
    window.showDirectoryPicker = async (options) => {
        noter('showDirectoryPicker appele, mode=' + (options && options.mode));
        const d = await pret;
        noter('poignee OPFS rendue, name=' + d.name + ', kind=' + d.kind);
        return d;
    };
})();
