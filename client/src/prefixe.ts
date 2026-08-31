// Le préfixe de VM, côté navigateur : d'où il vient, et comment il compose un
// nom de session.
//
// 🔴 CE MODULE EST PUR ET SANS DOM, sur le précédent explicite de
// `client/src/jeton.ts` : `client/` n'a aucun `vitest.config.*`, donc
// l'environnement de test est le Node par défaut — il n'y a ni `window` ni
// `localStorage`. Le coffre et la chaîne de requête sont des PARAMÈTRES ;
// `globalThis` n'est touché que dans le défaut d'un argument, à l'appel,
// jamais au chargement du module.
//
// ✅ LA SOURCE DÉFINITIVE DU PRÉFIXE EST LA PLATEFORME, ET ELLE EXISTE DEPUIS
// LE SOUS-BLOC P4. `client/src/connexion.ts` appelle `POST /session`
// (`plateforme/src/http/routes-session.ts`) une fois le jeton posé, et écrit
// ici par `poserPrefixe`. P3 avait transformé le littéral en PARAMÈTRE et
// laissé la source à P4 ; c'est fait, et ce module n'a eu qu'à gagner ses deux
// écrivains. `lirePrefixe` n'a pas changé d'une ligne.
//
// ⚠️ LE COÛT QUE LA SPEC §10 NOMME — « une plateforme mal configurée retombe
// silencieusement dans un espace de noms partagé » — EST RÉDUIT, PAS SOLDÉ, et
// il faut dire par quoi. Ce qui l'empêche désormais de passer inaperçu tient en
// deux gardes, aux deux bouts : `poserPrefixe` LÈVE sur la chaîne vide plutôt
// que de la coucher au coffre, et la route rend 409 `aucune-vm` au lieu d'un
// 200 à préfixe vide. Ce qui reste : le préfixe est PAR VM et non par session,
// et `signaling/propriete.ts` reste en mémoire — deux clients humains de la
// même VM retrouvent le même préfixe après un redémarrage du service.

/// Ce dont la LECTURE a besoin, et rien de plus.
export interface Coffre {
    getItem(cle: string): string | null;
}

/// Ce dont l'ÉCRITURE a besoin.
///
/// ⚠️ DEUX INTERFACES PLUTÔT QU'UNE, contrairement à `jeton.ts` qui n'en a
/// qu'une : élargir `Coffre` interdirait de passer à `lirePrefixe` une vue en
/// lecture seule, sans rien lui apporter. `window.localStorage` satisfait les
/// deux, et c'est le seul appelant de production.
export interface CoffreEcrivable extends Coffre {
    setItem(cle: string, valeur: string): void;
    removeItem(cle: string): void;
}

export const CLE_PREFIXE = 'guac.prefixe';

/// Le séparateur, tel que la spec §3.4 l'écrit. Miroir de
/// `SEPARATEUR_PREFIXE` dans `agent/src/superviseur/protocole.rs` : les deux
/// bouts composent le MÊME identifiant, et une divergence ne se verrait qu'en
/// session réelle.
export const SEPARATEUR = ':';

function coffreParDefaut(): Coffre | undefined {
    const global = globalThis as { localStorage?: Coffre };
    return global.localStorage;
}

function requeteParDefaut(): string {
    const global = globalThis as { location?: { search?: string } };
    return global.location?.search ?? '';
}

/// Le préfixe de la VM : celui du coffre, sinon celui de la chaîne de
/// requête, sinon la chaîne vide.
///
/// 🔴 LE COFFRE PASSE AVANT LA REQUÊTE. Dans l'autre ordre, un `?prefixe=`
/// resté dans une URL mise en favori écraserait à chaque rechargement le
/// préfixe que la plateforme a posé, et la page ouvrirait les sessions d'une
/// autre VM.
///
/// 🔴 L'ABSENCE REND LA CHAÎNE VIDE, jamais `undefined` et jamais une
/// exception : la page doit retomber sur `bureau`, ce que la spec §10 pose
/// déjà comme règle.
export function lirePrefixe(
    coffre: Coffre | undefined = coffreParDefaut(),
    requete: string = requeteParDefaut(),
): string {
    const duCoffre = coffre?.getItem(CLE_PREFIXE);
    if (duCoffre !== null && duCoffre !== undefined && duCoffre !== '') return duCoffre;
    return new URLSearchParams(requete).get('prefixe') ?? '';
}

/// Écrit le préfixe que la plateforme vient de délivrer.
///
/// 🔴 LÈVE SUR LA CHAÎNE VIDE, ET C'EST LA GARDE, PAS UNE VÉRIFICATION DE
/// POLITESSE. Un préfixe vide écrit au coffre n'y serait pas neutre :
/// `lirePrefixe` le traite comme une absence (l. `duCoffre !== ''`),
/// retomberait sur `?prefixe=` puis sur `''`, et la page rejoindrait
/// SILENCIEUSEMENT l'espace de noms partagé — la panne muette exacte que la
/// spec §10 nomme. Un appelant qui n'a pas de préfixe n'en a pas à écrire :
/// il appelle `effacerPrefixe`.
///
/// ⚠️ ELLE LÈVE PLUTÔT QUE DE RENDRE UN `boolean` : le seul appelant est du
/// câblage (`connexion.ts`), et un booléen ignoré y serait indiscernable d'un
/// succès. C'est le même argument que `orchestration/refus.ts` fait valoir en
/// sens inverse — là-bas un refus ATTENDU se rend en valeur, ici une
/// programmation FAUSSE se dit en exception.
export function poserPrefixe(coffre: CoffreEcrivable, prefixe: string): void {
    if (prefixe === '') {
        throw new Error(
            'préfixe vide refusé : le coucher au coffre ferait rejoindre ' +
                "l'espace de noms partagé sans que rien ne le dise (spec §10). " +
                'Une absence de VM s\'écrit par effacerPrefixe.',
        );
    }
    coffre.setItem(CLE_PREFIXE, prefixe);
}

/// Retire le préfixe. Appelée quand l'utilisateur n'a AUCUNE VM.
///
/// 🔴 NE PAS L'APPELER LAISSERAIT LE PRÉFIXE D'UNE VM QU'ON N'A PLUS, et la
/// page ouvrirait ses sessions au nom d'une autre machine — le coffre passant
/// avant la chaîne de requête, rien ne le corrigerait. C'est aussi ce qui rend
/// au mode d'essai local son `?prefixe=` : tant que le coffre porte quelque
/// chose, la requête ne sert à rien.
export function effacerPrefixe(coffre: CoffreEcrivable): void {
    coffre.removeItem(CLE_PREFIXE);
}

/// Ce qu'il faut faire du préfixe qu'une VM vient d'annoncer.
///
/// 🔴 **CETTE RÈGLE EXISTE PARCE QUE LE HUB NE POSAIT AUCUN PRÉFIXE** (revue
/// finale du 31 août 2026, critique ②). `poserPrefixe` n'avait qu'UN SEUL
/// appelant de production — `connexion.ts::chercherLaSession` —, qui ne court
/// que sur la page de connexion. Or un visiteur derrière Pomerium obtient son
/// jeton **sur le hub** (`jeton.ts::assurerAccesFrais` → `/auth/moi`) sans
/// jamais passer par cet écran : `lirePrefixe()` rendait alors `''`, le hub
/// écoutait la session `bureau` pendant que l'agent annonçait sur
/// `<prefixe>:bureau`, et **aucun `fenetre-ouverte` n'arrivait jamais**. Le
/// verrou d'élection, lui non plus préfixé, rendait vacueuse la protection que
/// `bureau/porteur-dom.ts::nomDuVerrou` déclare bruyamment offrir.
/// ⚠️ **Le défaut PRÉEXISTE au chantier `navigation-hub-unique`** — il date du
/// correctif Pomerium du 30 août 2026, et `shell-page.ts` en souffrait aussi.
/// Il devient critique parce que le hub est devenu la SEULE surface.
///
/// 🔴 **UNE RÈGLE, PAS DU CÂBLAGE**, au critère reproductible de ce dépôt : la
/// changer change ce que le produit DÉCIDE (quelle session il écoute), elle ne
/// route pas une décision prise ailleurs.
///
/// ⚠️ **LA CHAÎNE VIDE VAUT « EFFACER », ELLE NE LÈVE PAS.** `poserPrefixe`
/// lève sur `''`, et c'est juste pour LUI : un appelant qui n'a pas de préfixe
/// n'en a pas à écrire. Mais un service qui annoncerait `prefixe: ''` n'est pas
/// une programmation fausse du client — c'est une VM sans préfixe, et le geste
/// correct est d'effacer, jamais de faire lever le peuplement du catalogue.
/// C'est ce qui distingue cette règle de la garde de `poserPrefixe`, et les
/// deux sont écrites côte à côte pour qu'on ne les confonde pas.
export type ChoixPrefixe = { action: 'poser'; prefixe: string } | { action: 'effacer' };

export function prefixeDeLaVm(annonce: unknown): ChoixPrefixe {
    if (typeof annonce === 'string' && annonce !== '') return { action: 'poser', prefixe: annonce };
    return { action: 'effacer' };
}

/// Applique le choix ci-dessus au coffre. **Du câblage**, gardé ici pour que
/// les deux appelants (`connexion.ts` un jour, `hub/page.ts` aujourd'hui) ne
/// recopient pas le `if`.
export function retenirLePrefixe(coffre: CoffreEcrivable, annonce: unknown): void {
    const choix = prefixeDeLaVm(annonce);
    if (choix.action === 'poser') poserPrefixe(coffre, choix.prefixe);
    else effacerPrefixe(coffre);
}

/// Compose un identifiant de session : `<préfixe>:<nom>`, ou `<nom>` seul
/// quand aucun préfixe n'est connu.
export function composer(prefixe: string, nom: string): string {
    if (prefixe === '') return nom;
    return `${prefixe}${SEPARATEUR}${nom}`;
}
