use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::paths::{contract_tilde, expand_path, expand_tilde};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub editor: String,
    #[serde(alias = "central")]
    pub agm: AgmConfig,
    #[serde(default)]
    pub tools: BTreeMap<String, ToolConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgmConfig {
    pub prompt_source: String,
    pub skills_source: String,
    #[serde(default = "AgmConfig::default_agents_source")]
    pub agents_source: String,
    #[serde(default = "AgmConfig::default_commands_source")]
    pub commands_source: String,
    pub source_dir: String,
    #[serde(default)]
    pub disabled: Vec<String>,
}

impl AgmConfig {
    fn default_agents_source() -> String {
        "~/.local/share/agm/agents".into()
    }

    fn default_commands_source() -> String {
        "~/.local/share/agm/commands".into()
    }

    pub const TOGGLEABLE_FEATURES: &'static [&'static str] =
        &["prompt", "skills", "agents", "commands"];

    pub fn is_disabled(&self, feature: &str) -> bool {
        self.disabled.iter().any(|d| d == feature)
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
    pub agents_dir: String,
    #[serde(default)]
    pub commands_dir: String,
    #[serde(default)]
    pub mcp: Vec<String>,
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
                agents_dir: "agents".into(),
                commands_dir: "commands".into(),
                mcp: vec!["settings.json".into()],
            },
        );
        tools.insert(
            "codex".into(),
            ToolConfig {
                name: "Codex".into(),
                config_dir: "~/.codex".into(),
                settings: vec!["config.toml".into()],
                auth: vec!["auth.json".into()],
                prompt_filename: "AGENTS.md".into(),
                skills_dir: "skills".into(),
                agents_dir: "agents".into(),
                commands_dir: "commands".into(),
                mcp: vec!["config.toml".into()],
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
                agents_dir: "agents".into(),
                commands_dir: "commands".into(),
                mcp: vec!["mcp-config.json".into()],
            },
        );
        tools.insert(
            "crush".into(),
            ToolConfig {
                name: "Crush".into(),
                config_dir: "~/.config/crush".into(),
                settings: vec!["crush.json".into()],
                auth: vec!["crush.json".into()],
                prompt_filename: "AGENTS.md".into(),
                skills_dir: "skills".into(),
                agents_dir: "agents".into(),
                commands_dir: "commands".into(),
                mcp: vec!["crush.json".into()],
            },
        );
        tools.insert(
            "agy".into(),
            ToolConfig {
                name: "Antigravity CLI".into(),
                config_dir: "~/.gemini".into(),
                settings: vec!["antigravity-cli/settings.json".into()],
                auth: vec![
                    "oauth_creds.json".into(),
                    "accounts.json".into(),
                    "google_accounts.json".into(),
                ],
                prompt_filename: "GEMINI.md".into(),
                skills_dir: "skills".into(),
                agents_dir: "agents".into(),
                commands_dir: "commands".into(),
                mcp: vec!["config/mcp_config.json".into()],
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
                agents_dir: "agents".into(),
                commands_dir: "commands".into(),
                mcp: vec!["opencode.json".into()],
            },
        );
        tools.insert(
            "pi".into(),
            ToolConfig {
                name: "Pi".into(),
                config_dir: "~/.pi/agent".into(),
                settings: vec!["settings.json".into()],
                auth: vec!["auth.json".into()],
                prompt_filename: "AGENTS.md".into(),
                skills_dir: "skills".into(),
                agents_dir: "agents".into(),
                commands_dir: "commands".into(),
                mcp: vec![],
            },
        );

        Config {
            editor: String::new(),
            agm: AgmConfig {
                prompt_source: "~/.local/share/agm/prompts/MASTER.md".into(),
                skills_source: "~/.local/share/agm/skills".into(),
                agents_source: "~/.local/share/agm/agents".into(),
                commands_source: "~/.local/share/agm/commands".into(),
                source_dir: "~/.local/share/agm/source".into(),
                disabled: vec![],
            },
            tools,
        }
    }
}

impl ToolConfig {
    /// Resolve config_dir to absolute path
    pub fn resolved_config_dir(&self) -> PathBuf {
        expand_tilde(&self.config_dir)
    }

    /// Resolve a tool-relative path string to an absolute PathBuf.
    ///
    /// - Absolute path, or one starting with `~` or `$` → expand `~` and `$VAR`
    /// - Otherwise (including `sub/dir/file`) → relative to `config_dir`
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let is_absolute = std::path::Path::new(path).is_absolute()
            || path.starts_with('~')
            || path.starts_with('$')
            || (path.len() >= 2 && path.as_bytes()[1] == b':');
        if is_absolute {
            expand_path(path)
        } else {
            self.resolved_config_dir().join(path)
        }
    }

    /// Check if a linkable field is configured (non-empty) for this tool.
    pub fn is_field_configured(&self, field: &str) -> bool {
        match field {
            "prompt" => !self.prompt_filename.is_empty(),
            "skills" => !self.skills_dir.is_empty(),
            "agents" => !self.agents_dir.is_empty(),
            "commands" => !self.commands_dir.is_empty(),
            _ => false,
        }
    }

    /// Resolve the link path for a given field, with safety checks.
    ///
    /// Returns `None` if:
    /// - The field is not configured (empty string)
    /// - The resolved path would collide with `config_dir` itself
    ///   (e.g. field value is `""`, `"."`, or normalizes to the same path)
    pub fn resolved_link_path(&self, field: &str) -> Option<PathBuf> {
        if !self.is_field_configured(field) {
            return None;
        }
        let relative = match field {
            "prompt" => &self.prompt_filename,
            "skills" => &self.skills_dir,
            "agents" => &self.agents_dir,
            "commands" => &self.commands_dir,
            _ => return None,
        };
        let config_dir = self.resolved_config_dir();
        let link_path = config_dir.join(relative);
        let canonical_config = config_dir
            .canonicalize()
            .unwrap_or_else(|_| config_dir.clone());
        let canonical_link = link_path.canonicalize().unwrap_or_else(|_| {
            let parent = link_path.parent();
            match parent {
                Some(p) if p.exists() => p
                    .canonicalize()
                    .map(|c| c.join(link_path.file_name().unwrap_or_default()))
                    .unwrap_or_else(|_| link_path.clone()),
                _ => link_path.clone(),
            }
        });
        if canonical_link == canonical_config {
            eprintln!(
                "  {} Skipping {}: link path resolves to config_dir ({})",
                "warn".yellow(),
                field,
                contract_tilde(&config_dir)
            );
            return None;
        }
        Some(link_path)
    }

    /// Check if the tool's config directory exists on disk
    pub fn is_installed(&self) -> bool {
        self.resolved_config_dir().is_dir()
    }
}

/// Extract a [tools.{key}] section from raw config text.
/// Returns (section_lines, start_line_index, end_line_index).
pub fn extract_tool_section(
    config_text: &str,
    tool_key: &str,
) -> Option<(Vec<String>, usize, usize)> {
    let header = format!("[tools.{}]", tool_key);
    let lines: Vec<&str> = config_text.lines().collect();

    let start = lines.iter().position(|l| l.trim() == header)?;

    let sub_prefix = format!("[tools.{}.", tool_key);
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with(&sub_prefix) {
            end = i;
            break;
        }
    }

    let section_lines: Vec<String> = lines[start..end].iter().map(|l| l.to_string()).collect();
    Some((section_lines, start, end))
}

/// Replace a [tools.{key}] section in raw config text with new content.
pub fn replace_tool_section(
    config_text: &str,
    tool_key: &str,
    new_section: &str,
) -> Option<String> {
    let header = format!("[tools.{}]", tool_key);
    let lines: Vec<&str> = config_text.lines().collect();

    let start = lines.iter().position(|l| l.trim() == header)?;

    let sub_prefix = format!("[tools.{}.", tool_key);
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with(&sub_prefix) {
            end = i;
            break;
        }
    }

    let mut result: Vec<&str> = Vec::new();
    result.extend_from_slice(&lines[..start]);
    for line in new_section.lines() {
        result.push(line);
    }
    result.extend_from_slice(&lines[end..]);

    Some(result.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_serialization_roundtrip() {
        let config = Config::default_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.tools.len(), 7);
        assert!(parsed.tools.contains_key("claude"));
        assert!(parsed.tools.contains_key("codex"));
        assert!(parsed.tools.contains_key("copilot"));
        assert!(parsed.tools.contains_key("crush"));
        assert!(parsed.tools.contains_key("agy"));
        assert!(parsed.tools.contains_key("opencode"));
        assert!(parsed.tools.contains_key("pi"));
    }

    #[test]
    fn test_agm_defaults() {
        let config = Config::default_config();
        assert_eq!(
            config.agm.prompt_source,
            "~/.local/share/agm/prompts/MASTER.md"
        );
        assert_eq!(config.agm.skills_source, "~/.local/share/agm/skills");
        assert_eq!(config.agm.agents_source, "~/.local/share/agm/agents");
        assert_eq!(config.agm.commands_source, "~/.local/share/agm/commands");
        assert_eq!(config.agm.source_dir, "~/.local/share/agm/source");
    }

    #[test]
    fn test_legacy_central_section_alias() {
        // Configs written before the central -> agm rename use [central];
        // they must still load via the serde alias.
        let toml = r#"
[central]
prompt_source = "~/p/MASTER.md"
skills_source = "~/p/skills"
source_dir = "~/p/source"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.agm.prompt_source, "~/p/MASTER.md");
        assert_eq!(config.agm.skills_source, "~/p/skills");
        assert_eq!(config.agm.source_dir, "~/p/source");
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
            agents_dir: "agents".into(),
            commands_dir: "commands".into(),
            mcp: vec![],
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
            agents_dir: "".into(),
            commands_dir: "".into(),
            mcp: vec![],
        };
        let home = dirs::home_dir().unwrap();
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
            agents_dir: "".into(),
            commands_dir: "".into(),
            mcp: vec![],
        };
        let home = dirs::home_dir().unwrap();
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
            agents_dir: "".into(),
            commands_dir: "".into(),
            mcp: vec![],
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
            agents_dir: "".into(),
            commands_dir: "".into(),
            mcp: vec![],
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
    fn test_disabled_field_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        let mut config = Config::default_config();
        config.agm.disabled = vec!["skills".to_string(), "agents".to_string()];
        config.save_to(&config_path).unwrap();
        let loaded = Config::load_from(Some(config_path)).unwrap();
        assert_eq!(loaded.agm.disabled, vec!["skills", "agents"]);
    }

    #[test]
    fn test_disabled_field_default_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        let config = Config::default_config();
        config.save_to(&config_path).unwrap();
        let loaded = Config::load_from(Some(config_path)).unwrap();
        assert!(loaded.agm.disabled.is_empty());
    }

    #[test]
    fn test_is_disabled() {
        let mut config = Config::default_config();
        assert!(!config.agm.is_disabled("skills"));
        config.agm.disabled = vec!["skills".to_string()];
        assert!(config.agm.is_disabled("skills"));
        assert!(!config.agm.is_disabled("prompt"));
    }

    #[test]
    fn test_toggleable_features() {
        assert!(AgmConfig::TOGGLEABLE_FEATURES.contains(&"prompt"));
        assert!(AgmConfig::TOGGLEABLE_FEATURES.contains(&"skills"));
        assert!(AgmConfig::TOGGLEABLE_FEATURES.contains(&"agents"));
        assert!(AgmConfig::TOGGLEABLE_FEATURES.contains(&"commands"));
        assert!(!AgmConfig::TOGGLEABLE_FEATURES.contains(&"config"));
        assert!(!AgmConfig::TOGGLEABLE_FEATURES.contains(&"source"));
    }

    #[test]
    fn test_new_tools_have_agents_dir() {
        let config = Config::default_config();
        for (key, tool) in &config.tools {
            assert_eq!(
                tool.agents_dir, "agents",
                "Tool '{}' should have agents_dir = 'agents'",
                key
            );
        }
    }

    #[test]
    fn test_resolved_link_path_normal() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".test-tool");
        std::fs::create_dir_all(&config_dir).unwrap();
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: config_dir.to_string_lossy().to_string(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "TEST.md".into(),
            skills_dir: "skills".into(),
            agents_dir: "agents".into(),
            commands_dir: "commands".into(),
            mcp: vec![],
        };
        assert_eq!(
            tool.resolved_link_path("skills"),
            Some(config_dir.join("skills"))
        );
        assert_eq!(
            tool.resolved_link_path("agents"),
            Some(config_dir.join("agents"))
        );
        assert_eq!(
            tool.resolved_link_path("prompt"),
            Some(config_dir.join("TEST.md"))
        );
        assert_eq!(
            tool.resolved_link_path("commands"),
            Some(config_dir.join("commands"))
        );
    }

    #[test]
    fn test_resolved_link_path_empty_field() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".test-tool");
        std::fs::create_dir_all(&config_dir).unwrap();
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: config_dir.to_string_lossy().to_string(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "".into(),
            skills_dir: "".into(),
            agents_dir: "".into(),
            commands_dir: "".into(),
            mcp: vec![],
        };
        assert_eq!(tool.resolved_link_path("skills"), None);
        assert_eq!(tool.resolved_link_path("agents"), None);
        assert_eq!(tool.resolved_link_path("prompt"), None);
        assert_eq!(tool.resolved_link_path("commands"), None);
    }

    #[test]
    fn test_resolved_link_path_dot_field() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".test-tool");
        std::fs::create_dir_all(&config_dir).unwrap();
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: config_dir.to_string_lossy().to_string(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "TEST.md".into(),
            skills_dir: ".".into(),
            agents_dir: "agents".into(),
            commands_dir: "commands".into(),
            mcp: vec![],
        };
        assert_eq!(tool.resolved_link_path("skills"), None);
    }

    #[test]
    fn test_resolved_link_path_unknown_field() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".test-tool");
        std::fs::create_dir_all(&config_dir).unwrap();
        let tool = ToolConfig {
            name: "Test".into(),
            config_dir: config_dir.to_string_lossy().to_string(),
            settings: vec![],
            auth: vec![],
            prompt_filename: "TEST.md".into(),
            skills_dir: "skills".into(),
            agents_dir: "agents".into(),
            commands_dir: "commands".into(),
            mcp: vec![],
        };
        assert_eq!(tool.resolved_link_path("unknown"), None);
    }

    const SAMPLE_CONFIG: &str = r#"[agm]
prompt_source = "~/.local/share/agm/prompts/MASTER.md"
skills_source = "~/.local/share/agm/skills"

[tools.claude]
name = "Claude Code"
config_dir = "~/.claude"
prompt_filename = "CLAUDE.md"
skills_dir = "skills"

[tools.codex]
name = "Codex"
config_dir = "~/.codex"
prompt_filename = "AGENTS.md"
skills_dir = "skills"

[tools.copilot]
name = "Copilot"
config_dir = "~/.copilot"
"#;

    #[test]
    fn test_extract_tool_section() {
        let result = extract_tool_section(SAMPLE_CONFIG, "codex");
        assert!(result.is_some());
        let (section, start, end) = result.unwrap();
        assert!(section[0].contains("[tools.codex]"));
        assert!(start < end);
    }

    #[test]
    fn test_extract_tool_section_first_tool() {
        let result = extract_tool_section(SAMPLE_CONFIG, "claude");
        assert!(result.is_some());
        let (section, _, _) = result.unwrap();
        assert!(section[0].contains("[tools.claude]"));
        assert!(section.iter().any(|l| l.contains("Claude Code")));
    }

    #[test]
    fn test_extract_tool_section_last_tool() {
        let result = extract_tool_section(SAMPLE_CONFIG, "copilot");
        assert!(result.is_some());
        let (section, _, _) = result.unwrap();
        assert!(section[0].contains("[tools.copilot]"));
    }

    #[test]
    fn test_extract_tool_section_not_found() {
        let result = extract_tool_section(SAMPLE_CONFIG, "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_replace_tool_section() {
        let new_section = "[tools.codex]\nname = \"OpenAI Codex\"\nconfig_dir = \"~/.codex\"\n";
        let result = replace_tool_section(SAMPLE_CONFIG, "codex", new_section);
        assert!(result.is_some());
        let new_config = result.unwrap();
        assert!(new_config.contains("OpenAI Codex"));
        assert!(!new_config.contains("\"Codex\""));
        assert!(new_config.contains("[tools.claude]"));
        assert!(new_config.contains("[tools.copilot]"));
    }

    #[test]
    fn test_replace_tool_section_not_found() {
        let result = replace_tool_section(SAMPLE_CONFIG, "nonexistent", "whatever");
        assert!(result.is_none());
    }
}
