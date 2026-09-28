// Waiting for Chrome to accept DevTools connections, written ONCE.
//
// It lived identically in the three acceptance CDP drivers
// (`verify-webrtc.mjs`, `recette/harness.mjs`, `recette/paire-candidats.mjs`),
// up to wording differences — a silent `catch {}` here, a shorter error
// message there. Sub-block P2 had to add code to all three, and
// `verify-webrtc.mjs` was at 497 lines for a ceiling of 500: the gate of
// its task required an addition summing to ZERO OR LESS. That is what
// forced the extraction, and it was due anyway.
//
// ⚠️ A SINGLE BEHAVIOUR CHANGE, declared: `paire-candidats.mjs`
// threw "devtools timeout" and now throws the message below, which
// names the cause.

/// Waits for the DevTools endpoint to answer, or throws.
export async function attendreDevtools(port, tentatives = 50) {
    for (let i = 0; i < tentatives; i += 1) {
        try {
            const reponse = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (reponse.ok) return;
        } catch {
            // Chrome not ready yet to accept connections: retry.
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error('Chrome DevTools does not answer after the allotted delay');
}
