import { loadEngine } from "../src/engine.ts";
import { createAuthor } from "../src/author.ts";
const author = createAuthor(await loadEngine());
self.onmessage = ({ data }) => {
  const start = performance.now();
  try {
    let paintMs = 0,
      summary;
    if (data.document) {
      summary = author.render(data.document, data.width, data.mixer);
      paintMs = performance.now() - start;
    }
    const image = author.view(data.view);
    self.postMessage({ id: data.id, image, summary, paintMs, viewMs: performance.now() - start - paintMs }, [
      image.data.buffer,
    ]);
  } catch (e) {
    self.postMessage({ id: data.id, error: String(e) });
  }
};
self.postMessage({ ready: true, catalog: author.catalog() });
