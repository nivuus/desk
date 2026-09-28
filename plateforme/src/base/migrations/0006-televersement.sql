-- The upload of an installer, and the installation that follows from it.
--
-- ⚠️ THIS FILE CARRIES NEITHER APOSTROPHE NOR DOUBLE QUOTE, including in its
-- comments: `rendreMarqueurs` (base/pilote.ts) REFUSES any SQL that
-- carries one, and the migration could not be played. It is the convention of its
-- five neighbours, and it explains the contraction-free prose that follows.
--
-- 🔴 WHY ALL THE COLUMNS ARE BORN HERE. An ALTER TABLE ADD COLUMN NOT
-- NULL without a DEFAULT is REFUSED by SQLite as soon as the table carries a row, and
-- a literal DEFAULT is impossible (see above). Any mandatory
-- column a LATER sub-block wanted to add would therefore be
-- impossible to add from the first upload: they are all born now.
--
-- 🔴 THE FOREIGN KEYS ARE BORN WITH THEIR TABLES, AND WITHOUT ON DELETE. SQLite
-- cannot add a constraint through ALTER TABLE: a foreign key is born
-- with its table or never exists (legacy item no. 2 of sub-block P1). And they are
-- ENFORCED on both sides -- pilote-sqlite.ts sets PRAGMA foreign_keys = ON.
--
-- ⚠️ The ABSENCE of ON DELETE is the choice of `application.vm_id`, carried over: an
-- orphan row can be inserted NOWHERE, and deleting an upload
-- that an installation references is REFUSED. It is intended -- the history of an
-- installation must stay readable, and the CHUNKS on disk, for their part, are
-- swept elsewhere.
--
-- The timestamps are BIGINT and not INTEGER: INTEGER is 4 bytes on
-- Postgres, where a Date.now() overflows. They all carry the `_a` convention,
-- without which the static lint of sous-ensemble.test.ts cannot
-- see them.

-- A file dropped by a user, cut into chunks on the disk of the
-- service. The chunks are NEVER assembled: sealing is a
-- stream pass that recomputes the SHA-256, and resumption is a directory
-- LISTING -- never a bookkeeping that could diverge from the disk.
CREATE TABLE televersement (
    id              TEXT PRIMARY KEY,
    -- The owner. Every route checks this column and returns the SAME refusal,
    -- indistinguishable from an unknown resource: telling the two apart would be an
    -- enumeration oracle, and the owner of the repository settled this point
    -- for sub-block G1.
    utilisateur_id  TEXT NOT NULL REFERENCES utilisateur(id),
    -- The name as the BROWSER announces it. It is SANITISED on the agent side before
    -- becoming a path -- never here, where it is only data.
    nom             TEXT NOT NULL,
    taille          BIGINT NOT NULL,
    -- The hash of the WHOLE file, announced at creation by the browser
    -- and RECOMPUTED at sealing. A single value, comparable everywhere,
    -- including by a human with a sha256sum.
    sha256          TEXT NOT NULL,
    -- Frozen at creation: the split must not change under the chunks
    -- already dropped.
    taille_tranche  BIGINT NOT NULL,
    cree_a          BIGINT NOT NULL,
    -- NULL until the sealing has taken place. An unsealed upload
    -- is NEVER served to an agent: it would receive a partial file whose
    -- hash would fail, and a refusal at the right place is better than a
    -- refusal at the right time.
    scelle_a        BIGINT NULL
);

CREATE INDEX televersement_par_utilisateur ON televersement(utilisateur_id);

-- A request to install, on a named VM, a sealed upload.
CREATE TABLE installation (
    id               TEXT PRIMARY KEY,
    vm_id            TEXT NOT NULL REFERENCES vm(id),
    televersement_id TEXT NOT NULL REFERENCES televersement(id),
    demandee_a       BIGINT NOT NULL,
    -- en_attente | en_cours | terminee. The platform stops RE-EMITTING
    -- the order as soon as it is no longer en_attente: it is the first of the two
    -- belts against a double execution, the second being the marker on
    -- the disk of the VM.
    etat             TEXT NOT NULL,
    -- transfert | execution | reconciliation, or the empty string as long as nothing
    -- has started. ⚠️ `empreinte` is NOT a phase of this channel: it takes
    -- place in the browser, before this service has a row to
    -- write.
    phase            TEXT NOT NULL,
    octets_faits     BIGINT NOT NULL,
    -- Is zero in the `execution` phase, where there is nothing to total: an
    -- installer publishes no percentage, and inventing one would be lying.
    octets_total     BIGINT NOT NULL,
    ecoule_ms        BIGINT NOT NULL,
    -- 🔴 NULL means THAT THE CODE COULD NOT BE COLLECTED, and that is a fact
    -- different from any integer code. A -1 sentinel would conflate them. It is
    -- REPORTED, never interpreted: msiexec returns 3010 for a success that
    -- requests a restart, and many installers return 0 after a
    -- cancellation.
    code_sortie      INTEGER NULL,
    -- reussie | sans-effet | issue-inconnue | refusee. NULL as long as
    -- the installation is not finished.
    issue            TEXT NULL,
    -- The reason for a refusal, and NULL otherwise.
    motif            TEXT NULL,
    -- The TAIL of the installer log, bounded. ⚠️ An empty log is the
    -- NORMAL case: most Windows installers are graphical and
    -- write nothing to the standard streams.
    journal          TEXT NULL,
    journal_tronque  INTEGER NOT NULL,
    terminee_a       BIGINT NULL,
    maj_a            BIGINT NOT NULL
);

-- It is the query of the re-emission at enrolment: the installations
-- en_attente for a given VM.
CREATE INDEX installation_par_vm_et_etat ON installation(vm_id, etat);
