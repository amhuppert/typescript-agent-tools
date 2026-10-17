# Isolated Command Center baseline

This is a new baseline at `aa484f422f00dfb64967d2008fac3c4f9a92538e`,
not a reconstruction of the historical `92fe6ab` generated inputs or metadata.
The independent checkout is identified in `snapshot-provenance.json`.
Tracked files come from the exact Git commit. The unrelated dirty model catalog
in the live checkout was excluded and was not modified.

The dependency tree was copied with macOS copy-on-write file cloning, preserving
internal symlinks, including dangling links. All 68,816 file/directory/link rows
matched before/after the copy, and every copied regular file has an independent
inode. The complete dependency inventory and its hash are recorded in the
provenance; it remains beside the isolated snapshot under `.cc/temp` rather than
adding 1.52 GB of installed packages to this repository.

Six generated source inputs and `.next/package.json` were copied with exact
before/after byte hashes. No install, build or application generator was run.
The configured `.next/dev/types` and `.next/dev/dev/types` directories were absent.
The capture boundary is the resulting isolated filesystem, not an assertion
that several reads from the live checkout constituted an atomic snapshot.

Native program discovery and query captures are pending. The new manifest will
record ordered source identities separately from its sorted hash inventory.
Each completed query capture must join its own ordered source/provenance and
compiler-option observation to that manifest and verify every source hash.
Additional consumed metadata is permitted and retained by the shared capture
owner. `captureComplete` establishes this input contract, not product admission,
Q3 normalization, semantic latency or the final corpus acceptance gate.

Historical manifests and observations one directory above remain unchanged.


Completed current captures are under [single-threaded-native](single-threaded-native/README.md).
The root manifest and unfinished member artifact preserve the earlier parallel
profile. They were superseded after default-library external provenance varied
on unchanged inputs. No historical record was finalized using the new config.
