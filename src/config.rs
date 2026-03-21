use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::paths::{expand_path, expand_tilde};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub editor: String,
    pub central: CentralConfig,
    #[serde(default)]
    pub tools: BTreeMap<String, ToolConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CentralConfig {
    pub prompt_source: String,
    pub skills_source: String,
    pub source_dir: String,
    #[serde(default)]
    pub skill_repos: Vec<String>,
    #[serde(default = "CentralConfig::default_files_base")]
    pub files_base: String,
    /// Absolute paths (supports ~ and $VAR) to centrally managed files
    #[serde(default)]
    pub files: Vec<String>,
}

impl CentralConfig {
    fn default_files_base() -> String {
        "~/.local/share/agm/files".into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub name: String,
    pub config_dir: String,
    #[serde(default)]
    pub settings: Vec<String>,
    #[serde(default)]
    pub auth: Vec<String>,
    #[serde(default)]
    pub prompt_filename: String,
    #[serde(default)]
    pub skills_dir: String,
    #[serde(default)]
    pub mcp: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

impl Config {
    pub fn config_path() -> PathBuf {
        expand_tilde("~/.config/agm/config.toml")
    }

    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(None)
    }

    pub fn load_from(path: Option<PathBuf>) -> anyhow::Result<Self> {
        let path = path.unwrap_or_else(Self::config_path);
        if !path.exists() {
            anyhow::bail!(
                "Config not found at {}. Run `agm init` first.",
                path.display()
            );
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::config_path())
    }

    pub fn save_to(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn default_config() -> Self {
        let mut tools = BTreeMap::new();
        tools.insert(
            "claude".into(),
            ToolConfig {
                name: "Claude Code".into(),
                config_dir: "~/.claude".into(),
                settings: vec![
                    "~/.claude.json".into(),
                    "settings.json".into(),
                    "settings.local.json".into(),
                ],
                auth: vec![".credentials.json".into()],
                prompt_filename: "CLAUDE.md".into(),
                skills_dir: "skills".into(),
                mcp: vec!["settings.json".into()],
                files: vec![],
            },
        );
        tools.insert(
            "gemini".into(),
            ToolConfig {
                name: "Gemini CLI".into(),
                config_dir: "~/.gemini".into(),
                settings: vec!["settings.json".into()],
                auth: vec![
                    "oauth_creds.json".into(),
                    "accounts.json".into(),
                    "google_accounts.json".into(),
                ],
                prompt_filename: "GEMINI.md".into(),
                skills_dir: "skills".into(),
                mcp: vec!["settings.json".into()],
                files: vec![],
            },
        );
        tools.insert(
            "copilot".into(),
            ToolConfig {
                name: "Copilot CLI".into(),
                config_dir: "~/.copilot".into(),
                settings: vec!["config.json".into()],
                auth: vec!["config.json".into()],
                prompt_filename: "AGENTS.md".into(),
                skills_dir: "skills".into(),
                mcp: vec!["mcp-config.json".into()],
                files: vec![],
            },
        );
        tools.insert(
            "opencode".into(),
            ToolConfig {
                name: "OpenCode".into(),
                config_dir: "~/.config/opencode".into(),
                settings: vec!["opencode.json".into()],
                auth: vec!["~/.local/share/opencode/auth.json".into()],
                prompt_filename: "AGENTS.md".into(),
                skills_dir: "skills".into(),
                mcp: vec!["opencode.json".into()],
                files: vec![],
            },
        );

        Config {
            editor: String::new(),
            central: CentralConfig {
                prompt_source: "~/.local/share/agm/prompts/MASTER.md".into(),
                skills_source: "~/.local/share/agm/skills".into(),
                source_dir: "~/.local/share/agm/source".into(),
                skill_repos: vec![],
                files_base: "~/.local/share/agm/files".into(),
                files: vec![],
            },
            tools,
        }
    }

    /// Add a skill repo URL if not already present, then save
    pub fn add_skill_repo(&mut self, url: &str) -> anyhow::Result<()> {
        if !self.central.skill_repos.contains(&url.to_string()) {
            self.central.skill_repos.push(url.to_string());
            self.save()?;
            println!("Added {} to config", url);
        }
        Ok(())
    }

    /// Remove a skill repo URL if present
    pub fn remove_skill_repo(&mut self, url: &str) {
        self.central.skill_repos.retain(|u| u != url);
    }
}

impl ToolConfig {
    /// Resolve config_dir to absolute path
    pub fn resolved_config_dir(&self) -> PathBuf {
        expand_tilde(&self.config_dir)
    }

    /// Resolve a tool-relative path string to an absolute PathBuf.
    ///
    /// - Absolute-looking path (contains `/`, `\`, starts with `~`, or has drive letter) →
    ///   expand `~` and `$VAR`
    /// - Otherwise → relative to `config_dir`
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let is_absolute = path.contains('/')
            || path.contains('\\')
            || path.starts_with('~')
            || (path.len() >= 2 && path.as_bytes()[1] == b':');
        if is_absolute {
            expand_path(path)
        } else {
            self.resolved_config_dir().join(path)
        }
    }

    /// Check if the tool's config directory exists on disk
    pub fn is_installed(&self) -> bool {
        self.resolved_config_dir().is_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_serialization_roundtrip() {
        let config = Config::default_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.tools.len(), 4);
        assert!(parsed.tools.contains_key("claude"));
        assert!(parsed.tools.contains_key("gemini"));
        assert!(parsed.tools.contains_key("copilot"));
        assert!(parsed.tools.contains_key("opencode"));
    }

    #[test]
    fn test_central_defaults() {
        let config = Config::default_config();
        assert_eq!(
            config.central.prompt_source,
            "~/.local/share/agm/prompts/MASTER.md"
        );
        assert_eq!(config.central.skills_source, "~/.local/share/agm/skills");
        assert_eq!(config.central.source_dir, "~/.local/share/agm/source");
        assert!(config.central.skill_repos.is_empty());
        assert_eq!(config.central.files_base, "~/.local/share/agm/files");
        assert!(config.central.files.is_empty());
    }

    #[test]
    fn test_tool_config_resolved_path() {
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: "~/.test-tool".into(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "TEST.md".into(),
            skills_dir: "skills".into(),
            mcp: vec![],
            files: vec![],
        };
        let home = dirs::home_dir().unwrap();
        assert_eq!(tool.resolved_config_dir(), home.join(".test-tool"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: "~/.test-tool".into(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "".into(),
            skills_dir: "".into(),
            mcp: vec![],
            files: vec![],
        };
        let home = dirs::home_dir().unwrap();
        // No "/" → relative to config_dir
        assert_eq!(
            tool.resolve_path("settings.json"),
            home.join(".test-tool/settings.json")
        );
    }

    #[test]
    fn test_resolve_path_absolute_tilde() {
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: "~/.test-tool".into(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "".into(),
            skills_dir: "".into(),
            mcp: vec![],
            files: vec![],
        };
        let home = dirs::home_dir().unwrap();
        // Contains "/" → absolute, expand ~
        assert_eq!(
            tool.resolve_path("~/.claude.json"),
            home.join(".claude.json")
        );
    }

    #[test]
    fn test_resolve_path_absolute_slash() {
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: "~/.test-tool".into(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "".into(),
            skills_dir: "".into(),
            mcp: vec![],
            files: vec![],
        };
        assert_eq!(
            tool.resolve_path("/etc/some.conf"),
            PathBuf::from("/etc/some.conf")
        );
    }

    #[test]
    fn test_resolve_path_env_var() {
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: "~/.test-tool".into(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "".into(),
            skills_dir: "".into(),
            mcp: vec![],
            files: vec![],
        };
        std::env::set_var("AGM_TEST_RESOLVE", "/tmp/agm_resolve");
        assert_eq!(
            tool.resolve_path("$AGM_TEST_RESOLVE/auth.json"),
            PathBuf::from("/tmp/agm_resolve/auth.json")
        );
        std::env::remove_var("AGM_TEST_RESOLVE");
    }

    #[test]
    fn test_claude_default_first_setting() {
        let config = Config::default_config();
        let claude = config.tools.get("claude").unwrap();
        assert_eq!(claude.settings[0], "~/.claude.json");
    }

    #[test]
    fn test_remove_skill_repo() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let mut config = Config::default_config();
        config.central.skill_repos = vec![
            "https://github.com/user/repo1.git".to_string(),
            "https://github.com/user/repo2.git".to_string(),
        ];
        config.save_to(&config_path).unwrap();

        config.remove_skill_repo("https://github.com/user/repo1.git");
        assert_eq!(config.central.skill_repos.len(), 1);
        assert_eq!(
            config.central.skill_repos[0],
            "https://github.com/user/repo2.git"
        );

        // Removing non-existent URL is a no-op
        config.remove_skill_repo("https://github.com/user/nonexistent.git");
        assert_eq!(config.central.skill_repos.len(), 1);
    }
}
