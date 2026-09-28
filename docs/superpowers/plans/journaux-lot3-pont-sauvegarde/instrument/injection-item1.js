// Batch 3, item 1 (3.7) — injected BEFORE any script of the page, through
// `Target.setAutoAttach` + `waitForDebuggerOnStart` at BROWSER level.
//
// 🔴 WHAT IT SUBSTITUTES, AND NOTHING MORE: `showDirectoryPicker()`. Everything
// downstream — `choisirDossier()`, `creerEcrivain`, `creerMutateur`,
// `creerAdaptateur`, the channel, the bridge — is the PRODUCT, unmodified. The
// returned handle is a REAL `FileSystemDirectoryHandle` (OPFS), so
// `createWritable()` is the real one there, with its swap file and its
// commit on `close()`.
//
// ⚠️ WHAT OPFS DOES NOT COVER, AND IT IS DECLARED (a limit inherited from F1/F2):
// `showDirectoryPicker()` is never really called, the
// permission model (`queryPermission`/`requestPermission`) is not exercised, and
// neither is the transient user activation. OPFS has no permission
// model.
//
// 🔴 ONLY THE HUB PAGE POPULATES OPFS — an instrument defect paid for by F2: this
// injection is set on ALL targets, including the application
// windows opened by `window.open`. Each of them then purged OPFS and
// repopulated it WHILE the bridge was writing there, and the symptom read as a
// product defect (files "gone", a case guard "out of
// order"). The guard below is what prevents it.
(() => {
    const EST_HUB = location.pathname === '/'
        || location.pathname.endsWith('/hub.html')
        || location.pathname.endsWith('/index.html') === false && location.search === '';
    window.__item1 = { etapes: [], erreurs: [], est_hub: EST_HUB, chemin: location.pathname };
    const noter = (m) => {
        window.__item1.etapes.push({ t: Date.now(), m: String(m).slice(0, 400) });
        if (window.__item1.etapes.length > 400) window.__item1.etapes.shift();
    };
    noter('injection posée sur ' + location.pathname + location.search);

    if (!EST_HUB) { noter('pas le hub : OPFS laissé intact'); return; }

    // The name of the file the editor will reopen and save again. It carries a
    // known INITIAL content, so that "the save arrived" can be
    // told apart from "the file was already there".
    const NOM = 'item1-sauvegarde.txt';
    const INITIAL = 'contenu initial pose par le pilote du lot 3\n';

    window.__item1Preparer = async () => {
        const racine = await navigator.storage.getDirectory();
        const dossier = await racine.getDirectoryHandle('Mes documents', { create: true });
        // Purge: OPFS persists in the profile, and a second run would read
        // the first one's data set.
        for await (const nom of dossier.keys()) {
            await dossier.removeEntry(nom, { recursive: true });
        }
        const f = await dossier.getFileHandle(NOM, { create: true });
        const w = await f.createWritable();
        await w.write(INITIAL);
        await w.close();
        window.__item1Dossier = dossier;
        noter('OPFS préparé : ' + NOM + ' (' + INITIAL.length + ' octets)');
        return { nom: NOM, octets: INITIAL.length };
    };

    // The SUBSTITUTION, and it alone.
    window.showDirectoryPicker = async () => {
        if (!window.__item1Dossier) await window.__item1Preparer();
        noter('showDirectoryPicker substitué : poignée OPFS rendue');
        return window.__item1Dossier;
    };

    /// Rereads the local root: the content of each entry, and its digest.
    /// 🔴 IT IS THE SIDE THAT JUDGES. The agent's log says what ProjFS
    /// notified; ONLY this reading says whether the save ARRIVED.
    window.__item1Relire = async () => {
        const dossier = window.__item1Dossier
            ?? await (await navigator.storage.getDirectory())
                .getDirectoryHandle('Mes documents', { create: true });
        const sortie = [];
        for await (const [nom, poignee] of dossier.entries()) {
            if (poignee.kind !== 'file') { sortie.push({ nom, kind: poignee.kind }); continue; }
            const fichier = await poignee.getFile();
            const texte = await fichier.text();
            const octets = new TextEncoder().encode(texte);
            const condensat = await crypto.subtle.digest('SHA-256', octets);
            sortie.push({
                nom,
                taille: fichier.size,
                modifie: fichier.lastModified,
                sha256: [...new Uint8Array(condensat)]
                    .map((o) => o.toString(16).padStart(2, '0')).join(''),
                debut: texte.slice(0, 200),
            });
        }
        return sortie.sort((a, b) => a.nom.localeCompare(b.nom));
    };
})()
