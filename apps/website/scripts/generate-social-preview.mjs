import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

const websiteRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const markPath = join(
  websiteRoot,
  "..",
  "desktop-qt",
  "resources",
  "branding",
  "chaft-mark.png",
);
const outputPath = join(websiteRoot, "public", "og-chaft-v3.png");
const mark = await sharp(markPath)
  .resize(64, 64, { fit: "contain" })
  .png()
  .toBuffer();

const artwork = Buffer.from(`
  <svg width="1200" height="630" viewBox="0 0 1200 630"
       xmlns="http://www.w3.org/2000/svg">
    <defs>
      <pattern id="grid" width="28" height="28" patternUnits="userSpaceOnUse">
        <path d="M28 0H0V28" fill="none" stroke="#17211f" stroke-opacity="0.035"/>
      </pattern>
      <filter id="shadow" x="-20%" y="-20%" width="140%" height="150%">
        <feDropShadow dx="0" dy="18" stdDeviation="22" flood-color="#17211f" flood-opacity="0.18"/>
      </filter>
    </defs>

    <rect width="1200" height="630" fill="#f4f1e8"/>
    <rect width="1200" height="630" fill="url(#grid)"/>
    <circle cx="1095" cy="72" r="245" fill="#dcece3" fill-opacity="0.62"/>

    <text x="137" y="86" fill="#17211f"
          font-family="Arial, Helvetica, sans-serif" font-size="42"
          font-weight="600" letter-spacing="-1.7">chaft</text>

    <circle cx="64" cy="166" r="6" fill="#d8563f"/>
    <text x="84" y="173" fill="#077054"
          font-family="Arial, Helvetica, sans-serif" font-size="17"
          font-weight="700" letter-spacing="1.9">OPEN-SOURCE DESKTOP CHAT</text>

    <text x="58" y="255" fill="#17211f"
          font-family="Arial, Helvetica, sans-serif" font-size="55"
          font-weight="500" letter-spacing="-2.5">Team chat without a</text>
    <text x="58" y="319" fill="#17211f"
          font-family="Arial, Helvetica, sans-serif" font-size="55"
          font-weight="500" letter-spacing="-2.5">required central server.</text>

    <text x="60" y="382" fill="#34403c"
          font-family="Arial, Helvetica, sans-serif" font-size="22">
      Workspace history stays on your team's devices.
    </text>
    <text x="60" y="416" fill="#34403c"
          font-family="Arial, Helvetica, sans-serif" font-size="22">
      New activity syncs when authorized devices connect.
    </text>

    <rect x="58" y="476" width="460" height="48" rx="8" fill="#17211f"/>
    <circle cx="82" cy="500" r="6" fill="#72dbba"/>
    <text x="101" y="507" fill="#fffdf7"
          font-family="Arial, Helvetica, sans-serif" font-size="15"
          font-weight="700" letter-spacing="1.3">EARLY BUILD · WINDOWS · MACOS · LINUX</text>
    <text x="59" y="570" fill="#077054"
          font-family="Arial, Helvetica, sans-serif" font-size="17"
          font-weight="700" letter-spacing="2.2">CHAFT.AI</text>

    <g filter="url(#shadow)">
      <rect x="678" y="112" width="468" height="430" rx="16" fill="#111a18" stroke="#3a4b46"/>
      <path d="M678 128a16 16 0 0 1 16-16h436a16 16 0 0 1 16 16v38H678z" fill="#1b2a25"/>
      <circle cx="701" cy="139" r="5" fill="#e87660"/>
      <circle cx="719" cy="139" r="5" fill="#e2b661"/>
      <circle cx="737" cy="139" r="5" fill="#61b88e"/>
      <text x="892" y="145" fill="#dce5e1" text-anchor="middle"
            font-family="Arial, Helvetica, sans-serif" font-size="13">Fieldwork Studio</text>
      <circle cx="1074" cy="139" r="5" fill="#72dbba"/>
      <text x="1087" y="144" fill="#9fb0aa"
            font-family="Arial, Helvetica, sans-serif" font-size="12">Up to date</text>

      <rect x="678" y="166" width="132" height="376" fill="#17231f"/>
      <line x1="810" y1="166" x2="810" y2="542" stroke="#33443e"/>
      <rect x="695" y="188" width="34" height="34" rx="8" fill="#d8563f"/>
      <text x="712" y="211" fill="#fff" text-anchor="middle"
            font-family="Arial, Helvetica, sans-serif" font-size="16">F</text>
      <text x="741" y="201" fill="#f1f5f3"
            font-family="Arial, Helvetica, sans-serif" font-size="13" font-weight="600">Fieldwork</text>
      <text x="741" y="219" fill="#83948e"
            font-family="Arial, Helvetica, sans-serif" font-size="11">3 teammates</text>
      <rect x="694" y="241" width="101" height="34" rx="7" fill="#21302c" stroke="#34443f"/>
      <text x="706" y="263" fill="#81928c"
            font-family="Arial, Helvetica, sans-serif" font-size="11">⌕  Search</text>
      <text x="695" y="304" fill="#82938d"
            font-family="Arial, Helvetica, sans-serif" font-size="11">ROOMS</text>
      <text x="697" y="334" fill="#aebdb8"
            font-family="Arial, Helvetica, sans-serif" font-size="12">#  general</text>
      <rect x="689" y="348" width="112" height="36" rx="7" fill="#294139"/>
      <text x="697" y="371" fill="#fff"
            font-family="Arial, Helvetica, sans-serif" font-size="12">#  launch</text>
      <circle cx="782" cy="366" r="9" fill="#d8563f"/>
      <text x="782" y="370" fill="#fff" text-anchor="middle"
            font-family="Arial, Helvetica, sans-serif" font-size="10">2</text>
      <text x="697" y="410" fill="#aebdb8"
            font-family="Arial, Helvetica, sans-serif" font-size="12">#  design</text>

      <line x1="810" y1="222" x2="1146" y2="222" stroke="#33443e"/>
      <text x="830" y="194" fill="#f1f5f3"
            font-family="Arial, Helvetica, sans-serif" font-size="16" font-weight="600"># launch</text>
      <text x="830" y="213" fill="#81928c"
            font-family="Arial, Helvetica, sans-serif" font-size="11">Release planning and handoff</text>

      <line x1="834" y1="251" x2="938" y2="251" stroke="#33443e"/>
      <text x="972" y="255" fill="#71827c" text-anchor="middle"
            font-family="Arial, Helvetica, sans-serif" font-size="10">TODAY</text>
      <line x1="1008" y1="251" x2="1124" y2="251" stroke="#33443e"/>

      <rect x="832" y="276" width="30" height="30" rx="8" fill="#9a624f"/>
      <text x="847" y="296" fill="#fff" text-anchor="middle"
            font-family="Arial, Helvetica, sans-serif" font-size="12">M</text>
      <text x="876" y="286" fill="#fff"
            font-family="Arial, Helvetica, sans-serif" font-size="12" font-weight="600">Maya</text>
      <text x="914" y="286" fill="#7f918a"
            font-family="Arial, Helvetica, sans-serif" font-size="10">10:42</text>
      <text x="876" y="304" fill="#dce5e1"
            font-family="Arial, Helvetica, sans-serif" font-size="11">Can we review the launch checklist?</text>
      <rect x="876" y="316" width="125" height="26" rx="13" fill="#1b2d27" stroke="#416d5d"/>
      <text x="891" y="333" fill="#7bd8b7"
            font-family="Arial, Helvetica, sans-serif" font-size="10">↳ 2 replies · 10:47</text>

      <rect x="832" y="363" width="30" height="30" rx="8" fill="#526f7b"/>
      <text x="847" y="383" fill="#fff" text-anchor="middle"
            font-family="Arial, Helvetica, sans-serif" font-size="12">N</text>
      <text x="876" y="373" fill="#fff"
            font-family="Arial, Helvetica, sans-serif" font-size="12" font-weight="600">Nia</text>
      <text x="905" y="373" fill="#7f918a"
            font-family="Arial, Helvetica, sans-serif" font-size="10">10:45</text>
      <text x="876" y="391" fill="#dce5e1"
            font-family="Arial, Helvetica, sans-serif" font-size="11">I added the final copy and screenshots.</text>
      <rect x="876" y="405" width="230" height="54" rx="8" fill="#1c2a26" stroke="#344b44"/>
      <rect x="889" y="417" width="30" height="30" rx="7" fill="#294139"/>
      <text x="904" y="437" fill="#76d4b2" text-anchor="middle"
            font-family="Arial, Helvetica, sans-serif" font-size="12">↗</text>
      <text x="932" y="426" fill="#eef4f1"
            font-family="Arial, Helvetica, sans-serif" font-size="11" font-weight="600">launch-kit.zip</text>
      <text x="932" y="444" fill="#8fa19a"
            font-family="Arial, Helvetica, sans-serif" font-size="10">18 MB · encrypted before sync</text>

      <rect x="829" y="486" width="298" height="38" rx="8" fill="#1b2925" stroke="#3b4d47"/>
      <text x="844" y="510" fill="#82938d"
            font-family="Arial, Helvetica, sans-serif" font-size="11">Message #launch</text>
      <rect x="1097" y="493" width="24" height="24" rx="6" fill="#087054"/>
      <text x="1109" y="510" fill="#fff" text-anchor="middle"
            font-family="Arial, Helvetica, sans-serif" font-size="13">↑</text>
    </g>
  </svg>
`);

await sharp({
  create: {
    width: 1200,
    height: 630,
    channels: 4,
    background: "#f4f1e8",
  },
})
  .composite([
    { input: artwork, left: 0, top: 0 },
    { input: mark, left: 52, top: 38, blend: "over" },
  ])
  .png({ compressionLevel: 9 })
  .toFile(outputPath);

process.stdout.write(`generated ${outputPath}\n`);
