// La RÈGLE de résolution d'un chemin d'URL vers un fichier de la page bâtie.
// PURE : aucun `fs`, aucun `http`, aucune variable d'environnement. C'est ce
// qui la rend éprouvable sur l'hôte, et c'est là que vit toute la sécurité du
// servant — le module qui lit le disque ne fait qu'appliquer ce verdict.

/// 🔴 LISTE CLOSE. Une extension absente d'ici REFUSE.
export const TYPES_MIME: ReadonlyMap<string, string> = new Map([
    ['html', 'text/html; charset=utf-8'],
    ['js', 'text/javascript; charset=utf-8'],
    ['css', 'text/css; charset=utf-8'],
    ['webmanifest', 'application/manifest+json'],
    ['json', 'application/json; charset=utf-8'],
    ['ico', 'image/x-icon'],
    ['png', 'image/png'],
    ['svg', 'image/svg+xml'],
    ['woff2', 'font/woff2'],
]);

const PAGE = 'index.html';

export type Resolution =
    | { readonly ok: true; readonly fichier: string; readonly mime: string; readonly document: boolean }
    | {
          readonly ok: false;
          readonly motif: 'chemin-invalide' | 'octet-nul' | 'traversee' | 'extension-inconnue';
      };

/// Normalise un chemin en segments, en refusant toute remontée qui SORT.
///
/// ⚠️ LE COMPTE SE FAIT SUR LE RÉSULTAT, JAMAIS SUR LA PRÉSENCE DE `..` : c'est
/// ce qui rend `/assets/../index.html` légitime et `/assets/../../x` refusé,
/// là où un filtre par sous-chaîne refuserait les deux ou accepterait les deux.
function normaliser(brut: string): string[] | undefined {
    const sortie: string[] = [];
    for (const segment of brut.split('/')) {
        if (segment === '' || segment === '.') continue;
        // Un antislash n'est pas un séparateur sous Linux, mais un chemin qui
        // en porte un ne vient d'aucune page bâtie par Vite : le refuser coûte
        // zéro et ferme la variante Windows de la traversée.
        if (segment === '..' || segment.includes('\\')) {
            if (segment === '..' && sortie.length > 0) {
                sortie.pop();
                continue;
            }
            return undefined;
        }
        sortie.push(segment);
    }
    return sortie;
}

export function resoudre(cheminUrl: string): Resolution {
    let decode: string;
    try {
        decode = decodeURIComponent(cheminUrl);
    } catch {
        // `decodeURIComponent` LÈVE sur un `%` mal formé. Un servant qui
        // laisserait passer cette exception rendrait un 500 là où un refus
        // suffit — et le `catch` du serveur journaliserait une « route en
        // échec » pour une requête simplement mal écrite.
        return { ok: false, motif: 'chemin-invalide' };
    }
    if (decode.includes('\0')) return { ok: false, motif: 'octet-nul' };

    const segments = normaliser(decode);
    if (segments === undefined) return { ok: false, motif: 'traversee' };

    const dernier = segments[segments.length - 1];
    // Racine, ou chemin sans extension : le `try_files … /index.html` de nginx.
    const fichier = dernier === undefined || !dernier.includes('.') ? PAGE : segments.join('/');

    const point = fichier.lastIndexOf('.');
    const extension = fichier.slice(point + 1).toLowerCase();
    const mime = TYPES_MIME.get(extension);
    if (mime === undefined) return { ok: false, motif: 'extension-inconnue' };

    return { ok: true, fichier, mime, document: extension === 'html' };
}
