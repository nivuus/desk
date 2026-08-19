// La vérification d'un secret d'enrôlement, et le refus qui n'énumère pas.
//
// 🔴 LA ROUGE CENTRALE DE CE FICHIER est le refus INDISTINCT : une VM inconnue
// et un secret faux doivent rendre le MÊME refus, mot pour mot. Deux motifs
// distincts seraient un oracle d'énumération — l'appelant apprendrait par
// tâtonnement quelles VMs existent —, et c'est littéralement ce que la spec
// §4 P3 ② nomme comme la rouge de ce critère.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { hacher } from '../identite/mot-de-passe';
import { enroler } from '../depot/agent';
import { verifierEnrolement } from './enrolement';

let base: Pilote | undefined;

const PREFIXE = 'RhH1x2QmTz9kLpVbNc7dAw';
const SECRET = 'un-secret-d-enrolement-tire-au-sort-et-long';

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function baseEnrolee(nom: string): Promise<Pilote> {
    const p = await baseNeuve(nom);
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-1', 'vm-1', '192.168.3.2']);
    await enroler(p, 'v-1', await hacher(SECRET), PREFIXE);
    return p;
}

describe(`enrôlement d'un agent, moteur=${MOTEUR}`, () => {
    it('accepte le bon couple, et rend le préfixe de la VM', async () => {
        base = await baseEnrolee('enrol-bon');
        const journal: string[] = [];
        const v = await verifierEnrolement(base, 'v-1', SECRET, (l) => journal.push(l));
        expect(v).toEqual({ ok: true, vmId: 'v-1', prefixe: PREFIXE });
        // Un succès ne journalise pas de refus.
        expect(journal).toEqual([]);
    });

    it('🔴 refuse une VM INCONNUE et un SECRET FAUX du MÊME refus, mot pour mot', async () => {
        // 🔴 La rouge : rendre `'vm-inconnue'` d'un côté et `'secret-invalide'`
        // de l'autre. C'est un oracle d'énumération, et il suffit d'une
        // différence d'UN caractère pour qu'il le redevienne — d'où la
        // comparaison stricte des deux objets entiers.
        base = await baseEnrolee('enrol-indistinct');
        const inconnue = await verifierEnrolement(base, 'v-jamais-enrolee', SECRET, () => {});
        const fauxSecret = await verifierEnrolement(base, 'v-1', 'pas-le-bon-secret', () => {});
        expect(inconnue.ok).toBe(false);
        expect(fauxSecret.ok).toBe(false);
        expect(inconnue).toEqual(fauxSecret);
    });

    it('journalise le refus AVEC le nom de VM demandé', async () => {
        // 🔴 La rouge : ne rien journaliser. Le refus devient alors
        // indiagnosticable — et c'est le prix exact de l'indistinction
        // ci-dessus : ce que le demandeur n'apprend pas, l'exploitant doit
        // pouvoir le lire chez lui. Même partage `message` / `journal` que
        // `identite/garde.ts`.
        base = await baseEnrolee('enrol-journal');
        const journal: string[] = [];
        await verifierEnrolement(base, 'v-jamais-enrolee', SECRET, (l) => journal.push(l));
        expect(journal).toHaveLength(1);
        expect(journal[0]).toContain('v-jamais-enrolee');
        // 🔴 ET IL NE RECOPIE JAMAIS LE SECRET. Le balayage du critère ④ de P2
        // s'applique tel quel : on cherche le NOM du champ autant que la
        // valeur.
        expect(journal[0]).not.toContain(SECRET);
        expect(journal[0]).not.toContain('secret=');
    });

    it('🔴 rend un refus, JAMAIS une exception, sur une empreinte TRONQUÉE', async () => {
        // 🔴 La rouge : comparer sans egaliser les longueurs d'abord. MESURÉ en
        // P2 : `timingSafeEqual` LÈVE `Input buffers must have the same byte
        // length`, et l'appelant répondrait une erreur interne là où il doit
        // répondre un refus — l'écart de comportement serait à lui seul un
        // oracle. `identite/mot-de-passe.ts` gère déjà ce cas ; ce test
        // vérifie que l'enrôlement ne le défait pas.
        base = await baseNeuve('enrol-tronquee');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-1', 'vm-1', '10.0.0.1']);
        const entiere = await hacher(SECRET);
        await enroler(base, 'v-1', entiere.slice(0, entiere.length - 10), PREFIXE);

        const v = await verifierEnrolement(base, 'v-1', SECRET, () => {});
        expect(v.ok).toBe(false);
    });

    it('LAISSE PASSER l’exception d’un algorithme inconnu, sans la transformer en refus', async () => {
        // ⚠️ `identite/mot-de-passe.ts::verifier` LÈVE délibérément sur un
        // algorithme inconnu : un refus muet y serait indiscernable d'un secret
        // faux, et personne ne saurait diagnostiquer une base écrite par une
        // version future du service. L'enrôlement ne doit donc PAS l'avaler —
        // c'est le canal qui la traduira en refus, en la journalisant AVEC sa
        // cause.
        base = await baseNeuve('enrol-algo');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-1', 'vm-1', '10.0.0.1']);
        await enroler(base, 'v-1', 'argon2id$1$2$3$sel$empreinte', PREFIXE);
        await expect(verifierEnrolement(base, 'v-1', SECRET, () => {}))
            .rejects.toThrow(/algorithme de hachage inconnu/i);
    });
});
