//! User preference configuration — language, theme, and other persisted settings

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persisted identity of the user-picked external (fingerprint) browser.
///
/// The URL alone is not enough for reconnection: fingerprint browsers
/// (AdsPower & co.) typically launch with `--remote-debugging-port=0`, so a
/// reopened window listens on a NEW random port. With the exe path the running
/// process can be located and its actual debug port re-resolved (via cmdline
/// or `<user-data-dir>/DevToolsActivePort`) — see nuphus-browser's
/// `attach_external` self-healing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserIdentity {
    /// Human-readable platform name (e.g. "AdsPower") for UI/error display.
    pub name: String,
    /// Browser executable path — locates the running process.
    pub exe_path: String,
    /// `--user-data-dir` the window was launched with; fallback for
    /// DevToolsActivePort resolution when the process cmdline is unreadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_data_dir: Option<String>,
}

/// 项目书签：项目中心（输入框项目弹窗）维护的工作目录快捷入口。
///
/// 单一事实源落在这里（与 `project_dir` 同源），前端不再各自维护
/// localStorage 副本 —— 历史上前后端两套键（`nuphus_projects` /
/// `nuphus_project_bookmarks`）互不相通，是「Ctrl+K 加的书签在输入框看不到」
/// 的根因。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectBookmark {
    /// 展示名（默认取目录末段，可自定义）
    pub name: String,
    /// 绝对路径
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    /// User language preference, default "zh-CN"
    pub language: String,
    /// User-set project directory path
    #[serde(default)]
    pub project_dir: String,
    /// 项目书签列表（项目中心维护）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_bookmarks: Vec<ProjectBookmark>,
    /// External browser CDP endpoint (tri-state):
    /// `None` = never configured (leave any servers.yaml env untouched);
    /// `Some("")` = user explicitly switched back to managed Chrome (strip the env);
    /// `Some(url)` = attach all browser tools to this endpoint (e.g. fingerprint browser).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_cdp_url: Option<String>,
    /// Identity of the picked external browser. Only meaningful together with
    /// `browser_cdp_url: Some(url)`; cleared when switching back to managed
    /// Chrome or when a URL is set without identity (legacy/manual path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_identity: Option<BrowserIdentity>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            language: "zh-CN".to_string(),
            project_dir: String::new(),
            project_bookmarks: Vec::new(),
            browser_cdp_url: None,
            browser_identity: None,
        }
    }
}

impl UserPreferences {
    pub fn load() -> Self {
        let path = Self::path();
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            let prefs = UserPreferences::default();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&prefs) {
                let _ = std::fs::write(&path, json);
            }
            prefs
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create prefs dir failed: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize prefs failed: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("write prefs failed: {}", e))?;
        Ok(())
    }

    fn path() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".nuphus/preferences.json")
    }
}
