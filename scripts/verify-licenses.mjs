import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const invalidLicense = (license) =>
  !license || /^(unknown|unlicensed|none)$/i.test(license.trim());

export function buildLicenseReport(cargo, npm, cargoLock, pnpmLock) {
  const rust = cargo.packages
    .filter((pkg) => pkg.source)
    .map((pkg) => ({
      name: pkg.name,
      version: pkg.version,
      source: pkg.source,
      license: pkg.license || pkg.license_file,
    }));
  const npmPackages = Object.entries(npm).flatMap(([group, packages]) =>
    packages.map((pkg) => ({
      name: pkg.name,
      versions: pkg.versions,
      license: pkg.license || group,
    })),
  );
  const invalid = [...rust, ...npmPackages].filter((pkg) => invalidLicense(pkg.license));
  if (invalid.length) {
    throw new Error(`Missing or unknown license: ${invalid.map((pkg) => pkg.name).join(", ")}`);
  }

  const sha256 = (content) => createHash("sha256").update(content).digest("hex");
  return {
    lockfiles: {
      cargoSha256: sha256(cargoLock),
      pnpmSha256: sha256(pnpmLock),
    },
    rust: rust.sort((a, b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`)),
    npm: npmPackages.sort((a, b) => a.name.localeCompare(b.name)),
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [cargoPath, npmPath, outputPath] = process.argv.slice(2);
  if (!cargoPath || !npmPath || !outputPath) {
    throw new Error("usage: verify-licenses.mjs <cargo-metadata.json> <npm-licenses.json> <output.json>");
  }
  const report = buildLicenseReport(
    JSON.parse(readFileSync(cargoPath, "utf8")),
    JSON.parse(readFileSync(npmPath, "utf8")),
    readFileSync("Cargo.lock"),
    readFileSync("frontend/pnpm-lock.yaml"),
  );
  writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`licenses: ${report.rust.length} Rust and ${report.npm.length} npm entries verified; evidence: ${outputPath}`);
}
