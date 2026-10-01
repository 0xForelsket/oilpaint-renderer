import fs from "node:fs";
import { chromium } from "playwright";
const out = "docs/reports/orange-study";
const src = "https://commons.wikimedia.org/wiki/File:Single_Orange_%28Fruit%29.jpg";
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><title>One orange · frozen brush study</title><style>*{box-sizing:border-box}body{margin:0;padding:26px;width:840px;background:#f3f0e8;color:#36392f;font:15px Arial}h1{font-size:25px;margin:0 0 9px}p{line-height:1.45;margin:7px 0 16px}.pair{display:grid;grid-template-columns:384px 384px;gap:20px}figure{margin:0}img{display:block;width:384px;height:256px}figcaption{font-size:14px;padding-top:9px}footer{font-size:12px;color:#555b50;line-height:1.5;border-top:1px solid #c9c9bb;margin-top:20px;padding-top:12px}a{color:#53663c}</style><h1>One orange · frozen brush study</h1><p>One rounded form, a single dominant light above/front-left, and selective thick paint.<br>15 authored strokes · engine dev.5 and seven-preset catalog unchanged.</p><div class="pair"><figure><img src="reference.jpg"><figcaption>Reference photograph · Augustus Binu</figcaption></figure><figure><img src="lit-384.png"><figcaption>Painted study · actual 384 px renderer output</figcaption></figure></div><p style="margin-top:19px">Judge the whole form at this size first. The internal joins are quieter than the earlier lobe study, but the right-hand transition and cast shadow remain more graphic than the reference.</p><footer>Reference: <a href="${src}">Single Orange (Fruit)</a>, Augustus Binu, <a href="https://creativecommons.org/licenses/by-sa/3.0/">CC BY-SA 3.0</a>. Photo unchanged, displayed smaller. This painted interpretation and comparison are offered under the same license; the renderer/source-code license is unchanged. No blur, sharpening or image texture was applied to the painted output.</footer></html>`;
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
