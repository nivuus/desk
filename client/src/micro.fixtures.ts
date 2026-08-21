// Les objets factices du chantier E, **partagés** par les deux fichiers de
// tests du micro.
//
// ⚠️ **EXTRACTION, pas un remaniement.** `client/src/micro.test.ts` était à
// **411** lignes ; la famille de tests du bloc E3 l'a porté à **515**, donc
// AU-DELÀ de la porte des 500. La doctrine du dépôt est d'extraire, jamais de
// comprimer — ce dépôt a franchi ce plafond cinq fois et l'a rattrapé deux
// fois par une compression qu'il s'interdit.
//
// 🔴 **Le franchissement a eu lieu AVANT d'être vu.** Le plan budgétait « ~50 »
// lignes pour ce fichier ; la famille en a coûté **104**, et sa table de portes
// portait « ⚠️ à remesurer avant d'écrire ». La leçon n'est pas « mieux
// estimer » : c'est **remesurer après avoir écrit**, ce qu'aucune estimation ne
// remplace.
//
// ⚠️ **Le contenu est VERBATIM.** Seule la visibilité a changé — les cinq
// fonctions sont devenues `export` —, ce qui est la seule chose qu'une
// extraction ait le droit de changer.

/// Piste factice : ce que ce module fait d'une `MediaStreamTrack` se réduit à
/// `stop()`, et c'est précisément l'appel que la spec §9 rend obligatoire.
/// `arretee` est le seul instrument capable de distinguer une extinction RÉELLE
/// d'un `enabled = false` — le « mensonge visuel » que la spec écarte.
export function faussePiste(): MediaStreamTrack & { arretee: boolean; enabled: boolean } {
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
export function fauxFlux(piste: MediaStreamTrack): MediaStream {
    return { getAudioTracks: () => [piste] } as unknown as MediaStream;
}

/// Sender factice : mémorise TOUT ce qui lui est passé, dans l'ordre. Un
/// booléen « a reçu une piste » ne distinguerait pas une extinction d'une
/// absence d'allumage.
export function fauxSender() {
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
export function erreurDom(name: string): Error {
    const e = new Error(name);
    e.name = name;
    return e;
}

/// Bouton factice : ce que le module écrit dessus, et rien d'autre. Un vrai
/// `HTMLButtonElement` exigerait un DOM, que `client/src` évite partout par
/// injection de dépendances (`audio.ts`, `fullscreen.ts`, `visibilite.ts`).
export function fauxBouton() {
    const ecouteurs = new Map<string, Set<EventListener>>();
    return {
        hidden: true,
        disabled: false,
        title: '',
        dataset: {} as { etat?: string },
        addEventListener(type: string, e: EventListener) {
            if (!ecouteurs.has(type)) ecouteurs.set(type, new Set());
            ecouteurs.get(type)!.add(e);
        },
        removeEventListener(type: string, e: EventListener) {
            ecouteurs.get(type)?.delete(e);
        },
        cliquer() {
            for (const e of [...(ecouteurs.get('click') ?? [])]) e(new Event('click'));
        },
        nombreEcouteurs: () => (ecouteurs.get('click')?.size ?? 0),
    };
}
