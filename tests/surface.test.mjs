import assert from "node:assert/strict";
import { execSync } from "node:child_process";
import { readFile, stat } from "node:fs/promises";

const json = async (path) => JSON.parse(await readFile(path, "utf8"));
const text = (path) => readFile(path, "utf8");

const manifest = await json(".codex-plugin/plugin.json");
const mcp = await json(".mcp.json");
const pkg = await json("package.json");
const readme = await text("README.md");
const agents = await text("AGENTS.md");
const skill = await text("skills/canmore/SKILL.md");
const server = await text("server/src/main.rs");

assert.equal(manifest.name, "codex-canmore");
assert.equal(manifest.version, "1.0.0");
assert.match(manifest.interface.shortDescription, /visual medium/i);
assert.ok(manifest.mcpServers);
assert.ok(manifest.skills);

assert.deepEqual(mcp.mcpServers["codex-canmore"].command, "./bin/canmored.exe");
assert.deepEqual(mcp.mcpServers["codex-canmore"].args, ["mcp"]);
assert.doesNotMatch(JSON.stringify(mcp), /cargo|pwsh|powershell/i);

for (const tool of [
  "canmore_medium_recipe",
  "canmore_medium_create",
  "canmore_medium_event",
  "canmore_medium_read",
  "canmore_medium_promote",
  "canmore_image_asset_register",
]) {
  assert.match(skill, new RegExp(tool));
  assert.match(server, new RegExp(tool));
}

assert.match(readme, /medium layer/i);
assert.match(readme, /built-in `image_gen`/);
assert.match(agents, /host-provided built-in `image_gen`/);

assert.equal(pkg.scripts.build, "pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/build.ps1");
assert.equal(pkg.scripts.pretest, "npm run build");
assert.equal(pkg.scripts.test, "npm run test:rust && npm run test:surface");
assert.match(pkg.scripts["test:rust"], /--locked/);
assert.equal(pkg.scripts.prepack, "npm run build");

const binary = await stat("bin/canmored.exe");
assert.ok(binary.size > 0);

const pack = JSON.parse(
  execSync("npm pack --dry-run --json --ignore-scripts", {
    encoding: "utf8",
  }),
);
const packedFiles = pack[0].files.map((file) => file.path);
assert.ok(packedFiles.includes("bin/canmored.exe"));
assert.ok(packedFiles.includes("server/Cargo.lock"));
assert.ok(packedFiles.includes("scripts/build.ps1"));
assert.ok(packedFiles.includes("tests/surface.test.mjs"));
