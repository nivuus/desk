// Sonde de latence du lot 3 — injectée dans la page de session par CDP.
//
// 🔴 ELLE N'EST PAS UN CHANGEMENT DE PRODUIT : le client n'appelle pas
// requestVideoFrameCallback, et cette sonde ne l'y ajoute pas — elle
// s'attache depuis l'extérieur, le temps de la mesure.
//
// L'agent annonce l'instant de capture au pair par le sender report RTCP
// (agent/src/transport/piste_video.rs, tenu par le test
// `write_frame_annonce_l_instant_de_capture_au_pair_via_le_sender_report_rtcp`) ;
// le navigateur le rend dans metadata.captureTime. La soustraction EST la
// latence.
//
// ⚠️ LES DEUX HORODATAGES SONT RENDUS DANS L'HORLOGE DU DOCUMENT
// (`DOMHighResTimeStamp`, même origine que `performance.now()`), et c'est
// PRÉCISÉMENT ce qui rend la soustraction licite : `captureTime` n'est pas
// l'horloge de la VM, c'est l'instant de capture RAMENÉ dans l'horloge du
// document par la corrélation RTCP. Aucune synchronisation d'horloges entre
// l'hôte et l'invité n'est donc requise, et aucune n'est supposée.
//
// ⚠️ POURQUOI UN COMPTEUR DE TRAMES À CÔTÉ DES ÉCHANTILLONS. Un tableau vide
// a DEUX causes qu'il ne distingue pas : aucune trame n'arrive, ou des trames
// arrivent sans `captureTime`. Les confondre ferait conclure « pas de flux »
// là où l'on tient « pas de corrélation d'horloge », qui est un TOUT AUTRE
// défaut. Le compteur les sépare.
(() => {
  const echantillons = [];
  const compteurs = { trames: 0, sans_capture: 0, videos: 0 };
  const attacher = (video) => {
    if (typeof video.requestVideoFrameCallback !== 'function') return;
    compteurs.videos += 1;
    const tour = (maintenant, metadata) => {
      compteurs.trames += 1;
      // captureTime est ABSENT tant que l'horloge distante n'est pas connue :
      // un échantillon sans lui n'est pas un zéro, il n'existe pas.
      if (typeof metadata.captureTime === 'number') {
        echantillons.push({
          latence_ms: metadata.presentationTime - metadata.captureTime,
          traitement_ms: metadata.processingDuration === undefined
            ? null : metadata.processingDuration * 1000,
          rtp: metadata.rtpTimestamp,
          horodatage: maintenant,
        });
      } else {
        compteurs.sans_capture += 1;
      }
      video.requestVideoFrameCallback(tour);
    };
    video.requestVideoFrameCallback(tour);
  };
  document.querySelectorAll('video').forEach(attacher);
  window.__latenceLot3 = () => echantillons;
  window.__latenceLot3Compteurs = () => compteurs;
  return { videos: compteurs.videos };
})()
