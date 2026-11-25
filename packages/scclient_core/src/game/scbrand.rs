//! Puts the SC Client logo and name into the in-game menu mod (ChromePoly).
//!
//! The menu mod (based on PolyPlus by Polyfrost, GPL-3.0-only) draws its logo from a few SVG / PNG files inside its jar.
//! This swaps those files in the launcher's downloaded copy for the SC ones in `brand/`, and changes the visible text
//! "Chrome Client" / "OneClient" to "SC Client", and "Chrome" / "Polyfrost" / "PolyPlus" to "SC" (all-caps versions too), in
//! the classes and in the language files. Only plain text is changed - never names,
//! descriptors, file names or web addresses - and not the classes that detect which launcher is in use.
//! The jar gets a stamp entry when it's done, so a jar that is already branded is left alone.

use std::path::Path;

use async_zip::{Compression, ZipEntryBuilder};

const STAMP: &str = "scbrand.stamp";
const VERSION: &[u8] = b"sc-brand 4";

const FILES: &[(&str, &[u8])] = &[
    ("assets/polyplus/mainmenu/logo.svg", include_bytes!("brand/mark.svg")),
    ("assets/polyplus/mainmenu/footer-logo.svg", include_bytes!("brand/mark.svg")),
    ("assets/polyplus/brand/oneclient.svg", include_bytes!("brand/wordmark.svg")),
    ("assets/polyplus/startup/mark_64.png", include_bytes!("brand/mark_64.png")),
    ("assets/polyplus/startup/mark_128.png", include_bytes!("brand/mark_128.png")),
    ("assets/polyplus/startup/mark_256.png", include_bytes!("brand/mark_256.png")),
    ("assets/polyplus/startup/word_54.png", include_bytes!("brand/word_54.png")),
    ("assets/polyplus/startup/word_108.png", include_bytes!("brand/word_108.png")),
    ("assets/polyplus/startup/word_216.png", include_bytes!("brand/word_216.png")),
    // the badge in front of SC Client players' names (was the OneClient star)
    ("assets/polyplus/textures/badge_font.png", include_bytes!("brand/badge_font.png")),
];

/// Classes that compare the name with something (launcher / window-title detection): their text stays as it is
const KEEP_CLASSES: &[&str] = &["LauncherEnvironment", "OneClientWindowTitle"];
const NAMES: &[(&str, &str)] = &[
    ("Chrome Client", "SC Client"),
    ("CHROME CLIENT", "SC CLIENT"),
    ("OneClient", "SC Client"),
    ("ONECLIENT", "SC CLIENT"),
    ("PolyPlus", "SC Plus"),
    ("Polyfrost", "SC"),
    ("POLYFROST", "SC"),
    ("Chrome", "SC"),
    ("CHROME", "SC"),
];

fn is_word_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_alphanumeric())
}

/// Replaces whole-word occurrences of the old names
fn rename(text: &str) -> Option<String> {
    if text.contains("://") || text.contains(['/', '_', '@', '=', '\\']) || text.contains("http") {
        return None;
    }
    if !text.contains(' ') && text.contains('.') {
        return None;
    }
    let mut out = text.to_string();
    for (from, to) in NAMES {
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(at) = rest.find(from) {
            let before = rest[..at].chars().next_back();
            let after = rest[at + from.len()..].chars().next();
            result.push_str(&rest[..at]);
            if is_word_char(before) || is_word_char(after) {
                result.push_str(from);
            } else {
                result.push_str(to);
            }
            rest = &rest[at + from.len()..];
        }
        result.push_str(rest);
        out = result;
    }
    (out != text).then_some(out)
}

/// Renames the text in a language file (the values, never the keys)
fn patch_lang(data: &[u8]) -> Option<Vec<u8>> {
    let mut map: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(data).ok()?;
    let mut changed = false;
    for value in map.values_mut() {
        if let serde_json::Value::String(text) = value
            && let Some(new) = rename(text)
        {
            *text = new;
            changed = true;
        }
    }
    changed.then(|| serde_json::to_vec_pretty(&map).ok()).flatten()
}

fn u16_at(data: &[u8], pos: usize) -> Option<usize> {
    Some(u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]) as usize)
}

/// Rewrites the text strings in a .class file's constant pool; None when nothing changes (or the layout is unknown)
pub(crate) fn patch_class(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 10 || data[..4] != [0xca, 0xfe, 0xba, 0xbe] {
        return None;
    }
    if !NAMES.iter().any(|(from, _)| data.windows(from.len()).any(|w| w == from.as_bytes())) {
        return None;
    }
    let count = u16_at(data, 8)?;
    let (mut pos, mut i) = (10usize, 1usize);
    let mut utf: Vec<(usize, usize, usize)> = Vec::new(); // (index, start, end)
    let mut text_refs = std::collections::HashSet::new();
    let mut name_refs = std::collections::HashSet::new();
    while i < count {
        let tag = *data.get(pos)?;
        match tag {
            1 => {
                let n = u16_at(data, pos + 1)?;
                if pos + 3 + n > data.len() {
                    return None;
                }
                utf.push((i, pos, pos + 3 + n));
                pos += 3 + n;
            }
            3 | 4 | 9 | 10 | 11 | 17 | 18 => pos += 5,
            5 | 6 => {
                pos += 9;
                i += 1;
            }
            8 => {
                text_refs.insert(u16_at(data, pos + 1)?);
                pos += 3;
            }
            7 | 16 | 19 | 20 => {
                name_refs.insert(u16_at(data, pos + 1)?);
                pos += 3;
            }
            12 => {
                name_refs.insert(u16_at(data, pos + 1)?);
                name_refs.insert(u16_at(data, pos + 3)?);
                pos += 5;
            }
            15 => pos += 4,
            _ => return None,
        }
        i += 1;
    }
    let mut out = Vec::with_capacity(data.len() + 64);
    let mut last = 0usize;
    let mut changed = false;
    for (index, start, end) in utf {
        let raw = &data[start + 3..end];
        // a text constant, or any string with a space (settings labels live in annotations; code names never contain spaces)
        if name_refs.contains(&index) || (!text_refs.contains(&index) && !raw.contains(&b' ')) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(raw) else { continue };
        let Some(new) = rename(text) else { continue };
        let bytes = new.as_bytes();
        if bytes.len() > u16::MAX as usize {
            continue;
        }
        out.extend_from_slice(&data[last..start]);
        out.push(1);
        out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        out.extend_from_slice(bytes);
        last = end;
        changed = true;
    }
    if !changed {
        return None;
    }
    out.extend_from_slice(&data[last..]);
    Some(out)
}

/// Brands the jar at `path` in place. Returns true when it was changed.
#[allow(deprecated)] // the in-memory reader is fine for one jar; scio uses the same module
pub(crate) async fn brand_jar(path: &Path) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let data = tokio::fs::read(path).await?;
    let reader = async_zip::base::read::mem::ZipFileReader::new(data).await?;
    let entries: Vec<_> = reader.file().entries().to_vec();

    for (index, entry) in entries.iter().enumerate() {
        if entry.filename().as_bytes() == STAMP.as_bytes() {
            let mut buf = Vec::new();
            reader.reader_with_entry(index).await?.read_to_end_checked(&mut buf).await?;
            if buf == VERSION {
                return Ok(false);
            }
        }
    }

    let tmp = path.with_extension("jar.brand");
    let file = tokio::fs::File::create(&tmp).await?;
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        for (index, entry) in entries.iter().enumerate() {
            let name = String::from_utf8_lossy(entry.filename().as_bytes()).to_string();
            if name == STAMP {
                continue;
            }
            let mut buf = Vec::new();
            reader.reader_with_entry(index).await?.read_to_end_checked(&mut buf).await?;
            let data = if let Some((_, bytes)) = FILES.iter().find(|(n, _)| *n == name) {
                bytes.to_vec()
            } else if name.starts_with("assets/polyplus/lang/") && name.ends_with(".json") {
                patch_lang(&buf).unwrap_or(buf)
            } else if name.ends_with(".class") && !KEEP_CLASSES.iter().any(|k| name.contains(k)) {
                patch_class(&buf).unwrap_or(buf)
            } else {
                buf
            };
            let compression = if entry.compression() == Compression::Stored || name.ends_with('/') {
                Compression::Stored
            } else {
                Compression::Deflate
            };
            writer.write_entry_whole(ZipEntryBuilder::new(name.into(), compression), &data).await?;
        }
        writer
            .write_entry_whole(ZipEntryBuilder::new(STAMP.to_string().into(), Compression::Stored), VERSION)
            .await?;
        writer.close().await?;
        Ok(())
    }
    .await;
    if let Err(err) = result {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(err);
    }
    tokio::fs::rename(&tmp, path).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs on a real jar when SCBRAND_TEST_JAR points at a copy of one (it is changed in place)
    #[tokio::test]
    async fn brands_a_real_jar() {
        let Some(jar) = std::env::var_os("SCBRAND_TEST_JAR") else { return };
        let jar = std::path::PathBuf::from(jar);
        assert!(brand_jar(&jar).await.unwrap(), "first run should change the jar");
        assert!(!brand_jar(&jar).await.unwrap(), "second run should skip it");
    }

    #[test]
    fn renames_whole_words_only() {
        assert_eq!(rename("Chrome Client v1").as_deref(), Some("SC Client v1"));
        assert_eq!(rename("CHROME CLIENT").as_deref(), Some("SC CLIENT"));
        assert_eq!(rename("Chrome Client's settings: \u{1}").as_deref(), Some("SC Client's settings: \u{1}"));
        assert_eq!(rename("OneClients"), None);
        assert_eq!(rename("https://polyfrost.org OneClient"), None);
        assert_eq!(rename("org.polyfrost.OneClient"), None);
        assert_eq!(rename("Join the Polyfrost Discord").as_deref(), Some("Join the SC Discord"));
        assert_eq!(rename("Chrome User Indicators").as_deref(), Some("SC User Indicators"));
        assert_eq!(rename("Couldn't sign in to Chrome.").as_deref(), Some("Couldn't sign in to SC."));
        assert_eq!(rename("Polyfrost/app"), None);
        assert_eq!(rename("chromeclient"), None);
    }

    #[test]
    fn renames_language_values_not_keys() {
        let out = patch_lang(br#"{"polyplus.mmc.note":"Chrome note: hi","x.chrome":"plain"}"#).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["polyplus.mmc.note"], "SC note: hi");
        assert_eq!(v["x.chrome"], "plain");
    }
}
