// Lot 3, item 2 (3.6) — LES TROIS MURS DU PONT, RE-SITUÉS.
//
// Injectée par le pilote partagé du pont (`--injection=…`), réutilisé PAR
// PARAMÈTRE et jamais copié.
//
// 🔴 CE QU'ELLE SUBSTITUE : `showDirectoryPicker()`, et rien d'autre.
//
// LES TROIS ÉCHELLES, ET POURQUOI CHACUNE PORTE UN BARREAU DE TÉMOIN :
//   - TAILLE : 4 Kio est le témoin POSITIF — F4 relève que « un montage où
//     même 10 entrées échoueraient mesurerait une panne, pas un mur ». Si le
//     plus petit barreau échoue, il n'y a pas de mur à mesurer, il y a une
//     panne. 192 Kio encadre le mur que F4 donnait à 128 Kio.
//   - ENTRÉES : 100 est le témoin POSITIF, 3 200 le barreau que F4 donnait
//     pour ÉCHOUANT — c'est lui qui, s'il aboutit, dit que le mur a bougé.
//   - Les fichiers d'entrées sont créés VIDES (`getFileHandle` seul, sans
//     `createWritable`) : on mesure un RANG D'ÉNUMÉRATION, pas un volume.
(() => {
  const EST_HUB = location.pathname === '/' || location.pathname.endsWith('/hub.html');
  window.__item1 = { etapes: [], est_hub: EST_HUB, chemin: location.pathname };
  const noter = (m) => window.__item1.etapes.push({ t: Date.now(), m: String(m).slice(0, 300) });
  noter('injection item2 posée sur ' + location.pathname);
  if (!EST_HUB) { noter('pas le hub : OPFS laissé intact'); return; }

  const TAILLES_KIO = [4, 16, 32, 64, 128, 192, 256, 512];
  const RANGS = [100, 1000, 2000, 3000, 3150, 3200, 4000];

  window.__item2Preparer = async () => {
    const racine = await navigator.storage.getDirectory();
    const d = await racine.getDirectoryHandle('Mes documents', { create: true });
    for await (const nom of d.keys()) await d.removeEntry(nom, { recursive: true });

    // L'échelle de TAILLE. Le contenu est un motif répété, pas des zéros :
    // un tampon de zéros peut être compressé ou élidé quelque part sur le
    // chemin, et l'on mesurerait alors autre chose que ce qu'on croit.
    const bloc = new Uint8Array(1024);
    for (let i = 0; i < 1024; i += 1) bloc[i] = 33 + (i % 90);
    const faits = [];
    for (const kio of TAILLES_KIO) {
      const f = await d.getFileHandle(`taille-${kio}k.bin`, { create: true });
      const w = await f.createWritable();
      for (let i = 0; i < kio; i += 1) await w.write(bloc);
      await w.close();
      faits.push({ nom: `taille-${kio}k.bin`, octets: kio * 1024 });
    }

    // 🔴 LE JEU DU DÉBIT SOUTENU : VINGT fichiers DISTINCTS de 128 Kio.
    // La première rédaction relisait LE MÊME fichier en boucle et rendait
    // 1 312 669 Kio/s — quarante mille fois le débit du pont. Elle mesurait
    // le CACHE DE FICHIERS DE WINDOWS, pas la traversée. Des fichiers
    // distincts, lus une fois chacun, forcent un aller-retour par lecture.
    // 20 x 128 Kio = 2,5 Mio, soit ~80 s a 31 Kio/s : le palier depasse alors
    // plusieurs fois la periode de recensement (10 s), comme exige.
    const debit = await d.getDirectoryHandle('debit', { create: true });
    for (let i = 0; i < 20; i += 1) {
      const f = await debit.getFileHandle(`d${String(i).padStart(2, '0')}.bin`, { create: true });
      const w = await f.createWritable();
      for (let j = 0; j < 128; j += 1) await w.write(bloc);
      await w.close();
    }

    // L'échelle d'ENTRÉES, dans des sous-répertoires.
    const rangs = [];
    for (const n of RANGS) {
      const sous = await d.getDirectoryHandle(`rang-${n}`, { create: true });
      for (let i = 0; i < n; i += 1) {
        await sous.getFileHandle(`e${String(i).padStart(5, '0')}.txt`, { create: true });
      }
      let compte = 0;
      for await (const _ of sous.keys()) compte += 1;
      rangs.push({ dossier: `rang-${n}`, demande: n, cree: compte });
      noter(`rang-${n} : ${compte} entrées créées`);
    }
    window.__item2Dossier = d;
    return { tailles: faits, rangs };
  };

  window.showDirectoryPicker = async () => {
    if (!window.__item2Dossier) await window.__item2Preparer();
    return window.__item2Dossier;
  };

  /// Ce que le POSTE LOCAL contient réellement — la référence contre laquelle
  /// tout écart de la VM se lit.
  window.__item2Relire = async () => {
    const d = window.__item2Dossier
      ?? await (await navigator.storage.getDirectory())
           .getDirectoryHandle('Mes documents', { create: true });
    const sortie = [];
    for await (const [nom, p] of d.entries()) {
      if (p.kind === 'directory') {
        let n = 0;
        for await (const _ of p.keys()) n += 1;
        sortie.push({ nom, kind: 'directory', entrees: n });
      } else {
        sortie.push({ nom, kind: 'file', taille: (await p.getFile()).size });
      }
    }
    return sortie.sort((a, b) => a.nom.localeCompare(b.nom));
  };
})()
