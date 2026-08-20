// De qui vient cette requête ? Fonction PURE, aucune expression régulière sur
// le format d'une adresse, aucune lecture d'environnement.
//
// 🔴 POURQUOI CE MODULE EXISTE. Derrière un proxy inverse,
// `req.socket.remoteAddress` est l'adresse DU PROXY — la même pour tout le
// monde. Un frein par adresse fondé sur elle dégénère en frein GLOBAL : le
// service se refuse à lui-même au 51e échec, quelle que soit sa provenance.
// Et `X-Forwarded-For` est FORGEABLE par le demandeur : le croire aveuglément
// rendrait le frein contournable en une ligne d'en-tête.
//
// 🔴 LE DÉFAUT EST DE NE RIEN CROIRE. L'ensemble de confiance est VIDE quand
// `PLATEFORME_PROXY_DE_CONFIANCE` est absente — même doctrine que
// `PLATEFORME_ORIGINE_CLIENT` (`config.ts`), dont l'absence produit un refus et
// non une permission.
//
// 🔴 ET ON PREND LE DERNIER ÉLÉMENT, JAMAIS LE PREMIER. C'est la faute
// classique, et voici pourquoi c'en est une : `nginx` avec
// `$proxy_add_x_forwarded_for` AJOUTE l'adresse de son pair à ce que le client
// a envoyé. Un client qui envoie `X-Forwarded-For: 203.0.113.7` produit donc
// `203.0.113.7, <sa vraie adresse>`. Le PREMIER élément est celui que le
// client a forgé ; le DERNIER est le seul que le proxy ait écrit lui-même.
//
// ⚠️ CETTE RÈGLE SUPPOSE EXACTEMENT UN PROXY DE CONFIANCE EN TÊTE DE CHAÎNE.
// Avec deux proxies enchaînés, le dernier élément est l'adresse du PREMIER
// PROXY, pas celle du client. P5 ne livre pas la chaîne à N sauts : la
// configuration versionnée n'en pose qu'un seul, et `deploiement/nginx.conf`
// le documente. C'est une limite DÉCLARÉE, pas une omission.
//
// ⚠️ LE MODE DE DÉFAILLANCE DE L'OUBLI EST NOMMÉ, et le remède n'est PAS de
// croire l'en-tête par défaut — ce serait échanger une panne bruyante contre
// un contournement silencieux. Si l'exploitant pose un proxy sans déclarer sa
// confiance, TOUTES les requêtes portent l'adresse du proxy et le frein par
// adresse devient global. Le remède est que LA TRACE DU FREIN NOMME L'ADRESSE
// RETENUE : un exploitant qui lit `adresse=172.18.0.5` sur toutes les lignes
// reconnaît l'adresse de son proxy. Le runbook le dit aussi.

/// La valeur rendue quand le pair n'a pas d'adresse — un socket déjà fermé
/// rend `undefined` pour `remoteAddress`.
///
/// 🔴 ELLE EST NOMMÉE, ET CE N'EST PAS DE LA COQUETTERIE. Sans elle, la clé du
/// frein deviendrait la chaîne `"undefined"` par accident d'interpolation :
/// c'est exactement le piège que `signaling/turn-harnais.ts` documente pour
/// `process.env` (`"undefined"` est une chaîne VRAIE), et il se rejoue ici.
/// Une valeur nommée se lit dans une trace et se cherche dans ce fichier.
///
/// ⚠️ Tous les pairs sans adresse PARTAGENT donc un budget de frein. C'est
/// voulu : c'est le seul comportement qui ne rende pas le frein contournable
/// en fermant son socket avant que le service ne lise son adresse.
export const ADRESSE_INCONNUE = 'adresse-inconnue';

/// Le préfixe des IPv4 encapsulées en IPv6. Node rend couramment
/// `::ffff:172.18.0.5` pour un pair IPv4 sur une pile double.
const PREFIXE_MAPPE = '::ffff:';

/// ⚠️ NORMALISER EST OBLIGATOIRE DES DEUX CÔTÉS — la valeur comparée à
/// l'ensemble de confiance ET la valeur rendue. Sans quoi le même client
/// compte deux fois selon la pile employée, et son budget double ; et un proxy
/// déclaré sous sa forme nue ne serait jamais reconnu sous sa forme
/// encapsulée, ce qui ferait échouer la confiance EN SILENCE.
function normaliser(adresse: string): string {
    return adresse.startsWith(PREFIXE_MAPPE) ? adresse.slice(PREFIXE_MAPPE.length) : adresse;
}

export function adresseSource(
    remote: string | undefined,
    enteteXff: string | undefined,
    confiance: ReadonlySet<string>,
): string {
    if (remote === undefined || remote === '') return ADRESSE_INCONNUE;
    const pair = normaliser(remote);

    // L'ensemble est normalisé à la comparaison plutôt qu'à la lecture de
    // configuration : `config.ts` rend ce que l'exploitant a écrit, et c'est
    // ici — le seul lecteur — que la forme est décidée.
    let deConfiance = false;
    for (const declare of confiance) {
        if (normaliser(declare) === pair) {
            deConfiance = true;
            break;
        }
    }
    if (!deConfiance) return pair;

    if (enteteXff === undefined) return pair;
    // Le DERNIER élément NON VIDE : `203.0.113.7, ` a un dernier élément vide,
    // et le prendre rendrait une clé de frein vide que toutes les requêtes mal
    // formées partageraient.
    const elements = enteteXff.split(',');
    for (let i = elements.length - 1; i >= 0; i--) {
        const element = elements[i].trim();
        if (element !== '') return normaliser(element);
    }
    return pair;
}
