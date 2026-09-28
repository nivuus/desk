// Batch 3, item 3 (3.8) — THE SIBLING DIRECTORY THAT DISAPPEARS.
//
// Injected before any script of the page, by the shared driver
// `journaux-lot3-pont-sauvegarde/instrument/pilote-item1.mjs`
// (`--injection=…`), reused BY PARAMETER and never by copy.
//
// 🔴 WHAT IT SUBSTITUTES: `showDirectoryPicker()`, and nothing else. Everything
// downstream — `choisirDossier`, `creerEcrivain`, `creerMutateur`, the channel, the
// bridge — is the product.
//
// THE DATA SET REPRODUCES F5'S: a `sous-dossier` directory carrying a
// child, a file to rename, and two control files. It is the sibling
// `sous-dossier` that disappeared from the VM's listing after a rename,
// **while it still existed in OPFS** — and that is exactly what
// `__item3Relire` will look for.
(() => {
  const EST_HUB = location.pathname === '/' || location.pathname.endsWith('/hub.html');
  window.__item1 = { etapes: [], est_hub: EST_HUB, chemin: location.pathname };
  const noter = (m) => window.__item1.etapes.push({ t: Date.now(), m: String(m).slice(0, 300) });
  noter('injection item3 posée sur ' + location.pathname);
  // 🔴 ONLY THE HUB POPULATES OPFS: the injection is set on ALL targets,
  // and an application window purging OPFS while the bridge writes there
  // would make one read a product defect where there is none (F2).
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
    // The SIBLING, and its child: it is what we will look for after the rename.
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

  /// 🔴 THE AUTHORITATIVE LINK: what the LOCAL MACHINE really
  /// contains. F5's defect is precisely that the VM stops SEEING a
  /// directory that, here, HAS NOT MOVED. Without this reading, "it disappeared from the
  /// listing" would be indistinguishable from "it was deleted".
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
