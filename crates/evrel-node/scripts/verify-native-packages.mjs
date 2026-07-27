import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import { parseTriple } from "@napi-rs/cli";

const packageRoot = path.resolve(import.meta.dirname, "..");
const rootPackage = JSON.parse(await readFile(path.join(packageRoot, "package.json"), "utf8"));
const npmDirectory = path.join(packageRoot, "npm");
const configuredTargets = rootPackage.napi?.targets;

if (!Array.isArray(configuredTargets) || configuredTargets.length === 0) {
  fail("root package contains no napi.targets");
}

const expectedSuffixes = configuredTargets.map((target) => parseTriple(target).platformArchABI);
const expectedOptionalDependencies = Object.fromEntries(
  expectedSuffixes.map((suffix) => [
    `@evrel/compiler-binding-${suffix}`,
    rootPackage.version,
  ]),
);
const packageDirectories = (await readdir(npmDirectory, { withFileTypes: true }))
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();

assertEqual(packageDirectories, [...expectedSuffixes].sort(), "generated platform directories");
assertEqual(
  rootPackage.optionalDependencies,
  expectedOptionalDependencies,
  "root optional dependencies",
);

for (const suffix of expectedSuffixes) {
  const directory = path.join(npmDirectory, suffix);
  const packageJson = JSON.parse(await readFile(path.join(directory, "package.json"), "utf8"));
  const files = (await readdir(directory)).sort();
  const nativeFiles = files.filter((file) => file.endsWith(".node"));
  const expectedBinary = `evrel.${suffix}.node`;

  if (packageJson.name !== `@evrel/compiler-binding-${suffix}`) {
    fail(`${suffix} has unexpected package name ${JSON.stringify(packageJson.name)}`);
  }

  if (packageJson.version !== rootPackage.version) {
    fail(`${suffix} version ${packageJson.version} does not match root ${rootPackage.version}`);
  }

  assertEqual(nativeFiles, [expectedBinary], `${suffix} native artifacts`);

  if ((await stat(path.join(directory, expectedBinary))).size === 0) {
    fail(`${suffix} native artifact is empty`);
  }
}

process.stdout.write(`Verified ${expectedSuffixes.length} Evrel native packages.\n`);

function assertEqual(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} mismatch\nexpected: ${JSON.stringify(expected)}\nactual:   ${JSON.stringify(actual)}`);
  }
}

function fail(message) {
  process.stderr.write(`Native package verification failed: ${message}\n`);
  process.exit(1);
}
