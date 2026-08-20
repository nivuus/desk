// La fusion de catalogue : décider, SANS BASE ET SANS HORLOGE, ce qu'il faut
// écrire quand un agent annonce ce qu'il voit sur son disque.
//
// 🔴 CE MODULE EST PUR, et c'est ce qui rend ses trois règles éprouvables. Il
// ne lit aucune horloge — l'instant de disparition est écrit par le dépôt, qui
// le reçoit en paramètre — et il ne connaît aucun pilote. La même figure que
// `agents/fraicheur.ts` et `orchestration/selection.ts` : la règle vit là où
// un test peut la rougir sans ouvrir de moteur SQL. `catalogue.test.ts` le
// vérifie mécaniquement, en BLANCHISSANT les commentaires de ce fichier avant
// de chercher — sans quoi cette phrase-ci suffirait à faire mentir le contrôle.
//
// 🔴 L'APPARIEMENT SE FAIT SUR LA CLÉ, JAMAIS SUR L'IDENTIFIANT. L'agent ne
// connaît pas les identifiants de la plateforme et ne les a jamais vus : il
// n'annonce que des clés, qui sont l'empreinte du triplet (cible, arguments,
// répertoire). Les identifiants ne sortent d'ici que dans les trois listes
// d'écriture, pour désigner des lignes que la plateforme connaît déjà.
//
// 🔴 UNE LIGNE N'EST JAMAIS SUPPRIMÉE, seulement marquée disparue. Une
// application installée côté navigateur porte l'identifiant de sa ligne ; la
// supprimer et la ré-insérer à la réapparition lui en donnerait un autre, et
// l'installation pointerait dans le vide. C'est pour la même raison qu'une
// ligne disparue qui revient est RESSUSCITÉE plutôt qu'insérée.

import type { Application, CatalogueMessage } from '../../../proto/ts/plateforme';

/// Ce que la plateforme sait déjà d'une application de cette VM.
///
/// ⚠️ TROIS CHAMPS, ET PAS UN DE PLUS. La fusion n'a besoin de rien d'autre :
/// l'identité pour désigner la ligne, la clé pour l'apparier à ce que l'agent
/// annonce, et l'état de disparition pour distinguer une mise à jour d'une
/// résurrection. Lui passer la ligne entière lui donnerait les moyens de
/// décider sur des champs dont la règle ne parle pas.
export interface Connue {
    id: string;
    cle: string;
    /// `null` = vivante. Non nul = l'instant où elle a cessé d'être vue.
    disparue_a: number | null;
}

/// Ce qu'il faut écrire. Quatre listes, que le dépôt applique dans l'ordre
/// qu'il veut : elles sont DISJOINTES par construction sur `aInserer`,
/// `aMarquerDisparues` et `aRessusciter`.
///
/// ⚠️ `aRessusciter` ET `aMettreAJour` SE RECOUVRENT DÉLIBÉRÉMENT : une ligne
/// qui revient est dans les deux. Ressusciter remet `disparue_a` à NULL ;
/// mettre à jour rafraîchit le nom, le chemin et les trois champs
/// d'identité. Une résurrection seule rendrait de nouveau visible une ligne
/// aux champs périmés — le raccourci a pu être renommé pendant son absence.
export interface Fusion {
    /// Des applications entières : la plateforme ne les connaît pas encore et
    /// leur attribuera un identifiant.
    aInserer: Application[];
    aMettreAJour: Array<{ id: string; app: Application }>;
    /// Des IDENTIFIANTS, jamais des clés — ce sont des lignes que la
    /// plateforme connaît, et c'est par son identifiant qu'on désigne une
    /// ligne qu'on ne veut surtout pas confondre.
    aMarquerDisparues: string[];
    /// Des identifiants aussi, pour la même raison.
    aRessusciter: string[];
}

/// Fusionne ce que la plateforme sait avec ce que l'agent annonce.
///
/// 🔴 `complet` DÉCIDE DE LA SEULE RÈGLE QUI PUISSE PERDRE DES DONNÉES, et les
/// deux sens sont dangereux dans des directions opposées :
///
///   - à `true`, TOUTE ligne connue absente d'`applications` est marquée
///     disparue, et `disparues` est ignoré. Sans cela, une application
///     désinstallée pendant que le canal était coupé resterait au catalogue
///     POUR TOUJOURS : sa clé ne figurerait dans aucun delta, personne ne
///     l'ayant vue partir. C'est ce renvoi complet à chaque (ré)enrôlement qui
///     donne un TERME à la divergence, sur un canal qui ne garantit aucune
///     livraison ;
///   - à `false`, AUCUNE disparition n'est inventée : seules les clés de
///     `disparues` sont marquées. Traiter un delta comme un état complet
///     viderait le catalogue à chaque message ne portant qu'une apparition.
///
/// ⚠️ UNE LIGNE DÉJÀ DISPARUE N'EST PAS RE-MARQUÉE. `disparue_a` est posée,
/// jamais déplacée : la réécrire à chaque tour ferait dire à la colonne
/// « disparue il y a trente secondes » d'une application partie depuis un
/// mois, et le seul lecteur possible de cette date serait trompé.
///
/// ⚠️ UNE CLÉ INCONNUE DE `disparues` EST IGNORÉE, jamais une exception :
/// l'agent peut annoncer la disparition d'une application que la plateforme
/// n'a jamais enregistrée — un seul message montant perdu suffit. Lever
/// abattrait le canal d'un agent qui va très bien.
export function fusionner(connues: Connue[], message: CatalogueMessage): Fusion {
    const parCle = new Map(connues.map((c) => [c.cle, c]));
    const fusion: Fusion = {
        aInserer: [],
        aMettreAJour: [],
        aMarquerDisparues: [],
        aRessusciter: [],
    };

    const annoncees = new Set<string>();
    for (const app of message.applications) {
        annoncees.add(app.cle);
        const connue = parCle.get(app.cle);
        if (connue === undefined) {
            fusion.aInserer.push(app);
            continue;
        }
        fusion.aMettreAJour.push({ id: connue.id, app });
        if (connue.disparue_a !== null) fusion.aRessusciter.push(connue.id);
    }

    if (message.complet) {
        // ⚠️ L'ORDRE EST CELUI DE `connues`, et il est stable : le dépôt écrit
        // dans cet ordre, et un test qui compare des tableaux a besoin d'un
        // ordre décidé plutôt que de celui d'un `Set`.
        for (const c of connues) {
            if (annoncees.has(c.cle)) continue;
            if (c.disparue_a !== null) continue;
            fusion.aMarquerDisparues.push(c.id);
        }
        return fusion;
    }

    for (const cle of message.disparues) {
        const c = parCle.get(cle);
        // Inconnue, ou déjà disparue : rien à faire, et surtout pas d'erreur.
        if (c === undefined || c.disparue_a !== null) continue;
        fusion.aMarquerDisparues.push(c.id);
    }
    return fusion;
}
