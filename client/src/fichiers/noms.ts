// THE NAME CANONICALISER — the case remedy ON READ, and the only
// answer to what F1 bequeaths as no. 1. **PURE**: neither DOM, nor WebRTC, nor frame;
// the parent directory is INJECTED into it, as in `adaptateur.ts`.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 WHAT THIS MODULE FIXES, AND WHAT IT CANNOT FIX
// ════════════════════════════════════════════════════════════════════════════
//
// F1's legacy no. 1 says "case returns the wrong file". **The two
// halves of the phenomenon are not equally serious, and the half F1
// MEASURED is probably the benign one.** This is a REREADING of its evidence,
// not a new measurement, and it is said:
//
//   - **The VM half.** `Casse.txt` was HYDRATED (F1 records
//     `root hydrated … bytes=42 entries=1`). NTFS, case-insensitive,
//     resolves `casse.txt` onto the local file WITHOUT EVER REACHING THE BRIDGE.
//     The application gets the right content of the right file, and nothing is written
//     under a wrong name: no placeholder is created. **It is the
//     NORMAL behaviour of Windows, not a defect** — and this module can do
//     nothing about it: when NTFS answers, we are not consulted.
//   - **The BROWSER half.** On a case-INSENSITIVE local machine,
//     `getFileHandle('CASSE.TXT')` opens `Casse.txt`. **That is where the
//     wrong file is returned**, and that is where a write would overwrite.
//     **That half has NEVER been observed**: the acceptance instrument
//     is OPFS, and if OPFS is case-sensitive it cannot
//     happen there.
//   - **The inconsistency F1 records** — `casse.txt` passes, `GROS.BIN` fails,
//     in the SAME run — then reads without mystery: the first is
//     resolved by NTFS without us, the second reaches the bridge and hits a
//     case-sensitive OPFS.
//
// ⚠️ WHAT WOULD SETTLE IT: a run where the file requested with a different
// case was NEVER hydrated, AND where the "local machine" is case
// insensitive. **Neither condition is available on this setup**, and
// F3's results document says so again.
//
// WHAT IS DEMONSTRATED HERE, ON THE OTHER HAND: the behaviour on the host, with TWO
// fakes — one case-sensitive, the other INSENSITIVE —, and the disappearance of
// the `GROS.BIN` inconsistency on the instrument.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔵 UNICODE NORMALISATION, WHICH F2 DECLARES UNHANDLED AND BEQUEATHS HERE
// ════════════════════════════════════════════════════════════════════════════
//
// `client/src/fichiers/ecriture.ts` writes it out in full: "AND IT DOES NOT SEE
// UNICODE NORMALISATION. macOS stores its names in NFD, Windows in NFC:
// `résumé.txt` can exist there under two different sequences of code units, which
// `===` distinguishes and the user does not. The guard would
// then create a DUPLICATE instead of overwriting — less serious than the loss, but wrong.
// NOT HANDLED, declared; it is F3's canonicaliser."
//
// **It is handled**: folding applies `normalize('NFC')` BEFORE the case
// fold. Two names differing only by their normalisation form are
// therefore namesakes, exactly like two names differing only by
// case — and for the same reason: **the user does not tell them apart**.
//
// ⚠️ THE ORDER MATTERS, AND IT IS NOT ARBITRARY. `toLowerCase()` then
// `normalize()` is not the same function as `normalize()` then
// `toLowerCase()`: Unicode case folding can produce sequences
// that re-normalise. We normalise FIRST.
//
// ════════════════════════════════════════════════════════════════════════════
// ⚠️ NO CACHE. THE PARENT IS ENUMERATED AT EACH RESOLUTION.
// ════════════════════════════════════════════════════════════════════════════
//
// It is expensive — `adaptateur.ts` already declares the cost of one `getFile()` per entry
// in the listing —, and it is DELIBERATE: a cache nothing invalidates is
// exactly the old bridge's defect (`src/file.js`, cache WITHOUT TTL), and the
// only way to empty it — `Rafraichir` — is a deliverable of **F5**.
//
// **F3 therefore trades latency for correctness, and it is F4 that will say
// what the trade costs.**
//
// ⛔ **F4 DID NOT SAY IT, AND IT MUST BE WRITTEN RATHER THAN LETTING ONE BELIEVE THE
// OPPOSITE** (August 21st, 2026). No gesture of its campaign exercises case
// canonicalisation: its templates have neither a case namesake nor a path
// to correct. **The cost of this trade remains OWED.** The obvious optimisation — short-circuiting
// the enumeration when `poignee.name` already returns the stored name — IS NOT
// WRITTEN: it rests on a fact this setup cannot establish (it
// would take a real `showDirectoryPicker()`, which F1 measured unreachable on
// this host). **Bequeathed, not half implemented.**

import { FilesError, classer, type PoigneeRepertoire } from './adaptateur';
import { CODES_ECHEC, type CodeEchec } from '../../../proto/ts/fichiers';

/** What resolving a path component can return. */
export type Resolution =
    /** The **CANONICAL** name, that is, the one that is STORED. */
    | { sorte: 'trouve'; nom: string }
    | { sorte: 'absent' }
    /** Several entries fold onto the same name: we return NOTHING. */
    | { sorte: 'ambigu'; noms: string[] };

/**
 * The fold under which two names are "the same" for a user.
 *
 * ⚠️ NFC FIRST, case fold AFTERWARDS — see the header.
 */
export function plier(nom: string): string {
    return nom.normalize('NFC').toLowerCase();
}

/**
 * Resolves `demande` in `parent`, and returns the **STORED** name.
 *
 * The four outcomes, in the order they are decided:
 *
 *   1. an **exact** name found → it is that one, and it is the canonical name;
 *   2. no exact one, **exactly ONE** namesake → it is that one, and the canonical
 *      name is **the STORED name**, not the requested one;
 *   3. no exact one, **SEVERAL** namesakes → `ambigu`, we return nothing;
 *   4. nothing at all → `absent`.
 *
 * 🔴 RULE 1 PREVAILS, AND IT IS NOT A DETAIL. On a case-SENSITIVE local machine
 * carrying `note.txt` AND `Note.txt`, requesting `note.txt` is
 * perfectly designated: without the precedence of the exact one, it would become `ambigu`
 * and a legitimate read would be refused.
 */
export async function canoniser(
    parent: PoigneeRepertoire,
    demande: string,
): Promise<Resolution> {
    const cible = plier(demande);
    const homonymes: string[] = [];
    try {
        for await (const enfant of parent.values()) {
            // Rule 1: the exact one short-circuits everything, ambiguity included.
            if (enfant.name === demande) return { sorte: 'trouve', nom: demande };
            if (plier(enfant.name) === cible) homonymes.push(enfant.name);
        }
    } catch (e) {
        throw classer(e, 'chemin-introuvable');
    }
    if (homonymes.length === 1) return { sorte: 'trouve', nom: homonymes[0] };
    if (homonymes.length > 1) return { sorte: 'ambigu', noms: homonymes };
    return { sorte: 'absent' };
}

/**
 * [`canoniser`], but which THROWS instead of returning `absent` or `ambigu`.
 *
 * `siAbsent` distinguishes the two ways of being not found, exactly like
 * `adaptateur.classer`: a missing INTERMEDIATE component returns
 * `chemin-introuvable`, the FINAL component returns `introuvable`. ProjFS
 * distinguishes them too (`ERROR_PATH_NOT_FOUND` versus `ERROR_FILE_NOT_FOUND`), and
 * Explorer does not say the same thing about them.
 */
export async function canoniserOuLever(
    parent: PoigneeRepertoire,
    demande: string,
    siAbsent: CodeEchec,
): Promise<string> {
    const r = await canoniser(parent, demande);
    switch (r.sorte) {
        case 'trouve':
            return r.nom;
        case 'absent':
            throw new FilesError(siAbsent, `« ${demande} » n’existe pas`);
        case 'ambigu':
            // ⚠️ The message NAMES the namesakes, because that is all the
            // user will be able to do: rename one. The CODE crosses the
            // wire; the message stays in the console and in the shell page.
            throw new FilesError(
                'casse-ambigue',
                `« ${demande} » ne se distingue pas de « ${r.noms.join(' », « ')} » : ` +
                    `rendre l’un d’eux choisirait le mauvais fichier, rien n’a été fait`,
            );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FAULT INJECTION — the instrument of F3's criterion (4)
// ════════════════════════════════════════════════════════════════════════════
//
// THREE of the twelve causes of §5 are reachable by NO real gesture on
// this setup: `acces-refuse` (OPFS has no permission model, F1 §3),
// `disque-plein` (`QuotaExceededError` cannot be provoked there) and the
// timeout (it would take a browser that never answers).
//
// ⚠️ **This sentence announced "four" and only named three.** Fixed
// on the count, and F3's acceptance run found the TWO that were missing — they
// do not come under injection, but under a platform fact:
// `repertoire-non-vide` and `deja-present` say the mirror has DRIFTED, and
// **ProjFS shows the VM the content only the local machine knows**. Windows
// therefore resolves the drift BEFORE us, and those two codes stay out of reach
// of a real gesture. That makes FIVE causes out of twelve, for two
// different reasons that must not be confused.
//
// ⚠️ **AN INJECTION PROVES THE TABLE IS NOT DECORATIVE; IT DOES NOT PROVE
// THE CAUSE IS REACHABLE IN OPERATION.** The two columns are
// distinguished in §0.5 of the plan, and the results document will keep them
// distinct.

/** The prefix of a path component requesting a fault. */
export const PREFIXE_FAUTE = '.faute-';

/**
 * The suffix requesting a **silence** — the browser NEVER answers.
 *
 * ⚠️ It is not a `CodeEchec`: there is nothing to put on the wire, and that is
 * precisely the point. The bridge must notice the expiry itself,
 * that is, exercise `DelaiDepasse`, the only cause no answer can
 * produce.
 */
export const FAUTE_SILENCE = 'silence';

/**
 * Throws — or stays silent forever — if the FIRST path component requests a
 * fault **and** injection is ARMED.
 *
 * 🔴 **DISARMED BY DEFAULT, AND THE FLAG IS AN ARGUMENT.** Reading it from this
 * module (`location.search`, a module variable) would make it untestable, and
 * above all: a user creating a folder named `.faute-disque-plein`
 * would break their own bridge. The flag is read **once** in
 * `hub/page.ts` (`shell-page.ts` before the hub became the only
 * surface, August 31st, 2026) and passed as an argument, like `PLEIN_ECRAN` is on the
 * agent side.
 *
 * ⚠️ **THE FIRST COMPONENT, AND IT ALONE.** Scanning all components
 * would make a path crossing a folder named that way — even deep down —
 * fail, which would make injection hard to target and impossible to
 * disarm by gesture.
 */
export async function injecterFaute(
    parts: readonly string[],
    armee: boolean,
): Promise<void> {
    if (!armee || parts.length === 0) return;
    const premier = parts[0];
    if (!premier.startsWith(PREFIXE_FAUTE)) return;
    const demande = premier.slice(PREFIXE_FAUTE.length);
    if (demande === FAUTE_SILENCE) {
        // ⚠️ A PROMISE THAT NEVER RESOLVES. It is the only way
        // to exercise `DelaiDepasse`: an answer, whatever it is,
        // would prevent the bridge from expiring.
        return new Promise<void>(() => {});
    }
    if ((CODES_ECHEC as readonly string[]).includes(demande)) {
        throw new FilesError(
            demande as CodeEchec,
            `faute injectée par « ${premier} » : banc, jamais une configuration livrée`,
        );
    }
    // ⚠️ A `.faute-` whose suffix is NOT a known code is SAID, never
    // swallowed: without that, an acceptance typo would produce an ordinary
    // "not found", and the operator would believe they had exercised a code they had not
    // exercised.
    throw new FilesError(
        'interne',
        `« ${premier} » demande une faute inconnue « ${demande} » : ` +
            `les codes connus sont ${CODES_ECHEC.join(', ')}, plus « ${FAUTE_SILENCE} »`,
    );
}
