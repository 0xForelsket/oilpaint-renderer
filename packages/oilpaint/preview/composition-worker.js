import { loadEngine } from "../src/engine.ts";
import { createAuthor } from "../src/author.ts";
const e = await loadEngine(),
  a = createAuthor(e);
self.onmessage = ({ data }) => {
  try {
    const t = performance.now();
    if (data.op === "plan") {
      const p = e.plan(data.scene, { width: 256, seed: 1907 });
      self.postMessage({
        id: data.id,
        requestedAt: data.requestedAt,
        op: "plan",
        document: p.document,
        report: p.report,
        ms: performance.now() - t,
        region: data.region,
      });
      return;
    }
    let summary,
      paintMs = 0;
    if (data.document) {
      summary = a.render(data.document, data.width);
      paintMs = performance.now() - t;
    }
    const image = a.view(data.view);
    self.postMessage(
      {
        id: data.id,
        requestedAt: data.requestedAt,
        op: "render",
        image,
        summary,
        paintMs,
        viewMs: performance.now() - t - paintMs,
      },
      [image.data.buffer],
    );
  } catch (e) {
    self.postMessage({ id: data.id, error: String(e) });
  }
};
self.postMessage({ ready: true, catalog: a.catalog(), engine: e.version });
