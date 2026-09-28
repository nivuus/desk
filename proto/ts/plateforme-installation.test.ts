// Les messages de l'INSTALLATION, côté TypeScript.
//
// 🔴 CE FICHIER EXISTE POUR UNE GARDE QUE LES VECTEURS NE PEUVENT PAS
// ÉPROUVER. `plateforme-vectors.json` est un jeu de ROUND-TRIPS : il fige les
// chaînes que les deux langages doivent produire et relire. Il ne dit rien de
// ce qui doit être REFUSÉ — et la garde la plus fragile de v4 est précisément
// un refus : un `termine` dont la clé `motif` MANQUE.

import { describe, expect, it } from 'vitest';
import {
    PLATEFORME_VERSION,
    encodeTermine,
    parseVersLaPlateforme,
    type Issue,
} from './plateforme';

/** Le `termine` de référence, dont chaque cas ci-dessous retire une chose. */
function termineComplet(): Record<string, unknown> {
    return {
        type: 'termine',
        v: PLATEFORME_VERSION,
        installation: 'i-1',
        issue: 'reussie' as Issue,
        motif: null,
        code_sortie: 0,
        journal: '',
        journal_tronque: false,
    };
}

describe('`termine`: the optional fields are REQUIRED ON THE WIRE', () => {
    it('reads a complete `termine`, reason and code at null', () => {
        const lu = parseVersLaPlateforme(JSON.stringify(termineComplet()));
        expect(lu).toEqual({ ok: true, message: termineComplet() });
    });

    // 🔴 LA GARDE QUI COMPTE, ET ELLE EST LE JUMEAU EXACT DE
    // `champs::option_obligatoire` CÔTÉ RUST. `serde_derive` traite tout champ
    // `Option<T>` comme portant un `#[serde(default)]` IMPLICITE, et
    // `JSON.parse` rend `undefined` aussi bien pour « clé absente » que pour
    // « clé à undefined ». Sans garde, un `termine` d'une version ANTÉRIEURE —
    // qui n'a pas ces champs — serait accepté avec un motif silencieusement
    // absent : c'est le déguisement précis que le bump de version existe pour
    // empêcher, et `deny_unknown_fields` n'y peut rien, lui qui regarde les
    // champs EN TROP.
    it.each(['motif', 'code_sortie'])('🔴 REFUSES a `termine` without the key `%s`', (cle) => {
        const ampute = termineComplet();
        delete ampute[cle];
        expect(parseVersLaPlateforme(JSON.stringify(ampute))).toEqual({
            ok: false,
            motif: 'forme',
        });
    });

    it('accepts `null` on these two keys, and tells them apart from absence', () => {
        const withNull = { ...termineComplet(), motif: null, code_sortie: null };
        const lu = parseVersLaPlateforme(JSON.stringify(withNull));
        expect(lu).toEqual({ ok: true, message: withNull });
    });

    // ⚠️ `encodeTermine` EXIGE `| null`, PAS `?`, et c'est la seule chose qui
    // empêche l'omission d'être écrivable : `JSON.stringify` OMET un
    // `undefined` et ÉCRIT un `null`. Un encodeur à paramètre facultatif
    // produirait donc une chaîne que le jumeau Rust refuserait — et le seul
    // symptôme serait un refus `forme` très loin de sa cause.
    it('🔴 encodes `motif: null` by writing the key, never by omitting it', () => {
        const chain = encodeTermine('i-1', 'reussie', null, null, '', false);
        expect(chain).toContain('"motif":null');
        expect(chain).toContain('"code_sortie":null');
        expect(parseVersLaPlateforme(chain).ok).toBe(true);
    });

    it('refuses a non-integer exit code', () => {
        expect(
            parseVersLaPlateforme(
                JSON.stringify({ ...termineComplet(), code_sortie: 1.5 }),
            ),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    // 🔴 UN CODE DE SORTIE NÉGATIF EST LÉGITIME sous Windows : les `HRESULT`
    // d'échec ont le bit de poids fort à 1, et se lisent en `i32` signé. Le
    // refuser confondrait « code hors norme » avec « échec ordinaire ».
    it('accepts a NEGATIVE exit code', () => {
        const negatif = { ...termineComplet(), code_sortie: -1073741510 };
        expect(parseVersLaPlateforme(JSON.stringify(negatif))).toEqual({
            ok: true,
            message: negatif,
        });
    });
});

describe('`progression`: the counts are natural integers', () => {
    function progression(sur: Record<string, unknown> = {}): Record<string, unknown> {
        return {
            type: 'progression',
            v: PLATEFORME_VERSION,
            installation: 'i-1',
            phase: 'transfert',
            octets_faits: 1,
            octets_total: 2,
            ecoule_ms: 3,
            ...sur,
        };
    }

    it('reads a well-formed progress', () => {
        expect(parseVersLaPlateforme(JSON.stringify(progression()))).toEqual({
            ok: true,
            message: progression(),
        });
    });

    // ⚠️ `typeof x === 'number'` NE SUFFIT PAS : il laisse passer `NaN`,
    // `Infinity` et `1.5`. Un `NaN` traverserait jusqu'à la base, où il
    // deviendrait un `NULL` sur une colonne `NOT NULL` — c'est-à-dire une
    // erreur SQL très loin de sa cause.
    it.each([
        ['a negative count', { octets_faits: -1 }],
        ['a fractional count', { octets_total: 1.5 }],
        ['an unknown phase', { phase: 'empreinte' }],
        ['an empty installation', { installation: '' }],
    ])('🔴 REFUSES %s', (_nom, sur) => {
        expect(parseVersLaPlateforme(JSON.stringify(progression(sur)))).toEqual({
            ok: false,
            motif: 'forme',
        });
    });

    // 🔴 `empreinte` N'EST PAS UNE PHASE DE CE CANAL, et le cas ci-dessus le
    // fige : elle se déroule dans le NAVIGATEUR, avant que la plateforme n'ait
    // la moindre ligne à écrire. L'y accepter laisserait croire que l'agent
    // peut la rapporter.
    it('accepts an `execution` phase without a total', () => {
        const sansTotal = progression({ phase: 'execution', octets_faits: 0, octets_total: 0 });
        expect(parseVersLaPlateforme(JSON.stringify(sansTotal))).toEqual({
            ok: true,
            message: sansTotal,
        });
    });
});
