// LA CONTRE-PRESSION DU CANAL — l'autre moitié du contrôle de flux de F3.
// **PUR** : le canal lui est INJECTÉ, décrit par ce dont on se sert, et ce
// module ne connaît ni `RTCDataChannel`, ni le DOM, ni la trame.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 POURQUOI ELLE EST ICI ET NON DANS LE PONT
// ════════════════════════════════════════════════════════════════════════════
//
// La spec §7.3 pose : « le pont ne demande pas le morceau n+1 tant que le canal
// a plus de `SEUIL_TAMPON` octets en attente ». Mais **c'est le NAVIGATEUR qui
// émet les gros messages** — les octets d'un fichier lu —, et
// `bufferedAmount` est une propriété de SON canal. **Le pont ne la voit pas et
// ne peut pas la voir.**
//
// Les deux moitiés sont indissociables, et F3 livre **les deux ou aucune** :
// la fenêtre du pont (`agent/src/pont/lecture.rs`) sans la contre-pression
// remplirait la file SCTP ; la contre-pression sans la fenêtre n'aurait **rien
// à retenir**, puisque le pont ne demanderait jamais le morceau n+1 avant
// d'avoir reçu le n.
//
// ⚠️ **RELEVÉ DANS LE CODE DE F1/F2** : `canal.ts` ne posait **aucun**
// `bufferedAmountLowThreshold` — il ne passait que `{ ordered: true }` — et
// envoyait sans rien regarder, alors que la spec §3.4 l'exige (« posé »).
//
// ⚠️ **F3 NE REVENDIQUE AUCUN GAIN DE DÉBIT.** La seule mesure de débit du
// dépôt varie d'un facteur ~120 sans explication (F1 §11). C'est F4 qui jugera.

/**
 * Le sous-ensemble d'un `RTCDataChannel` dont la contre-pression se sert.
 *
 * ⚠️ **Un SOUS-ENSEMBLE STRUCTUREL**, comme les poignées d'`adaptateur.ts` : la
 * vraie classe le satisfait sans conversion (`canal.ts` le vérifie à la
 * compilation), et un faux en mémoire aussi. C'est ce qui rend ce module
 * testable sous le Node de Vitest, où `RTCDataChannel` n'existe pas.
 */
export interface CanalSortant {
    readonly bufferedAmount: number;
    bufferedAmountLowThreshold: number;
    readonly readyState: 'connecting' | 'open' | 'closing' | 'closed';
    addEventListener(type: 'bufferedamountlow' | 'close', ecouteur: () => void): void;
    removeEventListener(type: 'bufferedamountlow' | 'close', ecouteur: () => void): void;
}

/**
 * Combien d'octets peuvent attendre dans le tampon du canal avant qu'on cesse
 * d'émettre.
 *
 * ⚠️ **NON CALIBRÉE.** Elle rejoint `MORCEAUX_EN_VOL`, `DELAI_MUTATION`,
 * `PERIODE_RECENSEMENT`, les quatre de F1, celles de F2 et les huit du chantier
 * D dans la liste des constantes qu'aucune mesure n'a jugées.
 *
 * **Pourquoi cet ordre de grandeur, et c'est un RAISONNEMENT, pas une mesure** :
 * `MORCEAUX_EN_VOL` (4) × `TAILLE_TRAME_MAX` (64 Kio) = 256 Kio de réponses en
 * vol au plus. Le seuil est posé au quart, de sorte que le tampon se vide
 * avant que la fenêtre ne soit pleine — sans quoi la contre-pression ne
 * mordrait **jamais**, et serait un mécanisme incapable de se déclencher.
 */
export const SEUIL_TAMPON = 64 * 1024;

export interface ContrePression {
    /**
     * Attend que le tampon soit redescendu sous le seuil.
     *
     * 🔴 **REND IMMÉDIATEMENT si le canal est FERMÉ**, et ne suspend jamais :
     * un canal fermé n'émettra plus jamais `bufferedamountlow`, et l'attente ne
     * se terminerait donc **jamais** — un blocage PIRE que celui qu'on répare,
     * puisqu'il figerait la page au lieu de ralentir un transfert.
     */
    avantEnvoi(): Promise<void>;
}

export function contrePression(canal: CanalSortant, seuil = SEUIL_TAMPON): ContrePression {
    // ⚠️ **Le seuil est posé UNE FOIS, à la construction.** Le poser à chaque
    // envoi serait une écriture par trame sur un objet du navigateur, et le
    // changer en cours de route ferait qu'un `bufferedamountlow` déjà armé
    // se déclencherait sur l'ancienne valeur.
    canal.bufferedAmountLowThreshold = seuil;
    return {
        async avantEnvoi(): Promise<void> {
            if (canal.readyState !== 'open') return;
            if (canal.bufferedAmount <= seuil) return;
            await new Promise<void>((resolve) => {
                // ⚠️ **LES DEUX ÉCOUTEURS SONT RETIRÉS, quel que soit celui qui
                // gagne.** Les laisser ferait exactement le défaut relevé de
                // l'ancien pont : un écouteur `message` posé PAR REQUÊTE et
                // jamais retiré (`src/file.js:155`), dont le coût croissait
                // avec le nombre d'opérations passées, indéfiniment.
                const finir = (): void => {
                    canal.removeEventListener('bufferedamountlow', finir);
                    canal.removeEventListener('close', finir);
                    resolve();
                };
                canal.addEventListener('bufferedamountlow', finir);
                // 🔴 **`close` LIBÈRE AUSSI**, sans quoi une lecture en cours
                // figerait la page à la fermeture de l'onglet distant.
                canal.addEventListener('close', finir);
                // ⚠️ **RE-CONTRÔLE APRÈS L'ABONNEMENT.** Le tampon a pu
                // redescendre entre le test ci-dessus et l'abonnement :
                // l'événement serait alors déjà passé, et l'attente ne se
                // terminerait jamais. C'est la course classique de tout
                // mécanisme « tester puis attendre ».
                if (canal.bufferedAmount <= seuil || canal.readyState !== 'open') finir();
            });
        },
    };
}
