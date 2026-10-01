import { denseDryControls, formStudy, reviewCases } from "./study-cases.js";
import { loadEngine } from "../src/engine.ts";
import { createAuthor, editGroup, moveGroup, sampleMark, defaultView, softView, rakingView } from "../src/author.ts";
const $ = (id) => document.getElementById(id);
const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
let author,
  doc,
  selected = "brush-study",
  ready = false,
  busy = false,
  pending = null,
  timer,
  seq = 0,
  latest = 0,
  started = 0,
  needsPaint = true;
let serial = 1,
  lastImage,
  lastMetadata,
  flight,
  path = [];
const controls = [
  ["pressure", "Pressure", 0, 1, 0.01, 1],
  ["load", "Load", 0, 1.5, 0.01, 1],
  ["pickup", "Pickup", 0, 1, 0.01, 0.015],
  ["hgain", "Thickness", 0, 3, 0.01, 0.55],
  ["dry", "Dry after group", 0, 1, 0.01, 0],
  ["body", "Body coverage", 0, 1, 0.01, 0.97],
  ["opacity", "Opacity", 0, 1, 0.01, 0.99],
  ["vdry", "Dry breakup threshold", 0.05, 1, 0.01, 0.36],
];
for (const [id, label, min, max, step, value] of controls) {
  const l = document.createElement("label");
  l.textContent = label;
  const o = document.createElement("output");
  o.id = id + "Value";
  const input = document.createElement("input");
  Object.assign(input, { id, type: "range", min, max, step, value });
  l.append(o, input);
  $("controls").append(l);
  input.oninput = () => {
    editBrush(id);
  };
}
function color(hex) {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
}
function hex(rgb) {
  return (
    "#" +
    rgb
      .map((v) =>
        Math.round(v * 255)
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
  );
}
function group() {
  return doc.groups.find((g) => g.id === selected);
}
function preset() {
  return doc.catalog.presets.find((p) => p.id === $("preset").value);
}
function refreshGroups() {
  $("group").replaceChildren(...doc.groups.map((g) => new Option(g.name, g.id)));
  $("group").value = selected;
  $("name").value = group().name;
  $("visible").checked = group().visible;
  refreshPaths();
}
function refreshPaths() {
  $("stroke").replaceChildren(...group().strokes.map((s) => new Option(s.id, s.id)));
  showPath();
}
function showPath() {
  $("path").value = JSON.stringify(group().strokes.find((s) => s.id === $("stroke").value)?.path ?? [], null, 1);
}
function refreshCatalog() {
  const retained=author.catalog().presets;
  $("preset").replaceChildren(...doc.catalog.presets.map((p) => new Option(p.name + (JSON.stringify(p)===JSON.stringify(retained.find(b=>b.id===p.id))?" · retained":" · custom candidate"), p.id)));
}
function brushUi(mark) {
  const p = doc.catalog.presets.find((p) => p.id === (mark?.preset ?? $("preset").value)) ?? doc.catalog.presets[0];
  $("preset").value = p.id;
  $("width").min = p.width[0];
  $("width").max = p.width[2];
  $("width").value = mark?.width ?? p.width[1];
  for (const [id, , , , , value] of controls) {
    $(id).value =
      id === "dry"
        ? 1 - group().dryAfter
        : id === "pressure"
          ? (mark?.path[0]?.[2] ?? 1)
          : (mark?.controls[id] ?? p.paint[id] ?? value);
  }
  if (mark) $("paint").value = hex(mark.color);
  values();
}
function values() {
  for (const id of ["width", ...controls.map((c) => c[0])])
    $(id + "Value").textContent = Number($(id).value).toFixed(id === "width" ? 4 : 2);
}
function settings() {
  return Object.fromEntries(
    controls.filter((c) => !["pressure", "dry"].includes(c[0])).map((c) => [c[0], +$(c[0]).value]),
  );
}
function editBrush(field) {
  if (!doc) return;
  values();
  doc = editGroup(doc, selected, (g) => {
    if (field === "dry") g.dryAfter = 1 - +$("dry").value;
    for (const s of g.strokes) {
      if (field === "preset") {
        s.preset = preset().id;
        s.width = +$("width").value;
        s.controls = settings();
      } else if (field === "width") s.width = +$("width").value;
      else if (field === "color") s.color = color($("paint").value);
      else if (field === "pressure") for (const p of s.path) p[2] = +$("pressure").value;
      else if (field !== "dry") s.controls[field] = +$(field).value;
    }
  });
  refreshPaths();
  schedule(true);
}
function view() {
  return {
    mode: $("view").value,
    direction: [+$("lx").value, +$("ly").value, +$("lz").value],
    bump: +$("bump").value,
    contrast: +$("contrast").value,
    specular: +$("specular").value,
  };
}
function setLight(p) {
  for (const [id, value] of Object.entries({
    lx: p.direction[0],
    ly: p.direction[1],
    lz: p.direction[2],
    bump: p.bump,
    contrast: p.contrast,
    specular: p.specular,
  }))
    $(id).value = value;
}
for (const button of document.querySelectorAll("[data-light]"))
  button.onclick = () => {
    setLight({ soft: softView, studio: defaultView, raking: rakingView }[button.dataset.light]);
    schedule(false);
  };
function schedule(paint, urgent = false) {
  needsPaint ||= paint;
  latest = ++seq;
  started = performance.now();
  if (urgent && timer) return;
  clearTimeout(timer);
  timer = setTimeout(
    () => {
      timer = null;
      pending = {
        id: latest,
        startedAt: started,
        referenceMeta: {
          preset: $("preset").selectedOptions[0].textContent,
          mixer: $("mixer").value,
          view: view(),
          width: +$("resolution").value,
        },
        view: view(),
        width: +$("resolution").value,
        mixer: $("mixer").value,
        document: needsPaint ? structuredClone(doc) : undefined,
      };
      pump();
    },
    urgent ? 16 : paint ? 75 : 16,
  );
}
function pump() {
  if (!ready || busy || !pending) return;
  busy = true;
  const p = pending;
  pending = null;
  if (p.document && p.id === latest) needsPaint = false;
  flight = p;
  worker.postMessage(p);
}
function draw(canvas, img) {
  canvas.width = img.width;
  canvas.height = img.height;
  const rgba = new Uint8ClampedArray(img.width * img.height * 4);
  for (let i = 0, j = 0; i < img.data.length; i += 3, j += 4) {
    rgba[j] = img.data[i];
    rgba[j + 1] = img.data[i + 1];
    rgba[j + 2] = img.data[i + 2];
    rgba[j + 3] = 255;
  }
  canvas.getContext("2d").putImageData(new ImageData(rgba, img.width, img.height), 0, 0);
}
worker.onmessage = ({ data }) => {
  if (data.ready) {
    ready = true;
    if (doc) schedule(true);
    return;
  }
  busy = false;
  if (data.id === latest || drawingId) {
    if (data.error) {
      $("error").textContent = data.error;
    } else {
      $("error").textContent = "";
      draw($("live"), data.image);
      lastImage = data.image;
      lastMetadata = flight.referenceMeta;
      const latency = performance.now() - flight.startedAt;
      $("status").textContent =
        `Paint ${data.paintMs.toFixed(1)} ms · view ${data.viewMs.toFixed(1)} ms · input → canvas ${latency.toFixed(1)} ms${data.summary ? " · reused " + data.summary.reusedGroups + " groups" : " · relight only"}`;
      window.previewMeasurement = { paintMs: data.paintMs, viewMs: data.viewMs, latency, id: data.id };
    }
  }
  pump();
};
worker.onerror = (e) => {
  $("error").textContent = e.message;
  busy = false;
};
function addSample(kind) {
  const s = sampleMark(selected + "/sample", preset().id, +$("width").value, kind);
  s.color = color($("paint").value);
  s.controls = { ...settings(), ...s.controls };
  s.path.forEach((p) => (p[2] *= +$("pressure").value));
  doc = editGroup(doc, selected, (g) => {
    g.strokes = [s];
  });
  refreshPaths();
  schedule(true);
}
for (const button of document.querySelectorAll("[data-sample]"))
  button.onclick = () => addSample(button.dataset.sample);
$("denseDry").onclick = () => {
  $("preset").value = "dry-drag";
  brushUi();
  editBrush("preset");
  doc = editGroup(doc, selected, (g) => {
    for (const s of g.strokes) Object.assign(s.controls, denseDryControls);
  });
  brushUi(group().strokes[0]);
  schedule(true);
};
function openStudy(d) {
  doc = d;
  selected = d.groups.at(-1).id;
  serial = Date.now();
  refreshCatalog();
  refreshGroups();
  brushUi(group().strokes[0]);
  $("ground").value = hex(d.ground);
  schedule(true);
}
$("formStudy").onclick = () => openStudy(formStudy(author));
$("pickupTail").onclick = () => openStudy(reviewCases(author).find((c) => c.id === "pickup-tail").d);
$("preset").onchange = () => {
  brushUi();
  editBrush("preset");
};
$("paint").oninput = () => editBrush("color");
$("width").oninput = () => editBrush("width");
$("ground").oninput = () => {
  doc.ground = color($("ground").value);
  schedule(true);
};
$("mixer").onchange = () => schedule(true);
$("resolution").onchange = () => schedule(true);
for (const id of ["view", "lx", "ly", "lz", "bump", "contrast", "specular"]) $(id).oninput = () => schedule(false);
$("group").onchange = () => {
  selected = $("group").value;
  refreshGroups();
  brushUi(group().strokes[0]);
};
$("name").onchange = () => {
  doc = editGroup(doc, selected, (g) => {
    g.name = $("name").value || g.name;
  });
  refreshGroups();
};
$("visible").onchange = () => {
  doc = editGroup(doc, selected, (g) => {
    g.visible = $("visible").checked;
  });
  schedule(true);
};
$("add").onclick = () => {
  selected = "group-" + serial++;
  doc.groups.push({ id: selected, name: "Group " + serial, visible: true, dryAfter: 1, strokes: [] });
  refreshGroups();
};
for (const [id, delta] of [
  ["up", -1],
  ["down", 1],
])
  $(id).onclick = () => {
    const i = doc.groups.findIndex((g) => g.id === selected);
    doc = moveGroup(doc, selected, Math.max(0, Math.min(doc.groups.length - 1, i + delta)));
    refreshGroups();
    schedule(true);
  };
$("clear").onclick = () => {
  doc = editGroup(doc, selected, (g) => (g.strokes = []));
  refreshPaths();
  schedule(true);
};
$("transform").onclick = () => {
  try {
    const next = editGroup(doc, selected, (g) => {
      for (const s of g.strokes)
        for (const p of s.path) {
          p[0] = p[0] * +$("scale").value + +$("dx").value;
          p[1] = p[1] * +$("scale").value + +$("dy").value;
        }
    });
    author.compile(next);
    doc = next;
    refreshPaths();
    schedule(true);
  } catch (e) {
    $("error").textContent = String(e);
  }
};
$("stroke").onchange = showPath;
$("pathApply").onclick = () => {
  try {
    const next = editGroup(doc, selected, (g) => {
      g.strokes.find((s) => s.id === $("stroke").value).path = JSON.parse($("path").value);
    });
    author.compile(next);
    doc = next;
    schedule(true);
  } catch (e) {
    $("error").textContent = String(e);
  }
};
$("underpaint").onclick = () => {
  if (doc.groups.some((g) => g.id === "underpaint")) return;
  const s = sampleMark("underpaint-stroke", "loaded-flat", 0.09, "straight", 0.31);
  s.path = [
    [0.44, 0.16, 1],
    [0.44, 0.52, 1],
  ];
  s.color = [0.12, 0.35, 0.68];
  doc.groups.unshift({ id: "underpaint", name: "Wet underpaint", visible: true, dryAfter: 1, strokes: [s] });
  refreshGroups();
  schedule(true);
};
function save(name, data, type = "application/json") {
  const url = URL.createObjectURL(
    new Blob([typeof data === "string" || data instanceof Uint8Array ? data : JSON.stringify(data, null, 2)], { type }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
$("presetExport").onclick = () => save("candidate-brush-catalog.json", doc.catalog);
$("docExport").onclick = () => save("painting.oil-author.json", doc);
$("strokesExport").onclick = () => {
  try {
    save("painting.oilstrokes", author.compile(doc), "application/octet-stream");
  } catch (e) {
    $("error").textContent = String(e);
  }
};
for (const type of ["preset", "doc"])
  $(type + "Import").onclick = () => {
    $("file").value = "";
    $("file").onchange = async () => {
      try {
        const text = await $("file").files[0].text();
        if (type === "doc") {
          const opened = author.parse(text);
        if(opened.format==="oil-composition"){localStorage.setItem("oil-pending-composition",text);location.href="/packages/oilpaint/preview/composition.html";return;}
        doc=opened;
          selected = doc.groups[0]?.id;
          if (!selected) {
            selected = "brush-study";
            doc.groups.push({ id: selected, name: "Brush study", visible: true, dryAfter: 1, strokes: [] });
          }
          serial = Date.now();
          $("ground").value = hex(doc.ground);
        } else {
          const next = structuredClone(doc);
          next.catalog = author.validateCatalog(JSON.parse(text));
          author.compile(next);
          doc = next;
        }
        refreshCatalog();
        refreshGroups();
        brushUi(group().strokes[0]);
        schedule(true);
      } catch (e) {
        $("error").textContent = String(e);
      }
    };
    $("file").click();
  };
$("reset").onclick = () => {
  brushUi();
  editBrush("preset");
};
$("capture").onclick = () => {
  if (!lastImage || busy || pending || timer) return;
  draw($("reference"), lastImage);
  const captured = { url: $("reference").toDataURL(), metadata: lastMetadata };
  localStorage.setItem("oil-brush-reference", JSON.stringify(captured));
  $("referenceLabel").textContent =
    lastMetadata.preset +
    " · " +
    lastMetadata.mixer +
    " · " +
    lastMetadata.view.mode +
    " · " +
    lastMetadata.width +
    " px";
};
$("dropReference").onclick = () => {
  localStorage.removeItem("oil-brush-reference");
  $("reference").getContext("2d").clearRect(0, 0, $("reference").width, $("reference").height);
  $("referenceLabel").textContent = "No saved reference";
};
try {
  const saved = JSON.parse(localStorage.getItem("oil-brush-reference"));
  if (saved) {
    const img = new Image();
    img.onload = () => {
      $("reference").width = img.width;
      $("reference").height = img.height;
      $("reference").getContext("2d").drawImage(img, 0, 0);
    };
    img.src = saved.url;
    $("referenceLabel").textContent =
      saved.metadata.preset + " · " + saved.metadata.mixer + " · " + saved.metadata.view.mode;
  }
} catch {}
function pointer(e) {
  const r = $("live").getBoundingClientRect();
  return [
    (e.clientX - r.left) / r.width,
    (e.clientY - r.top) / r.width,
    e.pointerType === "pen" ? e.pressure : +$("pressure").value,
  ];
}
$("live").onpointerdown = (e) => {
  e.preventDefault();
  $("live").setPointerCapture(e.pointerId);
  path = [pointer(e)];
};
$("live").onpointermove = (e) => {
  if (!path.length) return;
  for (const p of e.getCoalescedEvents?.() ?? [e]) path.push(pointer(p));
  if (path.length > 1000) path = path.filter((_, i) => i % 2 === 0);
  previewPointer();
};
let drawingId;
function previewPointer() {
  if (!drawingId) drawingId = "stroke-" + serial++;
  let pts = path.length === 1 ? [path[0], [path[0][0] + 0.0001, path[0][1], path[0][2]]] : structuredClone(path);
  const s = {
    id: drawingId,
    preset: preset().id,
    width: +$("width").value,
    path: pts,
    color: color($("paint").value),
    controls: settings(),
  };
  doc = editGroup(doc, selected, (g) => {
    g.strokes = g.strokes.filter((s) => s.id !== drawingId);
    g.strokes.push(s);
  });
  schedule(true, true);
}
$("live").onpointerup = () => {
  if (path.length) {
    previewPointer();
    path = [];
    drawingId = null;
    refreshPaths();
  }
};
$("live").onpointercancel = () => {
  path = [];
  drawingId = null;
};
try {
  author = createAuthor(await loadEngine());
  doc = author.document();
  doc.ground = color($("ground").value);
  doc.groups = [{ id: selected, name: "Brush study", visible: true, dryAfter: 1, strokes: [] }];
  refreshCatalog();
  refreshGroups();
  brushUi();
  setLight(defaultView);
  addSample("straight");
} catch (e) {
  $("error").textContent = String(e);
}
