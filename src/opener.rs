use crate::error::Error;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub trait Source: Read + Seek + Send {}
impl<T: Read + Seek + Send> Source for T {}

pub trait Opener: Send + Sync {
    fn open(&self, source: &str) -> Result<Box<dyn Source>, Error>;
}

pub struct LocalOpener {
    base: PathBuf,
}

impl LocalOpener {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }

    pub fn resolve(&self, source: &str) -> PathBuf {
        let path = Path::new(strip_file_scheme(source));
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base.join(path)
        }
    }
}

impl Opener for LocalOpener {
    fn open(&self, source: &str) -> Result<Box<dyn Source>, Error> {
        match scheme(source) {
            None | Some("file") => Ok(Box::new(File::open(self.resolve(source))?)),
            Some(scheme) => Err(unsupported(scheme, source)),
        }
    }
}

pub struct MemoryOpener {
    entries: Vec<(String, Vec<u8>)>,
}

impl MemoryOpener {
    pub fn new(entries: Vec<(String, Vec<u8>)>) -> Self {
        Self { entries }
    }
}

impl Opener for MemoryOpener {
    fn open(&self, source: &str) -> Result<Box<dyn Source>, Error> {
        self.entries
            .iter()
            .find(|(name, _)| name == source)
            .map(|(_, content)| Box::new(Cursor::new(content.clone())) as Box<dyn Source>)
            .ok_or_else(|| Error::Config(format!("source not found: {source}")))
    }
}

fn unsupported(scheme: &str, source: &str) -> Error {
    if cfg!(feature = "optimized-streaming") {
        Error::Config(format!(
            "no object storage backend registered for {scheme} source: {source}"
        ))
    } else {
        Error::Config(format!(
            "{scheme} sources need the optimized-streaming feature: {source}"
        ))
    }
}

pub fn scheme(source: &str) -> Option<&str> {
    let (scheme, rest) = source.split_once("://")?;
    if rest.is_empty() || scheme.is_empty() {
        return None;
    }
    if scheme
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        Some(scheme)
    } else {
        None
    }
}

fn strip_file_scheme(source: &str) -> &str {
    source.strip_prefix("file://").unwrap_or(source)
}

thread_local! {
    static OPENER: RefCell<Option<Arc<dyn Opener>>> = const { RefCell::new(None) };
}

pub fn set_opener(opener: Arc<dyn Opener>) {
    OPENER.with(|slot| *slot.borrow_mut() = Some(opener));
}

pub fn opener() -> Arc<dyn Opener> {
    OPENER.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(|| {
                Arc::new(LocalOpener::new(
                    crate::config::config().mapping_config_directory.clone(),
                )) as Arc<dyn Opener>
            })
            .clone()
    })
}

pub fn open(source: &str) -> Result<Box<dyn Source>, Error> {
    opener().open(source)
}

pub fn read_to_string(source: &str) -> Result<String, Error> {
    let mut content = String::new();
    open(source)?.read_to_string(&mut content)?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use crate::opener::*;
    use serial_test::serial;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn schemes_are_detected() {
        assert_eq!(scheme("s3://bucket/key"), Some("s3"));
        assert_eq!(scheme("file:///tmp/x"), Some("file"));
        assert_eq!(scheme("plain/path.csv"), None);
        assert_eq!(scheme("/absolute/path.csv"), None);
    }

    #[test]
    fn local_opener_resolves_relative_paths() {
        let opener = LocalOpener::new("/base");
        assert_eq!(opener.resolve("a.csv"), std::path::Path::new("/base/a.csv"));
        assert_eq!(
            opener.resolve("/abs/a.csv"),
            std::path::Path::new("/abs/a.csv")
        );
        assert_eq!(
            opener.resolve("file:///abs/a.csv"),
            std::path::Path::new("/abs/a.csv")
        );
    }

    #[test]
    fn local_opener_reads_a_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "content").unwrap();
        file.flush().unwrap();

        let opener = LocalOpener::new("/");
        let mut source = opener.open(file.path().to_str().unwrap()).unwrap();
        let mut content = String::new();
        source.read_to_string(&mut content).unwrap();
        assert_eq!(content.trim(), "content");
    }

    #[test]
    fn object_storage_schemes_are_reported() {
        let opener = LocalOpener::new("/base");
        let error = match opener.open("s3://bucket/key") {
            Ok(_) => panic!("object storage sources must not open locally"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("s3"));
    }

    #[test]
    fn openers_do_not_leak_between_threads() {
        set_opener(Arc::new(MemoryOpener::new(vec![(
            "here.csv".into(),
            b"main".to_vec(),
        )])));

        let worker = std::thread::spawn(|| {
            let leaked = read_to_string("here.csv").is_ok();
            set_opener(Arc::new(MemoryOpener::new(vec![(
                "there.csv".into(),
                b"worker".to_vec(),
            )])));
            (leaked, read_to_string("there.csv").unwrap())
        });

        let (leaked, worker_content) = worker.join().unwrap();
        assert!(!leaked, "opener leaked into another thread");
        assert_eq!(worker_content, "worker");
        assert_eq!(read_to_string("here.csv").unwrap(), "main");
        assert!(read_to_string("there.csv").is_err(), "worker opener leaked");
    }

    #[test]
    #[serial]
    fn a_custom_opener_can_be_installed() {
        let previous = opener();
        set_opener(Arc::new(MemoryOpener::new(vec![(
            "codes.csv".into(),
            b"key,value\na,b\n".to_vec(),
        )])));

        let content = read_to_string("codes.csv").unwrap();
        assert!(content.contains("a,b"));
        assert!(
            read_to_string("missing.csv").is_err(),
            "missing source must fail"
        );

        set_opener(previous);
    }
}
