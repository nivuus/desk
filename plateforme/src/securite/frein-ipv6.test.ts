// The brake against IPv6 clients. An IPv6 client is delegated at least a
// /64 and can pick a fresh address in it for every request, so a budget
// keyed per /128 never fills; and flooding the table with such fresh
// addresses used to evict the per-account lockout of the account under
// attack. Addresses are now keyed by their /64, and address entries live in
// a pool of their own that cannot evict account entries.

import { describe, expect, it } from 'vitest';
import {
    BUDGET_ADRESSE,
    BUDGET_COMPTE,
    BUDGET_REQUETES,
    ECHECS_MAX_ADRESSE,
    ECHECS_MAX_COMPTE,
    Frein,
    cleAdresse,
    cleCompte,
    cleRequetes,
} from './frein';

const T0 = 1_787_000_000_000;

describe('address keys', () => {
    it('two addresses of one IPv6 /64 share a key, whatever their spelling', () => {
        const une = cleAdresse('2001:db8:1:2::a');
        expect(cleAdresse('2001:0DB8:0001:0002:ffff:0:0:1')).toBe(une);
        expect(cleAdresse('2001:db8:1:2:0:0:0:a%eth0')).toBe(une);
        expect(cleAdresse('2001:db8:1:3::a')).not.toBe(une);
        expect(cleRequetes('2001:db8:1:2::1')).toBe(cleRequetes('2001:db8:1:2:abcd::9'));
        expect(cleRequetes('2001:db8:1:2::1')).not.toBe(une);
    });

    it('IPv4 keys are unchanged', () => {
        expect(cleAdresse('203.0.113.7')).toBe('adr:203.0.113.7');
        expect(cleRequetes('203.0.113.7')).toBe('req:203.0.113.7');
    });

    it('rotating addresses inside one /64 exhausts ONE budget', () => {
        const f = new Frein();
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i += 1) {
            f.echec([[cleAdresse(`2001:db8:1:2::${(i + 1).toString(16)}`), BUDGET_ADRESSE]], T0 + i);
        }
        expect(f.consulter([[cleAdresse('2001:db8:1:2::ffff'), BUDGET_ADRESSE]], T0 + 100).freine)
            .toBe(true);
        // Witness: the neighbouring /64 is untouched.
        expect(f.consulter([[cleAdresse('2001:db8:1:3::1'), BUDGET_ADRESSE]], T0 + 100).freine)
            .toBe(false);
    });
});

describe('eviction', () => {
    it('filling the table with address entries does not reset a locked account', () => {
        const f = new Frein(8);
        const compte = [cleCompte('ada@exemple.test'), BUDGET_COMPTE] as const;
        for (let i = 0; i < ECHECS_MAX_COMPTE; i += 1) f.echec([compte], T0 + i);
        expect(f.consulter([compte], T0 + 10).freine).toBe(true);

        for (let i = 0; i < 100; i += 1) {
            f.echec([[cleAdresse(`2001:db8:${i.toString(16)}::1`), BUDGET_ADRESSE]], T0 + 20 + i);
            f.echec([[cleRequetes(`198.51.100.${i}`), BUDGET_REQUETES]], T0 + 20 + i);
        }
        // Witness: the flood did force evictions among the address entries.
        expect(f.evictions()).toBeGreaterThan(0);
        expect(f.consulter([compte], T0 + 200).freine).toBe(true);
    });
});
