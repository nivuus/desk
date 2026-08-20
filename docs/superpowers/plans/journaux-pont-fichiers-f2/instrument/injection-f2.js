// Injecte AVANT tout script de la page (Page.addScriptToEvaluateOnNewDocument).
//
// Repris de `journaux-pont-fichiers/instrument/injection-f1.js`, avec ce que F2
// ajoute : une RELECTURE d'OPFS et son condensat, et la lecture du compteur
// d'ecritures dues.
//
// 🔴 LA POIGNEE RENDUE EST UNE VRAIE `FileSystemDirectoryHandle` (OPFS), donc
// `createWritable()` y est le VRAI, avec son fichier d'echange et sa
// committaison au `close()`. C'est ce qui rend le critere ⑤ mesurable.
//
// ⚠️ CE QU'OPFS NE COUVRE PAS, et c'est declare : `showDirectoryPicker()` n'est
// jamais appele, le modele de permission (`queryPermission` /
// `requestPermission`) n'est pas exerce, et le mode `readwrite` que F2 pose
// n'est pas eprouve — OPFS n'a aucun modele de permission.
//
// ⚠️ LE PILOTE SUBSTITUE PAR `replaceAll`, ET CE COMMENTAIRE N'EN NOMME AUCUN
// MARQUEUR : une premiere version de F1 les citait en toutes lettres ici, et la
// substitution frappait le COMMENTAIRE en laissant le vrai marqueur intact.
(() => {
    try {
        localStorage.setItem('guac.jeton.acces', '__JETON_ACCES__');
        localStorage.setItem('guac.jeton.rafraichissement', '__JETON_RAFRAICHISSEMENT__');
        localStorage.setItem('guac.prefixe', '__PREFIXE__');
    } catch (e) { /* page sans localStorage */ }

    // Capture de la `RTCPeerConnection`, pour le critere ⑥ (`framesDecoded`).
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
    } catch (e) { /* le critere ⑥ le dira */ }

    const BASE = '__BASE_JEU__';
    window.__f2 = { etapes: [], erreurs: [] };
    const noter = (m) => {
        window.__f2.etapes.push({ t: Date.now(), m: String(m).slice(0, 300) });
        if (window.__f2.etapes.length > 300) window.__f2.etapes.shift();
    };

    // 🔴 OPFS N'EST PEUPLÉ QUE PAR LA PAGE-SHELL, ET C'EST UN DÉFAUT
    // D'INSTRUMENT PAYÉ SUR PLACE.
    //
    // L'injection est posée par `Target.setAutoAttach`, donc sur TOUTES les
    // pages — y compris les fenêtres d'application ouvertes par
    // `window.open('/?session=…')`. Chacune exécutait alors la PURGE d'OPFS et
    // la repeuplait, **pendant que le pont y écrivait**.
    //
    // ⚠️ **LE SYMPTÔME SE LIT COMME UN DÉFAUT DU PRODUIT** : à l'exécution
    // `arme-2`, `Casse.txt`, `gros-lecture.bin` et `sous-dossier` avaient
    // DISPARU d'OPFS, et `CASSE.TXT` — que la garde de casse refuse en temps
    // normal — s'y trouvait créé. **La garde n'avait pas failli : l'homonyme
    // qu'elle cherche avait été effacé sous elle par une autre page.**
    const EST_SHELL = location.pathname.endsWith('/shell.html');
    const pret = (async () => {
        if (!EST_SHELL) throw new Error('OPFS n est peuple que par la page-shell');
        const racineOpfs = await navigator.storage.getDirectory();
        const dossier = await racineOpfs.getDirectoryHandle('Mes documents', { create: true });
        // Purge : OPFS persiste dans le profil, et une seconde execution lirait
        // le jeu de la premiere.
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

    // 🔴 LA RELECTURE, ET C'EST LE CRITERE ①. Elle lit OPFS — c'est-a-dire ce
    // que le POSTE LOCAL porte reellement — et rend taille et condensat. Un
    // critere « le fichier apparait » serait satisfait par un fichier tronque,
    // par des morceaux dans le desordre et par un dernier morceau manquant :
    // les TROIS sont des defauts que `pont/decoupe.rs` et le fil d'ecriture
    // peuvent reellement produire, et AUCUN ne se voit a l'oeil.
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

    /** L'arborescence complete d'OPFS, pour voir ce qui est arrive et ce qui ne l'est pas. */
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

    // 🔴 LE COMPTEUR SE LIT DANS DES ATTRIBUTS, JAMAIS DANS LE TEXTE.
    // C'est le remede au piege que F1 a paye neuf minutes : deux messages qui
    // partagent une sous-chaine. Et `vues` est CUMULATIF : `dues=0` SEUL est
    // indiscernable d'une MESURE NON PRISE.
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
