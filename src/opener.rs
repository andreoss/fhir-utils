use crate::error::Error;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub trait Source: Read + Seek + Send {}
impl<T: Read + Seek + Send> Source for T {}

type WrittenStore = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

pub trait Sink: Write + Send {}
impl<T: Write + Send> Sink for T {}

pub trait Opener: Send + Sync {
    fn open(&self, source: &str) -> Result<Box<dyn Source>, Error>;

    fn create(&self, target: &str) -> Result<Box<dyn Sink>, Error> {
        Err(Error::Config(format!("this opener cannot write: {target}")))
    }
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
            None | Some("file") => {
                let path = self.resolve(source);
                let resolved = path.to_string_lossy();
                if let Some(scheme) = scheme(&resolved) {
                    return open_scheme(scheme, &resolved);
                }
                let file = File::open(&path).map_err(|error| {
                    Error::Config(format!("cannot open {}: {error}", path.display()))
                })?;
                Ok(Box::new(file))
            }
            Some(scheme) => open_scheme(scheme, source),
        }
    }

    /// Writes resolve as given, so a relative output stays relative to the run.
    fn create(&self, target: &str) -> Result<Box<dyn Sink>, Error> {
        match scheme(target) {
            None | Some("file") => {
                let path = Path::new(strip_file_scheme(target));
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|error| {
                            Error::Config(format!(
                                "cannot create {}: {error}",
                                parent.display()
                            ))
                        })?;
                    }
                }
                let file = File::create(path).map_err(|error| {
                    Error::Config(format!("cannot create {}: {error}", path.display()))
                })?;
                Ok(Box::new(file))
            }
            Some(scheme) => Err(unsupported(scheme, target)),
        }
    }
}

pub struct MemoryOpener {
    entries: Vec<(String, Vec<u8>)>,
    written: WrittenStore,
}

impl MemoryOpener {
    pub fn new(entries: Vec<(String, Vec<u8>)>) -> Self {
        Self {
            entries,
            written: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn written(&self) -> Vec<(String, Vec<u8>)> {
        self.written.lock().unwrap().clone()
    }
}

struct MemorySink {
    name: String,
    buffer: Vec<u8>,
    store: WrittenStore,
}

impl Write for MemorySink {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for MemorySink {
    fn drop(&mut self) {
        self.store
            .lock()
            .unwrap()
            .push((self.name.clone(), std::mem::take(&mut self.buffer)));
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

    fn create(&self, target: &str) -> Result<Box<dyn Sink>, Error> {
        Ok(Box::new(MemorySink {
            name: target.to_string(),
            buffer: Vec::new(),
            store: self.written.clone(),
        }))
    }
}

fn open_scheme(scheme: &str, source: &str) -> Result<Box<dyn Source>, Error> {
    #[cfg(feature = "optimized-streaming")]
    if matches!(scheme, "http" | "https") {
        return fetch(source);
    }
    Err(unsupported(scheme, source))
}

#[cfg(feature = "optimized-streaming")]
fn fetch(source: &str) -> Result<Box<dyn Source>, Error> {
    let response = ureq::get(source)
        .call()
        .map_err(|error| Error::Config(format!("cannot fetch {source}: {error}")))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|error| Error::Config(format!("cannot fetch {source}: {error}")))?;
    Ok(Box::new(Cursor::new(bytes)))
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
    decompressed(source, opener().open(source)?)
}

pub fn open_path(path: &Path) -> Result<Box<dyn Source>, Error> {
    let file = File::open(path)?;
    decompressed(&path.to_string_lossy(), Box::new(file))
}

/// A `.gz` source is inflated into memory so the reader can seek.
fn decompressed(source: &str, mut raw: Box<dyn Source>) -> Result<Box<dyn Source>, Error> {
    if !source.to_ascii_lowercase().ends_with(".gz") {
        return Ok(raw);
    }
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(&mut raw)
        .read_to_end(&mut bytes)
        .map_err(|error| Error::Config(format!("cannot decompress {source}: {error}")))?;
    Ok(Box::new(Cursor::new(bytes)))
}

pub fn create(target: &str) -> Result<Box<dyn Sink>, Error> {
    opener().create(target)
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
    fn gz_sources_are_inflated() {
        let mut file = tempfile::Builder::new().suffix(".csv.gz").tempfile().unwrap();
        let mut encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"a,b\n1,2\n").unwrap();
        file.write_all(&encoder.finish().unwrap()).unwrap();
        file.flush().unwrap();

        let mut source = open_path(file.path()).unwrap();
        let mut content = String::new();
        source.read_to_string(&mut content).unwrap();
        assert_eq!(content, "a,b\n1,2\n");
    }

    #[test]
    fn a_broken_gz_source_names_the_file() {
        let mut file = tempfile::Builder::new().suffix(".gz").tempfile().unwrap();
        file.write_all(b"not gzip").unwrap();
        file.flush().unwrap();

        let error = match open_path(file.path()) {
            Ok(_) => panic!("a broken gz source must not open"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("cannot decompress"));
    }

    #[test]
    fn a_scheme_base_carries_relative_sources_to_the_remote_opener() {
        let opener = LocalOpener::new("https://host/config");
        let error = match opener.open("map.csv") {
            Ok(_) => panic!("a remote base must not open locally"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("https://host/config/map.csv"), "{error}");
    }

    #[cfg(feature = "optimized-streaming")]
    #[test]
    fn http_sources_fetch_over_the_wire() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buffer);
            let body = "a,b\n1,2\n";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            std::io::Write::write_all(&mut stream, response.as_bytes()).unwrap();
        });

        let opener = LocalOpener::new("/base");
        let mut source = opener.open(&format!("http://{address}/codes.csv")).unwrap();
        let mut content = String::new();
        source.read_to_string(&mut content).unwrap();
        assert_eq!(content, "a,b\n1,2\n");
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
