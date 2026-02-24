use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::paths::expand_tilde;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub editor: String,
    pub central: CentralConfig,
    #[serde(default)]
    pub tools: BTreeMap<String, ToolConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CentralConfig {
    pub prompt_source: String,
    pub skills_source: String,
    pub source_dir: String,
    #[serde(default)]
    pub skill_repos: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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
}

impl Config {
    pub fn config_path() -> PathBuf {
        expand_tilde("~/.config/agm/config.toml")
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            anyhow::bail!(
                "Config not found at {}. Run `agm init` first.",
                path.display()
            );
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn default_config() -> Self {
        let mut tools = BTreeMap::new();
        tools.insert(
            "claude".into(),
            ToolConfig {
                name: "Claude Code".into(),
                config_dir: "~/.claude".into(),
                settings: vec!["settings.json".into(), "settings.local.json".into()],
                auth: vec![".credentials.json".into()],
                prompt_filename: "CLAUDE.md".into(),
                skills_dir: "skills".into(),
                mcp: vec!["settings.json".into()],
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
            },
        );

        Config {
            editor: String::new(),
            central: CentralConfig {
                prompt_source: "~/.local/share/agm/prompts/MASTER.md".into(),
                skills_source: "~/.local/share/agm/skills".into(),
                source_dir: "~/.local/share/agm/source".into(),
                skill_repos: vec![],
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
}

impl ToolConfig {
    /// Resolve config_dir to absolute path
    pub fn resolved_config_dir(&self) -> PathBuf {
        expand_tilde(&self.config_dir)
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
        };
        let home = dirs::home_dir().unwrap();
        assert_eq!(tool.resolved_config_dir(), home.join(".test-tool"));
    }
}
