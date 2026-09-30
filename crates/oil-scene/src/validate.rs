//! Parsing and validation (spec/ERRORS.md): every problem as a structured error with a JSON Pointer, and a fix
//! where one exists. Schema errors come from serde (the first one only: serde stops there); the semantic pass
//! reports all of them.
use crate::color;
use crate::spec::*;
use oil_errors::{suggest, Error};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Errors stop a run; warnings (same shape) never do.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Report {
    pub errors: Vec<Error>,
    pub warnings: Vec<Error>,
}

/// Parse and validate a ScenePlan. On success returns the plan and its warnings; otherwise every error found.
pub fn load(text: &str) -> Result<(ScenePlan, Vec<Error>), Vec<Error>> {
    let plan = parse(text)?;
    let r = validate(&plan);
    if r.errors.is_empty() {
        Ok((plan, r.warnings))
    } else {
        Err(r.errors)
    }
}

/// Parse JSON text into a ScenePlan (schema checks only).
pub fn parse(text: &str) -> Result<ScenePlan, Vec<Error>> {
    let v: Value = serde_json::from_str(text).map_err(|e| {
        vec![Error::new("SCHEMA", format!("not valid JSON: {e}")).got(format!("line {}, column {}", e.line(), e.column()))]
    })?;
    from_value(v)
}

pub fn from_value(v: Value) -> Result<ScenePlan, Vec<Error>> {
    match v.get("sceneplan") {
        Some(n) if n.as_u64() == Some(1) => {}
        Some(n) => {
            return Err(vec![Error::new("UNSUPPORTED_SCENEPLAN", "this engine reads ScenePlan generation 1")
                .path("/sceneplan")
                .got(n)
                .expected(1)])
        }
        None if v.is_object() => {
            return Err(vec![Error::new("SCHEMA", "missing key sceneplan").path("/sceneplan").fix("add \"sceneplan\": 1")])
        }
        None => return Err(vec![Error::new("SCHEMA", "a ScenePlan is a JSON object").path("").got(kind(&v))]),
    }
    serde_path_to_error::deserialize(v).map_err(|e| vec![schema_error(&e)])
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn pointer_escape(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// v1 (Python) key names and their ScenePlan names.
const V1_KEYS: [(&str, &str); 4] = [("T", "errorThreshold"), ("fg", "gridFactor"), ("fs", "referenceBlur"), ("L_floor", "lFloor")];

fn camel(s: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in s.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Backquoted names in a serde message, in order.
fn quoted(msg: &str) -> Vec<&str> {
    msg.split('`').skip(1).step_by(2).collect()
}

fn schema_error(e: &serde_path_to_error::Error<serde_json::Error>) -> Error {
    use serde_path_to_error::Segment;
    let mut path = String::new();
    for seg in e.path().iter() {
        path.push('/');
        match seg {
            Segment::Seq { index } => path.push_str(&index.to_string()),
            Segment::Map { key } => path.push_str(&pointer_escape(key)),
            Segment::Enum { variant } => path.push_str(&pointer_escape(variant)),
            Segment::Unknown => path.push('?'),
        }
    }
    let msg = e.inner().to_string();
    let msg = msg.split(" at line ").next().unwrap_or(&msg).to_string();
    let names = quoted(&msg);
    if msg.starts_with("unknown field") || msg.starts_with("unknown variant") {
        let what = if msg.starts_with("unknown field") { "key" } else { "kind" };
        let got = names.first().copied().unwrap_or("");
        let cands: Vec<&str> = names.iter().skip(1).copied().collect();
        let key = format!("/{}", pointer_escape(got));
        let path = if path.ends_with(&key) { path } else { path + &key };
        let mut err = Error::new("SCHEMA", format!("unknown {what} {got}")).path(path).got(got);
        let cc = camel(got);
        if let Some((_, new)) = V1_KEYS.iter().find(|(old, _)| *old == got) {
            err = err.fix(format!("v1's {got} is \"{new}\" in a ScenePlan"));
        } else if cc != got && cands.contains(&cc.as_str()) {
            err = err.fix(format!("keys are camelCase: use \"{cc}\""));
        } else if let Some(s) = suggest(got, cands.iter().copied()) {
            err = err.fix(format!("did you mean \"{s}\"?"));
        } else {
            err = err.expected(cands.join(", "));
        }
        return err;
    }
    if let Some(field) = msg.strip_prefix("missing field ").map(|_| names.first().copied().unwrap_or("")) {
        return Error::new("SCHEMA", format!("missing key {field}")).path(format!("{path}/{field}")).fix(format!("add \"{field}\""));
    }
    let untagged = [
        ("ColorSpec", "a colour: \"#rrggbb\", a tube name, [r, g, b] in [0, 1], or [\"a\", \"b\", t]"),
        ("NumOrMap", "a number, or {region: number}"),
        ("RegionSel", "\"all\" or a list of region names"),
        ("CurveSpec", "\"boundary\" or a polyline [[x, y], ...]"),
    ];
    if let Some((_, exp)) = untagged.iter().find(|(t, _)| msg.contains(&format!("untagged enum {t}"))) {
        return Error::new("SCHEMA", format!("not {}", exp.split(':').next().unwrap_or(exp))).path(path).expected(*exp);
    }
    Error::new("SCHEMA", msg).path(path)
}

// ------------------------------------------------------------------ semantic checks

/// All semantic checks: ranges, units, names, references and colours.
pub fn validate(plan: &ScenePlan) -> Report {
    let mut v = V { errs: Vec::new(), ar: 1.0, fields: &plan.fields };
    let mut warnings = Vec::new();
    if let Some(e) = &plan.engine {
        if e != oil_kernel::ENGINE_VERSION {
            warnings.push(
                Error::new("ENGINE_VERSION_DIFFERS", format!("the scene was authored with engine {e}; this is engine {}", oil_kernel::ENGINE_VERSION))
                    .path("/engine")
                    .got(e)
                    .expected(oil_kernel::ENGINE_VERSION)
                    .fix("check the result, then set \"engine\" to this version"),
            );
        }
    }
    let [aw, ah] = plan.canvas.aspect;
    if aw == 0 || ah == 0 || aw.max(ah) > 16 * aw.min(ah) {
        v.errs.push(Error::new("RANGE", "aspect is [width, height], positive integers, at most 16:1").path("/canvas/aspect").got(format!("[{aw}, {ah}]")));
    } else {
        v.ar = ah as f64 / aw as f64;
    }
    v.color("/canvas/ground", &plan.canvas.ground);
    for (name, f) in &plan.fields {
        let p = format!("/fields/{}", pointer_escape(name));
        if f.width == 0 || f.height == 0 || f.width > 16384 || f.height > 16384 {
            v.errs.push(Error::new("RANGE", "field size is 1..16384 per side").path(format!("{p}/width")).got(format!("{} x {}", f.width, f.height)));
        }
        if f.sha256.len() != 64 || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
            v.errs.push(Error::new("RANGE", "sha256 is 64 lowercase hex digits").path(format!("{p}/sha256")).got(&f.sha256));
        }
    }
    for (i, op) in plan.target.iter().enumerate() {
        v.target(&format!("/target/{i}"), op);
    }

    let mut names = BTreeSet::new();
    if plan.regions.is_empty() {
        v.errs.push(Error::new("RANGE", "a scene needs at least one region").path("/regions").fix("add {\"name\": \"all\", \"shape\": {\"all\": true}}"));
    }
    if plan.regions.len() > 255 {
        v.errs.push(Error::new("RANGE", "at most 255 regions").path("/regions").got(plan.regions.len()));
    }
    for (i, r) in plan.regions.iter().enumerate() {
        let p = format!("/regions/{i}");
        if r.name.is_empty() || r.name.len() > 64 {
            v.errs.push(Error::new("RANGE", "a region name has 1 to 64 characters").path(format!("{p}/name")).got(&r.name));
        }
        if !names.insert(r.name.as_str()) {
            v.errs.push(Error::new("DUPLICATE_NAME", format!("region {} is declared twice", r.name)).path(format!("{p}/name")).got(&r.name));
        }
        v.shape(&format!("{p}/shape"), &r.shape);
        v.cw0(&format!("{p}/edge"), r.edge, 0.5);
        if let Some(f) = &r.flow {
            v.flow(&format!("{p}/flow"), f);
        }
    }
    for (i, l) in plan.lights.iter().enumerate() {
        v.light(&format!("/lights/{i}"), l);
    }
    for (name, st) in &plan.styles {
        let p = format!("/styles/{}", pointer_escape(name));
        v.region_name(&p, name, &names);
        v.style_keys(&p, &serde_json::to_value(st).unwrap_or(Value::Null));
    }
    let mut layer_names = BTreeSet::new();
    for (i, l) in plan.layers.iter().enumerate() {
        let p = format!("/layers/{i}");
        if !layer_names.insert(l.name.as_str()) {
            v.errs.push(Error::new("DUPLICATE_NAME", format!("layer {} is declared twice", l.name)).path(format!("{p}/name")).got(&l.name));
        }
        v.layer(&p, l, &names);
    }
    Report { errors: v.errs, warnings }
}

struct V<'a> {
    errs: Vec<Error>,
    /// Canvas height in cw (aspect height / width).
    ar: f64,
    fields: &'a BTreeMap<String, FieldDecl>,
}

const PX_FIX: &str = "coordinates and sizes are in canvas widths (cw): divide pixels by the canvas width in pixels";

impl V<'_> {
    fn range(&mut self, p: &str, x: f64, lo: f64, hi: f64) {
        if !(lo..=hi).contains(&x) {
            self.errs.push(Error::new("RANGE", format!("{} is out of range", p.rsplit('/').next().unwrap_or(p))).path(p).got(x).expected(format!("[{lo}, {hi}]")));
        }
    }

    fn positive(&mut self, p: &str, x: f64) {
        if x <= 0.0 {
            self.errs.push(Error::new("RANGE", format!("{} must be positive", p.rsplit('/').next().unwrap_or(p))).path(p).got(x).expected("> 0"));
        }
    }

    /// A size in cw, `0 < x <= hi` (`0 <= x` with `zero_ok`). Above `hi` it is probably pixels (UNITS).
    fn size(&mut self, p: &str, x: f64, zero_ok: bool, hi: f64) {
        let key = p.rsplit('/').next().unwrap_or(p);
        let expected = format!("{}0, {hi}] cw", if zero_ok { "[" } else { "(" });
        if x > hi {
            let e = Error::new("UNITS", format!("{key} is too large for canvas widths")).path(p).got(x).expected(expected);
            self.errs.push(if x >= 2.0 { e.fix(format!("{PX_FIX}: {x} px on a 600 px plan is {:.4} cw", x / 600.0)) } else { e });
        } else if x < 0.0 || (x == 0.0 && !zero_ok) {
            self.errs.push(Error::new("RANGE", format!("{key} must be positive")).path(p).got(x).expected(expected));
        }
    }

    fn cw(&mut self, p: &str, x: f64, hi: f64) {
        self.size(p, x, false, hi);
    }

    fn cw0(&mut self, p: &str, x: f64, hi: f64) {
        self.size(p, x, true, hi);
    }

    fn pos(&mut self, p: &str, pt: &Point) {
        let (x, y) = (pt[0], pt[1]);
        if !(-1.0..=2.0).contains(&x) || !(-1.0..=self.ar + 1.0).contains(&y) {
            let e = Error::new("UNITS", "a point is far outside the canvas")
                .path(p)
                .got(format!("[{x}, {y}]"))
                .expected(format!("x in [-1, 2], y in [-1, {}] (the canvas is 1 x {} cw)", self.ar + 1.0, self.ar));
            self.errs.push(if x.abs().max(y.abs()) >= 4.0 { e.fix(PX_FIX) } else { e });
        }
    }

    fn poly(&mut self, p: &str, pts: &[Point], min: usize) {
        if pts.len() < min {
            self.errs.push(Error::new("RANGE", format!("needs at least {min} points")).path(p).got(pts.len()).expected(format!(">= {min} points")));
        }
        for (i, pt) in pts.iter().enumerate() {
            self.pos(&format!("{p}/{i}"), pt);
        }
    }

    fn color(&mut self, p: &str, c: &ColorSpec) {
        self.errs.extend(color::check(c, p));
    }

    fn seedless_noise(&mut self, p: &str, noise: f64) {
        self.range(&format!("{p}/noise"), noise, 0.0, 4.0);
    }

    fn angle(&mut self, p: &str, a: f64) {
        self.range(p, a, -720.0, 720.0);
    }

    fn field(&mut self, p: &str, name: &str, want: FieldKind) {
        match self.fields.get(name) {
            Some(f) if f.kind == want => {}
            Some(f) => self.errs.push(
                Error::new("UNKNOWN_FIELD", format!("field {name} is a {:?} field", f.kind)).path(p).got(name).expected(format!("a {want:?} field")),
            ),
            None => {
                let mut e = Error::new("UNKNOWN_FIELD", format!("field {name} is not declared in /fields")).path(p).got(name);
                if let Some(s) = suggest(name, self.fields.keys().map(String::as_str)) {
                    e = e.fix(format!("did you mean \"{s}\"?"));
                }
                self.errs.push(e);
            }
        }
    }

    fn shape(&mut self, p: &str, s: &Shape) {
        match s {
            Shape::All(_) => {}
            Shape::Above(y) | Shape::Below(y) => self.pos(p, &[0.5, *y]),
            Shape::Ellipse(e) => {
                self.pos(&format!("{p}/ellipse/c"), &e.c);
                self.cw(&format!("{p}/ellipse/r/0"), e.r[0], 4.0);
                self.cw(&format!("{p}/ellipse/r/1"), e.r[1], 4.0);
                self.range(&format!("{p}/ellipse/softness"), e.softness, 0.0, 10.0);
            }
            Shape::Disc(d) => {
                self.pos(&format!("{p}/disc/c"), &d.c);
                self.cw(&format!("{p}/disc/r"), d.r, 4.0);
                self.range(&format!("{p}/disc/softness"), d.softness, 0.0, 10.0);
            }
            Shape::Polygon(pts) => self.poly(&format!("{p}/polygon"), pts, 3),
            Shape::Wedge(w) => self.wedge(&format!("{p}/wedge"), &w.apex, w.angle, w.spread, w.length),
            Shape::BandAround(b) => {
                self.poly(&format!("{p}/bandAround/polygon"), &b.polygon, 3);
                self.cw(&format!("{p}/bandAround/dist"), b.dist, 1.0);
            }
            Shape::Union(v) | Shape::Intersect(v) => {
                let k = if matches!(s, Shape::Union(_)) { "union" } else { "intersect" };
                if v.is_empty() {
                    self.errs.push(Error::new("RANGE", format!("{k} needs at least one shape")).path(format!("{p}/{k}")));
                }
                for (i, s) in v.iter().enumerate() {
                    self.shape(&format!("{p}/{k}/{i}"), s);
                }
            }
            Shape::Not(s) => self.shape(&format!("{p}/not"), s),
            Shape::Noisy(n) => {
                self.shape(&format!("{p}/noisy/shape"), &n.shape);
                self.range(&format!("{p}/noisy/amount"), n.amount, 0.0, 4.0);
                self.cw(&format!("{p}/noisy/scale"), n.scale, 4.0);
            }
            Shape::Field(name) => self.field(&format!("{p}/field"), name, FieldKind::Mask),
        }
    }

    fn wedge(&mut self, p: &str, apex: &Point, angle: f64, spread: f64, length: f64) {
        self.pos(&format!("{p}/apex"), apex);
        self.angle(&format!("{p}/angle"), angle);
        self.range(&format!("{p}/spread"), spread, 1e-3, 180.0);
        self.cw(&format!("{p}/length"), length, 4.0);
    }

    fn target(&mut self, p: &str, op: &TargetOp) {
        match op {
            TargetOp::Fill(f) => {
                let p = format!("{p}/fill");
                match (&f.color, &f.gradient_v) {
                    (Some(c), None) => self.color(&format!("{p}/color"), c),
                    (None, Some(stops)) => {
                        if stops.is_empty() {
                            self.errs.push(Error::new("RANGE", "a gradient needs at least one stop").path(format!("{p}/gradientV")));
                        }
                        for (i, (y, c)) in stops.iter().enumerate() {
                            self.pos(&format!("{p}/gradientV/{i}/0"), &[0.5, *y]);
                            self.color(&format!("{p}/gradientV/{i}/1"), c);
                            if i > 0 && *y <= stops[i - 1].0 {
                                self.errs.push(
                                    Error::new("RANGE", "gradient stops must have increasing y")
                                        .path(format!("{p}/gradientV/{i}/0"))
                                        .got(y)
                                        .expected(format!("> {}", stops[i - 1].0)),
                                );
                            }
                        }
                    }
                    _ => self.errs.push(Error::new("SCHEMA", "fill takes exactly one of color and gradientV").path(p.clone())),
                }
                if let Some(m) = &f.mask {
                    self.shape(&format!("{p}/mask"), m);
                }
            }
            TargetOp::Blob(b) => {
                let p = format!("{p}/blob");
                self.pos(&format!("{p}/c"), &b.c);
                self.cw(&format!("{p}/r/0"), b.r[0], 4.0);
                self.cw(&format!("{p}/r/1"), b.r[1], 4.0);
                self.color(&format!("{p}/color"), &b.color);
                self.range(&format!("{p}/softness"), b.softness, 0.0, 10.0);
                self.seedless_noise(&p, b.noise);
                self.range(&format!("{p}/strength"), b.strength, 0.0, 1.0);
            }
            TargetOp::Polygon(g) => {
                let p = format!("{p}/polygon");
                self.poly(&format!("{p}/points"), &g.points, 3);
                self.color(&format!("{p}/color"), &g.color);
                self.cw0(&format!("{p}/softness"), g.softness, 1.0);
                self.seedless_noise(&p, g.noise);
                self.range(&format!("{p}/strength"), g.strength, 0.0, 1.0);
            }
            TargetOp::Bands(b) => {
                let p = format!("{p}/bands");
                self.poly(&format!("{p}/points"), &b.points, 3);
                self.cw0(&format!("{p}/softness"), b.softness, 1.0);
                for (i, (f0, f1, c)) in b.bands.iter().enumerate() {
                    self.range(&format!("{p}/bands/{i}/0"), *f0, 0.0, 1.0);
                    self.range(&format!("{p}/bands/{i}/1"), *f1, *f0, 1.0);
                    self.color(&format!("{p}/bands/{i}/2"), c);
                }
            }
            TargetOp::Glow(g) => {
                let p = format!("{p}/glow");
                self.pos(&format!("{p}/c"), &g.c);
                self.cw(&format!("{p}/r"), g.r, 4.0);
                self.color(&format!("{p}/color"), &g.color);
                self.range(&format!("{p}/strength"), g.strength, 0.0, 2.0);
                self.range(&format!("{p}/power"), g.power, 0.1, 8.0);
            }
            TargetOp::Beam(b) => {
                let p = format!("{p}/beam");
                self.wedge(&p, &b.apex, b.angle, b.spread, b.length);
                self.color(&format!("{p}/color"), &b.color);
                self.range(&format!("{p}/strength"), b.strength, 0.0, 2.0);
                self.range(&format!("{p}/softness"), b.softness, 0.0, 10.0);
            }
        }
    }

    fn flow(&mut self, p: &str, f: &Flow) {
        match f {
            Flow::Constant(c) => {
                self.angle(&format!("{p}/constant/angle"), c.angle);
                self.seedless_noise(&format!("{p}/constant"), c.noise);
            }
            Flow::Sweep(s) => {
                let p = format!("{p}/sweep");
                self.angle(&format!("{p}/angle"), s.angle);
                self.range(&format!("{p}/curl"), s.curl, 0.0, 4.0);
                self.seedless_noise(&p, s.noise);
                self.cw(&format!("{p}/scale"), s.scale, 4.0);
            }
            Flow::Waves(w) => {
                let p = format!("{p}/waves");
                self.angle(&format!("{p}/angle"), w.angle);
                self.range(&format!("{p}/amplitude"), w.amplitude, 0.0, 90.0);
                self.cw(&format!("{p}/wavelength"), w.wavelength, 4.0);
                self.seedless_noise(&p, w.noise);
                self.pos(&format!("{p}/horizon"), &[0.5, w.horizon]);
            }
            Flow::SwirlAround(s) => {
                let p = format!("{p}/swirlAround");
                if s.centres.is_empty() {
                    self.errs.push(Error::new("RANGE", "swirlAround needs at least one centre").path(format!("{p}/centres")));
                }
                for (i, c) in s.centres.iter().enumerate() {
                    self.pos(&format!("{p}/centres/{i}"), &[c[0], c[1]]);
                    self.cw(&format!("{p}/centres/{i}/2"), c[2], 4.0);
                    self.cw(&format!("{p}/centres/{i}/3"), c[3], 4.0);
                }
                self.range(&format!("{p}/strength"), s.strength, 0.0, 1.0);
                self.seedless_noise(&p, s.noise);
            }
            Flow::RadialFrom(r) => {
                self.pos(&format!("{p}/radialFrom/c"), &r.c);
                self.seedless_noise(&format!("{p}/radialFrom"), r.noise);
            }
            Flow::Upward(u) => self.seedless_noise(&format!("{p}/upward"), u.noise),
            Flow::Contour(c) => {
                self.poly(&format!("{p}/contour/polygon"), &c.polygon, 3);
                self.seedless_noise(&format!("{p}/contour"), c.noise);
            }
            Flow::Field(name) => self.field(&format!("{p}/field"), name, FieldKind::Flow),
        }
    }

    fn light(&mut self, p: &str, l: &Light) {
        match l {
            Light::Glow(g) => {
                self.pos(&format!("{p}/glow/c"), &g.c);
                self.cw(&format!("{p}/glow/r"), g.r, 4.0);
                self.range(&format!("{p}/glow/strength"), g.strength, 0.0, 4.0);
            }
            Light::Beam(b) => {
                self.wedge(&format!("{p}/beam"), &b.apex, b.angle, b.spread, b.length);
                self.range(&format!("{p}/beam/strength"), b.strength, 0.0, 4.0);
            }
            Light::Lamp(g) => {
                self.pos(&format!("{p}/lamp/c"), &g.c);
                self.cw(&format!("{p}/lamp/r"), g.r, 4.0);
                self.range(&format!("{p}/lamp/strength"), g.strength, 0.0, 4.0);
            }
        }
    }

    fn region_name(&mut self, p: &str, name: &str, names: &BTreeSet<&str>) {
        if !names.contains(name) {
            let mut e = Error::new("UNKNOWN_REGION", format!("region {name} is not declared")).path(p).got(name);
            e = match suggest(name, names.iter().copied()) {
                Some(s) => e.fix(format!("did you mean \"{s}\"?")),
                None => e.expected(names.iter().copied().collect::<Vec<_>>().join(", ")),
            };
            self.errs.push(e);
        }
    }

    /// Style keys, shared by styles and layers, checked from their serialized form against one rules table.
    fn style_keys(&mut self, p: &str, v: &Value) {
        for (key, rule) in STYLE_RULES.iter() {
            let Some(x) = v.get(key) else { continue };
            let kp = format!("{p}/{key}");
            let num = |x: &Value| x.as_f64().unwrap_or(f64::NAN);
            match rule {
                R::Range(lo, hi) => self.range(&kp, num(x), *lo, *hi),
                R::Pos => self.positive(&kp, num(x)),
                R::Any => {}
                R::Pair(lo, hi, cw) => {
                    let (a, b) = (num(&x[0]), num(&x[1]));
                    if *cw {
                        self.cw(&format!("{kp}/0"), a, *hi);
                        self.cw(&format!("{kp}/1"), b, *hi);
                    } else {
                        self.range(&format!("{kp}/0"), a, *lo, *hi);
                        self.range(&format!("{kp}/1"), b, *lo, *hi);
                    }
                    if a > b {
                        self.errs.push(Error::new("RANGE", "a range is [min, max]").path(kp.clone()).got(format!("[{a}, {b}]")).fix(format!("use [{b}, {a}]")));
                    }
                }
                R::Color => {
                    if let Ok(c) = serde_json::from_value::<ColorSpec>(x.clone()) {
                        self.color(&kp, &c);
                    }
                }
                R::Colors => {
                    for (i, c) in x.as_array().into_iter().flatten().enumerate() {
                        if let Ok(c) = serde_json::from_value::<ColorSpec>(c.clone()) {
                            self.color(&format!("{kp}/{i}"), &c);
                        }
                    }
                }
                R::Flecks => {
                    for (i, f) in x.as_array().into_iter().flatten().enumerate() {
                        if let Ok(c) = serde_json::from_value::<ColorSpec>(f[0].clone()) {
                            self.color(&format!("{kp}/{i}/0"), &c);
                        }
                        self.range(&format!("{kp}/{i}/1"), num(&f[1]), 0.0, 1.0);
                    }
                }
                R::SizeByY => {
                    self.pos(&format!("{kp}/0"), &[0.5, num(&x[0])]);
                    self.pos(&format!("{kp}/1"), &[0.5, num(&x[1])]);
                    if num(&x[0]) == num(&x[1]) {
                        self.errs.push(Error::new("RANGE", "sizeByY needs y0 != y1").path(format!("{kp}/1")).got(num(&x[1])));
                    }
                    self.range(&format!("{kp}/2"), num(&x[2]), 0.01, 100.0);
                    self.range(&format!("{kp}/3"), num(&x[3]), 0.01, 100.0);
                }
            }
        }
    }

    fn layer(&mut self, p: &str, l: &Layer, names: &BTreeSet<&str>) {
        match &l.regions {
            RegionSel::Name(n) if n == "all" => {}
            RegionSel::Name(n) => self.errs.push(
                Error::new("SCHEMA", "regions is \"all\" or a list of region names").path(format!("{p}/regions")).got(n).fix(format!("use [\"{n}\"]")),
            ),
            RegionSel::List(v) => {
                if v.is_empty() {
                    self.errs.push(Error::new("RANGE", "a layer needs at least one region").path(format!("{p}/regions")));
                }
                for (i, n) in v.iter().enumerate() {
                    self.region_name(&format!("{p}/regions/{i}"), n, names);
                }
            }
        }
        self.range(&format!("{p}/errorThreshold"), l.error_threshold, 0.0, 200.0);
        self.range(&format!("{p}/gridFactor"), l.grid_factor, 0.05, 20.0);
        self.range(&format!("{p}/referenceBlur"), l.reference_blur, 0.0, 20.0);
        for (key, v, lo, hi) in [("spacing", &l.spacing, 0.05, 100.0), ("coverage", &l.coverage, 0.0, 1.0)] {
            match v {
                NumOrMap::Num(x) => self.range(&format!("{p}/{key}"), *x, lo, hi),
                NumOrMap::Map(m) => {
                    for (n, x) in m {
                        let kp = format!("{p}/{key}/{}", pointer_escape(n));
                        self.region_name(&kp, n, names);
                        self.range(&kp, *x, lo, hi);
                    }
                }
            }
        }
        if let Some(d) = l.dry_after {
            self.range(&format!("{p}/dryAfter"), d, 0.0, 1.0);
        }
        self.range(&format!("{p}/jitterPos"), l.jitter_pos, 0.0, 1.0);
        self.cw(&format!("{p}/hblurSigma"), l.hblur_sigma, 1.0);
        self.range(&format!("{p}/relief"), l.relief, 0.0, 10.0);
        if let Some(c) = l.max_cover {
            self.range(&format!("{p}/maxCover"), c, 0.0, 10.0);
        }
        self.range(&format!("{p}/gapCover"), l.gap_cover, 0.0, 1.0);
        if let Some(x) = l.dry_thresh {
            self.range(&format!("{p}/dryThresh"), x, 0.0, 10.0);
        }
        if let Some(x) = l.dry_width {
            self.range(&format!("{p}/dryWidth"), x, 1e-6, 10.0);
        }
        match (&l.curve, l.placement) {
            (None, Placement::Curve) => {
                self.errs.push(Error::new("SCHEMA", "curve placement needs curve: {region: \"boundary\" | [[x, y], ...]}").path(format!("{p}/curve")))
            }
            (Some(m), _) => {
                for (n, c) in m {
                    let kp = format!("{p}/curve/{}", pointer_escape(n));
                    self.region_name(&kp, n, names);
                    match c {
                        CurveSpec::Named(s) if s == "boundary" => {}
                        CurveSpec::Named(s) => {
                            self.errs.push(Error::new("SCHEMA", "a curve is \"boundary\" or a polyline").path(kp).got(s).expected("\"boundary\""))
                        }
                        CurveSpec::Points(pts) => self.poly(&kp, pts, 2),
                    }
                }
            }
            (None, _) => {}
        }
        self.range(&format!("{p}/curveOffset"), l.curve_offset, -20.0, 20.0);
        self.range(&format!("{p}/curveSpacing"), l.curve_spacing, 0.05, 100.0);
        self.range(&format!("{p}/curveJitter"), l.curve_jitter, 0.0, 20.0);
        self.style_keys(p, &serde_json::to_value(l).unwrap_or(Value::Null));
    }
}

enum R {
    Range(f64, f64),
    Pos,
    /// [min, max] within [lo, hi]; `true` if the values are cw sizes (UNITS when too large).
    Pair(f64, f64, bool),
    Color,
    Colors,
    Flecks,
    SizeByY,
    Any,
}

/// Every style key and its rule (the defaults are in spec/SCENEPLAN_V1.md).
const STYLE_RULES: [(&str, R); 46] = [
    ("mode", R::Any),
    ("colors", R::Colors),
    ("flecks", R::Flecks),
    ("width", R::Pair(0.0, 0.5, true)),
    ("length", R::Pair(0.0, 2.0, true)),
    ("curvature", R::Range(0.0, 1.0)),
    ("align", R::Range(0.0, 1.0)),
    ("opacity", R::Pair(0.0, 1.0, false)),
    ("pickup", R::Range(0.0, 1.0)),
    ("load", R::Range(0.0, 10.0)),
    ("deplete", R::Range(0.0, 10.0)),
    ("vdry", R::Pos),
    ("hgain", R::Range(0.0, 20.0)),
    ("flatten", R::Range(0.0, 1.0)),
    ("streak", R::Range(0.0, 10.0)),
    ("streakMix", R::Range(0.0, 1.0)),
    ("body", R::Range(0.0, 1.0)),
    ("hardness", R::Range(0.0, 1.0)),
    ("grain", R::Range(0.0, 10.0)),
    ("nbPerCw", R::Range(0.0, 5000.0)),
    ("nbBase", R::Range(0.0, 68.0)),
    ("release", R::Range(0.0, 1.0)),
    ("dropout", R::Range(0.0, 1.0)),
    ("ragged", R::Range(0.0, 10.0)),
    ("ridge", R::Range(0.0, 10.0)),
    ("levee", R::Range(0.0, 10.0)),
    ("furrow", R::Range(0.0, 10.0)),
    ("blob", R::Range(0.0, 10.0)),
    ("stiff", R::Range(0.0, 10.0)),
    ("marble", R::Range(0.0, 1.0)),
    ("load2", R::Color),
    ("splay", R::Range(0.0, 3.0)),
    ("snap", R::Range(0.0, 1.0)),
    ("jitter", R::Pair(0.0, 100.0, false)),
    ("warmth", R::Range(0.0, 1.0)),
    ("warmColor", R::Color),
    ("priority", R::Any),
    ("reverseP", R::Range(0.0, 1.0)),
    ("spill", R::Range(0.0, 1.0)),
    ("stopAtEdge", R::Range(0.0, 1.0)),
    ("minAspect", R::Range(0.0, 100.0)),
    ("endWidth", R::Range(0.0, 1.0)),
    ("endPressure", R::Range(0.0, 1.0)),
    ("hgainJitter", R::Range(0.0, 1.0)),
    ("sizeByY", R::SizeByY),
    ("opacityByLight", R::Range(0.0, 1.0)),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> Value {
        serde_json::json!({
            "sceneplan": 1,
            "canvas": {"aspect": [4, 5], "ground": "#e9e1d6"},
            "target": [{"fill": {"color": "lead_white"}}],
            "regions": [{"name": "all", "shape": {"all": true}}],
            "styles": {"all": {"width": [0.01, 0.02]}},
            "layers": [{"name": "one", "regions": ["all"]}]
        })
    }

    fn errs(v: Value) -> Vec<Error> {
        match from_value(v) {
            Ok(p) => validate(&p).errors,
            Err(e) => e,
        }
    }

    #[test]
    fn minimal_plan_is_valid() {
        assert!(errs(minimal()).is_empty());
    }

    #[test]
    fn schema_errors_point_at_the_key() {
        let mut v = minimal();
        v["styles"]["all"]["widht"] = serde_json::json!([0.01, 0.02]);
        let e = &errs(v)[0];
        assert_eq!((e.code, e.path.as_deref()), ("SCHEMA", Some("/styles/all/widht")));
        assert_eq!(e.fix.as_deref(), Some("did you mean \"width\"?"));

        let mut v = minimal();
        v["styles"]["all"]["stop_at_edge"] = serde_json::json!(0.5);
        assert_eq!(errs(v)[0].fix.as_deref(), Some("keys are camelCase: use \"stopAtEdge\""));

        let mut v = minimal();
        v["layers"][0]["T"] = serde_json::json!(12);
        assert_eq!(errs(v)[0].fix.as_deref(), Some("v1's T is \"errorThreshold\" in a ScenePlan"));

        let mut v = minimal();
        v["regions"][0]["shape"] = serde_json::json!({"elipse": {"c": [0.5, 0.5], "r": [0.1, 0.1]}});
        let e = &errs(v)[0];
        assert_eq!(e.path.as_deref(), Some("/regions/0/shape/elipse"));
        assert_eq!(e.fix.as_deref(), Some("did you mean \"ellipse\"?"));

        let mut v = minimal();
        v["sceneplan"] = serde_json::json!(2);
        assert_eq!(errs(v)[0].code, "UNSUPPORTED_SCENEPLAN");
    }

    #[test]
    fn semantic_errors_are_all_reported() {
        let mut v = minimal();
        v["styles"]["all"]["width"] = serde_json::json!([25, 40]);
        v["styles"]["sky"] = serde_json::json!({});
        v["regions"].as_array_mut().unwrap().push(serde_json::json!({"name": "all", "shape": {"polygon": [[0, 0], [1, 1]]}}));
        v["target"].as_array_mut().unwrap().push(serde_json::json!({"blob": {"c": [0.5, 0.5], "r": [0.1, 0.1], "color": "cobalt_bleu"}}));
        let e = errs(v);
        let codes: Vec<(&str, &str)> = e.iter().map(|e| (e.code, e.path.as_deref().unwrap_or(""))).collect();
        assert!(codes.contains(&("UNITS", "/styles/all/width/0")), "{codes:?}");
        assert!(codes.contains(&("UNKNOWN_REGION", "/styles/sky")), "{codes:?}");
        assert!(codes.contains(&("DUPLICATE_NAME", "/regions/1/name")), "{codes:?}");
        assert!(codes.contains(&("RANGE", "/regions/1/shape/polygon")), "{codes:?}");
        assert!(codes.contains(&("UNKNOWN_COLOR", "/target/1/blob/color")), "{codes:?}");
    }
}
