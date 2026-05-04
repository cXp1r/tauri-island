import { readFileSync, mkdirSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

// 读取 SVG
const svgPath = join(root, "src/assets/logo-bw-morph.svg");
const svgBuffer = readFileSync(svgPath);

// Tauri 需要的图标尺寸
const sizes = [32, 128, 256];

// 动态 import sharp（通过 npx 安装）
const { default: sharp } = await import("sharp");

mkdirSync(join(root, "src/assets/icons"), { recursive: true });

for (const size of sizes) {
  const outPath = join(root, "src/assets/icons", `icon-${size}x${size}.png`);
  await sharp(svgBuffer)
    .resize(size, size)
    .png()
    .toFile(outPath);
  console.log(`✓ ${size}x${size} → ${outPath}`);
}

// 同时生成 icon.png (256x256) 覆盖 src-tauri/icons/icon.png
const tauriIconPath = join(root, "src-tauri/icons/icon.png");
await sharp(svgBuffer).resize(256, 256).png().toFile(tauriIconPath);
console.log(`✓ 256x256 → ${tauriIconPath} (tauri icon)`);

console.log("\n完成！");
