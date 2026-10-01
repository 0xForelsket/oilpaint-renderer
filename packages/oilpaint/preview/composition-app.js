import { loadEngine } from "../src/engine.ts";
import { createAuthor } from "../src/author.ts";
import { editRegion, replaceRegion } from "../src/composition.ts";
import { catalogStillLife } from "/scenes/catalog_still_life.ts";
const $ = (id) => document.getElementById(id);
const a = createAuthor(await loadEngine());
const worker = new Worker(new URL("./composition-worker.js", import.meta.url), {
  type: "module",
});
let catalog,
  doc,
  last,
  ready = false,
  busy = false,
  planning = true,
  queued = null,
  id = 0,
  latest = 0,
  timer;
const view = () => ({
  mode: $("view").value,
  direction: [-0.55, -0.6, 0.35],
  bump: 0.65,
  contrast: 0.25,
  specular: 0.05,
});
function draw(canvas, i) {
  canvas.width = i.width;
  canvas.height = i.height;
  const rgba = new Uint8ClampedArray(i.width * i.height * 4);
  for (let s = 0, d = 0; s < i.data.length; s += 3, d += 4) {
    rgba[d] = i.data[s];
    rgba[d + 1] = i.data[s + 1];
    rgba[d + 2] = i.data[s + 2];
    rgba[d + 3] = 255;
  }
  canvas
    .getContext("2d")
    .putImageData(new ImageData(rgba, i.width, i.height), 0, 0);
}
function controls() {
  for (const button of document.querySelectorAll(
    "button,input,select,textarea",
  ))
    button.disabled = planning;
  if (!$("region").value)
    for (const id of ["tint", "visible", "translate", "replan"])
      $(id).disabled = true;
}
function regions() {
  const current = $("region").value;
  $("region").replaceChildren(
    ...[...new Set(doc.groups.map((g) => g.region).filter(Boolean))].map(
      (r) => new Option(r, r),
    ),
  );
  $("region").value = [...$("region").options].some((o) => o.value === current)
    ? current
    : ($("region").options[0]?.value ?? "");
  select();
}
function select() {
  const g = doc.groups.filter((g) => g.region === $("region").value);
  $("visible").checked = g.every((g) => g.visible);
  const preset = g.flatMap((g) => g.strokes).find((s) => s.preset)?.preset;
  if (preset) $("preset").value = preset;
}
function enqueue(job) {
  queued = { requestedAt: performance.now(), ...job, id: ++id };
  latest = id;
  pump();
}
function pump() {
  if (!ready || busy || !queued) return;
  busy = true;
  const job = queued;
  queued = null;
  worker.postMessage(job);
}
function render(requestedAt = performance.now()) {
  if (typeof requestedAt !== "number") requestedAt = performance.now();
  clearTimeout(timer);
  timer = setTimeout(
    () =>
      enqueue({
        op: "render",
        document: doc,
        width: +$("width").value,
        view: view(),
        requestedAt,
      }),
    40,
  );
}
function plan(region) {
  try {
    clearTimeout(timer);
    const scene = JSON.parse($("source").value);
    planning = true;
    controls();
    $("status").textContent = "Planning…";
    enqueue({ op: "plan", scene, region });
  } catch (e) {
    $("error").textContent = String(e);
  }
}
function reset() {
  const s = catalogStillLife(catalog).spec;
  $("source").value = JSON.stringify(s, null, 2);
  plan();
}
function commit(next) {
  const requestedAt = performance.now();
  try {
    a.compile(next);
    doc = next;
    $("error").textContent = "";
    render(requestedAt);
  } catch (e) {
    $("error").textContent = String(e);
  }
}
controls();
worker.onmessage = ({ data }) => {
  if (data.ready) {
    catalog = data.catalog;
    $("preset").replaceChildren(
      ...catalog.presets.map((p) => new Option(p.name, p.id)),
    );
    ready = true;
    const pending = localStorage.getItem("oil-pending-composition");
    if (pending) {
      localStorage.removeItem("oil-pending-composition");
      try {
        doc = a.parse(pending);
        planning = false;
        controls();
        $("source").value = JSON.stringify(
          catalogStillLife(catalog).spec,
          null,
          2,
        );
        regions();
        render();
      } catch (e) {
        $("error").textContent = String(e);
        reset();
      }
    } else reset();
    return;
  }
  busy = false;
  if (data.error) {
    $("error").textContent = data.error;
    planning = false;
    controls();
    pump();
    return;
  }
  if (data.op === "plan") {
    try {
      doc = data.region
        ? replaceRegion(doc, data.document, data.region)
        : data.document;
      regions();
      $("caption").textContent =
        `${doc.groups.reduce((n, g) => n + g.strokes.length, 0)} strokes · ${data.ms.toFixed(0)} ms planning${data.region ? " · accepted only " + data.region : ""}`;
      planning = false;
      controls();
      render(data.requestedAt);
    } catch (e) {
      planning = false;
      controls();
      $("error").textContent = String(e);
    }
  } else if (data.id === latest) {
    last = data.image;
    draw($("live"), last);
    const latency = performance.now() - data.requestedAt;
    $("status").textContent =
      `Paint ${data.paintMs.toFixed(1)} ms · view ${data.viewMs.toFixed(1)} ms · update ${latency.toFixed(0)} ms · reused ${data.summary?.reusedGroups ?? "all"} groups`;
    window.compositionMeasurement = {
      id: data.id,
      paintMs: data.paintMs,
      viewMs: data.viewMs,
      latency,
    };
    $("error").textContent = "";
  }
  pump();
};
$("region").onchange = select;
$("tint").onclick = () => {
  const rgb = [1, 3, 5].map(
      (i) => parseInt($("color").value.slice(i, i + 2), 16) / 255,
    ),
    t = +$("amount").value;
  commit(
    editRegion(doc, $("region").value, (g) => {
      for (const s of g.strokes)
        for (const key of ["color", "color2"])
          s[key] = s[key].map((v, i) => v * (1 - t) + rgb[i] * t);
    }),
  );
};
$("visible").onchange = () =>
  commit(
    editRegion(
      doc,
      $("region").value,
      (g) => (g.visible = $("visible").checked),
    ),
  );
$("translate").onclick = () =>
  commit(
    editRegion(doc, $("region").value, (g) => {
      for (const s of g.strokes)
        for (const p of s.points) {
          p[0] += +$("dx").value;
          p[1] += +$("dy").value;
        }
    }),
  );
$("replan").onclick = () => {
  try {
    const scene = JSON.parse($("source").value),
      region = $("region").value,
      p = catalog.presets.find((p) => p.id === $("preset").value);
    scene.styles[region] = {
      ...scene.styles[region],
      brushPreset: p.id,
      width: [
        Math.max(p.width[0], p.width[1] * 0.8),
        Math.min(p.width[2], p.width[1] * 1.3),
      ],
    };
    $("source").value = JSON.stringify(scene, null, 2);
    plan(region);
  } catch (e) {
    $("error").textContent = String(e);
  }
};
$("plan").onclick = () => plan();
$("reset").onclick = reset;
$("reference").onclick = () => {
  if (last) draw($("saved"), last);
};
$("view").onchange = () => {
  if (doc) enqueue({ op: "render", width: +$("width").value, view: view() });
};
$("width").onchange = render;
function save(name, value) {
  const url = URL.createObjectURL(
    new Blob([
      typeof value === "string" || value instanceof Uint8Array
        ? value
        : JSON.stringify(value, null, 2),
    ]),
  );
  const l = document.createElement("a");
  l.href = url;
  l.download = name;
  l.click();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
}
$("save").onclick = () => save("painting.composition.json", doc);
$("strokes").onclick = () => save("painting.oilstrokes", a.compile(doc));
$("open").onclick = () => {
  $("file").value = "";
  $("file").click();
};
$("file").onchange = async () => {
  try {
    const d = a.parse(await $("file").files[0].text());
    if (d.format !== "oil-composition")
      throw Error(
        "Open an oil-composition document; authored brush studies belong in the Brush laboratory.",
      );
    doc = d;
    regions();
    controls();
    render();
  } catch (e) {
    $("error").textContent = String(e);
  }
};
