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
// fenêtre à se reconnecter. C'est un ARBITRAGE, pas un oubli.
//
// ⚠️ **P5 EST PASSÉ, ET L'ARBITRAGE N'A PAS ÉTÉ ROUVERT** (revue transverse,
// 20 août 2026). Cette phrase annonçait qu'« il se rouvrira[it] [au] sous-bloc
// P5, avec les en-têtes de sécurité » : les en-têtes ont été livrés — deux par
// le service (`plateforme/src/http/entetes.ts`), le reste par le proxy
// (`deploiement/nginx.conf`, dont une CSP à `script-src 'self'`) — et **le
// stockage n'a pas changé**. Ce n'est pas un oubli non plus : une CSP réduit
// la surface d'injection sans la supprimer, et le remède réel — un cookie
// `HttpOnly` — reste **hors du périmètre de ⑤**, qui n'a livré aucun cookie.
// **L'arbitrage tient, et il est désormais DÛ plutôt qu'ANNONCÉ.**
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

/// Pose le seul jeton d'accès, et EFFACE celui de rafraîchissement.
///
/// 🔴 L'EFFACEMENT EST LE POINT, PAS UN NETTOYAGE DE CONFORT. Le mode
/// `pomerium` ne délivre aucun jeton de rafraîchissement. Un jeton laissé par
/// un montage `motdepasse` antérieur serait présenté à une route qui rend
/// désormais 404, et le symptôme serait une déconnexion inexpliquée dix minutes
/// après chaque ouverture de page.
///
/// ⚠️ CE QUE FAIT LE PRODUIT À L'EXPIRATION, ET NON CE QU'ON VOUDRAIT QU'IL
/// FASSE. Ce commentaire a écrit « à l'expiration, le client rappelle
/// `GET /auth/moi` » : **aucun code ne le fait**, et la revue transverse du
/// chantier `auth-pomerium` l'a relevé. `rafraichirSiNecessaire` (plus bas)
/// n'a **aucun appelant de production** :
///
///     grep -rn 'rafraichirSiNecessaire(' client/src --include='*.ts' | grep -v '///'
///
/// rend CINQ lignes — une définition et quatre usages de test, pas un appel.
/// ⚠️ LE SECOND `grep` N'EST PAS DÉCORATIF : sans lui, la commande compte LES
/// LIGNES DE CE COMMENTAIRE, et le chiffre annoncé cesse d'être celui qu'elle
/// rend. La vague de correction du 21 août 2026 a payé ce patron QUATRE fois
/// dans la même ronde — un `grep` cité s'ancre sur la syntaxe, jamais sur un
/// nom que la prose environnante répète. Et `tenterPomerium`
/// (`connexion.ts`) ne court **qu'au chargement de la page de connexion**. Ce
/// qui se passe réellement en mode `pomerium` : le jeton expire, la poignée de
/// main suivante est refusée, et l'utilisateur RECHARGE la page — c'est ce
/// rechargement, et lui seul, qui rappelle `/auth/moi`. Le cookie Pomerium
/// vivant 8640 h, ce rechargement est silencieux pour lui ; il n'en reste pas
/// moins un geste, pas un rafraîchissement automatique.
export function poserAcces(coffre: Coffre, acces: string): void {
    coffre.setItem(CLE_ACCES, acces);
    coffre.removeItem(CLE_RAFRAICHISSEMENT);
}

/// Le jeton d'accès porté par le corps de `GET /auth/moi`, ou `undefined` si
/// ce corps n'en porte pas d'utilisable.
///
/// 🔴 C'EST UNE RÈGLE, PAS DU CÂBLAGE, ET C'EST POURQUOI ELLE VIT ICI ET NON
/// DANS `connexion.ts`. Le critère de ce dépôt est reproductible — « une
/// condition est une règle si la CHANGER change ce que le PRODUIT décide ; elle
/// est du câblage si elle ne fait que router une décision déjà prise ailleurs,
/// et testée là-bas ». Celle-ci ne route RIEN : elle VALIDE une valeur que le
/// service est contractuellement tenu de fournir, et personne d'autre ne la
/// valide. **Ce qu'un retrait produit, mesuré plutôt que supposé** : le corps
/// `{}` fait écrire la chaîne `"undefined"` au coffre, puis envoyer
/// `Bearer undefined` à `POST /session`, puis afficher une erreur de session
/// au lieu du formulaire de connexion — **et le coffre reste empoisonné** pour
/// tous les chargements suivants. Le produit décide autre chose ; c'est donc
/// bien une règle, et elle est tenue par les tests de ce fichier.
///
/// 🔴 LA CHAÎNE VIDE EST REFUSÉE SÉPARÉMENT DU NON-CHAÎNE, et le test de la
/// chaîne vide n'est pas redondant : `typeof '' === 'string'`. C'est le même
/// piège que `plateforme/src/config.ts` a payé — `env.X ?? 'defaut'` ne
/// rattrape pas `''`. Un `''` posé au coffre serait un jeton qu'aucun
/// `Authorization` ne peut porter, et `jetonAcces` le rendrait comme s'il
/// valait quelque chose.
///
/// ⚠️ PURE, ET SANS COFFRE : elle ne pose rien elle-même. Poser est le geste de
/// `poserAcces` juste au-dessus, et les garder distincts est ce qui permet à
/// l'appelant de ne RIEN toucher quand la réponse est mauvaise.
export function accesDeReponse(corps: unknown): string | undefined {
    if (typeof corps !== 'object' || corps === null) return undefined;
    const acces = (corps as { acces?: unknown }).acces;
    if (typeof acces !== 'string' || acces === '') return undefined;
    return acces;
}

export function jetonAcces(coffre: Coffre | undefined = coffreParDefaut()): string | undefined {
    return coffre?.getItem(CLE_ACCES) ?? undefined;
}

/* ── L'ACCÈS AUTOMATIQUE — AJOUTÉ LE 30 AOÛT 2026, POUR FERMER UNE
   INCOMPLÉTUDE TROUVÉE EN PRODUCTION CE MATIN-LÀ ─────────────────────────

   Le hub (`hub/page.ts`), servi à la racine depuis la veille, se contentait
   de LIRE le coffre et de se plaindre s'il était vide ("Aucun jeton :
   connectez-vous d'abord.", sans rien à faire). Le seul code qui savait
   obtenir un jeton par Pomerium était `connexion.ts::tenterPomerium`, et il
   ne courait QU'AU CHARGEMENT DE LA PAGE DE CONNEXION. Tant que la racine
   servait la page de session, personne n'avait vu un visiteur atterrir
   DIRECTEMENT sur le hub sans être passé par cet écran : le lot qui a mis le
   hub à la racine avait vérifié que `/` SERT le hub, jamais qu'un visiteur
   SANS JETON puisse s'en servir — un contrôle qu'on n'a jamais vu rougir.

   Les deux fonctions ci-dessous DESCENDENT ici, où elles sont testées, pour
   que `connexion.ts` (qui appelle toujours Pomerium au chargement) ET
   `hub/page.ts` (qui ne doit l'appeler QUE si le coffre est vide) les
   PARTAGENT au lieu de la recopier — la clause que ce correctif s'impose. */

export interface ReponseAuthMoi {
    ok: boolean;
    json(): Promise<unknown>;
}
export type AppelAuthMoi = (url: string) => Promise<ReponseAuthMoi>;

/// Demande un jeton d'accès frais à Pomerium — le CHEMIN qu'avait
/// `tenterPomerium` (`connexion.ts`), EXTRAIT ici tel quel (mêmes trois
/// gestes : appeler, vérifier `ok`, valider le corps par `accesDeReponse`).
///
/// Rend `undefined` sur toute issue qui n'est PAS un jeton exploitable : un
/// réseau injoignable, un corps illisible, et — le cas du mode
/// `motdepasse` — un `404`, que `routes-identite.ts::servirIdentite` rend
/// LUI-MÊME pour porter le mode jusqu'au client (voir l'en-tête de
/// `connexion.ts` autour de `tenterPomerium`). Cette fonction ne distingue
/// PAS ces issues entre elles : c'est à l'APPELANT de décider quoi en faire
/// (rediriger vers l'écran de connexion, par exemple), jamais à elle de
/// choisir à sa place.
export async function accesParPomerium(
    base: string,
    appel: AppelAuthMoi,
): Promise<string | undefined> {
    try {
        const reponse = await appel(`${base}/auth/moi`);
        if (!reponse.ok) return undefined;
        return accesDeReponse(await reponse.json().catch(() => undefined));
    } catch {
        return undefined;
    }
}

/// Assure qu'un jeton d'accès est disponible, EN L'OBTENANT SI BESOIN.
///
/// Rend le jeton du coffre s'il y en a déjà un — SANS appeler le réseau : un
/// aller-retour à chaque ouverture de page serait un coût pour un cas qui
/// n'en a pas besoin. Sinon, tente `accesParPomerium` et, s'il aboutit, POSE
/// le jeton obtenu (`poserAcces`) avant de le rendre — c'est ce qui rend un
/// rechargement ultérieur du hub gratuit, comme il l'est déjà pour la page de
/// connexion.
///
/// Rend `undefined` quand aucun jeton n'a pu être obtenu par AUCUNE des deux
/// voies : c'est le signal, pour l'appelant, qu'il doit renvoyer vers l'écran
/// de connexion plutôt que de montrer une page vide ou un message qui ne dit
/// pas quoi faire — la règle que `connexion.ts` s'impose déjà pour ses
/// propres échecs.
export async function assurerAcces(
    coffre: Coffre,
    base: string,
    appel: AppelAuthMoi,
): Promise<string | undefined> {
    const existant = jetonAcces(coffre);
    if (existant !== undefined) return existant;
    const frais = await accesParPomerium(base, appel);
    if (frais === undefined) return undefined;
    poserAcces(coffre, frais);
    return frais;
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
