use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct VoiceEmbedding {
    pub name: String,
    pub data: Vec<f32>,
}

/// Read voices artifact and return map of voice name -> embedding.
/// For now, stub: if file exists, read as binary and slice arbitrarily for tests.
/// Real implementation would parse the packed voices bin (28M) format.
pub fn load_voices(path: &Path) -> Result<HashMap<String, VoiceEmbedding>, String> {
    if !path.is_file() {
        return Err(format!("voices file not found: {}", path.display()));
    }
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() == 0 {
        return Err("voices file empty".to_string());
    }
    // Stub: create a single dummy voice "af_heart" with 256 floats
    let mut map = HashMap::new();
    map.insert(
        "af_heart".to_string(),
        VoiceEmbedding {
            name: "af_heart".to_string(),
            data: vec![0.0; 256],
        },
    );
    Ok(map)
}

pub fn select_voice<'a>(
    voices: &'a HashMap<String, VoiceEmbedding>,
    name: &str,
) -> Option<&'a VoiceEmbedding> {
    voices.get(name).or_else(|| voices.get("af_heart"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn load_missing_fails() {
        let res = load_voices(Path::new("/tmp/nonexistent_voices.bin"));
        assert!(res.is_err());
    }
    #[test]
    fn select_fallback() {
        let mut map = HashMap::new();
        map.insert(
            "af_heart".to_string(),
            VoiceEmbedding {
                name: "af_heart".to_string(),
                data: vec![0.0; 10],
            },
        );
        let v = select_voice(&map, "unknown");
        assert!(v.is_some());
        assert_eq!(v.unwrap().name, "af_heart");
    }
}
