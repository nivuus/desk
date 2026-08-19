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
// ⚠️ LA SOURCE DÉFINITIVE DU PRÉFIXE EST LA PLATEFORME, ET ELLE N'EXISTE PAS
// ENCORE. La spec §3.4 annonce « reçu de la plateforme » pour cette page,
// mais la route qui le délivrerait — « l'utilisateur demande une session → la
// plateforme vérifie l'attribution et la fraîcheur de l'agent → elle rend
// l'identifiant de session, le préfixe et la configuration ICE » — est
// assignée au sous-bloc P4. P3 transforme donc le littéral en PARAMÈTRE ; P4
// branchera la source, et n'aura qu'à écrire dans le coffre.
//
// ⚠️ LE COÛT EST CELUI QUE LA SPEC §10 NOMME ELLE-MÊME : « une plateforme mal
// configurée retombe silencieusement dans un espace de noms partagé ». Il est
// accepté pour la même raison — la garde refuse l'agent sans enrôlement, donc
// aucune session ne s'établit du tout, et le silence n'est jamais celui d'une
// session qui marcherait à moitié.

export interface Coffre {
    getItem(cle: string): string | null;
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

/// Compose un identifiant de session : `<préfixe>:<nom>`, ou `<nom>` seul
/// quand aucun préfixe n'est connu.
export function composer(prefixe: string, nom: string): string {
    if (prefixe === '') return nom;
    return `${prefixe}${SEPARATEUR}${nom}`;
}
