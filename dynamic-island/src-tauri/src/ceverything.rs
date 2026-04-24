use libloading::{Library, Symbol};
use serde::Serialize;
use std::{
    env,
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
};

type EverythingSetSearchW = unsafe extern "system" fn(*const u16);
type EverythingSetMax = unsafe extern "system" fn(u32);
type EverythingSetOffset = unsafe extern "system" fn(u32);
type EverythingQueryW = unsafe extern "system" fn(i32) -> i32;
type EverythingGetNumResults = unsafe extern "system" fn() -> u32;
type EverythingGetLastError = unsafe extern "system" fn() -> u32;
type EverythingIsFileResult = unsafe extern "system" fn(u32) -> i32;
type EverythingIsFolderResult = unsafe extern "system" fn(u32) -> i32;
type EverythingIsVolumeResult = unsafe extern "system" fn(u32) -> i32;
type EverythingGetResultFullPathNameW = unsafe extern "system" fn(u32, *mut u16, u32) -> u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryType {
    File,
    Folder,
    Volume,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntry {
    pub r#type: EntryType,
    pub path: PathBuf,
}

fn dll_path() -> Result<PathBuf, String> {
    let exe_dir = env::current_exe()
        .map_err(|err| format!("failed to get current exe path: {err}"))?
        .parent()
        .ok_or_else(|| "failed to resolve exe directory".to_string())?
        .to_path_buf();

    // 优先 exe 同目录（dev 模式），再查 resources 子目录（打包后）
    let direct = exe_dir.join("Everything64.dll");
    if direct.exists() {
        return Ok(direct);
    }
    let bundled = exe_dir.join("resources").join("Everything64.dll");
    if bundled.exists() {
        return Ok(bundled);
    }
    Err(format!("Everything64.dll not found in {} or {}", direct.display(), bundled.display()))
}

fn load_symbol<'lib, T>(lib: &'lib Library, name: &[u8]) -> Result<Symbol<'lib, T>, String> {
    unsafe { lib.get(name) }.map_err(|err| {
        format!(
            "failed to load symbol {}: {err}",
            String::from_utf8_lossy(name)
        )
    })
}

fn widestr_to_path(buf: &[u16]) -> PathBuf {
    PathBuf::from(OsString::from_wide(buf))
}

fn get_result_path(
    get_full_path: &Symbol<EverythingGetResultFullPathNameW>,
    index: u32,
) -> PathBuf {
    let mut buf = vec![0u16; 260];

    loop {
        let written =
            unsafe { get_full_path(index, buf.as_mut_ptr(), buf.len() as u32) as usize };

        if written < buf.len().saturating_sub(1) {
            return widestr_to_path(&buf[..written]);
        }

        buf.resize(buf.len() * 2, 0);
    }
}

pub fn search_everything(keyword: &str, max: u32, offset: u32) -> Result<Vec<SearchEntry>, String> {
    let dll_path = dll_path()?;
    let lib = unsafe { Library::new(&dll_path) }
        .map_err(|err| format!("failed to load {}: {err}", dll_path.display()))?;

    let set_search: Symbol<EverythingSetSearchW> = load_symbol(&lib, b"Everything_SetSearchW")?;
    let set_max: Symbol<EverythingSetMax> = load_symbol(&lib, b"Everything_SetMax")?;
    let set_offset: Symbol<EverythingSetOffset> = load_symbol(&lib, b"Everything_SetOffset")?;
    let query: Symbol<EverythingQueryW> = load_symbol(&lib, b"Everything_QueryW")?;
    let get_count: Symbol<EverythingGetNumResults> =
        load_symbol(&lib, b"Everything_GetNumResults")?;
    let get_last_error: Symbol<EverythingGetLastError> =
        load_symbol(&lib, b"Everything_GetLastError")?;
    let is_file: Symbol<EverythingIsFileResult> = load_symbol(&lib, b"Everything_IsFileResult")?;
    let is_folder: Symbol<EverythingIsFolderResult> =
        load_symbol(&lib, b"Everything_IsFolderResult")?;
    let is_volume: Symbol<EverythingIsVolumeResult> =
        load_symbol(&lib, b"Everything_IsVolumeResult")?;
    let get_full_path: Symbol<EverythingGetResultFullPathNameW> =
        load_symbol(&lib, b"Everything_GetResultFullPathNameW")?;

    let search_text = format!("{keyword}\0")
        .encode_utf16()
        .collect::<Vec<u16>>();

    unsafe {
        set_search(search_text.as_ptr());
        set_max(max);
        set_offset(offset);

        if query(1) == 0 {
            return Err(format!(
                "Everything_QueryW failed, last error: {}",
                get_last_error()
            ));
        }

        let count = get_count();
        let mut entries = Vec::with_capacity(count as usize);

        for index in 0..count {
            let entry_type = if is_file(index) != 0 {
                EntryType::File
            } else if is_folder(index) != 0 {
                EntryType::Folder
            } else if is_volume(index) != 0 {
                EntryType::Volume
            } else {
                EntryType::Unknown
            };

            let path = get_result_path(&get_full_path, index);
            entries.push(SearchEntry {
                r#type: entry_type,
                path,
            });
        }

        Ok(entries)
    }
}

// ── Tauri Commands ──

/// 前端可序列化的搜索结果
#[derive(Debug, Clone, Serialize)]
pub struct SearchResultItem {
    pub id: String,
    pub title: String,
    pub desc: String,
    pub icon: String,
    pub action: String,
}

/// 搜索文件，返回 count 条（默认 10），offset 翻页（子线程执行，不阻塞主线程）
#[tauri::command]
pub async fn search_query(query: String, offset: Option<usize>, count: Option<usize>) -> Result<Vec<SearchResultItem>, String> {
    let count = count.unwrap_or(10);
    let offset = offset.unwrap_or(0);

    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    println!("[Everything] search_query: query='{}' offset={} count={}", query, offset, count);

    let items = tokio::task::spawn_blocking(move || -> Result<Vec<SearchResultItem>, String> {
        let entries = search_everything(&query, count as u32, offset as u32)?;

        println!("[Everything] SDK returned {} entries (max={}, offset={})", entries.len(), count, offset);

        let items: Vec<SearchResultItem> = entries
            .into_iter()
            .map(|entry| {
                let full_path = entry.path.to_string_lossy().to_string();
                let file_name = entry.path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| full_path.clone());
                let parent = entry.path
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let icon = match entry.r#type {
                    EntryType::Folder => "📁",
                    EntryType::Volume => "💿",
                    _ => "📄",
                }.to_string();
                SearchResultItem {
                    id: full_path.clone(),
                    title: file_name,
                    desc: parent,
                    icon,
                    action: full_path,
                }
            })
            .collect();

        println!("[Everything] returning {} items (offset={}, count={})", items.len(), offset, count);
        Ok(items)
    })
    .await
    .map_err(|e| format!("搜索线程异常: {e}"))??;

    Ok(items)
}

/// 执行搜索结果：用系统默认程序打开文件/文件夹
#[tauri::command]
pub fn search_execute(id: String, _action: String) {
    println!("[Everything] opening: {}", id);
    if let Err(e) = open::that(&id) {
        eprintln!("[Everything] 打开 {} 失败: {}", id, e);
    }
}
