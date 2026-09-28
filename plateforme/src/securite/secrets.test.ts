// The sweep "no secret in a versioned file".
//
// 🔴 WHY NAMES AND NOT VALUES. Searching for VALUES is
// undecidable — a secret is an arbitrary string, and nothing distinguishes it
// from a test identifier. Searching for an ASSIGNMENT OF A KNOWN NAME is
// decidable, and it is exactly the gesture a human makes inadvertently:
// pasting the value of `TURN_SECRET` into a log, a plan, or a
// compose file.
//
// 🔴 THIS TEST MUST NEVER PRINT A VALUE. Its failure message names the
// FILE, the LINE and the NAME — never what follows the equals sign. A
// security test that copied the secret into the output of the test suite
// would write it into every CI log that captures it, and would leak it through
// the door it guarded. It is the same rule as
// `docker compose … config`, which P5's plan notes prints
// `--static-auth-secret` in clear.
//
// ⚠️ IT SWEEPS `git ls-files` AT THE ROOT OF THE REPOSITORY, `docs/` INCLUDED. It is
// deliberate and it is even the most likely case: acceptance logs
// are what one pours in the fastest and rereads the least.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const RACINE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');

/// The names whose literal assignment is a leaked secret.
const NOMS = [
    'TURN_SECRET',
    'PLATEFORME_SECRET_JETON',
    'AGENT_SECRET',
    'POSTGRES_PASSWORD',
    'WINDOWS_PASSWORD',
    'WINDOWS_ADMIN_PASSWORD',
    'RECETTE_MOTDEPASSE',
] as const;

/// The fingerprint of a value — never the value.
///
/// 🔴 IT IS WHAT MAKES AN EXCEPTION HONEST. An exception named only
/// by `<file>:<NAME>` would authorise ANY future value at that
/// place: the day someone replaced the acceptance fixture with a
/// real secret, the exception would cover it SILENTLY. By pinning
/// the fingerprint, any change of value turns the test RED, and one must
/// then look.
///
/// ⚠️ IT IS NOT THERE TO HIDE, AND IT MUST BE SAID: the values exempted
/// below are public fixtures, and several of their fingerprints
/// can be guessed in a second (`ba7816bf8f01cfea` is the SHA-256 of
/// "abc"). The fingerprint serves to DETECT A CHANGE, not to protect a
/// secret — and it is exactly what is asked of it, since no exempted
/// value is a secret.
function empreinte(value: string): string {
    return createHash('sha256').update(value, 'utf8').digest('hex').slice(0, 16);
}

interface Exception {
    file: string;
    nom: string;
    /// The fingerprint of the ALLOWED value, and of it alone.
    empreinte: string;
    raison: string;
}

/// The exceptions, EACH WITH ITS WRITTEN REASON — on the pattern of
/// `DRAPEAUX_INTERDITS` in `admin/enroler-agent.ts`.
///
/// ⚠️ NONE IS A REAL SECRET. The sweep was played on the state of the repository
/// as of 20 August 2026, and the thirteen assignments it first denounced were
/// READ ONE BY ONE: two were false positives of the regular
/// expression (the `${VAR:?message}` idiom, fixed since), and the other eleven
/// are fixtures — test ones, acceptance ones, or documentation examples.
/// No production secret has ever been versioned in this repository.
const EXCEPTIONS: readonly Exception[] = [
    {
        file: 'docker-compose.plateforme.yml',
        nom: 'POSTGRES_PASSWORD',
        empreinte: '3b132f52b3b4ad4d',
        raison:
            "The TEST Postgres instance, disposable, whose file header declares " +
            "in plain words that « a production database will NEVER use this " +
            'file ». The value is the same for everybody and protects nothing.',
    },
    {
        file: 'docs/superpowers/plans/2026-08-19-plateforme-p5.md',
        nom: 'POSTGRES_PASSWORD',
        // The SAME fingerprint as the line of the compose file: it is the
        // same value, and the pinning shows it rather than asserting it.
        empreinte: '3b132f52b3b4ad4d',
        raison:
            'The P5 plan QUOTES the line of the composition file above, to ' +
            "explain why it is exempted. It is the same fixture value, " +
            'copied into a sentence.',
    },
    // ❌ **A PER-PATH EXCEPTION WAS REMOVED HERE IN BATCH 33**:
    // `docs/superpowers/plans/2026-07-27-jalon1-tranche-verticale.md` /
    // `WINDOWS_ADMIN_PASSWORD` / fingerprint `ab5df625bc76dbd4`. Its written
    // reason was "a value that is literally "...", a
    // PLACEHOLDER" — that is, exactly what the `...` branch of
    // `inoffensive` now covers, for ALL paths. Removing it is
    // what proves the branch bites: if it did not bite, this file
    // would turn red.
    {
        file: 'docs/superpowers/plans/2026-07-29-traversee-nat.md',
        nom: 'TURN_SECRET',
        empreinte: '2bb80d537b1da3e3',
        raison:
            "A test fixture copied into the plan: the value is the word " +
            '« secret » itself, and the same literal lives in `signaling/ice.test.ts`.',
    },
    {
        file: 'plateforme/src/signaling/ice.test.ts',
        nom: 'TURN_SECRET',
        empreinte: '2bb80d537b1da3e3',
        raison:
            'The fixture of the `configurationIce` test: the value is the word ' +
            "« secret ». A real secret would be useless there — the test checks the SHAPE " +
            "of the derived identifier, not its strength.",
    },
    {
        file: 'docs/superpowers/plans/2026-08-19-plateforme-p3.md',
        nom: 'AGENT_SECRET',
        empreinte: 'ba7816bf8f01cfea',
        raison:
            "A three-letter value in a SYNTAX check command " +
            "(`bash -n`): the script is never run, and the value reaches " +
            'no VM.',
    },
    {
        file: 'docs/superpowers/plans/journaux-corrections/instrument/compter-enrolements.sh',
        nom: 'PLATEFORME_SECRET_JETON',
        empreinte: '6b82a0dca0d6fa4d',
        raison:
            "The signing secret of an ACCEPTANCE instrument, drawn for that " +
            "acceptance run and dead with it. It signs no token of a live service.",
    },
    {
        file: 'docs/superpowers/plans/journaux-plateforme-p3/instrument/jouer.sh',
        nom: 'TURN_SECRET',
        empreinte: '20e73cf9ccbda64a',
        raison:
            "The TURN secret of a P3 acceptance instrument, named « recette-p3-… » " +
            "precisely so that it is not mistaken for the one of the real relay.",
    },
    {
        file: 'docs/superpowers/plans/journaux-plateforme-p3/instrument/rouge-1a.ts',
        nom: 'PLATEFORME_SECRET_JETON',
        empreinte: '2923f1439452d95c',
        raison:
            'Likewise: the signing fixture of red run ①A of P3, named ' +
            '« recette-p3-… », dead with its acceptance run.',
    },
    {
        file: 'plateforme/src/config.test.ts',
        nom: 'PLATEFORME_SECRET_JETON',
        empreinte: '94e4c4bc7d176bd9',
        raison:
            "The value is literally « trop-court »: it is the test that checks " +
            "that `lireConfig` REFUSES a secret under `LONGUEUR_SECRET_MIN`.",
    },
    {
        file: 'tests/desk_activate_fixtures.py',
        nom: 'AGENT_SECRET',
        empreinte: 'fc66b5649cf2782e',
        raison:
            "The SIMULATED output of a fake `npm run admin:agent`, in the tests of the " +
            "`desk` package: this dummy script prints this line so that the " +
            "activate.py hook under test believes it enrolled an agent, without ever talking to " +
            "a real platform. It is the identifier of NO real agent. " +
            "Rewriting the fixture would be pointless — the detector looks for NAMES, " +
            "never for values (see the file header), so a changed value " +
            "would be reported just the same; and concatenating the string to dodge " +
            "the regular expression would be worse than the harm: the fixture would become " +
            "invisible to any future audit, whereas here it is visible and discussed.",
    },
];

/// A value is HARMLESS if it cannot be a pasted secret.
///
/// ⚠️ EACH BRANCH IS A POTENTIAL HOLE, and that is why they are
/// enumerated here rather than drowned in a regular expression: a
/// successor who adds one will have to write why.
function inoffensive(value: string): boolean {
    // Empty: `TURN_SECRET=` carries nothing.
    if (value === '') return true;
    // Shell, docker compose or Windows interpolation: the value comes
    // from elsewhere, and "elsewhere" is not versioned.
    if (/^[$%]/.test(value)) return true;
    // A command SUBSTITUTION: `$(openssl rand -hex 32)`.
    if (value.startsWith('(')) return true;
    // A code IDENTIFIER, in upper case: `PLATEFORME_SECRET_JETON: SECRET`
    // designates a TypeScript constant, not a value. A real secret in
    // pure upper case with no lower-case digit would be a laughable secret.
    if (/^[A-Z][A-Z0-9_]*$/.test(value)) return true;
    // An explicit PLACEHOLDER: `<generated>`, `<your secret>`.
    if (value.startsWith('<')) return true;
    // 🔴 THE ELLIPSIS, WHICH IS THE SAME PLACEHOLDER UNDER ANOTHER
    // SIGN — added in batch 33, after the check CRIED WOLF.
    //
    // `RECETTE_MOTDEPASSE=...` in a usage line
    // (`journaux-lot31/instrument/pilote-lot31.mjs:19`) turned all of
    // `verify-all.sh` red, for two steps, on a line that carries no
    // value. **It was not an isolated false positive: it was a CLASS**,
    // and it was handled exception by exception, PER PATH — so a
    // new usage example, in any document or acceptance
    // instrument, reopened it.
    //
    // ⚠️ **A CHECK THAT MUST BE AMENDED AT EVERY NON-FINDING TEACHES
    // ITS READERS TO BRUSH IT ASIDE** — and that is how a
    // real finding ends up brushed aside too. Closing the class at the level of the
    // VALUE is better than lengthening the list of paths.
    //
    // ⚠️ **WHAT THIS BRANCH DOES NOT OPEN**: it only accepts the
    // WHOLE value `...` (or `…`), never a prefix or a suffix. `abc...` stays
    // denounced, and a real secret that was exactly three dots is
    // not a secret.
    if (value === '...' || value === '…') return true;
    return false;
}

interface Trouvaille {
    file: string;
    ligne: number;
    nom: string;
    /// The fingerprint of the value — never the value.
    empreinte: string;
}

/// Extracts the assigned value, without ever returning it to the caller other
/// than to classify it.
function valueAfter(reste: string): string {
    const t = reste.trimStart();
    // A quoted string: we read up to the closing quote.
    const cite = /^(['"`])(.*?)\1/.exec(t);
    if (cite) return cite[2];
    // Otherwise, up to the first separator.
    //
    // ⚠️ THE BACKTICK IS A SEPARATOR, AND IT IS NOT A DETAIL: it
    // closes a code span in Markdown, and this repository is written in
    // Markdown. Without it, the sentence "`TURN_SECRET=` carries nothing" of a
    // comment makes the value "`" be extracted, which no branch
    // of `inoffensive` catches — and the sweep denounces ITSELF.
    //
    // 🔴 IT HAPPENED, AND THE WAY IT WAS MISSED IS THE LESSON. The test
    // went GREEN before being committed — hence before being tracked by git,
    // hence BEFORE `git ls-files` SAW IT. A sweep that reads
    // `git ls-files` only measures itself once TRACKED: the green
    // from before the commit did not measure the repository after it. It only turned red at
    // the red of `essai-secret.env`, which revealed it by accident.
    //
    // ⚠️ THIS TOLERANCE CREATES NO HOLE. A name followed by "=" then
    // by a real value, inside a Markdown code span, still returns
    // that value, which stays denounced. Only an assignment
    // IMMEDIATELY followed by a backtick — hence EMPTY — becomes
    // harmless, and an empty assignment is harmless anyway.
    //
    // ⚠️ AND THIS VERY COMMENT DEMONSTRATED IT: its first
    // draft carried the literal example, the sweep denounced it, and it had
    // to be rephrased. The test therefore watches itself, which is the
    // property asked of it — but it forces one to WRITE its examples
    // without ever composing them.
    return /^[^\s,;)}\]`]*/.exec(t)?.[0] ?? '';
}

function balayer(): Trouvaille[] {
    const files = execFileSync('git', ['ls-files', '-z'], {
        cwd: RACINE,
        maxBuffer: 64 * 1024 * 1024,
    })
        .toString('utf8')
        .split('\0')
        .filter((f) => f !== '');

    // `(?:=(?!=))`: `===` and `==` are COMPARISONS, not
    // assignments. Without this guard, `AGENT_SECRET === x` would be denounced.
    // `(?<!\$)\{`: in `${VAR:?message}` or `${VAR:+…}`, the `:` is part
    // of an INTERPOLATION, not of an assignment. Without this guard, the idiom
    // `\${VAR:?message}` — the very one the deployment uses to make
    // a variable mandatory — would be denounced as a secret. MEASURED: it
    // produced 3 of the first 13 findings.
    //
    // `(?:=(?!=))`: `===` and `==` are COMPARISONS. Without this guard,
    // `AGENT_SECRET === x` would be denounced.
    const motif = new RegExp(
        `(?:^|[\\s"'\`(,;]|(?<!\\$)\\{)(${NOMS.join('|')})\\s*(?::|=(?!=))(.*)$`,
    );

    const trouvailles: Trouvaille[] = [];
    for (const file of files) {
        const chemin = path.join(RACINE, file);
        let size: number;
        try {
            size = statSync(chemin).size;
        } catch {
            // A file tracked but absent from disk (uncommitted
            // deletion): there is nothing to read, and it is not our subject.
            continue;
        }
        // Large binaries have no readable assignment, and reading them
        // would cost without teaching anything.
        if (size > 4 * 1024 * 1024) continue;
        let contenu: string;
        try {
            contenu = readFileSync(chemin, 'utf8');
        } catch {
            continue;
        }
        if (!NOMS.some((n) => contenu.includes(n))) continue;

        const lignes = contenu.split('\n');
        for (let i = 0; i < lignes.length; i++) {
            const m = motif.exec(lignes[i]);
            if (!m) continue;
            const value = valueAfter(m[2]);
            if (inoffensive(value)) continue;
            trouvailles.push({
                file,
                ligne: i + 1,
                nom: m[1],
                empreinte: empreinte(value),
            });
        }
    }
    return trouvailles;
}

describe('no secret in a versioned file', () => {
    it('🔴 finds no literal assignment outside the declared exceptions', () => {
        const trouvailles = balayer();
        const autorisees = new Set(
            EXCEPTIONS.map((e) => `${e.file}:${e.nom}:${e.empreinte}`),
        );
        const hors = trouvailles.filter(
            (t) => !autorisees.has(`${t.file}:${t.nom}:${t.empreinte}`),
        );
        // ⚠️ THE MESSAGE NAMES THE FILE, THE LINE AND THE NAME — NEVER THE
        // VALUE. See the header: a security test that printed the
        // secret would leak it through the door it guards.
        expect(
            hors.map((t) => `${t.file}:${t.ligne} assigns ${t.nom} [fingerprint ${t.empreinte}]`),
            "secrets are assigned in plaintext in versioned files " +
                "(the value is deliberately not shown)",
        ).toEqual([]);
    });

    it('each exception carries a non-empty REASON', () => {
        // Without this assertion, within two workstreams the list would become a
        // list of things one gave up understanding.
        for (const e of EXCEPTIONS) {
            expect(e.raison.trim().length, `the exception ${e.file}:${e.nom} has no reason`)
                .toBeGreaterThan(30);
            expect(e.empreinte, `the exception ${e.file}:${e.nom} pins no value`)
                .toMatch(/^[0-9a-f]{16}$/);
        }
    });

    it('🔴 each exception matches a REAL finding', () => {
        // 🔴 AN EXCEPTION THAT NO LONGER COVERS ANYTHING IS A SLEEPING LIE:
        // the day the file changes, it keeps authorising a path
        // nobody rereads. This assertion brings it down the day
        // it stops being necessary.
        const reelles = new Set(
            balayer().map((t) => `${t.file}:${t.nom}:${t.empreinte}`),
        );
        for (const e of EXCEPTIONS) {
            const cle = `${e.file}:${e.nom}:${e.empreinte}`;
            expect(reelles.has(cle), `the exception ${e.file}:${e.nom} no longer covers anything`)
                .toBe(true);
        }
    });

    it('the sweep REALLY sees files — it cannot be empty by accident', () => {
        // ⚠️ CHECK OF THE CHECK. A `git ls-files` that returned zero files
        // — wrong `cwd`, absent repository — would make the main test pass
        // green without having swept anything. It is the pattern of the vacuous check,
        // which this repository has paid for four times.
        const files = execFileSync('git', ['ls-files', '-z'], {
            cwd: RACINE,
            maxBuffer: 64 * 1024 * 1024,
        })
            .toString('utf8')
            .split('\0')
            .filter((f) => f !== '');
        expect(files.length).toBeGreaterThan(500);
        expect(files).toContain('docker-compose.plateforme.yml');
    });
});
