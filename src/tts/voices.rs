use std::collections::HashMap;
use std::path::Path;

use crate::tts::phonemes::STYLE_DIM;

#[derive(Debug, Clone)]
pub struct VoiceBank {
    pub data: Vec<f32>,
    pub rows: usize,
}

#[derive(Debug, Clone)]
pub struct VoiceEmbedding {
    pub name: String,
    pub data: Vec<f32>,
}

/// Load all voice style banks from voices-v1.0.bin (npz zip bundle).
pub fn load_all_voices(path: &Path) -> Result<HashMap<String, VoiceBank>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(format!(
            "{} is not an npz bundle (expected ZIP magic)",
            path.display()
        ));
    }
    let entries = walk_stored_zip_entries(&bytes);
    let mut out = HashMap::new();
    for (name, method, range) in entries {
        if !name.ends_with(".npy") {
            continue;
        }
        if method != 0 {
            return Err(format!(
                "{name} in {} is deflate-compressed; only stored npz members supported",
                path.display()
            ));
        }
        let voice_name = name.trim_end_matches(".npy").to_string();
        let bank = parse_npy_f32(&bytes[range])?;
        out.insert(voice_name, bank);
    }
    if out.is_empty() {
        return Err(format!("no voice entries in {}", path.display()));
    }
    Ok(out)
}

pub fn select_voice_bank<'a>(
    voices: &'a HashMap<String, VoiceBank>,
    name: &str,
) -> Option<&'a VoiceBank> {
    voices.get(name).or_else(|| voices.get("af_heart"))
}

pub fn style_row(bank: &VoiceBank, n_phoneme_tokens: usize) -> Result<Vec<f32>, String> {
    let row = n_phoneme_tokens
        .min(crate::tts::phonemes::MAX_PHONEME_TOKENS)
        .min(bank.rows.saturating_sub(1));
    let start = row * STYLE_DIM;
    let end = start + STYLE_DIM;
    bank.data
        .get(start..end)
        .map(|s| s.to_vec())
        .ok_or_else(|| format!("style row {row} out of range"))
}

fn walk_stored_zip_entries(zip: &[u8]) -> Vec<(String, u16, std::ops::Range<usize>)> {
    const LOCAL_HEADER_SIG: u32 = 0x0403_4B50;
    const ZIP64_EXTRA_TAG: u16 = 0x0001;
    let mut entries = Vec::new();
    let mut pos = 0usize;
    while pos + 30 <= zip.len() {
        if u32_le(zip, pos) != Some(LOCAL_HEADER_SIG) {
            break;
        }
        let (Some(method), Some(csize32), Some(usize32), Some(name_len), Some(extra_len)) = (
            u16_le(zip, pos + 8),
            u32_le(zip, pos + 18),
            u32_le(zip, pos + 22),
            u16_le(zip, pos + 26),
            u16_le(zip, pos + 28),
        ) else {
            break;
        };
        let name_start = pos + 30;
        let Some(name_bytes) = zip.get(name_start..name_start + name_len as usize) else {
            break;
        };
        let extra_start = name_start + name_len as usize;
        let Some(extra) = zip.get(extra_start..extra_start + extra_len as usize) else {
            break;
        };
        let mut csize = u64::from(csize32);
        if csize32 == u32::MAX || usize32 == u32::MAX {
            let mut e = 0usize;
            while e + 4 <= extra.len() {
                let (Some(tag), Some(sz)) = (u16_le(extra, e), u16_le(extra, e + 2)) else {
                    break;
                };
                let body_start = e + 4;
                let Some(body) = extra.get(body_start..body_start + sz as usize) else {
                    break;
                };
                if tag == ZIP64_EXTRA_TAG {
                    let mut off = 0usize;
                    if usize32 == u32::MAX && body.len() >= off + 8 {
                        off += 8;
                    }
                    if csize32 == u32::MAX && body.len() >= off + 8 {
                        csize = u64_le(body, off).unwrap_or(0);
                    }
                }
                e = body_start + sz as usize;
            }
        }
        let Some(data_start) = extra_start.checked_add(extra_len as usize) else {
            break;
        };
        let Some(data_end) = data_start.checked_add(csize as usize) else {
            break;
        };
        if data_end > zip.len() {
            break;
        }
        if let Ok(name) = std::str::from_utf8(name_bytes) {
            entries.push((name.to_string(), method, data_start..data_end));
        }
        pos = data_end;
    }
    entries
}

fn parse_npy_f32(buf: &[u8]) -> Result<VoiceBank, String> {
    if !buf.starts_with(b"\x93NUMPY") {
        return Err("bad npy magic".to_string());
    }
    let Some(&major) = buf.get(6) else {
        return Err("truncated npy magic".to_string());
    };
    let (header_len, header_start) = match major {
        1 => {
            let Some(hb) = buf.get(8..10) else {
                return Err("truncated npy v1 header".to_string());
            };
            (usize::from(u16::from_le_bytes([hb[0], hb[1]])), 10_usize)
        }
        2 | 3 => {
            let Some(hb) = buf.get(8..12) else {
                return Err("truncated npy v2/v3 header".to_string());
            };
            (
                u32::from_le_bytes([hb[0], hb[1], hb[2], hb[3]]) as usize,
                12_usize,
            )
        }
        v => return Err(format!("unsupported npy version {v}")),
    };
    let header_end = header_start + header_len;
    let header = buf
        .get(header_start..header_end)
        .and_then(|h| std::str::from_utf8(h).ok())
        .ok_or_else(|| "truncated npy header".to_string())?;
    if !header.contains("'descr': '<f4'") {
        return Err(format!("expected npy descr '<f4', got: {}", header.trim()));
    }
    let shape_pos = header
        .find("'shape':")
        .ok_or_else(|| "npy header has no shape".to_string())?;
    let dims = parse_npy_shape(&header[shape_pos..])?;
    let rows = match dims.as_slice() {
        [d] if *d == STYLE_DIM => 1,
        [r, d] if *d == STYLE_DIM => *r,
        [r, one, d] if *one == 1 && *d == STYLE_DIM => *r,
        _ => return Err(format!("unexpected voice bank shape {dims:?}")),
    };
    if rows == 0 {
        return Err("voice bank has 0 style rows".to_string());
    }
    let total = rows
        .checked_mul(STYLE_DIM)
        .ok_or_else(|| format!("voice bank shape {dims:?} overflows"))?;
    let need = total
        .checked_mul(4)
        .ok_or_else(|| format!("voice bank shape {dims:?} overflows"))?;
    let data = &buf[header_end..];
    if data.len() < need {
        return Err(format!(
            "npy data truncated: need {need} bytes, have {}",
            data.len()
        ));
    }
    let mut floats = Vec::with_capacity(total);
    for chunk in data[..need].chunks_exact(4) {
        floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(VoiceBank { data: floats, rows })
}

fn parse_npy_shape(header_from_shape: &str) -> Result<Vec<usize>, String> {
    let open = header_from_shape
        .find('(')
        .ok_or_else(|| "malformed npy shape".to_string())?;
    let close = header_from_shape[open..]
        .find(')')
        .map(|i| open + i)
        .ok_or_else(|| "malformed npy shape".to_string())?;
    let inner = &header_from_shape[open + 1..close];
    let mut dims = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let d: usize = part
            .parse()
            .map_err(|_| format!("bad npy shape dim '{part}'"))?;
        dims.push(d);
    }
    Ok(dims)
}

fn u16_le(buf: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(buf.get(at..at + 2)?.try_into().ok()?))
}

fn u32_le(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(buf.get(at..at + 4)?.try_into().ok()?))
}

fn u64_le(buf: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(buf.get(at..at + 8)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_fails() {
        let res = load_all_voices(Path::new("/tmp/nonexistent_voices.bin"));
        assert!(res.is_err());
    }
}
