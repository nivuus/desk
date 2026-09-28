// What the service SAYS about its configuration at startup, before listening.
//
// 🔴 WHY THIS MODULE EXISTS, AND THE RULE IT APPLIES WAS ALREADY WRITTEN
// IN `serveur.ts`. The two optional disk stores log there
// the path they retained, and the reason is named there: "the variable being
// optional, it is the only thing that makes visible to the operator the store
// they are really working on". The "page behind Pomerium" batch
// added a THIRD optional disk root — `PLATEFORME_PAGE` — without
// applying this rule to it: set wrongly, it returned `404 introuvable` on
// EVERY page, which is STRICTLY INDISTINGUISHABLE from "variable absent", and
// nothing said so, neither at startup nor on the request.
//
// 🔴 AND THE TRUST SET IS OF THE SAME CLASS, NOT ANOTHER ONE: a
// host name written in `PLATEFORME_PROXY_DE_CONFIANCE` matches
// NO `remoteAddress` — which is always an IP address —, so that
// `pairDeConfiance` refuses EVERYBODY, `/auth/moi` returns `401
// pair-non-de-confiance` to Pomerium itself, and the service answers
// anyway. Same silent failure, same remedy: say what was RETAINED.
//
// ⚠️ WE LOG WHAT THE SERVICE RETAINED, NEVER WHAT IT WAS GIVEN.
// A trace that copied the raw value of the environment would only prove
// that the environment was read; what the operator must be able to
// recognise is the RESOLVED ABSOLUTE path and the entries SURVIVING the
// splitting — that is, what the product really works on.
//
// ⚠️ THIS MODULE RETURNS ITS LINES, IT DOES NOT WRITE THEM — same convention as
// `obs/journal.ts`, and for the same reason: it is what makes them testable
// without `spyOn(console)`. Writing is a separate gesture (`write`), called by
// `serveur.ts`.

import { constants } from 'node:fs';
import { access, stat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { ligne } from '../obs/journal';

/// A line ready to write, and the LEVEL that carries it.
///
/// 🔴 THE LEVEL IS IN THE DATA, NOT IN THE CALLER: a root that is set
/// but unreadable must be LOUD (`console.error`), and leaving that choice to the
/// call site would make it invisible to the test that checks it.
export interface Annonce {
    readonly niveau: 'info' | 'error';
    readonly texte: string;
}

/// What the service retained from `PLATEFORME_PAGE`, once the disk has been probed.
export type EtatRacinePage =
    | { readonly arme: false }
    | { readonly arme: true; readonly chemin: string; readonly lisible: true }
    | {
          readonly arme: true;
          readonly chemin: string;
          readonly lisible: false;
          readonly cause: string;
      };

/// PURE. The line that `PLATEFORME_PAGE` deserves, in the three cases.
///
/// 🔴 "NO PAGE SERVED" IS OPERATIONAL INFORMATION, NOT A
/// SILENCE. It is the nginx setup (`motdepasse` mode), where the platform must
/// serve nothing: saying so is what tells "I serve nothing because
/// nobody asked me to" apart from "I serve nothing because my root is
/// wrong". Without this line, both read as an identical `404`.
export function annonceRacinePage(etat: EtatRacinePage): Annonce {
    if (!etat.arme) {
        return {
            niveau: 'info',
            texte: ligne('page served', {
                racine: 'none',
                raison: 'PLATEFORME_PAGE absent or empty',
                effet: 'GET / returns 404 not found',
            }),
        };
    }
    if (etat.lisible) {
        return {
            niveau: 'info',
            texte: ligne('page served', { racine: etat.chemin, lisible: 'yes' }),
        };
    }
    // 🔴 `error`, NEVER `info`: it is the case this module exists to
    // make loud. The service STARTS anyway — refusing to start
    // would cut the API and the signaling for one page, which would be
    // disproportionate —, but it no longer does so silently.
    return {
        niveau: 'error',
        texte: ligne('page served', {
            racine: etat.chemin,
            lisible: 'no',
            cause: etat.cause,
            effet: 'every page will return 404 not found',
        }),
    };
}

/// PURE. The line of the RETAINED trust set.
///
/// 🔴 **THE EMPTY SET IS `info` AND NOT `error` — BUT NOT BECAUSE IT
/// WOULD BE THE "SAFE DEFAULT" OF THE `motdepasse` MODE: that sentence was
/// FALSIFIED by the review of correction round 3 of `frein(pont)`.**
///
/// ⚠️ **THIS DOC ANNOUNCED "fixed in three places
/// (`docker-compose.plateforme.yml`, `frein.ts`, here)": THE ENUMERATION
/// WAS WRONG IN BOTH DIRECTIONS** (found by the review of correction
/// round 4). `frein.ts` NEVER carried that claim — it says
/// not a word about it —, and two sites fixed by that same round were missing from
/// the list. **NO COUNT IS WRITTEN HERE, AND THIS IS INTENDED** — a count
/// ages from one round to the next, and round 4 itself added two
/// sites. The places are LOOKED UP:
///
/// ```sh
/// grep -rln 'aucun attaquant\|AUCUN ATTAQUANT' --include='*.ts' \
///   --include='*.yml' --include='*.md' --include='*.exemple' \
///   plateforme deploiement docker-compose.plateforme.yml
/// ```
///
/// A claim of COMPLETENESS in production code is checked with
/// a command, never from memory.
///
/// An empty set means "`X-Forwarded-For` is not trusted, and
/// `adresseSource` falls back on `req.socket.remoteAddress`" — safe ONLY
/// if that address is the one of the real CLIENT, that is, only if the
/// platform is exposed DIRECTLY. **That is NOT the setup this
/// repository SHIPS**: `docker-compose.plateforme.yml` puts nginx in front of it,
/// even in `motdepasse` mode, so that `remoteAddress` is ALWAYS
/// the address of the nginx container for every real request — the empty set there
/// makes `BUDGET_ADRESSE` **and** `BUDGET_REQUETES` degenerate (the latter
/// covering `GET /vm`, `POST /session` and `/signal`, HTTP and WebSocket
/// combined since the "frein(volume)" batch) into a budget SHARED by ALL
/// THE TRAFFIC, without any attacker having to forge anything — the
/// degeneration is automatic as soon as the second proxy exists.
///
/// ⚠️ **WHY THIS LEVEL STAYS `info` DESPITE THIS SEVERITY**: this
/// function is PURE and only receives the RETAINED set — it has NO
/// way of knowing whether the process calling it runs BEHIND a proxy or
/// exposed directly, and that is a question of DEPLOYMENT TOPOLOGY,
/// not of configuration that `config.ts` could settle for it. Making this
/// case `error` unconditionally would wrongly alarm the deployment where
/// the empty set is legitimately safe (direct exposure, no proxy).
/// **The `info` line therefore remains the operator's only witness** — that is
/// why it already names the exact effect (`X-Forwarded-For is not
/// trusted, and /auth/moi refuses every peer`) rather than a mere boolean — and
/// it is `deploiement/README.md` (invariant ③) that bears the responsibility
/// of saying, for THE setup this repository specifically ships, that this
/// `info` line actually announces an `error` risk.
export function annonceProxyDeConfiance(confiance: ReadonlySet<string>): Annonce {
    if (confiance.size === 0) {
        return {
            niveau: 'info',
            texte: ligne('trusted proxies', {
                retenus: 'none',
                effet: 'X-Forwarded-For is not trusted, and /auth/moi refuses every peer',
            }),
        };
    }
    return {
        niveau: 'info',
        texte: ligne('trusted proxies', {
            // ⚠️ ALL THE ENTRIES, NEVER A SAMPLE OR A TRUNCATION:
            // `obs/journal.ts` already carries the reason — a truncated address is
            // AMBIGUOUS, and the operator would no longer recognise their own. It is
            // also what makes a host name VISIBLE where it would not be
            // in a count.
            retenus: [...confiance].join(' '),
            count: confiance.size,
        }),
    };
}

/// The REAL probe of the disk. Rejects with a readable cause.
///
/// ⚠️ THREE THINGS ARE PROBED, NOT ONE: that the path exists, that it is
/// a DIRECTORY, and that it is READABLE AND TRAVERSABLE. An ordinary file
/// set as the root, or a directory without the `x` bit, produce exactly the
/// same silent `404` as a missing root — telling them apart at startup is
/// the whole point of this module.
export async function sonderRepertoire(chemin: string): Promise<void> {
    const infos = await stat(chemin);
    if (!infos.isDirectory()) throw new Error('this is not a directory');
    await access(chemin, constants.R_OK | constants.X_OK);
}

/// Resolves and probes, without writing anything.
///
/// ⚠️ `resolve()` IS CALLED HERE AND NOWHERE ELSE IN THIS MODULE: it is
/// the RESOLVED path that goes to the log, because it is the one the server
/// uses (`page/routes-page.ts` does the same `resolve`). Logging the
/// raw value would leave an ambiguous relative path, whose anchoring depends on the
/// working directory of the process.
///
/// ⚠️ THE PROBE IS INJECTED, with a real default: it is what makes the
/// three states testable without crafting an unreadable directory on the
/// disk of the test — and the injection costs the product nothing, since it never
/// calls this function otherwise than with its default.
export async function etatRacinePage(
    brut: string | undefined,
    sonder: (chemin: string) => Promise<void> = sonderRepertoire,
): Promise<EtatRacinePage> {
    // ⚠️ `''` IS TREATED AS ABSENCE, exactly as in
    // `page/routes-page.ts`: `resolve('')` returns the WORKING directory of the
    // process, and announcing it as the served root would be a lie.
    if (brut === undefined || brut === '') return { arme: false };
    const chemin = resolve(brut);
    try {
        await sonder(chemin);
    } catch (cause) {
        return { arme: true, chemin, lisible: false, cause: String(cause) };
    }
    return { arme: true, chemin, lisible: true };
}

/// Writing, isolated into a single gesture — the only one of this module that touches the
/// console.
export function write(annonce: Annonce): void {
    if (annonce.niveau === 'error') console.error(annonce.texte);
    else console.info(annonce.texte);
}
