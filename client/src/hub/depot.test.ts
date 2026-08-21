import { describe, expect, it } from 'vitest';
import { deposer, resumer } from './depot';
import type { DepsTeleversement, Issue } from './televersement';

const FICHIER = new File([new Uint8Array([1, 2, 3])], 'app.msi');

describe('resumer', () => {
    it('rend un ton de SUCCÈS et nomme le fichier quand le scellement passe', () => {
        const issue: Issue = { etat: 'scelle', id: 't-1', taille: 3, sha256: 'ab', deposees: [0] };
        expect(resumer(FICHIER, issue)).toEqual({
            ton: 'succes',
            texte: 'app.msi a été téléversé et scellé (1 tranche(s) déposée(s)).',
            id: 't-1',
        });
    });

    it('rend le MOTIF DU SERVICE tel quel, jamais réécrit', () => {
        const issue: Issue = {
            etat: 'refus',
            id: 't-2',
            refus: { source: 'service', etape: 'scellement', statut: 409, motif: 'empreinte-divergente' },
        };
        const r = resumer(FICHIER, issue);
        expect(r.ton).toBe('danger');
        // 🔴 LE MOTIF DOIT APPARAÎTRE MOT POUR MOT : le traduire en ferait une
        //    copie qu'aucun type ne confronte à sa source.
        expect(r.texte).toContain('empreinte-divergente');
        expect(r.id).toBe('t-2');
    });

    it('distingue un refus LOCAL d\'un refus du SERVICE', () => {
        const issue: Issue = {
            etat: 'refus',
            refus: { source: 'client', motif: 'fichier-different', detail: 'la taille a changé' },
        };
        const r = resumer(FICHIER, issue);
        expect(r.texte).toContain('fichier-different');
        expect(r.texte).toContain('la taille a changé');
        expect(r.texte).not.toContain('service');
        expect(r.id).toBeUndefined();
    });
});

describe('deposer — le point de convergence', () => {
    /// 🔴 CE TEST EST CELUI QUI REND LA ROUGE DU CRITÈRE ② HONNÊTE : il éprouve
    ///    que `deposer` mène RÉELLEMENT au téléversement, et pas seulement
    ///    qu'il rend un objet. Les deux chemins du hub — glisser-déposer et
    ///    `launchQueue` — appellent CETTE fonction, et aucune autre.
    it('mène un fichier jusqu\'au scellement, et le rapporte', async () => {
        const vus: string[] = [];
        const deps: DepsTeleversement = {
            base: 'https://x',
            jeton: 'J',
            maintenant: () => 0,
            fetch: async (url, init) => {
                vus.push(`${init?.method ?? 'GET'} ${url.replace('https://x', '')}`);
                if (url.endsWith('/televersement')) {
                    // ⚠️ `taille_tranche` EST UN CONTRAT, pas un ornement :
                    //    `televerser` refuse en `etat-illisible` sans lui. Un
                    //    premier jet de ce factice l'omettait, et le test a
                    //    rougi — il éprouve donc bien la séquence RÉELLE.
                    //    À 2 octets par tranche, un fichier de 3 en fait DEUX.
                    return {
                        ok: true,
                        status: 201,
                        json: async () => ({ id: 't-9', taille_tranche: 2, tranches_presentes: [] }),
                    };
                }
                return { ok: true, status: 200, json: async () => ({}) };
            },
        };
        const resume = await deposer(FICHIER, deps);
        expect(resume.ton).toBe('succes');
        // La séquence RÉELLE : création, une tranche, scellement.
        expect(vus[0]).toBe('POST /televersement');
        expect(vus.filter((v) => v.startsWith('PUT /televersement/t-9/tranche/'))).toEqual([
            'PUT /televersement/t-9/tranche/0',
            'PUT /televersement/t-9/tranche/1',
        ]);
        expect(vus.at(-1)).toBe('POST /televersement/t-9/sceller');
    });

    it('rend un refus du service SANS lever', async () => {
        const deps: DepsTeleversement = {
            base: 'https://x',
            jeton: 'J',
            maintenant: () => 0,
            fetch: async () => ({ ok: false, status: 413, json: async () => ({ refus: 'trop-gros' }) }),
        };
        const resume = await deposer(FICHIER, deps);
        expect(resume.ton).toBe('danger');
        expect(resume.texte).toContain('trop-gros');
    });
});
