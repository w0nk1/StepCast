import { readFileSync, writeFileSync } from "fs";

const pkg = JSON.parse(readFileSync("package.json", "utf8"));
const cargoPath = "src-tauri/Cargo.toml";
const cargo = readFileSync(cargoPath, "utf8");
const updated = cargo.replace(
  /^(version\s*=\s*)"[^"]*"/m,
  `$1"${pkg.version}"`,
);
writeFileSync(cargoPath, updated);
console.log(`Synced Cargo.toml to ${pkg.version}`);
