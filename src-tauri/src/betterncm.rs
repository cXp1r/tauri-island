use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::read::ZipArchive;
use zip::write::FileOptions;
use zip::ZipWriter;

const DEFAULT_BETTERNCM_ROOT: &str = r"C:\betterncm";
const BETTERNCM_PLUGINMARKET_OLD_SOURCE: &str =
    "https://raw.gitcode.com/intensity/bncm-plugin-packed/raw/master/";
const BETTERNCM_PLUGINMARKET_NEW_SOURCE: &str =
    "https://raw.githubusercontent.com/BetterNCM/BetterNCM-Packed-Plugins/master/";

fn normalize_betterncm_root(install_root: Option<String>) -> PathBuf {
    let root = install_root
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_BETTERNCM_ROOT.to_string());
    PathBuf::from(root)
}

fn patch_pluginmarket_source_text(input: &str) -> (String, bool) {
    if input.contains(BETTERNCM_PLUGINMARKET_OLD_SOURCE) {
        (
            input.replace(
                BETTERNCM_PLUGINMARKET_OLD_SOURCE,
                BETTERNCM_PLUGINMARKET_NEW_SOURCE,
            ),
            true,
        )
    } else {
        (input.to_string(), false)
    }
}

fn patch_pluginmarket_runtime(root: &Path) -> Result<bool, String> {
    let runtime_main = root.join("plugins_runtime").join("PluginMarket").join("main.js");
    if !runtime_main.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&runtime_main)
        .map_err(|e| format!("读取运行时 PluginMarket 失败 {}: {e}", runtime_main.display()))?;
    let (patched, changed) = patch_pluginmarket_source_text(&content);
    if changed {
        fs::write(&runtime_main, patched)
            .map_err(|e| format!("写入运行时 PluginMarket 失败 {}: {e}", runtime_main.display()))?;
    }
    Ok(changed)
}

fn patch_pluginmarket_archive(root: &Path) -> Result<bool, String> {
    let plugin_archive = root.join("plugins").join("PluginMarket.plugin");
    if !plugin_archive.exists() {
        return Ok(false);
    }

    let src_file = fs::File::open(&plugin_archive)
        .map_err(|e| format!("读取 PluginMarket.plugin 失败 {}: {e}", plugin_archive.display()))?;
    let mut src_zip =
        ZipArchive::new(src_file).map_err(|e| format!("解析 PluginMarket.plugin 失败: {e}"))?;

    let tmp_archive = root.join("plugins").join("PluginMarket.plugin.tmp");
    let tmp_file = fs::File::create(&tmp_archive)
        .map_err(|e| format!("创建临时文件失败 {}: {e}", tmp_archive.display()))?;
    let mut writer = ZipWriter::new(tmp_file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut changed = false;
    for i in 0..src_zip.len() {
        let mut entry = src_zip
            .by_index(i)
            .map_err(|e| format!("读取 PluginMarket 条目失败: {e}"))?;
        let name = entry.name().replace('\\', "/");
        if entry.is_dir() || name.ends_with('/') {
            writer
                .add_directory(name, options.clone())
                .map_err(|e| format!("写入目录条目失败: {e}"))?;
            continue;
        }

        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| format!("读取条目内容失败: {e}"))?;

        if name.eq_ignore_ascii_case("main.js") {
            let text = String::from_utf8_lossy(&data).to_string();
            let (patched, did_change) = patch_pluginmarket_source_text(&text);
            if did_change {
                data = patched.into_bytes();
                changed = true;
            }
        }

        writer
            .start_file(name, options.clone())
            .map_err(|e| format!("写入文件条目失败: {e}"))?;
        writer
            .write_all(&data)
            .map_err(|e| format!("写入文件内容失败: {e}"))?;
    }

    writer
        .finish()
        .map_err(|e| format!("写入临时插件包失败: {e}"))?;
    drop(src_zip);

    if changed {
        let backup = root.join("plugins").join("PluginMarket.plugin.bak");
        if !backup.exists() {
            fs::copy(&plugin_archive, &backup)
                .map_err(|e| format!("创建 PluginMarket 备份失败 {}: {e}", backup.display()))?;
        }
        fs::copy(&tmp_archive, &plugin_archive).map_err(|e| {
            format!(
                "写回 PluginMarket.plugin 失败 {}: {e}",
                plugin_archive.display()
            )
        })?;
    }

    let _ = fs::remove_file(&tmp_archive);
    Ok(changed)
}

#[tauri::command]
pub fn install_betterncm_support(install_root: Option<String>) -> Result<serde_json::Value, String> {
    let root = normalize_betterncm_root(install_root);
    if !root.exists() {
        return Err(format!("BetterNCM 目录不存在: {}", root.display()));
    }

    let runtime_patched = patch_pluginmarket_runtime(&root)?;
    let archive_patched = patch_pluginmarket_archive(&root)?;

    Ok(serde_json::json!({
        "root": root.to_string_lossy().to_string(),
        "runtime_patched": runtime_patched,
        "archive_patched": archive_patched
    }))
}
