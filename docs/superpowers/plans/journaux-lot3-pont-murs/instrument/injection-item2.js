// Batch 3, item 2 (3.6) — THE BRIDGE'S THREE WALLS, RE-LOCATED.
//
// Injected by the bridge's shared driver (`--injection=…`), reused BY
// PARAMETER and never copied.
//
// 🔴 WHAT IT SUBSTITUTES: `showDirectoryPicker()`, and nothing else.
//
// THE THREE LADDERS, AND WHY EACH CARRIES A CONTROL RUNG:
//   - SIZE: 4 KiB is the POSITIVE control — F4 notes that "a setup where
//     even 10 entries would fail would measure a failure, not a wall". If the
//     smallest rung fails, there is no wall to measure, there is a
//     failure. 192 KiB brackets the wall F4 put at 128 KiB.
//   - ENTRIES: 100 is the POSITIVE control, 3,200 the rung F4 gave
//     as FAILING — it is the one that, if it succeeds, says the wall has moved.
//   - The entry files are created EMPTY (`getFileHandle` alone, without
//     `createWritable`): we measure an ENUMERATION RANK, not a volume.
(() => {
  const EST_HUB = location.pathname === '/' || location.pathname.endsWith('/hub.html');
  window.__item1 = { etapes: [], est_hub: EST_HUB, chemin: location.pathname };
  const noter = (m) => window.__item1.etapes.push({ t: Date.now(), m: String(m).slice(0, 300) });
  noter('injection item2 posée sur ' + location.pathname);
  if (!EST_HUB) { noter('pas le hub : OPFS laissé intact'); return; }

  const SIZES_KIB = [4, 16, 32, 64, 128, 192, 256, 512];
  const RANGS = [100, 1000, 2000, 3000, 3150, 3200, 4000];

  window.__item2Preparer = async () => {
    const racine = await navigator.storage.getDirectory();
    const d = await racine.getDirectoryHandle('Mes documents', { create: true });
    for await (const nom of d.keys()) await d.removeEntry(nom, { recursive: true });

    // The SIZE ladder. The content is a repeated pattern, not zeros:
    // a buffer of zeros can be compressed or elided somewhere along the
    // path, and we would then measure something other than what we believe.
    const bloc = new Uint8Array(1024);
    for (let i = 0; i < 1024; i += 1) bloc[i] = 33 + (i % 90);
    const faits = [];
    for (const kio of SIZES_KIB) {
      const f = await d.getFileHandle(`taille-${kio}k.bin`, { create: true });
      const w = await f.createWritable();
      for (let i = 0; i < kio; i += 1) await w.write(bloc);
      await w.close();
      faits.push({ nom: `taille-${kio}k.bin`, octets: kio * 1024 });
    }

    // 🔴 THE SUSTAINED THROUGHPUT DATA SET: TWENTY DISTINCT files of 128 KiB.
    // The first wording reread THE SAME file in a loop and returned
    // 1,312,669 KiB/s — forty thousand times the bridge's throughput. It measured
    // the WINDOWS FILE CACHE, not the crossing. Distinct
    // files, read once each, force one round trip per read.
    // 20 x 128 KiB = 2.5 MiB, i.e. ~80 s at 31 KiB/s: the step then exceeds
    // the census period (10 s) several times, as required.
    const debit = await d.getDirectoryHandle('debit', { create: true });
    for (let i = 0; i < 20; i += 1) {
      const f = await debit.getFileHandle(`d${String(i).padStart(2, '0')}.bin`, { create: true });
      const w = await f.createWritable();
      for (let j = 0; j < 128; j += 1) await w.write(bloc);
      await w.close();
    }

    // The ENTRIES ladder, in subdirectories.
    const rangs = [];
    for (const n of RANGS) {
      const sub = await d.getDirectoryHandle(`rang-${n}`, { create: true });
      for (let i = 0; i < n; i += 1) {
        await sub.getFileHandle(`e${String(i).padStart(5, '0')}.txt`, { create: true });
      }
      let compte = 0;
      for await (const _ of sub.keys()) compte += 1;
      rangs.push({ dossier: `rang-${n}`, demande: n, cree: compte });
      noter(`rang-${n} : ${compte} entrées créées`);
    }
    window.__item2Dossier = d;
    return { sizes: faits, rangs };
  };

  window.showDirectoryPicker = async () => {
    if (!window.__item2Dossier) await window.__item2Preparer();
    return window.__item2Dossier;
  };

  /// What the LOCAL MACHINE really contains — the reference against which
  /// any deviation of the VM is read.
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
