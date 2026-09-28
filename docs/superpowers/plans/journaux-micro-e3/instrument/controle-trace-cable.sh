#!/usr/bin/env bash
# The acceptance check of deliverable ③ (block E3): does the cable trace carry
# `occupation_ms` and `famines`?
#
# ✅ **PLAYED, AND GREEN** — 2 runs, `agent-1.log` and `agent-2.log`: **166
# lines, 166 carrying `occupation_ms=`, 166 carrying `famines=`**, in both.
# ❌ *This header said "THIS CHECK WAS NOT PLAYED […] filed READY, not
# GREEN", the VM then being held by a concurrent workstream. It was given back
# the same day.*
#
# 🔵 **AND IT DID NOT EVEN PARSE** when it was filed "ready": see the
# `journal=` line below. An acceptance check is EXECUTED before being
# prescribed, and this one had not been.
#
# ⚠️ **The plan's red run R6 is flagged in advance as the weakest**, and
# playing it on the host would be a tautology: `windows_micro.rs` is
# `#[cfg(windows)]`, no host test can run it, and "the field is
# in the source" is not "the field comes out in the log". The only red run
# that counts is removing the field, rebuilding, relaunching, and seeing this
# `grep` fall back to zero.
#
# Usage : controle-trace-cable.sh <agent.log>

set -uo pipefail
# ❌ This line carried `${1:?journal d'agent}` — the apostrophe OPENS a
# quote there, because bash PARSES the word of a `${par:?word}` expansion even inside
# double quotes: the script did NOT PARSE ("unexpected EOF while
# looking for matching `''", line 37, that is, thirty lines
# further down). 🔵 **It had been filed "READY, not GREEN" — and it was not even
# that.** It is the repository's doctrine taken literally: an acceptance check
# must be EXECUTED before being prescribed, and this one had not been.
journal="${1:?chemin du journal d agent attendu}"

# 🔴 `grep -a` MANDATORY: a log with a tail of NUL bytes is classified
# "binary", and `grep` then returns an EMPTY OUTPUT — not a zero, and the two
# read the same (a trap paid for in D10).
plat="$(mktemp)"
sed 's/\x1b\[[0-9;]*m//g' "$journal" > "$plat"

lignes="$(grep -ac 'micro ecrit sur le cable' "$plat" || true)"
occ="$(grep -a 'micro ecrit sur le cable' "$plat" | grep -ac 'occupation_ms=' || true)"
fam="$(grep -a 'micro ecrit sur le cable' "$plat" | grep -ac 'famines=' || true)"

echo "lignes 'micro ecrit sur le cable' : ${lignes}"
echo "  dont portant occupation_ms=     : ${occ}"
echo "  dont portant famines=           : ${fam}"
rm -f "$plat"

# ⚠️ `lignes = 0` IS NOT A VERDICT: it is a measurement NOT TAKEN. The trace
# is periodic (`PERIODE_TRACE`), and a mic never turned on emits none.
if [ "$lignes" -eq 0 ]; then
    echo "VERDICT : NON MESURABLE — aucune trace de câble dans ce journal."
    exit 2
fi
if [ "$occ" -eq "$lignes" ] && [ "$fam" -eq "$lignes" ]; then
    echo "VERDICT : VERT — les deux champs sont sur TOUTES les lignes."
    exit 0
fi
echo "VERDICT : ROUGE — un champ manque sur au moins une ligne."
exit 1
