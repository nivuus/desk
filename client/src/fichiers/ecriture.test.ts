import { describe, expect, it } from 'vitest';
import { EchecFichiers } from './adaptateur';
import { creerEcrivain, type FluxInscriptible, type RacineInscriptible } from './ecriture';

/* ── UN FAUX SYSTÈME DE FICHIERS, INSENSIBLE À LA CASSE PAR CONSTRUCTION ──
   🔴 L'INSENSIBILITÉ EST LE POINT, ET NON UN DÉTAIL DE COMMODITÉ. C'est ce que
   font Windows et macOS par défaut, et c'est ce qui rend la perte possible :
   `getFileHandle('CASSE.TXT', { create: true })` y ouvre `Casse.txt`. Un faux
   SENSIBLE à la casse rendrait le test de la garde VACUEUX — il créerait
   simplement un second fichier, et rien ne serait jamais écrasé. */

interface Noeud {
    kind: 'file' | 'directory';
    name: string;
    contenu: number[];
    enfants: Map<string, Noeud>;
}

function dossier(name = ''): Noeud {
    return { kind: 'directory', name, contenu: [], enfants: new Map() };
}

class Faux {
    ouverts = 0;
    fermes = 0;
    racineNoeud = dossier();

    /** Recherche INSENSIBLE à la casse, comme un poste local ordinaire. */
    private trouver(parent: Noeud, nom: string): Noeud | undefined {
        for (const enfant of parent.enfants.values()) {
            if (enfant.name.toLowerCase() === nom.toLowerCase()) return enfant;
        }
        return undefined;
    }

    poser(chemin: string, contenu: number[] = []): void {
        const parts = chemin.split('/');
        let ici = this.racineNoeud;
        for (const p of parts.slice(0, -1)) {
            let suivant = this.trouver(ici, p);
            if (suivant === undefined) {
                suivant = dossier(p);
                ici.enfants.set(p, suivant);
            }
            ici = suivant;
        }
        const nom = parts[parts.length - 1];
        ici.enfants.set(nom, { kind: 'file', name: nom, contenu, enfants: new Map() });
    }

    lire(chemin: string): { nom: string; contenu: number[] } | undefined {
        const parts = chemin.split('/');
        let ici = this.racineNoeud;
        for (const p of parts.slice(0, -1)) {
            const suivant = this.trouver(ici, p);
            if (suivant === undefined) return undefined;
            ici = suivant;
        }
        const n = this.trouver(ici, parts[parts.length - 1]);
        return n === undefined ? undefined : { nom: n.name, contenu: n.contenu };
    }

    racine(): RacineInscriptible {
        return this.envelopper(this.racineNoeud);
    }

    private envelopper(noeud: Noeud): RacineInscriptible {
        const faux = this;
        return {
            kind: 'directory',
            name: noeud.name,
            async getDirectoryHandle(nom, options) {
                let enfant = faux.trouver(noeud, nom);
                if (enfant === undefined) {
                    if (!options?.create) throw new DOMException('absent', 'NotFoundError');
                    enfant = dossier(nom);
                    noeud.enfants.set(nom, enfant);
                }
                return faux.envelopper(enfant);
            },
            async getFileHandle(nom, options) {
                let enfant = faux.trouver(noeud, nom);
                if (enfant === undefined) {
                    if (!options?.create) throw new DOMException('absent', 'NotFoundError');
                    enfant = { kind: 'file', name: nom, contenu: [], enfants: new Map() };
                    noeud.enfants.set(nom, enfant);
                }
                const cible = enfant;
                return {
                    kind: 'file',
                    name: cible.name,
                    async getFile() {
                        return {
                            size: cible.contenu.length,
                            lastModified: 0,
                            slice: () => ({ arrayBuffer: async () => new ArrayBuffer(0) }),
                            arrayBuffer: async () => new ArrayBuffer(0),
                        };
                    },
                    async createWritable(options): Promise<FluxInscriptible> {
                        faux.ouverts += 1;
                        // 🔴 SANS `keepExistingData`, LE FICHIER PART DE ZÉRO.
                        // C'est ce que le vrai `createWritable()` fait, et c'est
                        // ce qui empêche qu'un fichier réécrit plus court garde
                        // sa queue d'octets.
                        const tampon = options?.keepExistingData ? [...cible.contenu] : [];
                        return {
                            async write({ position, data }) {
                                for (let i = 0; i < data.length; i += 1) {
                                    tampon[position + i] = data[i];
                                }
                            },
                            async close() {
                                faux.fermes += 1;
                                cible.contenu = tampon.map((o) => o ?? 0);
                            },
                        };
                    },
                };
            },
            async *values() {
                for (const enfant of noeud.enfants.values()) {
                    yield { kind: enfant.kind, name: enfant.name };
                }
            },
        };
    }
}

const octets = (...o: number[]) => new Uint8Array(o);

describe('la garde de casse', () => {
    it('🔴 REFUSE d’écrire dans un homonyme de casse', async () => {
        const faux = new Faux();
        faux.poser('Casse.txt', [1, 2, 3]);
        const e = creerEcrivain(faux.racine());
        await expect(e.ecrire('CASSE.TXT', 0, octets(9), true, true)).rejects.toThrow(
            EchecFichiers,
        );
    });

    it('🔴 …ET `Casse.txt` N’EST PAS ÉCRASÉ — c’est LE test de F2', async () => {
        // 🔴 **UN TEST SÉPARÉ, ET C'EST LA LEÇON ①A-bis DE P2.** `expect`
        // interrompt un test à sa PREMIÈRE assertion en échec : mettre le refus
        // et le non-écrasement dans le même test ferait que le second ne serait
        // ÉPROUVÉ PAR RIEN dès que le premier tombe — et c'est le second qui
        // porte la perte de données. La rouge le montre : sans la garde, celui
        // du dessus échoue sur « promise resolved instead of rejecting », et
        // celui-ci sur le CONTENU.
        const faux = new Faux();
        faux.poser('Casse.txt', [1, 2, 3]);
        const e = creerEcrivain(faux.racine());
        await e.ecrire('CASSE.TXT', 0, octets(9), true, true).catch(() => {});
        expect(faux.lire('Casse.txt')).toEqual({ nom: 'Casse.txt', contenu: [1, 2, 3] });
    });

    it('🔴 …ET AUCUN FLUX N’EST MÊME OUVERT', async () => {
        // Troisième assertion, troisième test, même raison. « Rien n'est
        // écrit » et « rien n'est même ouvert » ne se déduisent pas l'un de
        // l'autre : un flux ouvert puis abandonné laisse un fichier d'échange.
        const faux = new Faux();
        faux.poser('Casse.txt', [1, 2, 3]);
        const e = creerEcrivain(faux.racine());
        await e.ecrire('CASSE.TXT', 0, octets(9), true, true).catch(() => {});
        expect(faux.ouverts).toBe(0);
    });

    it('porte le code `casse-ambigue` et NOMME les deux fichiers', async () => {
        const faux = new Faux();
        faux.poser('Casse.txt');
        const e = creerEcrivain(faux.racine());
        const erreur = await e
            .ecrire('CASSE.TXT', 0, octets(9), true, true)
            .then(() => undefined)
            .catch((x: unknown) => x as EchecFichiers);
        expect(erreur).toBeInstanceOf(EchecFichiers);
        if (erreur === undefined) throw new Error('inatteignable');
        expect(erreur.code).toBe('casse-ambigue');
        // Le message reste dans la console et dans la page-shell ; il doit dire
        // ce que l'utilisateur peut faire, c'est-à-dire renommer l'un des deux.
        expect(erreur.message).toContain('CASSE.TXT');
        expect(erreur.message).toContain('Casse.txt');
    });

    it('refuse aussi une CRÉATION ambiguë', async () => {
        const faux = new Faux();
        faux.poser('Dossier');
        const e = creerEcrivain(faux.racine());
        await expect(e.creer('DOSSIER', true)).rejects.toThrow(/casse/);
    });

    it('écrit dans le nom EXACT quand il existe', async () => {
        // 🔴 Une garde trop stricte ferait échouer TOUTE écriture : ce test est
        // ce qui l'empêche.
        const faux = new Faux();
        faux.poser('Casse.txt', [1, 2, 3]);
        const e = creerEcrivain(faux.racine());
        await e.ecrire('Casse.txt', 0, octets(7, 8), true, true);
        expect(faux.lire('Casse.txt')?.contenu).toEqual([7, 8]);
    });

    it('crée quand rien ne ressemble au nom demandé', async () => {
        const faux = new Faux();
        faux.poser('autre.txt');
        const e = creerEcrivain(faux.racine());
        await e.ecrire('neuf.txt', 0, octets(4), true, true);
        expect(faux.lire('neuf.txt')?.contenu).toEqual([4]);
    });
});

describe('les flux', () => {
    it('🔴 un fichier réécrit PLUS COURT ne garde pas sa queue d’octets', async () => {
        // 🔴 C'est le défaut EXACT de l'ancien pont (spec §12) : il employait
        // `keepExistingData: true` sans `truncate`. Passer `true` ici fait
        // survivre la queue, et le fichier local porte alors un contenu que la
        // VM n'a JAMAIS eu.
        const faux = new Faux();
        faux.poser('note.txt', [1, 2, 3, 4, 5, 6, 7, 8]);
        const e = creerEcrivain(faux.racine());
        await e.ecrire('note.txt', 0, octets(9, 9), true, true);
        expect(faux.lire('note.txt')?.contenu).toEqual([9, 9]);
    });

    it('ouvre UNE fois et ferme UNE fois, sur plusieurs morceaux', async () => {
        const faux = new Faux();
        const e = creerEcrivain(faux.racine());
        await e.ecrire('gros.bin', 0, octets(1, 2), true, false);
        await e.ecrire('gros.bin', 2, octets(3, 4), false, false);
        await e.ecrire('gros.bin', 4, octets(5), false, true);
        expect([faux.ouverts, faux.fermes]).toEqual([1, 1]);
        expect(faux.lire('gros.bin')?.contenu).toEqual([1, 2, 3, 4, 5]);
    });

    it('n’écrit RIEN tant que le dernier morceau n’est pas arrivé', async () => {
        // 🔵 L'ATOMICITÉ de `createWritable()` : la committaison est au
        // `close()`. Une poussée interrompue laisse le fichier local INCHANGÉ.
        const faux = new Faux();
        faux.poser('note.txt', [42]);
        const e = creerEcrivain(faux.racine());
        await e.ecrire('note.txt', 0, octets(1, 2), true, false);
        // Inchangé AVANT le `close()` : c'est l'atomicité.
        expect(faux.lire('note.txt')?.contenu).toEqual([42]);
        await e.ecrire('note.txt', 2, octets(3), false, true);
        expect(faux.lire('note.txt')?.contenu).toEqual([1, 2, 3]);
    });

    it('🔴 abandonner ferme les flux restés ouverts', async () => {
        const faux = new Faux();
        const e = creerEcrivain(faux.racine());
        await e.ecrire('a.txt', 0, octets(1), true, false);
        expect(faux.fermes).toBe(0);
        e.abandonner();
        // `abandonner` est SYNCHRONE : le `close()` est lancé sans être attendu,
        // parce qu'il est appelé depuis la fermeture du canal, qui l'est aussi.
        await Promise.resolve();
        expect(faux.fermes).toBe(1);
    });

    it('un rejeu ferme le flux précédent au lieu d’en laisser deux', async () => {
        const faux = new Faux();
        const e = creerEcrivain(faux.racine());
        await e.ecrire('a.txt', 0, octets(1), true, false);
        // La poussée est interrompue, puis relancée depuis le début.
        await e.ecrire('a.txt', 0, octets(7, 7), true, true);
        expect([faux.ouverts, faux.fermes]).toEqual([2, 2]);
        expect(faux.lire('a.txt')?.contenu).toEqual([7, 7]);
    });

    it('🔴 refuse un morceau NON initial sans flux ouvert', async () => {
        // Ouvrir ici écrirait un fichier TRONQUÉ à ce morceau-ci : la
        // troncature serait silencieuse, ce qui est pire qu'un refus.
        const faux = new Faux();
        const e = creerEcrivain(faux.racine());
        await expect(e.ecrire('a.txt', 64, octets(1), false, true)).rejects.toThrow(/flux/);
        expect(faux.lire('a.txt')).toBeUndefined();
    });
});

describe('les créations', () => {
    it('crée un répertoire, et un fichier VIDE sans le tronquer', async () => {
        const faux = new Faux();
        faux.poser('deja.txt', [1, 2, 3]);
        const e = creerEcrivain(faux.racine());
        await e.creer('dossier', true);
        await e.creer('deja.txt', false);
        // 🔴 UNE CRÉATION N'OUVRE AUCUN FLUX : en ouvrir un TRONQUERAIT le
        // fichier local existant, alors qu'une création est sans effet sur ce
        // qui est déjà là.
        expect(faux.lire('deja.txt')?.contenu).toEqual([1, 2, 3]);
        expect(faux.ouverts).toBe(0);
    });

    it('crée les répertoires intermédiaires d’un chemin profond', async () => {
        const faux = new Faux();
        const e = creerEcrivain(faux.racine());
        await e.ecrire('a/b/c.txt', 0, octets(5), true, true);
        expect(faux.lire('a/b/c.txt')?.contenu).toEqual([5]);
    });
});

describe('le classement des échecs', () => {
    it('🔴 un quota dépassé rend `disque-plein`, et pas `interne`', async () => {
        // Le laisser tomber dans le `default` de `classer` ferait `interne`, et
        // le journal ne dirait plus POURQUOI : l'utilisateur ne saurait pas
        // qu'il doit libérer de la place.
        const faux = new Faux();
        const racine = faux.racine();
        const vraiGetFileHandle = racine.getFileHandle.bind(racine);
        racine.getFileHandle = async (nom, options) => {
            const poignee = await vraiGetFileHandle(nom, options);
            return {
                ...poignee,
                createWritable: async () => {
                    throw new DOMException('plein', 'QuotaExceededError');
                },
            };
        };
        const e = creerEcrivain(racine);
        const erreur = await e
            .ecrire('a.txt', 0, octets(1), true, true)
            .then(() => undefined)
            .catch((x: unknown) => x as EchecFichiers);
        expect(erreur).toBeInstanceOf(EchecFichiers);
        expect(erreur?.code).toBe('disque-plein');
    });
});
