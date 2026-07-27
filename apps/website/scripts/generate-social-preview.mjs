import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

const websiteRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const sourcePath = join(websiteRoot, "public", "og-chaft-v2.png");
const outputPath = join(websiteRoot, "public", "og-chaft-v3.png");

const overlay = Buffer.from(`
  <svg width="1200" height="630" viewBox="0 0 1200 630"
       xmlns="http://www.w3.org/2000/svg">
    <rect x="48" y="165" width="610" height="315" rx="4" fill="#fbf2de"/>
    <g stroke="#d9cfba" stroke-width="1" opacity="0.32">
      <path d="M76 165V480 M114 165V480 M152 165V480 M190 165V480
               M228 165V480 M266 165V480 M304 165V480 M342 165V480
               M380 165V480 M418 165V480 M456 165V480 M494 165V480
               M532 165V480 M570 165V480 M608 165V480 M646 165V480"/>
      <path d="M48 190H658 M48 228H658 M48 266H658 M48 304H658
               M48 342H658 M48 380H658 M48 418H658 M48 456H658"/>
    </g>
    <circle cx="82" cy="210" r="6" fill="#d8563f"/>
    <text x="102" y="217" fill="#17211f"
          font-family="Arial, Helvetica, sans-serif" font-size="18"
          font-weight="700" letter-spacing="1.4">OPEN SOURCE · CANARY</text>
    <text x="77" y="306" fill="#17211f"
          font-family="Arial, Helvetica, sans-serif" font-size="58"
          font-weight="500" letter-spacing="-2">Team chat that runs</text>
    <text x="77" y="378" fill="#d8563f"
          font-family="Georgia, 'Times New Roman', serif" font-size="66"
          font-style="italic" letter-spacing="-2">on your devices.</text>
    <text x="79" y="440" fill="#34403c"
          font-family="Arial, Helvetica, sans-serif" font-size="24"
          font-weight="400">On-device history · Encrypted content · Direct sync</text>
  </svg>
`);

await sharp(sourcePath)
  .composite([{ input: overlay, left: 0, top: 0 }])
  .png({ compressionLevel: 9 })
  .toFile(outputPath);

process.stdout.write(`generated ${outputPath}\n`);
