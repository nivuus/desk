// D'où le client tire l'adresse de la plateforme, et pourquoi ce n'est plus un
// littéral.
//
// 🔴 CE MODULE EXISTE POUR UNE PANNE QUE LE DÉPLOIEMENT DE P5 AURAIT RENDUE
// CERTAINE. Trois fichiers portaient l'adresse en dur :
//
//     connexion.ts   `http://${window.location.hostname}:8080`
//     main.ts        `ws://${window.location.hostname}:8080`
//     shell-page.ts  `ws://${window.location.hostname}:8080`
//
// Une page servie en **`https://`** par le proxy qui ouvre `ws://…:8080` est du
// **CONTENU MIXTE** : le navigateur refuse la connexion. Et il la refuse en
// silence pour tout ce qui n'est pas la console — la page se charge, le jeton
// est accepté, et le média ne s'établit jamais. **Aucun test Node ne peut le
// voir** ; `cors.ts` écrit déjà cette phrase pour son propre sujet. Le
// déploiement aurait donc été livré non fonctionnel ET VERT.
//
// Le port 8080 était faux pour la même raison : le proxy sert sur 443, et rien
// n'écoute 8080 depuis l'extérieur.
//
// 🔴 PUR ET SANS DOM, sur le précédent de `resize.ts`, `prefixe.ts` et
// `jeton.ts` : `client/` n'a aucun `vitest.config.*`, donc l'environnement de
// test est le Node par défaut — il n'y a ni `window` ni `location`. Ce qu'il
// faut de `location` est un PARAMÈTRE, et c'est ce qui rend `https:` éprouvable
// sans navigateur.
//
// ⚠️ DEUX FONCTIONS ET NON UNE, alors qu'elles ne diffèrent que par le schéma :
// leurs appelants n'ont pas le même paramètre d'échappement (`?plateforme=` est
// une URL http, `?signaling=` une URL ws), et une fonction unique rendant les
// deux ne saurait pas quoi faire d'un explicite qui n'en couvre qu'un.

/// Ce dont ce module a besoin de `location`, et rien de plus.
///
/// ⚠️ `host` ET NON `hostname` : `host` porte LE PORT DE LA PAGE quand il n'est
/// pas celui du schéma. C'est précisément ce que le code d'avant jetait pour le
/// remplacer par 8080.
export interface Emplacement {
    protocol: string;
    host: string;
}

/// L'adresse HTTP de la plateforme : `?plateforme=` s'il est posé, sinon
/// l'origine de la page.
export function adressePlateforme(
    emplacement: Emplacement,
    explicite?: string | null,
): string {
    if (explicite) return explicite;
    return `${emplacement.protocol === 'https:' ? 'https' : 'http'}://${emplacement.host}`;
}

/// Le chemin du relais sur le service. Voir `plateforme/src/http/serveur.ts`.
const CHEMIN_SIGNAL = '/signal';

/// L'adresse WebSocket du signaling : `?signaling=` s'il est posé, sinon
/// l'origine de la page, **avec le schéma qui correspond au sien**.
///
/// 🔴 C'EST LA CORRESPONDANCE DES SCHÉMAS QUI COMPTE : `https:` ⇒ `wss:`, et
/// jamais `ws:`. Un `ws:` depuis une page `https:` est du contenu mixte, et le
/// navigateur le refuse.
export function adresseSignaling(
    emplacement: Emplacement,
    explicite?: string | null,
): string {
    // ⚠️ UN EXPLICITE RESTE EXPLICITE, ET NE REÇOIT PAS LE SUFFIXE. C'est le
    // contrat de ce paramètre depuis P5 : il remplace l'adresse ENTIÈRE, pas
    // son autorité. Y ajouter `/signal` ferait `/signal/signal` chez qui l'a
    // déjà écrit, et personne ne saurait lequel des deux comportements est le
    // bon. CONSÉQUENCE À CONNAÎTRE : une recette qui pose `?signaling=` doit
    // désormais écrire le chemin.
    //
    // 🔴 CE COMMENTAIRE A AFFIRMÉ « AUCUNE N'EN POSE », ET C'ÉTAIT FAUX — la
    // PORTÉE de son `grep` ne couvrait pas ce que la phrase prétendait couvrir.
    // Il cherchait dans `client/*.mjs client/recette/*.mjs scripts/*.sh`, où il
    // n'y a effectivement rien ; **mais TOUS les pilotes de recette de ce dépôt
    // vivent dans `docs/superpowers/plans/journaux-*/`**, que la commande
    // n'atteignait pas. C'est le patron du « naufrage du 487 » de `CLAUDE.md` :
    // une affirmation de complétude dont la commande n'a pas balayé son propre
    // périmètre.
    //
    // 🔴 LE RELEVÉ JUSTE, 21 août 2026, sur les fichiers SUIVIS PAR GIT :
    //
    //     git grep -n 'signaling=' -- 'docs/superpowers/plans/journaux-*'
    //
    // **ONZE fichiers sous `journaux-*/instrument/` posent `?signaling=`** —
    // les pilotes de `accent-a1`, `micro-e3` (le pilote et son
    // `injection-e3.js`), `pont-fichiers` f1 à f5, et `presse-papier` p1 à p3 —,
    // **plus un DOUZIÈME hors de ce répertoire**,
    // `journaux-micro-e2/pilote-recette-e2.mjs`. Les douze passent une URL
    // **sans chemin** (`ws://192.168.3.1:8080`, `ws://<hôte>:8090`, ou l'URL de
    // la plateforme avec `http` -> `ws`), donc visent la racine `/` — **que ce
    // chantier vient de fermer**.
    //
    // ⚠️ ILS SONT DONC TOUS À RÉPARER, ET AUCUN NE L'A ÉTÉ ICI : c'est un
    // chantier à part, inscrit aux « Legs ouverts » de `CLAUDE.md`. ⚠️ Le
    // `grep` ci-dessus attrape aussi les `.log` et `.json` de sortie, qui sont
    // des RELEVÉS et non des pilotes : la règle de sélection est « les sources
    // de pilote », pas « toute occurrence ».
    if (explicite) return explicite;
    const schema = emplacement.protocol === 'https:' ? 'wss' : 'ws';
    return `${schema}://${emplacement.host}${CHEMIN_SIGNAL}`;
}

// ⚠️ LE TEST DU SCHÉMA EST UNE ÉGALITÉ À `'https:'`, PAS UNE ABSENCE DE
// `'http:'`. La différence se voit sur `file:`, qui arrive quand on ouvre le
// HTML depuis le disque : l'égalité le fait retomber sur le clair — inutile,
// mais lisible —, là où une négation aurait produit `wss://` sur une page
// locale, dont le message d'erreur n'aurait désigné aucune cause.
//
// ⚠️ UN PARAMÈTRE VIDE N'EST PAS UN PARAMÈTRE. `URLSearchParams.get` rend `''`
// sur `?signaling=` et `null` sur un paramètre absent ; le `if (explicite)`
// écarte les deux. Traiter `''` comme explicite produirait une adresse vide,
// donc une panne sans message.
