// PLATEFORME_HOTE n'a AUCUN défaut, et c'est le point de cette fonction.
//
// L'ex-`signaling/src/server.ts:67` — aujourd'hui `src/signaling/relais.ts`,
// le paquet `signaling/` ayant disparu au sous-bloc P1 — faisait
// `new WebSocketServer({ port })` sans
// `host` : le service écoutait sur toutes les interfaces, et délivrait des
// identifiants TURN valables 24 h (`ice.ts:16`) à quiconque atteignait le
// port. Poser un défaut ici — même `127.0.0.1` — ferait passer le critère ④
// du sous-bloc P1 sans rien garantir : l'opérateur ne saurait jamais sur quoi
// il écoute. Une rupture bruyante vaut mieux qu'une écoute universelle
// silencieuse.
//
// ⚠️ UNE MOITIÉ DE CE « À QUICONQUE » AVAIT SURVÉCU AU SOUS-BLOC P2, ET ELLE
// EST FERMÉE DEPUIS P3. Le texte disait ici qu'un pair se déclarant
// `{"role":"agent"}` était toujours accepté sans identité et recevait ses
// identifiants TURN de 86 400 s ; ce n'est plus vrai — le rôle `agent` exige
// son jeton, de type `agent` et à sujet préfixant la session
// (`identite/garde.ts`), et ce jeton s'obtient sur le canal `/agent` contre le
// secret d'enrôlement de la VM.
//
// **L'argument ci-dessus n'en perd RIEN**, et c'est pourquoi le paragraphe est
// corrigé plutôt que supprimé : `PLATEFORME_HOTE` borne QUI PEUT ATTEINDRE le
// port, ce qui vaut avant toute authentification et pour les deux chemins —
// le relais comme le canal d'enrôlement.
//
// ✅ **P5 A LIVRÉ CE QUE CETTE PHRASE ANNONÇAIT**, et elle est corrigée plutôt
// que supprimée : elle disait « les tentatives de secret ne sont bridées par
// rien à ce jour (c'est le sujet de P5) ». Elles le sont — `agents/canal.ts`
// consulte le frein AVANT `verifierEnrolement`, donc avant tout `scrypt`.
// L'argument de `PLATEFORME_HOTE` ci-dessus n'en perd rien : il vaut toujours
// avant toute authentification, et il couvre le relais, que le frein NE couvre
// PAS (voir l'annotation de `signaling/resilience.test.ts`).

//
// La lecture d'environnement se fait ICI et nulle part ailleurs : `env` est un
// paramètre, jamais `process.env` lu en douce, ce qui rend la fonction pure et
// testable sans salir l'environnement du processus de test.

import { LONGUEUR_SECRET_MIN } from './identite/jeton';

export interface Config {
    /// PLATEFORME_HOTE — aucun défaut, voir le commentaire de tête.
    hote: string;
    /// PLATEFORME_PORT, défaut 8080.
    port: number;
    /// PLATEFORME_BASE, défaut 'sqlite'. Une valeur inconnue LÈVE : un repli
    /// silencieux sur sqlite ferait tourner la production sur un fichier
    /// local sans que rien ne le dise.
    base: 'sqlite' | 'postgres';
    /// PLATEFORME_BASE_URL — chemin de fichier SQLite ou URL de connexion pg.
    urlBase: string;
    /// PLATEFORME_SECRET_JETON — AUCUN défaut, `LONGUEUR_SECRET_MIN`
    /// caractères au moins. Voir le commentaire ci-dessous : un secret tiré
    /// au hasard au démarrage serait pire qu'une absence de secret.
    secretJeton: string;
    /// PLATEFORME_ORIGINE_CLIENT — FACULTATIVE. Absente, aucun en-tête CORS
    /// n'est émis et le navigateur refuse : le défaut est le refus.
    origineClient?: string;
    /// PLATEFORME_PROXY_DE_CONFIANCE — FACULTATIVE, liste séparée par des
    /// virgules. Absente ou vide, l'ensemble est VIDE : on ne croit l'en-tête
    /// `X-Forwarded-For` d'AUCUNE source. Voir le commentaire au point de
    /// lecture, plus bas.
    ///
    /// ⚠️ ELLE N'EST JAMAIS `undefined` : un ensemble vide se traverse, un
    /// `undefined` se déréférence. C'est l'asymétrie voulue avec
    /// `origineClient` ci-dessus, dont l'absence a un sens pour l'appelant
    /// (« n'émets aucun en-tête ») là où celle-ci n'en a qu'un (« ne crois
    /// personne »), déjà porté par l'ensemble vide.
    proxyDeConfiance: ReadonlySet<string>;
    /// PLATEFORME_ICONES — FACULTATIVE, défaut `donnees/icones`. Le répertoire
    /// du magasin d'icônes adressé par contenu (sous-bloc G2).
    ///
    /// ⚠️ **ASYMÉTRIE ASSUMÉE AVEC `PLATEFORME_HOTE`, ET IL FAUT DIRE
    /// POURQUOI.** Le commentaire de tête de ce fichier fonde l'absence de
    /// défaut sur le fait qu'un mauvais défaut EXPOSERAIT le service. Ici, un
    /// mauvais répertoire coûte **un retéléversement, borné et automatique** :
    /// le magasin se reconstruit tout seul à la réconciliation suivante, parce
    /// que la plateforme demande ce qui lui manque en interrogeant son DISQUE.
    /// Une rupture bruyante ne serait pas proportionnée — mais un silence non
    /// plus, d'où la ligne de journal à l'ouverture du magasin.
    repertoireIcones: string;
    /// PLATEFORME_TELEVERSEMENTS — FACULTATIVE, défaut `donnees/televersements`.
    /// La racine du magasin des TRANCHES : `<racine>/<id>/<n>`, un fichier par
    /// tranche, et jamais de fichier assemblé (sous-bloc G3).
    ///
    /// ⚠️ **MÊME ASYMÉTRIE ASSUMÉE AVEC `PLATEFORME_HOTE` QUE `repertoireIcones`
    /// CI-DESSUS, ET IL FAUT LA DIRE PLUTÔT QUE DE L'HÉRITER.** Là, un mauvais
    /// défaut EXPOSERAIT le service ; ici il coûte un RETÉLÉVERSEMENT — borné,
    /// et visible de l'utilisateur qui le refait. Une rupture bruyante ne
    /// serait pas proportionnée.
    ///
    /// ⚠️ **MAIS LA CONSÉQUENCE EST PLUS LOURDE QUE POUR LES ICÔNES, ET CE
    /// N'EST PAS LE MÊME MOT.** Le magasin d'icônes se reconstruit TOUT SEUL —
    /// la plateforme redemande à l'agent ce que son disque n'a pas. Un
    /// téléversement perdu, lui, ne se reconstruit pas : il faut qu'un humain
    /// redépose son fichier. Le silence est donc encore moins acceptable
    /// ici — d'où la ligne de journal à l'ouverture du magasin.
    repertoireTeleversements: string;
    /// PLATEFORME_PAGE — FACULTATIVE, et **AUCUN DÉFAUT**, à la différence de
    /// `PLATEFORME_ICONES` et `PLATEFORME_TELEVERSEMENTS` juste en dessous.
    ///
    /// 🔴 ABSENTE OU VIDE ⇒ LE SERVICE NE SERT AUCUN FICHIER, et son
    /// comportement est celui d'avant le lot À L'OCTET PRÈS : `GET /` rend
    /// `404 introuvable`. C'est ce qui rend l'ajout strictement additif — et
    /// c'est ce qui rend le témoin négatif de la recette jouable.
    ///
    /// ⚠️ UN DÉFAUT SERAIT UN DÉFAUT DE SÉCURITÉ, pas une commodité : dans le
    /// montage nginx, la plateforme ne doit RIEN servir, et un défaut la
    /// ferait publier ce que son répertoire courant contient.
    racinePage?: string;
    /// PLATEFORME_AUTH, défaut 'pomerium'. Une valeur inconnue LÈVE.
    ///
    /// ⚠️ CE N'EST PAS UN ARMEMENT, C'EST UN CHOIX DE MODE — la convention
    /// `=0 désarme` de `agent/` ne s'applique pas ici. Le précédent est
    /// `PLATEFORME_BASE` quinze lignes plus haut, et pour la même raison : un
    /// repli silencieux ferait tourner un mode sous le nom de l'autre, et
    /// l'un des deux sens est une OUVERTURE.
    auth: 'pomerium' | 'motdepasse';
}

const BASES = ['sqlite', 'postgres'] as const;
const AUTHS = ['pomerium', 'motdepasse'] as const;

/// Les adresses qui font écouter le service sur TOUTES les interfaces.
///
/// 🔴 CE N'EST PAS « L'ÉCOUTE EST BORNÉE », ET LA DIFFÉRENCE EST ÉCRITE PLUTÔT
/// QUE MAQUILLÉE. Le contrôle qu'on aimerait — « ce doit être une adresse de
/// bouclage » — casserait le déploiement livré, qui pose `PLATEFORME_HOTE:
/// plateforme`, un nom de service Docker sans port publié, et qui est le
/// montage le plus sûr des trois. Ce qui est décidable est le refus de
/// l'écoute UNIVERSELLE ; que seul Pomerium atteigne le port reste à la charge
/// de l'exploitant, et c'est dit au § 9 de la spec.
const ECOUTES_UNIVERSELLES = new Set(['0.0.0.0', '::', '[::]', '*']);

export function lireConfig(env: Record<string, string | undefined>): Config {
    const hote = env.PLATEFORME_HOTE;
    if (hote === undefined || hote === '') {
        throw new Error(
            "PLATEFORME_HOTE est obligatoire et n'a aucun défaut : nommer l'adresse " +
                "d'écoute, sans quoi le service écouterait sur toutes les interfaces.",
        );
    }

    const brutAuth = env.PLATEFORME_AUTH;
    const auth = (brutAuth === undefined || brutAuth === '' ? 'pomerium' : brutAuth) as Config['auth'];
    if (!(AUTHS as readonly string[]).includes(auth)) {
        throw new Error(
            `PLATEFORME_AUTH doit valoir ${AUTHS.join(' ou ')}, reçu : ${brutAuth}`,
        );
    }

    // 🔴 LIÉE AU MODE, ET NON UNIVERSELLE. En mode `motdepasse`, le service
    // s'authentifie lui-même et une écoute large ne le rend pas anonyme ; en
    // mode `pomerium`, l'identité arrive dans un en-tête EN CLAIR, et une
    // écoute universelle l'offre à quiconque atteint la machine.
    if (auth === 'pomerium' && ECOUTES_UNIVERSELLES.has(hote.trim())) {
        throw new Error(
            `PLATEFORME_HOTE=${hote} est une écoute universelle, refusée en mode ` +
                "pomerium : l'identité arrive dans un en-tête en clair, que seul le " +
                'proxy doit pouvoir poser. Nommer une adresse précise ' +
                '(192.168.3.1, 127.0.0.1) ou un nom de service de réseau interne.',
        );
    }

    const brutPort = env.PLATEFORME_PORT;
    let port = 8080;
    if (brutPort !== undefined && brutPort !== '') {
        if (!/^\d+$/.test(brutPort)) {
            throw new Error(`PLATEFORME_PORT doit être un entier, reçu : ${brutPort}`);
        }
        port = Number(brutPort);
    }

    const brutBase = env.PLATEFORME_BASE;
    const base = (brutBase === undefined || brutBase === '' ? 'sqlite' : brutBase) as Config['base'];
    if (!(BASES as readonly string[]).includes(base)) {
        throw new Error(
            `PLATEFORME_BASE doit valoir ${BASES.join(' ou ')}, reçu : ${brutBase}`,
        );
    }

    const urlBase = env.PLATEFORME_BASE_URL ?? ':memory:';

    // ⚠️ LE TEST DE LA CHAÎNE VIDE EST DISTINCT DE CELUI DE L'ABSENCE :
    // `env.X ?? 'defaut'` ne rattrape PAS `''`, et P1 a payé cette erreur
    // exacte. Un `PLATEFORME_ICONES=` vide doit retomber sur le défaut, pas
    // faire du magasin le répertoire courant.
    const brutIcones = env.PLATEFORME_ICONES;
    const repertoireIcones =
        brutIcones === undefined || brutIcones === '' ? 'donnees/icones' : brutIcones;

    // Même garde de la chaîne VIDE, et pour la même raison qu'au-dessus : un
    // `PLATEFORME_TELEVERSEMENTS=` vide ferait de la racine des tranches le
    // répertoire COURANT du service, où elles se mêleraient à ses sources.
    const brutTeleversements = env.PLATEFORME_TELEVERSEMENTS;
    const repertoireTeleversements =
        brutTeleversements === undefined || brutTeleversements === ''
            ? 'donnees/televersements'
            : brutTeleversements;

    // Même garde de la chaîne VIDE qu'au-dessus, mais SANS repli : ici, vide
    // et absente valent toutes deux « aucun servant ».
    const brutPage = env.PLATEFORME_PAGE;
    const racinePage = brutPage === undefined || brutPage === '' ? undefined : brutPage;

    // 🔴 AUCUN DÉFAUT, et surtout pas un défaut ALÉATOIRE. Un secret tiré au
    // démarrage passerait tous les tests de forme, puis invaliderait à chaque
    // redémarrage l'ensemble des jetons délivrés — les utilisateurs seraient
    // déconnectés sans qu'aucune trace n'en donne la cause. C'est la même
    // décision que `PLATEFORME_HOTE` : une rupture bruyante vaut mieux qu'une
    // dégradation silencieuse.
    //
    // Le test de la chaîne VIDE est distinct de celui de l'absence, parce que
    // `env.X ?? 'defaut'` ne rattrape pas `''` — P1 a payé cette erreur exacte
    // à sa tâche 1, où un des deux rouges annoncés était en réalité vert.
    const secretJeton = env.PLATEFORME_SECRET_JETON;
    if (secretJeton === undefined || secretJeton === '') {
        throw new Error(
            "PLATEFORME_SECRET_JETON est obligatoire et n'a aucun défaut : sans lui " +
                'aucun jeton ne peut être signé, et un défaut aléatoire invaliderait ' +
                'toutes les sessions à chaque redémarrage.',
        );
    }
    if (secretJeton.length < LONGUEUR_SECRET_MIN) {
        throw new Error(
            `PLATEFORME_SECRET_JETON est trop court : ${secretJeton.length} caractères, ` +
                `${LONGUEUR_SECRET_MIN} au moins sont exigés — un secret devinable ` +
                "n'authentifie personne.",
        );
    }

    // FACULTATIVE, contrairement à `PLATEFORME_HOTE`, et l'asymétrie est dans
    // les conséquences : une origine absente produit un refus BRUYANT du
    // navigateur, que l'opérateur voit immédiatement ; une adresse d'écoute
    // absente produirait une écoute universelle SILENCIEUSE. Refuser de
    // démarrer pour elle casserait par ailleurs le déploiement de P5, où le
    // proxy inverse met le client et la plateforme sur la MÊME origine et où
    // aucune valeur n'aurait de sens.
    const brutOrigine = env.PLATEFORME_ORIGINE_CLIENT;
    const origineClient = brutOrigine === undefined || brutOrigine === '' ? undefined : brutOrigine;

    // FACULTATIVE, comme l'origine ci-dessus, et pour une raison voisine : un
    // déploiement SANS proxy inverse — celui des tests, et celui d'un
    // exploitant qui expose le service directement — n'a aucune valeur qui ait
    // du sens ici. Refuser de démarrer casserait ces deux cas.
    //
    // 🔴 MAIS SON DÉFAUT EST LE REFUS DE CROIRE, JAMAIS UNE PERMISSION. Même
    // doctrine que `PLATEFORME_ORIGINE_CLIENT` : absente, l'ensemble est vide,
    // et `http/adresse-source.ts` ignore alors `X-Forwarded-For` quel qu'il
    // soit. Un défaut permissif — croire l'en-tête de tout le monde, ou même
    // seulement des adresses privées — rendrait l'adresse du client FORGEABLE
    // PAR LE CLIENT, donc le frein par adresse contournable en une ligne
    // d'en-tête. C'est le seul défaut qui échange une panne bruyante contre
    // un contournement silencieux, et c'est exactement ce que ce fichier
    // refuse depuis `PLATEFORME_HOTE`.
    //
    // ⚠️ LE MODE DE DÉFAILLANCE DE L'OUBLI EST NOMMÉ, et il n'est pas
    // silencieux par accident mais par CHOIX ASSUMÉ. Un exploitant qui pose un
    // proxy sans déclarer sa confiance verra TOUTES les requêtes porter
    // l'adresse du proxy : le frein par adresse dégénère en frein GLOBAL, et
    // le service se refuse à lui-même au 51e échec. Le remède n'est PAS de
    // croire par défaut — ce serait le contournement ci-dessus — mais que LA
    // TRACE DU FREIN NOMME L'ADRESSE RETENUE : un exploitant qui lit
    // `adresse=172.18.0.5` sur toutes les lignes reconnaît l'adresse de son
    // proxy. `deploiement/README.md` le dit aussi.
    //
    // ⚠️ EXACTEMENT UN PROXY EN TÊTE DE CHAÎNE. Deux proxies enchaînés font
    // rendre à `adresseSource` l'adresse du PREMIER PROXY, pas celle du
    // client : P5 ne livre pas la chaîne à N sauts, et
    // `http/adresse-source.ts` le documente.
    //
    // Le test de la chaîne VIDE est distinct de celui de l'absence, pour la
    // raison déjà payée par P1 plus haut dans ce fichier : `??` ne rattrape
    // pas `''`. Ici, `''.split(',')` rendrait `['']` — donc un ensemble à UNE
    // entrée vide, qui rendrait de confiance tout pair sans adresse.
    const brutProxy = env.PLATEFORME_PROXY_DE_CONFIANCE;
    const proxyDeConfiance: ReadonlySet<string> = new Set(
        (brutProxy ?? '')
            .split(',')
            .map((entree) => entree.trim())
            // Sans ce filtre, `'172.18.0.5,,10.0.0.1'` porterait une entrée
            // vide — et `'  '` en porterait une aussi, après `trim`.
            .filter((entree) => entree !== ''),
    );

    // 🔴 TROISIÈME GARDE LIÉE AU MODE, après celle de `PLATEFORME_HOTE`. Même
    // raison : en `pomerium`, l'identité arrive dans un en-tête EN CLAIR
    // qu'aucune signature ne vérifie, et sans la liste des adresses autorisées
    // à le poser, l'en-tête est croyable par n'importe qui.
    //
    // ⚠️ POURQUOI UN REFUS DE DÉMARRER ET NON UN 401 : un refus se lit AVANT
    // d'agir et nomme la variable. Un 401 pour tout le monde se lirait APRÈS,
    // sur un service qui répond et sert les dix autres routeurs.
    if (auth === 'pomerium' && proxyDeConfiance.size === 0) {
        throw new Error(
            'PLATEFORME_PROXY_DE_CONFIANCE est obligatoire en mode pomerium : ' +
                "l'identité arrive dans un en-tête en clair qu'aucune signature ne vérifie, " +
                'et sans la liste des adresses autorisées à le poser, quiconque atteint le ' +
                "port obtient un jeton pour l'identité de son choix. Poser l'adresse du " +
                'proxy, ou PLATEFORME_AUTH=motdepasse.',
        );
    }

    return {
        hote,
        port,
        base,
        urlBase,
        secretJeton,
        origineClient,
        proxyDeConfiance,
        repertoireIcones,
        repertoireTeleversements,
        racinePage,
        auth,
    };
}
