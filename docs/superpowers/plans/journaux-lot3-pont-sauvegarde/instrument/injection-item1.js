// Lot 3, item 1 (3.7) — injectée AVANT tout script de la page, par
// `Target.setAutoAttach` + `waitForDebuggerOnStart` au niveau NAVIGATEUR.
//
// 🔴 CE QU'ELLE SUBSTITUE, ET RIEN DE PLUS : `showDirectoryPicker()`. Tout ce
// qui est en aval — `choisirDossier()`, `creerEcrivain`, `creerMutateur`,
// `creerAdaptateur`, le canal, le pont — est le PRODUIT, non modifié. La
// poignée rendue est une VRAIE `FileSystemDirectoryHandle` (OPFS), donc
// `createWritable()` y est le vrai, avec son fichier d'échange et sa
// committaison au `close()`.
//
// ⚠️ CE QU'OPFS NE COUVRE PAS, ET C'EST DÉCLARÉ (limite héritée de F1/F2) :
// `showDirectoryPicker()` n'est jamais réellement appelé, le modèle de
// permission (`queryPermission`/`requestPermission`) n'est pas exercé, et
// l'activation utilisateur transitoire non plus. OPFS n'a aucun modèle de
// permission.
//
// 🔴 SEULE LA PAGE DU HUB PEUPLE OPFS — défaut d'instrument payé par F2 : cette
// injection est posée sur TOUTES les cibles, y compris les fenêtres
// d'application ouvertes par `window.open`. Chacune purgeait alors OPFS et le
// repeuplait PENDANT que le pont y écrivait, et le symptôme se lisait comme un
// défaut du produit (des fichiers « disparus », une garde de casse « en
// panne »). Le garde ci-dessous est ce qui l'empêche.
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

    // Le nom du fichier que l'éditeur va rouvrir et réenregistrer. Il porte un
    // contenu INITIAL connu, pour que « la sauvegarde est arrivée » se
    // distingue de « le fichier était déjà là ».
    const NOM = 'item1-sauvegarde.txt';
    const INITIAL = 'contenu initial pose par le pilote du lot 3\n';

    window.__item1Preparer = async () => {
        const racine = await navigator.storage.getDirectory();
        const dossier = await racine.getDirectoryHandle('Mes documents', { create: true });
        // Purge : OPFS persiste dans le profil, et une seconde exécution lirait
        // le jeu de la première.
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

    // La SUBSTITUTION, et elle seule.
    window.showDirectoryPicker = async () => {
        if (!window.__item1Dossier) await window.__item1Preparer();
        noter('showDirectoryPicker substitué : poignée OPFS rendue');
        return window.__item1Dossier;
    };

    /// Relit la racine locale : le contenu de chaque entrée, et son condensat.
    /// 🔴 C'EST LE CÔTÉ QUI JUGE. Le journal de l'agent dit ce que ProjFS a
    /// notifié ; SEUL ce relevé dit si la sauvegarde est ARRIVÉE.
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
