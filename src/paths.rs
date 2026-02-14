use std::path::{Path, PathBuf};

/// Expand ~ to home directory in path strings
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Contract home directory to ~ for display
pub fn contract_tilde(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~/.config/agm"), home.join(".config/agm"));
    }

    #[test]
    fn test_expand_no_tilde() {
        assert_eq!(expand_tilde("/tmp/foo"), PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn test_contract_tilde() {
        let home = dirs::home_dir().unwrap();
        let path = home.join(".config/agm");
        assert_eq!(contract_tilde(&path), "~/.config/agm");
    }

    #[test]
    fn test_contract_no_home() {
        assert_eq!(contract_tilde(Path::new("/tmp/foo")), "/tmp/foo");
    }
}
