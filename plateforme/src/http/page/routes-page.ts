// The server of the built page. It applies the verdict of `resolution.ts`,
// but DOES NOT STOP THERE.
//
// 🔴 « ALL THE SECURITY LIVES IN THE PURE RULE » WAS FALSE, AND IT WAS A
// REVIEW BY EXECUTION THAT ESTABLISHED IT (round 2, Critical 3), NOT A READING:
// `resolve()`, used further down, is LEXICAL — it follows NO symbolic
// link. A link placed in the built root (`/lien.json → ../dessus/
// secret.json`) goes through the guard of `resolution.ts` WITHOUT ANY `..`
// EVER SHOWING UP IN THE URL: the whole composition — pure rule PLUS
// this module — is the boundary, not the rule alone. The real guarantee against
// links is the `realpath` below, on the CANONICAL path.
//
// 🔴 THIS MODULE READS THE DISK, AND THAT IS WHAT MAKES IT DIFFERENT FROM
// `resolution.ts`: a hole judged « not exploitable » over there (because
// that rule touches nothing) may be exploitable HERE. Paid three times in
// this batch — the empty name, the symbolic link, and the undestroyed stream below.

import { createReadStream } from 'node:fs';
import { realpath, stat } from 'node:fs/promises';
import type { IncomingMessage, ServerResponse } from 'node:http';
import { resolve, sep } from 'node:path';
import type { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import {
    ENTETES_DOCUMENT,
    ENTETES_RESSOURCE_EMPREINTEE,
    ENTETES_RESSOURCE_REVALIDABLE,
} from './entetes-page';
import { resoudre } from './resolution';

export interface DependancesPage {
    /// Missing OR EMPTY ⇒ the server steps aside, and the generic 404 takes
    /// over. ⚠️ `''` MUST be handled the same as `undefined`: the
    /// type allows it, and `resolve('')` yields the CURRENT DIRECTORY of the
    /// process — a server that only tested `=== undefined`
    /// would publish the whole repository on an almost empty configuration.
    racinePage?: string;
}

export async function servirPage(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesPage,
): Promise<boolean> {
    return serveWithStream(req, rep, deps, (chemin) => createReadStream(chemin));
}

/// The same route, with the READ stream injected — the only extension point
/// compared with `servirPage`, which only ever calls `createReadStream`.
/// It exists ONLY to make the error path below testable:
/// a read that breaks AFTER the headers have gone.
///
/// 🔴 THIS COMMENT LIED THREE TIMES IN A ROW ON THIS SAME TOPIC, EACH
/// FIX PRODUCING A NEW ONE — the pattern `CLAUDE.md` names
/// « fixing a false claim can produce another one ». Hence the
/// shape below: EACH claim carries the command that establishes it.
/// Believe none of them; rerun them. All run from the
/// repository root, except the two `vitest` ones, from `plateforme/`.
///
/// ① THE DEFAULT CALLBACK IS NOT A COVERAGE HOLE. The only caller
/// of `serveWithStream` in the product is `servirPage` itself; all the
/// rest goes through `chaine.ts`, which only calls `servirPage`. In
/// `routes-page.test.ts`, 21 of the 23 tests stand up a REAL server
/// (`startServer` → `servirTout` → `servirPage`, hence a real
/// `createReadStream`); the other 2 are the unit tests of here.
///   grep -rn 'servirPage\|serveWithStream' plateforme/src
///   grep -c 'await startServer(' plateforme/src/http/page/routes-page.test.ts   → 21
///   grep -c '    it(' plateforme/src/http/page/routes-page.test.ts                  → 23
/// ⚠️ THESE TWO COUNTS SAID 18 AND 20, AND THEY WERE RIGHT AT THE TIME
/// THEY WERE WRITTEN: the fix wave of the final review added
/// THREE cache tests to the same file. **Rerun them, never
/// copy them** — that is the « shipwreck of 487 » of `CLAUDE.md`, and this block
/// exists precisely so the next reader redoes the check.
///
/// ② WHAT `signaling/resilience.test.ts` ESTABLISHES — the PATTERN, and that alone:
/// testing the death of a process IS possible in this repository. It launches the
/// real entry point `src/index.ts` as a CHILD process (`spawn(tsxBin,
/// …)`) and watches its SURVIVAL (`child.exitCode`, `child.killed`). Claiming
/// that such a precedent is missing would be false — that was lie no. 1.
///   grep -n 'spawn(tsxBin\|child.exitCode' plateforme/src/signaling/resilience.test.ts
///
/// ③ 🔴 WHAT IT DOES NOT ESTABLISH — that was lie no. 2, which credited it with
/// « a real `createReadStream` »: it exercises NO file read
/// served to the network, and touches neither this module nor any HTTP route. Its 3
/// tests open ONLY WebSockets, on `/signal` and `/agent`, against a
/// `null` frame and a frame beyond `maxPayload`. It is therefore a
/// precedent only for the SHAPE of the setup, never for its object.
///   grep -n createReadStream plateforme/src/signaling/resilience.test.ts   → NOTHING
///   grep -n 'new WebSocket(' plateforme/src/signaling/resilience.test.ts
///     (the only 3 connections of the file; no `http.get`/`http.request`)
///   grep -c WebSocket        plateforme/src/signaling/resilience.test.ts   → 8
///     (negative control of the same file: the NOTHING above is a measured
///      absence, not a grep that cannot find — `CLAUDE.md`, « a zero
///      can only be read with a negative control »)
///   grep -n 'describe(' plateforme/src/signaling/resilience.test.ts
///
/// ④ WHY THIS PATTERN WAS NOT REUSED HERE — a CHOICE, not an
/// impossibility (③ says why it would not carry over as is, but
/// nothing forbade writing its equivalent): its cost, and a suite
/// whose duration already regressed once in this batch (round 3, New 4).
/// Measured on 22 August 2026, from `plateforme/`, FOUR runs of each:
///   npx vitest run src/signaling/resilience.test.ts
///     → 3 tests, `tests` from 523 ms to 1.15 s, i.e. ~175 to ~385 ms per test
///   npx vitest run src/http/page/routes-page.test.ts
///     → 20 tests, `tests` from 516 to 872 ms, i.e. ~26 to ~44 ms per test
/// 🔴 DO NOT QUOTE A SINGLE RUN: these durations vary up to twofold
/// from one run to the next, and an isolated value will read as false to the
/// first person who reruns it — the first draft of this block did exactly
/// that. What carries the argument is the ORDER OF MAGNITUDE, not a figure:
/// pairing the EXTREMES, the process setup stays from 4× (175/44)
/// to 15× (385/26) more costly per test — never less than 4×. ⚠️ And vitest
/// switches to `1.15s` beyond one second: a tally that only
/// looks for `ms` LOSES the slowest run.
/// This factor is moreover the MOST favourable case for the process setup:
/// `resilience.test.ts` amortises its `spawn` over the whole file through a
/// single `beforeAll`, whereas the faulty stream here is rebuilt per test.
///   grep -c '^beforeAll(' plateforme/src/signaling/resilience.test.ts   → 1
///     (⚠️ `grep -c beforeAll` would yield 2: the `import` line counts too)
///
/// ⑤ 🔴 WHAT THIS CHOICE COSTS, AND WHAT NOBODY MUST READ AS COVERED:
/// the TWO tests that call `serveWithStream` (through a shared factory)
/// only observe the FUNCTION facing a stream error, on a MADE-UP
/// `req`/`rep` — no socket, no port, no server process. NO test of
/// `plateforme/src` has ever seen a MID-RESPONSE read error on a
/// real HTTP server, nor the survival of the service to a REAL `EMFILE`: the only
/// mid-stream error ever exercised is pushed by hand, and it says so.
///   grep -rn EMFILE plateforme/src | grep -vE ':[0-9]+: *//'
///     → ONE single line, in the test: a `this.emit('error', …)` on a
///       made-up stream. All the rest is just comment.
///     ⚠️ THE `grep -v` FILTER IS NOT AN ORNAMENT: without it, this grep
///       COUNTS ITSELF — the paragraph you are reading names `EMFILE` three
///       times. The first draft of this block announced « 3 comments
///       of this file » and was false the instant it was written.
///       Draw NO number from it without the filter.
export async function serveWithStream(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesPage,
    ouvrirFlux: (chemin: string) => Readable,
): Promise<boolean> {
    if (!deps.racinePage) return false;
    // 🔴 OUTSIDE GET/HEAD, WE STEP ASIDE — never a 405. See § 4.3 of the spec:
    // the SPA fallback resolves any path, so a 405 would mask the
    // typo of an API call instead of naming it.
    if (req.method !== 'GET' && req.method !== 'HEAD') return false;

    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    const verdict = resoudre(chemin);
    // A refusal returns `false`: the chain ends on the generic 404, rather
    // than inventing a SECOND form of 404 that nothing would test.
    if (!verdict.ok) return false;

    const racine = resolve(deps.racinePage);
    const candidat = resolve(racine, verdict.file);
    // LEXICAL belt. It only protects against a composition that would get out
    // through SEGMENTS (`..`) — `resolution.ts` already refused it upstream, so
    // this costs nothing more; it does NOT protect against a symbolic link,
    // which never lets any `..` show here. The guarantee against
    // links is the `realpath` block that follows.
    if (candidat !== racine && !candidat.startsWith(racine + sep)) return false;

    let racineReelle: string;
    let realFile: string;
    try {
        // 🔴 THE ROOT ITSELF MAY LEGITIMATELY BE A LINK — resolve it
        // too, never just the file: comparing a canonical path
        // to a path that is not would make the prefix comparison
        // arbitrary.
        //
        // ⚠️ `realpath` → `open` RACE (TOCTOU), judged and filed (round 3):
        // real, but it does not matter here — the attacker would need to be able
        // to WRITE into the built root, where they already have a
        // simpler attack, and `createReadStream` below opens the
        // CANONICAL path (`realFile`), not the link.
        racineReelle = await realpath(racine);
        realFile = await realpath(candidat);
    } catch {
        // A DEAD link or a missing file throws here — it is the new
        // « file not found », handled the same way: silent refusal,
        // never a 500.
        return false;
    }
    // 🔴 THE REAL GUARANTEE AGAINST SYMBOLIC LINKS: the comparison
    // applies to the TWO CANONICAL paths, after links are resolved by
    // `realpath`. That is what the lexical belt above could NOT
    // offer — measured (round 2, Critical 3): `/lien.json → ../dessus/
    // vole.json` and `/lien-rep/vole.html → ../dessus/vole.html` both returned
    // `200` with the STOLEN content before this block.
    if (realFile !== racineReelle && !realFile.startsWith(racineReelle + sep)) {
        return false;
    }

    let infos;
    try {
        infos = await stat(realFile);
    } catch {
        return false;
    }
    if (!infos.isFile()) return false;

    // 🔴 THREE HEADER SETS, AND THE CHOICE READS IN ONE LINE BECAUSE THE
    // RULE HAS ALREADY DECIDED. `verdict.empreinte` comes from `resolution.ts`, which
    // classifies by LOCATION (the Vite assets directory) and not by
    // extension: it is that classification that prevents a year of `immutable` on
    // `hub.webmanifest` or `favicon.ico`, whose name never changes.
    // Deciding here, on the disk, would have made the policy untestable without
    // a disk.
    const entetes = verdict.document
        ? ENTETES_DOCUMENT
        : verdict.empreinte
          ? ENTETES_RESSOURCE_EMPREINTEE
          : ENTETES_RESSOURCE_REVALIDABLE;
    rep.writeHead(200, { 'content-type': verdict.mime, ...entetes });
    if (req.method === 'HEAD') {
        rep.end();
        return true;
    }

    try {
        // 🔴 `pipeline()`, NEVER `.pipe()` — measured (round 2, Criticals 1 and
        // 2), and both get fixed BY THE SAME CHANGE:
        // `.pipe()` DOES NOT DESTROY THE SOURCE when the destination dies
        // (a client that gives up — reload, navigation, closed tab):
        // the descriptor stays open forever, monotonic, never given back
        // — `0 abandons → 1 fd`, `40 → 41`, `160 → 161`. `pipeline()` destroys
        // BOTH ends on a premature close OR an error, on
        // both sides.
        // `.pipe()` also attaches NO error handler on the
        // source: a read that breaks AFTER the headers have gone
        // (an `EMFILE`, reached once the leak above has piled up enough)
        // would then emit an error WITH NO LISTENER — and an `EventEmitter` that
        // emits `error` with no listener THROWS, out of any scope that a
        // `try/catch` of this file could catch: measured, the WHOLE
        // service dies (signaling included).
        await pipeline(ouvrirFlux(realFile), rep);
    } catch (cause) {
        // The headers have already GONE: the status can no longer change, so
        // there is nothing more accurate to send back than a refusal. The only
        // decision left is NOT to leave the response hanging on a
        // chunked body never finished (`.pipe()` only calls `end()` on
        // the `'end'` event, never on an error): we destroy the
        // connection rather than leave it open indefinitely.
        //
        // 🔴 A SILENT `catch` WAS NEW DEFECT 1 OF ROUND 3: without this
        // line, an `EMFILE` went from FATAL AND LOUD (before this batch) to
        // SILENT AND TRACELESS — the most discreet failure possible, which
        // `CLAUDE.md` fights first. The CALL SITE of `servirTout`
        // in `startServer` (`serveur.ts`) can see NOTHING: `return
        // true` at the end of the function tells it the route was served. The
        // requested path is logged, NOT the request body — this
        // repository never writes to a log what could carry a
        // secret.
        // ⚠️ THIS COMMENT USED TO QUOTE A LINE NUMBER
        // (`serveur.ts:308`) THAT `CLAUDE.md` FORBIDS — AND THE QUOTE WAS NOT
        // AT FAULT: it was ACCURATE when it was set down, and
        // STILL ACCURATE when a later survey requalified it
        // as « accurate today ». It became false AFTERWARDS, silently,
        // because a later task added lines higher up in
        // `serveur.ts` and shifted the target from 308 to 316:
        //   git show 912f1e2:plateforme/src/http/serveur.ts | grep -n "HTTP route failed"
        //     → 308:                console.error(`HTTP route failed: ${String(cause)}`);
        //   git show 9e06398:plateforme/src/http/serveur.ts | grep -n "HTTP route failed"
        //     → 308:                console.error(`HTTP route failed: ${String(cause)}`);
        //   grep -n "HTTP route failed" plateforme/src/http/serveur.ts   # at HEAD
        //     → 316:                console.error(`HTTP route failed: ${String(cause)}`);
        // That is exactly what `CLAUDE.md` says of this pattern: « a line
        // number is false as soon as someone writes above it — and someone always writes
        // above it ». Nobody was careless; that is what makes the rule
        // necessary against TIME, not against inattention. Fixed by
        // NAMING the thing rather than counting the lines between them
        // — cross-cutting review at the end of the batch, 22 August 2026 (fix round
        // 1: the first draft of this fix wrongly accused the two
        // commits above of having set and then approved a false quote,
        // for not having searched for this precise text on these precise commits).
        console.error(`page servie en echec de lecture, chemin=${chemin} : ${String(cause)}`);
        if (!rep.destroyed) rep.destroy();
    }
    return true;
}
