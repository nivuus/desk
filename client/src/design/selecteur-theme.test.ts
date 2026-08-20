import { describe, expect, it } from 'vitest';
import { installerSelecteurDeTheme, type BoutonDeTheme } from './selecteur-theme';
import { CLE_THEME } from './theme';

/**
 * Des doubles ÉCRITS À LA MAIN, sur le patron de `faireBouton()` de
 * `client/src/fullscreen.test.ts` : `client/` n'a NI jsdom NI happy-dom, et le
 * module ne lit donc aucun global dans le chemin testé.
 *
 * ⚠️ AUCUN DE CES DOUBLES N'EST UN NO-OP. Le sous-bloc D10 a payé « une source
 * factice qui implémente un effet de bord en NO-OP rend une famille entière de
 * défauts invisible aux tests d'hôte » : ici le coffre RETIENT ce qu'on lui
 * écrit, la racine RETIENT son attribut, et chaque bouton RETIENT ses
 * `aria-pressed` successifs.
 */
function banc(themeInitial: string | null = null) {
    const ecrits: Array<[string, string]> = [];
    const stockage = new Map<string, string>();
    if (themeInitial !== null) stockage.set(CLE_THEME, themeInitial);

    const coffre = {
        getItem: (cle: string) => stockage.get(cle) ?? null,
        setItem: (cle: string, valeur: string) => {
            ecrits.push([cle, valeur]);
            stockage.set(cle, valeur);
        },
    };

    let attribut: string | null = null;
    const racine = {
        setAttribute: (_nom: string, valeur: string) => { attribut = valeur; },
        removeAttribute: () => { attribut = null; },
    };

    const boutons: Array<BoutonDeTheme & { cliquer(): void }> = [];
    const creerBouton = (): BoutonDeTheme => {
        let clic: (() => void) | null = null;
        const bouton = {
            dataset: {} as { theme?: string },
            textContent: null as string | null,
            attributs: new Map<string, string>(),
            setAttribute(nom: string, valeur: string) { this.attributs.set(nom, valeur); },
            addEventListener(_type: 'click', ecouteur: () => void) { clic = ecouteur; },
            cliquer() { clic?.(); },
        };
        boutons.push(bouton);
        return bouton;
    };

    let surStorage: ((e: { key: string | null; newValue: string | null }) => void) | null = null;
    const source = {
        addEventListener(
            _type: 'storage',
            ecouteur: (e: { key: string | null; newValue: string | null }) => void,
        ) { surStorage = ecouteur; },
    };

    let rappels = 0;
    installerSelecteurDeTheme({
        hote: { append: () => {} },
        racine,
        coffre,
        source,
        creerBouton,
        apres: () => { rappels += 1; },
    });

    const parTheme = (nom: string) => boutons.find((b) => b.dataset.theme === nom)!;
    const presse = (nom: string) =>
        (parTheme(nom) as unknown as { attributs: Map<string, string> }).attributs.get('aria-pressed');

    return {
        ecrits,
        boutons,
        parTheme,
        presse,
        attributRacine: () => attribut,
        rappels: () => rappels,
        // 🔴 LE COFFRE EST MIS À JOUR AVANT LA DIFFUSION, PARCE QUE C'EST CE
        // QUE FAIT LE NAVIGATEUR : la spécification HTML garantit que
        // l'événement `storage` est émis APRÈS que la zone de stockage a été
        // modifiée. Un double qui diffuserait sans écrire ferait échouer le
        // test du marquage POUR UNE RAISON ÉTRANGÈRE au défaut qu'il éprouve —
        // et la correction aurait l'air de ne pas marcher. C'est la première
        // rédaction de ce banc, et elle a été corrigée ici et non dans le code.
        emettreStorage: (key: string | null, newValue: string | null) => {
            if (key !== null) {
                if (newValue === null) stockage.delete(key);
                else stockage.set(key, newValue);
            }
            surStorage?.({ key, newValue });
        },
    };
}

describe('installerSelecteurDeTheme', () => {
    it('pose exactement trois boutons, un par état', () => {
        const b = banc();
        expect(b.boutons.map((x) => x.dataset.theme)).toEqual(['systeme', 'clair', 'sombre']);
    });

    it('un clic sur « clair » ÉCRIT LA CLÉ', () => {
        // Première moitié, séparée de la seconde comme le contrôle §7.5 sépare
        // les siennes : écrire et appliquer sont deux effets, et `expect`
        // interrompt au premier échec.
        const b = banc();
        (b.parTheme('clair') as unknown as { cliquer(): void }).cliquer();
        expect(b.ecrits).toEqual([[CLE_THEME, 'clair']]);
    });

    it('un clic sur « clair » POSE `data-theme` LOCALEMENT', () => {
        const b = banc();
        (b.parTheme('clair') as unknown as { cliquer(): void }).cliquer();
        expect(b.attributRacine()).toBe('clair');
    });

    it('`aria-pressed` suit le clic', () => {
        const b = banc();
        expect(b.presse('systeme')).toBe('true');
        (b.parTheme('sombre') as unknown as { cliquer(): void }).cliquer();
        expect(b.presse('sombre')).toBe('true');
        expect(b.presse('systeme')).toBe('false');
    });

    it('`aria-pressed` suit un `storage` VENU D\'UNE AUTRE FENÊTRE', () => {
        // 🔴 LA ROUGE DE LA CORRECTION, et elle tombe sur le code d'avant S3.
        // Ce module déclarait lui-même le défaut : « IL NE RAPPELLE PAS
        // `marquer()` […] un `aria-pressed` posé ici reste celui du thème
        // d'avant tant que l'utilisateur ne clique pas dans CETTE fenêtre. Le
        // défaut est réel et il est PRÉEXISTANT ».
        //
        // Sur un INSTRUMENT, c'était une gêne. Sur le PRODUIT, c'est une
        // interface qui MENT sur son propre état — et le cas d'une fenêtre
        // voisine qui change le thème est le cas NOMINAL du multi-fenêtres,
        // qui est la raison d'être du mécanisme `storage` (spec §4.2).
        const b = banc();
        expect(b.presse('systeme')).toBe('true');
        b.emettreStorage(CLE_THEME, 'sombre');
        expect(b.presse('sombre')).toBe('true');
        expect(b.presse('systeme')).toBe('false');
    });

    it('applique le thème reçu par `storage` à la racine', () => {
        const b = banc();
        b.emettreStorage(CLE_THEME, 'clair');
        expect(b.attributRacine()).toBe('clair');
    });

    it('un `storage` sur la clé VOISINE `guac.jeton.acces` ne change RIEN', () => {
        // ⚠️ LA VRAIE CLÉ VOISINE, jamais une clé inventée : `jeton.ts` la
        // déclare et `connexion.ts` l'écrit réellement à chaque connexion, donc
        // toute fenêtre voisine reçoit cet événement-là. Sans le filtre,
        // `data-theme` vaudrait un JWT.
        const b = banc('clair');
        b.emettreStorage('guac.jeton.acces', 'eyJhbGciOi.faux.jeton');
        expect(b.attributRacine()).toBe('clair');
        expect(b.presse('clair')).toBe('true');
        expect(b.rappels()).toBe(0);
    });

    it("n'écrit RIEN dans le coffre en réagissant à un `storage`", () => {
        // Sans quoi deux fenêtres se renverraient l'événement sans terme.
        // ⚠️ CE TEST N'EST PAS GARANTI PAR LA SIGNATURE, contrairement à celui
        // de `surStockageModifie` : ici la fermeture DÉTIENT le coffre et le
        // LIT à chaque marquage. Il peut donc tomber, et c'est pourquoi il est
        // écrit.
        const b = banc();
        b.emettreStorage(CLE_THEME, 'sombre');
        expect(b.ecrits).toEqual([]);
    });
});
