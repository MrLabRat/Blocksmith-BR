mod modules;

use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;
use tauri::{Manager, AppHandle, Emitter};
use tokio::sync::mpsc;
use modules::{PackInfo, PackType, Settings, FileMover, LogEntry, MoveOperation, scan_single_pack};
use serde::{Deserialize, Serialize};
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::sync::atomic::{AtomicBool, Ordering};
use once_cell::sync::Lazy;
use regex::Regex;

static ICON_BLACKRED_NOBORDER: &[u8] = include_bytes!("../icons/blackrednoborder.png");
static ICON_BLACKRED_BORDER:   &[u8] = include_bytes!("../icons/blackredborder.png");
static ICON_DEFAULT_NOBORDER:  &[u8] = include_bytes!("../icons/defaultnoborder.png");
static ICON_DEFAULT_BORDER:    &[u8] = include_bytes!("../icons/defaultborder.png");
static SKINMASTER_EXE:         &[u8] = include_bytes!("../resources/SkinMaster.exe");

fn icon_bytes_for(name: &str) -> Option<&'static [u8]> {
    match name {
        "blackrednoborder" => Some(ICON_BLACKRED_NOBORDER),
        "blackredborder"   => Some(ICON_BLACKRED_BORDER),
        "defaultnoborder"  => Some(ICON_DEFAULT_NOBORDER),
        "defaultborder"    => Some(ICON_DEFAULT_BORDER),
        _ => None,
    }
}

fn decode_icon(bytes: &[u8]) -> Option<tauri::image::Image<'static>> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(tauri::image::Image::new_owned(rgba.into_raw(), width, height))
}

static VERSION_PATTERN_1: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+v?\.\d+(\.\d+)*$").unwrap());
static VERSION_PATTERN_2: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+v\d+(\.\d+)*$").unwrap());
static VERSION_PATTERN_3: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+\d+(\.\d+)+$").unwrap());
static VERSION_PATTERN_4: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+\d+$").unwrap());

static EXTRACT_VERSION_1: Lazy<Regex> = Lazy::new(|| Regex::new(r"v?\.(\d+(?:\.\d+)*)").unwrap());
static EXTRACT_VERSION_2: Lazy<Regex> = Lazy::new(|| Regex::new(r"v(\d+(?:\.\d+)*)").unwrap());
static EXTRACT_VERSION_3: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s(\d+(?:\.\d+)+)\s*\(").unwrap());
static EXTRACT_VERSION_4: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s(\d+(?:\.\d+)+)$").unwrap());
static EXTRACT_VERSION_5: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s(\d+)\s*\(").unwrap());
static EXTRACT_VERSION_6: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s(\d+(?:\.\d+)*)\s").unwrap());

struct AppState {
    settings: RwLock<Settings>,
    watching: AtomicBool,
    debug_mode: AtomicBool,
    watch_stop_tx: parking_lot::Mutex<Option<std::sync::mpsc::SyncSender<()>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherEvent {
    pub timestamp: String,
    pub event_type: String,
    pub path: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumCachePack {
    pub folder_name: String,
    pub display_name: String,
    pub path: String,
}

#[tauri::command]
async fn scan_packs(directory: String, app: AppHandle) -> Result<Vec<PackInfo>, String> {
    emit_log(&app, "INFO", &format!("Scanning directory: {}", directory));
    
    let path = std::path::Path::new(&directory);
    if !path.exists() {
        emit_log(&app, "ERROR", "Directory does not exist");
        return Err("Directory does not exist".to_string());
    }
    
    let _ = app.emit("progress", serde_json::json!({
        "current": 0,
        "total": 0,
        "message": "Finding pack files..."
    }));
    
    let pack_extensions = ["mcpack", "mcaddon", "mctemplate"];
    let files: Vec<std::path::PathBuf> = std::fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|ext| pack_extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();
    
    let total_files = files.len();
    
    if total_files == 0 {
        emit_log(&app, "INFO", "No pack files found");
        return Ok(vec![]);
    }
    
    emit_log(&app, "INFO", &format!("Found {} pack files to scan", total_files));
    
    let _ = app.emit("progress", serde_json::json!({
        "current": 0,
        "total": total_files,
        "message": "Scanning packs in parallel..."
    }));
    
    let app_for_progress = app.clone();
    let progress_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_for_progress = total_files;
    let progress_last_emit = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    
    let files_for_scan = files.clone();
    let mut packs = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;
        
        let counter = Arc::clone(&progress_counter);
        let last_emit = Arc::clone(&progress_last_emit);
        let app_clone = app_for_progress.clone();
        
        files_for_scan
            .par_iter()
            .flat_map(|file| {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    scan_single_pack(file)
                }));
                
                let current = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                let last = last_emit.load(std::sync::atomic::Ordering::SeqCst);
                if current == total_for_progress || current.saturating_sub(last) >= 5 {
                    last_emit.store(current, std::sync::atomic::Ordering::SeqCst);
                    let _ = app_clone.emit("progress", serde_json::json!({
                        "current": current,
                        "total": total_for_progress,
                        "message": format!("Scanned {}/{}", current, total_for_progress)
                    }));
                }
                
                match result {
                    Ok(p) => p,
                    Err(_) => {
                        eprintln!("Panic while scanning: {:?}", file);
                        vec![]
                    }
                }
            })
            .collect::<Vec<_>>()
    }).await.map_err(|e| format!("Scan failed: {}", e))?;
    
    emit_log(&app, "INFO", &format!("Found {} packs in {} files", packs.len(), total_files));
    
    let mut size_cache: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for file in &files {
        if let Ok(metadata) = std::fs::metadata(file) {
            size_cache.insert(file.to_string_lossy().to_string(), metadata.len());
        }
    }
    for pack in &mut packs {
        if pack.folder_size.is_none() {
            if let Some(size) = size_cache.get(&pack.path) {
                pack.folder_size = Some(*size);
                pack.folder_size_formatted = Some(format_bytes(*size));
            }
        }
    }
    
    {
        let state = app.state::<AppState>();
        let mut settings = state.settings.write();
        settings.scan_location = Some(directory);
        let _ = save_settings_to_file(&settings);
    }
    
    let _ = app.emit("progress", serde_json::json!({
        "current": total_files,
        "total": total_files,
        "message": "Scan complete",
        "estimated_seconds": 0
    }));
    
    Ok(packs)
}

#[tauri::command]
async fn compute_pack_status(packs: Vec<PackInfo>, app: AppHandle) -> Result<Vec<PackInfo>, String> {
    let app_for_emit = app.clone();
    tokio::task::spawn_blocking(move || {
        let installed_packs = get_installed_packs_info(&app_for_emit);
        let installed_by_uuid: std::collections::HashMap<&str, usize> = installed_packs
            .iter()
            .enumerate()
            .filter_map(|(idx, ip)| ip.uuid.as_deref().map(|u| (u, idx)))
            .collect();
        let installed_base_names: std::collections::HashMap<(PackType, String), usize> = installed_packs
            .iter()
            .enumerate()
            .map(|(idx, ip)| ((ip.pack_type, extract_base_name(&ip.name)), idx))
            .collect();
        let mut size_cache: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        let mut results = packs;

        for pack in &mut results {
            let installed_index = if let Some(uuid) = pack.uuid.as_deref() {
                installed_by_uuid.get(uuid).copied()
            } else {
                let pack_base = extract_base_name(&pack.name);
                installed_base_names.get(&(pack.pack_type, pack_base)).copied()
            };

            if let Some(idx) = installed_index {
                let installed = &installed_packs[idx];
                let uuid_match = pack.uuid.is_some() && pack.uuid == installed.uuid;

                let new_ver: Option<String> = if uuid_match {
                    extract_version_from_name(&pack.name)
                        .or_else(|| extract_version_from_path(&pack.path))
                        .or_else(|| pack.version.clone())
                } else {
                    pack.version.clone()
                        .or_else(|| extract_version_from_name(&pack.name))
                        .or_else(|| extract_version_from_path(&pack.path))
                };

                let old_ver: Option<String> = if uuid_match {
                    extract_version_from_name(&installed.folder_name)
                        .or_else(|| extract_version_from_path(&installed.path))
                        .or_else(|| installed.version.clone())
                } else {
                    installed.version.clone()
                        .or_else(|| extract_version_from_name(&installed.name))
                        .or_else(|| extract_version_from_path(&installed.path))
                };

                match (new_ver.clone(), old_ver.clone()) {
                    (Some(new_version), Some(old_version)) => {
                        if new_version == old_version {
                            pack.is_installed = Some(true);
                            pack.installed_version = Some(old_version);
                        } else {
                            pack.is_installed = Some(true);
                            pack.is_update = Some(true);
                            pack.installed_version = Some(old_version);
                        }
                    }
                    (Some(_), None) | (None, Some(_)) => {
                        pack.is_installed = Some(true);
                        pack.installed_version = old_ver.clone();
                    }
                    (None, None) => {
                        pack.is_installed = Some(true);
                        let old_size = size_cache.entry(installed.path.clone()).or_insert_with(|| {
                            let path = std::path::Path::new(&installed.path);
                            calculate_folder_size(path)
                        });
                        if let Some(new_size) = pack.folder_size {
                            let size_diff = if new_size > *old_size {
                                new_size as f64 / *old_size as f64
                            } else {
                                *old_size as f64 / new_size as f64
                            };
                            if size_diff > 1.1 {
                                pack.is_update = Some(true);
                            }
                        }
                    }
                }
            }
        }

        results
    })
    .await
    .map_err(|e| format!("Status check failed: {}", e))
}

#[tauri::command]
async fn process_packs(packs: Vec<PackInfo>, app: AppHandle) -> Result<Vec<MoveOperation>, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    
    let total = packs.len();
    let delete_source = settings.delete_source;
    let (log_tx, mut log_rx) = mpsc::unbounded_channel();
    
    let mut mover = FileMover::new(settings.clone());
    mover.set_log_sender(log_tx);
    let mover = Arc::new(mover);
    
    let scan_dir = settings.scan_location.as_ref().map(|s| PathBuf::from(s));
    
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(log) = log_rx.recv().await {
            let _ = app_clone.emit("log", log);
        }
    });
    
    let results = Arc::new(RwLock::new(Vec::new()));
    let processed_sources = Arc::new(RwLock::new(Vec::new()));
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    
    let mut handles = Vec::new();
    let max_concurrent = 8;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    
    for pack in packs {
        let mover_clone = Arc::clone(&mover);
        let scan_dir_clone = scan_dir.clone();
        let results_clone = Arc::clone(&results);
        let processed_sources_clone = Arc::clone(&processed_sources);
        let counter_clone = Arc::clone(&counter);
        let app_clone = app.clone();
        let semaphore_clone = Arc::clone(&semaphore);
        let delete_source_clone = delete_source;
        let source_path = pack.path.clone();
        
        let handle = tokio::spawn(async move {
            let _permit = semaphore_clone.acquire().await.unwrap();
            
            let current = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let _ = app_clone.emit("progress", serde_json::json!({
                "current": current,
                "total": total,
                "message": format!("Processing {}", pack.name)
            }));
            
            let result = mover_clone.process_pack(&pack, scan_dir_clone.as_ref()).await;
            
            if result.success && delete_source_clone {
                processed_sources_clone.write().push(source_path);
            }
            
            results_clone.write().push(result);
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        let _ = handle.await;
    }
    
    let mut final_results = Arc::try_unwrap(results).unwrap().into_inner();
    
    if delete_source {
        for source in Arc::try_unwrap(processed_sources).unwrap().into_inner() {
            if std::fs::remove_file(&source).is_ok() {
                emit_log(&app, "INFO", &format!("Deleted source file: {}", source));
            }
        }
    }
    
    let _ = app.emit("progress", serde_json::json!({
        "current": total,
        "total": total,
        "message": "Complete"
    }));
    
    final_results.sort_by(|a, b| a.pack_name.cmp(&b.pack_name));
    Ok(final_results)
}

#[tauri::command]
async fn rollback_last(app: AppHandle) -> Result<Option<MoveOperation>, String> {
    emit_log(&app, "INFO", "Attempting to rollback last operation");
    
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    
    let (log_tx, mut log_rx) = mpsc::unbounded_channel();
    
    let mut mover = FileMover::new(settings);
    mover.set_log_sender(log_tx);
    let mover = Arc::new(mover);
    
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(log) = log_rx.recv().await {
            let _ = app_clone.emit("log", log);
        }
    });
    
    let result = mover.rollback_last().await;
    
    Ok(result)
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    settings
}

#[tauri::command]
fn save_settings(settings: Settings, app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    *state.settings.write() = settings.clone();
    save_settings_to_file(&settings)
}

#[tauri::command]
fn save_ui_scale(scale: u32, app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut settings = state.settings.read().clone();
    settings.ui_scale = Some(scale);
    *state.settings.write() = settings.clone();
    save_settings_to_file(&settings)
}

fn save_settings_to_file(settings: &Settings) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Could not determine config directory".to_string())?;
    
    let app_config_dir = config_dir.join("blocksmith");
    if std::fs::create_dir_all(&app_config_dir).is_err() {
        return Err("Failed to create config directory".to_string());
    }
    
    let settings_path = app_config_dir.join("settings.json");
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| e.to_string())?;
    
    std::fs::write(&settings_path, content)
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

fn load_settings_from_file() -> Settings {
    if let Some(config_dir) = dirs::config_dir() {
        let settings_path = config_dir.join("blocksmith").join("settings.json");
        
        if settings_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&settings_path) {
                if let Ok(mut settings) = serde_json::from_str::<Settings>(&content) {
                    // Reconcile background_style with theme so a mismatch never persists
                    let is_minecraft = settings.theme.as_deref() == Some("minecraft");
                    let bg = settings.background_style.as_deref().unwrap_or("");
                    if is_minecraft && (bg == "embers" || bg == "matrix") {
                        settings.background_style = Some("mc-terrain".to_string());
                    } else if !is_minecraft && (bg == "mc-terrain" || bg == "minecraft") {
                        settings.background_style = Some("embers".to_string());
                    }
                    return settings;
                }
            }
        }
    }
    
    auto_detect_mc_paths()
}

fn auto_detect_mc_paths() -> Settings {
    let mut settings = Settings::default();
    
    if let Some(roaming) = dirs::config_dir() {
        let mc_base = roaming.join("Minecraft Bedrock").join("Users");
        
        // Check Shared folder for all pack types
        let shared_path = mc_base.join("Shared").join("games").join("com.mojang");
        if shared_path.exists() {
            let bp = shared_path.join("behavior_packs");
            let rp = shared_path.join("resource_packs");
            let sp = shared_path.join("skin_packs");
            let wt = shared_path.join("world_templates");
            
            if bp.exists() {
                settings.behavior_pack_path = Some(bp.to_string_lossy().to_string());
            }
            if rp.exists() {
                settings.resource_pack_path = Some(rp.to_string_lossy().to_string());
            }
            if sp.exists() {
                settings.skin_pack_path = Some(sp.to_string_lossy().to_string());
            }
            if wt.exists() {
                settings.world_template_path = Some(wt.to_string_lossy().to_string());
            }
        }
    }
    
    // Auto-detect ToolCoin downloads path
    if let Some(home) = dirs::home_dir() {
        let toolcoin_downloads = home.join("Downloads").join("ToolCoin");
        if toolcoin_downloads.exists() {
            settings.scan_location = Some(toolcoin_downloads.to_string_lossy().to_string());
        }
    }
    
    settings
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Settings {
    let settings = load_settings_from_file();
    let state = app.state::<AppState>();
    *state.settings.write() = settings.clone();
    settings
}

#[tauri::command]
fn get_destination_for_pack_type(pack_type: PackType, app: AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let settings = state.settings.read();
    
    match pack_type {
        PackType::BehaviorPack => settings.behavior_pack_path.clone(),
        PackType::ResourcePack => settings.resource_pack_path.clone(),
        PackType::SkinPack => settings.skin_pack_path.clone(),
        PackType::SkinPack4D => settings.scan_location.as_ref().map(|s| {
            std::path::PathBuf::from(s).join("4D Skin Packs").to_string_lossy().into_owned()
        }),
        PackType::WorldTemplate | PackType::MashupPack => settings.world_template_path.clone(),
        PackType::Unknown => None,
    }
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let path = std::path::Path::new(&path);
    let target = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    
    let target_str = target.to_string_lossy().to_string();
    
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(&target_str)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    
    Ok(())
}

#[tauri::command]
fn auto_detect_paths(app: AppHandle) -> Settings {
    let detected = auto_detect_mc_paths();
    let state = app.state::<AppState>();
    let mut current = state.settings.read().clone();
    // Only update path fields — leave all other user preferences untouched
    if detected.behavior_pack_path.is_some() {
        current.behavior_pack_path = detected.behavior_pack_path;
    }
    if detected.resource_pack_path.is_some() {
        current.resource_pack_path = detected.resource_pack_path;
    }
    if detected.skin_pack_path.is_some() {
        current.skin_pack_path = detected.skin_pack_path;
    }
    if detected.world_template_path.is_some() {
        current.world_template_path = detected.world_template_path;
    }
    if detected.scan_location.is_some() {
        current.scan_location = detected.scan_location;
    }
    *state.settings.write() = current.clone();
    current
}

#[tauri::command]
fn get_premium_cache_packs() -> Result<Vec<PremiumCachePack>, String> {
    if let Some(roaming) = dirs::config_dir() {
        let premium_cache = roaming
            .join("Minecraft Bedrock")
            .join("premium_cache")
            .join("skin_packs");
        
        if !premium_cache.exists() {
            return Err("Premium cache folder not found. Open Minecraft and visit the skin packs section first.".to_string());
        }
        
        let mut packs = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&premium_cache) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let folder_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    
                    let display_name = get_pack_display_name(&path).unwrap_or_else(|| folder_name.clone());
                    
                    packs.push(PremiumCachePack {
                        folder_name: folder_name.clone(),
                        display_name,
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
        
        if packs.is_empty() {
            return Err("No premium skin packs found in cache. Download some from the Minecraft Marketplace first.".to_string());
        }
        
        packs.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        return Ok(packs);
    }
    
    Err("Could not find AppData folder".to_string())
}

fn get_pack_display_name(pack_path: &std::path::Path) -> Option<String> {
    let manifest_path = pack_path.join("manifest.json");
    let mut internal_name: Option<String> = None;
    
    if manifest_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(header) = json.get("header") {
                    if let Some(name) = header.get("name").and_then(|n| n.as_str()) {
                        internal_name = Some(name.to_string());
                    }
                }
                if internal_name.is_none() {
                    if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                        internal_name = Some(name.to_string());
                    }
                }
            }
        }
    }
    
    if let Some(ref int_name) = internal_name {
        let lang_path = pack_path.join("texts").join("en_US.lang");
        if lang_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&lang_path) {
                let search_key = format!("skinpack.{}=", int_name);
                for line in content.lines() {
                    if line.starts_with(&search_key) {
                        return Some(line.strip_prefix(&search_key).unwrap_or(line).to_string());
                    }
                }
            }
        }
    }
    
    let skins_json_path = pack_path.join("skins.json");
    if skins_json_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&skins_json_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(name) = json.get("localization_name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
                if let Some(name) = json.get("serialize_name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
            }
        }
    }
    
    None
}

#[tauri::command]
fn open_skinmaster(app: AppHandle) -> Result<(), String> {
    let temp_dir = std::env::temp_dir().join("Blocksmith");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    let skinmaster_path = temp_dir.join("SkinMaster.exe");

    std::fs::write(&skinmaster_path, SKINMASTER_EXE)
        .map_err(|e| format!("Failed to extract SkinMaster.exe: {}", e))?;

    std::process::Command::new(&skinmaster_path)
        .current_dir(&temp_dir)
        .spawn()
        .map_err(|e| format!("Failed to launch SkinMaster: {}", e))?;

    emit_log(&app, "INFO", "Launched SkinMaster");

    Ok(())
}

#[tauri::command]
fn open_premium_cache() -> Result<(), String> {
    if let Some(roaming) = dirs::config_dir() {
        let premium_cache = roaming
            .join("Minecraft Bedrock")
            .join("premium_cache")
            .join("skin_packs");
        
        if premium_cache.exists() {
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("explorer")
                    .arg(&premium_cache)
                    .spawn()
                    .map_err(|e| format!("Failed to open folder: {}", e))?;
            }
            return Ok(());
        }
    }
    
    Err("Premium cache folder not found".to_string())
}


fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_entry = entry.path();
        let dst_entry = dst.join(entry.file_name());
        
        if src_entry.is_dir() {
            std::fs::create_dir_all(&dst_entry).map_err(|e| e.to_string())?;
            copy_dir_recursive(&src_entry, &dst_entry)?;
        } else {
            std::fs::copy(&src_entry, &dst_entry).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn import_4d_skin_to_premium(
    skin_pack_path: String,
    premium_pack_path: String,
    app: AppHandle,
) -> Result<(), String> {
    emit_log(&app, "INFO", &format!("Importing 4D skin from '{}' to '{}'", skin_pack_path, premium_pack_path));
    
    let skin_path = std::path::Path::new(&skin_pack_path);
    let premium_path = std::path::Path::new(&premium_pack_path);
    
    let allowed_base = if let Some(roaming) = dirs::config_dir() {
        roaming.join("Minecraft Bedrock").join("premium_cache").join("skin_packs")
    } else {
        return Err("Could not determine AppData directory".to_string());
    };
    if !premium_path.starts_with(&allowed_base) {
        return Err("premium_pack_path is outside the premium cache skin_packs directory".to_string());
    }

    if !skin_path.exists() {
        return Err("4D skin pack folder does not exist".to_string());
    }
    
    if !premium_path.exists() {
        return Err("Premium pack folder does not exist".to_string());
    }
    
    let texts_folder = premium_path.join("texts");
    if texts_folder.exists() {
        std::fs::remove_dir_all(&texts_folder)
            .map_err(|e| format!("Failed to remove texts folder: {}", e))?;
        emit_log(&app, "INFO", "Removed existing texts folder");
    }
    
    for entry in std::fs::read_dir(skin_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = premium_path.join(&file_name);
        
        if file_name == "manifest.json" {
            emit_log(&app, "INFO", "Skipping manifest.json (keeping premium pack's manifest)");
            continue;
        }
        
        if src_path.is_dir() {
            if dst_path.exists() {
                std::fs::remove_dir_all(&dst_path)
                    .map_err(|e| format!("Failed to remove existing folder: {}", e))?;
            }
            std::fs::create_dir_all(&dst_path)
                .map_err(|e| format!("Failed to create folder: {}", e))?;
            
            copy_dir_recursive(&src_path, &dst_path)?;
            emit_log(&app, "INFO", &format!("Copied folder: {:?}", file_name));
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
            emit_log(&app, "INFO", &format!("Copied file: {:?}", file_name));
        }
    }
    
    emit_log(&app, "SUCCESS", "4D skin pack imported successfully! Restart Minecraft to see the changes.");
    
    Ok(())
}

#[tauri::command]
fn watch_premium_cache(app: AppHandle) -> Result<(), String> {
    let watching = app.state::<AppState>().watching.load(Ordering::SeqCst);
    if watching {
        return Err("Already watching".to_string());
    }
    
    let premium_cache = if let Some(roaming) = dirs::config_dir() {
        roaming.join("Minecraft Bedrock").join("premium_cache")
    } else {
        return Err("Could not find AppData folder".to_string());
    };
    
    if !premium_cache.exists() {
        return Err("Premium cache folder not found".to_string());
    }
    
    app.state::<AppState>().watching.store(true, Ordering::SeqCst);
    
    let (stop_tx, stop_rx) = std::sync::mpsc::sync_channel::<()>(0);
    *app.state::<AppState>().watch_stop_tx.lock() = Some(stop_tx);

    let app_clone = app.clone();
    
    std::thread::spawn(move || {
        let mut watcher: notify::RecommendedWatcher = match Watcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let timestamp = chrono::Local::now().format("%H:%M:%S%.3f").to_string();
                    
                    let event_type = match event.kind {
                        EventKind::Create(_) => "CREATE",
                        EventKind::Modify(_) => "MODIFY",
                        EventKind::Remove(_) => "DELETE",
                        EventKind::Any => "ANY",
                        EventKind::Access(_) => "ACCESS",
                        _ => "OTHER",
                    }.to_string();
                    
                    for path in event.paths.iter() {
                        let path_str = path.to_string_lossy().to_string();
                        let mut details: Option<String> = None;
                        
                        if path.extension().map(|e| e == "json").unwrap_or(false) && path.exists() {
                            if let Ok(content) = std::fs::read_to_string(path) {
                                if content.len() < 5000 {
                                    details = Some(content);
                                }
                            }
                        }
                        
                        let watcher_event = WatcherEvent {
                            timestamp: timestamp.clone(),
                            event_type: event_type.clone(),
                            path: path_str,
                            details,
                        };
                        
                        let _ = app_clone.emit("watcher-event", watcher_event);
                    }
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create watcher: {}", e);
                return;
            }
        };
        
        if let Err(e) = watcher.watch(&premium_cache, RecursiveMode::Recursive) {
            eprintln!("Failed to watch: {}", e);
            return;
        }
        
        emit_log(&app, "INFO", &format!("Watching: {}", premium_cache.display()));
        
        let _ = stop_rx.recv();
    });
    
    Ok(())
}

#[tauri::command]
fn stop_watching(app: AppHandle) -> Result<(), String> {
    app.state::<AppState>().watching.store(false, Ordering::SeqCst);
    if let Some(tx) = app.state::<AppState>().watch_stop_tx.lock().take() {
        let _ = tx.send(());
    }
    emit_log(&app, "INFO", "Stopped watching premium cache");
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackStats {
    pub pack_type: String,
    pub count: usize,
    pub total_size: u64,
    pub total_size_formatted: String,
}

#[tauri::command]
async fn get_installed_packs_stats(app: AppHandle) -> Result<Vec<PackStats>, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    
    let folders = vec![
        ("BehaviorPack", settings.behavior_pack_path.clone()),
        ("ResourcePack", settings.resource_pack_path.clone()),
        ("SkinPack", settings.skin_pack_path.clone()),
        ("WorldTemplate", settings.world_template_path.clone()),
    ];
    
    let stats: Vec<PackStats> = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;

        let mut results: Vec<PackStats> = Vec::new();
        let mut world_template_count = 0;
        let mut world_template_size = 0u64;
        let mut mashup_count = 0;
        let mut mashup_size = 0u64;

        for (pack_type, path_opt) in folders {
            if let Some(path_str) = path_opt {
                let path = std::path::Path::new(&path_str);
                if path.exists() {
                    let dirs: Vec<std::path::PathBuf> = std::fs::read_dir(path)
                        .ok()
                        .into_iter()
                        .flat_map(|entries| {
                            entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir())
                        })
                        .collect();

                    if pack_type == "WorldTemplate" {
                        for dir in &dirs {
                            let name = dir.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let is_mashup = name.contains("mashup")
                                || name.contains("mash-up")
                                || name.contains("mash up");
                            let size = calculate_folder_size(dir);
                            if is_mashup {
                                mashup_count += 1;
                                mashup_size += size;
                            } else {
                                world_template_count += 1;
                                world_template_size += size;
                            }
                        }
                    } else {
                        let count = dirs.len();
                        let total_size: u64 = dirs.par_iter()
                            .map(|dir| calculate_folder_size(dir))
                            .sum();
                        results.push(PackStats {
                            pack_type: pack_type.to_string(),
                            count,
                            total_size,
                            total_size_formatted: format_bytes(total_size),
                        });
                    }
                }
            }
        }

        if world_template_count > 0 {
            results.push(PackStats {
                pack_type: "WorldTemplate".to_string(),
                count: world_template_count,
                total_size: world_template_size,
                total_size_formatted: format_bytes(world_template_size),
            });
        }

        if mashup_count > 0 {
            results.push(PackStats {
                pack_type: "MashupPack".to_string(),
                count: mashup_count,
                total_size: mashup_size,
                total_size_formatted: format_bytes(mashup_size),
            });
        }

        results
    }).await.map_err(|e| e.to_string())?;
    
    Ok(stats)
}

#[tauri::command]
fn launch_minecraft(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "minecraft:"])
            .spawn()
            .map_err(|e| format!("Failed to launch Minecraft: {}", e))?;
        emit_log(&app, "INFO", "Launched Minecraft");
    }
    Ok(())
}

#[tauri::command]
fn check_toolcoin_installed() -> bool {
    let toolcoin_path = std::path::Path::new("C:\\Program Files\\alphtoolcoin\\ToolCoin.exe");
    toolcoin_path.exists()
}

#[tauri::command]
fn launch_toolcoin(app: AppHandle) -> Result<(), String> {
    let toolcoin_path = std::path::Path::new("C:\\Program Files\\alphtoolcoin\\ToolCoin.exe");
    
    if toolcoin_path.exists() {
        std::process::Command::new(toolcoin_path)
            .spawn()
            .map_err(|e| format!("Failed to launch ToolCoin: {}", e))?;
        emit_log(&app, "INFO", "Launched ToolCoin");
        Ok(())
    } else {
        Err("ToolCoin is not installed. Please install it from https://github.com/MrLabRat/ToolCoin".to_string())
    }
}

#[tauri::command]
fn delete_all_packs(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    
    let folders = vec![
        ("Behavior Packs", settings.behavior_pack_path.clone()),
        ("Resource Packs", settings.resource_pack_path.clone()),
        ("Skin Packs", settings.skin_pack_path.clone()),
        ("World Templates", settings.world_template_path.clone()),
    ];
    
    for (name, path_opt) in folders {
        if let Some(path_str) = path_opt {
            let path = std::path::Path::new(&path_str);
            if path.exists() {
                for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        std::fs::remove_dir_all(&entry_path)
                            .map_err(|e| format!("Failed to delete {:?}: {}", entry_path, e))?;
                        emit_log(&app, "INFO", &format!("Deleted: {:?}", entry_path));
                    }
                }
                emit_log(&app, "INFO", &format!("Cleared {} folder", name));
            }
        }
    }
    
    emit_log(&app, "SUCCESS", "All pack folders have been cleared!");
    Ok(())
}

#[tauri::command]
async fn get_directory_folders(app: AppHandle) -> Result<Vec<PackInfo>, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    
    let pack_folders = vec![
        ("BehaviorPack", settings.behavior_pack_path.clone()),
        ("ResourcePack", settings.resource_pack_path.clone()),
        ("SkinPack", settings.skin_pack_path.clone()),
        ("WorldTemplate", settings.world_template_path.clone()),
    ];
    
    let all_folders: Vec<PackInfo> = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;

        let mut folder_paths: Vec<(String, String, String)> = Vec::new();

        for (pack_type_str, path_opt) in &pack_folders {
            if let Some(path_str) = path_opt {
                let path = std::path::Path::new(path_str);
                if path.exists() && path.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(path) {
                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if entry_path.is_dir() {
                                let folder_name = entry_path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Unknown")
                                    .to_string();
                                folder_paths.push((
                                    entry_path.to_string_lossy().to_string(),
                                    folder_name,
                                    pack_type_str.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        let mut final_results: Vec<PackInfo> = folder_paths
            .into_par_iter()
            .map(|(path, folder_name, pack_type_str)| {
                let entry_path = std::path::Path::new(&path);
                let (uuid, display_name, version) = read_pack_metadata_fast(entry_path);
                let icon = read_pack_icon(entry_path);
                let name_lower = folder_name.to_lowercase();
                let is_mashup = name_lower.contains("mashup")
                    || name_lower.contains("mash-up")
                    || name_lower.contains("mash up");
                let pack_type = if is_mashup && pack_type_str == "WorldTemplate" {
                    PackType::MashupPack
                } else {
                    parse_pack_type(&pack_type_str)
                };
                PackInfo {
                    path: path.clone(),
                    name: display_name.unwrap_or_else(|| folder_name.clone()),
                    pack_type,
                    uuid,
                    version,
                    extracted: false,
                    icon_base64: icon,
                    subfolder: None,
                    folder_size: None,
                    folder_size_formatted: None,
                    needs_attention: None,
                    attention_message: None,
                    is_installed: None,
                    is_update: None,
                    installed_version: None,
                }
            })
            .collect();

        final_results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        final_results
    }).await.map_err(|e| e.to_string())?;
    
    Ok(all_folders)
}

fn read_pack_icon(folder_path: &std::path::Path) -> Option<String> {
    let icon_names = ["pack_icon.png", "Pack_Icon.png", "world_icon.jpeg", "world_icon.jpg", "icon.png"];
    const MAX_ICON_SIZE: u64 = 2 * 1024 * 1024;
    
    for icon_name in &icon_names {
        let icon_path = folder_path.join(icon_name);
        if icon_path.exists() {
            if icon_path.metadata().map(|m| m.len()).unwrap_or(u64::MAX) > MAX_ICON_SIZE {
                continue;
            }
            if let Ok(icon_data) = std::fs::read(&icon_path) {
                return Some(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &icon_data));
            }
        }
    }
    
    None
}

fn read_pack_metadata_fast(folder_path: &std::path::Path) -> (Option<String>, Option<String>, Option<String>) {
    let manifest_path = folder_path.join("manifest.json");
    
    if manifest_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let uuid = json.get("header")
                    .and_then(|h| h.get("uuid"))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string());
                
                let name = json.get("header")
                    .and_then(|h| h.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                
                let version = json.get("header")
                    .and_then(|h| h.get("version"))
                    .and_then(|v| {
                        if let Some(arr) = v.as_array() {
                            Some(
                                arr.iter()
                                    .filter_map(|n| n.as_u64())
                                    .map(|n| n.to_string())
                                    .collect::<Vec<_>>()
                                    .join("."),
                            )
                        } else if let Some(s) = v.as_str() {
                            Some(s.to_string())
                        } else {
                            None
                        }
                    });
                
                return (uuid, name, version);
            }
        }
    }
    
    (None, None, None)
}

fn extract_base_name(name: &str) -> String {
    let mut cleaned = name.to_lowercase();
    
    // Remove common suffixes first
    let suffixes = [
        " (addon)", "(addon)", " (add-on)", "(add-on)",
        " (resource)", "(resource)", " (resources)", "(resources)",
        " (behavior)", "(behavior)", " (behaviour)", "(behaviour)",
        " (bp)", "(bp)", " (rp)", "(rp)",
        " (skin)", "(skin)", " (skins)", "(skins)",
        " (template)", "(template)", " (world_template)", "(world_template)",
        " (mashup)", "(mashup)", " (mash-up)", "(mash-up)",
    ];
    
    for suffix in &suffixes {
        if cleaned.ends_with(suffix) {
            cleaned = cleaned[..cleaned.len() - suffix.len()].to_string();
        }
    }
    
    // Remove version patterns using pre-compiled regex (early-exit after first match)
    let version_patterns = [
        &VERSION_PATTERN_1,
        &VERSION_PATTERN_2,
        &VERSION_PATTERN_3,
        &VERSION_PATTERN_4,
    ];
    for pattern in &version_patterns {
        let result = pattern.replace(&cleaned, "");
        if result.len() != cleaned.len() {
            cleaned = result.into_owned();
            break;
        }
    }
    
    cleaned.trim().to_string()
}

fn extract_version_from_name(name: &str) -> Option<String> {
    let name_lower = name.to_lowercase();
    
    // Try each pre-compiled pattern (order matters - more specific first)
    let patterns: &[&Lazy<Regex>] = &[
        &EXTRACT_VERSION_1,  // "V.1.0.1" or ".1.0.1"
        &EXTRACT_VERSION_2,  // "v1.0.1"
        &EXTRACT_VERSION_3,  // " 1.8.1 ("
        &EXTRACT_VERSION_4,  // " 1.8.1" at end
        &EXTRACT_VERSION_5,  // " 1 ("
        &EXTRACT_VERSION_6,  // " 1.1 " (version surrounded by spaces)
    ];
    
    for pattern in patterns {
        if let Some(caps) = pattern.captures(&name_lower) {
            if let Some(ver) = caps.get(1) {
                return Some(ver.as_str().to_string());
            }
        }
    }
    
    None
}

fn extract_version_from_path(path: &str) -> Option<String> {
    // Extract filename/foldername from path
    let name = path.split(|c| c == '/' || c == '\\').last().unwrap_or(path);
    
    // Remove extension if present
    let name_without_ext = name
        .trim_end_matches(".mcpack")
        .trim_end_matches(".mcaddon")
        .trim_end_matches(".mctemplate");
    
    // First try: extract version from the name/folder name
    if let Some(v) = extract_version_from_name(name_without_ext) {
        return Some(v);
    }
    
    // Second try: strip type suffixes first, then extract version
    let suffixes = [
        " (ADDON)", "(ADDON)", " (addon)", "(addon)",
        " (RESOURCE)", "(RESOURCE)", " (resource)", "(resource)",
        " (SKIN)", "(SKIN)", " (skin)", "(skin)",
        " (TEMPLATE)", "(TEMPLATE)", " (template)", "(template)",
        " (MASHUP)", "(MASHUP)", " (mashup)", "(mashup)",
    ];
    
    let mut cleaned = name_without_ext.to_string();
    for suffix in &suffixes {
        if cleaned.ends_with(suffix) {
            cleaned = cleaned[..cleaned.len() - suffix.len()].to_string();
            break;
        }
    }
    
    extract_version_from_name(&cleaned)
}

struct InstalledPackInfo {
    uuid: Option<String>,
    name: String,
    pack_type: PackType,
    version: Option<String>,
    path: String,
    folder_name: String,
}

fn get_installed_packs_info(app: &AppHandle) -> Vec<InstalledPackInfo> {
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    
    let pack_folders = vec![
        ("BehaviorPack", settings.behavior_pack_path.clone()),
        ("ResourcePack", settings.resource_pack_path.clone()),
        ("SkinPack", settings.skin_pack_path.clone()),
        ("WorldTemplate", settings.world_template_path.clone()),
    ];
    
    let mut installed_packs: Vec<InstalledPackInfo> = Vec::new();
    
    for (pack_type_str, path_opt) in &pack_folders {
        if let Some(path_str) = path_opt {
            let path = std::path::Path::new(path_str);
            if path.exists() && path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            let folder_name = entry_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .to_string();
                            
                            let (uuid, display_name, version) = read_pack_metadata_fast(&entry_path);
                            
                            let name_lower = folder_name.to_lowercase();
                            let is_mashup = name_lower.contains("mashup") 
                                || name_lower.contains("mash-up") 
                                || name_lower.contains("mash up");
                            
                            let pack_type = if is_mashup && pack_type_str == &"WorldTemplate" {
                                PackType::MashupPack
                            } else {
                                parse_pack_type(pack_type_str)
                            };
                            
                            installed_packs.push(InstalledPackInfo {
                                uuid,
                                name: display_name.clone().unwrap_or_else(|| folder_name.clone()),
                                pack_type,
                                version,
                                path: entry_path.to_string_lossy().to_string(),
                                folder_name: folder_name.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    
    installed_packs
}

#[tauri::command]
async fn get_all_folder_sizes(paths: Vec<String>) -> Result<Vec<(String, u64, String)>, String> {
    let results: Vec<(String, u64, String)> = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;
        paths.into_par_iter()
            .filter_map(|path| {
                let folder_path = std::path::Path::new(&path);
                if folder_path.exists() && folder_path.is_dir() {
                    let size = calculate_folder_size(folder_path);
                    let formatted = format_bytes(size);
                    Some((path, size, formatted))
                } else {
                    None
                }
            })
            .collect()
    }).await.map_err(|e| e.to_string())?;
    
    Ok(results)
}

#[tauri::command]
fn get_folder_size(path: String) -> Result<(u64, String), String> {
    let folder_path = std::path::Path::new(&path);
    if !folder_path.exists() || !folder_path.is_dir() {
        return Err(format!("Path does not exist or is not a directory: {}", path));
    }
    
    let size = calculate_folder_size(folder_path);
    let formatted = format_bytes(size);
    Ok((size, formatted))
}

fn is_within_configured_dirs(path: &std::path::Path, app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let settings = state.settings.read();
    let configured: Vec<String> = [
        settings.behavior_pack_path.as_ref(),
        settings.resource_pack_path.as_ref(),
        settings.skin_pack_path.as_ref(),
        settings.skin_pack_4d_path.as_ref(),
        settings.world_template_path.as_ref(),
        settings.scan_location.as_ref(),
    ]
    .into_iter()
    .flatten()
    .cloned()
    .collect();

    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    configured.iter().any(|dir| {
        let base = std::path::Path::new(dir);
        let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
        canonical_path.starts_with(&canonical_base)
    })
}

#[tauri::command]
fn delete_pack(path: String, app: AppHandle) -> Result<(), String> {
    let folder_path = std::path::Path::new(&path);
    if !is_within_configured_dirs(folder_path, &app) {
        return Err("Path is outside configured pack directories".to_string());
    }
    if !folder_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    
    std::fs::remove_dir_all(folder_path)
        .map_err(|e| format!("Failed to delete pack: {}", e))
}

#[tauri::command]
fn move_pack(path: String, destination: String, app: AppHandle) -> Result<String, String> {
    let source_path = std::path::Path::new(&path);
    let dest_path = std::path::Path::new(&destination);
    
    if !is_within_configured_dirs(source_path, &app) {
        return Err("Source path is outside configured pack directories".to_string());
    }
    if !is_within_configured_dirs(dest_path, &app) {
        return Err("Destination is outside configured pack directories".to_string());
    }

    if !source_path.exists() {
        return Err(format!("Source path does not exist: {}", path));
    }
    
    let folder_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");
    
    let final_dest = dest_path.join(folder_name);
    
    if final_dest.exists() {
        return Err(format!("Destination already exists: {}", final_dest.display()));
    }
    
    std::fs::rename(source_path, &final_dest)
        .map_err(|e| format!("Failed to move pack: {}", e))?;
    
    Ok(final_dest.to_string_lossy().to_string())
}

#[tauri::command]
fn rename_pack(path: String, new_name: String, app: AppHandle) -> Result<String, String> {
    if new_name.contains('/') || new_name.contains('\\') || new_name.contains("..") {
        return Err("Invalid name: must not contain path separators or '..'".to_string());
    }
    let folder_path = std::path::Path::new(&path);
    if !is_within_configured_dirs(folder_path, &app) {
        return Err("Path is outside configured pack directories".to_string());
    }
    if !folder_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    
    let parent = folder_path.parent()
        .ok_or("Cannot rename root directory")?;
    
    let new_path = parent.join(&new_name);
    
    if new_path.exists() {
        return Err(format!("A folder named '{}' already exists", new_name));
    }
    
    std::fs::rename(folder_path, &new_path)
        .map_err(|e| format!("Failed to rename pack: {}", e))?;
    
    Ok(new_path.to_string_lossy().to_string())
}

#[tauri::command]
fn delete_packs(paths: Vec<String>, app: AppHandle) -> Result<Vec<String>, String> {
    let mut deleted = Vec::new();
    let mut errors = Vec::new();
    
    for path in paths {
        let folder_path = std::path::Path::new(&path);
        if !is_within_configured_dirs(folder_path, &app) {
            errors.push(format!("{}: outside configured pack directories", path));
            continue;
        }
        match std::fs::remove_dir_all(&path) {
            Ok(_) => deleted.push(path),
            Err(e) => errors.push(format!("{}: {}", path, e)),
        }
    }
    
    if !errors.is_empty() {
        return Err(format!("Some deletions failed: {}", errors.join("; ")));
    }
    
    Ok(deleted)
}

#[tauri::command]
fn delete_source_file(path: String, app: AppHandle) -> Result<(), String> {
    let file_path = std::path::Path::new(&path);
    let allowed_extensions = ["mcpack", "mcaddon", "mctemplate"];
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !allowed_extensions.contains(&ext.as_str()) {
        return Err(format!("Not a pack file: {}", path));
    }
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }
    let state = app.state::<AppState>();
    let settings = state.settings.read();
    let scan_location = settings.scan_location.as_deref().unwrap_or("");
    if scan_location.is_empty() {
        return Err("No scan location configured".to_string());
    }
    let parent = file_path
        .parent()
        .ok_or_else(|| "Could not determine file parent directory".to_string())?;
    let canonical_parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
    let canonical_scan = std::path::Path::new(scan_location)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(scan_location));
    let parent_str = canonical_parent.to_string_lossy().to_lowercase();
    let scan_str = canonical_scan.to_string_lossy().to_lowercase();
    if parent_str != scan_str {
        return Err("File is outside the scan folder".to_string());
    }
    std::fs::remove_file(file_path)
        .map_err(|e| format!("Failed to delete file: {}", e))
}

#[tauri::command]
async fn get_all_pack_icons(paths: Vec<String>) -> Result<Vec<(String, Option<String>)>, String> {
    let results: Vec<(String, Option<String>)> = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;
        paths.into_par_iter()
            .map(|path| {
                let icon = read_pack_icon(std::path::Path::new(&path));
                (path, icon)
            })
            .collect()
    }).await.map_err(|e| e.to_string())?;
    
    Ok(results)
}

#[tauri::command]
fn get_pack_icon(path: String) -> Option<String> {
    let folder_path = std::path::Path::new(&path);
    if !folder_path.exists() || !folder_path.is_dir() {
        return None;
    }
    
    read_pack_icon(folder_path)
}

#[tauri::command]
fn is_debug_mode(app: AppHandle) -> bool {
    let state = app.state::<AppState>();
    state.debug_mode.load(std::sync::atomic::Ordering::Relaxed)
}

#[tauri::command]
fn get_pack_info(path: String) -> Option<(String, String)> {
    // Returns (uuid, name) from manifest.json if found
    let folder_path = std::path::Path::new(&path);
    if !folder_path.exists() || !folder_path.is_dir() {
        return None;
    }
    
    let manifest_path = folder_path.join("manifest.json");
    if !manifest_path.exists() {
        return None;
    }
    
    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            let uuid = json.get("header")
                .and_then(|h| h.get("uuid"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string());
            
            let name = json.get("header")
                .and_then(|h| h.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
            
            if let (Some(uuid), Some(name)) = (uuid, name) {
                return Some((uuid, name));
            }
        }
    }
    
    None
}

#[tauri::command]
fn export_debug_log() -> Result<String, String> {
    let mut log_content = String::new();
    log_content.push_str("=== Blocksmith Debug Log ===\n");
    log_content.push_str(&format!("Timestamp: {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
    log_content.push_str("\n--- Environment ---\n");
    
    // Add system info
    if let Ok(os) = std::env::var("OS") {
        log_content.push_str(&format!("OS: {}\n", os));
    }
    if let Some(home) = dirs::home_dir() {
        log_content.push_str(&format!("Home: {}\n", home.display()));
    }
    if let Some(config) = dirs::config_dir() {
        log_content.push_str(&format!("Config Dir: {}\n", config.display()));
    }
    
    log_content.push_str("\n--- App Info ---\n");
    log_content.push_str(&format!("Version: {}\n", env!("CARGO_PKG_VERSION")));
    
    Ok(log_content)
}

#[tauri::command]
async fn set_window_icon(style: String, bordered: bool, app: AppHandle) -> Result<(), String> {
    let icon_name = if style == "default" {
        if bordered { "defaultborder" } else { "defaultnoborder" }
    } else {
        if bordered { "blackredborder" } else { "blackrednoborder" }
    };

    emit_log(&app, "INFO", &format!("Setting icon: {}", icon_name));

    let bytes = icon_bytes_for(icon_name)
        .ok_or_else(|| format!("Unknown icon: {}", icon_name))?;

    let icon = decode_icon(bytes)
        .ok_or_else(|| format!("Failed to decode icon: {}", icon_name))?;

    let window = app.get_webview_window("main")
        .ok_or("Main window not found")?;

    window.set_icon(icon)
        .map_err(|e| {
            let msg = format!("Failed to set icon: {}", e);
            emit_log(&app, "ERROR", &msg);
            msg
        })?;

    emit_log(&app, "INFO", &format!("Window icon changed to: {}", icon_name));

    Ok(())
}

#[tauri::command]
fn minimize_window(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main")
        .ok_or("Main window not found")?;
    window.minimize().map_err(|e| format!("Failed to minimize: {}", e))?;
    Ok(())
}

#[tauri::command]
fn maximize_window(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main")
        .ok_or("Main window not found")?;
    let is_maximized = window.is_maximized().unwrap_or(false);
    if is_maximized {
        window.unmaximize().map_err(|e| format!("Failed to unmaximize: {}", e))?;
    } else {
        window.maximize().map_err(|e| format!("Failed to maximize: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn close_window(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main")
        .ok_or("Main window not found")?;
    window.close().map_err(|e| format!("Failed to close: {}", e))?;
    Ok(())
}

fn calculate_folder_size(path: &std::path::Path) -> u64 {
    let mut size = 0;
    let mut stack = vec![path.to_path_buf()];
    
    while let Some(current_path) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current_path) {
            for entry in entries.flatten() {
                match entry.metadata() {
                    Ok(metadata) => {
                        if metadata.is_dir() {
                            stack.push(entry.path());
                        } else {
                            size += metadata.len();
                        }
                    }
                    Err(_) => {
                        // Skip files/dirs we can't read metadata for
                        continue;
                    }
                }
            }
        }
    }
    size
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let bytes_f = bytes as f64;
    let mut size = bytes_f;
    let mut unit_idx = 0;
    
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

fn parse_pack_type(type_str: &str) -> PackType {
    match type_str {
        "BehaviorPack" => PackType::BehaviorPack,
        "ResourcePack" => PackType::ResourcePack,
        "SkinPack" => PackType::SkinPack,
        "SkinPack4D" => PackType::SkinPack4D,
        "WorldTemplate" => PackType::WorldTemplate,
        "MashupPack" => PackType::MashupPack,
        _ => PackType::Unknown,
    }
}

fn emit_log(app: &AppHandle, level: &str, message: &str) {
    let log = LogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        level: level.to_string(),
        message: message.to_string(),
    };
    let _ = app.emit("log", log);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = load_settings_from_file();
    let icon_style = settings.taskbar_icon_style.clone().unwrap_or_else(|| "blackred".to_string());
    let icon_bordered = settings.taskbar_icon_border.unwrap_or(false);
    
    let debug_mode = std::env::args().any(|arg| arg == "--debug") || {
        if let Some(config_dir) = dirs::config_dir() {
            let debug_file = config_dir.join("blocksmith").join(".debug");
            debug_file.exists()
        } else {
            false
        }
    };
    
    if debug_mode {
        eprintln!("[DEBUG] Debug mode enabled");
    }
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            settings: RwLock::new(settings),
            watching: AtomicBool::new(false),
            debug_mode: AtomicBool::new(debug_mode),
            watch_stop_tx: parking_lot::Mutex::new(None),
        })
        .setup(move |app| {
            let icon_name = if icon_style == "default" {
                if icon_bordered { "defaultborder" } else { "defaultnoborder" }
            } else {
                if icon_bordered { "blackredborder" } else { "blackrednoborder" }
            };

            if let Some(icon) = icon_bytes_for(icon_name).and_then(decode_icon) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_icon(icon);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_packs,
            process_packs,
            rollback_last,
            get_settings,
            save_settings,
            load_settings,
            get_destination_for_pack_type,
            open_folder,
            auto_detect_paths,
            get_premium_cache_packs,
            open_skinmaster,
            open_premium_cache,
            import_4d_skin_to_premium,
            watch_premium_cache,
            stop_watching,
            get_installed_packs_stats,
            launch_minecraft,
            launch_toolcoin,
            check_toolcoin_installed,
            delete_all_packs,
            get_directory_folders,
            get_all_folder_sizes,
            get_folder_size,
            get_all_pack_icons,
            delete_pack,
            move_pack,
            rename_pack,
            delete_packs,
            delete_source_file,
            get_pack_icon,
            is_debug_mode,
            export_debug_log,
            get_pack_info,
            set_window_icon,
            minimize_window,
            maximize_window,
            close_window,
            save_ui_scale,
            compute_pack_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
