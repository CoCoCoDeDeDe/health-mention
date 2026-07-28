use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMeta {
    /// 目录名（组件 id，read 时用它定位）
    pub dir_name: String,
    pub name: String,
    pub version: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginFiles {
    pub component_js: String,
    pub style_css: Option<String>,
}

/// 防路径穿越：名字只能是单层目录/文件名
fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.contains("..") && !name.contains('/') && !name.contains('\\')
}

/// 扫描 <app_data>/plugins/progress/*/，有 component.js 的才算组件
pub fn list_progress_plugins(plugins_dir: &Path) -> Vec<PluginMeta> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || !path.join("component.js").is_file() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let meta = fs::read_to_string(path.join("meta.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            let get = |k: &str| {
                meta.as_ref()
                    .and_then(|m| m.get(k))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            };
            out.push(PluginMeta {
                name: get("name").unwrap_or_else(|| dir_name.clone()),
                dir_name,
                version: get("version"),
                author: get("author"),
            });
        }
    }
    out.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    out
}

pub fn read_progress_plugin(plugins_dir: &Path, name: &str) -> Result<PluginFiles, String> {
    if !valid_name(name) {
        return Err("组件名无效".into());
    }
    let dir = plugins_dir.join(name);
    let component_js = fs::read_to_string(dir.join("component.js"))
        .map_err(|e| format!("读取 component.js 失败：{e}"))?;
    let style_css = fs::read_to_string(dir.join("style.css")).ok();
    Ok(PluginFiles {
        component_js,
        style_css,
    })
}

/// 扫描 <app_data>/themes/*.css，返回主题名（文件名去扩展名）
pub fn list_themes(themes_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("css") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    out.push(stem.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

pub fn read_theme(themes_dir: &Path, name: &str) -> Result<String, String> {
    if !valid_name(name) {
        return Err("主题名无效".into());
    }
    fs::read_to_string(themes_dir.join(format!("{name}.css")))
        .map_err(|e| format!("读取主题失败：{e}"))
}
