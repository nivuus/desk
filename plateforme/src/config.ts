// PLATEFORME_HOTE has NO default, and that is the point of this function.
//
// The former `signaling/src/server.ts:67` — today `src/signaling/relais.ts`,
// the `signaling/` package having disappeared in sub-block P1 — did
// `new WebSocketServer({ port })` without
// `host`: the service listened on all interfaces, and handed out
// TURN credentials valid for 24 h (`ice.ts:16`) to anyone reaching the
// port. Setting a default here — even `127.0.0.1` — would make criterion ④
// of sub-block P1 pass without guaranteeing anything: the operator would never know what
// it listens on. A loud break is better than a silent universal
// listen.
//
// ⚠️ ONE HALF OF THIS "TO ANYONE" HAD SURVIVED SUB-BLOCK P2, AND IT HAS
// BEEN CLOSED SINCE P3. The text here said that a peer declaring itself
// `{"role":"agent"}` was still accepted without identity and received its
// 86,400 s TURN credentials; that is no longer true — the `agent` role requires
// its token, of type `agent` and with a subject prefixing the session
// (`identite/garde.ts`), and that token is obtained on the `/agent` channel in exchange for the
// enrolment secret of the VM.
//
// **The argument above loses NOTHING by it**, and that is why the paragraph is
// corrected rather than removed: `PLATEFORME_HOTE` bounds WHO CAN REACH the
// port, which holds before any authentication and for both paths —
// the relay as well as the enrolment channel.
//
// ✅ **P5 DELIVERED WHAT THIS SENTENCE ANNOUNCED**, and it is corrected rather
// than removed: it said "secret attempts are throttled by
// nothing to date (that is the subject of P5)". They are — `agents/canal.ts`
// consults the throttle BEFORE `verifyEnrolment`, hence before any `scrypt`.
// The `PLATEFORME_HOTE` argument above loses nothing by it: it still holds
// before any authentication, and it covers the relay, which the throttle does NOT
// cover (see the annotation of `signaling/resilience.test.ts`).

//
// The environment is read HERE and nowhere else: `env` is a
// parameter, never `process.env` read on the sly, which makes the function pure and
// testable without dirtying the environment of the test process.

import { MIN_SECRET_LENGTH } from './identite/jeton';

export interface Config {
    /// PLATEFORME_HOTE — no default, see the header comment.
    hote: string;
    /// PLATEFORME_PORT, default 8080.
    port: number;
    /// PLATEFORME_BASE, default 'sqlite'. An unknown value THROWS: a silent
    /// fallback to sqlite would run production on a local
    /// file without anything saying so.
    base: 'sqlite' | 'postgres';
    /// PLATEFORME_BASE_URL — SQLite file path or pg connection URL.
    urlBase: string;
    /// PLATEFORME_SECRET_JETON — NO default, at least `MIN_SECRET_LENGTH`
    /// characters. See the comment below: a secret drawn
    /// at random at startup would be worse than no secret at all.
    secretJeton: string;
    /// PLATEFORME_ORIGINE_CLIENT — OPTIONAL. When absent, no CORS header
    /// is emitted and the browser refuses: the default is refusal.
    origineClient?: string;
    /// PLATEFORME_PROXY_DE_CONFIANCE — OPTIONAL, comma-separated
    /// list. Absent or empty, the set is EMPTY: the `X-Forwarded-For` header
    /// is trusted from NO source. See the comment at the point where it is
    /// read, further down.
    ///
    /// ⚠️ IT IS NEVER `undefined`: an empty set can be iterated, an
    /// `undefined` gets dereferenced. That is the intended asymmetry with
    /// `origineClient` above, whose absence means something to the caller
    /// ("emit no header") where this one's means only one thing ("trust
    /// nobody"), already carried by the empty set.
    proxyDeConfiance: ReadonlySet<string>;
    /// PLATEFORME_ICONES — OPTIONAL, default `donnees/icones`. The directory (policy: allow-fr - path on the target)
    /// of the content-addressed icon store (sub-block G2).
    ///
    /// ⚠️ **ASYMMETRY WITH `PLATEFORME_HOTE` ACCEPTED, AND WE MUST SAY
    /// WHY.** The header comment of this file grounds the absence of a
    /// default on the fact that a bad default WOULD EXPOSE the service. Here, a
    /// bad directory costs **one re-upload, bounded and automatic**:
    /// the store rebuilds itself at the next reconciliation, because
    /// the platform asks for what it lacks by querying its DISK.
    /// A loud break would not be proportionate — but neither would
    /// silence, hence the log line when the store opens.
    repertoireIcones: string;
    /// PLATEFORME_TELEVERSEMENTS — OPTIONAL, default `donnees/televersements`. (policy: allow-fr - path on the target)
    /// The root of the SLICE store: `<racine>/<id>/<n>`, one file per
    /// slice, and never an assembled file (sub-block G3).
    ///
    /// ⚠️ **SAME ASYMMETRY WITH `PLATEFORME_HOTE` ACCEPTED AS `repertoireIcones`
    /// ABOVE, AND IT MUST BE STATED RATHER THAN INHERITED.** There, a bad
    /// default WOULD EXPOSE the service; here it costs a RE-UPLOAD — bounded,
    /// and visible to the user who redoes it. A loud break would not
    /// be proportionate.
    ///
    /// ⚠️ **BUT THE CONSEQUENCE IS HEAVIER THAN FOR THE ICONS, AND IT
    /// IS NOT THE SAME WORD.** The icon store rebuilds ITSELF —
    /// the platform asks the agent again for what its disk lacks. A
    /// lost upload, on the other hand, is not rebuilt: a human has to
    /// upload their file again. Silence is therefore even less acceptable
    /// here — hence the log line when the store opens.
    repertoireTeleversements: string;
    /// PLATEFORME_PAGE — OPTIONAL, and **NO DEFAULT**, unlike
    /// `PLATEFORME_ICONES` and `PLATEFORME_TELEVERSEMENTS` just below.
    ///
    /// 🔴 ABSENT OR EMPTY ⇒ THE SERVICE SERVES NO FILE, and its
    /// behaviour is that from before the batch DOWN TO THE BYTE: `GET /` returns
    /// `404 introuvable`. That is what makes the addition strictly additive — and
    /// it is what makes the negative witness of the acceptance run playable.
    ///
    /// ⚠️ A DEFAULT WOULD BE A SECURITY DEFECT, not a convenience: in the
    /// nginx setup, the platform must serve NOTHING, and a default would
    /// make it publish whatever its working directory contains.
    racinePage?: string;
    /// PLATEFORME_SOCKET_VM: absolute path of the host control channel (Unix
    /// socket) through which the service wakes the VM. Absent or empty, the
    /// channel is disabled and the service keeps the static inventory, which
    /// refuses to start a VM (`non-supporte`). A relative path THROWS: it would
    /// resolve against the working directory and silently point nowhere.
    socketVm?: string;
    /// PLATEFORME_AUTH, default 'pomerium'. An unknown value THROWS.
    ///
    /// ⚠️ IT IS NOT AN ARMING, IT IS A MODE CHOICE — the `=0 disarms`
    /// convention of `agent/` does not apply here. The precedent is
    /// `PLATEFORME_BASE` fifteen lines above, and for the same reason: a
    /// silent fallback would run one mode under the name of the other, and
    /// one of the two directions is an OPENING.
    auth: 'pomerium' | 'motdepasse';
}

const BASES = ['sqlite', 'postgres'] as const;
const AUTHS = ['pomerium', 'motdepasse'] as const;

/// The addresses that make the service listen on ALL interfaces.
///
/// 🔴 IT IS NOT "THE LISTEN IS BOUNDED", AND THE DIFFERENCE IS WRITTEN DOWN RATHER
/// THAN DISGUISED. The check one would like — "it must be a loopback
/// address" — would break the shipped deployment, which sets `PLATEFORME_HOTE:
/// plateforme`, a Docker service name with no published port, and which is the
/// safest setup of the three. What is decidable is refusing
/// the UNIVERSAL listen; that only Pomerium reaches the port remains the
/// operator's responsibility, and that is stated in § 9 of the spec.
const ECOUTES_UNIVERSELLES = new Set(['0.0.0.0', '::', '[::]', '*']);

export function lireConfig(env: Record<string, string | undefined>): Config {
    const hote = env.PLATEFORME_HOTE;
    if (hote === undefined || hote === '') {
        throw new Error(
            "PLATEFORME_HOTE is required and has no default: name the listen " +
                "address, otherwise the service would listen on every interface.",
        );
    }

    const brutAuth = env.PLATEFORME_AUTH;
    const auth = (brutAuth === undefined || brutAuth === '' ? 'pomerium' : brutAuth) as Config['auth'];
    if (!(AUTHS as readonly string[]).includes(auth)) {
        throw new Error(
            `PLATEFORME_AUTH must be ${AUTHS.join(' or ')}, got: ${brutAuth}`,
        );
    }

    // 🔴 TIED TO THE MODE, AND NOT UNIVERSAL. In `motdepasse` mode, the service
    // authenticates by itself and a wide listen does not make it anonymous; in
    // `pomerium` mode, the identity arrives in a CLEARTEXT header, and a
    // universal listen offers it to anyone reaching the machine.
    if (auth === 'pomerium' && ECOUTES_UNIVERSELLES.has(hote.trim())) {
        throw new Error(
            `PLATEFORME_HOTE=${hote} is a universal listen, refused in ` +
                "pomerium mode: the identity arrives in a plaintext header that only the " +
                'proxy must be able to set. Name a precise address ' +
                '(192.168.3.1, 127.0.0.1) or an internal network service name.',
        );
    }

    const brutPort = env.PLATEFORME_PORT;
    let port = 8080;
    if (brutPort !== undefined && brutPort !== '') {
        if (!/^\d+$/.test(brutPort)) {
            throw new Error(`PLATEFORME_PORT must be an integer, got: ${brutPort}`);
        }
        port = Number(brutPort);
    }

    const brutBase = env.PLATEFORME_BASE;
    const base = (brutBase === undefined || brutBase === '' ? 'sqlite' : brutBase) as Config['base'];
    if (!(BASES as readonly string[]).includes(base)) {
        throw new Error(
            `PLATEFORME_BASE must be ${BASES.join(' or ')}, got: ${brutBase}`,
        );
    }

    const urlBase = env.PLATEFORME_BASE_URL ?? ':memory:';

    // ⚠️ THE EMPTY STRING TEST IS DISTINCT FROM THE ABSENCE ONE:
    // `env.X ?? 'defaut'` does NOT catch `''`, and P1 paid for this exact
    // mistake. An empty `PLATEFORME_ICONES=` must fall back to the default, not
    // make the working directory the store.
    const brutIcones = env.PLATEFORME_ICONES;
    const repertoireIcones =
        brutIcones === undefined || brutIcones === '' ? 'donnees/icones' : brutIcones;

    // Same EMPTY string guard, and for the same reason as above: an
    // empty `PLATEFORME_TELEVERSEMENTS=` would make the slice root the
    // WORKING directory of the service, where they would mix with its sources.
    const brutTeleversements = env.PLATEFORME_TELEVERSEMENTS;
    const repertoireTeleversements =
        brutTeleversements === undefined || brutTeleversements === ''
            ? 'donnees/televersements'
            : brutTeleversements;

    // Same EMPTY string guard as above, but WITHOUT a fallback: here, empty
    // and absent both mean "no server".
    const brutPage = env.PLATEFORME_PAGE;
    const racinePage = brutPage === undefined || brutPage === '' ? undefined : brutPage;

    const brutSocketVm = env.PLATEFORME_SOCKET_VM;
    const socketVm = brutSocketVm === undefined || brutSocketVm === '' ? undefined : brutSocketVm;
    if (socketVm !== undefined && !socketVm.startsWith('/')) {
        throw new Error(
            `PLATEFORME_SOCKET_VM must be an absolute path, received: ${socketVm}. ` +
                'Empty, the VM wake channel is disabled.',
        );
    }

    // 🔴 NO DEFAULT, and above all not a RANDOM default. A secret drawn at
    // startup would pass all the shape tests, then invalidate at every
    // restart the whole set of tokens handed out — users would be
    // logged out without any trace giving the cause. It is the same
    // decision as `PLATEFORME_HOTE`: a loud break is better than a
    // silent degradation.
    //
    // The EMPTY string test is distinct from the absence one, because
    // `env.X ?? 'defaut'` does not catch `''` — P1 paid for this exact mistake
    // in its task 1, where one of the two announced reds was in fact green.
    const secretJeton = env.PLATEFORME_SECRET_JETON;
    if (secretJeton === undefined || secretJeton === '') {
        throw new Error(
            "PLATEFORME_SECRET_JETON is required and has no default: without it " +
                'no token can be signed, and a random default would invalidate ' +
                'every session at each restart.',
        );
    }
    if (secretJeton.length < MIN_SECRET_LENGTH) {
        throw new Error(
            `PLATEFORME_SECRET_JETON is too short: ${secretJeton.length} characters, ` +
                `at least ${MIN_SECRET_LENGTH} are required — a guessable secret ` +
                "authenticates nobody.",
        );
    }

    // OPTIONAL, unlike `PLATEFORME_HOTE`, and the asymmetry lies in
    // the consequences: a missing origin produces a LOUD refusal from the
    // browser, which the operator sees immediately; a missing listen
    // address would produce a SILENT universal listen. Refusing to
    // start for it would moreover break the P5 deployment, where the
    // reverse proxy puts the client and the platform on the SAME origin and where
    // no value would make sense.
    const brutOrigine = env.PLATEFORME_ORIGINE_CLIENT;
    const origineClient = brutOrigine === undefined || brutOrigine === '' ? undefined : brutOrigine;

    // OPTIONAL — but only IN `motdepasse` MODE, AND IT HAS BECOME FALSE
    // IN THE OTHER MODE (task 6, review "correction round 1", 22 August
    // 2026). This sentence said "refusing to start would break these two
    // cases": a deployment WITHOUT a reverse proxy — the one of the tests, and the one
    // of an operator who exposes the service directly — would have no
    // value that makes sense here. THAT WAS TRUE BEFORE THE GUARD FURTHER DOWN IN
    // THIS FUNCTION, WHICH DOES PRECISELY THAT IN `pomerium` MODE — THE
    // DEFAULT: `lireConfig` now refuses to start if this variable
    // is absent or empty AND the mode is `pomerium`. The sentence remains
    // true for the `motdepasse` mode ALONE, where the header is read by
    // nobody and where the absence of a declared proxy is a perfectly healthy case
    // (the tests of this file, for instance).
    //
    // 🔴 BUT ITS DEFAULT IS THE REFUSAL TO TRUST, NEVER A PERMISSION. Same
    // doctrine as `PLATEFORME_ORIGINE_CLIENT`: absent, the set is empty,
    // and `http/adresse-source.ts` then ignores `X-Forwarded-For` whatever it
    // is. A permissive default — trusting everybody's header, or even
    // only private addresses — would make the client address FORGEABLE
    // BY THE CLIENT, hence the per-address throttle bypassable with one header
    // line. It is the only default that trades a loud failure for
    // a silent bypass, and that is exactly what this file has been
    // refusing since `PLATEFORME_HOTE`.
    //
    // ⚠️ THE FAILURE MODE OF FORGETTING IS NAMED, and it is not
    // silent by accident but by DELIBERATE CHOICE. An operator who sets up a
    // proxy without declaring trust in it will see ALL requests carry
    // the proxy address: the per-address throttle degenerates into a GLOBAL throttle, and
    // the service refuses itself at the 51st failure. The remedy is NOT to
    // trust by default — that would be the bypass above — but that THE
    // THROTTLE TRACE NAMES THE ADDRESS IT RETAINED: an operator who reads
    // `adresse=172.18.0.5` on every line recognises the address of their
    // proxy. `deploiement/README.md` says so too.
    //
    // ⚠️ EXACTLY ONE PROXY AT THE HEAD OF THE CHAIN. Two chained proxies make
    // `adresseSource` return the address of the FIRST PROXY, not that of the
    // client: P5 does not deliver the N-hop chain, and
    // `http/adresse-source.ts` documents it.
    //
    // The EMPTY string test is distinct from the absence one, for the
    // reason already paid for by P1 further up in this file: `??` does not catch
    // `''`. Here, `''.split(',')` would return `['']` — hence a set with ONE
    // empty entry, which would make trusted any peer without an address.
    const brutProxy = env.PLATEFORME_PROXY_DE_CONFIANCE;
    const proxyDeConfiance: ReadonlySet<string> = new Set(
        (brutProxy ?? '')
            .split(',')
            .map((entree) => entree.trim())
            // Without this filter, `'172.18.0.5,,10.0.0.1'` would carry an empty
            // entry — and `'  '` would carry one too, after `trim`.
            .filter((entree) => entree !== ''),
    );

    // 🔴 THIRD GUARD TIED TO THE MODE, after the `PLATEFORME_HOTE` one. Same
    // reason: in `pomerium`, the identity arrives in a CLEARTEXT header
    // that no signature verifies, and without the list of addresses allowed
    // to set it, the header can be believed from anybody.
    //
    // ⚠️ WHY A REFUSAL TO START AND NOT A 401: a refusal is read BEFORE
    // acting and names the variable. A 401 for everybody would be read AFTER,
    // on a service that answers and serves the ten other routers.
    if (auth === 'pomerium' && proxyDeConfiance.size === 0) {
        throw new Error(
            'PLATEFORME_PROXY_DE_CONFIANCE is required in pomerium mode: ' +
                "the identity arrives in a plaintext header that no signature verifies, " +
                'and without the list of addresses allowed to set it, anyone reaching the ' +
                "port gets a token for the identity of their choice. Set the address of the " +
                'proxy, or PLATEFORME_AUTH=motdepasse.',
        );
    }

    return {
        hote,
        port,
        base,
        urlBase,
        secretJeton,
        origineClient,
        proxyDeConfiance,
        repertoireIcones,
        repertoireTeleversements,
        racinePage,
        socketVm,
        auth,
    };
}
