import { describe, expect, it } from 'vitest';
import { ADRESSE_INCONNUE, adresseSource } from './adresse-source';

/// Des adresses de documentation (RFC 5737), toutes DISTINCTES et toutes
/// plausibles : un module qui normalise des adresses ne peut pas être éprouvé
/// sur des étiquettes `a`/`b`/`c`.
const CLIENT = '203.0.113.7';
const INTERMEDIAIRE = '198.51.100.4';
/// L'adresse d'un proxy sur un réseau docker interne — la valeur qu'un
/// exploitant posera réellement dans `PLATEFORME_PROXY_DE_CONFIANCE`.
const PROXY = '172.18.0.5';

const AUCUNE_CONFIANCE: ReadonlySet<string> = new Set();
const PROXY_DE_CONFIANCE: ReadonlySet<string> = new Set([PROXY]);

describe('adresseSource', () => {
    it("(a) 🔴 une source NON de confiance voit son en-tête IGNORÉ", () => {
        // Le demandeur prétend venir d'ailleurs ; personne ne l'a autorisé à
        // le dire. Croire cet en-tête serait une usurpation d'identité, et
        // rendrait le frein par adresse contournable en une ligne d'en-tête.
        expect(adresseSource(CLIENT, `${INTERMEDIAIRE}, ${PROXY}`, AUCUNE_CONFIANCE)).toBe(CLIENT);
    });

    it("(a bis) la confiance VIDE est le défaut, et elle ne croit personne", () => {
        expect(adresseSource(PROXY, CLIENT, AUCUNE_CONFIANCE)).toBe(PROXY);
    });

    it('(b) 🔴 une source de confiance : on prend le DERNIER élément, jamais le premier', () => {
        // `nginx` avec `$proxy_add_x_forwarded_for` AJOUTE l'adresse de son
        // pair à ce que le client a envoyé. Un client qui envoie
        // `X-Forwarded-For: 203.0.113.7` produit donc
        // `203.0.113.7, <sa vraie adresse>` : le PREMIER élément est celui
        // que le client a forgé, le DERNIER est le seul que le proxy ait
        // écrit lui-même.
        expect(adresseSource(PROXY, `${CLIENT}, ${INTERMEDIAIRE}`, PROXY_DE_CONFIANCE))
            .toBe(INTERMEDIAIRE);
    });

    it('(c) en-tête absent ⇒ l’adresse du pair', () => {
        expect(adresseSource(PROXY, undefined, PROXY_DE_CONFIANCE)).toBe(PROXY);
    });

    it('(d) en-tête présent mais vide ou blanc ⇒ l’adresse du pair', () => {
        expect(adresseSource(PROXY, ' , ', PROXY_DE_CONFIANCE)).toBe(PROXY);
        expect(adresseSource(PROXY, '', PROXY_DE_CONFIANCE)).toBe(PROXY);
    });

    it('(d bis) les éléments vides de FIN sont sautés, pas pris pour le dernier', () => {
        // `X-Forwarded-For: 203.0.113.7, ` a un dernier élément VIDE. Le
        // prendre rendrait une clé de frein vide, que toutes les requêtes
        // mal formées partageraient.
        expect(adresseSource(PROXY, `${CLIENT}, `, PROXY_DE_CONFIANCE)).toBe(CLIENT);
    });

    it('(e) 🔴 une IPv4 encapsulée en IPv6 rend la MÊME clé que sa forme nue', () => {
        // Sans cette normalisation, le même client compte DEUX fois selon la
        // pile employée, et son budget de frein double.
        expect(adresseSource(`::ffff:${CLIENT}`, undefined, AUCUNE_CONFIANCE))
            .toBe(adresseSource(CLIENT, undefined, AUCUNE_CONFIANCE));
        expect(adresseSource(`::ffff:${CLIENT}`, undefined, AUCUNE_CONFIANCE)).toBe(CLIENT);
    });

    it("(e bis) la CONFIANCE se juge sur la forme normalisée, des deux côtés", () => {
        // Node rend couramment `::ffff:172.18.0.5` pour un pair IPv4 sur une
        // pile double. Comparer la forme brute à la valeur configurée ferait
        // échouer la confiance en silence — et le frein par adresse
        // dégénérerait en frein GLOBAL sans qu'aucune ligne ne le dise.
        expect(adresseSource(`::ffff:${PROXY}`, CLIENT, PROXY_DE_CONFIANCE)).toBe(CLIENT);
        // Et la configuration écrite sous forme encapsulée vaut la nue.
        expect(adresseSource(PROXY, CLIENT, new Set([`::ffff:${PROXY}`]))).toBe(CLIENT);
    });

    it("(e ter) l'élément d'en-tête retenu est normalisé lui aussi", () => {
        expect(adresseSource(PROXY, `::ffff:${CLIENT}`, PROXY_DE_CONFIANCE)).toBe(CLIENT);
    });

    it('(f) 🔴 un pair sans adresse rend une valeur NOMMÉE, jamais « undefined »', () => {
        // Un socket déjà fermé rend `undefined` pour `remoteAddress`. La clé
        // du frein ne doit pas devenir la chaîne `"undefined"` par accident
        // d'interpolation : c'est le piège que `signaling/turn-harnais.ts`
        // documente pour `process.env`, et il se rejoue ici.
        const rendu = adresseSource(undefined, undefined, AUCUNE_CONFIANCE);
        expect(rendu).toBe(ADRESSE_INCONNUE);
        expect(rendu).not.toBe('undefined');
        expect(rendu.length).toBeGreaterThan(0);
    });

    it("(f bis) un pair sans adresse ne devient JAMAIS de confiance", () => {
        // Si `ADRESSE_INCONNUE` figurait par mégarde dans l'ensemble de
        // confiance, tous les pairs anonymes pourraient forger leur adresse.
        expect(adresseSource(undefined, CLIENT, new Set([ADRESSE_INCONNUE]))).toBe(ADRESSE_INCONNUE);
    });
});
