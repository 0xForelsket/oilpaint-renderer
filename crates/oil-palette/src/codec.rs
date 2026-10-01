//! OPJ1: three u32-length-prefixed payloads (OPP1, OPL1, StrokeList), four f32
//! ground weights, u32 load count and 12 f32 per stroke, then SHA256 of all above.
//! OPJ2: u32 paint count and decoder=0 after magic, then the same framing with
//! an empty table section, N ground weights and 3*N f32 per stroke. Direct mode.
use super::*;
use sha2::{Digest, Sha256};
const MAX_BYTES: usize = 128 * 1024 * 1024;

impl<const N: usize, const PREPARED: bool, const B: usize> PaletteJobN<N, PREPARED, B> {
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut out = if PREPARED {
            b"OPJ1".to_vec()
        } else {
            b"OPJ2".to_vec()
        };
        if !PREPARED {
            out.extend_from_slice(&(N as u32).to_le_bytes());
            out.extend_from_slice(&0_u32.to_le_bytes()); // Direct spectral decoder.
        }
        for section in [
            self.mixer.palette().to_bytes(),
            self.mixer
                .prepared_table()
                .map(|t| t.to_bytes())
                .unwrap_or_default(),
            self.geometry.to_bytes(),
        ] {
            let len = u32::try_from(section.len())
                .map_err(|_| Error("Replay section too large".into()))?;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&section);
        }
        for v in self.ground {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let count =
            u32::try_from(self.loads.len()).map_err(|_| Error("Too many recipe loads".into()))?;
        out.extend_from_slice(&count.to_le_bytes());
        for l in &self.loads {
            for v in l.main.into_iter().chain(l.secondary).chain(l.dz) {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        if out.len() > MAX_BYTES - 32 {
            return Err(Error("Replay exceeds 128 MiB".into()));
        }
        let hash = Sha256::digest(&out);
        out.extend_from_slice(&hash);
        Ok(out)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if !(1..=oil_mix::palette::MAX_PAINTS).contains(&N)
            || bytes.len() < 68
            || bytes.len() > MAX_BYTES
            || (PREPARED && (N != 4 || B != 81 || &bytes[..4] != b"OPJ1"))
            || (!PREPARED && &bytes[..4] != b"OPJ2")
        {
            return Err(Error(
                "Invalid palette replay header, decoder or size".into(),
            ));
        }
        let end = bytes.len() - 32;
        if Sha256::digest(&bytes[..end])[..] != bytes[end..] {
            return Err(Error("Replay checksum mismatch".into()));
        }
        let mut reader = Reader {
            bytes: &bytes[4..end],
            at: 0,
        };
        if !PREPARED && (reader.u32()? as usize != N || reader.u32()? != 0) {
            return Err(Error("Replay paint count or decoder mismatch".into()));
        }
        let palette = reader.section()?;
        let table = reader.section()?;
        let geometry = StrokeList::from_bytes(reader.section()?)
            .map_err(|e| Error(format!("Invalid or incompatible stroke geometry: {e:?}")))?;
        let ground = reader.weights()?;
        let count = reader.u32()? as usize;
        if count != geometry.strokes.len()
            || count.checked_mul(12 * N) != Some(reader.bytes.len() - reader.at)
        {
            return Err(Error("Recipe load count or length mismatch".into()));
        }
        let mut loads = Vec::with_capacity(count);
        for _ in 0..count {
            loads.push(RecipeLoadN {
                main: reader.weights()?,
                secondary: reader.weights()?,
                dz: reader.weights()?,
            });
        }
        // Parse the LUT after all cheap framing/version checks.
        Self::new(
            PaletteMixerN::<N, PREPARED, B>::from_bytes(palette, table)?,
            geometry,
            ground,
            loads,
        )
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(n)
            .ok_or_else(|| Error("Replay length overflow".into()))?;
        let bytes = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| Error("Truncated replay".into()))?;
        self.at = end;
        Ok(bytes)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn section(&mut self) -> Result<&'a [u8], Error> {
        let n = self.u32()? as usize;
        self.take(n)
    }
    fn weights<const N: usize>(&mut self) -> Result<[f32; N], Error> {
        let mut v = [0.0; N];
        for x in &mut v {
            *x = f32::from_le_bytes(self.take(4)?.try_into().unwrap());
        }
        Ok(v)
    }
}
