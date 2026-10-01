// maplibre-gl 6 runs its tile work in a module worker loaded from a file next
// to the library (plus the chunk it shares with the main thread). The bundler
// doesn't emit those, so copy them into public/ before dev and build, and the
// site map canvas points maplibre at them with setWorkerUrl.

import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const from = join(root, "node_modules", "maplibre-gl", "dist");
const to = join(root, "public", "maplibre");
mkdirSync(to, { recursive: true });
for (const f of ["maplibre-gl-worker.mjs", "maplibre-gl-shared.mjs"]) {
  copyFileSync(join(from, f), join(to, f));
}
