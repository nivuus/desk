// Durable "temporary file then rename" writes, for the two on-disk stores.
//
// 🔴 A RENAME IS ATOMIC, NOT DURABLE. Without `fsync`, after a host freeze or a
// power cut the new name can be on disk while the data is not: the file then
// exists under its final name with 0 bytes. ext4's `auto_da_alloc` only covers
// renaming over an EXISTING file, and both stores always write a NEW name (a
// chunk rank, an icon digest). This host has frozen nine times.
//
// The order is the one POSIX requires: flush the data, rename, then flush the
// DIRECTORY, which holds the new name. A new directory must likewise be flushed
// in its parent.
//
// ⚠️ `write-file-atomic` (npm) was considered: it flushes the file but not the
// parent directory, and it takes a whole buffer, while the chunk store streams
// under a byte cap. These three calls are the whole mechanism.

import { closeSync, fsyncSync, openSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';

/// Flushes a file's data, or a directory's entries (the names created or
/// renamed in it). `fsync` acts on the inode: a descriptor opened for this
/// purpose flushes what another descriptor wrote.
export function synchroniser(chemin: string): void {
    const fd = openSync(chemin, 'r');
    try {
        fsyncSync(fd);
    } finally {
        closeSync(fd);
    }
}

/// Writes `octets` under `cible` so that, after a crash, `cible` is either
/// absent or complete — never present and short.
///
/// The temporary name carries a random suffix: two concurrent writers of the
/// same target never share a temporary file.
export function ecrireFichierDurable(cible: string, octets: Uint8Array): void {
    const provisoire = `${cible}.${process.pid}.${Math.random().toString(36).slice(2)}.part`;
    try {
        const fd = openSync(provisoire, 'w');
        try {
            writeFileSync(fd, octets);
            fsyncSync(fd);
        } finally {
            closeSync(fd);
        }
        renameSync(provisoire, cible);
    } catch (error) {
        rmSync(provisoire, { force: true });
        throw error;
    }
    synchroniser(dirname(cible));
}
