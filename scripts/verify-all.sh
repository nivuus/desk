#!/usr/bin/env bash
#
# Chains all the checks of the project in a single command, in
# order: Rust tests, Rust lint, TypeScript tests and typing (client, proto,
# then plateforme). It plays the TEN steps to the end, even after a
# red, and sums up at the end the ones that failed — see the box placed
# on `echecs` below, which says what this trade-off cost when it was
# the other way round.
#
# Why this script exists: it is the only place in the project that checks
# strict TypeScript typing. `npm test` (Vitest) and `npm run build` (Vite)
# both rest on esbuild, which transpiles without ever checking
# types — a file can thus have entirely green tests with
# broken typing. During work item B ("Game input"), two
# successive reviews approved a task on that basis alone, while
# `tsc --noEmit` failed with two errors located in
# production code. This script exists so that it does not happen again.
#
# ⚠️ THIS SCRIPT DEPENDS ON A POSTGRES INSTANCE, and that is INTENDED. `plateforme`
# exercises its SQL subset against BOTH engines, and a skip is a
# failure (spec §7.1 of sub-project ⑤): if the instance is missing, the step
# `plateforme: npm run test:postgres` FAILS — it is not skipped with a
# warning. A test that vanishes when its dependency is missing turns green a
# state it did not measure. To launch it:
#
#     docker compose -f docker-compose.plateforme.yml up -d
#
# Until sub-block P1 (19 August 2026), the signaling service was OUTSIDE
# this net: its tests were played by no step, and its package did not
# even declare a `typecheck` script. That is the gap the three
# `plateforme` steps close.

set -euo pipefail

racine="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$racine"

etape() {
    echo
    echo "==> $1"
}

# 🔴 THE SCRIPT NO LONGER STOPS AT THE FIRST FAILURE, AND THAT IS A TRADE-OFF, NOT
# A GIVEN. It used to stop; on 20 August 2026, a single red step
# (`plateforme: npm run test:sqlite`) hid the LAST TWO —
# `test:postgres` and `typecheck` — for the whole duration of the defect. Nobody
# knew whether they were green: they were not measured. An early
# failure thus cost TWO losses, its own and that of everything downstream.
#
# What stopping bought — speed — is worth almost nothing here: the whole
# chain runs in ~30 s fully cached (measured). What it cost is the
# verdict itself: a barrier that reports only one line out of ten does not tell
# the state of the tree, it tells the state of its first step.
#
# The ten steps are INDEPENDENT — each is a self-contained `(cd X && …)`,
# and none consumes the output of another —, so continuing after a
# red measures nothing wrong. The exit status stays 1 as soon as a single one has
# failed: it is not a barrier being softened, it is a barrier that
# finally reports everything it measured.
echecs=()

echec() {
    echo "FAILED: $1" >&2
    echecs+=("$1")
}

etape "cargo test --workspace"
cargo test --workspace || echec "cargo test --workspace"

etape "cargo clippy --workspace"
# Without -D warnings: the workspace carries 33 pre-existing dead_code
# warnings, measured on 30 July 2026 (file size debt reduction
# work item) — the figure of 31 inherited from work item B had
# never been measured again since. All due to code compiled for Windows or by
# the tests but invisible to an ordinary Linux lint (`geometry.rs`,
# `opus.rs`, `rebuild.rs`, `gamepad.rs`, `cursor.rs`, `audio.rs`,
# `diagnostics/entree.rs`, `transport/piste_audio.rs::set_audio_source`...).
# Fixing them is not this script's role; hiding them with -D warnings
# would be even less so — we leave them visible, without blocking on them.
cargo clippy --workspace || echec "cargo clippy --workspace"

etape "client: npm test"
(cd client && npm test) || echec "client: npm test"

etape "client: npm run typecheck"
(cd client && npm run typecheck) || echec "client: npm run typecheck"

# The seven checks of the visual foundation (sub-project ⑥, spec §7). Six of them
# are scripts and live here; the seventh, §7.5 (the theme switch), is
# a unit test and runs in the `client: npm test` step above — that is
# why we read six verdicts and not seven.
#
# ⚠️ Without this step, "applied continuously" (framing §5 ⑥) would remain a wish:
# the checks would exist, and nothing would run them.
etape "client: npm run design:verifier"
(cd client && npm run design:verifier) || echec "client: npm run design:verifier"  # policy: allow-fr - npm script name

etape "proto: npm test"
(cd proto && npm test) || echec "proto: npm test"

etape "proto: npm run typecheck"
(cd proto && npm run typecheck) || echec "proto: npm run typecheck"

etape "plateforme: npm run test:sqlite"
(cd plateforme && npm run test:sqlite) || echec "plateforme: npm run test:sqlite"

etape "plateforme: npm run test:postgres"
(cd plateforme && npm run test:postgres) || echec "plateforme: npm run test:postgres"

etape "plateforme: npm run typecheck"
(cd plateforme && npm run typecheck) || echec "plateforme: npm run typecheck"

echo
if [ ${#echecs[@]} -eq 0 ]; then
    echo "All 10 steps passed."
    exit 0
fi

echo "══ ${#echecs[@]} step(s) out of 10 FAILED ══" >&2
for e in "${echecs[@]}"; do
    echo "  - $e" >&2
done
exit 1
