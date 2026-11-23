// Bundles and runs tests/hand_formats.test.ts (no test framework needed).
import { build } from "esbuild";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(mkdtempSync(join(tmpdir(), "hh-")), "t.mjs");
await build({ entryPoints: [join(root, process.argv[2] ?? "tests/hand_formats.test.ts")], bundle: true, platform: "node", format: "esm", outfile: out, logLevel: "error" });
await import(pathToFileURL(out).href);
