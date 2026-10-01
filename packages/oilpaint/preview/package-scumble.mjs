import fs from "node:fs";
import { chromium } from "playwright";
const out = "docs/reports/scumble-refinement";
const cases = [
  ["flat", "Flat ground · broad and narrow widths"],
  ["irregular", "Irregular dry underpainting · identical starting paint planes"],
  ["overlap", "Crossing pass · deposits catch earlier scumble and underpaint"],
];
const im = (id, label) => `<figure><img src="${id}.png"><figcaption>${label}</figcaption></figure>`;
const rows = cases
  .map(
    ([id, title]) =>
      `<section><h2>${title}</h2><div class="pair">${im(`${id}-768-before-studio`, "dev.5 · previous scumble")}${im(`${id}-768-after-studio`, "dev.6 · refined scumble")}</div></section>`,
  )
  .join("");
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><title>Scumble only · size range and broken contact</title><style>*{box-sizing:border-box}body{width:1608px;padding:26px;margin:0;background:#202522;color:#e9e5d9;font:19px Arial}h1{font-size:33px;margin:0 0 12px}h2{font-size:23px;margin:0 0 12px}p{line-height:1.45;margin:8px 0 18px}.pair{display:grid;grid-template-columns:768px 768px;gap:20px}figure{margin:0}img{display:block;width:768px;height:384px}figcaption{padding-top:9px;font-size:17px;color:#c8d1bd}section{padding-top:18px;margin-top:20px;border-top:1px solid #53634e}footer{font-size:17px;line-height:1.45;margin-top:22px}a{color:#c7daaf}</style><h1>Scumble only · mixed contact sizes</h1><p>Solid patches, medium contacts, smaller catches, ragged interruptions and short connections.<br>Same paths, pigments, pressure and light. The other six brushes and the accepted orange match dev.5 paint planes exactly.</p>${rows}<footer>Three 768 px comparisons; the original 384 px renders, unlit/height views, documents and hashes are also saved here.<br>Only scumble deposition changed. The entire catalog is unchanged. Surface catching remains a deterministic approximation; this candidate still needs visual review.<br><a href="verification.json">Runtime and preservation checks</a> · <a href="substrate.png">Shared underpainting before scumble</a></footer></html>`;
fs.writeFileSync(`${out}/index.html`, html);
const browser = await chromium.launch();
try {
  const p = await browser.newPage({ viewport: { width: 1608, height: 900 }, deviceScaleFactor: 1 });
  await p.goto("http://127.0.0.1:4173/" + out + "/index.html");
  await p.screenshot({ path: out + "/review.png", fullPage: true });
  console.log(out + "/review.png");
} finally {
  await browser.close();
}
