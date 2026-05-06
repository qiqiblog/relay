// Production bundle via Bun's built-in bundler.
// Run with: bun run build.ts
import tailwind from "bun-plugin-tailwind";
import { rm } from "node:fs/promises";

await rm("./dist", { recursive: true, force: true });

const result = await Bun.build({
  entrypoints: ["./src/index.html"],
  outdir: "./dist",
  minify: true,
  target: "browser",
  // Absolute root so deep-link refresh (e.g. /nodes/foo) still resolves
  // chunk URLs to /chunk-xxx.js, not /nodes/chunk-xxx.js.
  publicPath: "/",
  plugins: [tailwind],
  sourcemap: "linked",
});

if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}

for (const out of result.outputs) {
  console.log(`  ${out.path}  (${(out.size / 1024).toFixed(2)} kB)`);
}
console.log("✓ build complete → ./dist");
