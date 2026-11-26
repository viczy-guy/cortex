use std::io::{Read, Write};
use std::path::Path;

pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("file path has no parent"))?;
    std::fs::create_dir_all(dir)?;
    let mut name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("path has no file name"))?
        .to_os_string();
    name.push(format!(".tmp-{}", std::process::id()));
    let tmp = dir.join(name);
    // A leftover from a crashed run (same pid) must not block every
    // later save; `create_new` then refuses anything planted in between.
    let _ = std::fs::remove_file(&tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    let result = (|| {
        file.write_all(contents)?;
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub fn read_bounded(path: &Path, max_bytes: u64) -> std::io::Result<Option<String>> {
    let Some(bytes) = read_bounded_bytes(path, max_bytes)? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// The whole file, or `None` when it is missing or larger than `max_bytes`.
pub fn read_bounded_bytes(path: &Path, max_bytes: u64) -> std::io::Result<Option<Vec<u8>>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if file.metadata()?.len() > max_bytes {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Ok(None);
    }
    Ok(Some(bytes))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!("cortex-files-{label}-{}", std::process::id()))
            .join("test.json")
    }
    #[test]
    fn private_writes_replace_atomically_and_leave_no_temporary_file() {
        let p = path("write");
        write_private(&p, b"one").unwrap();
        write_private(&p, b"two").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"two");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }
    #[test]
    fn bounded_reads_reject_missing_oversized_and_invalid_text() {
        let p = path("read");
        assert_eq!(read_bounded(&p, 3).unwrap(), None);
        write_private(&p, b"abc").unwrap();
        assert_eq!(read_bounded(&p, 3).unwrap().as_deref(), Some("abc"));
        assert_eq!(read_bounded(&p, 2).unwrap(), None);
        write_private(&p, &[255]).unwrap();
        assert!(read_bounded(&p, 3).is_err());
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }
}
