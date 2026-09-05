// Lot 3, item 3 (3.8) — LE RÉPERTOIRE FRÈRE QUI DISPARAÎT.
//
// Injectée avant tout script de la page, par le pilote partagé
// `journaux-lot3-pont-sauvegarde/instrument/pilote-item1.mjs`
// (`--injection=…`), réutilisé PAR PARAMÈTRE et jamais par copie.
//
// 🔴 CE QU'ELLE SUBSTITUE : `showDirectoryPicker()`, et rien d'autre. Tout
// l'aval — `choisirDossier`, `creerEcrivain`, `creerMutateur`, le canal, le
// pont — est le produit.
//
// LE JEU REPRODUIT CELUI DE F5 : un répertoire `sous-dossier` portant un
// enfant, un fichier à renommer, et deux fichiers témoins. C'est le frère
// `sous-dossier` qui disparaissait du listage de la VM après un renommage,
// **alors qu'il existait toujours dans OPFS** — et c'est exactement ce que
// `__item3Relire` va rechercher.
(() => {
  const EST_HUB = location.pathname === '/' || location.pathname.endsWith('/hub.html');
  window.__item1 = { etapes: [], est_hub: EST_HUB, chemin: location.pathname };
  const noter = (m) => window.__item1.etapes.push({ t: Date.now(), m: String(m).slice(0, 300) });
  noter('injection item3 posée sur ' + location.pathname);
  // 🔴 SEUL LE HUB PEUPLE OPFS : l'injection est posée sur TOUTES les cibles,
  // et une fenêtre d'application qui purgerait OPFS pendant que le pont y
  // écrit ferait lire un défaut du produit là où il n'y en a pas (F2).
  if (!EST_HUB) { noter('pas le hub : OPFS laissé intact'); return; }

  window.__item3Preparer = async () => {
    const racine = await navigator.storage.getDirectory();
    const d = await racine.getDirectoryHandle('Mes documents', { create: true });
    for await (const nom of d.keys()) await d.removeEntry(nom, { recursive: true });
    const ecrire = async (dossier, nom, texte) => {
      const f = await dossier.getFileHandle(nom, { create: true });
      const w = await f.createWritable();
      await w.write(texte);
      await w.close();
    };
    // Le FRÈRE, et son enfant : c'est lui qu'on cherchera après le renommage.
    const sous = await d.getDirectoryHandle('sous-dossier', { create: true });
    await ecrire(sous, 'autre.txt', 'enfant du frere\n');
    await ecrire(d, 'a-renommer.txt', 'ce fichier va etre renomme dans la VM\n');
    await ecrire(d, 'temoin-1.txt', 'temoin qui ne bouge pas\n');
    await ecrire(d, 'temoin-2.txt', 'second temoin qui ne bouge pas\n');
    window.__item3Dossier = d;
    noter('OPFS préparé : sous-dossier/, a-renommer.txt, temoin-1.txt, temoin-2.txt');
    return await window.__item3Relire();
  };

  window.showDirectoryPicker = async () => {
    if (!window.__item3Dossier) await window.__item3Preparer();
    noter('showDirectoryPicker substitué : poignée OPFS rendue');
    return window.__item3Dossier;
  };

  /// 🔴 LE MAILLON QUI FAIT AUTORITÉ : ce que le POSTE LOCAL contient
  /// réellement. Le défaut de F5 est précisément que la VM cesse de VOIR un
  /// répertoire qui, ici, N'A PAS BOUGÉ. Sans ce relevé, « il a disparu du
  /// listage » serait indiscernable de « il a été supprimé ».
  window.__item3Relire = async () => {
    const d = window.__item3Dossier
      ?? await (await navigator.storage.getDirectory())
           .getDirectoryHandle('Mes documents', { create: true });
    const sortie = [];
    for await (const [nom, p] of d.entries()) {
      if (p.kind === 'directory') {
        const enfants = [];
        for await (const n of p.keys()) enfants.push(n);
        sortie.push({ nom, kind: 'directory', enfants: enfants.sort() });
      } else {
        const f = await p.getFile();
        sortie.push({ nom, kind: 'file', taille: f.size });
      }
    }
    return sortie.sort((a, b) => a.nom.localeCompare(b.nom));
  };
})()
