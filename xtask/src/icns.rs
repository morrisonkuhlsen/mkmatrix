//! `cargo xtask icns <out.icns> <dir.iconset>`: pack an iconset (the folder `iconutil` takes:
//! `icon_16x16.png`, `icon_16x16@2x.png` … `icon_512x512@2x.png`) into a macOS `.icns`, so the
//! icons can be regenerated off macOS too.
//!
//! Every entry is stored as PNG, which macOS reads for each of these types. A file the iconset
//! lacks is skipped; each one's size is checked against its name.

use std::path::Path;

/// The iconset's file names, the pixel size each holds, and its `.icns` entry type.
const ENTRIES: [(&str, u32, &[u8; 4]); 10] = [
    ("icon_16x16.png", 16, b"icp4"),
    ("icon_16x16@2x.png", 32, b"ic11"),
    ("icon_32x32.png", 32, b"icp5"),
    ("icon_32x32@2x.png", 64, b"ic12"),
    ("icon_128x128.png", 128, b"ic07"),
    ("icon_128x128@2x.png", 256, b"ic13"),
    ("icon_256x256.png", 256, b"ic08"),
    ("icon_256x256@2x.png", 512, b"ic14"),
    ("icon_512x512.png", 512, b"ic09"),
    ("icon_512x512@2x.png", 1024, b"ic10"),
];

/// An `.icns` holding `entries` (type, PNG bytes), in order.
pub fn pack(entries: &[(&[u8; 4], Vec<u8>)]) -> Result<Vec<u8>, String> {
    if entries.is_empty() {
        return Err("need at least one image".into());
    }
    let mut body = Vec::new();
    for (kind, png) in entries {
        let len = u32::try_from(png.len() + 8).map_err(|_| "image too large for .icns".to_string())?;
        body.extend_from_slice(*kind);
        body.extend_from_slice(&len.to_be_bytes());
        body.extend_from_slice(png);
    }
    let total = u32::try_from(body.len() + 8).map_err(|_| ".icns too large".to_string())?;
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&total.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn run(args: &[&str]) -> Result<(), String> {
    let [out, set] = args else {
        return Err("usage: cargo xtask icns <out.icns> <dir.iconset>".into());
    };
    let mut entries = Vec::new();
    for (name, size, kind) in ENTRIES {
        let path = Path::new(set).join(name);
        let Ok(png) = std::fs::read(&path) else { continue };
        let (w, h) = crate::ico::png_size(&png)?;
        if (w, h) != (size, size) {
            return Err(format!("{}: expected {size}x{size}, got {w}x{h}", path.display()));
        }
        entries.push((kind, png));
    }
    let icns = pack(&entries)?;
    std::fs::write(Path::new(out), &icns).map_err(|e| format!("write {out}: {e}"))?;
    println!("{out}: {} images, {} bytes", entries.len(), icns.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_typed_entries_with_lengths() {
        let a = b"\x89PNG one".to_vec();
        let b = b"\x89PNG two!".to_vec();
        let icns = pack(&[(b"icp4", a.clone()), (b"ic10", b.clone())]).unwrap();
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize, icns.len());
        assert_eq!(&icns[8..12], b"icp4");
        assert_eq!(u32::from_be_bytes(icns[12..16].try_into().unwrap()) as usize, a.len() + 8);
        assert_eq!(&icns[16..16 + a.len()], &a[..]);
        let at = 16 + a.len();
        assert_eq!(&icns[at..at + 4], b"ic10");
        assert_eq!(&icns[at + 8..], &b[..]);
        assert!(pack(&[]).is_err());
    }

    #[test]
    fn committed_icns_is_valid() {
        let path = crate::root().join("assets/app-icon/mkmatrix.icns");
        let Ok(icns) = std::fs::read(&path) else { return };
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize, icns.len());
    }
}
