// L'attente que Chrome accepte des connexions DevTools, écrite UNE fois.
//
// Elle vivait à l'identique dans les trois pilotes CDP de recette
// (`verify-webrtc.mjs`, `recette/harness.mjs`, `recette/paire-candidats.mjs`),
// à des différences de rédaction près — un `catch {}` muet ici, un message
// d'erreur plus court là. Le sous-bloc P2 devait ajouter du code aux trois, et
// `verify-webrtc.mjs` était à 497 lignes pour un plafond de 500 : la porte de
// sa tâche exigeait une addition à somme NULLE OU NÉGATIVE. C'est ce qui a
// forcé l'extraction, et elle était de toute façon due.
//
// ⚠️ UN SEUL CHANGEMENT DE COMPORTEMENT, déclaré : `paire-candidats.mjs`
// levait « devtools timeout » et lève désormais le message ci-dessous, qui
// nomme la cause.

/// Attend que l'endpoint DevTools réponde, ou lève.
export async function attendreDevtools(port, tentatives = 50) {
    for (let i = 0; i < tentatives; i += 1) {
        try {
            const reponse = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (reponse.ok) return;
        } catch {
            // Chrome pas encore prêt à accepter des connexions : on retente.
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error('Chrome DevTools ne répond pas après le délai imparti');
}
