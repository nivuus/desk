import { describe, expect, it } from 'vitest';
import { ADRESSE_INCONNUE, adresseSource, pairDeConfiance } from './adresse-source';

/// Des adresses de documentation (RFC 5737), toutes DISTINCTES et toutes
/// plausibles : un module qui normalise des adresses ne peut pas être éprouvé
/// sur des étiquettes `a`/`b`/`c`.
const CLIENT = '203.0.113.7';
const INTERMEDIAIRE = '198.51.100.4';
/// L'adresse d'un proxy sur un réseau docker interne — la valeur qu'un
/// exploitant posera réellement dans `PLATEFORME_PROXY_DE_CONFIANCE`.
const PROXY = '172.18.0.5';

const NO_TRUST: ReadonlySet<string> = new Set();
const PROXY_DE_CONFIANCE: ReadonlySet<string> = new Set([PROXY]);

describe('adresseSource', () => {
    it("(a) 🔴 a NON-trusted source has its header IGNORED", () => {
        // Le demandeur prétend venir d'ailleurs ; personne ne l'a autorisé à
        // le dire. Croire cet en-tête serait une usurpation d'identité, et
        // rendrait le frein par adresse contournable en une ligne d'en-tête.
        expect(adresseSource(CLIENT, `${INTERMEDIAIRE}, ${PROXY}`, NO_TRUST)).toBe(CLIENT);
    });

    it("(a bis) EMPTY trust is the default, and it trusts nobody", () => {
        expect(adresseSource(PROXY, CLIENT, NO_TRUST)).toBe(PROXY);
    });

    it('(b) 🔴 a trusted source: we take the LAST element, never the first', () => {
        // `nginx` avec `$proxy_add_x_forwarded_for` AJOUTE l'adresse de son
        // pair à ce que le client a envoyé. Un client qui envoie
        // `X-Forwarded-For: 203.0.113.7` produit donc
        // `203.0.113.7, <sa vraie adresse>` : le PREMIER élément est celui
        // que le client a forgé, le DERNIER est le seul que le proxy ait
        // écrit lui-même.
        expect(adresseSource(PROXY, `${CLIENT}, ${INTERMEDIAIRE}`, PROXY_DE_CONFIANCE))
            .toBe(INTERMEDIAIRE);
    });

    it('(c) header absent ⇒ the address of the peer', () => {
        expect(adresseSource(PROXY, undefined, PROXY_DE_CONFIANCE)).toBe(PROXY);
    });

    it('(d) header present but empty or blank ⇒ the address of the peer', () => {
        expect(adresseSource(PROXY, ' , ', PROXY_DE_CONFIANCE)).toBe(PROXY);
        expect(adresseSource(PROXY, '', PROXY_DE_CONFIANCE)).toBe(PROXY);
    });

    it('(d bis) TRAILING empty elements are skipped, not taken for the last one', () => {
        // `X-Forwarded-For: 203.0.113.7, ` a un dernier élément VIDE. Le
        // prendre rendrait une clé de frein vide, que toutes les requêtes
        // mal formées partageraient.
        expect(adresseSource(PROXY, `${CLIENT}, `, PROXY_DE_CONFIANCE)).toBe(CLIENT);
    });

    it('(e) 🔴 an IPv4 wrapped in IPv6 returns the SAME key as its bare form', () => {
        // Sans cette normalisation, le même client compte DEUX fois selon la
        // pile employée, et son budget de frein double.
        expect(adresseSource(`::ffff:${CLIENT}`, undefined, NO_TRUST))
            .toBe(adresseSource(CLIENT, undefined, NO_TRUST));
        expect(adresseSource(`::ffff:${CLIENT}`, undefined, NO_TRUST)).toBe(CLIENT);
    });

    it("(e bis) TRUST is judged on the normalised form, on both sides", () => {
        // Node rend couramment `::ffff:172.18.0.5` pour un pair IPv4 sur une
        // pile double. Comparer la forme brute à la valeur configurée ferait
        // échouer la confiance en silence — et le frein par adresse
        // dégénérerait en frein GLOBAL sans qu'aucune ligne ne le dise.
        expect(adresseSource(`::ffff:${PROXY}`, CLIENT, PROXY_DE_CONFIANCE)).toBe(CLIENT);
        // Et la configuration écrite sous forme encapsulée vaut la nue.
        expect(adresseSource(PROXY, CLIENT, new Set([`::ffff:${PROXY}`]))).toBe(CLIENT);
    });

    it("(e ter) the retained header element is normalised too", () => {
        expect(adresseSource(PROXY, `::ffff:${CLIENT}`, PROXY_DE_CONFIANCE)).toBe(CLIENT);
    });

    it('(f) 🔴 a peer without an address returns a NAMED value, never « undefined »', () => {
        // Un socket déjà fermé rend `undefined` pour `remoteAddress`. La clé
        // du frein ne doit pas devenir la chaîne `"undefined"` par accident
        // d'interpolation : c'est le piège que `signaling/turn-harnais.ts`
        // documente pour `process.env`, et il se rejoue ici.
        const rendu = adresseSource(undefined, undefined, NO_TRUST);
        expect(rendu).toBe(ADRESSE_INCONNUE);
        expect(rendu).not.toBe('undefined');
        expect(rendu.length).toBeGreaterThan(0);
    });

    it("(f bis) a peer without an address NEVER becomes trusted", () => {
        // Si `ADRESSE_INCONNUE` figurait par mégarde dans l'ensemble de
        // confiance, tous les pairs anonymes pourraient forger leur adresse.
        expect(adresseSource(undefined, CLIENT, new Set([ADRESSE_INCONNUE]))).toBe(ADRESSE_INCONNUE);
    });
});

describe('pairDeConfiance', () => {
    it('accepts a declared peer', () => {
        expect(pairDeConfiance('10.0.0.1', new Set(['10.0.0.1']))).toBe(true);
    });

    // Le préfixe des adresses IPv4 mappées, comme `adresseSource` le fait déjà.
    it('normalises the ::ffff: prefix ON THE PEER SIDE', () => {
        expect(pairDeConfiance('::ffff:10.0.0.1', new Set(['10.0.0.1']))).toBe(true);
    });

    // 🔴 ROUND DE CORRECTION 1 — LA NORMALISATION DU CÔTÉ DÉCLARÉ N'ÉTAIT
    // ÉPROUVÉE PAR RIEN : mesuré, remplacer `normaliser(declare)` par
    // `declare` dans `pairDeConfiance` laissait 645/645 tests verts. Le sens
    // de la panne est fermé (un exploitant déclarant une forme encapsulée
    // verrait TOUT LE MONDE refusé, jamais une ouverture), mais l'affirmation
    // du commentaire de tête — « NORMALISER EST OBLIGATOIRE DES DEUX CÔTÉS » —
    // ne tenait sur rien. Ce test ferme le trou : la confiance est déclarée
    // sous forme ENCAPSULÉE, le pair se présente sous forme NUE.
    it('normalises the ::ffff: prefix ON THE DECLARED SIDE', () => {
        expect(pairDeConfiance('10.0.0.1', new Set(['::ffff:10.0.0.1']))).toBe(true);
    });

    it('refuses an undeclared peer', () => {
        expect(pairDeConfiance('10.0.0.2', new Set(['10.0.0.1']))).toBe(false);
    });

    // 🔴 UNE LISTE VIDE NE FAIT CONFIANCE À PERSONNE. Le contraire ferait de
    // l'absence de configuration une ouverture — l'inverse exact du défaut sûr.
    it('refuses everybody when the list is empty', () => {
        expect(pairDeConfiance('10.0.0.1', new Set())).toBe(false);
    });

    it('refuses an absent address', () => {
        expect(pairDeConfiance(undefined, new Set(['10.0.0.1']))).toBe(false);
    });
});
