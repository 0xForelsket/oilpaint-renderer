//! StrokeList v2 codec (spec/STROKELIST_V2.md).
//!
//! The same stroke list always serialises to the same bytes. The frozen 64-byte header carries the engine version,
//! and a reader refuses any other version (`ENGINE_VERSION_MISMATCH`) and v1's `.npz` files (`V1_STROKE_FILE`)
//! before it parses anything else.
#![forbid(unsafe_code)]
// Errors are structured, agent-readable values (spec/ERRORS.md) returned on cold paths only.
#![allow(clippy::result_large_err)]

use oil_kernel::BrushParams;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

pub const MAGIC: [u8; 8] = [0x89, b'O', b'I', b'L', 0x0D, 0x0A, 0x1A, 0x0A];
pub const GENERATION: u16 = 2;
pub const HEADER_BYTES: usize = 64;
pub const MAX_VERSION_BYTES: usize = 51;
pub const NO_REGION: u32 = u32::MAX;
/// Fields (4 bytes each) of one `STRK` record.
pub const STROKE_FIELDS: usize = 36;
const LAYER_FIELDS: usize = 6;
const SECTIONS: [&[u8; 4]; 7] = [b"META", b"CANV", b"LAYR", b"OFFS", b"PNTS", b"STRK", b"END "];

// ------------------------------------------------------------------ data

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanMeta {
    pub mixer: String,
    pub plan_width: u32,
    pub seed: u32,
    pub portable: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inputs {
    pub sceneplan: Option<String>,
    pub fields: BTreeMap<String, String>,
    pub hooks: Vec<String>,
}

/// Provenance, stored as canonical JSON. Nothing host- or time-dependent may go here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Meta {
    pub generator: String,
    pub title: Option<String>,
    pub plan: Option<PlanMeta>,
    pub inputs: Inputs,
    pub layers: Vec<String>,
    pub regions: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub start: u32,
    pub end: u32,
    /// Blur the height plane with this sigma (cw) before the layer (needed by scumble strokes).
    pub hblur_sigma: Option<f32>,
    /// Multiply wetness by this factor after the layer.
    pub dry_after: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub layer: u32,
    pub region: u32,
    /// Main load, authored sRGB in [0, 1].
    pub color: [f32; 3],
    /// Second load for `marble` (equal to `color` when there is none).
    pub color2: [f32; 3],
    pub streak_amount: f32,
    /// Brush parameters, including `mode` and `seed`.
    pub brush: BrushParams,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeList {
    pub meta: Meta,
    pub aspect: [u32; 2],
    pub ground: [f32; 3],
    pub layers: Vec<Layer>,
    /// Point offsets, one more than the stroke count.
    pub offsets: Vec<u32>,
    /// (x, y, width, pressure) in canvas-width units.
    pub points: Vec<[f32; 4]>,
    pub strokes: Vec<Stroke>,
}

impl StrokeList {
    /// Canvas height for width `w`: round(w * aspectH / aspectW), half up, in integers.
    pub fn height_for(&self, w: u32) -> u32 {
        let [aw, ah] = self.aspect.map(|v| v as u64);
        ((2 * w as u64 * ah + aw) / (2 * aw)) as u32
    }

    pub fn stroke_points(&self, i: usize) -> &[[f32; 4]] {
        &self.points[self.offsets[i] as usize..self.offsets[i + 1] as usize]
    }

    /// Serialise with the running engine's version.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes_with_version(oil_kernel::ENGINE_VERSION)
    }

    /// Serialise with the given engine version string (1-51 ASCII bytes of [0-9A-Za-z.+-]).
    pub fn to_bytes_with_version(&self, version: &str) -> Vec<u8> {
        assert!(valid_version(version), "invalid engine version string {version:?}");
        let mut out = Vec::with_capacity(HEADER_BYTES + 64 + self.points.len() * 16 + self.strokes.len() * STROKE_FIELDS * 4);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&GENERATION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.push(version.len() as u8);
        out.extend_from_slice(version.as_bytes());
        out.resize(HEADER_BYTES, 0);

        section(&mut out, b"META", &serde_json::to_vec(&self.meta).expect("META serialises"));
        let mut canv = Vec::with_capacity(24);
        put_u32(&mut canv, self.aspect[0]);
        put_u32(&mut canv, self.aspect[1]);
        self.ground.iter().for_each(|v| put_f32(&mut canv, *v));
        put_u32(&mut canv, 0);
        section(&mut out, b"CANV", &canv);
        let mut layr = Vec::with_capacity(self.layers.len() * LAYER_FIELDS * 4);
        for l in &self.layers {
            put_u32(&mut layr, l.start);
            put_u32(&mut layr, l.end);
            put_u32(&mut layr, l.hblur_sigma.is_some() as u32 | (l.dry_after.is_some() as u32) << 1);
            put_f32(&mut layr, l.hblur_sigma.unwrap_or(0.0));
            put_f32(&mut layr, l.dry_after.unwrap_or(0.0));
            put_u32(&mut layr, 0);
        }
        section(&mut out, b"LAYR", &layr);
        let mut offs = Vec::with_capacity(self.offsets.len() * 4);
        self.offsets.iter().for_each(|v| put_u32(&mut offs, *v));
        section(&mut out, b"OFFS", &offs);
        let mut pnts = Vec::with_capacity(self.points.len() * 16);
        self.points.iter().flatten().for_each(|v| put_f32(&mut pnts, *v));
        section(&mut out, b"PNTS", &pnts);
        let mut strk = Vec::with_capacity(self.strokes.len() * STROKE_FIELDS * 4);
        for s in &self.strokes {
            encode_stroke(&mut strk, s);
        }
        section(&mut out, b"STRK", &strk);
        let digest = Sha256::digest(&out);
        section(&mut out, b"END ", &digest);
        out
    }

    /// Parse and validate a StrokeList written by the running engine version.
    pub fn from_bytes(bytes: &[u8]) -> Result<StrokeList, Vec<Error>> {
        Self::from_bytes_with_version(bytes, oil_kernel::ENGINE_VERSION)
    }

    /// Parse and validate a StrokeList, accepting only `version`.
    pub fn from_bytes_with_version(bytes: &[u8], version: &str) -> Result<StrokeList, Vec<Error>> {
        let file_version = read_version(bytes).map_err(|e| vec![e])?;
        if file_version != version {
            return Err(vec![Error::new(
                "ENGINE_VERSION_MISMATCH",
                format!("This stroke list was written by engine {file_version}; this is engine {version}."),
            )
            .path("/header/engineVersion")
            .got(&file_version)
            .expected(version)
            .fix(format!(
                "Re-plan from the ScenePlan with this engine, or paint it with a package that ships engine {file_version} \
                 (see spec/ENGINE_VERSIONS.md)."
            ))]);
        }
        let list = parse_sections(bytes).map_err(|e| vec![e])?;
        let problems = list.validate();
        if problems.is_empty() {
            Ok(list)
        } else {
            Err(problems)
        }
    }

    /// Every range and consistency rule of the spec; returns all violations.
    pub fn validate(&self) -> Vec<Error> {
        let mut v = Vec::new();
        let n = self.strokes.len();
        let bad = |v: &mut Vec<Error>, path: String, got: String, expected: &str| {
            if v.len() < 200 {
                v.push(Error::new("INVALID_STROKELIST", format!("{path} is out of range")).path(path).got(got).expected(expected));
            }
        };
        if self.aspect[0] == 0 || self.aspect[1] == 0 {
            bad(&mut v, "/canvas/aspect".into(), format!("{:?}", self.aspect), "positive integers");
        }
        for (c, g) in self.ground.iter().enumerate() {
            if !(0.0..=1.0).contains(g) {
                bad(&mut v, format!("/canvas/ground/{c}"), g.to_string(), "[0, 1]");
            }
        }
        if self.meta.layers.len() != self.layers.len() {
            bad(&mut v, "/meta/layers".into(), self.meta.layers.len().to_string(), &format!("{} layer names", self.layers.len()));
        }
        let mut next = 0u32;
        for (i, l) in self.layers.iter().enumerate() {
            if l.start != next || l.end < l.start {
                bad(&mut v, format!("/layers/{i}"), format!("[{}, {})", l.start, l.end), &format!("contiguous from {next}"));
            }
            next = l.end;
            if let Some(s) = l.hblur_sigma {
                if !(s.is_finite() && s > 0.0) {
                    bad(&mut v, format!("/layers/{i}/hblurSigma"), s.to_string(), "> 0");
                }
            }
            if let Some(d) = l.dry_after {
                if !(0.0..=1.0).contains(&d) {
                    bad(&mut v, format!("/layers/{i}/dryAfter"), d.to_string(), "[0, 1]");
                }
            }
        }
        if next as usize != n {
            bad(&mut v, "/layers".into(), next.to_string(), &format!("layers covering all {n} strokes"));
        }
        if self.offsets.len() != n + 1 || self.offsets.first() != Some(&0) || self.offsets.last().copied() != Some(self.points.len() as u32) {
            bad(&mut v, "/offsets".into(), format!("{} offsets", self.offsets.len()), "stroke count + 1, from 0 to the point count");
            return v;
        }
        for i in 0..n {
            if self.offsets[i + 1] < self.offsets[i] + 2 {
                bad(&mut v, format!("/strokes/{i}/points"), (self.offsets[i + 1] as i64 - self.offsets[i] as i64).to_string(), ">= 2 points");
            }
        }
        for (i, p) in self.points.iter().enumerate() {
            if !(p[0].is_finite() && p[1].is_finite()) {
                bad(&mut v, format!("/points/{i}"), format!("{p:?}"), "finite x, y");
            }
            if !(p[2].is_finite() && p[2] > 0.0) {
                bad(&mut v, format!("/points/{i}/width"), p[2].to_string(), "> 0");
            }
            if !(0.0..=1.0).contains(&p[3]) {
                bad(&mut v, format!("/points/{i}/pressure"), p[3].to_string(), "[0, 1]");
            }
        }
        let n_regions = self.meta.regions.len() as u32;
        for (i, s) in self.strokes.iter().enumerate() {
            let layer_ok = (s.layer as usize) < self.layers.len()
                && self.layers[s.layer as usize].start as usize <= i
                && i < self.layers[s.layer as usize].end as usize;
            if !layer_ok {
                bad(&mut v, format!("/strokes/{i}/layer"), s.layer.to_string(), "the layer whose range holds the stroke");
            }
            if s.region != NO_REGION && s.region >= n_regions {
                bad(&mut v, format!("/strokes/{i}/region"), s.region.to_string(), &format!("< {n_regions} or none"));
            }
            for (c, x) in s.color.iter().chain(&s.color2).enumerate() {
                if !(0.0..=1.0).contains(x) {
                    bad(&mut v, format!("/strokes/{i}/{}/{}", if c < 3 { "color" } else { "color2" }, c % 3), x.to_string(), "[0, 1]");
                }
            }
            let b = &s.brush;
            if !(0..=3).contains(&b.mode) {
                bad(&mut v, format!("/strokes/{i}/mode"), b.mode.to_string(), "0-3");
            }
            if !(2..=68).contains(&b.nb) {
                bad(&mut v, format!("/strokes/{i}/nb"), b.nb.to_string(), "2-68");
            }
            let unit = [("opacity", b.opacity), ("pickup", b.pickup), ("flatten", b.flatten), ("hardness", b.hardness), ("dropout", b.dropout),
                        ("body", b.body), ("release", b.release), ("streakMix", b.streak_mix), ("marble", b.marble)];
            for (name, x) in unit {
                if !(0.0..=1.0).contains(&x) {
                    bad(&mut v, format!("/strokes/{i}/{name}"), x.to_string(), "[0, 1]");
                }
            }
            let nonneg = [("streakAmount", s.streak_amount), ("load", b.load), ("deplete", b.deplete), ("hgain", b.hgain), ("streak", b.streak),
                          ("grain", b.grain), ("dryThresh", b.dry_thresh), ("ragged", b.ragged), ("ridge", b.ridge), ("levee", b.levee),
                          ("furrow", b.furrow), ("blob", b.blob), ("stiff", b.stiff)];
            for (name, x) in nonneg {
                if !(x.is_finite() && x >= 0.0) {
                    bad(&mut v, format!("/strokes/{i}/{name}"), x.to_string(), ">= 0");
                }
            }
            for (name, x) in [("vdry", b.vdry), ("dryWidth", b.dry_width)] {
                if !(x.is_finite() && x > 0.0) {
                    bad(&mut v, format!("/strokes/{i}/{name}"), x.to_string(), "> 0");
                }
            }
            if !(0.0..=3.0).contains(&b.splay) {
                bad(&mut v, format!("/strokes/{i}/splay"), b.splay.to_string(), "[0, 3]");
            }
        }
        v
    }
}

// ------------------------------------------------------------------ errors

/// A structured error (spec/ERRORS.md).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub got: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Error { code, message: message.into(), path: None, got: None, expected: None, fix: None }
    }
    pub fn path(mut self, p: impl Into<String>) -> Self {
        self.path = Some(p.into());
        self
    }
    pub fn got(mut self, g: impl fmt::Display) -> Self {
        self.got = Some(g.to_string());
        self
    }
    pub fn expected(mut self, e: impl fmt::Display) -> Self {
        self.expected = Some(e.to_string());
        self
    }
    pub fn fix(mut self, f: impl Into<String>) -> Self {
        self.fix = Some(f.into());
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let Some(fix) = &self.fix {
            write!(f, " {fix}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

// ------------------------------------------------------------------ reading

fn valid_version(v: &str) -> bool {
    (1..=MAX_VERSION_BYTES).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
}

/// Check the frozen header and return the engine version that wrote the file, without parsing anything else.
pub fn read_version(bytes: &[u8]) -> Result<String, Error> {
    if bytes.starts_with(b"PK\x03\x04") {
        return Err(Error::new("V1_STROKE_FILE", "This is a stroke file from the v1 Python renderer (strokes.npz), which this engine does not load.")
            .fix("Paint it with the v1 renderer: git checkout v1-python-c (Mixbox files), or re-plan the scene with this engine."));
    }
    if bytes.len() < HEADER_BYTES {
        return Err(Error::new("CORRUPT_STROKELIST", format!("File is {} bytes, shorter than the 64-byte header.", bytes.len())));
    }
    if bytes[..8] != MAGIC {
        return Err(Error::new("NOT_A_STROKELIST", "The file does not start with the StrokeList magic.").got(format!("{:02x?}", &bytes[..8])));
    }
    let generation = u16::from_le_bytes([bytes[8], bytes[9]]);
    if generation != GENERATION {
        return Err(Error::new("UNSUPPORTED_GENERATION", format!("StrokeList generation {generation} is not supported.")).got(generation).expected(GENERATION));
    }
    let n = bytes[12] as usize;
    let v = std::str::from_utf8(&bytes[13..13 + n.min(MAX_VERSION_BYTES)]).unwrap_or("");
    if !valid_version(v) || n > MAX_VERSION_BYTES || bytes[13 + n..HEADER_BYTES].iter().any(|b| *b != 0) {
        return Err(Error::new("CORRUPT_STROKELIST", "The engine version in the header is malformed."));
    }
    Ok(v.to_string())
}

struct Cursor<'a> {
    b: &'a [u8],
    o: usize,
}

impl<'a> Cursor<'a> {
    fn section(&mut self, tag: &[u8; 4]) -> Result<&'a [u8], Error> {
        let corrupt = |m: String| Error::new("CORRUPT_STROKELIST", m);
        if self.b.len() < self.o + 8 {
            return Err(corrupt(format!("Truncated before section {}.", String::from_utf8_lossy(tag))));
        }
        if &self.b[self.o..self.o + 4] != tag {
            return Err(corrupt(format!(
                "Expected section {} at byte {}, found {}.",
                String::from_utf8_lossy(tag),
                self.o,
                String::from_utf8_lossy(&self.b[self.o..self.o + 4])
            )));
        }
        let len = u32::from_le_bytes(self.b[self.o + 4..self.o + 8].try_into().unwrap()) as usize;
        let start = self.o + 8;
        let end = start.checked_add(len).filter(|e| *e <= self.b.len()).ok_or_else(|| corrupt(format!("Section {} overruns the file.", String::from_utf8_lossy(tag))))?;
        let padded = end + (4 - len % 4) % 4;
        if padded > self.b.len() || self.b[end..padded].iter().any(|b| *b != 0) {
            return Err(corrupt(format!("Bad padding after section {}.", String::from_utf8_lossy(tag))));
        }
        self.o = padded;
        Ok(&self.b[start..end])
    }
}

fn u32s(b: &[u8]) -> impl Iterator<Item = u32> + '_ {
    b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap()))
}

fn f32s(b: &[u8]) -> impl Iterator<Item = f32> + '_ {
    u32s(b).map(f32::from_bits)
}

fn parse_sections(bytes: &[u8]) -> Result<StrokeList, Error> {
    let corrupt = |m: &str| Error::new("CORRUPT_STROKELIST", m.to_string());
    let mut c = Cursor { b: bytes, o: HEADER_BYTES };
    let meta = c.section(SECTIONS[0])?;
    let canv = c.section(SECTIONS[1])?;
    let layr = c.section(SECTIONS[2])?;
    let offs = c.section(SECTIONS[3])?;
    let pnts = c.section(SECTIONS[4])?;
    let strk = c.section(SECTIONS[5])?;
    let hashed_end = c.o;
    let end = c.section(SECTIONS[6])?;
    if c.o != bytes.len() {
        return Err(corrupt("Trailing bytes after the END section."));
    }
    if end != Sha256::digest(&bytes[..hashed_end]).as_slice() {
        return Err(corrupt("The SHA-256 in the END section does not match the file."));
    }
    if canv.len() != 24 || layr.len() % (LAYER_FIELDS * 4) != 0 || offs.len() % 4 != 0 || pnts.len() % 16 != 0 || strk.len() % (STROKE_FIELDS * 4) != 0 {
        return Err(corrupt("A section length does not match its record size."));
    }
    let meta: Meta = serde_json::from_slice(meta).map_err(|e| {
        Error::new("INVALID_STROKELIST", format!("META is not valid StrokeList metadata: {e}")).path("/meta")
    })?;
    let cv: Vec<u32> = u32s(canv).collect();
    let layers = layr
        .chunks_exact(LAYER_FIELDS * 4)
        .map(|r| {
            let w: Vec<u32> = u32s(r).collect();
            Layer {
                start: w[0],
                end: w[1],
                hblur_sigma: (w[2] & 1 != 0).then(|| f32::from_bits(w[3])),
                dry_after: (w[2] & 2 != 0).then(|| f32::from_bits(w[4])),
            }
        })
        .collect();
    let points = f32s(pnts).collect::<Vec<_>>().chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect();
    let strokes = strk.chunks_exact(STROKE_FIELDS * 4).map(decode_stroke).collect();
    Ok(StrokeList {
        meta,
        aspect: [cv[0], cv[1]],
        ground: [f32::from_bits(cv[2]), f32::from_bits(cv[3]), f32::from_bits(cv[4])],
        layers,
        offsets: u32s(offs).collect(),
        points,
        strokes,
    })
}

// ------------------------------------------------------------------ records

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_bits().to_le_bytes());
}

fn section(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(tag);
    put_u32(out, payload.len() as u32);
    out.extend_from_slice(payload);
    out.resize(out.len() + (4 - payload.len() % 4) % 4, 0);
}

fn encode_stroke(out: &mut Vec<u8>, s: &Stroke) {
    let b = &s.brush;
    put_u32(out, s.layer);
    put_u32(out, s.region);
    put_u32(out, b.seed);
    put_u32(out, b.mode as u32);
    s.color.iter().chain(&s.color2).for_each(|v| put_f32(out, *v));
    put_f32(out, s.streak_amount);
    for v in [b.opacity, b.pickup, b.load, b.deplete, b.vdry, b.hgain, b.flatten, b.streak, b.hardness, b.grain, b.dry_thresh, b.dry_width] {
        put_f32(out, v);
    }
    put_u32(out, b.nb as u32);
    for v in [b.dropout, b.ragged, b.body, b.release, b.streak_mix, b.ridge, b.levee, b.furrow, b.blob, b.stiff, b.marble, b.splay] {
        put_f32(out, v);
    }
}

fn decode_stroke(r: &[u8]) -> Stroke {
    let w: Vec<u32> = u32s(r).collect();
    let f = |i: usize| f32::from_bits(w[i]);
    Stroke {
        layer: w[0],
        region: w[1],
        color: [f(4), f(5), f(6)],
        color2: [f(7), f(8), f(9)],
        streak_amount: f(10),
        brush: BrushParams {
            seed: w[2],
            mode: w[3] as i32,
            opacity: f(11),
            pickup: f(12),
            load: f(13),
            deplete: f(14),
            vdry: f(15),
            hgain: f(16),
            flatten: f(17),
            streak: f(18),
            hardness: f(19),
            grain: f(20),
            dry_thresh: f(21),
            dry_width: f(22),
            nb: w[23] as i32,
            dropout: f(24),
            ragged: f(25),
            body: f(26),
            release: f(27),
            streak_mix: f(28),
            ridge: f(29),
            levee: f(30),
            furrow: f(31),
            blob: f(32),
            stiff: f(33),
            marble: f(34),
            splay: f(35),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StrokeList {
        let brush = BrushParams { seed: 42, ..BrushParams::default() };
        let stroke = |layer, c: f32| Stroke { layer, region: 0, color: [c, 0.5, 0.25], color2: [c, 0.5, 0.25], streak_amount: 1.0, brush };
        StrokeList {
            meta: Meta {
                generator: format!("oilpaint-engine {}", oil_kernel::ENGINE_VERSION),
                title: Some("test".into()),
                plan: None,
                inputs: Inputs::default(),
                layers: vec!["a".into(), "b".into()],
                regions: vec!["sky".into()],
            },
            aspect: [4, 5],
            ground: [0.9, 0.88, 0.84],
            layers: vec![
                Layer { start: 0, end: 1, hblur_sigma: None, dry_after: Some(0.3) },
                Layer { start: 1, end: 2, hblur_sigma: Some(0.01), dry_after: None },
            ],
            offsets: vec![0, 2, 5],
            points: vec![[0.1, 0.2, 0.03, 1.0], [0.4, 0.2, 0.03, 0.8], [0.2, 0.5, 0.02, 1.0], [0.3, 0.6, 0.02, 1.0], [0.4, 0.62, 0.02, 0.5]],
            strokes: vec![stroke(0, 0.1), stroke(1, 0.9)],
        }
    }

    #[test]
    fn round_trips_to_identical_bytes() {
        let s = sample();
        let b = s.to_bytes();
        assert_eq!(b.len() % 4, 0);
        let back = StrokeList::from_bytes(&b).expect("valid");
        assert_eq!(back, s);
        assert_eq!(back.to_bytes(), b);
        assert_eq!(s.height_for(600), 750);
        assert_eq!(s.height_for(1001), 1251);
    }

    #[test]
    fn version_gate_and_v1_files_are_refused() {
        let b = sample().to_bytes_with_version("2.0.0-dev.99");
        let e = StrokeList::from_bytes(&b).unwrap_err();
        assert_eq!(e[0].code, "ENGINE_VERSION_MISMATCH");
        assert_eq!(e[0].got.as_deref(), Some("2.0.0-dev.99"));
        assert_eq!(read_version(&b).unwrap(), "2.0.0-dev.99");
        assert_eq!(StrokeList::from_bytes(b"PK\x03\x04rest of an npz").unwrap_err()[0].code, "V1_STROKE_FILE");
        assert_eq!(StrokeList::from_bytes(&[0u8; 80]).unwrap_err()[0].code, "NOT_A_STROKELIST");
        assert_eq!(StrokeList::from_bytes(&MAGIC).unwrap_err()[0].code, "CORRUPT_STROKELIST");
    }

    #[test]
    fn corruption_and_bad_values_are_reported() {
        let mut b = sample().to_bytes();
        let i = b.len() - 40;
        b[i] ^= 1;
        assert_eq!(StrokeList::from_bytes(&b).unwrap_err()[0].code, "CORRUPT_STROKELIST");
        let mut s = sample();
        s.strokes[1].brush.opacity = 1.5;
        s.points[3][3] = -0.1;
        s.layers[0].end = 2;
        let codes: Vec<_> = StrokeList::from_bytes(&s.to_bytes()).unwrap_err().iter().map(|e| e.path.clone().unwrap()).collect();
        assert!(codes.contains(&"/strokes/1/opacity".to_string()), "{codes:?}");
        assert!(codes.contains(&"/points/3/pressure".to_string()), "{codes:?}");
        assert!(codes.contains(&"/layers/1".to_string()), "{codes:?}");
    }
}
