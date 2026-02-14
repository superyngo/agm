use std::path::Path;
use std::process::Command;

use crate::config::Config;

/// Get the editor to use: config.editor → $EDITOR → vi
pub fn get_editor(config: &Config) -> String {
    if !config.editor.is_empty() {
        return config.editor.clone();
    }
    std::env::var("EDITOR").unwrap_or_else(|_| "vi".into())
}

/// Open one or more files in the editor
pub fn open_files(editor: &str, files: &[&Path]) -> anyhow::Result<()> {
    let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    let status = Command::new(editor).args(&paths).status()?;
    if !status.success() {
        anyhow::bail!("Editor exited with error");
    }
    Ok(())
}
