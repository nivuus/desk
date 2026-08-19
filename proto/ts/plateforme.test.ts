import { describe, expect, it } from 'vitest';
import vecteurs from '../plateforme-vectors.json';
import {
    PLATEFORME_VERSION,
    encodeEnroler,
    encodeBattement,
    parseDepuisLaPlateforme,
} from './plateforme';

/**
 * Typage local des cas : le JSON mélange des champs propres à chaque `kind`,
 * tous optionnels ici puisqu'aucun cas ne les porte tous. Même choix que
 * `input.test.ts`, pour la même raison — éviter une union imprécise sans
 * disperser des `as any`.
 */
interface CasVecteur {
    name: string;
    sens: string;
    kind: string;
    json: string;
    vm?: string;
    secret?: string;
    prefixe?: string;
    jeton?: string;
    expire_a?: number;
    motif?: string;
}

const cas: CasVecteur[] = vecteurs.cases;

describe('vecteurs partagés du canal plateforme', () => {
    it('🔴 déclare la MÊME version que le protocole', () => {
        // 🔴 La rouge : l'omettre. C'est exactement la lacune que
        // `vectors.json` traîne côté Rust — `input.rs` ne vérifie jamais
        // `doc["version"]` —, corrigée ici DES DEUX CÔTÉS pour le fichier neuf.
        expect(PLATEFORME_VERSION).toBe(vecteurs.version);
    });

    it('🔴 porte au moins un cas', () => {
        // 🔴 ANTI-TAUTOLOGIE : un fichier vide ferait passer toute la boucle
        // ci-dessous sans rien éprouver. Même garde que `input.rs:326` et que
        // `sous-ensemble.test.ts`.
        expect(cas.length).toBeGreaterThan(0);
    });

    it.each(cas.filter((c) => c.sens === 'vers'))(
        'encode « $name » exactement comme le vecteur',
        (c) => {
            const produit = c.kind === 'enroler'
                ? encodeEnroler(c.vm!, c.secret!)
                : encodeBattement();
            expect(produit).toBe(c.json);
        },
    );

    it.each(cas.filter((c) => c.sens === 'depuis'))(
        'relit « $name » et retrouve chacun de ses champs',
        (c) => {
            const lu = parseDepuisLaPlateforme(c.json) as unknown as Record<string, unknown>;
            expect(lu.type).toBe(c.kind);
            expect(lu.v).toBe(PLATEFORME_VERSION);
            // Les champs propres à chaque variante, comparés un par un : un
            // `toBe(c.kind)` seul passerait sur un message tronqué.
            for (const champ of ['prefixe', 'jeton', 'expire_a', 'motif'] as const) {
                if (c[champ] !== undefined) expect(lu[champ]).toBe(c[champ]);
            }
        },
    );

    it('exerce les DEUX sens, et aucun cas n’est sauté', () => {
        // 🔴 Sans ce compte, un `sens` mal orthographié ferait sauter des cas
        // en silence : les deux `it.each` ci-dessus rendraient simplement moins
        // de tests, et rien ne le dirait. Même garde que côté Rust.
        const vers = cas.filter((c) => c.sens === 'vers').length;
        const depuis = cas.filter((c) => c.sens === 'depuis').length;
        expect(vers).toBeGreaterThan(0);
        expect(depuis).toBeGreaterThan(0);
        expect(vers + depuis).toBe(cas.length);
    });
});

describe('miroir TypeScript du canal plateforme', () => {
    it('encode `enroler` exactement comme Rust', () => {
        // La chaîne EXACTE, comparée à celle que `plateforme.rs` asserte de
        // son côté. Une divergence d'un caractère et les deux bouts ne se
        // parlent plus.
        expect(encodeEnroler('w1', 'chut'))
            .toBe('{"type":"enroler","v":1,"vm":"w1","secret":"chut"}');
    });

    it('encode `battement`', () => {
        expect(encodeBattement()).toBe('{"type":"battement","v":1}');
    });

    it('lit un `enrole` bien formé', () => {
        const m = parseDepuisLaPlateforme(
            '{"type":"enrole","v":1,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}',
        );
        expect(m).toEqual({
            type: 'enrole', v: 1, prefixe: 'PPP', jeton: 'jjj', expire_a: 1787136773742,
        });
    });

    it('🔴 REJETTE une version PLATEFORME_VERSION + 1', () => {
        // 🔴 La rouge : ne comparer que `parsed.type`. Le message passerait, et
        // c'est la moitié TypeScript du critère ③.
        expect(() => parseDepuisLaPlateforme(
            `{"type":"refus","v":${PLATEFORME_VERSION + 1},"motif":"version"}`,
        )).toThrow(/version de plateforme non supportée/);
    });

    it('🔴 REJETTE une version ABSENTE', () => {
        // 🔴 La rouge nommée par le plan est de comparer par `!=` au lieu de
        // `!==`. ⚠️ ELLE NE ROUGIT PAS, et c'est vérifié plutôt que supposé :
        // `undefined != 1` vaut `true` en JavaScript, donc le refus tombe
        // quand même. La mutation qui rougit RÉELLEMENT ce test est
        // l'inverse — accepter l'absence, par exemple `parsed.v ?? VERSION`.
        // Le plan nous demandait de le vérifier avant de le déclarer : c'est
        // fait, et il avait raison de douter.
        expect(() => parseDepuisLaPlateforme('{"type":"refus","motif":"version"}'))
            .toThrow(/version de plateforme non supportée/);
    });

    it('🔴 REJETTE une version NULLE, que `?? ` laisserait passer', () => {
        // Complément du test précédent : `null ?? 1` vaut `1`, donc une
        // implémentation par valeur par défaut accepterait ce message-ci sans
        // que rien d'autre ne bouge.
        expect(() => parseDepuisLaPlateforme('{"type":"refus","v":null,"motif":"version"}'))
            .toThrow(/version de plateforme non supportée/);
    });

    it('REJETTE un `type` inconnu', () => {
        expect(() => parseDepuisLaPlateforme('{"type":"vol","v":1}'))
            .toThrow(/type de message de plateforme inconnu/);
    });

    it('🔴 REJETTE un `type` du sens AGENT -> PLATEFORME', () => {
        // 🔴 Le parseur ne lit QUE le sens plateforme -> agent. Accepter
        // `enroler` ici ferait qu'un agent traiterait son propre message comme
        // une réponse — une confusion de sens qu'aucun autre test ne verrait.
        expect(() => parseDepuisLaPlateforme('{"type":"enroler","v":1,"vm":"w","secret":"s"}'))
            .toThrow(/type de message de plateforme inconnu/);
    });
});
