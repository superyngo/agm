use std::path::Path;
use std::process::Command;

use crate::config::Config;

/// Get the editor to use: config.editor → $EDITOR → platform default
pub fn get_editor(config: &Config) -> String {
    if !config.editor.is_empty() {
        return config.editor.clone();
    }
    std::env::var("EDITOR").unwrap_or_else(|_| crate::platform::default_editor().into())
}

/// Open one or more files in the editor
pub fn open_files(editor: &str, files: &[&Path]) -> anyhow::Result<()> {
    let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    // The editor setting may carry arguments (e.g. `code --wait`).
    let mut parts = editor.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("editor command is empty"))?;
    let status = Command::new(program).args(parts).args(&paths).status()?;
    if !status.success() {
        anyhow::bail!("Editor exited with error");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_with_arguments_runs() {
        // `true --wait` ignores its args and succeeds.
        assert!(open_files("true --wait", &[Path::new("x")]).is_ok());
        assert!(open_files("", &[Path::new("x")]).is_err());
    }
}
