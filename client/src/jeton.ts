// Le seul endroit du client qui sache OÙ vit le jeton et s'il est encore
// frais. Tout le reste du navigateur passe par ici.
//
// 🔴 CE MODULE EST PUR ET SANS DOM. Relevé le 19 août 2026 : `client/` n'a
// aucun `vitest.config.*`, donc l'environnement de test est le Node par
// défaut — il n'y a ni `window` ni `localStorage`. Le `Coffre` est un
// PARAMÈTRE ; `globalThis.localStorage` n'est touché que dans le défaut d'un
// argument, à l'appel, jamais au chargement du module. Un `const coffre =
// localStorage` en tête de fichier suffirait à rendre ce module impossible à
// charger sous Node, et donc impossible à tester.
//
// 🔴 LE STOCKAGE EST `localStorage`, ET LE COÛT EST ICI PLUTÔT QUE DÉCOUVERT :
// un jeton en `localStorage` est lisible par TOUT script de la page, donc par
// une injection de script. `sessionStorage` ne convient pas — la page-shell
// ouvre ses fenêtres par `window.open` (`shell-page.ts`), et le stockage de
// session n'est pas garanti partagé avec elles, ce qui obligerait chaque
// fenêtre à se reconnecter. C'est un ARBITRAGE, pas un oubli, et l'endroit où
// il se rouvrira est le sous-bloc P5, avec les en-têtes de sécurité.
//
// ⚠️ `exp` EST EN MILLISECONDES, et ce n'est pas une erreur de lecture :
// `plateforme/src/identite/jeton.ts` déclare cette divergence délibérée avec
// la RFC 7519, pour qu'il n'y ait qu'une seule unité de temps dans tout le
// service. Ce fichier en est le miroir côté navigateur ; changer l'un sans
// l'autre casserait la fraîcheur en silence.

export interface Coffre {
    getItem(cle: string): string | null;
    setItem(cle: string, valeur: string): void;
    removeItem(cle: string): void;
}

export interface Paire {
    acces: string;
    rafraichissement: string;
}

export const CLE_ACCES = 'guac.jeton.acces';
export const CLE_RAFRAICHISSEMENT = 'guac.jeton.rafraichissement';

/// Le coffre par défaut, lu À L'APPEL et jamais au chargement. Rend
/// `undefined` hors navigateur, ce qui laisse l'appelant décider — plutôt que
/// de lever au premier `import` sous Node.
function coffreParDefaut(): Coffre | undefined {
    const global = globalThis as { localStorage?: Coffre };
    return global.localStorage;
}

export function poser(coffre: Coffre, paire: Paire): void {
    coffre.setItem(CLE_ACCES, paire.acces);
    coffre.setItem(CLE_RAFRAICHISSEMENT, paire.rafraichissement);
}

/// Efface LES DEUX clés. N'en effacer qu'une laisserait un rafraîchissement
/// utilisable derrière une déconnexion.
export function vider(coffre: Coffre): void {
    coffre.removeItem(CLE_ACCES);
    coffre.removeItem(CLE_RAFRAICHISSEMENT);
}

export function jetonAcces(coffre: Coffre | undefined = coffreParDefaut()): string | undefined {
    return coffre?.getItem(CLE_ACCES) ?? undefined;
}

export function jetonRafraichissement(
    coffre: Coffre | undefined = coffreParDefaut(),
): string | undefined {
    return coffre?.getItem(CLE_RAFRAICHISSEMENT) ?? undefined;
}

/// Dit si le jeton sera périmé à `instant`.
///
/// 🔴 LIT `exp` SANS VÉRIFIER LA SIGNATURE, ET C'EST DÉLIBÉRÉ : le navigateur
/// n'a pas le secret de signature et ne peut donc RIEN vérifier. Croire qu'il
/// vérifie serait pire que savoir qu'il ne le fait pas — la seule vérification
/// qui compte est celle du service (`identite/jeton.ts`), sur chaque poignée
/// de main. Ce que cette fonction sert, c'est à éviter un aller-retour inutile,
/// pas à décider d'une autorisation.
///
/// Un jeton ILLISIBLE est réputé périmé : le tenir pour valable ferait échouer
/// la session plus tard, ailleurs, sur un refus que rien ne relierait à ici.
export function expireAvant(jeton: string, instant: number): boolean {
    const morceaux = jeton.split('.');
    if (morceaux.length !== 3) return true;
    try {
        const charge: unknown = JSON.parse(
            decoderBase64url(morceaux[1]),
        );
        if (typeof charge !== 'object' || charge === null) return true;
        const exp = (charge as { exp?: unknown }).exp;
        if (typeof exp !== 'number' || !Number.isFinite(exp)) return true;
        return exp <= instant;
    } catch {
        return true;
    }
}

/// Décode un segment base64url en texte. Écrit à la main plutôt qu'avec
/// `Buffer` : ce module tourne dans un navigateur, où `Buffer` n'existe pas.
function decoderBase64url(segment: string): string {
    const base64 = segment.replace(/-/g, '+').replace(/_/g, '/');
    const complet = base64 + '='.repeat((4 - (base64.length % 4)) % 4);
    const binaire = atob(complet);
    // `atob` rend des unités de code latin-1 : les recomposer en UTF-8 avant
    // de parler de JSON, sans quoi un courriel accentué casserait la lecture.
    const octets = Uint8Array.from(binaire, (c) => c.charCodeAt(0));
    return new TextDecoder().decode(octets);
}

/// Rafraîchit la paire si le jeton d'accès expire dans moins de `margeMs`.
///
/// Rend `true` si, au retour, le coffre porte un jeton d'accès utilisable —
/// qu'il ait fallu appeler ou non. Rend `false` quand il n'y a plus rien à
/// présenter : le coffre est alors VIDÉ, pour que l'appelant renvoie vers
/// l'écran de connexion plutôt que de boucler sur un refus.
///
/// `appel` est INJECTÉ, jamais `fetch` global : c'est ce qui rend cette règle
/// éprouvable sans réseau.
export async function rafraichirSiNecessaire(
    coffre: Coffre,
    maintenant: number,
    margeMs: number,
    appel: (corps: unknown) => Promise<{ acces: string; rafraichissement: string } | undefined>,
): Promise<boolean> {
    const acces = jetonAcces(coffre);
    if (acces !== undefined && !expireAvant(acces, maintenant + margeMs)) return true;

    const rafraichissement = jetonRafraichissement(coffre);
    if (rafraichissement === undefined) {
        // Rien à présenter : on efface ce qui reste plutôt que de laisser un
        // accès périmé que la poignée de main refuserait.
        vider(coffre);
        return false;
    }

    const neuve = await appel({ rafraichissement });
    if (!neuve) {
        vider(coffre);
        return false;
    }
    poser(coffre, neuve);
    return true;
}
