// Injecte AVANT tout script de la page (Page.addScriptToEvaluateOnNewDocument).
//
// Trois choses, et rien de plus :
//   1. les jetons et le prefixe dans `localStorage` ;
//   2. une arborescence OPFS peuplee depuis le jeu de donnees de l'hote ;
//   3. `window.showDirectoryPicker` surcharge pour rendre cette arborescence.
//
// 🔴 LA POIGNEE RENDUE EST UNE VRAIE `FileSystemDirectoryHandle`. Ce n'est pas
// un faux objet : OPFS rend la meme classe que le selecteur, avec les memes
// methodes et de vrais `File`. Voir l'en-tete de `commun-f1.mjs` pour ce que
// cela couvre et ce que cela ne couvre pas.
//
// Les valeurs entre doubles tirets bas sont substituees par le pilote.
//
// ⚠️ LE PILOTE SUBSTITUE PAR `replaceAll`, ET CE COMMENTAIRE N'EN NOMME PLUS
// AUCUNE. Une premiere version citait les marqueurs en toutes lettres ICI, et
// `String.replace` -- qui ne remplace que la PREMIERE occurrence -- substituait
// le COMMENTAIRE en laissant le vrai marqueur intact. La page stockait alors le
// marqueur litteral comme jeton : non vide, donc pas de redirection vers
// `connexion.html`, et le seul symptome etait un `jeton refuse (forme)` dans le
// journal du service. Le controle ne pouvait pas voir sa propre panne.
(() => {
    try {
        localStorage.setItem('guac.jeton.acces', '__JETON_ACCES__');
        localStorage.setItem('guac.jeton.rafraichissement', '__JETON_RAFRAICHISSEMENT__');
        localStorage.setItem('guac.prefixe', '__PREFIXE__');
    } catch (e) { /* page sans localStorage */ }

    // 🔴 CAPTURE DE LA `RTCPeerConnection`, pour le critere 4.
    // Il n'existe aucun autre moyen d'atteindre `getStats()` depuis le
    // pilote : ni `main.ts` ni `shell-page.ts` n'exposent leur connexion sur
    // `window`. Technique reprise TELLE QUELLE de
    // `journaux-multifenetres-d11/instrument/pilote-cout-d11.mjs:81-83`.
    //
    // ⚠️ Cette injection doit atteindre les fenetres ouvertes par
    // `window.open` : `Page.addScriptToEvaluateOnNewDocument` NE COURT PAS sur
    // elles (piege mesure au sous-bloc D5). C'est pourquoi le pilote passe par
    // `Target.setAutoAttach` avec `waitForDebuggerOnStart`, et non par une
    // simple cible page.
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
    } catch (e) { /* rien a faire : le critere 4 le dira */ }

    const BASE = '__BASE_JEU__';
    // Le journal du pilote : la page y ecrit, le pilote le relit. Il est
    // BORNE (200 entrees) -- un tampon non borne est le defaut d'instrument
    // que D8 a laisse ouvert sur `window.__pleinEcran`.
    window.__f1 = { etapes: [], erreurs: [] };
    const noter = (m) => {
        window.__f1.etapes.push({ t: Date.now(), m: String(m).slice(0, 300) });
        if (window.__f1.etapes.length > 200) window.__f1.etapes.shift();
    };

    // La population OPFS est lancee TOUT DE SUITE : elle doit etre finie quand
    // l'utilisateur (le pilote) clique. Le picker surcharge l'attend de toute
    // facon, donc un clic precoce ne casse rien -- il attend.
    const pret = (async () => {
        const racineOpfs = await navigator.storage.getDirectory();
        // Un sous-repertoire NOMME : la racine OPFS a un `name` vide, et
        // `choisirDossier` rend `poignee.name` a la page, qui l'affiche.
        const dossier = await racineOpfs.getDirectoryHandle('Mes documents', { create: true });

        // Purge : OPFS persiste dans le profil. Sans cela, une seconde
        // execution lirait le jeu de la premiere et un fichier retire du jeu
        // survivrait -- un controle qui ne pourrait pas voir la difference.
        for await (const nom of dossier.keys()) {
            await dossier.removeEntry(nom, { recursive: true });
        }

        const liste = await (await fetch(BASE + '/liste')).json();
        // Les repertoires d'abord, pour que les fichiers aient leur parent.
        for (const e of liste.filter((x) => x.type === 'directory')) {
            const parts = e.chemin.split('/');
            let ici = dossier;
            for (const p of parts) ici = await ici.getDirectoryHandle(p, { create: true });
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
        // 🔴 LE CONDENSAT DE CE QUE LA PAGE TIENT REELLEMENT.
        // Sans lui, un condensat faux cote VM serait inattribuable : on ne
        // saurait pas si le pont a mal transporte les octets, ou si la page
        // n'avait deja pas les bons. Il se compare a `sha256-origine.txt`,
        // calcule sur le disque de l'hote : trois points sur la meme chaine.
        try {
            const gh = await dossier.getFileHandle('gros.bin');
            const f = await gh.getFile();
            const d = await crypto.subtle.digest('SHA-256', await f.arrayBuffer());
            window.__f1.sha_opfs = Array.from(new Uint8Array(d))
                .map((b) => b.toString(16).padStart(2, '0')).join('');
            window.__f1.taille_opfs = f.size;
            noter('sha256 OPFS de gros.bin : ' + window.__f1.sha_opfs);
        } catch (e) {
            window.__f1.erreurs.push('condensat OPFS : ' + String(e).slice(0, 200));
        }
        noter('OPFS peuple : ' + liste.length + ' entrees');
        window.__f1.peuple = liste.length;
        return dossier;
    })().catch((e) => {
        window.__f1.erreurs.push('peuplement OPFS : ' + String(e).slice(0, 300));
        throw e;
    });

    // 🔴 LA SURCHARGE. `choisirDossier()` lit `globalThis.showDirectoryPicker`
    // A L'APPEL, jamais au chargement du module : la poser ici suffit, et tout
    // le code produit tourne inchange derriere.
    window.showDirectoryPicker = async (options) => {
        noter('showDirectoryPicker appele, mode=' + (options && options.mode));
        const d = await pret;
        noter('poignee OPFS rendue, name=' + d.name + ', kind=' + d.kind);
        return d;
    };
})();
