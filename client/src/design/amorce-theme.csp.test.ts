import { describe, expect, it } from 'vitest';
// 🔴 CE TEST IMPORTE `plateforme/src/http/page/entetes-page.ts` — UN AUTRE
// PAQUET — ET C'EST LE POINT. Le défaut mesuré le 29 août 2026
// (`Executing inline script violates … script-src 'self'`) existait
// précisément parce qu'aucun test ne regardait les deux côtés à la fois : la
// CSP vit dans `plateforme/`, l'amorce anti-FOUC vit dans `client/`, et
// chaque paquet a sa propre suite `vitest`, qui ne voit que la moitié du
// problème. `CLAUDE.md` nomme la classe : « CE QU'UN NAVIGATEUR EXIGE, AUCUN
// TEST DE NODE NE LE VOIT ». Ce fichier est le garde qui relie les deux —
// s'il devait un jour être supprimé pour « alléger » ce paquet, le défaut
// redeviendrait invisible à `npm test`, exactement comme avant ce lot.
import { CSP } from '../../../plateforme/src/http/page/entetes-page';
import { NOM_FICHIER_AMORCE, baliseAmorce } from './amorce-theme-greffon';
// `?raw`, JAMAIS `node:fs` : voir `amorce-theme-greffon.ts`, qui explique
// pourquoi la lecture de CONTENU ne vit pas dans ce module partagé.
import AMORCE from './amorce-theme.js?raw';

/** La balise que le greffon `guac-amorce-theme` injecte réellement dans
 * `<head>` — LA MÊME FONCTION que `vite.config.ts` appelle, jamais une copie :
 * voir `amorce-theme-greffon.ts` pour pourquoi elle vit sous `src/` et non
 * dans `vite.config.ts`, qui n'est jamais typechecké. */
function baliseInjectee() {
    return baliseAmorce();
}

describe("l'amorce anti-FOUC ne peut pas être bloquée par la CSP servie", () => {
    // 🔴 C'EST LA RÉGRESSION ELLE-MÊME, REJOUÉE À L'ENVERS. Avant ce lot, le
    // greffon rendait `{ tag: 'script', children: AMORCE, … }` : un script EN
    // LIGNE. La CSP ci-dessous n'a JAMAIS admis `'unsafe-inline'` ni aucun
    // hash pour `script-src` — donc Chrome le bloquait, exactement comme
    // mesuré. Ce test échoue si quiconque réintroduit `children` sans changer
    // la CSP, ET si quiconque durcit la CSP sans vérifier que l'amorce reste
    // servable.
    it('le greffon injecte une balise `src`, JAMAIS `children` (un script en ligne)', () => {
        const balise = baliseInjectee();
        expect(balise.attrs).toHaveProperty('src');
        expect(balise).not.toHaveProperty('children');
    });

    it("la balise n'est ni `async`, ni `defer`, ni `type=\"module\"` — l'anti-FOUC l'exige", () => {
        // Un script asynchrone, différé, ou de type module s'exécuterait
        // APRÈS l'analyse du `<body>`, donc après la première peinture : ce
        // serait le FOUC que cette amorce existe pour éviter. Un `<script
        // src>` CLASSIQUE, lui, bloque l'analyse du document jusqu'à son
        // exécution — exactement comme le script en ligne qu'il remplace.
        //
        // ⚠️ `as Record<string, unknown>` : le type RÉEL de `attrs`
        // (`{ src: string }`) ne porte AUCUNE de ces trois clés — c'est
        // précisément ce que ce test veut prouver, donc y accéder exige de
        // s'en écarter ICI, dans le test, sans élargir le type de production.
        const attrs = baliseInjectee().attrs as Record<string, unknown>;
        expect(attrs.async).toBeUndefined();
        expect(attrs.defer).toBeUndefined();
        expect(attrs.type).not.toBe('module');
    });

    it("l'URL servie est de MÊME ORIGINE (un chemin absolu, jamais un hôte tiers)", () => {
        // 🔴 `script-src 'self'` n'admet QUE la même origine. Une URL absolue
        // (`https://…`) échapperait silencieusement à ce test tout en étant
        // bloquée par la CSP en pratique — la même classe de défaut, une
        // deuxième fois.
        const balise = baliseInjectee();
        expect(balise.attrs?.src).toBe(`/${NOM_FICHIER_AMORCE}`);
        expect(balise.attrs?.src).toMatch(/^\//);
    });

    it("la CSP n'a besoin d'AUCUN `'unsafe-inline'` ni hash pour admettre ce fichier", () => {
        // 🔴 CETTE ASSERTION EST CE QUI REND LE REMÈDE (b) DÉCISIF : `'self'`
        // seul couvre déjà un fichier de même origine. Un hash `sha256-…`
        // aurait dû être TENU IDENTIQUE, à la main, dans
        // `deploiement/nginx.conf` — qui porte sa propre copie STATIQUE de
        // cette CSP (voir `entetes-page.test.ts`, « la CSP ne dérive pas de
        // celle de nginx ») et ne peut PAS calculer un hash à la volée. C'est
        // exactement le « naufrage du 487 » : un hash recopié survit au
        // contenu qu'il décrivait. `'self'` ne se recopie pas, il ne dérive
        // donc jamais.
        const scriptSrc = CSP.split(';')
            .map((d) => d.trim())
            .find((d) => d.startsWith('script-src'));
        expect(scriptSrc).toBe("script-src 'self'");
    });
});

describe("l'actif bâti ne diverge pas de sa source", () => {
    // 🔴 LA DÉMONSTRATION DE ROUGE DE CE LOT PASSE PAR CE TEST. `client/dist`
    // est un ARTEFACT DE BUILD (gitignoré) : ce test exige `npm run build`
    // AU PRÉALABLE, et lève LOUDLY (aucun `||` de repli, aucun `try/catch`
    // muet) si `client/dist/amorce-theme.js` est absent — « un `||` de repli
    // transforme *fichier absent* en *contrôle vert* » (`CLAUDE.md`).
    //
    // Ce que ce test établit : le fichier RÉELLEMENT servi par la plateforme
    // (celui que `PLATEFORME_PAGE` expose, copié depuis `client/dist` vers
    // `/opt/nivuus/desk/client/dist` en production) est OCTET POUR OCTET
    // celui que `src/design/amorce-theme.js` définit aujourd'hui. Rougi et
    // revert le 29 août 2026 : `cp src/design/amorce-theme.js
    // <scratchpad>/amorce-theme.backup.js`, un octet ajouté à la source SANS
    // rebuild, ce test rougit (la source a changé, `dist/` non), restauré
    // depuis la copie nommée — jamais `git checkout --`, qui aurait aussi
    // effacé un éventuel travail non commité (`CLAUDE.md`, pièges du shell).
    it('`dist/amorce-theme.js` est identique à `src/design/amorce-theme.js`', async () => {
        // 🔴 IMPORT DYNAMIQUE, PAS STATIQUE : un `import … from '../../dist/…'`
        // en tête de fichier ferait échouer la RÉSOLUTION DE MODULE dès la
        // collecte, et ferait tomber les QUATRE AUTRES tests de ce fichier
        // avec lui — un diagnostic qui pointerait vers le mauvais coupable.
        // Ici, seul CE test échoue, avec le message qui dit quoi faire.
        let servi: string;
        try {
            const module = await import('../../dist/amorce-theme.js?raw');
            servi = module.default;
        } catch (cause) {
            throw new Error(
                `client/dist/amorce-theme.js est absent : lancer 'npm run build' dans ` +
                    `client/ avant ce test — un fichier de build manquant n'est pas un ` +
                    `contrôle vert. Cause : ${String(cause)}`,
            );
        }
        expect(servi).toBe(AMORCE);
    });
});
