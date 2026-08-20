// Le harnais partagé des tests du canal `/agent` : constantes, pair de test, et
// l'enrôlement d'une VM en base.
//
// 🔴 IL EST EXTRAIT AVANT L'ADDITION QU'IL SERT, jamais après. `canal.test.ts`
// portait ces quatre-vingts lignes et pesait 361 ; le sous-bloc G1 y ajoute une
// seconde famille de cas (le catalogue et le lancement), qui lui aurait fait
// franchir la porte de 450 lignes que `CLAUDE.md` fixe. Les recopier dans le
// fichier neuf aurait produit deux `ouvrirUrl` qui divergeraient à la première
// correction portée sur un seul des deux.
//
// ⚠️ CE MODULE VIT DANS `src/`, ET C'EST LA CONVENTION DU DÉPÔT pour un harnais
// de test : `base/harnais.ts` y est depuis P1, pour la même raison — un helper
// que plusieurs fichiers de test importent n'a pas d'autre endroit où vivre.

import { WebSocket } from 'ws';
import type { Pilote } from '../base/pilote';
import { enroler, lireParVm } from '../depot/agent';
import { hacher } from '../identite/mot-de-passe';

/// Le secret de signature du SERVICE — celui des jetons. À ne pas confondre
/// avec le secret d'ENRÔLEMENT ci-dessous : ils n'ont ni la même durée de vie,
/// ni le même détenteur, et les confondre dans un test rendrait le second
/// vérifiable par le premier.
export const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
/// Le secret d'enrôlement de la VM, celui que `npm run admin:agent` tire.
export const SECRET_VM = 'un-secret-d-enrolement-de-la-vraie-longueur';
/// Une époque réelle : les petites valeurs ne mesurent rien (leçon de P1).
export const T0 = 1_787_000_000_000;
/// Un préfixe de la VRAIE longueur que `agents/prefixe.ts` produit.
export const P = 'RhH1x2QmTz9kLpVbNc7dAw';

/// Enrôle une VM, ligne `vm` comprise : `agent_enrole.vm_id` la RÉFÉRENCE
/// (`0003-agents.sql`), et SQLite applique la clé étrangère. L'empreinte est
/// une VRAIE empreinte `scrypt`, jamais une chaîne courte.
export async function enrolerUneVm(p: Pilote, vmId: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [
        vmId,
        `vm-${vmId}`,
        '192.168.3.2',
    ]);
    await enroler(p, vmId, await hacher(SECRET_VM), P);
}

export interface Pair {
    socket: WebSocket;
    /// L'ordre RÉEL des évènements, `message` et `close` tels qu'ils sont
    /// arrivés : c'est ce qui permet d'asserter qu'un refus est PARVENU avant
    /// la fermeture, et non l'inverse.
    ordre: string[];
    ferme: Promise<void>;
    /// Envoie un message brut et rend la réponse. BORNÉE, jamais une attente
    /// infinie : un canal muet doit rougir, pas pendre.
    dire(brut: string): Promise<Record<string, unknown>>;
    /// Attend le prochain message POUSSÉ par le canal, SANS rien envoyer.
    ///
    /// 🔴 ELLE N'EST PAS UN `dire('')`. Le canal `/agent` pousse désormais des
    /// ordres que rien n'a demandés au pair (`lancer`), et les attendre par un
    /// envoi bidon serait une COURSE : `dire` n'envoie que si aucun message
    /// n'est déjà arrivé, si bien que le test enverrait — ou n'enverrait pas —
    /// selon l'ordonnancement, et provoquerait un refus `forme` une fois sur
    /// deux. Bornée pour la même raison que `dire`.
    recevoir(): Promise<Record<string, unknown>>;
}

/// Ouvre un pair sur une URL complète — le chemin compte, le service en
/// routant deux (`http/serveur.ts`).
export function ouvrirUrl(url: string): Promise<Pair> {
    return new Promise((resolve, reject) => {
        const w = new WebSocket(url);
        const enAttente: ((m: Record<string, unknown>) => void)[] = [];
        const recus: Record<string, unknown>[] = [];
        const pair: Pair = {
            socket: w,
            ordre: [],
            ferme: new Promise((r) => w.once('close', () => r())),
            recevoir() {
                return new Promise((r, rej) => {
                    const minuteur = setTimeout(
                        () => rej(new Error('aucun message poussé par le canal en 2000 ms')),
                        2000,
                    );
                    enAttente.push((m) => {
                        clearTimeout(minuteur);
                        r(m);
                    });
                    const dejaLa = recus.shift();
                    if (dejaLa) enAttente.shift()!(dejaLa);
                });
            },
            dire(brut) {
                return new Promise((r, rej) => {
                    const minuteur = setTimeout(
                        () => rej(new Error(`aucune réponse du canal en 2000 ms à ${brut}`)),
                        2000,
                    );
                    enAttente.push((m) => {
                        clearTimeout(minuteur);
                        r(m);
                    });
                    const dejaLa = recus.shift();
                    if (dejaLa) enAttente.shift()!(dejaLa);
                    else w.send(brut);
                });
            },
        };
        w.on('message', (brut) => {
            pair.ordre.push('message');
            const m = JSON.parse(brut.toString()) as Record<string, unknown>;
            const attendu = enAttente.shift();
            if (attendu) attendu(m);
            else recus.push(m);
        });
        w.on('close', () => pair.ordre.push('close'));
        w.once('open', () => resolve(pair));
        w.once('error', (cause) => reject(cause));
    });
}

export function ouvrir(port: number): Promise<Pair> {
    return ouvrirUrl(`ws://127.0.0.1:${port}`);
}

/// Attend que `vu_a` satisfasse `predicat`, ou ÉCHOUE au bout de `borneMs`.
///
/// ⚠️ BORNÉE, ET ÉCHOUANT SUR EXPIRATION. L'écriture de `vu_a` est délibérément
/// lancée SANS être attendue (règle de P1 : une promesse rejetée dans un
/// gestionnaire `ws` abat tout le process), donc une écriture perdue ne se
/// manifeste que par une valeur qui n'arrive pas. Une boucle sans borne
/// pendrait au lieu de rougir.
export async function attendreVu(
    p: Pilote,
    vmId: string,
    predicat: (vu: number | null) => boolean,
    quoi: string,
    borneMs = 2000,
): Promise<number | null> {
    const fin = Date.now() + borneMs;
    for (;;) {
        const ligne = await lireParVm(p, vmId);
        const vu = ligne?.vu_a === undefined || ligne.vu_a === null ? null : Number(ligne.vu_a);
        if (predicat(vu)) return vu;
        if (Date.now() > fin) {
            throw new Error(`vu_a ${quoi} jamais atteint pour ${vmId} en ${borneMs} ms (vu=${vu})`);
        }
        await new Promise((r) => setTimeout(r, 25));
    }
}
