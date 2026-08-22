// La RÈGLE de résolution d'un chemin d'URL vers un fichier de la page bâtie.
// PURE : aucun `fs`, aucun `http`, aucune variable d'environnement. C'est ce
// qui la rend éprouvable sur l'hôte, SANS DISQUE.
//
// 🔴 CETTE RÈGLE NE PORTE PAS « TOUTE » LA SÉCURITÉ DU SERVANT, ET UNE REVUE
// PAR EXÉCUTION L'A ÉTABLI (round 2, Critique 3) — CETTE PHRASE LE DISAIT
// ENCORE ICI ALORS QU'ELLE ÉTAIT DÉJÀ FAUSSE DANS `page/routes-page.ts`.
// Cette règle ferme la traversée LEXICALE (`..`, encodée ou non) — ce qui
// SUFFISAIT tant que rien ne lisait le disque. `page/routes-page.ts`, lui,
// LIT le disque, et un lien symbolique déposé dans la racine bâtie traverse
// cette règle sans qu'aucun `..` n'apparaisse jamais dans l'URL : la garde
// réelle contre les liens (`realpath`, sur le chemin CANONIQUE) vit donc
// dans ce module-là, pas ici. Ce que CETTE règle garantit reste vrai et
// nécessaire — elle n'est simplement plus SUFFISANTE seule.

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
          readonly motif:
              | 'chemin-invalide'
              | 'octet-nul'
              | 'traversee'
              | 'extension-inconnue'
              | 'nom-vide';
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

    // 🔴 UN NOM RÉDUIT À UNE EXTENSION NUE (`/.json`, `/assets/.webmanifest`…)
    // A UN NOM VIDE AVANT L'EXTENSION — CE N'EST PAS UN CAS LÉGITIME, C'EST UN
    // TROU DE LA RÈGLE. Une revue antérieure l'avait jugé SANS EXPLOITABILITÉ
    // parce que CE MODULE ne lit rien lui-même ; mais celui qui l'applique
    // (`page/routes-page.ts`) LIT LE DISQUE, et servirait tel quel un fichier
    // littéralement nommé `.json` s'il existait à la racine bâtie. La garde
    // est posée ICI, APRÈS la liste MIME et jamais avant : `/.env` et
    // `/.htaccess` sont déjà refusés par `extension-inconnue` (leur
    // « extension » n'y figure pas), et ce verdict-là reste inchangé — le
    // déplacer aurait menti sur la raison de LEUR refus. Ce que cette ligne
    // ferme est la CLASSE que la liste MIME laisse passer par accident :
    // n'importe quelle extension CONNUE (`.json` en fait partie) portée par
    // un nom vide. Fermer la classe évite de dépendre au cas par cas d'une
    // liste qui n'a pas été écrite pour trancher cette question.
    if (dernier !== undefined && dernier.lastIndexOf('.') === 0) {
        return { ok: false, motif: 'nom-vide' };
    }

    return { ok: true, fichier, mime, document: extension === 'html' };
}
