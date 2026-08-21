// Le bloc E3 : le refus d'exclusivité du câble de la VM, côté bouton.
//
// ⚠️ **Fichier NÉ D'UNE EXTRACTION**, pas d'un découpage thématique : ces
// tests vivaient dans `micro.test.ts`, qu'ils ont porté de 411 à 515 lignes —
// au-delà de la porte des 500. Voir l'en-tête de `micro.fixtures.ts`.

import { describe, expect, it } from 'vitest';

import { attacherBoutonMicro } from './micro';
import { faussePiste, fauxBouton, fauxFlux, fauxSender } from './micro.fixtures';

describe("attacherBoutonMicro — l'exclusivité du câble (bloc E3)", () => {

    // ── Bloc E3 : le refus d'exclusivité du câble de la VM ──────────────────

    /// Le montage commun : bouton allumé, messages capturés.
    async function boutonAllume() {
        const bouton = fauxBouton();
        const messages: string[] = [];
        const controle = attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => fauxFlux(faussePiste()),
            surMessage: (texte) => messages.push(texte),
        });
        controle.annoncerDisponibilite(true);
        bouton.cliquer();
        await controle.enCours();
        expect(bouton.dataset.etat).toBe('actif');
        return { bouton, controle, messages };
    }

    /// 🔴 C'est la rouge R5, et sa moitié la plus importante est la SECONDE
    /// assertion : l'état ne bouge pas.
    ///
    /// Ranger ce refus dans `'refuse'` confondrait deux causes qui appellent
    /// deux gestes OPPOSÉS — l'une se répare dans les réglages du navigateur,
    /// l'autre en fermant l'autre fenêtre. Et éteindre le bouton d'une fenêtre
    /// dont le navigateur émet réellement, indicateur de capture allumé, est
    /// le mensonge visuel que la spec §9 « Vie privée » écarte.
    it("un refus d'exclusivité change le LIBELLÉ et le bandeau, jamais l'état", async () => {
        const { bouton, controle, messages } = await boutonAllume();
        const titreNominal = bouton.title;

        controle.annoncerExclusivite(false);

        expect(bouton.dataset.etat).toBe('actif');
        expect(bouton.disabled).toBe(false);
        expect(bouton.title).not.toBe(titreNominal);
        expect(bouton.title).toMatch(/autre fenêtre/);
        expect(messages.at(-1)).toMatch(/autre fenêtre/);
    });

    it("la reprise repose le libellé nominal et le DIT", async () => {
        const { bouton, controle, messages } = await boutonAllume();
        const titreNominal = bouton.title;

        controle.annoncerExclusivite(false);
        controle.annoncerExclusivite(true);

        expect(bouton.title).toBe(titreNominal);
        expect(messages.at(-1)).toMatch(/de nouveau entendu/);
    });

    /// Sans ce garde, chaque `mic-state { granted: true }` — le cas COURANT,
    /// celui d'une fenêtre seule — pousserait un bandeau « de nouveau entendu »
    /// à une fenêtre qui n'a jamais cessé de l'être.
    it("un accord qui ne suit aucun refus ne pousse aucun bandeau", async () => {
        const { controle, messages } = await boutonAllume();
        const avant = messages.length;
        controle.annoncerExclusivite(true);
        controle.annoncerExclusivite(true);
        expect(messages.length).toBe(avant);
    });

    /// ⚠️ Un `mic-state` en vol qui arriverait après une extinction écraserait
    /// le titre d'un bouton FERMÉ avec un libellé qui parle d'un micro ouvert.
    it("un mic-state reçu micro fermé ne touche à rien", async () => {
        const bouton = fauxBouton();
        const messages: string[] = [];
        const controle = attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => fauxFlux(faussePiste()),
            surMessage: (texte) => messages.push(texte),
        });
        controle.annoncerDisponibilite(true);
        const titreFerme = bouton.title;

        controle.annoncerExclusivite(false);

        expect(bouton.dataset.etat).toBe('ferme');
        expect(bouton.title).toBe(titreFerme);
        expect(messages).toEqual([]);
    });

    /// Éteindre puis rallumer alors que l'autre fenêtre tient TOUJOURS le câble
    /// doit refaire monter le bandeau. Sans la remise à neuf du drapeau sur
    /// transition d'état, le second refus serait vu comme « pas un changement ».
    it("un refus qui dure remonte après une extinction et un rallumage", async () => {
        const { bouton, controle, messages } = await boutonAllume();
        controle.annoncerExclusivite(false);
        expect(messages.at(-1)).toMatch(/autre fenêtre/);

        bouton.cliquer();
        await controle.enCours();
        expect(bouton.dataset.etat).toBe('ferme');
        bouton.cliquer();
        await controle.enCours();
        expect(bouton.dataset.etat).toBe('actif');

        const avant = messages.length;
        controle.annoncerExclusivite(false);
        expect(messages.length).toBe(avant + 1);
        expect(bouton.title).toMatch(/autre fenêtre/);
    });
});
