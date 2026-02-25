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

/// Expand environment variables ($VAR, ${VAR}) and ~ in a path string.
/// Variables that are not set are left unexpanded.
pub fn expand_path(path: &str) -> PathBuf {
    let expanded = expand_env_vars(path);
    expand_tilde(&expanded)
}

fn expand_env_vars(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' {
            i += 1;
            if i < chars.len() && chars[i] == '{' {
                // ${VAR} form
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '}' {
                    i += 1;
                }
                let var_name: String = chars[start..i].iter().collect();
                if i < chars.len() {
                    i += 1; // skip '}'
                }
                match std::env::var(&var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => {
                        result.push_str("${");
                        result.push_str(&var_name);
                        result.push('}');
                    }
                }
            } else {
                // $VAR form — variable name is alphanumeric + underscore
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let var_name: String = chars[start..i].iter().collect();
                if var_name.is_empty() {
                    result.push('$');
                } else {
                    match std::env::var(&var_name) {
                        Ok(val) => result.push_str(&val),
                        Err(_) => {
                            result.push('$');
                            result.push_str(&var_name);
                        }
                    }
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
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
    fn test_expand_path_tilde() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_path("~/.config/agm"), home.join(".config/agm"));
    }

    #[test]
    fn test_expand_path_dollar_var() {
        std::env::set_var("AGM_TEST_DIR", "/tmp/agmtest");
        assert_eq!(
            expand_path("$AGM_TEST_DIR/foo"),
            PathBuf::from("/tmp/agmtest/foo")
        );
        std::env::remove_var("AGM_TEST_DIR");
    }

    #[test]
    fn test_expand_path_dollar_brace_var() {
        std::env::set_var("AGM_TEST_DIR2", "/tmp/agmtest2");
        assert_eq!(
            expand_path("${AGM_TEST_DIR2}/bar"),
            PathBuf::from("/tmp/agmtest2/bar")
        );
        std::env::remove_var("AGM_TEST_DIR2");
    }

    #[test]
    fn test_expand_path_unknown_var_kept() {
        let result = expand_path("$AGM_NONEXISTENT_XYZ/foo");
        assert!(result.to_string_lossy().contains("AGM_NONEXISTENT_XYZ"));
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
