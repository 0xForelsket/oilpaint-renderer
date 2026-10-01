import fs from "node:fs";
import { chromium } from "playwright";
const out = "docs/reports/brush-review-3";
const panel = (id, title, note) =>
  `<figure><h2>${title}</h2><p>${note}</p><img src="data:image/png;base64,${fs.readFileSync(`${out}/${id}.png`).toString("base64")}"></figure>`;
const panels = [
  ["scumble-flat-before", "Scumble / iteration 2", "Same path, light and pigment; flat ground"],
  ["scumble-flat-studio", "Scumble / iteration 3", "Brush-scale clusters and short dragged connections"],
  ["scumble-relief-studio", "Scumble / raised dry surface", "Same cluster stroke responds to actual deposited relief"],
  ["impasto-before-raking", "Impasto / iteration 2", "Controlled raking illumination"],
  ["impasto-raking", "Impasto / iteration 3", "Smoother body between selected ridges; hgain remains 1.5"],
  [
    "pressure-studio",
    "Loaded pressure / 0.15 · 0.40 · 1.00",
    "Narrower/thinner contact at low pressure; not merely a wash",
  ],
  [
    "dry-density-studio",
    "Dry drag / sparse above, denser below",
    "Both over a dry solid underlayer; same preset, different controls",
  ],
  [
    "rounded-turn-studio",
    "Rounded contact / short turns",
    "Curved paths with changing pressure, not axial brush-roll simulation",
  ],
  ["fine-detail-studio", "Fine detail / practical marks", "Stem, branch, contour, tiny highlights and crossing lines"],
  ["pickup-tail-before", "Wet mixing / iteration 2", "Same long stroke after the blue crossing"],
  [
    "pickup-tail-studio",
    "Wet mixing / iteration 3",
    "Bundles gather and spread; individual contamination fades downstream",
  ],
  ["overlap-studio", "Varied overlap / several brushes", "Different directions, lengths, spacing and pressure"],
];
const html = `<!doctype html><html><meta charset="utf-8"><style>*{box-sizing:border-box}body{margin:0;width:3000px;padding:40px;background:#202522;color:#ece8dd;font:24px Arial}h1{font-size:48px;margin:0 0 13px}h2{font-size:25px;margin:0 0 5px}h3{font-size:30px;margin:24px 0 14px}p{margin:4px 0 14px;color:#cad2c2;line-height:1.4}.grid{display:grid;grid-template-columns:repeat(3,960px);gap:25px 20px}figure{margin:0}figure p{font-size:19px;min-height:25px}img{display:block;width:960px}.last{display:grid;grid-template-columns:960px 1fr;gap:30px}footer{font-size:21px;line-height:1.5;margin-top:22px}</style><h1>Brush combinations · iteration 3</h1><p>Engine dev.5 / author & catalog schema 2 · seven existing candidates · 960 px original renderer panels · 2026-10-02</p><p>Review clustered contact, smooth versus raised body, changing bristle bundles, loaded pressure and form-following overlap. Same-path before/after panels isolate engine/default changes; the form study is deliberately a different painting strategy.</p><div class="grid">${panels.map((p) => panel(...p)).join("")}</div><h3>Form-following placement / all seven brushes used for distinct roles</h3><div class="last">${panel("form-following-studio", "Leaf · rounded fruit · rough stone", "Directional leaf strokes, curved fruit turns and broad stone planes")}<div><p>The earlier stacked-rib studies are preserved in the iteration-two gallery. Here an overlapping thin foundation closes the silhouettes, then deliberately varied paths and pressure describe each form.</p><p>Loaded flat supplies planes; Rounded dab turns around the fruit; Dry drag and Scumble catch the stone; Fine detail supplies veins and highlights; Wet mixing joins a transition; Impasto accents the light.</p><p>This is a controlled painting-strategy probe, not a finished realistic painting. The marks still read as constructed, and hard overlaps remain visible.</p></div></div><footer>Real Rust paint deposition and height-based relighting; no image texture overlays. Scumble/tooth and bundle grouping remain approximations. No axial brush roll, cast shadows or layered paint film is claimed. Presets await artist review.<br>Full source documents, before/after, unlit/height/raking images and runtime evidence: docs/reports/brush-review-3/index.html.</footer></html>`;
fs.mkdirSync("out/brush-iteration-3", { recursive: true });
fs.writeFileSync("out/brush-iteration-3/compact.html", html);
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 3000, height: 900 }, deviceScaleFactor: 1 });
  await page.setContent(html, { waitUntil: "load" });
  await page.screenshot({ path: out + "/review.png", fullPage: true });
  console.log(
    JSON.stringify({
      pixels: await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.scrollHeight]),
      bytes: fs.statSync(out + "/review.png").size,
    }),
  );
} finally {
  await browser.close();
}
