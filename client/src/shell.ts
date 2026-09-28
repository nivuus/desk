// THE DESKTOP: the rule of the surface that opens one browser window per
// Windows window, and it alone — no application page has that power.
//
// ⚠️ THIS SURFACE WAS CALLED "THE SHELL PAGE" UNTIL AUGUST 31ST, 2026, and
// this file opened with that name. **It has been the HUB since** (`hub.html`, served
// at the root, wired by `bureau/porteur-dom.ts`): `shell.html` is now only
// a redirect. The word "shell page" remains further down in this
// file and in its neighbours as a ROLE NAME — the surface holding the
// control session —, never as a page name.
//
// Why a dedicated page rather than the first application page: without
// it, closing that first page would cut the ability to open all the
// following ones. Here, no application window is special.
//
// All the logic is here, separated from the DOM and the WebSocket, to be
// testable: `createDesktop` receives its effects by injection.

/**
 * The TONE of a banner — one of the four of the primitives' `message` family
 * (sub-block S2). It is a RULE, and it lives here rather than in the wiring:
 * `bureau/porteur-dom.ts` (`shell-page.ts` before the hub became the
 * only surface, August 31st, 2026) only sets the corresponding class, and
 * a condition appearing there would be in the wrong place.
 *
 * ⚠️ `alerte` HAS NO CALLER IN THIS FILE, and it is deliberate: the shell
 * page has today no state that is a warning without being a
 * refusal. The tone exists in the primitives' family, and the type names it so
 * that the day such a state appears, it is not voiced as `danger` for lack
 * of having the word at hand.
 */
export type Ton = 'neutre' | 'succes' | 'alerte' | 'danger';

export interface FenetreConnue {
    session: string;
    titre: string;
    ouverte: boolean;
}

/** An OWED write: bytes that live on the VM and not here yet. */
export interface EcritureDue {
    chemin: string;
    octets: number;
}

export interface OptionsBureau {
    /// Returns `null` if the browser blocked the opening.
    ouvrirFenetre(session: string, titre: string): Window | null;
    envoyer(message: unknown): void;
    show(message: string, ton: Ton): void;
    /// The state of the file drive, separate from the general banner: the two
    /// messages do not chase each other away.
    showFilesState(texte: string, ton: Ton): void;
    /// The counter of owed writes.
    ///
    /// 🔴 **`dues` AND `vues` ARE TWO NUMBERS, AND THE SECOND IS CUMULATIVE.**
    /// `dues` goes down, `vues` never. A `dues = 0` **alone** says NOTHING:
    /// it is also what a machine where nothing has happened yet returns. *A
    /// negative verdict requires the measured thing to be ABSENT, not merely
    /// zero* — the clipboard's P0 probe returned a false eliminating
    /// verdict for having read three zeros on a healthy VM.
    showPendingWrites(dues: number, vues: number, texte: string, ton: Ton): void;
    /**
     * **F5** — the bridge HOLDS BACK its owed writes: the announced directory is
     * not the one that was registered (spec §6.4 case 2).
     *
     * 🔴 **The "Resume saving" button ONLY appears if it is
     * true**, and disappears otherwise. *An always-present button that does nothing
     * most of the time is a click trap*: the user who has seen it inert
     * ten times will no longer see it the day it counts.
     */
    showRetained(retenues: boolean): void;
}

export interface Bureau {
    fenetreOuverte(session: string, titre: string): void;
    fenetreFermee(session: string): void;
    refus(titre: string, motif: string): void;
    viewportRecu(session: string, largeur: number, hauteur: number): void;
    list(): FenetreConnue[];
    rouvrir(session: string): void;
    /// The `Mes Fichiers` drive is mounted on the folder `nom`. (policy: allow-fr, real Windows drive name)
    lecteurMonte(nom: string): void;
    /// The drive is no longer mounted: the state is ERASED, not left in place.
    lecteurDemonte(): void;
    /// Mounting failed. DISTINCT from `lecteurDemonte`: "nothing is
    /// shared" and "sharing failed, here is why" do not call for the
    /// same user gesture.
    lecteurEchoue(motif: string): void;
    /// The bridge announces what has NOT yet arrived on the local machine.
    ecrituresDues(dues: EcritureDue[], retenues: boolean): void;
    /// A write failed. It stays owed, and it is NAMED.
    ecritureEchouee(chemin: string, motif: string): void;
    /// **F3** — a MUTATION failed: renaming or deletion.
    ///
    /// 🔴 **DISTINCT from `ecritureEchouee`, and it is not a subtlety.** A
    /// failed write stays OWED: the bridge will push it again, and the counter
    /// will go down. A failed mutation, for its part, **will never be replayed** —
    /// ProjFS sends no notification for a gesture already accomplished in
    /// the VM. The two sides have therefore DIVERGED, for good, and the only
    /// remedy is human.
    ///
    /// ⚠️ **They do not accumulate the same way either**: failed mutations
    /// pile up until the drive is remounted, whereas failed writes
    /// disappear as soon as their path stops being owed.
    mutationEchouee(quoi: string, motif: string): void;
    /// Should the user be warned before closing the tab?
    ///
    /// ⚠️ **PURE PREDICATE, tested here**; the `beforeunload` wiring lives in
    /// `bureau/porteur-dom.ts` (`shell-page.ts` before the hub became
    /// the only surface, August 31st, 2026), which is not tested. Warning
    /// ALWAYS would teach the user to ignore the warning, which would
    /// make it useless exactly the day it counts.
    doitPrevenir(): boolean;
    /// 🔴 **NEW — FIX OF THE MISSING-BRAKES LEGACY (correction round 1,
    /// critical ④), August 25th, 2026.** The control session's socket
    /// (`bureau/porteur-dom.ts`, `shell-page.ts` at the time) can now receive a
    /// `{type:'error'}` message that NO branch of its switch recognised —
    /// notably the `trop-de-requetes` volume refusal this same batch has just
    /// opened on `/signal` (`signaling/relais.ts`). Without this method, the
    /// page kept displaying "desktop connected" and died silently: the
    /// exact silent failure this repository fights, opened by this very batch.
    ///
    /// `motif` prevails over `reason` when both are meaningless to
    /// the user — SEE THE IMPLEMENTATION, which documents the arbitration.
    canalDeControleRefuse(reason: string | undefined, motif: string | undefined, retryApresS: number | undefined): void;
    /// The control session's socket closed while it was
    /// open — network loss, service restart, or the end of a refusal. The
    /// same silence defect as above, on the `close` event rather
    /// than on an `error` message: `shell-page.ts` installed NO
    /// `close` or `error` listener on this socket before this fix.
    canalDeControlePerdu(): void;
}

interface Entree {
    titre: string;
    fenetre: Window | null;
}

export function createDesktop(options: OptionsBureau): Bureau {
    const connues = new Map<string, Entree>();
    /** The writes owed right now. Goes back down to zero. */
    let dues: EcritureDue[] = [];
    /**
     * The CUMULATIVE number of owed writes ever seen. **Monotonic, never reset.**
     *
     * 🔴 It is what distinguishes "nothing is owed" from "nothing happened". Without
     * it, `data-dues="0"` on a healthy machine would be indistinguishable from a
     * MEASUREMENT NOT TAKEN, and a negative verdict would read as a success.
     */
    let vues = 0;
    /** Failures, per path. They outlive the counter: the entry stays owed. */
    const echecs = new Map<string, string>();
    /**
     * The failed MUTATIONS, in their order of arrival.
     *
     * 🔴 **THEY NEVER DISAPPEAR ON THEIR OWN**, unlike
     * failed writes: nothing will replay them. They are erased when the
     * drive is remounted, and only then — that is, by a gesture of
     * the user, which is the only remedy.
     */
    let mutations: string[] = [];
    /**
     * **F5** — the bridge holds back its owed writes for lack of recognising the directory.
     *
     * ⚠️ **It is a state of the BRIDGE, not of the interface**: it is not reset
     * by a local gesture, but by the next announcement.
     */
    let retenu = false;

    function redessinerLesDues(): void {
        const texte = [phraseDesDues(dues, echecs), phraseDesMutations(mutations)]
            .filter((p) => p.length > 0)
            .join(' ');
        // DANGER as soon as a failure is named — the user must ACT. Otherwise
        // ALERT as long as owed writes remain: it is not a refusal, it is a
        // wait, but a wait one must not close by accident.
        //
        // ⚠️ **A failed MUTATION is a DANGER even without any owed write**, and
        // that is what distinguishes it: the two sides have diverged, and nothing will
        // reconcile them on its own.
        const ton: Ton =
            echecs.size > 0 || mutations.length > 0
                ? 'danger'
                : dues.length > 0
                  ? 'alerte'
                  : 'neutre';
        // ⚠️ **HOLDING BACK IS AN ALERT, never a `neutre`**: nothing will go out again
        // without a gesture, and a neutral tone would suggest the bridge
        // is still working.
        const tonFinal: Ton = retenu ? 'alerte' : ton;
        options.showPendingWrites(dues.length, vues, texte, tonFinal);
    }

    function ouvrir(session: string, titre: string): void {
        const fenetre = options.ouvrirFenetre(session, titre);
        if (!fenetre) {
            // DANGER: the user must ACT — allow pop-ups. A
            // neutral tone would suggest the window is on its way.
            options.show(
                `« ${titre} » could not open: the browser blocked the pop-up. ` +
                `Allow pop-ups for this site, then reopen the window.`,
                'danger',
            );
        }
        connues.set(session, { titre, fenetre });
    }

    return {
        fenetreOuverte(session, titre) {
            ouvrir(session, titre);
        },

        fenetreFermee(session) {
            const entree = connues.get(session);
            if (!entree) return;
            // The Windows window disappeared: its page has nothing left to show.
            entree.fenetre?.close();
            connues.delete(session);
        },

        refus(titre, motif) {
            // DANGER: the window will not exist.
            options.show(`« ${titre} » could not open: ${motif}.`, 'danger');
        },

        viewportRecu(session, largeur, hauteur) {
            // The message comes from `postMessage`: any page of the
            // same origin can emit one. We only relay what we
            // opened ourselves.
            if (!connues.has(session)) return;
            options.envoyer({ type: 'viewport', session, largeur, hauteur });
        },

        list() {
            return [...connues.entries()].map(([session, e]) => ({
                session,
                titre: e.titre,
                // `closed` is the only source of truth: the user may
                // have closed the page without anyone warning us.
                ouverte: e.fenetre !== null && !e.fenetre.closed,
            }));
        },

        rouvrir(session) {
            const entree = connues.get(session);
            if (!entree) return;
            ouvrir(session, entree.titre);
        },

        lecteurMonte(nom) {
            // SUCCESS — and it is the only positive state of the product.
            options.showFilesState(`« Mes Fichiers » drive mounted on « ${nom} ».`, 'succes');
        },

        lecteurDemonte() {
            // Remounting is the ONLY remedy for a failed mutation: nothing
            // will replay it. Erase them here, and only here.
            mutations = [];
            redessinerLesDues();
            // 🔴 THE EMPTY STRING, AND NOT AN "unmounted" MESSAGE. It is the defect
            // found in D5: the `#status` banner kept its `textContent`
            // after `expirer()`, so that reading the text proved a
            // message had ARRIVED, never that it was DISPLAYED — a whole acceptance run
            // read a stale banner believing it read the current state.
            // A drive state that does not clear would suggest a folder
            // still shared when it no longer is, which is worse than a
            // stale text: it is a false statement about a permission.
            //
            // 🔴 AND THE TONE STAYS `neutre`: an EMPTY banner must not carry
            // a colour. A coloured badge without text would be an alarm
            // without a statement — the worst of both worlds, and the exact mirror of the
            // defect above. Empty text and neutral tone are TWO
            // properties, and `shell.test.ts` tests them separately.
            options.showFilesState('', 'neutre');
        },

        ecrituresDues(neuves, retenues) {
            // ⚠️ **THE ANNOUNCEMENT OVERWRITES, it does not add up.** The bridge sends
            // the complete STATE of its journal at each change: accumulating would make
            // an acknowledged path stay displayed forever.
            dues = neuves;
            vues += neuves.length;
            // A path that is no longer owed has no failure left to show: it has
            // arrived.
            for (const chemin of [...echecs.keys()]) {
                if (!neuves.some((d) => d.chemin === chemin)) echecs.delete(chemin);
            }
            // ⚠️ **HELD BACK WITHOUT ANY OWED WRITE MAKES NO SENSE**, and displaying it
            // would offer to resume what there is nothing to resume. The bridge
            // does not emit it, but relying on it would make the interface depend
            // on a property no type guarantees.
            retenu = retenues && neuves.length > 0;
            options.showRetained(retenu);
            redessinerLesDues();
        },

        ecritureEchouee(chemin, motif) {
            // 🔴 **THE FILE IS NAMED, AND SO IS THE CAUSE.** "A write
            // failed" does not tell the user which document to reopen.
            echecs.set(chemin, motif);
            redessinerLesDues();
        },

        mutationEchouee(quoi, motif) {
            // 🔴 **`quoi` CARRIES BOTH PATHS OF A RENAME** (`de → vers`),
            // because "cannot rename X" does not say to what — and
            // that is precisely what the user must check: the
            // destination may already exist.
            mutations.push(`« ${quoi} » (${motif})`);
            redessinerLesDues();
        },

        doitPrevenir() {
            return dues.length > 0;
        },

        lecteurEchoue(motif) {
            // DANGER: sharing failed, and "nothing is shared" does not call for
            // the same gesture as "sharing failed, here is why".
            options.showFilesState(
                `The « Mes Fichiers » drive could not be mounted: ${motif}.`,
                'danger',
            );
        },

        canalDeControleRefuse(reason, motif, retryApresS) {
            // 🔴 `reason` FIRST: it is the sentence meant for a human
            // (`identite/garde.ts::Verdict.message`, or the fixed text of
            // `relais.ts` for `trop-de-requetes`); `motif` is a stable
            // KEYWORD for code, not a sentence — see `bureau.refus`
            // above, which follows the same hierarchy for the same reason.
            const cause = reason ?? motif ?? 'unknown reason';
            // ⚠️ `retryApresS` ONLY accompanies the volume refusal
            // (`signaling/relais.ts`): a handshake refusal (token
            // absent or invalid) has nothing to retry, reconnecting will
            // change nothing. Absent, the sentence therefore promises nothing that would
            // not hold.
            const attente =
                typeof retryApresS === 'number' ? ` New attempt possible in ${retryApresS} s.` : '';
            options.show(`Desktop refused: ${cause}.${attente}`, 'danger');
        },

        canalDeControlePerdu() {
            // DANGER, never NEUTRAL: no window can open or
            // close any more until the page is reloaded, and saying it
            // neutrally would suggest a desktop that still works.
            options.show(
                'Connection to the desktop lost. Reload the page to sign in again.',
                'danger',
            );
        },
    };
}

/**
 * The counter's sentence. **It NAMES the files**, because the
 * `beforeunload` dialog cannot.
 *
 * ⛔ **The custom message of `beforeunload` is IGNORED by all
 * modern browsers**: they only display a generic label of their
 * choice. Spec §6.2 asks for "a `beforeunload` with a text that names the
 * files" — **that text does not exist**. Naming them IN THE PAGE, next to the
 * counter, is what remains. *(A platform fact, not measured here, declared
 * as such.)*
 */
function phraseDesMutations(mutations: string[]): string {
    if (mutations.length === 0) return '';
    const pluriel = mutations.length > 1 ? 's' : '';
    return (
        `${mutations.length} rename${pluriel} or removal${pluriel} ${
            mutations.length > 1 ? 'have' : 'has'
        } NOT been applied on this host: ${mutations.join(', ')}. ` +
        `The two sides have diverged, and nothing will replay it.`
    );
}

function phraseDesDues(dues: EcritureDue[], echecs: Map<string, string>): string {
    if (dues.length === 0) return '';
    const noms = dues
        .map((d) => {
            const motif = echecs.get(d.chemin);
            return motif === undefined ? `« ${d.chemin} »` : `« ${d.chemin} » (${motif})`;
        })
        .join(', ');
    const pluriel = dues.length > 1 ? 's' : '';
    return (
        `${dues.length} file${pluriel} saved in the VM ${dues.length > 1 ? 'have' : 'has'} ` +
        `not been copied to this host yet: ${noms}. Do not close this tab.`
    );
}
