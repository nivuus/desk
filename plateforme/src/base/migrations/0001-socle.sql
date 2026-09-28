-- Portable subset (spec §3.2): TEXT/UUID v4 identifiers, BIGINT
-- timestamps in milliseconds ALWAYS written by the application, booleans as
-- INTEGER 0/1, no literal value.
--
-- The timestamps are BIGINT and not INTEGER, and it is MEASURED, not precautionary.
-- `INTEGER` is up to 8 bytes on SQLite and exactly 4 on Postgres:
-- on 19 August 2026, on PostgreSQL 16.15, writing a `Date.now()` into an
-- INTEGER column returned
--     value "1787136773742" is out of range for type integer
-- and the service could not apply its OWN migrations. SQLite
-- accepted it without a word -- it is a fourth blind spot of the
-- lint / double pass pair, that of the CHOICE OF VALUES.
--
-- Convention that makes the rule checkable: every timestamp column carries
-- a name ending in `_a` (cree_a, vue_a, ouverte_a, fermee_a, applique_a), and the lint
-- of `sous-ensemble.test.ts` refuses an `_a INTEGER`.

CREATE TABLE schema_migration (
    version    INTEGER PRIMARY KEY,
    applique_a BIGINT NOT NULL
);

-- `utilisateur` is created by P1 and STAYS EMPTY: P2 gives it its
-- behaviour, not its table.
--
-- ✅ P2 DID IT (19 August 2026): the table is no longer empty. `depot/utilisateur.ts`
-- writes it and reads it back, `admin/creer-utilisateur.ts` creates an account in it from the
-- command line, and `identite/mot-de-passe.ts` fills `empreinte_mdp` in the
-- `scrypt$N$r$p$sel$empreinte` format. **The table itself did NOT move** —
-- it is exactly what the sentence above promised, and it is what
-- made D3's constraint pay off. `0002-identite.sql` leans on it.
--

-- Why it cannot wait: `vm.utilisateur_id` references it, and
-- SQLite cannot add a constraint through ALTER TABLE -- measured on
-- 19 August 2026 on SQLite 3.50.4: near "CONSTRAINT": syntax error. A foreign
-- key is born with its table or never exists.
CREATE TABLE utilisateur (
    id            TEXT PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    empreinte_mdp TEXT NOT NULL,
    cree_a        BIGINT NOT NULL
);

CREATE TABLE vm (
    id             TEXT PRIMARY KEY,
    nom            TEXT NOT NULL,
    adresse        TEXT NOT NULL,
    utilisateur_id TEXT NULL REFERENCES utilisateur(id),
    vue_a          BIGINT NULL
);

-- PARTIAL unique index: several unassigned VMs coexist, a second
-- assignment is refused. Tested on 19 August 2026 on SQLite 3.50.4 --
-- two NULLs tolerated: OK, double assignment: REFUSED -> UNIQUE constraint
-- failed: vm.utilisateur_id. P4's criterion 2 will depend on it.
--
-- ❌ THE LAST SENTENCE ABOVE IS FALSE, AND THE SENTENCE "a second
-- assignment is refused" IS AMBIGUOUS TO THE POINT OF MISLEADING (found on
-- 20 August 2026, cross-cutting review of sub-block P4, by MEASUREMENT and not by
-- reading). What this index forbids is ONE USER HAVING TWO VMs:
-- `utilisateur_id` is unique ACROSS ROWS. It forbids NOTHING of
-- `UPDATE vm SET utilisateur_id = 'bob' WHERE id = 'v1'` when v1 already belongs to
-- alice — a VM has only one `utilisateur_id`, and OVERWRITING it violates no
-- uniqueness. The theft of a VM by a third party therefore passes, index in place.
--
-- 🔵 IT IS NOT A REASONING, IT IS A RED THAT WAS PLAYED. Log filed:
-- `docs/superpowers/plans/journaux-plateforme-p4/rouge-2a-vol-sans-clause-conditionnelle.log`.
-- With this index INTACT, it was enough to remove the clause `AND utilisateur_id IS
-- NULL` from `depot/vm.ts::attribuerSiLibre` for the theft to succeed —
-- `rows touched by the theft UPDATE: 1`, owner of v1 changed — on
-- SQLite 3.50.4 as on PostgreSQL 16.15. ONE run per engine.
--
-- What P4's criterion 2 requires therefore splits into TWO properties, with TWO
-- guards, which are NOT both here:
--   ②a "a VM is assigned only once" -> the clause `AND
--       utilisateur_id IS NULL` of the UPDATE, in `depot/vm.ts`, NOT this
--       index;
--   ②b "a user receives only one VM"  -> this index, which THROWS.
-- The same wrong attribution lives in spec §3.2 and its §4 "P4"; it is
-- annotated there at the same place and on the same date.
CREATE UNIQUE INDEX vm_un_utilisateur ON vm(utilisateur_id)
    WHERE utilisateur_id IS NOT NULL;

-- `nom_session` is ABSENT from the spec's schema (§5), and added here: without
-- it, the row written at pairing designates nothing -- it is the only
-- identifier a session has before P2 and P3.
--
-- `utilisateur_id` and `vm_id` are born NULL whereas the spec wants them
-- NOT NULL: in P1 there is neither user nor VM, and there is no honest
-- value. They will NOT be tightened later -- see the comment of
-- `utilisateur`: SQLite requires a table rebuild, which Postgres does not
-- do the same way.
--
-- ✅ P2 FILLS `utilisateur_id` (19 August 2026), and the column stays NULLABLE
-- FOR A REASON THAT IS NOT DEBT: a session paired by an `agent`
-- peer alone -- the `bureau` control session at a VM's startup -- has
-- nobody to record: the agent CLAIMS nothing, its session having to stay
-- claimable by the human client that will join it (P3 gave it an
-- identity, not an ownership). `NOT NULL`
-- would therefore be WRONG, not merely costly.
--
-- ✅ `vm_id` IS FILLED SINCE P3 (19 August 2026), and this line still said
-- "stays entirely empty: it is P3". The session name CARRIES the VM
-- (`<prefix>:bureau`): `signaling/trace.ts` cuts out the prefix, looks it up
-- in `agent_enrole` and records the identifier found.
--
-- ⚠️ IT STAYS NULLABLE, AND FOR A REASON, NOT OUT OF DEBT: two cases yield
-- `null` honestly -- a session without a prefix (local trial mode, spec
-- §10) and a prefix unknown to the database. `NOT NULL` would then refuse to write
-- a trace that is otherwise correct.
--
-- ⚠️ The fix of this line was MISSED a first time: commit
-- `254fdd5` of this same branch rewrote the six preceding lines without
-- sweeping the next two. It is the "487" wreck -- fixing a
-- claim where it was shown to us, instead of SEARCHING for it.

CREATE TABLE session (
    id             TEXT PRIMARY KEY,
    nom_session    TEXT NOT NULL,
    utilisateur_id TEXT NULL,
    vm_id          TEXT NULL,
    ouverte_a      BIGINT NOT NULL,
    fermee_a       BIGINT NULL,
    motif          TEXT NULL
);

CREATE INDEX session_ouvertes ON session(fermee_a) WHERE fermee_a IS NULL;
