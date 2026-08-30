// L'URL SIGNÉE d'une icône d'application. Module PUR : aucune base, aucun DOM,
// aucune horloge lue ici — `maintenant` est un PARAMÈTRE, comme dans
// `identite/jeton.ts` et `depot/session.ts`. C'est ce qui rend l'expiration
// éprouvable sur trois instants distincts au lieu d'être inerte.
//
// 🔴 DÉCISION DU PROPRIÉTAIRE DU DÉPÔT, PRISE LE 30 AOÛT 2026 — CE N'EST PAS
// UNE COMMODITÉ D'IMPLÉMENTATION. Le legs ouvert depuis le sous-bloc G5
// disait : « un `<img src>` ne porte pas d'en-tête `Authorization` », donc
// `GET /application/:id/icone` était inatteignable par une balise d'image, et
// le hub non installable faute d'icône chargeable par le navigateur. G5 a
// explicitement laissé la décision au propriétaire, parce que c'est une
// décision de SÉCURITÉ et non un choix d'écriture. Il a tranché : **URL
// signée**, et il a ÉCARTÉ NOMMÉMENT les deux autres voies —
//   ① servir les icônes SANS jeton : révélerait la liste des applications
//      installées sur la VM à quiconque atteint le port ;
//   ② les inliner en `data:` dans le catalogue : support inégal des `data:`
//      dans un manifeste PWA.
//
// 🔴 CE QUE CE MÉCANISME EST, DIT SANS ENJOLIVER : une CAPACITÉ AU PORTEUR.
// Qui détient l'URL peut lire l'icône, sans s'identifier, jusqu'à son
// expiration. C'est exactement ce qu'on lui demande — un `<img src>` ne peut
// rien porter d'autre que son URL — et c'est ce qui borne sa portée : elle ne
// vaut QUE pour une icône, QUE pour une application, QUE pour une VM, et QUE
// pendant `DUREE_URL_ICONE_MS`. Elle n'ouvre ni le catalogue, ni le
// lancement, ni la session.

import { createHmac, hkdfSync, timingSafeEqual } from 'node:crypto';

/* ── LA CLÉ ───────────────────────────────────────────────────────────── */

/// L'étiquette de dérivation. ⚠️ ELLE FAIT PARTIE DU CONTRAT : la changer
/// invalide toutes les URL en vol, ce qui est sans conséquence (elles vivent
/// cinq minutes) mais doit être VOULU.
const ETIQUETTE_DERIVATION = 'nivuus-desk/url-icone/v1';

/// Le sel HKDF. ⚠️ IL N'EST PAS SECRET, et il n'a pas à l'être : HKDF-Extract
/// admet un sel public — c'est le matériel d'entrée qui porte l'entropie.
/// Il est FIXE parce qu'un sel aléatoire par démarrage rendrait toute URL
/// invalide au redémarrage du service, ce qui est le défaut exact qu'un
/// secret de jeton aléatoire aurait produit sur les sessions
/// (`config.ts`, ligne du secret de jeton).
const SEL_DERIVATION = 'nivuus-desk/sel/url-icone';

/// 🔴 POURQUOI UNE SOUS-CLÉ ET NON LE SECRET DE JETON LUI-MÊME.
/// Le secret de jeton de la plateforme signe déjà les JWT de session ET les
/// jetons d'agent. Employer la MÊME clé pour un troisième usage est la faute
/// classique de réutilisation de clé : deux mécanismes qui signent des
/// messages de formats différents avec la même clé s'exposent l'un l'autre le
/// jour où l'un accepte un message que l'autre a produit. Ici la menace est
/// concrète et pas théorique — les deux formats sont voisins, tous deux en
/// `base64url`, tous deux HMAC-SHA256 — et la parade coûte une ligne.
///
/// La dérivation est HKDF-SHA256 (`node:crypto`, aucune dépendance ajoutée —
/// la contrainte d'`base/pilote.test.ts`). La sous-clé fait 32 octets, la
/// taille de bloc de sortie de SHA-256.
///
/// ⚠️ ELLE EST DÉRIVÉE À CHAQUE APPEL, ET C'EST ASSUMÉ : `hkdfSync` sur 32
/// octets est deux HMAC, c'est-à-dire moins cher que la lecture de base qui
/// suit. Un cache mémoïsé ferait vivre une clé dans un état global, que les
/// tests devraient alors savoir vider.
export function sousCleIcone(secretJeton: string): Buffer {
    return Buffer.from(hkdfSync('sha256', secretJeton, SEL_DERIVATION, ETIQUETTE_DERIVATION, 32));
}

/* ── LA DURÉE ─────────────────────────────────────────────────────────── */

/// **CINQ MINUTES.**
///
/// 🔴 SA RAISON, ÉCRITE PLUTÔT QUE SUPPOSÉE — ce dépôt porte un legs entier
/// sur les constantes non calibrées, et en ajouter une en silence l'aggrave.
///
///   ① **Le plancher.** Une page de hub charge quarante icônes d'un coup ;
///      toutes les URL sont frappées dans la seconde qui suit la lecture du
///      catalogue. Cinq minutes laissent aussi survivre un onglet resté
///      ouvert quelques minutes avant que l'utilisateur ne le regarde, et une
///      image rechargée par le navigateur après un retour d'arrière-plan.
///
///   ② **Le plafond, et c'est LUI qui fixe la valeur.** L'URL est frappée
///      AVEC le jeton porteur, par le catalogue, qui est authentifié. Elle ne
///      doit donc jamais survivre au jeton qui l'a fait naître :
///      `DUREE_JETON_ACCES_MS` vaut 600 000 ms (`identite/jeton.ts`), et
///      300 000 en est la moitié franche. Une URL qui vivrait plus longtemps
///      que le jeton serait une capacité qui SURVIT à la session — exactement
///      ce qu'une durée courte doit empêcher.
///
/// ⚠️ **ELLE N'EST PAS CALIBRÉE POUR AUTANT** : aucune mesure de charge, aucun
/// relevé de navigateur ne l'a jugée. Elle rejoint la liste des constantes non
/// calibrées de `CLAUDE.md`, et son rapport de 1/2 au jeton d'accès est un
/// ARBITRAGE, pas un résultat. ⚠️ Elle et `DUREE_JETON_ACCES_MS` se
/// recalibrent ENSEMBLE : baisser le jeton sous cinq minutes rendrait cette
/// borne fausse sans qu'aucun test ne le dise, et c'est pourquoi un test
/// l'épingle nommément (`url-icone.test.ts`).
export const DUREE_URL_ICONE_MS = 300_000;

/// **UNE MINUTE** — le PAS auquel l'expiration est arrondie vers le haut.
///
/// 🔴 IL EXISTE POUR QUE `Cache-Control: immutable` GARDE UN SENS, ET SANS LUI
/// CE LOT AURAIT DÉTRUIT LE CACHE QU'IL PRÉTEND SERVIR. La réponse d'icône
/// porte `max-age=31536000, immutable` (`http/routes-icone.ts`) : elle est
/// adressée par contenu, et la mettre en cache est tout l'objet de la route.
/// Mais une URL SIGNÉE change à chaque frappe — `x` et `s` en font partie —,
/// donc la clé de cache changerait à chaque lecture du catalogue et AUCUNE
/// entrée ne serait jamais relue. On remplirait le cache d'entrées mortes.
///
/// En arrondissant l'expiration au pas supérieur, toutes les URL frappées
/// dans la même minute sont IDENTIQUES, octet pour octet : un rechargement de
/// page dans cette minute retombe sur le cache.
///
/// ⚠️ CE QU'IL COÛTE, DIT PLUTÔT QUE TU : la durée de vie réelle est comprise
/// entre `DUREE_URL_ICONE_MS` et `DUREE_URL_ICONE_MS + PAS_URL_ICONE_MS`,
/// c'est-à-dire entre 5 et 6 minutes. **Le PLANCHER est garanti** — c'est le
/// sens de l'arrondi vers le HAUT —, et c'est le plancher qui portait
/// l'exigence « quarante icônes ne doivent pas expirer en cours de route ».
/// Le plafond reste très en dessous du jeton d'accès (10 min).
///
/// ⚠️ NON CALIBRÉE, comme sa voisine : aucune mesure de taux de succès de
/// cache ne l'a jugée.
export const PAS_URL_ICONE_MS = 60_000;

/* ── CE QUE LA SIGNATURE COUVRE ───────────────────────────────────────── */

/// Les trois champs liés, plus l'expiration.
///
/// 🔴 CHACUN EST VÉRIFIABLE CÔTÉ SERVEUR, ET C'EST LE CRITÈRE D'ADMISSION :
/// une signature qui couvrirait ce que le serveur ne peut pas recontrôler est
/// un ornement. Le détail, champ par champ :
///
///   - `application` — l'identifiant, LU DANS LE CHEMIN de la requête. C'est
///     lui qui empêche qu'une URL signée pour une application vaille pour une
///     autre.
///   - `vm` — l'identifiant de la VM. Recontrôlé contre `application.vm_id`
///     après la lecture de base. Il porte l'AUTORISATION : c'est parce que
///     l'appartenance de la VM a été vérifiée au moment de la frappe (route
///     du catalogue, jeton porteur en main) que l'URL vaut quelque chose. Le
///     lier ici fait qu'une application repointée vers une autre VM cesse
///     d'être servie par les URL déjà frappées.
///   - `expiration` — en MILLISECONDES, comme partout dans ce paquet (voir la
///     divergence déclarée en tête d'`identite/jeton.ts`).
///
/// ⚠️ CE QUE LA SIGNATURE NE COUVRE PAS, ET POURQUOI : l'identité de
/// l'UTILISATEUR. Elle serait invérifiable — la requête d'un `<img src>` ne
/// porte rien qui permette de la confronter. L'écrire dans l'URL sans pouvoir
/// la vérifier donnerait l'illusion d'un lien qui n'existe pas.
///
/// ⚠️ L'EMPREINTE DE L'ICÔNE N'EST PAS DANS CETTE STRUCTURE, ET C'EST VOULU :
/// elle n'est pas une autorisation mais une VERSION. La route la recontrôle
/// contre `application.icone`, et un `?e=` périmé rend 404 — c'est ce qui
/// rend `Cache-Control: immutable` honnête (voir `routes-icone.ts`). La
/// signer LIERAIT la capacité à une version, si bien qu'une icône mise à jour
/// invaliderait des URL déjà frappées ET déjà servies : deux mécanismes pour
/// une même chose, dont l'un ne dit rien de plus que l'autre.
export interface PorteeIcone {
    application: string;
    vm: string;
    /// ⚠️ UNE CHAÎNE, ET NON UN NOMBRE — voir `messageCanonique` juste en
    /// dessous : c'est la forme TEXTUELLE qui est signée.
    expiration: string;
}

/// La chaîne SIGNÉE, canonique et NON AMBIGUË.
///
/// 🔴 CHAQUE CHAMP EST PRÉFIXÉ DE SA LONGUEUR, ET CE N'EST PAS UNE COQUETTERIE.
/// Un simple `a|b|c` est FORGEABLE dès qu'un champ peut contenir le
/// séparateur : `application='x|y'` et `vm='z'` produiraient la même chaîne
/// que `application='x'` et `vm='y|z'`, donc la même signature — une URL
/// signée pour une paire vaudrait pour une AUTRE paire. Les identifiants
/// d'application sont des UUID engendrés par la plateforme, mais celui de la
/// VM vient de `npm run admin:agent`, donc d'un humain, donc de n'importe
/// quels caractères. Le préfixe de longueur ferme la question pour de bon,
/// quels que soient les champs de demain.
///
/// ⚠️ `v1` EN TÊTE : une version future qui couvrirait un champ de plus ne
/// pourra pas être confondue avec celle-ci, même à clé égale.
///
/// 🔴 L'EXPIRATION EST SIGNÉE DANS SA FORME TEXTUELLE EXACTE, celle qui voyage
/// dans l'URL — jamais reconvertie en nombre puis reformatée. Sans cela,
/// `x=010` et `x=10` deviendraient le MÊME message, donc la MÊME signature :
/// une seule URL frappée en vaudrait une famille entière, et le contrôle de
/// forme d'en face serait contournable en changeant l'écriture du nombre.
function messageCanonique(portee: PorteeIcone): string {
    const champ = (v: string): string => `${String(v.length)}:${v}`;
    return ['v1', champ(portee.application), champ(portee.vm), champ(portee.expiration)].join('\n');
}

/* ── FRAPPER ──────────────────────────────────────────────────────────── */

/// La signature seule, en `base64url` (43 caractères sur SHA-256).
export function signature(portee: PorteeIcone, secretJeton: string): string {
    return createHmac('sha256', sousCleIcone(secretJeton))
        .update(messageCanonique(portee), 'utf8')
        .digest('base64url');
}

/// L'URL RELATIVE d'une icône, prête à poser dans un `src`.
///
/// 🔴 RELATIVE, ET JAMAIS ABSOLUE. La plateforme ne connaît pas l'origine
/// publique sous laquelle un proxy la publie — `deploiement/nginx.conf` et
/// Pomerium en posent chacun une. Fabriquer une origine ici la ferait
/// diverger de celle de la page, et l'image serait alors soit inatteignable,
/// soit refusée par la CSP (`img-src 'self'`). Le client la résout contre
/// `location.origin`, qui est la seule origine juste par construction.
///
/// ⚠️ CHAQUE VALEUR EST ENCODÉE : l'identifiant de VM vient d'un humain.
export function signerUrlIcone(
    application: string,
    vm: string,
    empreinte: string,
    secretJeton: string,
    maintenant: number,
    dureeMs: number = DUREE_URL_ICONE_MS,
): string {
    // 🔴 ARRONDI VERS LE HAUT, JAMAIS VERS LE BAS : vers le bas, une URL
    // frappée juste avant un pas vivrait quelques millisecondes, et le
    // plancher de durée ne serait plus garanti. Voir `PAS_URL_ICONE_MS`.
    const brute = maintenant + dureeMs;
    const expiration = String(Math.ceil(brute / PAS_URL_ICONE_MS) * PAS_URL_ICONE_MS);
    const q = new URLSearchParams({
        e: empreinte,
        v: vm,
        x: expiration,
        s: signature({ application, vm, expiration }, secretJeton),
    });
    return `/application/${encodeURIComponent(application)}/icone?${q.toString()}`;
}

/* ── VÉRIFIER ─────────────────────────────────────────────────────────── */

export type MotifUrlIcone = 'parametre-absent' | 'signature-invalide' | 'url-expiree';

export type VerdictUrlIcone = { ok: true; vm: string } | { ok: false; motif: MotifUrlIcone };

/// Vérifie la signature d'une URL d'icône. Rend TOUJOURS un verdict, jamais
/// une exception : tout vient du réseau, et un jet ferait répondre 500 là où
/// il faut refuser.
///
/// 🔴 L'ORDRE DES CONTRÔLES EST CELUI DE `verifierJeton`, ET IL EST DÉLIBÉRÉ :
/// la SIGNATURE d'abord, l'EXPIRATION ensuite. Une URL forgée ET périmée doit
/// s'entendre dire « signature invalide », jamais « expirée » — sans quoi le
/// refus renseignerait un faussaire sur la moitié de son travail qui a abouti.
///
/// 🔴 L'EXPIRATION EST JUGÉE ICI, CONTRE L'HORLOGE DU SERVEUR, ET LE CHAMP `x`
/// DE L'URL N'EST CRU QUE PARCE QU'IL EST SIGNÉ. Le lire sans le signer
/// laisserait le client choisir sa propre date de péremption, c'est-à-dire
/// aucune.
///
/// 🔴 LA COMPARAISON EST EN TEMPS CONSTANT (`timingSafeEqual`), JAMAIS `===`.
/// Un `===` sur une chaîne s'arrête au premier octet différent, et la durée du
/// refus dit alors combien d'octets étaient justes — de quoi reconstruire une
/// signature octet par octet. Les LONGUEURS sont comparées d'abord : mesuré,
/// `timingSafeEqual` LÈVE quand elles diffèrent.
export function verifierUrlIcone(
    application: string,
    parametres: URLSearchParams,
    secretJeton: string,
    maintenant: number,
): VerdictUrlIcone {
    const vm = parametres.get('v');
    const brutExpiration = parametres.get('x');
    const recue = parametres.get('s');
    if (vm === null || vm === '') return { ok: false, motif: 'parametre-absent' };
    if (brutExpiration === null || brutExpiration === '') {
        return { ok: false, motif: 'parametre-absent' };
    }
    if (recue === null || recue === '') return { ok: false, motif: 'parametre-absent' };

    // La signature porte la forme TEXTUELLE de `x`, telle qu'elle est arrivée.
    const attendue = Buffer.from(
        signature({ application, vm, expiration: brutExpiration }, secretJeton),
        'utf8',
    );
    const fournie = Buffer.from(recue, 'utf8');
    if (attendue.length !== fournie.length) return { ok: false, motif: 'signature-invalide' };
    if (!timingSafeEqual(attendue, fournie)) return { ok: false, motif: 'signature-invalide' };

    // 🔴 CE GARDE VIENT APRÈS LA SIGNATURE, ET IL N'EST PAS DÉCORATIF :
    // `Number('pas-un-nombre')` rend `NaN`, et `maintenant >= NaN` est FAUX —
    // une expiration illisible serait donc ACCEPTÉE, c'est-à-dire éternelle.
    // ⚠️ IL EST INATTEIGNABLE PAR LE PRODUIT, qui ne frappe que des entiers :
    // le seul chemin qui l'atteigne est une signature calculée avec la VRAIE
    // clé sur un `x` non entier — et c'est exactement ainsi que son test le
    // fait rougir, plutôt que de le laisser vert par construction.
    const expiration = Number(brutExpiration);
    if (!Number.isInteger(expiration)) return { ok: false, motif: 'signature-invalide' };

    // Borne FRANCHE, écrite pour que le test puisse l'assiéger des deux côtés
    // — même geste que `verifierJeton`.
    if (maintenant >= expiration) return { ok: false, motif: 'url-expiree' };

    return { ok: true, vm };
}
