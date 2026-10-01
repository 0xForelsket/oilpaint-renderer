use crate::{catalog, Document, Replay, View};
use oil_mix::{OchrellMixer, RgbMixer};
use serde_json::{json, Value};
#[derive(Default)]
pub struct Session {
    ochrell: Replay<OchrellMixer>,
    rgb: Replay<RgbMixer>,
    active: u32,
    pub pixels: Vec<u8>,
    pub strokes: Vec<u8>,
}
impl Session {
    pub fn call(&mut self, text: &str) -> Result<Value, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        match v["op"].as_str().unwrap_or("") {
            "catalog" => Ok(json!(catalog())),
            "schema" => Ok(json!(schemars::schema_for!(Document))),
            "validateCatalog" => {
                let c: crate::Catalog =
                    serde_json::from_value(v["catalog"].clone()).map_err(|e| e.to_string())?;
                c.validate()?;
                Ok(json!(c))
            }
            "compile" | "render" => {
                let list = if v["document"]["format"] == "oil-composition" {
                    let d: crate::Composition = serde_json::from_value(v["document"].clone()).map_err(|e|e.to_string())?;
                    d.compile()?
                } else {
                    let d: Document = serde_json::from_value(v["document"].clone()).map_err(|e| e.to_string())?;
                    d.compile()?
                };
                if v["op"] == "compile" {
                    self.strokes = list.to_bytes();
                    return Ok(json!({"bytes":self.strokes.len()}));
                }
                let width = v["width"]
                    .as_u64()
                    .filter(|w| *w <= 4096)
                    .ok_or("INVALID_WIDTH")? as u32;
                let mixer = match v["mixer"].as_str() {
                    Some("ochrell") => 0,
                    Some("rgb") => 1,
                    _ => return Err("UNKNOWN_MIXER".into()),
                };
                let height = list.height_for(width);
                let reused = if mixer == 0 {
                    self.ochrell.render(&OchrellMixer, list, width)?
                } else {
                    self.rgb.render(&RgbMixer, list, width)?
                };
                // Release the other mixer's canvases when switching.
                if mixer != self.active {
                    if mixer == 0 {
                        self.rgb = Replay::default()
                    } else {
                        self.ochrell = Replay::default()
                    }
                }
                self.active = mixer;
                Ok(json!({"width":width,"height":height,"reusedGroups":reused}))
            }
            "view" => {
                let view: View =
                    serde_json::from_value(v["view"].clone()).map_err(|e| e.to_string())?;
                let (w, h, p) = if self.active == 0 {
                    let c = self.ochrell.canvas.as_ref().ok_or("NO_CANVAS")?;
                    (c.w, c.h, crate::image(c, &view)?)
                } else {
                    let c = self.rgb.canvas.as_ref().ok_or("NO_CANVAS")?;
                    (c.w, c.h, crate::image(c, &view)?)
                };
                self.pixels = p;
                Ok(json!({"width":w,"height":h}))
            }
            "hashes" => {
                use sha2::{Digest, Sha256};
                let mut hashes = serde_json::Map::new();
                for name in oil_paint::PLANES {
                    let b = if self.active == 0 {
                        oil_paint::plane_bytes(
                            self.ochrell.canvas.as_ref().ok_or("NO_CANVAS")?,
                            name,
                        )
                    } else {
                        oil_paint::plane_bytes(self.rgb.canvas.as_ref().ok_or("NO_CANVAS")?, name)
                    };
                    hashes.insert(name.into(), json!(format!("{:x}", Sha256::digest(b))));
                }
                Ok(Value::Object(hashes))
            }
            _ => Err("UNKNOWN_AUTHOR_OPERATION".into()),
        }
    }
}
