// Le `404` du service, à UN SEUL endroit.
//
// 🔴 POURQUOI CE MODULE EXISTE, ET CE N'EST PAS UN GOÛT DE FACTORISATION. Le
// `404` du service portait un CONTRAT documenté aux deux bouts : les deux
// gardes de mode (`routes-identite.ts` et `routes-auth.ts`) se retiraient
// pour le laisser répondre, et `client/src/connexion.ts` LIT ce `404` comme
// « ce montage authentifie par mot de passe ». Le lot « page derrière
// Pomerium » a chaîné un servant de fichiers EN DERNIER, dont le repli SPA
// résout n'importe quel chemin : le `404` que les gardes appelaient est
// devenu, en mode `motdepasse` et la page armée, un `200 text/html`. MESURÉ :
// `GET /auth/moi` rendait `200 text/html` au lieu de `404`.
//
// 🔴 LE DÉFAUT AVAIT UN JUMEAU SYMÉTRIQUE, et c'est ce qui a décidé du remède.
// `routes-auth.ts` porte la garde de polarité OPPOSÉE — elle se retire en mode
// `pomerium` —, si bien que `GET /auth/connexion` était avalé de la même façon
// dans l'autre mode. Les deux gardes PARTITIONNENT les modes : elles ont donc
// le MÊME défaut, chacune dans l'autre mode.
//
// 🔴 LE REMÈDE RETENU : LES DEUX GARDES RÉPONDENT LE `404` ELLES-MÊMES, au
// lieu de le déléguer. Les alternatives pesées, et pourquoi elles cèdent :
//   - exclure les préfixes d'API du repli SPA — il faudrait tenir une LISTE,
//     et une route d'API ajoutée sans mettre la liste à jour retomberait
//     silencieusement dans le repli : on remplacerait une panne muette par
//     une autre ;
//   - conditionner le repli à l'en-tête `Accept` — auto-maintenu, mais il
//     DIVERGE de nginx, dont le `try_files` est inconditionnel, et fait
//     dépendre le comportement d'un en-tête que le client ne contrôle pas
//     toujours ;
//   - documenter que la promesse ne tient plus — ce serait livrer sciemment
//     un mécanisme cassé.
// Répondre soi-même est chirurgical, ne demande aucune liste, et préserve
// EXACTEMENT le contrat documenté : « le 404 dit la vérité ».
//
// ⚠️ LE CORPS N'EST PAS TOUCHÉ, ET C'EST TOUT L'INTÉRÊT DE L'AVOIR ICI. Il
// vient de P1, `routes-auth.test.ts` le fige, et une SECONDE forme de 404 —
// écrite à la main dans chaque garde — dériverait de celle du serveur sans
// que rien ne le dise. Un seul texte, un seul jeu d'en-têtes, trois appelants.

import type { ServerResponse } from 'node:http';
import { ENTETES_SECURITE } from './entetes';

/// Le corps exact, conservé MOT POUR MOT depuis P1.
export const CORPS_INTROUVABLE = 'introuvable\n';

/// ⚠️ `nosniff` N'EST PAS OPTIONNEL ICI : sans `ENTETES_SECURITE`, un chemin
/// inconnu serait la SEULE réponse du service à ne pas le porter — la raison
/// pour laquelle `serveur.ts` traitait déjà son 404 lui-même plutôt que de le
/// laisser à Node.
export function repondreIntrouvable(rep: ServerResponse): void {
    rep.writeHead(404, {
        'content-type': 'text/plain; charset=utf-8',
        ...ENTETES_SECURITE,
    });
    rep.end(CORPS_INTROUVABLE);
}
