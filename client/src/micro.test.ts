import { describe, expect, it, vi } from 'vitest';

import { attacherMicro, type EtatMicro } from './micro';

/// Piste factice : ce que ce module fait d'une `MediaStreamTrack` se réduit à
/// `stop()`, et c'est précisément l'appel que la spec §9 rend obligatoire.
/// `arretee` est le seul instrument capable de distinguer une extinction RÉELLE
/// d'un `enabled = false` — le « mensonge visuel » que la spec écarte.
function faussePiste(): MediaStreamTrack & { arretee: boolean; enabled: boolean } {
    return {
        kind: 'audio',
        enabled: true,
        arretee: false,
        stop(this: { arretee: boolean }) {
            this.arretee = true;
        },
    } as unknown as MediaStreamTrack & { arretee: boolean; enabled: boolean };
}

/// Flux factice porteur d'une seule piste audio, comme en rend `getUserMedia`.
function fauxFlux(piste: MediaStreamTrack): MediaStream {
    return { getAudioTracks: () => [piste] } as unknown as MediaStream;
}

/// Sender factice : mémorise TOUT ce qui lui est passé, dans l'ordre. Un
/// booléen « a reçu une piste » ne distinguerait pas une extinction d'une
/// absence d'allumage.
function fauxSender() {
    const recus: Array<MediaStreamTrack | null> = [];
    return {
        recus,
        replaceTrack(piste: MediaStreamTrack | null): Promise<void> {
            recus.push(piste);
            return Promise.resolve();
        },
    };
}

/// Erreur telle que `getUserMedia` la lève : c'est le `name` qui porte le sens,
/// jamais le message.
function erreurDom(name: string): Error {
    const e = new Error(name);
    e.name = name;
    return e;
}

describe('attacherMicro — la bascule et ses quatre états', () => {
    it('au départ, le sender n’a pas de piste et l’état est « fermé »', () => {
        const sender = fauxSender();
        const micro = attacherMicro({
            sender,
            demanderFlux: async () => fauxFlux(faussePiste()),
        });

        // Rien n'a été demandé au navigateur : la permission se demande AU
        // CLIC, jamais à l'ouverture de session (spec §9, « Vie privée »).
        expect(sender.recus).toEqual([]);
        expect(micro.etat()).toBe('ferme');
    });

    it('la bascule appelle getUserMedia une seule fois, puis replaceTrack', async () => {
        const piste = faussePiste();
        const sender = fauxSender();
        const demanderFlux = vi.fn(async () => fauxFlux(piste));
        const etats: Array<[EtatMicro, string | undefined]> = [];
        const micro = attacherMicro({
            sender,
            demanderFlux,
            surEtat: (etat, detail) => etats.push([etat, detail]),
        });

        await expect(micro.basculer()).resolves.toBe('actif');

        expect(demanderFlux).toHaveBeenCalledTimes(1);
        // Les contraintes de la spec §7 : AEC, suppression de bruit, gain
        // automatique. L'AEC est celle du NAVIGATEUR, seul endroit qui
        // connaisse à la fois le flux capté et le flux restitué.
        //
        // ⚠️ LE LITTÉRAL EST RECOPIÉ, ET C'EST LE FOND DU TEST. La rédaction
        // précédente importait la constante du module et assertait
        // `toHaveBeenCalledWith(CONTRAINTES)` : les deux côtés de l'égalité
        // lisaient le même objet, et la remplacer par `audio: true` laissait
        // ce test VERT. La mutation a été jouée et vue verte avant correction.
        expect(demanderFlux).toHaveBeenCalledWith({
            audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        });
        expect(sender.recus).toEqual([piste]);
        expect(micro.etat()).toBe('actif');
        expect(etats).toEqual([['actif', undefined]]);
    });

    it("l'extinction appelle replaceTrack(null) ET track.stop()", async () => {
        const piste = faussePiste();
        const sender = fauxSender();
        const micro = attacherMicro({ sender, demanderFlux: async () => fauxFlux(piste) });

        await micro.basculer();
        await expect(micro.basculer()).resolves.toBe('ferme');

        // ⚠️ LE TEST CENTRAL DE CE MODULE. `enabled = false` seul laisserait le
        // périphérique ouvert et l'indicateur de Chrome allumé : la spec §9
        // qualifie ce mensonge visuel d'inacceptable « sur cette fonction
        // précisément ». Les DEUX assertions sont nécessaires — un test qui ne
        // vérifierait que `replaceTrack(null)` ne distinguerait pas les deux
        // implémentations, et ne garderait donc rien.
        expect(piste.arretee).toBe(true);
        expect(sender.recus).toEqual([piste, null]);
        expect(micro.etat()).toBe('ferme');
    });

    it('un refus de permission mène à l’état « refusé » sans exception', async () => {
        const sender = fauxSender();
        const etats: Array<[EtatMicro, string | undefined]> = [];
        const micro = attacherMicro({
            sender,
            demanderFlux: async () => {
                throw erreurDom('NotAllowedError');
            },
            surEtat: (etat, detail) => etats.push([etat, detail]),
        });

        // Ne LÈVE PAS : le micro ne tue jamais une session qui fonctionne
        // (spec §10). `resolves` échouerait si la promesse rejetait.
        await expect(micro.basculer()).resolves.toBe('refuse');
        expect(micro.etat()).toBe('refuse');
        expect(sender.recus).toEqual([]);

        // Le détail porte COMMENT RÉTABLIR la permission (spec §10), pas
        // seulement le fait du refus : sans cela l'utilisateur qui a cliqué
        // « bloquer » une fois n'a plus aucun moyen de revenir en arrière.
        const [, detail] = etats[0];
        expect(detail).toMatch(/barre d'adresse/);
    });

    it("l'absence de périphérique d'entrée mène à « indisponible », pas à « refusé »", async () => {
        const sender = fauxSender();
        const micro = attacherMicro({
            sender,
            demanderFlux: async () => {
                throw erreurDom('NotFoundError');
            },
        });

        // Deux situations DISTINCTES au tableau de la spec §10 : « permission
        // refusée » veut un message pour la rétablir, « aucun périphérique
        // d'entrée » veut un bouton désactivé. Les confondre enverrait
        // l'utilisateur régler une permission qui n'est pas en cause.
        await expect(micro.basculer()).resolves.toBe('indisponible');
        expect(micro.etat()).toBe('indisponible');
    });

    it('un flux sans piste audio mène à « indisponible », pas à « actif »', async () => {
        const sender = fauxSender();
        const micro = attacherMicro({
            sender,
            demanderFlux: async () => ({ getAudioTracks: () => [] }) as unknown as MediaStream,
        });

        await expect(micro.basculer()).resolves.toBe('indisponible');
        expect(sender.recus).toEqual([]);
    });

    it('deux clics rapides ne demandent le flux qu’une fois', async () => {
        const piste = faussePiste();
        let debloquer!: (flux: MediaStream) => void;
        const enAttente = new Promise<MediaStream>((resolve) => {
            debloquer = resolve;
        });
        const demanderFlux = vi.fn(() => enAttente);
        const micro = attacherMicro({ sender: fauxSender(), demanderFlux });

        // Le second clic tombe pendant que la boîte de dialogue de permission
        // est encore ouverte : sans garde, il demanderait un SECOND flux, donc
        // une seconde piste — et la première fuirait, jamais arrêtée, micro
        // ouvert pour la vie de la page.
        const premier = micro.basculer();
        const second = micro.basculer();
        debloquer(fauxFlux(piste));

        await expect(premier).resolves.toBe('actif');
        await expect(second).resolves.toBe('ferme');
        expect(demanderFlux).toHaveBeenCalledTimes(1);
    });

    it('un détachement pendant la demande de flux arrête la piste obtenue', async () => {
        const piste = faussePiste();
        let debloquer!: (flux: MediaStream) => void;
        const enAttente = new Promise<MediaStream>((resolve) => {
            debloquer = resolve;
        });
        const sender = fauxSender();
        const micro = attacherMicro({ sender, demanderFlux: () => enAttente });

        // La session se termine alors que la boîte de dialogue est ouverte, et
        // l'utilisateur autorise APRÈS. Sans cette garde, la piste arrive dans
        // le vide : plus personne ne la détient, `detacher()` est déjà passé,
        // et le micro reste ouvert jusqu'à la fermeture de l'onglet.
        const bascule = micro.basculer();
        micro.detacher();
        debloquer(fauxFlux(piste));

        await expect(bascule).resolves.toBe('ferme');
        expect(piste.arretee).toBe(true);
        expect(sender.recus).toEqual([]);
    });

    it('après détachement, un clic ne demande plus rien', async () => {
        const demanderFlux = vi.fn(async () => fauxFlux(faussePiste()));
        const micro = attacherMicro({ sender: fauxSender(), demanderFlux });

        micro.detacher();
        await expect(micro.basculer()).resolves.toBe('ferme');
        expect(demanderFlux).not.toHaveBeenCalled();
    });

    it('détacher un micro ACTIF éteint réellement la piste', async () => {
        const piste = faussePiste();
        const sender = fauxSender();
        const micro = attacherMicro({ sender, demanderFlux: async () => fauxFlux(piste) });

        await micro.basculer();
        micro.detacher();

        // `webrtc.ts::close()` arrête déjà la piste du sender, mais une fin de
        // session doit AUSSI ramener l'état du bouton à « fermé » : c'est ce
        // que `main.ts` obtient en appelant ce détachement.
        expect(piste.arretee).toBe(true);
        expect(micro.etat()).toBe('ferme');
    });

    it('un refus n’est pas définitif : le clic suivant retente', async () => {
        const piste = faussePiste();
        let premierAppel = true;
        const demanderFlux = vi.fn(async () => {
            if (premierAppel) {
                premierAppel = false;
                throw erreurDom('NotAllowedError');
            }
            return fauxFlux(piste);
        });
        const micro = attacherMicro({ sender: fauxSender(), demanderFlux });

        // L'utilisateur a bloqué, puis rétabli la permission dans les réglages
        // du site — exactement ce que le message de l'état « refusé » lui
        // demande de faire. Un état terminal rendrait ce conseil inapplicable.
        await expect(micro.basculer()).resolves.toBe('refuse');
        await expect(micro.basculer()).resolves.toBe('actif');
        expect(demanderFlux).toHaveBeenCalledTimes(2);
    });
});
