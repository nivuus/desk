// F4 — le GABARIT, et rien d'autre.
//
// 🔴 CE FICHIER EST CONCATENE APRES `injection-f2.js`, JAMAIS A LA PLACE.
// Le pilote lit les DEUX fichiers depuis leurs repertoires d'origine et les
// joint : « une copie eprouverait la copie, pas l'instrument » (F3). Tout ce
// que F2 pose — le jeton, `showDirectoryPicker`, `__relire`, `__arbre`,
// `__compteur`, la capture de `RTCPeerConnection` — vaut donc ici sans une
// ligne recopiee.
//
// 🔴 LE GABARIT EST PEUPLE A LA DEMANDE DU PILOTE, PAS AU CHARGEMENT.
// Ecrire 100 Mio et 10 000 entrees a chaque navigation couterait des minutes a
// chaque execution, et une mesure lancee sur un gabarit A DEMI ECRIT rendrait
// un chiffre qui ne veut rien dire. Le pilote appelle, puis attend le FAIT —
// `__compteEntrees` — jamais une duree.
//
// ⚠️ LE GARDE `EST_SHELL` DE F2 EST CONSERVE PAR CONSTRUCTION : ce fichier
// n'agit que sur appel du pilote, et le pilote n'appelle que la session de la
// page-shell. En M1 il n'y a AUCUNE fenetre d'application (plan §0.7), donc
// rien a garder — et le garde reste, parce que M2 en a.
//
// ⚠️ LE PILOTE SUBSTITUE PAR `replaceAll`, ET CE COMMENTAIRE N'EN NOMME AUCUN
// MARQUEUR.
(() => {
    window.__f4 = { notes: [], erreurs: [] };
    const noter = (m) => {
        window.__f4.notes.push({ t: Date.now(), m: String(m).slice(0, 300) });
        if (window.__f4.notes.length > 400) window.__f4.notes.shift();
    };

    const racine = async () => {
        const r = await navigator.storage.getDirectory();
        return r.getDirectoryHandle('Mes documents', { create: true });
    };

    /** Descend (et cree) un chemin `a/b/c` sous « Mes documents ». */
    const dossier = async (chemin) => {
        let ici = await racine();
        for (const p of String(chemin).split('/').filter((s) => s.length > 0)) {
            ici = await ici.getDirectoryHandle(p, { create: true });
        }
        return ici;
    };

    // ⚠️ UN NOM DE **DIX-HUIT** CARACTERES, ET C'EST UNE CONTRAINTE DE MESURE,
    // PAS UN GOUT. Le §0.1 du plan calcule le poids d'une entree sur le fil
    // (85 o) avec des noms de cette longueur ; un jeu de noms plus longs
    // deplacerait le mur du listage vers le BAS, un jeu plus court vers le
    // haut, et le rang mesure ne serait plus celui que le calcul predit.
    //   e n t r e e - 0 0 0 0 0 - f . t x t   =  18
    const nomEntree = (i) => 'entree-' + String(i).padStart(5, '0') + '-f.txt';

    /** Contenu pseudo-aleatoire de GRAINE FIXE : deux executions ecrivent les
     *  memes octets, donc le meme condensat, donc une comparaison possible. */
    const octets = (taille, graine) => {
        const u = new Uint8Array(taille);
        let x = (graine >>> 0) || 1;
        for (let i = 0; i < taille; i += 1) {
            x ^= x << 13; x >>>= 0;
            x ^= x >> 17;
            x ^= x << 5; x >>>= 0;
            u[i] = x & 0xff;
        }
        return u;
    };

    /**
     * Peuple `listage/<N>/` avec N fichiers de ZERO octet.
     *
     * ⚠️ Zero octet A DESSEIN : on mesure l'ENUMERATION, pas l'hydratation. Un
     * jeu de fichiers non vides ferait que le premier `Get-ChildItem` de
     * l'Explorateur declenche aussi des lectures, et les deux se confondraient.
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
            window.__f4.erreurs.push('gabaritListage ' + n + ' : ' + String(e).slice(0, 300));
            return JSON.stringify({ demande: n, erreur: String(e).slice(0, 300) });
        }
    };

    /** Peuple `<sousDossier>/<nom>` de `taille` octets. */
    window.__gabaritFichier = async (sousDossier, nom, taille, graine) => {
        try {
            const d = await dossier(sousDossier);
            const fh = await d.getFileHandle(nom, { create: true });
            const w = await fh.createWritable();
            // Par tranches d'un Mio : une Uint8Array de 100 Mio d'un coup passe,
            // mais la generer octet par octet en une fois fige l'onglet.
            const TRANCHE = 1 << 20;
            let ecrit = 0;
            let g = graine;
            while (ecrit < taille) {
                const n = Math.min(TRANCHE, taille - ecrit);
                await w.write(octets(n, g));
                ecrit += n;
                g = (g * 1664525 + 1013904223) >>> 0;
            }
            await w.close();
            const f = await (await d.getFileHandle(nom)).getFile();
            noter('gabarit ' + sousDossier + '/' + nom + ' : ' + f.size + ' octets');
            return JSON.stringify({ chemin: sousDossier + '/' + nom, taille: f.size });
        } catch (e) {
            window.__f4.erreurs.push('gabaritFichier ' + nom + ' : ' + String(e).slice(0, 300));
            return JSON.stringify({ chemin: sousDossier + '/' + nom, erreur: String(e).slice(0, 300) });
        }
    };

    /** Le FAIT que le pilote attend : combien d'entrees `chemin` porte VRAIMENT. */
    window.__compteEntrees = async (chemin) => {
        try {
            const d = await dossier(chemin);
            let n = 0;
            for await (const _ of d.keys()) n += 1;
            return JSON.stringify({ chemin, entrees: n });
        } catch (e) {
            return JSON.stringify({ chemin, erreur: String(e).slice(0, 200) });
        }
    };

    // 🔴 LA NEUTRALISATION DE `move` EST L'INSTRUMENT, PAS LE PRODUIT.
    //
    // Le repli de renommage par copie de F3 (`client/src/fichiers/copie.ts`)
    // N'A JAMAIS COURU : `mutation.ts` teste `typeof poignee.move === 'function'`
    // A L'APPEL, et `move()` existe sur un fichier OPFS (sonde S2 de F3). Le
    // seul moyen de mesurer le cout du repli est donc de retirer `move` — ce
    // qui est une mesure FORCEE, et le rapport le dit. Le bras TEMOIN est le
    // meme geste SANS cet appel.
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

    /** Ce que la couche d'injection a note, et ce qui lui a echappe. */
    window.__f4Etat = () => JSON.stringify({
        notes: window.__f4.notes.slice(-40),
        erreurs: window.__f4.erreurs,
        move: typeof FileSystemFileHandle.prototype.move,
    });
})();
