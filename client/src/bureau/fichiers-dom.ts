// THE "MY FILES" READER — the ProjFS bridge's wiring, with injected
// dependencies.
//
// 🔴 EXTRACTED FROM `shell-page.ts` ON AUGUST 31st, 2026, VERBATIM. The six blocks
// (four 🔴, two ⚠️) it carries each document a MEASURED defect:
// 🔴 the transient user activation `showDirectoryPicker()` requires,
// 🔴 the cast that is NOT a check, 🔴 the order of the `Bonjour` (F5),
// 🔴 waiting for the channel to open (the `Bonjour` went into the void, and
// NO due write would ever have been pushed), ⚠️ the `pont !== ce` guard
// (a listener outliving its object), and ⚠️ closing the streams on
// `close`. None is decorative: they were moved to the letter, never
// summarised.
//
// ⚠️ THIS FILE IS NOT TESTED, AND CANNOT BE HERE: it opens an
// `RTCPeerConnection`, a `WebSocket` and a directory picker, none of
// which exists under Node — and `client/` has neither jsdom nor happy-dom, by
// convention (`accent-dom.test.ts`). Everything DECIDABLE it carries is
// tested elsewhere: `shell.ts`, `fichiers/protocole.ts` and
// `fichiers/adaptateur.ts` are pure and covered.

import { createAdapter } from '../fichiers/adaptateur';
import {
    choisirDossier,
    connectFilesChannel,
    sessionDuPont,
    type FilesChannel,
} from '../fichiers/canal';
import { createWriter, type RacineInscriptible } from '../fichiers/ecriture';
import type { RacineMutable } from '../fichiers/mutation';
import { createMutator } from '../fichiers/mutation-service';
import { createServer, trameBonjour, trameRafraichir } from '../fichiers/protocole';
import type { Bureau } from '../shell';

export interface FilesDeps {
    bureau: Bureau;
    signalingUrl: string;
    /// 🔴 **A FRESH TOKEN PROVIDER, AND THERE WAS NONE** (critical
    /// ① of the final review of August 31st, 2026). `fichiers/canal.ts` read
    /// `jetonAcces()` — the vault's **RAW** content, without going through
    /// `assurerAccesFrais` —, yet "Choose my folder" is a gesture that can
    /// happen any time after the page loads, and an access
    /// token lives **ten minutes** (`plateforme/src/identite/jeton.ts`).
    /// The bridge was therefore refused its session without anything linking
    /// the failure to expiry.
    ///
    /// ⚠️ **CALLED AFTER THE DIRECTORY PICKER, NEVER BEFORE**: an
    /// `await` placed before `showDirectoryPicker()` would consume the transient
    /// user activation, and the picker would be refused without anything
    /// saying so. See `monterLeLecteur`, where the order is applied.
    jetonFrais(): Promise<string | undefined>;
    /// `?faute-fichiers=1` — BENCH variable, never a shipped
    /// configuration. Read ONCE by the page and passed here, never reread: it is the
    /// convention of `PLEIN_ECRAN` and `PART_SONDAGE` on the agent side — the
    /// mechanism reads a flag it is given. And a user who
    /// created a folder with the reserved name would not break their own bridge.
    fautesArmees: boolean;
    boutonDossier: HTMLButtonElement;
    boutonRafraichir: HTMLButtonElement;
    boutonReprendre: HTMLButtonElement;
    /// The `<details>` to unfold on click.
    ///
    /// ⚠️ **MANDATORY SINCE THE FINAL REVIEW (Minor ①).** Its doc said
    /// "`undefined` when the page has no fold — that is the case of
    /// `shell.html`": **wrong since task 9**, where `shell.html`
    /// became a redirect without any UI. The only caller
    /// (`bureau/porteur-dom.ts`) ALWAYS passed a section, so that
    /// the optional was nothing more than dead code justified by a
    /// false sentence.
    section: HTMLDetailsElement;
}

export function installerLePont(deps: FilesDeps): void {
    let pont: FilesChannel | null = null;

    deps.boutonDossier.addEventListener('click', () => {
        // ⚠️ UNFOLDING HAPPENS ON CLICK, AND BEFORE ANY `await`. Unfolding it on
        // SUCCESSFUL mount would give the impression, on a picker
        // cancellation, that the click did nothing.
        deps.section.open = true;
        // 🔴 `showDirectoryPicker()` REQUIRES A TRANSIENT USER ACTIVATION,
        // and that is why it is called from this handler, and
        // never from a channel message. The handler is not `async`: an
        // `await` before the call would consume the activation, and the picker would be
        // refused without anything saying so. Same constraint as `window.open()`, which
        // this page already knows.
        void monterLeLecteur();
    });

    async function monterLeLecteur(): Promise<void> {
        // A second click replaces the folder: the old bridge goes first, otherwise
        // two `PeerConnection`s would fight over the `…:files` session and
        // the second would be refused by the relay.
        pont?.close();
        pont = null;
        deps.bureau.lecteurDemonte();

        const choix = await choisirDossier().catch((e: unknown) => {
            deps.bureau.lecteurEchoue((e as Error).message);
            return undefined;
        });
        // `null` = deliberate cancellation, `undefined` = failure already reported.
        if (choix === null || choix === undefined) return;

        // 🔴 **THE TOKEN IS REQUESTED AGAIN HERE, AND ONLY HERE** — after the
        // picker (which requires user activation, hence no `await`
        // before it) and before the signaling handshake. It is the
        // "bridge" half of critique ① of the final review: `canal.ts`
        // read the RAW vault, whose token may have expired since
        // the page loaded.
        const jeton = await deps.jetonFrais();
        if (jeton === undefined) {
            deps.bureau.lecteurEchoue(
                'Votre session a expiré. Rechargez la page pour vous reconnecter.',
            );
            return;
        }

        // 🔴 THE SAME HANDLE IS USED TO READ, TO WRITE AND TO MUTATE.
        //
        // ❌ **THIS CAST IS NOT A CHECK, AND F2 DECLARED IT AS ONE.**
        // These lines said: "the cast is F2's STRUCTURAL COMPATIBILITY
        // CHECK: if the real handle stopped satisfying it,
        // `tsc --noEmit` would say so HERE". **It is wrong, and it is measured**:
        // `choix.racine` is typed `Racine`, and `RacineInscriptible` is a
        // SUBtype of it — an `as` to a subtype ASSERTS, it does not check.
        // Adding to `RacineInscriptible` a method
        // `FileSystemDirectoryHandle` does not have only turned the test
        // fake red.
        //
        // ✅ **THE REAL CHECK NOW LIVES IN `fichiers/canal.ts`**, on the
        // REAL handle, before any widening — and it found an
        // incompatibility of F2 as soon as it was set (see
        // `journaux-pont-fichiers-f3/t9-controle-structurel-de-f2-vacueux.txt`).
        // These two lines are now just wiring.
        const racineInscriptible: RacineInscriptible = choix.racine as RacineInscriptible;
        const racineMutable: RacineMutable = choix.racine as RacineMutable;
        const ecrivain = createWriter(racineInscriptible);
        const mutateur = createMutator(racineMutable);
        const serveur = createServer(
            createAdapter(choix.racine, deps.fautesArmees),
            (m) => console.warn(m),
            {
                ecrivain,
                mutateur,
                onDues: (dues, retenues) => deps.bureau.ecrituresDues(dues, retenues),
                onEchecEcriture: (chemin, code) => deps.bureau.ecritureEchouee(chemin, code),
                onEchecMutation: (quoi, code) => deps.bureau.mutationEchouee(quoi, code),
                // 🔵 **THE INSTRUMENTATION SPEC §3.5.1 REQUIRES**, and it goes out
                // through the log because the browser is the only one to KNOW what
                // it did. The bridge, for its part, logs what IT knows — see the
                // divergence declared in `protocole.ts`.
                onRenommagePorCopie: (de, vers, octets, entrees) => {
                    console.warn(
                        `renommage par copie « ${de} » → « ${vers} » : ${octets} octets, ` +
                            `${entrees} entree(s) — move() absente, repli LOCAL (zero octet sur le canal)`,
                    );
                },
            },
        );
        try {
            pont = await connectFilesChannel({
                signalingUrl: deps.signalingUrl,
                sessionId: sessionDuPont(),
                jeton,
                onStatus: (m) => console.info(m),
                traiter: (octets) => serveur.traiter(octets),
            });
        } catch (e) {
            deps.bureau.lecteurEchoue((e as Error).message);
            return;
        }
        deps.bureau.lecteurMonte(choix.nom);

        // ════════════════════════════════════════════════════════════════════
        // 🔴 **F5 — `Bonjour` GOES OUT HERE, AND THE ORDER IS NOT INDIFFERENT.**
        //
        // It is sent **AFTER** the writer, the mutator and the adapter are
        // set and the channel is open — never before. It is it, and it alone,
        // that triggers the resumption of due writes on the bridge side: before F5, that
        // ran at the THREAD's startup, that is, *without knowing whether a browser
        // is there, nor which, nor on which directory*. F2 measured, twice out of
        // two, the replay push **0.8 s BEFORE** this mount announcement, then
        // an expiry **+30.2 s** later.
        //
        // ⚠️ **`choix.nom` is the directory handle's `name`**, and it is the
        // SAME value a directory chosen through `showDirectoryPicker()` or through
        // OPFS would return: that is what lets the acceptance run exercise the rule without
        // the picker. **It does not exercise the permission model for all that**,
        // which is called nowhere in this repository.
        //
        // ⚠️ **`forcer: false` at mount, ALWAYS.** Forcing is a user
        // gesture, never a default: a `true` here would make the
        // "Resume" button unreachable and would reintroduce the danger of §6.4 case 2.
        // ════════════════════════════════════════════════════════════════════
        // 🔴 **THE ANNOUNCEMENT WAITS FOR THE CHANNEL TO OPEN, AND IT IS A DEFECT ONLY
        // THE REAL PATH COULD SHOW.**
        //
        // *The first draft sent right here, without waiting.*
        // `connectFilesChannel` returns as soon as the SDP answer is received; the data
        // channel, for its part, opens **afterwards**. Measured on the VM, in this order:
        // "file bridge: answer received" → **`file channel closed: announcement not
        // sent`** → "connecting" → "connected" → "file channel open".
        // The `Bonjour` went into the void, **and therefore NO due write would
        // ever have been pushed** — a silence, that is, worse than the thirty
        // seconds F2 had measured and that F5 exists to remove.
        //
        // 🔵 **It is my own `console.warn` that exposed it.** A send that
        // had failed silently would have left the acceptance run green on its cache
        // criteria and silent on this one.
        //
        // ⚠️ **BOTH BRANCHES ARE NECESSARY**: the channel may already be
        // open when we get here (nothing forbids it), and listening only to
        // `'open'` would then miss the event forever.
        const envoyerAuPont = (trame: ArrayBuffer): void => {
            if (!pont) {
                console.warn('aucun pont : annonce non envoyee');
                return;
            }
            const canal = pont.canal;
            if (canal.readyState === 'open') canal.send(trame);
            else if (canal.readyState === 'connecting') {
                canal.addEventListener('open', () => canal.send(trame), { once: true });
            } else console.warn('canal fichiers ferme : annonce non envoyee');
        };
        envoyerAuPont(trameBonjour(choix.nom, false));
        deps.boutonRafraichir.onclick = () => envoyerAuPont(trameRafraichir());
        deps.boutonReprendre.onclick = () => {
            // **Resuming is a FORCED `Bonjour`**, and not one more verb: it is
            // exactly "I confirm this directory is the right one". The bridge
            // then stores the announced name, and the button disappears at the next
            // announcement — without any local state having to be reset here.
            envoyerAuPont(trameBonjour(choix.nom, true));
        };

        // ⚠️ UNMOUNTING FOLLOWS THE CONNECTION, NOT THE CHANNEL ALONE: a channel closed on
        // a connection that recovers would be reopened by the agent, whereas a
        // `failed` or `closed` connection is final for this bridge.
        //
        // ⚠️ THE CURRENT BRIDGE IS CAPTURED, and the listener goes silent if it is no longer
        // that one. Without this guard, closing the OLD bridge — which the
        // remount has just caused — would erase the NEW one's state: a
        // listener outliving its object is the exact pattern of a race one
        // only sees by clicking twice.
        const ce = pont;
        ce.pc.addEventListener('connectionstatechange', () => {
            if (pont !== ce) return;
            const etat = ce.pc.connectionState;
            if (etat === 'failed' || etat === 'closed') deps.bureau.lecteurDemonte();
        });
        // ⚠️ **OPEN STREAMS CLOSE WITH THE CHANNEL.** A
        // `createWritable()` stream left open keeps its swap file, and its
        // destination file stays UNCHANGED — the commit happens at `close()`.
        ce.canal.addEventListener('close', () => ecrivain.abandonner());
    }
}
