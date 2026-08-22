import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { CSP, ENTETES_DOCUMENT, ENTETES_RESSOURCE } from './entetes-page';

describe('les en-têtes de document', () => {
    // 🔴 TROIS TESTS DISTINCTS, PAS TROIS ASSERTIONS DANS UN SEUL : `expect`
    // s'arrête au premier échec, et ces trois propriétés de sécurité sont
    // INDÉPENDANTES — si la CSP est fausse, personne n'apprend si
    // `X-Frame-Options` l'est aussi.
    it('porte la CSP', () => {
        expect(ENTETES_DOCUMENT['Content-Security-Policy']).toBe(CSP);
    });

    it('porte Referrer-Policy: no-referrer', () => {
        expect(ENTETES_DOCUMENT['Referrer-Policy']).toBe('no-referrer');
    });

    it('porte X-Frame-Options: DENY', () => {
        expect(ENTETES_DOCUMENT['X-Frame-Options']).toBe('DENY');
    });

    // 🔴 HSTS RESTE AU TERMINATEUR TLS. La plateforme est joignable en clair
    // sur 192.168.3.1:8080 ; y affirmer que l'origine est HTTPS pour un an
    // serait une affirmation qu'elle n'est pas en position de faire.
    it("n'émet PAS Strict-Transport-Security", () => {
        expect(ENTETES_DOCUMENT).not.toHaveProperty('Strict-Transport-Security');
    });

    it('garde no-store sur le document, qui peut porter un jeton', () => {
        expect(ENTETES_DOCUMENT['Cache-Control']).toBe('no-store');
    });
});

describe('les en-têtes de ressource', () => {
    // 🔴 LA MOITIÉ QUI COMPTE. `no-store` ici tuerait le cache du navigateur
    // sur des noms que Vite empreinte déjà — la panne silencieuse type.
    it('est immutable, JAMAIS no-store', () => {
        expect(ENTETES_RESSOURCE['Cache-Control']).toBe('public, max-age=31536000, immutable');
    });

    // 🔴 DEUX TESTS DISTINCTS, PAS DEUX ASSERTIONS DANS UN SEUL — même raison
    // que ci-dessus.
    it("ne porte PAS Content-Security-Policy : c'est un en-tête de document", () => {
        expect(ENTETES_RESSOURCE).not.toHaveProperty('Content-Security-Policy');
    });

    it("ne porte PAS X-Frame-Options : c'est un en-tête de document", () => {
        expect(ENTETES_RESSOURCE).not.toHaveProperty('X-Frame-Options');
    });
});

// 🔴 DEUX COPIES D'UNE MÊME POLITIQUE DÉRIVENT. Ce test est la seule chose qui
// l'empêche : un durcissement appliqué d'un seul côté livrerait deux montages
// aux sécurités différentes sans qu'aucune suite ne bronche.
describe('la CSP ne dérive pas de celle de nginx', () => {
    it('est identique à celle de deploiement/nginx.conf', () => {
        // ⚠️ AUCUN REPLI : `readFileSync` LÈVE si le fichier manque, et c'est
        // voulu. « Un `||` de repli transforme *fichier absent* en *contrôle
        // vert*. »
        const nginx = readFileSync(
            new URL('../../../../deploiement/nginx.conf', import.meta.url),
            'utf8',
        );
        const trouvees = [...nginx.matchAll(/add_header Content-Security-Policy "([^"]+)"/g)];
        // Si nginx en déclarait deux, comparer « la première » choisirait en
        // silence. On exige l'unicité plutôt que de trancher.
        expect(trouvees).toHaveLength(1);
        expect(trouvees[0][1]).toBe(CSP);
    });
});
