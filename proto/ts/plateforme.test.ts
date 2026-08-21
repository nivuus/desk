import { describe, expect, it } from 'vitest';
import vecteurs from '../plateforme-vectors.json';
import {
    PLATEFORME_VERSION,
    encodeInstaller,
    encodeProgression,
    encodeTermine,
    type Issue,
    type Phase,
    encodeEnroler,
    encodeBattement,
    encodeEnrole,
    encodeBattementRecu,
    encodeRefus,
    parseDepuisLaPlateforme,
    parseVersLaPlateforme,
    type MotifCanal,
    type Application,
    type IssueLancement,
    encodeCatalogue,
    encodeIconesManquantes,
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
    installation?: string;
    url?: string;
    nom?: string;
    taille?: number;
    sha256?: string;
    phase?: string;
    octets_faits?: number;
    octets_total?: number;
    ecoule_ms?: number;
    code_sortie?: number | null;
    journal?: string;
    journal_tronque?: boolean;
    json: string;
    vm?: string;
    secret?: string;
    prefixe?: string;
    jeton?: string;
    expire_a?: number;
    motif?: string;
    complet?: boolean;
    applications?: Application[];
    disparues?: string[];
    demande?: string;
    issue?: string;
    cle?: string;
    empreintes?: string[];
}

const cas: CasVecteur[] = vecteurs.cases as CasVecteur[];

/**
 * 🔴 UN `kind` INCONNU LÈVE, il ne retombe pas sur un défaut. Le dispatch
 * était un ternaire dont la dernière branche servait de fourre-tout : un
 * `kind` mal orthographié y aurait été encodé par la mauvaise fonction, et le
 * seul symptôme aurait été une chaîne fausse — impossible à distinguer d'une
 * divergence d'encodage réelle. C'est le `panic!` du côté Rust, transposé.
 */
function encodeVers(c: CasVecteur): string {
    switch (c.kind) {
        case 'enroler': return encodeEnroler(c.vm!, c.secret!);
        case 'battement': return encodeBattement();
        case 'catalogue': return encodeCatalogue(c.complet!, c.applications!, c.disparues!);
        case 'lancee': return encodeLancee(c.demande!, c.issue as IssueLancement);
        case 'progression':
            return encodeProgression(
                c.installation!, c.phase as Phase,
                c.octets_faits!, c.octets_total!, c.ecoule_ms!,
            );
        case 'termine':
            return encodeTermine(
                c.installation!, c.issue as Issue,
                // ⚠️ `?? null` SERAIT UNE FAUTE ICI : il confondrait « la clé
                // manque du vecteur » avec « la clé porte null », et un vecteur
                // amputé encoderait quand même. Le JSON les distingue, et
                // `c.motif` vaut littéralement `null` dans les cas qui le
                // portent — c'est ce que la lecture non typée rend.
                c.motif as string | null, c.code_sortie as number | null,
                c.journal!, c.journal_tronque!,
            );
        default: throw new Error(`kind inconnu dans le sens vers : ${c.kind}`);
    }
}

function encodeDepuis(c: CasVecteur): string {
    switch (c.kind) {
        case 'enrole': return encodeEnrole(c.prefixe!, c.jeton!, c.expire_a!);
        case 'battement-recu': return encodeBattementRecu(c.jeton!, c.expire_a!);
        case 'refus': return encodeRefus(c.motif as MotifCanal);
        case 'lancer': return encodeLancer(c.demande!, c.cle!);
        case 'icones-manquantes': return encodeIconesManquantes(c.empreintes!);
        case 'installer':
            return encodeInstaller(c.installation!, c.url!, c.nom!, c.taille!, c.sha256!);
        default: throw new Error(`kind inconnu dans le sens depuis : ${c.kind}`);
    }
}

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
            expect(encodeVers(c)).toBe(c.json);
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
            for (const champ of ['prefixe', 'jeton', 'expire_a', 'motif', 'demande', 'cle'] as const) {
                if (c[champ] !== undefined) expect(lu[champ]).toBe(c[champ]);
            }
            // ⚠️ `empreintes` est un TABLEAU : `toBe` y comparerait les
            // références et passerait pour la mauvaise raison sur `undefined`.
            if (c.empreintes !== undefined) expect(lu.empreintes).toEqual(c.empreintes);
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
            expect(encodeDepuis(c)).toBe(c.json);
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
            .toBe('{"type":"enroler","v":4,"vm":"w1","secret":"chut"}');
    });

    it('encode `battement`', () => {
        expect(encodeBattement()).toBe('{"type":"battement","v":4}');
    });

    it('lit un `enrole` bien formé', () => {
        const m = parseDepuisLaPlateforme(
            '{"type":"enrole","v":4,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}',
        );
        expect(m).toEqual({
            type: 'enrole', v: PLATEFORME_VERSION, prefixe: 'PPP', jeton: 'jjj', expire_a: 1787136773742,
        });
    });

    it('🔴 REJETTE une version PLATEFORME_VERSION + 1', () => {
        // 🔴 La rouge : ne comparer que `parsed.type`. Le message passerait, et
        // c'est la moitié TypeScript du critère ③.
        //
        // ⚠️ CE CAS PORTAIT UN `refus` JUSQU'AU 20 AOÛT 2026. Le refus est
        // désormais la seule variante hors versionnement — voir
        // `RefusMessage` —, il ne peut donc plus porter cette propriété. Un
        // `lancer` la porte, et c'est le message où elle mord le plus : une
        // version future pourrait donner à `cle` un tout autre sens.
        expect(() => parseDepuisLaPlateforme(
            `{"type":"lancer","v":${PLATEFORME_VERSION + 1},"demande":"d","cle":"c"}`,
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
        //
        // ⚠️ LE VÉHICULE RESTE UN `refus`, ET C'EST DÉSORMAIS LE MEILLEUR
        // ENDROIT POUR CE TEST : depuis le 20 août 2026 le refus tolère toute
        // VALEUR de `v`, et cette assertion est ce qui garde la frontière —
        // « toute valeur » n'est pas « pas de champ du tout ».
        expect(() => parseDepuisLaPlateforme('{"type":"refus","motif":"version"}'))
            .toThrow(/version de plateforme absente ou non numérique/);
    });

    it('🔴 REJETTE une version NULLE, que `?? ` laisserait passer', () => {
        // Complément du test précédent : `null ?? 1` vaut `1`, donc une
        // implémentation par valeur par défaut accepterait ce message-ci sans
        // que rien d'autre ne bouge.
        expect(() => parseDepuisLaPlateforme('{"type":"refus","v":null,"motif":"version"}'))
            .toThrow(/version de plateforme absente ou non numérique/);
    });

    it('REJETTE un `type` inconnu', () => {
        expect(() => parseDepuisLaPlateforme('{"type":"vol","v":4}'))
            .toThrow(/type de message de plateforme inconnu/);
    });

    it('🔴 REJETTE un `type` du sens AGENT -> PLATEFORME', () => {
        // 🔴 Le parseur ne lit QUE le sens plateforme -> agent. Accepter
        // `enroler` ici ferait qu'un agent traiterait son propre message comme
        // une réponse — une confusion de sens qu'aucun autre test ne verrait.
        expect(() => parseDepuisLaPlateforme('{"type":"enroler","v":4,"vm":"w","secret":"s"}'))
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
        expect(parseVersLaPlateforme('{"type":"enroler","v":4,"vm":"w1","secret":"chut"}')).toEqual({
            ok: true,
            message: { type: 'enroler', v: PLATEFORME_VERSION, vm: 'w1', secret: 'chut' },
        });
    });

    it('lit un `battement`', () => {
        expect(parseVersLaPlateforme('{"type":"battement","v":4}')).toEqual({
            ok: true,
            message: { type: 'battement', v: PLATEFORME_VERSION },
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
            '{"type":"enrole","v":4,"prefixe":"P","jeton":"j","expire_a":1}',
            '{"type":"battement-recu","v":4,"jeton":"j","expire_a":1}',
            '{"type":"refus","v":4,"motif":"forme"}',
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
            '{"type":"enroler","v":4,"secret":"chut"}',
            '{"type":"enroler","v":4,"vm":"w1"}',
            '{"type":"enroler","v":4,"vm":"","secret":"chut"}',
            '{"type":"enroler","v":4,"vm":42,"secret":"chut"}',
        ]) {
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });
});


describe('la version du protocole, et les listes blanches DÉRIVÉES de l’union', () => {
    // ⚠️ LE NUMÉRO N'EST ÉCRIT QU'UNE FOIS, dans l'assertion ci-dessous. Il
    // vivait aussi dans le titre du `describe` et dans celui de l'`it`, qui
    // annonçaient « la version 3 » : trois places pour un seul fait, dont deux
    // qu'aucun test ne pouvait faire rougir. Le bump de G3 les a trouvées
    // périmées — elles disaient « 3 » sur un protocole en 4.
    it('🔴 annonce sa version, et REFUSE un message v1', () => {
        // 🔴 Oublier le bump côté TypeScript ferait diverger les deux bouts EN
        // SILENCE : le Rust émettrait `v:3`, ce parseur attendrait `v:2`, et
        // seuls les vecteurs partagés le diraient.
        // 🔴 LITTÉRAL DÉLIBÉRÉ, ET C'EST UN TRÉBUCHET : `toBe(PLATEFORME_VERSION)`
        // serait une tautologie. Ce nombre existe pour qu'un bump OBLIGE une
        // main humaine à passer ici, et le commentaire du dessus dit pourquoi.
        expect(PLATEFORME_VERSION).toBe(4);
        // ⚠️ CE CAS PORTAIT UN `refus` JUSQU'AU 20 AOÛT 2026, et il épinglait
        // le défaut au lieu de le garder : le refus est désormais la SEULE
        // variante hors versionnement, précisément pour qu'un agent v1 puisse
        // lire celui qui lui apprend qu'il est périmé. La propriété gardée ici
        // — « un message v1 est refusé » — reste éprouvée, sur une variante qui
        // la porte encore.
        expect(() =>
            parseDepuisLaPlateforme('{"type":"enrole","v":1,"prefixe":"P","jeton":"j","expire_a":1}'),
        ).toThrow(/version de plateforme non supportée/);
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
            ['battement-recu', 'enrole', 'icones-manquantes', 'installer', 'lancer', 'refus'],
        );
        expect([...typesVers()].sort()).toEqual([
            'battement', 'catalogue', 'enroler', 'lancee', 'progression', 'termine',
        ]);
    });
});

// ---------------------------------------------------------------------------
// Correction du 20 août 2026 — UN REFUS DOIT ÊTRE LISIBLE PAR SON DESTINATAIRE.
// ---------------------------------------------------------------------------

describe('le refus est une enveloppe HORS versionnement', () => {
    // 🔴 LA ROUGE DU DÉFAUT 2, côté miroir. Mesuré en recette G1 : un agent v1
    // face à une plateforme v2 ne peut pas lire le refus qui lui dit POURQUOI
    // il est refusé, parce que le contrôle de version s'applique aussi au
    // refus. Les deux bouts doivent tolérer, sans quoi le miroir divergerait
    // du Rust en silence.
    it('🔴 se lit quelle que soit la version de son émetteur', () => {
        const futur = parseDepuisLaPlateforme('{"type":"refus","v":97,"motif":"version"}') as
            unknown as Record<string, unknown>;
        expect(futur.type).toBe('refus');
        expect(futur.motif).toBe('version');
        const passe = parseDepuisLaPlateforme('{"type":"refus","v":1,"motif":"enrolement"}') as
            unknown as Record<string, unknown>;
        expect(passe.motif).toBe('enrolement');
    });

    it('🔴 conserve un motif qu’aucune version de ce dépôt ne connaît', () => {
        const lu = parseDepuisLaPlateforme('{"type":"refus","v":98,"motif":"quota-depasse"}') as
            unknown as Record<string, unknown>;
        expect(lu.motif).toBe('quota-depasse');
    });

    it('🔴 et les AUTRES types restent refusés sur une version divergente', () => {
        // Sans cette moitié, la tolérance ci-dessus pourrait s'obtenir en ne
        // vérifiant plus rien du tout.
        for (const brut of [
            '{"type":"enrole","v":97,"prefixe":"P","jeton":"j","expire_a":1}',
            '{"type":"battement-recu","v":97,"jeton":"j","expire_a":1}',
            '{"type":"lancer","v":97,"demande":"d","cle":"c"}',
        ]) {
            expect(() => parseDepuisLaPlateforme(brut)).toThrow(
                /version de plateforme non supportée/,
            );
        }
    });
});

describe('les refus lisibles du fichier de vecteurs partagés', () => {
    // 🔴 LES MÊMES CHAÎNES QUE `conformite_aux_refus_lisibles_partages` CÔTÉ
    // RUST. Sans ce vecteur partagé, le remède pourrait ne vivre que d'un
    // côté — c'est exactement la divergence silencieuse que
    // `plateforme-vectors.json` existe pour fermer.
    const lisibles = (vecteurs as unknown as {
        refus_lisibles: { name: string; json: string; v: number; motif: string }[];
    }).refus_lisibles;

    it('🔴 en porte quatre, et pas zéro', () => {
        // Anti-tautologie : un tableau vide ferait passer la boucle ci-dessous
        // sans rien éprouver. Même garde que du côté Rust.
        expect(lisibles).toHaveLength(4);
    });

    for (const cas of lisibles) {
        it(`lit « ${cas.name} » quelle que soit sa version`, () => {
            const lu = parseDepuisLaPlateforme(cas.json) as unknown as Record<string, unknown>;
            expect(lu.type).toBe('refus');
            expect(lu.v).toBe(cas.v);
            expect(lu.motif).toBe(cas.motif);
        });
    }
});
