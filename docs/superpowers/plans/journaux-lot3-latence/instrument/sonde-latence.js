// Batch 3 latency probe — injected into the session page through CDP.
//
// 🔴 IT IS NOT A PRODUCT CHANGE: the client does not call
// requestVideoFrameCallback, and this probe does not add it there — it
// attaches from outside, for the duration of the measurement.
//
// The agent announces the capture instant to the peer through the RTCP sender report
// (agent/src/transport/piste_video.rs, held by the test
// `write_frame_annonce_l_instant_de_capture_au_pair_via_le_sender_report_rtcp`);
// the browser returns it in metadata.captureTime. The subtraction IS the
// latency.
//
// ⚠️ BOTH TIMESTAMPS ARE RETURNED IN THE DOCUMENT'S CLOCK
// (`DOMHighResTimeStamp`, same origin as `performance.now()`), and that is
// PRECISELY what makes the subtraction legitimate: `captureTime` is not
// the VM's clock, it is the capture instant BROUGHT BACK into the document's
// clock by the RTCP correlation. No clock synchronisation between
// host and guest is therefore required, and none is assumed.
//
// ⚠️ WHY A FRAME COUNTER NEXT TO THE SAMPLES. An empty array
// has TWO causes it does not distinguish: no frame arrives, or frames
// arrive without `captureTime`. Confusing them would make one conclude "no stream"
// where what we have is "no clock correlation", which is an ENTIRELY DIFFERENT
// defect. The counter separates them.
(() => {
  const echantillons = [];
  const compteurs = { trames: 0, sans_capture: 0, videos: 0 };
  const attacher = (video) => {
    if (typeof video.requestVideoFrameCallback !== 'function') return;
    compteurs.videos += 1;
    const tour = (maintenant, metadata) => {
      compteurs.trames += 1;
      // captureTime is ABSENT as long as the remote clock is not known:
      // a sample without it is not a zero, it does not exist.
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
