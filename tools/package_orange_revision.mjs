import fs from "node:fs";
import { chromium } from "playwright";
const out = "docs/reports/orange-study-2";
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><title>Orange revision · contact and turning planes</title><style>*{box-sizing:border-box}body{margin:0;padding:26px;width:840px;background:#f3f0e8;color:#36392f;font:15px Arial}h1{font-size:25px;margin:0 0 9px}p{line-height:1.45;margin:7px 0 16px}.pair{display:grid;grid-template-columns:384px 384px;gap:20px}figure{margin:0}img{display:block;width:384px;height:256px}figcaption{font-size:14px;padding-top:9px}footer{font-size:12px;color:#555b50;line-height:1.5;border-top:1px solid #c9c9bb;margin-top:20px;padding-top:12px}a{color:#53663c}</style><h1>The same orange · contact, turn and light</h1><p>Same renderer, presets, lighting and underpainting paths.<br>Additional thin glazes concentrate the contact shadow and connect the painted values.</p><div class="pair"><figure><img src="../orange-study/lit-384.png"><figcaption>Approved form-coherence study · 15 strokes</figcaption></figure><figure><img src="lit-384.png"><figcaption>Painting revision · 28 strokes</figcaption></figure></div><p style="margin-top:19px">A darker contact with a softer outward fade; a broader right-hand turn; a quieter light-facing area with a smaller accent. Both paintings are shown at their actual 384 px rendering size.</p><footer>Previous painting and source files are preserved. The revised base color is slightly darker; its geometry and brush settings are unchanged. Most added marks are thin shadow glazes. No image blur or other post-render paint effects were used.<br>Based on <a href="https://commons.wikimedia.org/wiki/File:Single_Orange_%28Fruit%29.jpg">Single Orange (Fruit), Augustus Binu</a>, <a href="https://creativecommons.org/licenses/by-sa/3.0/">CC BY-SA 3.0</a>. Painting revisions and comparisons carry the same license; source-code licensing is unchanged.</footer></html>`;
fs.writeFileSync(out + "/index.html", html);
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 840, height: 500 }, deviceScaleFactor: 1 });
  await page.goto("http://127.0.0.1:4173/" + out + "/index.html");
  await page.screenshot({ path: out + "/comparison.png", fullPage: true });
  console.log(out + "/comparison.png");
} finally {
  await browser.close();
}
