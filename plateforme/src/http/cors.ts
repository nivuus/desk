// Les en-têtes CORS, ou rien. Fonction PURE, aucune expression régulière, une
// égalité de chaînes.
//
// 🔴 POURQUOI CE MODULE EXISTE, et pourquoi la spec n'en parle pas : relevé le
// 19 août 2026, `client/vite.config.ts` sert le navigateur sur 5173 quand
// `config.ts` écoute sur 8080. Un `POST` du navigateur vers `/auth/connexion`
// est donc CROSS-ORIGIN, et sans `Access-Control-Allow-Origin` le navigateur
// refuse de lire la réponse — sans qu'aucun test côté serveur ne le voie,
// puisque les tests parlent en `fetch` Node.
//
// 🔴 LE DÉFAUT EST LE REFUS : origine autorisée absente, aucun en-tête. Et la
// valeur `*` n'est produite SOUS AUCUNE CONDITION — un joker autoriserait
// n'importe quel site à parler à cette API au nom du navigateur d'un
// utilisateur connecté.
//
// ⚠️ L'origine demandée est COMPARÉE, jamais renvoyée telle quelle : renvoyer
// l'`Origin` du demandeur revient à autoriser tout le monde en le disant d'une
// autre façon. Et la comparaison est une ÉGALITÉ, jamais un préfixe — un
// `startsWith` accepterait `http://127.0.0.1:5173.attaquant.test`.

export function entetesCors(
    origineDemandee: string | undefined,
    origineAutorisee: string | undefined,
): Record<string, string> | undefined {
    if (origineAutorisee === undefined || origineAutorisee === '') return undefined;
    if (origineDemandee === undefined || origineDemandee === '') return undefined;
    if (origineDemandee !== origineAutorisee) return undefined;
    // Jamais `*` : la valeur rendue est l'origine CONFIGURÉE, dont on vient de
    // vérifier qu'elle est aussi celle demandée.
    if (origineAutorisee === '*') return undefined;
    return {
        'Access-Control-Allow-Origin': origineAutorisee,
        // Sans `Vary`, un cache intermédiaire servirait la réponse d'une
        // origine à une autre.
        Vary: 'Origin',
        // `GET` depuis P4 : `GET /vm` est la première route de ce service que
        // le navigateur atteigne autrement qu'en `POST`.
        //
        // ✅ ET G1 N'A RIEN EU À CHANGER ICI : `GET /applications` est le
        // second consommateur de la même valeur, et le sous-bloc l'a trouvée
        // déjà posée. La modification que son plan prescrivait était donc
        // idempotente, et sa « rouge gratuite » n'était plus jouable — P4
        // l'avait jouée, et son test l'annonçait en toutes lettres
        // (`cors.test.ts`). Relevé plutôt que supposé fait.
        'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
        // 🔴 `authorization` DEPUIS P4, ET SANS LUI RIEN N'EST ATTEIGNABLE.
        // Les deux routes de P4 — et les deux de G1 — exigent
        // `Authorization: Bearer`
        // (`http/porteur.ts`), et cet en-tête rend la requête NON SIMPLE : le
        // navigateur envoie une requête préalable portant
        // `Access-Control-Request-Headers: authorization`, qu'un serveur ne
        // répondant que `content-type` refuse. ⚠️ Comme tout ce que ce fichier
        // règle, AUCUN test Node ne peut le voir — voir l'en-tête : les tests
        // parlent en `fetch` Node, qui n'applique pas la politique d'origine.
        // La garde est l'assertion de `cors.test.ts`, et rien d'autre.
        'Access-Control-Allow-Headers': 'content-type, authorization',
    };
}
