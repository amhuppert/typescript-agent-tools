// Where harness tests keep what they write beside the checkout rather than
// in a project of their own: the native observations they record for
// inspection and the scratch directories they create. It is this checkout's
// `target/harness`, which Git ignores, and it exists once `scratchPath`
// returns, so that no test depends on another, or on the environment, having
// created it.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const directory = fileURLToPath(new URL('../target/harness/', import.meta.url));

/** The path of `segments` under the harness's scratch directory. */
export function scratchPath(...segments) {
  fs.mkdirSync(directory, { recursive: true });
  return path.join(directory, ...segments);
}
