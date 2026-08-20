import { describe, expect, it } from 'vitest';
import vecteurs from '../plateforme-vectors.json';
import {
    PLATEFORME_VERSION,
    encodeEnroler,
    encodeBattement,
    encodeEnrole,
    encodeBattementRecu,
    encodeRefus,
    parseDepuisLaPlateforme,
    parseVersLaPlateforme,
    type MotifCanal,
    type Application,
    encodeCatalogue,
    encodeLancee,
    encodeLancer,
    typesDepuis,
    typesVers,
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

    it.each(cas.filter((c) => c.sens === 'depuis'))(
        '🔴 ENCODE « $name » exactement comme le vecteur',
        (c) => {
            // 🔴 CE CONTRÔLE MANQUAIT, et son absence était une asymétrie
            // réelle : Rust sérialise les DEUX sens et compare la chaîne
            // exacte (`plateforme.rs`), quand TypeScript ne faisait que RELIRE
            // le sens `depuis`. Or c'est la PLATEFORME qui émet ces
            // messages-là, en TypeScript. Personne ne fixait donc les octets
            // qu'elle met réellement sur le fil — un ordre de champs qui
            // divergerait du Rust ne se serait vu nulle part.
            const produit =
                c.kind === 'enrole'
                    ? encodeEnrole(c.prefixe!, c.jeton!, c.expire_a!)
                    : c.kind === 'battement-recu'
                      ? encodeBattementRecu(c.jeton!, c.expire_a!)
                      : encodeRefus(c.motif as MotifCanal);
            expect(produit).toBe(c.json);
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
            .toBe('{"type":"enroler","v":2,"vm":"w1","secret":"chut"}');
    });

    it('encode `battement`', () => {
        expect(encodeBattement()).toBe('{"type":"battement","v":2}');
    });

    it('lit un `enrole` bien formé', () => {
        const m = parseDepuisLaPlateforme(
            '{"type":"enrole","v":2,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}',
        );
        expect(m).toEqual({
            type: 'enrole', v: 2, prefixe: 'PPP', jeton: 'jjj', expire_a: 1787136773742,
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
        expect(() => parseDepuisLaPlateforme('{"type":"vol","v":2}'))
            .toThrow(/type de message de plateforme inconnu/);
    });

    it('🔴 REJETTE un `type` du sens AGENT -> PLATEFORME', () => {
        // 🔴 Le parseur ne lit QUE le sens plateforme -> agent. Accepter
        // `enroler` ici ferait qu'un agent traiterait son propre message comme
        // une réponse — une confusion de sens qu'aucun autre test ne verrait.
        expect(() => parseDepuisLaPlateforme('{"type":"enroler","v":2,"vm":"w","secret":"s"}'))
            .toThrow(/type de message de plateforme inconnu/);
    });
});

describe('le parseur du sens AGENT -> PLATEFORME', () => {
    // ⚠️ CELUI-CI LIT CE QU'UN VRAI TIERS ÉCRIT. `parseDepuisLaPlateforme`
    // valide à la frontière puis caste, parce que son émetteur est le service
    // lui-même ; ici l'émetteur est un pair du réseau, qui n'a aucune raison
    // d'être bien élevé. Les champs sont donc VÉRIFIÉS, un par un.
    //
    // ⚠️ IL REND UN VERDICT, IL NE LÈVE PAS, et c'est ce qui le distingue de
    // son jumeau : la plateforme doit RÉPONDRE un motif typé au pair, pas
    // seulement échouer.

    it('lit un `enroler` bien formé', () => {
        expect(parseVersLaPlateforme('{"type":"enroler","v":2,"vm":"w1","secret":"chut"}')).toEqual({
            ok: true,
            message: { type: 'enroler', v: 2, vm: 'w1', secret: 'chut' },
        });
    });

    it('lit un `battement`', () => {
        expect(parseVersLaPlateforme('{"type":"battement","v":2}')).toEqual({
            ok: true,
            message: { type: 'battement', v: 2 },
        });
    });

    it('🔴 REJETTE une version PLATEFORME_VERSION + 1, motif `version`', () => {
        // 🔴 C'est la moitié TypeScript du critère ③ vue depuis la PLATEFORME.
        // Ne comparer que `type` laisserait entrer un message d'une version
        // future, dont les champs pourraient dire tout autre chose.
        expect(
            parseVersLaPlateforme(
                `{"type":"battement","v":${PLATEFORME_VERSION + 1}}`,
            ),
        ).toEqual({ ok: false, motif: 'version' });
        // 🔴 ET LA VERSION PASSE AVANT LE TYPE : un message qui est mauvais
        // sur les DEUX plans doit dire `version`, pas `forme`. Sans cette
        // assertion, l'ordre des deux contrôles ne serait fixé par rien, et le
        // pair qui lit le motif pour décider s'il doit se METTRE À JOUR ou se
        // CORRIGER recevrait un jour la mauvaise réponse.
        expect(parseVersLaPlateforme(`{"type":"vol","v":${PLATEFORME_VERSION + 1}}`)).toEqual({
            ok: false,
            motif: 'version',
        });
    });

    it('🔴 REJETTE une version ABSENTE et une version NULLE', () => {
        // La rouge réelle est d'écrire `parsed.v ?? PLATEFORME_VERSION` :
        // `undefined ?? 1` et `null ?? 1` valent tous deux `1`. (Le `!=` au
        // lieu de `!==` ne rougit PAS — mesuré sur l'autre parseur.)
        expect(parseVersLaPlateforme('{"type":"battement"}')).toEqual({
            ok: false,
            motif: 'version',
        });
        expect(parseVersLaPlateforme('{"type":"battement","v":null}')).toEqual({
            ok: false,
            motif: 'version',
        });
    });

    it('🔴 REJETTE un `type` du sens PLATEFORME -> AGENT, motif `forme`', () => {
        // 🔴 La confusion de sens, gardée dans les DEUX directions. Accepter
        // `enrole` ici ferait que la plateforme traiterait sa propre réponse
        // comme une demande.
        for (const brut of [
            '{"type":"enrole","v":2,"prefixe":"P","jeton":"j","expire_a":1}',
            '{"type":"battement-recu","v":2,"jeton":"j","expire_a":1}',
            '{"type":"refus","v":2,"motif":"forme"}',
        ]) {
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });

    it('REJETTE ce qui n’est pas un objet JSON, motif `forme`', () => {
        // `null` est le cas dangereux : `null.type` LÈVE, là où un nombre ou
        // une chaîne rendraient `undefined`. Même garde qu'`isJsonObject` du
        // relais, et pour la même raison — une exception non rattrapée dans un
        // gestionnaire `message` de `ws` abat tout le process Node.
        for (const brut of ['null', '"une chaîne"', '42', '[]', 'pas du json']) {
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });

    it('🔴 REJETTE un `enroler` dont `vm` ou `secret` manque, motif `forme`', () => {
        // 🔴 SANS CE CONTRÔLE, `undefined` traverserait jusqu'à la requête
        // SQL : `lireParVm(p, undefined)` ne rend rien sur SQLite mais n'est
        // pas la même requête sur Postgres, et surtout le refus qui en
        // sortirait dirait `enrolement` — donc « secret faux » — pour un
        // message qui n'a jamais porté de VM.
        for (const brut of [
            '{"type":"enroler","v":2,"secret":"chut"}',
            '{"type":"enroler","v":2,"vm":"w1"}',
            '{"type":"enroler","v":2,"vm":"","secret":"chut"}',
            '{"type":"enroler","v":2,"vm":42,"secret":"chut"}',
        ]) {
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });
});

// ---------------------------------------------------------------------------
// Sous-bloc G1 — catalogue d'applications et lancement (v2).
// ---------------------------------------------------------------------------

const APP_TEMOIN: Application = {
    cle: 'a1b2',
    nom: 'Bloc-notes',
    chemin: 'C:\\Users\\u\\Desktop\\Bloc-notes.lnk',
    cible: 'c:\\windows\\system32\\notepad.exe',
    arguments: '',
    repertoire: 'c:\\windows\\system32',
};

const CATALOGUE_TEMOIN =
    '{"type":"catalogue","v":2,"complet":true,"applications":[{"cle":"a1b2",'
    + '"nom":"Bloc-notes","chemin":"C:\\\\Users\\\\u\\\\Desktop\\\\Bloc-notes.lnk",'
    + '"cible":"c:\\\\windows\\\\system32\\\\notepad.exe","arguments":"",'
    + '"repertoire":"c:\\\\windows\\\\system32"}],"disparues":["disparue-1"]}';

describe('le catalogue et le lancement, sens AGENT -> PLATEFORME', () => {
    it('encode `catalogue` exactement comme Rust', () => {
        // 🔴 L'ordre des champs est `type` PUIS `v` : serde émet le tag interne
        // en premier, `JSON.stringify` respecte l'ordre d'insertion, et le
        // vecteur fige la chaîne octet pour octet. Écrire `v` d'abord — ce que
        // fait `control.ts` — produirait une chaîne différente, et la
        // divergence a été trouvée par ce test en P3, pas par la relecture.
        expect(encodeCatalogue(true, [APP_TEMOIN], ['disparue-1'])).toBe(CATALOGUE_TEMOIN);
    });

    it('encode `lancee` exactement comme Rust', () => {
        expect(encodeLancee('d-7', 'raccourci')).toBe(
            '{"type":"lancee","v":2,"demande":"d-7","issue":"raccourci"}',
        );
    });

    it('lit un `catalogue` bien formé, et retrouve chacun de ses champs', () => {
        const lu = parseVersLaPlateforme(CATALOGUE_TEMOIN);
        expect(lu.ok).toBe(true);
        if (!lu.ok) return;
        expect(lu.message.type).toBe('catalogue');
        if (lu.message.type !== 'catalogue') return;
        expect(lu.message.complet).toBe(true);
        expect(lu.message.disparues).toEqual(['disparue-1']);
        expect(lu.message.applications).toEqual([APP_TEMOIN]);
    });

    it('🔴 REJETTE un `catalogue` dont `applications` n’est pas un tableau, motif `forme`', () => {
        // 🔴 C'est le SEUL parseur du fichier dont les octets viennent d'un
        // tiers. Sans cette garde, un `applications` absent ou scalaire
        // traverserait jusqu'à la requête SQL. Et rendre `enrolement` ferait
        // lire « secret faux » au pair pour un message parfaitement
        // authentifié : le motif désigne la cause, il ne la déguise pas.
        expect(
            parseVersLaPlateforme('{"type":"catalogue","v":2,"complet":true,"applications":3,"disparues":[]}'),
        ).toEqual({ ok: false, motif: 'forme' });
        expect(
            parseVersLaPlateforme('{"type":"catalogue","v":2,"complet":true,"disparues":[]}'),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('🔴 REJETTE un `catalogue` dont une application est incomplète, motif `forme`', () => {
        expect(
            parseVersLaPlateforme(
                '{"type":"catalogue","v":2,"complet":true,"applications":[{"cle":"a"}],"disparues":[]}',
            ),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('🔴 REJETTE une `lancee` dont l’issue est inconnue, motif `forme`', () => {
        expect(
            parseVersLaPlateforme('{"type":"lancee","v":2,"demande":"d","issue":"peut-etre"}'),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('lit une `lancee` bien formée', () => {
        const lu = parseVersLaPlateforme('{"type":"lancee","v":2,"demande":"d-7","issue":"echec"}');
        expect(lu).toEqual({
            ok: true,
            message: { type: 'lancee', v: 2, demande: 'd-7', issue: 'echec' },
        });
    });
});

describe('le lancement, sens PLATEFORME -> AGENT', () => {
    it('encode `lancer` exactement comme Rust', () => {
        expect(encodeLancer('d-7', 'a1b2')).toBe(
            '{"type":"lancer","v":2,"demande":"d-7","cle":"a1b2"}',
        );
    });

    it('relit un `lancer`', () => {
        // 🔴 La rouge : l'omettre de `TYPES_DEPUIS`. Le parseur lèverait
        // « type de message de plateforme inconnu » sur un ordre valide.
        const lu = parseDepuisLaPlateforme(
            '{"type":"lancer","v":2,"demande":"d-7","cle":"a1b2"}',
        ) as unknown as Record<string, unknown>;
        expect(lu.type).toBe('lancer');
        expect(lu.demande).toBe('d-7');
        expect(lu.cle).toBe('a1b2');
    });
});

describe('la version 2, et les listes blanches DÉRIVÉES de l’union', () => {
    it('🔴 annonce la version 2, et REFUSE un message v1', () => {
        // 🔴 Oublier le bump côté TypeScript ferait diverger les deux bouts EN
        // SILENCE : le Rust émettrait `v:2`, ce parseur attendrait `v:1`, et
        // seuls les vecteurs partagés le diraient.
        expect(PLATEFORME_VERSION).toBe(2);
        expect(() => parseDepuisLaPlateforme('{"type":"refus","v":1,"motif":"version"}')).toThrow(
            /version de plateforme non supportée/,
        );
        expect(parseVersLaPlateforme('{"type":"battement","v":1}')).toEqual({
            ok: false,
            motif: 'version',
        });
    });

    it('🔴 les deux listes blanches couvrent EXACTEMENT leur union', () => {
        // 🔴 REMÈDE STRUCTUREL, ET C'EST SA MOITIÉ OBSERVABLE À L'EXÉCUTION.
        // L'autre moitié est le typecheck : `TOUS_DEPUIS` et `TOUS_VERS` sont
        // des `Record<Union['type'], true>`, et `tsc` refuse une clé
        // manquante. C'est le jumeau de `TYPES_AGENT` (`control.ts`), écrit à
        // la main, que rien ne confronte à son union — et dont l'oubli ne
        // casse « ni compilation ni test ». Ici l'oubli casse le typecheck.
        expect([...typesDepuis()].sort()).toEqual(
            ['battement-recu', 'enrole', 'lancer', 'refus'],
        );
        expect([...typesVers()].sort()).toEqual(['battement', 'catalogue', 'enroler', 'lancee']);
    });
});
