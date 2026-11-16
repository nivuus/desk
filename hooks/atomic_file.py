#!/usr/bin/env python3
"""Replacing a secret-bearing file atomically, with an exact final mode.

Shared by the three writers of the files that hold the product's secrets:
`installed_files.write_env` and `installed_files.write_turnserver_conf`
(`install`), and `env_file.add_env_variables` (`activate`).

Rewriting such a file IN PLACE (`O_TRUNC` then write, or `write_text`) has
two defects:

- a crash or a power loss between the truncation and the flush leaves an
  empty or partial file. For `desk.env` that is not a cosmetic loss: the
  next install finds no persisted secret, draws new ones and drops the
  agent enrolment keys — the silent rotation `lire_secret_persiste` exists
  to prevent;
- the `mode` of `open(2)` with `O_CREAT` only applies to a NEW file, so an
  existing file that is more permissive keeps its mode.

`replace_file` writes a temporary file in the SAME directory (created
0600 by `tempfile.mkstemp`, so "other" never sees the content), flushes and
`fsync`s it, sets the final group and mode on the descriptor, then
`os.replace`s it over the target and `fsync`s the directory. The path
therefore always holds either the complete previous content or the complete
new content; on any failure the temporary file is removed and the error
propagates.

Standard library only, like every hook of this package: `tempfile` and
`os.replace` are the documented building blocks; the PyPI `atomicwrites`
package is deprecated by its author and would be the hooks' first
third-party dependency.
"""
import contextlib
import os
import pathlib
import stat
import tempfile

# The "other" permission bits: a secret-bearing file never carries any.
_OTHER_BITS = stat.S_IRWXO


def replace_file(path: pathlib.Path, content: str, mode: int,
                 gid: int | None = None) -> None:
    """Atomically replaces `path` with `content` (UTF-8), final mode `mode`.

    `gid`, when given, becomes the group of the new file (set before the
    mode, on the descriptor). `mode` must not grant anything to "other".
    """
    if mode & _OTHER_BITS:
        raise ValueError(f"refusing mode {oct(mode)} for {path}: it grants "
                         "access to other users")
    directory = path.parent
    directory.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(dir=directory,
                                             prefix=f".{path.name}.",
                                             suffix=".tmp")
    try:
        try:
            data = memoryview(content.encode("utf-8"))
            while data:
                written = os.write(descriptor, data)
                data = data[written:]
            if gid is not None:
                os.fchown(descriptor, -1, gid)
            os.fchmod(descriptor, mode)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        os.replace(temporary, path)
    except BaseException:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(temporary)
        raise
    fsync_directory(directory)


def fsync_directory(directory: pathlib.Path) -> None:
    """Makes the rename itself durable: the directory entry is data too."""
    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
