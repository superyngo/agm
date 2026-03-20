//! Platform abstraction for link operations.
//!
//! Unix: uses symlinks for both files and directories.
//! Windows: uses NTFS junctions for directories and hardlinks for files.

use std::io;
use std::path::{Path, PathBuf};

/// Result of checking the system's ability to create links.
pub enum LinkCapability {
    /// Full junction + hardlink support
    Full,
    /// Partial support with explanation
    Limited(String),
    /// Cannot create links
    Unavailable(String),
}

/// Create a directory link (Unix: symlink, Windows: junction).
/// `target` must be an existing directory on Windows.
pub fn link_dir(target: &Path, link_path: &Path) -> io::Result<()> {
    sys::link_dir(target, link_path)
}

/// Create a file link (Unix: symlink, Windows: hardlink).
/// `target` must be an existing file on Windows.
pub fn link_file(target: &Path, link_path: &Path) -> io::Result<()> {
    sys::link_file(target, link_path)
}

/// Remove a link (symlink, junction, or hardlink).
pub fn remove_link(link_path: &Path) -> io::Result<()> {
    sys::remove_link(link_path)
}

/// Check if a path is a directory link (symlink on Unix, junction on Windows).
pub fn is_dir_link(path: &Path) -> bool {
    sys::is_dir_link(path)
}

/// Read the target of a directory link. Returns None if not a dir link.
pub fn read_dir_link_target(path: &Path) -> Option<PathBuf> {
    sys::read_dir_link_target(path)
}

/// Check if two paths refer to the same underlying file (by inode on Unix, file index on Windows).
pub fn same_file(a: &Path, b: &Path) -> io::Result<bool> {
    sys::same_file(a, b)
}

/// Default editor command when $EDITOR is unset.
pub fn default_editor() -> &'static str {
    sys::DEFAULT_EDITOR
}

/// Probe the system's ability to create links.
pub fn check_link_capability() -> LinkCapability {
    sys::check_link_capability()
}

// ===== Unix implementation =====

#[cfg(unix)]
mod sys {
    use super::*;
    use std::fs;
    use std::os::unix::fs as unix_fs;

    pub const DEFAULT_EDITOR: &str = "vi";

    pub fn link_dir(target: &Path, link_path: &Path) -> io::Result<()> {
        unix_fs::symlink(target, link_path)
    }

    pub fn link_file(target: &Path, link_path: &Path) -> io::Result<()> {
        unix_fs::symlink(target, link_path)
    }

    pub fn remove_link(link_path: &Path) -> io::Result<()> {
        fs::remove_file(link_path)
    }

    pub fn is_dir_link(path: &Path) -> bool {
        path.symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }

    pub fn read_dir_link_target(path: &Path) -> Option<PathBuf> {
        fs::read_link(path).ok()
    }

    pub fn same_file(a: &Path, b: &Path) -> io::Result<bool> {
        use std::os::unix::fs::MetadataExt;
        let ma = fs::metadata(a)?;
        let mb = fs::metadata(b)?;
        Ok(ma.dev() == mb.dev() && ma.ino() == mb.ino())
    }

    pub fn check_link_capability() -> LinkCapability {
        LinkCapability::Full
    }
}

// ===== Windows implementation =====

#[cfg(windows)]
mod sys {
    use super::*;
    use std::fs;

    pub const DEFAULT_EDITOR: &str = "notepad";

    pub fn link_dir(target: &Path, link_path: &Path) -> io::Result<()> {
        junction::create(target, link_path)
    }

    pub fn link_file(target: &Path, link_path: &Path) -> io::Result<()> {
        fs::hard_link(target, link_path)
    }

    pub fn remove_link(link_path: &Path) -> io::Result<()> {
        if is_dir_link(link_path) {
            fs::remove_dir(link_path)
        } else {
            fs::remove_file(link_path)
        }
    }

    pub fn is_dir_link(path: &Path) -> bool {
        // junction::exists may fail for broken junctions (target deleted).
        // Fall back to checking reparse point attribute via symlink_metadata.
        if junction::exists(path).unwrap_or(false) {
            return true;
        }
        // Check FILE_ATTRIBUTE_REPARSE_POINT (0x0400) + FILE_ATTRIBUTE_DIRECTORY (0x10)
        // via symlink_metadata which does NOT follow reparse points.
        use std::os::windows::fs::MetadataExt;
        fs::symlink_metadata(path)
            .map(|m| m.file_attributes() & 0x0400 != 0 && m.file_attributes() & 0x10 != 0)
            .unwrap_or(false)
    }

    pub fn read_dir_link_target(path: &Path) -> Option<PathBuf> {
        if is_dir_link(path) {
            junction::get_target(path).ok()
        } else {
            None
        }
    }

    pub fn same_file(a: &Path, b: &Path) -> io::Result<bool> {
        use std::os::windows::io::AsRawHandle;

        #[repr(C)]
        #[allow(non_snake_case)]
        struct BY_HANDLE_FILE_INFORMATION {
            dwFileAttributes: u32,
            ftCreationTime: [u32; 2],
            ftLastAccessTime: [u32; 2],
            ftLastWriteTime: [u32; 2],
            dwVolumeSerialNumber: u32,
            nFileSizeHigh: u32,
            nFileSizeLow: u32,
            nNumberOfLinks: u32,
            nFileIndexHigh: u32,
            nFileIndexLow: u32,
        }

        extern "system" {
            fn GetFileInformationByHandle(
                hFile: *mut std::ffi::c_void,
                lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
            ) -> i32;
        }

        let fa = fs::File::open(a)?;
        let fb = fs::File::open(b)?;

        unsafe {
            let mut info_a = std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>();
            let mut info_b = std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>();

            if GetFileInformationByHandle(fa.as_raw_handle() as *mut _, &mut info_a) == 0 {
                return Err(io::Error::last_os_error());
            }
            if GetFileInformationByHandle(fb.as_raw_handle() as *mut _, &mut info_b) == 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(info_a.dwVolumeSerialNumber == info_b.dwVolumeSerialNumber
                && info_a.nFileIndexHigh == info_b.nFileIndexHigh
                && info_a.nFileIndexLow == info_b.nFileIndexLow)
        }
    }

    pub fn check_link_capability() -> LinkCapability {
        let tmp = std::env::temp_dir().join("agm_capability_check");
        let _ = fs::remove_dir_all(&tmp);
        if fs::create_dir_all(&tmp).is_err() {
            return LinkCapability::Unavailable("Cannot create temp directory".into());
        }

        let junc_target = tmp.join("junc_target");
        let junc_link = tmp.join("junc_link");
        let _ = fs::create_dir(&junc_target);
        let junc_ok = junction::create(&junc_target, &junc_link).is_ok();

        let hl_src = tmp.join("hl_src.txt");
        let hl_dst = tmp.join("hl_dst.txt");
        let _ = fs::write(&hl_src, "test");
        let hl_ok = fs::hard_link(&hl_src, &hl_dst).is_ok();

        let _ = fs::remove_dir_all(&tmp);

        match (junc_ok, hl_ok) {
            (true, true) => LinkCapability::Full,
            (true, false) => LinkCapability::Limited("Cannot create hardlinks".into()),
            (false, true) => LinkCapability::Limited("Cannot create junctions".into()),
            (false, false) => LinkCapability::Unavailable(
                "Cannot create junctions or hardlinks. Check NTFS permissions.".into(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_link_dir_and_read_target() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target_dir");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link_dir");

        link_dir(&target, &link).unwrap();
        assert!(is_dir_link(&link));

        let read_target = read_dir_link_target(&link).unwrap();
        let canonical_target = fs::canonicalize(&target).unwrap();
        let canonical_read = fs::canonicalize(&read_target).unwrap();
        assert_eq!(canonical_read, canonical_target);
    }

    #[test]
    fn test_link_file_and_same_file() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target.txt");
        fs::write(&target, "hello").unwrap();
        let link = tmp.path().join("link.txt");

        link_file(&target, &link).unwrap();
        assert!(same_file(&link, &target).unwrap());
        assert_eq!(fs::read_to_string(&link).unwrap(), "hello");
    }

    #[test]
    fn test_remove_dir_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target_dir");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link_dir");
        link_dir(&target, &link).unwrap();

        remove_link(&link).unwrap();
        assert!(!link.exists());
        assert!(target.exists());
    }

    #[test]
    fn test_remove_file_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target.txt");
        fs::write(&target, "hello").unwrap();
        let link = tmp.path().join("link.txt");
        link_file(&target, &link).unwrap();

        remove_link(&link).unwrap();
        assert!(!link.exists());
        assert!(target.exists());
    }

    #[test]
    fn test_is_dir_link_false_for_regular_dir() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("regular_dir");
        fs::create_dir(&dir).unwrap();
        assert!(!is_dir_link(&dir));
    }

    #[test]
    fn test_same_file_different_files() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        assert!(!same_file(&a, &b).unwrap());
    }

    #[test]
    fn test_default_editor_not_empty() {
        assert!(!default_editor().is_empty());
    }
}
