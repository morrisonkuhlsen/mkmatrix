//! OPC package access: reading zip parts with decompression limits, relationships and part-name
//! resolution; writing a zip package deterministically.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use crate::IoError;
use crate::xml::{self, El};

/// Largest part accepted however tightly it is packed. A bigger one has to pay with its share of
/// the file: see [`MAX_RATIO`].
pub const MAX_TIGHT_PART: u64 = 512 * 1024 * 1024;
/// Largest total read from a package however tightly it is packed.
const MAX_TIGHT_TOTAL: u64 = 1024 * 1024 * 1024;
/// How many bytes each byte of the file may unpack to past the tight limits. Sheet XML unpacks
/// about 8 to 20 times, whatever is in it, and a zip bomb about 700 to 1000 times, so this
/// lets in big sheets and keeps bombs at the tight limits.
const MAX_RATIO: u64 = 100;
/// Largest decompressed part accepted: about 30 to 70 million cells in one sheet.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_PART: u64 = 2048 * 1024 * 1024;
/// wasm's address space stops at 4 GB, and running out of it ends the app.
#[cfg(target_arch = "wasm32")]
pub const MAX_PART: u64 = MAX_TIGHT_PART;
/// Largest total decompressed size read from one package.
#[cfg(not(target_arch = "wasm32"))]
const MAX_TOTAL: u64 = 4096 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
const MAX_TOTAL: u64 = MAX_TIGHT_TOTAL;
/// Most entries accepted in the zip directory.
pub const MAX_ENTRIES: usize = 200_000;

pub struct Package<'a> {
    zip: zip::ZipArchive<Cursor<&'a [u8]>>,
    /// Lower-case normalized part name → zip index.
    names: HashMap<String, usize>,
    total: u64,
    /// What this package may unpack to in all.
    allowance: u64,
    /// The file's size: no part's compressed size can honestly be over it.
    file: u64,
    /// [`MAX_TIGHT_PART`], but for tests that don't want to inflate half a gigabyte.
    tight_part: u64,
}

/// What a part of `compressed` bytes may unpack to.
fn part_limit(compressed: u64, tight_part: u64) -> u64 {
    compressed.saturating_mul(MAX_RATIO).clamp(tight_part, MAX_PART.max(tight_part))
}

/// A relationship.
#[derive(Clone, Debug)]
pub struct Rel {
    pub id: String,
    /// Last path segment of the relationship type (`worksheet`, `styles`, …): the same for the
    /// transitional and strict namespaces.
    pub kind: String,
    /// Resolved part name (no leading `/`), or the raw target for external relationships.
    pub target: String,
    pub external: bool,
}

pub fn normalize(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches('/').to_ascii_lowercase()
}

impl<'a> Package<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<Package<'a>, IoError> {
        let zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| IoError::Zip(e.to_string()))?;
        if zip.len() > MAX_ENTRIES {
            return Err(IoError::TooLarge(format!("{} zip entries", zip.len())));
        }
        let mut names = HashMap::new();
        for i in 0..zip.len() {
            if let Some(Ok(n)) = zip.name_for_index(i) {
                names.entry(normalize(&n)).or_insert(i);
            }
        }
        let file = bytes.len() as u64;
        Ok(Package {
            zip,
            names,
            total: 0,
            allowance: file.saturating_mul(MAX_RATIO).clamp(MAX_TIGHT_TOTAL, MAX_TOTAL),
            file,
            tight_part: MAX_TIGHT_PART,
        })
    }

    pub fn has(&self, part: &str) -> bool {
        self.names.contains_key(&normalize(part))
    }

    /// Reads a part's bytes; `Ok(None)` when it doesn't exist.
    pub fn read(&mut self, part: &str) -> Result<Option<Vec<u8>>, IoError> {
        self.read_limited(part, MAX_PART)
    }

    /// Reads a small metadata part without allowing it to expand to the general part limit.
    pub fn read_limited(&mut self, part: &str, maximum: u64) -> Result<Option<Vec<u8>>, IoError> {
        let Some(&idx) = self.names.get(&normalize(part)) else {
            return Ok(None);
        };
        let mut f = self.zip.by_index(idx).map_err(|e| IoError::Zip(format!("{part}: {e}")))?;
        let maximum = maximum.min(part_limit(f.compressed_size().min(self.file), self.tight_part));
        if f.size() > maximum {
            return Err(IoError::TooLarge(format!("{part} expands to {} bytes", f.size())));
        }
        let remaining = self.allowance.saturating_sub(self.total);
        let limit = maximum.min(remaining);
        let mut out = Vec::with_capacity(f.size().min(16 * 1024 * 1024) as usize);
        let read = (&mut f).take(limit + 1).read_to_end(&mut out).map_err(|e| {
            // A read that fails partway has already spent decompression work and buffered data
            // the package budget never sees: count the allowance as spent, so a loop over many
            // broken parts can't decompress it over and over.
            self.total = self.allowance;
            IoError::Zip(format!("{part}: {e}"))
        });
        if out.len() as u64 > limit {
            // The allowance was spent getting here; make the exhaustion terminal instead of
            // letting later reads pay the same decompression cost again.
            self.total = self.allowance;
            return Err(IoError::TooLarge(if remaining < maximum {
                format!("package expands to more than {} MB", self.allowance >> 20)
            } else {
                format!("{part} expands to more than {maximum} bytes")
            }));
        }
        read?;
        self.total += out.len() as u64;
        Ok(Some(out))
    }

    pub fn read_xml(&mut self, part: &str) -> Result<Option<El>, IoError> {
        match self.read(part)? {
            Some(b) => xml::parse(&b).map(Some).map_err(|e| prefix(part, e)),
            None => Ok(None),
        }
    }

    /// Relationships of a part (`xl/workbook.xml` → `xl/_rels/workbook.xml.rels`). Missing or
    /// broken rels parts give an empty list.
    pub fn rels(&mut self, part: &str) -> Result<Vec<Rel>, IoError> {
        let (dir, file) = split_dir(part);
        let rels_name = if dir.is_empty() { format!("_rels/{file}.rels") } else { format!("{dir}/_rels/{file}.rels") };
        let root = match self.read(&rels_name)? {
            Some(b) => match xml::parse(&b) {
                Ok(r) => r,
                Err(_) => return Ok(vec![]),
            },
            None => return Ok(vec![]),
        };
        Ok(root
            .kids("Relationship")
            .filter_map(|r| {
                let id = r.attr("Id")?.to_string();
                let ty = r.attr("Type").unwrap_or("");
                let kind = ty.rsplit('/').next().unwrap_or("").to_string();
                let target_raw = r.attr("Target")?;
                let external = r.attr("TargetMode").is_some_and(|m| m.eq_ignore_ascii_case("External"));
                let target = if external { target_raw.to_string() } else { resolve(dir, target_raw) };
                Some(Rel { id, kind, target, external })
            })
            .collect())
    }
}

/// The first part whose declared unpacked size is over what [`Package::read`] accepts for it,
/// with that size.
pub fn oversized_part(bytes: &[u8]) -> Option<(String, u64)> {
    let file = bytes.len() as u64;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    (0..zip.len().min(MAX_ENTRIES)).find_map(|i| {
        let f = zip.by_index_raw(i).ok()?;
        let name = f.name().ok()?.into_owned();
        (f.size() > part_limit(f.compressed_size().min(file), MAX_TIGHT_PART)).then(|| (name, f.size()))
    })
}

fn prefix(part: &str, e: IoError) -> IoError {
    match e {
        IoError::Xml(m) => IoError::Xml(format!("{part}: {m}")),
        o => o,
    }
}

/// `xl/worksheets/sheet1.xml` → (`xl/worksheets`, `sheet1.xml`).
pub fn split_dir(part: &str) -> (&str, &str) {
    let p = part.trim_start_matches('/');
    match p.rfind('/') {
        Some(i) => (p.get(..i).unwrap_or(""), p.get(i + 1..).unwrap_or("")),
        None => ("", p),
    }
}

/// Resolves a relationship target against the source part's directory.
pub fn resolve(dir: &str, target: &str) -> String {
    let target = target.replace('\\', "/");
    let target = target.split('#').next().unwrap_or("").to_string();
    let mut segs: Vec<&str> = if target.starts_with('/') { vec![] } else { dir.split('/').filter(|s| !s.is_empty()).collect() };
    for s in target.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    segs.join("/")
}

// ---------------------------------------------------------------- writing

pub struct ZipOut {
    zip: zip::ZipWriter<Cursor<Vec<u8>>>,
}

impl ZipOut {
    pub fn new() -> ZipOut {
        ZipOut { zip: zip::ZipWriter::new(Cursor::new(Vec::new())) }
    }
    pub fn add(&mut self, name: &str, data: &[u8]) -> Result<(), IoError> {
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6))
            .last_modified_time(zip::DateTime::default())
            .large_file(data.len() as u64 >= u32::MAX as u64);
        self.zip.start_file(name, opts).map_err(|e| IoError::Zip(e.to_string()))?;
        self.zip.write_all(data).map_err(|e| IoError::Zip(e.to_string()))?;
        Ok(())
    }
    pub fn finish(self) -> Result<Vec<u8>, IoError> {
        self.zip.finish().map(|c| c.into_inner()).map_err(|e| IoError::Zip(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolving() {
        assert_eq!(resolve("xl", "worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(resolve("xl/worksheets", "../drawings/d1.xml"), "xl/drawings/d1.xml");
        assert_eq!(resolve("xl/worksheets", "/xl/tables/t.xml"), "xl/tables/t.xml");
        assert_eq!(resolve("", "xl/workbook.xml"), "xl/workbook.xml");
        assert_eq!(resolve("a", "../../../x"), "x");
        assert_eq!(split_dir("xl/workbook.xml"), ("xl", "workbook.xml"));
        assert_eq!(split_dir("x.xml"), ("", "x.xml"));
    }

    /// An over-budget read must spend the package's allowance once and for all: loops over many
    /// broken parts (a sheet's optional VML relationships, for one) otherwise re-decompress the
    /// remaining allowance again and again.
    #[test]
    fn a_failed_part_ends_the_package_budget() {
        let z = crate::tests::make_zip(&[("a.xml", b"0123456789"), ("b.xml", b"hello")]);
        let mut pkg = Package::open(&z).unwrap();
        assert!(pkg.read("a.xml").unwrap().is_some());
        // Nothing left to spend within the per-part limit either.
        pkg.total = pkg.allowance - 1;
        assert!(pkg.read("b.xml").is_err());
        assert_eq!(pkg.total, pkg.allowance, "the failed read is charged");
        // Oversized repeated attempts stay cheap errors now: no allowance is left to pay for.
        assert!(matches!(pkg.read("b.xml"), Err(IoError::TooLarge(_))));
    }

    /// What a sheet's XML looks like to deflate: it packs about ten times, not a thousand.
    fn sheet_xml(bytes: usize) -> Vec<u8> {
        let mut xml = Vec::with_capacity(bytes + 64);
        let mut seed = 1u64;
        while xml.len() < bytes {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            xml.extend_from_slice(format!("<c r=\"A{}\"><v>{}</v></c>", xml.len(), seed >> 44).as_bytes());
        }
        xml
    }

    /// A real inflate, with the tight limit scaled down: a part past it opens when it packs like a
    /// sheet and is refused when it packs like a bomb, at the same size.
    #[test]
    fn past_the_tight_limit_a_part_has_to_pay_with_its_share_of_the_file() {
        let size = 2 * 1024 * 1024;
        let sheet = sheet_xml(size);
        let bomb = vec![b'0'; sheet.len()];
        let z = crate::tests::make_zip(&[("sheet.xml", &sheet), ("bomb.xml", &bomb)]);
        let mut pkg = Package::open(&z).unwrap();
        pkg.tight_part = 256 * 1024;
        assert_eq!(pkg.read("sheet.xml").unwrap().as_deref(), Some(sheet.as_slice()));
        assert!(matches!(pkg.read("bomb.xml"), Err(IoError::TooLarge(_))));
        // Under the tight limit, how it packs doesn't matter.
        let z = crate::tests::make_zip(&[("bomb.xml", &bomb)]);
        let mut pkg = Package::open(&z).unwrap();
        pkg.tight_part = sheet.len() as u64;
        assert_eq!(pkg.read("bomb.xml").unwrap().as_deref(), Some(bomb.as_slice()));
    }

    /// The limit reads what the part really unpacks to, not what its header says.
    #[test]
    fn a_part_that_lies_about_its_size_stops_at_the_limit() {
        let size = 2 * 1024 * 1024;
        let bomb = vec![b'0'; size];
        let mut z = crate::tests::make_zip(&[("bomb.xml", &bomb)]);
        // The central directory's size goes down to 1 KB, which the header check believes.
        let at = z.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        z[at + 24..at + 28].copy_from_slice(&1024u32.to_le_bytes());
        let mut pkg = Package::open(&z).unwrap();
        pkg.tight_part = 256 * 1024;
        assert!(pkg.read("bomb.xml").is_err());
    }

    /// A part can't claim a compressed size bigger than the file to buy a bigger limit.
    #[test]
    fn a_part_cant_claim_more_compressed_bytes_than_the_file_has() {
        let bomb = vec![b'0'; 2 * 1024 * 1024];
        let mut z = crate::tests::make_zip(&[("bomb.xml", &bomb)]);
        let at = z.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        z[at + 20..at + 24].copy_from_slice(&(1024u32 * 1024 * 1024).to_le_bytes());
        if let Ok(mut pkg) = Package::open(&z) {
            pkg.tight_part = 256 * 1024;
            assert!(pkg.read("bomb.xml").is_err());
        }
    }

    #[test]
    fn the_limits_step_with_the_file() {
        const MB: u64 = 1024 * 1024;
        let limit = |compressed| part_limit(compressed, MAX_TIGHT_PART);
        // A tiny file keeps the tight limit, so a bomb costs what it always cost.
        assert_eq!(limit(0), MAX_TIGHT_PART);
        assert_eq!(limit(MB), MAX_TIGHT_PART);
        assert_eq!(limit(5 * MB), MAX_TIGHT_PART);
        // The sheet that found this: 518 MB in about 60 MB.
        #[cfg(not(target_arch = "wasm32"))]
        assert!(limit(60 * MB) > 518 * MB);
        assert_eq!(limit(u64::MAX), MAX_PART);
        // A small file's package keeps its old allowance.
        let z = crate::tests::make_zip(&[("a.xml", b"0")]);
        assert_eq!(Package::open(&z).unwrap().allowance, MAX_TIGHT_TOTAL);
    }
}
