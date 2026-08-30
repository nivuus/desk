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

// 🔴 `hub.html`, PAS `index.html` — DÉCISION DU PROPRIÉTAIRE DU DÉPÔT
// (« Sers le hub à la racine », 30 août 2026), après un défaut mesuré EN
// PRODUCTION : `https://app.allanic.me/` rendait `index.html`, la page de
// SESSION — et sans paramètre `?session=`, `client/src/main.ts` inventait
// alors une session `demo` SANS jeton (corrigé dans le même lot, voir
// `client/src/session-id.ts`). L'agent journalisait « poignée de main
// refusée : poignée de main sans jeton sur la session demo », et le
// propriétaire voyait « Échec de la session » — une panne INDISCERNABLE
// d'une vraie pour qui ouvre juste l'adresse du service.
//
// La racine et tout chemin sans extension replient désormais sur le HUB, la
// seule surface qui ouvre une session avec un jeton réel. `index.html` (la
// page de session) N'EST PAS RETIRÉ : il reste servi à SON PROPRE chemin
// EXPLICITE, `/index.html` — segment avec extension, donc jamais résolu par
// `PAGE` ci-dessous (voir `resoudre()`). Seul le repli change de cible.
const PAGE = 'hub.html';

/// 🔴 LE RÉPERTOIRE DONT VITE EMPREINTE **TOUS** LES NOMS, ET LE SEUL.
///
/// C'est `build.assetsDir`, dont le défaut est `assets` et que `client/
/// vite.config.ts` ne surcharge pas. Relevé sur la page réellement bâtie, le
/// 22 août 2026 :
///   ls client/dist         -> assets/ + connexion.html design.html hub.html
///                             hub.webmanifest index.html primitives.html
///                             shell.html
///   ls client/dist/assets  -> adresse-plateforme-uutwZeXQ.js,
///                             main-DOC38JmJ.css, hub-B8O-1KAt.js, …
/// **La racine ne porte AUCUN nom empreinté** ; le répertoire d'actifs n'en
/// porte que. C'est cette partition-là, et non une expression régulière sur la
/// forme d'un nom, qui décide du cache — un nom se déguise, un emplacement
/// non.
export const REPERTOIRE_ACTIFS = 'assets';

export type Resolution =
    | {
          readonly ok: true;
          readonly fichier: string;
          readonly mime: string;
          readonly document: boolean;
          /// 🔴 LE NOM PORTE-T-IL RÉELLEMENT UNE EMPREINTE ? C'est la question
          /// que la première rédaction ne posait PAS : elle classait par
          /// EXTENSION, et donnait donc un an d'`immutable` à `hub.webmanifest`
          /// et `favicon.ico` — des noms que Vite n'empreinte JAMAIS. MESURÉ
          /// sur le vrai `client/dist` : `/hub.webmanifest` rendait
          /// `public, max-age=31536000, immutable`, ce qui rend **le manifeste
          /// PWA du hub non révisable pendant un an** chez tout navigateur
          /// l'ayant vu. Sous nginx, `location /` n'émet AUCUN
          /// `Cache-Control` : c'était une régression que le seul montage
          /// Pomerium introduisait.
          ///
          /// ⚠️ LA DISTINCTION VIT ICI, DANS LA RÈGLE PURE, ET NON DANS LE
          /// SERVANT : c'est là que vit déjà la classification, et c'est ce
          /// qui la rend éprouvable SANS DISQUE.
          readonly empreinte: boolean;
      }
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
    // Racine, ou chemin sans extension : le repli SPA — l'équivalent du
    // `try_files … /index.html` de nginx, sauf que la CIBLE, ici, est
    // `hub.html` depuis le 30 août 2026 (voir `PAGE` ci-dessus).
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

    return {
        ok: true,
        fichier,
        mime,
        document: extension === 'html',
        // ⚠️ LE PRÉFIXE PORTE LE SÉPARATEUR : sans lui, un fichier nommé
        // `assetsX.js` posé à la racine passerait pour un actif empreinté.
        // `fichier` est déjà NORMALISÉ (segments recomposés, aucune remontée
        // survivante), donc ce test porte bien sur le premier segment.
        empreinte: fichier.startsWith(`${REPERTOIRE_ACTIFS}/`),
    };
}
