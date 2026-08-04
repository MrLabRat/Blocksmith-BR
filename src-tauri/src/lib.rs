use zip::ZipArchive;

mod modules;

use modules::{
    archive_rejection_reason, scan_single_pack, FileMover, LogEntry, MoveOperation, PackInfo,
    PackType, Settings,
};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

static ICON_BLACKRED_NOBORDER: &[u8] = include_bytes!("../icons/blackrednoborder.png");
static ICON_BLACKRED_BORDER: &[u8] = include_bytes!("../icons/blackredborder.png");
static ICON_DEFAULT_NOBORDER: &[u8] = include_bytes!("../icons/defaultnoborder.png");
static ICON_DEFAULT_BORDER: &[u8] = include_bytes!("../icons/defaultborder.png");
static SKINMASTER_EXE: &[u8] = include_bytes!("../resources/SkinMaster.exe");

fn icon_bytes_for(name: &str) -> Option<&'static [u8]> {
    match name {
        "blackrednoborder" => Some(ICON_BLACKRED_NOBORDER),
        "blackredborder" => Some(ICON_BLACKRED_BORDER),
        "defaultnoborder" => Some(ICON_DEFAULT_NOBORDER),
        "defaultborder" => Some(ICON_DEFAULT_BORDER),
        _ => None,
    }
}

fn decode_icon(bytes: &[u8]) -> Option<tauri::image::Image<'static>> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(tauri::image::Image::new_owned(
        rgba.into_raw(),
        width,
        height,
    ))
}

static VERSION_PATTERN_1: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+v?\.\d+(\.\d+)*$").unwrap());
static VERSION_PATTERN_2: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+v\d+(\.\d+)*$").unwrap());
static VERSION_PATTERN_3: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+\d+(\.\d+)+$").unwrap());
static VERSION_PATTERN_4: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+\d+$").unwrap());

static EXTRACT_VERSION_1: Lazy<Regex> = Lazy::new(|| Regex::new(r"[vV]\.(\d+(?:\.\d+)*)").unwrap());
static EXTRACT_VERSION_2: Lazy<Regex> = Lazy::new(|| Regex::new(r"v(\d+(?:\.\d+)*)").unwrap());
static EXTRACT_VERSION_3: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s(\d+(?:\.\d+)+)\s*\(").unwrap());
static EXTRACT_VERSION_4: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s(\d+(?:\.\d+)+)$").unwrap());
static EXTRACT_VERSION_5: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s(\d+)\s*\(").unwrap());
static EXTRACT_VERSION_6: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s(\d+(?:\.\d+)*)\s").unwrap());

struct AppState {
    settings: RwLock<Settings>,
    move_history: modules::MoveHistory,
    watching: AtomicBool,
    debug_mode: AtomicBool,
    watch_stop_tx: parking_lot::Mutex<Option<std::sync::mpsc::SyncSender<()>>>,
    scan_watching: AtomicBool,
    scan_watch_stop_tx: parking_lot::Mutex<Option<std::sync::mpsc::Sender<()>>>,
    cancel_requested: AtomicBool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatePack {
    pub uuid: String,
    pub name: String,
    pub pack_type: String,
    pub path: String,
    pub folder_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub uuid: String,
    pub name: String,
    pub packs: Vec<DuplicatePack>,
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
fn resolve_scan_directory(paths: Vec<String>) -> Result<String, String> {
    if paths.is_empty() {
        return Err("No paths provided".to_string());
    }
    let first = std::path::Path::new(&paths[0]);
    if paths.len() == 1 && first.is_dir() {
        return Ok(first.to_string_lossy().to_string());
    }
    let parent = first
        .parent()
        .ok_or_else(|| "Could not determine parent directory".to_string())?;
    Ok(parent.to_string_lossy().to_string())
}

#[tauri::command]
fn request_cancel(app: AppHandle) {
    app.state::<AppState>()
        .cancel_requested
        .store(true, Ordering::SeqCst);
}

#[tauri::command]
async fn scan_packs(directory: String, app: AppHandle) -> Result<Vec<PackInfo>, String> {
    emit_log(&app, "INFO", &format!("Scanning directory: {}", directory));
    app.state::<AppState>()
        .cancel_requested
        .store(false, Ordering::SeqCst);

    let path = std::path::Path::new(&directory);
    if !path.exists() {
        emit_log(&app, "ERROR", "Directory does not exist");
        return Err("Directory does not exist".to_string());
    }

    let _ = app.emit(
        "progress",
        serde_json::json!({
            "current": 0,
            "total": 0,
            "message": "Finding pack files..."
        }),
    );

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

    emit_log(
        &app,
        "INFO",
        &format!("Found {} pack files to scan", total_files),
    );

    let _ = app.emit(
        "progress",
        serde_json::json!({
            "current": 0,
            "total": total_files,
            "message": "Scanning packs in parallel..."
        }),
    );

    let app_for_progress = app.clone();
    let progress_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_for_progress = total_files;
    let progress_last_emit = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // `files` is moved into the blocking closure (instead of cloned) and the
    // per-file size lookup is computed in the same rayon pass instead of a
    // second sequential `std::fs::metadata` loop afterward.
    //
    // This runs on a bounded custom thread pool (leaving one logical core
    // free) whose worker threads enter Windows background-processing mode,
    // same as `get_directory_folders`. Previously this used rayon's bare
    // `.par_iter()` (the GLOBAL pool, all cores, normal priority), which under
    // heavy archive extraction/decompression could starve the system badly
    // enough that the transparent app window (see tauri.conf.json
    // `transparent: true`) failed to composite any frame and showed straight
    // through to the desktop ("frozen + transparent" symptom).
    let (mut packs, size_cache) = tokio::task::spawn_blocking(
        move || -> Result<(Vec<PackInfo>, std::collections::HashMap<String, u64>), String> {
            use rayon::prelude::*;

            let counter = Arc::clone(&progress_counter);
            let last_emit = Arc::clone(&progress_last_emit);
            let app_clone = app_for_progress.clone();

            let pool = build_background_pool()?;

            let packs: Vec<PackInfo> = pool.install(|| {
                files
                    .par_iter()
                    .flat_map(|file| {
                        if app_for_progress
                            .state::<AppState>()
                            .cancel_requested
                            .load(std::sync::atomic::Ordering::SeqCst)
                        {
                            return vec![];
                        }

                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            scan_single_pack(file)
                        }));

                        let current = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                        let last = last_emit.load(std::sync::atomic::Ordering::SeqCst);
                        if current == total_for_progress || current.saturating_sub(last) >= 5 {
                            last_emit.store(current, std::sync::atomic::Ordering::SeqCst);
                            let _ = app_clone.emit(
                                "progress",
                                serde_json::json!({
                                    "current": current,
                                    "total": total_for_progress,
                                    "message": format!("Scanned {}/{}", current, total_for_progress)
                                }),
                            );
                        }

                        match result {
                            Ok(p) => {
                                if p.is_empty() {
                                    if let Some(reason) = archive_rejection_reason(file) {
                                        emit_log(
                                            &app_clone,
                                            "WARNING",
                                            &format!("Skipped '{}': {}", file.display(), reason),
                                        );
                                    }
                                }
                                p
                            }
                            Err(_) => {
                                eprintln!("Panic while scanning: {:?}", file);
                                vec![]
                            }
                        }
                    })
                    .collect()
            });

            let size_cache: std::collections::HashMap<String, u64> = pool.install(|| {
                files
                    .par_iter()
                    .filter_map(|file| {
                        std::fs::metadata(file)
                            .ok()
                            .map(|metadata| (file.to_string_lossy().to_string(), metadata.len()))
                    })
                    .collect()
            });

            Ok((packs, size_cache))
        },
    )
    .await
    .map_err(|e| format!("Scan failed: {}", e))??;

    if app
        .state::<AppState>()
        .cancel_requested
        .load(Ordering::SeqCst)
    {
        emit_log(&app, "WARNING", "Scan cancelled by user");
        let _ = app.emit(
            "progress",
            serde_json::json!({ "current": 0, "total": 0, "message": "Cancelled" }),
        );
        return Ok(packs);
    }

    emit_log(
        &app,
        "INFO",
        &format!("Found {} packs in {} files", packs.len(), total_files),
    );

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
        if settings.remember_scan_location.unwrap_or(true) {
            let _ = save_settings_to_file(&settings);
        }
    }

    let _ = app.emit(
        "progress",
        serde_json::json!({
            "current": total_files,
            "total": total_files,
            "message": "Scan complete",
            "estimated_seconds": 0
        }),
    );

    Ok(packs)
}

#[tauri::command]
async fn compute_pack_status(
    packs: Vec<PackInfo>,
    app: AppHandle,
) -> Result<Vec<PackInfo>, String> {
    let app_for_emit = app.clone();
    tokio::task::spawn_blocking(move || {
        let installed_packs = get_installed_packs_info(&app_for_emit);
        let installed_by_uuid: std::collections::HashMap<&str, usize> = installed_packs
            .iter()
            .enumerate()
            .filter_map(|(idx, ip)| ip.uuid.as_deref().map(|u| (u, idx)))
            .collect();
        let installed_base_names: std::collections::HashMap<(PackType, String), usize> =
            installed_packs
                .iter()
                .enumerate()
                .map(|(idx, ip)| ((ip.pack_type, extract_base_name(&ip.name)), idx))
                .collect();
        let mut size_cache: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        let mut results = packs;

        for pack in &mut results {
            let installed_index = if let Some(uuid) = pack.uuid.as_deref() {
                installed_by_uuid.get(uuid).copied()
            } else {
                let pack_base = extract_base_name(&pack.name);
                installed_base_names
                    .get(&(pack.pack_type, pack_base))
                    .copied()
            };

            if let Some(idx) = installed_index {
                let installed = &installed_packs[idx];
                let uuid_match = pack.uuid.is_some() && pack.uuid == installed.uuid;

                let new_ver: Option<String> = if uuid_match {
                    extract_version_from_name(&pack.name)
                        .or_else(|| extract_version_from_path(&pack.path))
                        .or_else(|| pack.version.clone())
                } else {
                    pack.version
                        .clone()
                        .or_else(|| extract_version_from_name(&pack.name))
                        .or_else(|| extract_version_from_path(&pack.path))
                };

                let old_ver: Option<String> = if uuid_match {
                    extract_version_from_name(&installed.folder_name)
                        .or_else(|| extract_version_from_path(&installed.path))
                        .or_else(|| installed.version.clone())
                } else {
                    installed
                        .version
                        .clone()
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
                        let old_size =
                            size_cache.entry(installed.path.clone()).or_insert_with(|| {
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
    validate_process_packs(&packs, settings.scan_location.as_deref())?;
    state.cancel_requested.store(false, Ordering::SeqCst);

    let total = packs.len();
    let delete_source = settings.delete_source;
    let (log_tx, mut log_rx) = mpsc::unbounded_channel();

    let mut mover = FileMover::with_history(settings.clone(), Arc::clone(&state.move_history));
    mover.set_log_sender(log_tx);
    let mover = Arc::new(mover);

    let scan_dir = settings.scan_location.as_ref().map(PathBuf::from);

    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(log) = log_rx.recv().await {
            let _ = app_clone.emit("log", log);
        }
    });

    let results = Arc::new(RwLock::new(Vec::new()));
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut handles = Vec::new();
    let max_concurrent = 4;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    for pack in packs {
        let mover_clone = Arc::clone(&mover);
        let scan_dir_clone = scan_dir.clone();
        let results_clone = Arc::clone(&results);
        let counter_clone = Arc::clone(&counter);
        let app_clone = app.clone();
        let semaphore_clone = Arc::clone(&semaphore);

        let handle = tokio::spawn(async move {
            let _permit = semaphore_clone.acquire().await.unwrap();

            let current = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

            if app_clone
                .state::<AppState>()
                .cancel_requested
                .load(Ordering::SeqCst)
            {
                results_clone.write().push(MoveOperation {
                    source: pack.path.clone(),
                    destination: String::new(),
                    pack_name: pack.name.clone(),
                    pack_type: pack.pack_type,
                    success: false,
                    error: Some("Cancelled by user".to_string()),
                    is_template_update: None,
                    skin_pack_4d_path: None,
                    deleted_old_path: None,
                });
                return;
            }

            let _ = app_clone.emit(
                "progress",
                serde_json::json!({
                    "current": current,
                    "total": total,
                    "message": format!("Processing {}", pack.name)
                }),
            );

            let result = mover_clone
                .process_pack(&pack, scan_dir_clone.as_ref())
                .await;

            results_clone.write().push(result);
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    let mut final_results = Arc::try_unwrap(results).unwrap().into_inner();

    if delete_source {
        let source_results: std::collections::HashMap<&str, bool> = final_results.iter().fold(
            std::collections::HashMap::new(),
            |mut results, operation| {
                results
                    .entry(&operation.source)
                    .and_modify(|all_succeeded| *all_succeeded &= operation.success)
                    .or_insert(operation.success);
                results
            },
        );
        for (source, all_succeeded) in source_results {
            if !all_succeeded {
                emit_log(
                    &app,
                    "WARN",
                    &format!("Kept source file after a partial failure: {}", source),
                );
                continue;
            }
            match std::fs::remove_file(source) {
                Ok(()) => emit_log(&app, "INFO", &format!("Deleted source file: {}", source)),
                Err(error) => emit_log(
                    &app,
                    "WARN",
                    &format!("Failed to delete source file '{}': {}", source, error),
                ),
            }
        }
    }

    let _ = app.emit(
        "progress",
        serde_json::json!({
            "current": total,
            "total": total,
            "message": "Complete"
        }),
    );

    final_results.sort_by(|a, b| a.pack_name.cmp(&b.pack_name));
    Ok(final_results)
}

#[tauri::command]
async fn rollback_last(app: AppHandle) -> Result<Option<MoveOperation>, String> {
    emit_log(&app, "INFO", "Attempting to rollback last operation");

    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();

    let (log_tx, mut log_rx) = mpsc::unbounded_channel();

    let history = Arc::clone(&state.move_history);
    let mut mover = FileMover::with_history(settings, history);
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
    validate_settings(&settings)?;
    let mut settings = settings;
    if !settings.remember_scan_location.unwrap_or(true) {
        settings.scan_location = None;
    }
    let state = app.state::<AppState>();
    *state.settings.write() = settings.clone();
    save_settings_to_file(&settings)
}

fn validate_pack_destination(
    path: &Option<String>,
    expected_leaf: &str,
    setting_name: &str,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    let canonical = std::path::Path::new(path)
        .canonicalize()
        .map_err(|_| format!("{} must be an existing directory", setting_name))?;
    if !canonical.is_dir()
        || !canonical
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(expected_leaf))
    {
        return Err(format!(
            "{} must be the Minecraft '{}' directory",
            setting_name, expected_leaf
        ));
    }
    let minecraft_users = dirs::config_dir()
        .ok_or("Could not determine the Minecraft data directory")?
        .join("Minecraft Bedrock")
        .join("Users");
    let canonical_users = minecraft_users
        .canonicalize()
        .map_err(|_| "Minecraft Bedrock users directory was not found")?;
    let minecraft_root = canonical
        .parent()
        .ok_or("Invalid Minecraft pack directory structure")?;
    if !canonical.starts_with(&canonical_users)
        || !minecraft_root
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("com.mojang"))
        || !minecraft_root.parent().is_some_and(|parent| {
            parent
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("games"))
        })
    {
        return Err(format!(
            "{} must be inside Minecraft Bedrock's com.mojang directory",
            setting_name
        ));
    }
    Ok(())
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    validate_pack_destination(
        &settings.behavior_pack_path,
        "behavior_packs",
        "Behavior pack path",
    )?;
    validate_pack_destination(
        &settings.resource_pack_path,
        "resource_packs",
        "Resource pack path",
    )?;
    validate_pack_destination(&settings.skin_pack_path, "skin_packs", "Skin pack path")?;
    validate_pack_destination(
        &settings.world_template_path,
        "world_templates",
        "World template path",
    )?;
    Ok(())
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
    let config_dir =
        dirs::config_dir().ok_or_else(|| "Could not determine config directory".to_string())?;

    let app_config_dir = config_dir.join("blocksmith");
    if std::fs::create_dir_all(&app_config_dir).is_err() {
        return Err("Failed to create config directory".to_string());
    }

    let settings_path = app_config_dir.join("settings.json");
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;

    std::fs::write(&settings_path, content).map_err(|e| e.to_string())?;

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
                    if validate_settings(&settings).is_ok() {
                        return settings;
                    }
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

        // Collect all com.mojang candidate paths: Shared + all numeric GUID subfolders.
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&mc_base) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let mojang = p.join("games").join("com.mojang");
                    if mojang.exists() {
                        candidates.push(mojang);
                    }
                }
            }
        }

        // Helper: count immediate subdirectories in a folder.
        let subdir_count = |dir: &std::path::Path| -> usize {
            std::fs::read_dir(dir)
                .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).count())
                .unwrap_or(0)
        };

        // For each pack-type subfolder, pick the candidate that has the MOST entries.
        // This ensures we land on the folder where the user's packs actually live,
        // rather than an empty mirror folder in another location.
        let pick_best = |subfolder: &str| -> Option<String> {
            candidates
                .iter()
                .map(|c| c.join(subfolder))
                .filter(|p| p.exists())
                .max_by_key(|p| subdir_count(p))
                .map(|p| p.to_string_lossy().into_owned())
        };

        settings.behavior_pack_path = pick_best("behavior_packs");
        settings.resource_pack_path = pick_best("resource_packs");
        settings.skin_pack_path = pick_best("skin_packs");
        settings.world_template_path = pick_best("world_templates");
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
            std::path::PathBuf::from(s)
                .join("4D Skin Packs")
                .to_string_lossy()
                .into_owned()
        }),
        PackType::WorldTemplate | PackType::MashupPack => settings.world_template_path.clone(),
        PackType::Unknown => None,
    }
}

#[tauri::command]
fn open_folder(path: String, app: AppHandle) -> Result<(), String> {
    let target = std::path::Path::new(&path);
    let target = if target.is_file() {
        target
            .parent()
            .ok_or("Could not determine parent directory")?
    } else {
        target
    };
    if !is_within_configured_dirs(target, &app) && !is_managed_4d_skin_directory(target, &app) {
        return Err("Path is outside configured pack directories".to_string());
    }

    // Use the shell's registered "open" handler for folders instead of hardcoding
    // explorer.exe, so that a user-configured default file manager (e.g. Directory
    // Opus) is respected. rundll32/url.dll's FileProtocolHandler invokes ShellExecute
    // under the hood; the path is passed as a plain argv entry (no cmd.exe/shell
    // string re-parsing), so shell metacharacters in the path are inert.
    #[cfg(target_os = "windows")]
    std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", &target.display().to_string()])
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;
    Ok(())
}

fn is_managed_4d_skin_directory(path: &std::path::Path, app: &AppHandle) -> bool {
    let Ok(canonical_path) = path.canonicalize() else {
        return false;
    };
    let state = app.state::<AppState>();
    let settings = state.settings.read();
    let Some(scan_location) = settings.scan_location.as_ref() else {
        return false;
    };
    let Ok(canonical_scan_location) = std::path::Path::new(scan_location).canonicalize() else {
        return false;
    };
    canonical_path == canonical_scan_location.join("4D Skin Packs")
}

#[tauri::command]
fn open_help_page(page: String) -> Result<(), String> {
    let url = match page.as_str() {
        "repository" => "https://github.com/MrLabRat/Blocksmith-BR",
        "issues" => "https://github.com/MrLabRat/Blocksmith-BR/issues",
        "discussions" => "https://github.com/MrLabRat/Blocksmith-BR/discussions",
        _ => return Err("Unknown help page".to_string()),
    };
    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer")
        .arg(url)
        .spawn()
        .map_err(|e| format!("Failed to open help page: {}", e))?;
    Ok(())
}

#[tauri::command]
fn write_export_file(path: String, content: String, extension: String) -> Result<(), String> {
    const MAX_EXPORT_BYTES: usize = 16 * 1024 * 1024;
    if !matches!(extension.as_str(), "csv" | "json") || content.len() > MAX_EXPORT_BYTES {
        return Err("Invalid export request".to_string());
    }
    let export_path = std::path::Path::new(&path);
    if export_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(&extension))
        != Some(true)
    {
        return Err("Export file extension does not match the requested format".to_string());
    }
    let parent = export_path
        .parent()
        .ok_or("Export path has no parent directory")?;
    if !parent.is_dir() {
        return Err("Export directory does not exist".to_string());
    }
    let metadata = std::fs::symlink_metadata(export_path).ok();
    if metadata.is_some_and(|metadata| metadata.file_type().is_symlink() || metadata.is_dir()) {
        return Err("Refusing to overwrite a symlink or directory".to_string());
    }
    std::fs::write(export_path, content).map_err(|e| format!("Failed to write export: {}", e))
}

#[tauri::command]
fn auto_detect_paths(app: AppHandle) -> Settings {
    let state = app.state::<AppState>();
    let mut current = state.settings.read().clone();
    let detected = auto_detect_mc_paths();
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
                    let folder_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    let display_name =
                        get_pack_display_name(&path).unwrap_or_else(|| folder_name.clone());

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
    let temp_dir = std::env::temp_dir().join(format!("Blocksmith-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&temp_dir)
        .map_err(|e| format!("Failed to create temporary launch directory: {}", e))?;

    let skinmaster_path = temp_dir.join("SkinMaster.exe");
    let mut skinmaster_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&skinmaster_path)
        .map_err(|e| format!("Failed to create SkinMaster executable: {}", e))?;
    use std::io::Write;
    skinmaster_file
        .write_all(SKINMASTER_EXE)
        .map_err(|e| format!("Failed to extract SkinMaster.exe: {}", e))?;
    skinmaster_file
        .sync_all()
        .map_err(|e| format!("Failed to finalize SkinMaster.exe: {}", e))?;
    drop(skinmaster_file);

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
    emit_log(
        &app,
        "INFO",
        &format!(
            "Importing 4D skin from '{}' to '{}'",
            skin_pack_path, premium_pack_path
        ),
    );

    let skin_path = std::path::Path::new(&skin_pack_path);
    let premium_path = std::path::Path::new(&premium_pack_path);

    let allowed_base = if let Some(roaming) = dirs::config_dir() {
        roaming
            .join("Minecraft Bedrock")
            .join("premium_cache")
            .join("skin_packs")
    } else {
        return Err("Could not determine AppData directory".to_string());
    };
    if !premium_path.starts_with(&allowed_base) {
        return Err(
            "premium_pack_path is outside the premium cache skin_packs directory".to_string(),
        );
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
            emit_log(
                &app,
                "INFO",
                "Skipping manifest.json (keeping premium pack's manifest)",
            );
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

    emit_log(
        &app,
        "SUCCESS",
        "4D skin pack imported successfully! Restart Minecraft to see the changes.",
    );

    Ok(())
}

#[tauri::command]
fn find_duplicate_packs(app: AppHandle) -> Vec<DuplicateGroup> {
    let installed_packs = installed_packs_in_configured_destinations(&app);

    let to_duplicate_pack = |p: &InstalledPackInfo| DuplicatePack {
        uuid: p.uuid.clone().unwrap_or_default(),
        name: p.name.clone(),
        pack_type: format!("{:?}", p.pack_type),
        path: p.path.clone(),
        folder_name: p.folder_name.clone(),
    };

    let mut uuid_groups: std::collections::HashMap<String, Vec<InstalledPackInfo>> =
        std::collections::HashMap::new();
    // Fallback grouping for packs whose manifest has no UUID: correlate by pack type
    // plus a version-aware base name (same normalization used for update detection)
    // so folders like "Actions Stuff 1.11" and "Actions Stuff 1.12" still surface as
    // the same pack installed twice.
    let mut name_groups: std::collections::HashMap<(PackType, String), Vec<InstalledPackInfo>> =
        std::collections::HashMap::new();

    for pack in installed_packs {
        if let Some(ref uuid) = pack.uuid {
            uuid_groups.entry(uuid.clone()).or_default().push(pack);
        } else {
            let base = extract_base_name(&pack.name);
            name_groups
                .entry((pack.pack_type, base))
                .or_default()
                .push(pack);
        }
    }

    let mut duplicates: Vec<DuplicateGroup> = uuid_groups
        .into_iter()
        .filter(|(_, packs)| packs.len() > 1)
        .map(|(uuid, packs)| {
            let name = packs[0].name.clone();
            DuplicateGroup {
                uuid,
                name,
                packs: packs.iter().map(to_duplicate_pack).collect(),
            }
        })
        .collect();

    duplicates.extend(
        name_groups
            .into_iter()
            .filter(|(_, packs)| packs.len() > 1)
            .map(|((_, base), packs)| {
                let name = packs[0].name.clone();
                DuplicateGroup {
                    uuid: base,
                    name,
                    packs: packs.iter().map(to_duplicate_pack).collect(),
                }
            }),
    );

    duplicates.sort_by(|a, b| a.name.cmp(&b.name));
    duplicates
}

#[tauri::command]
fn watch_scan_folder(path: String, app: AppHandle) -> Result<(), String> {
    let scan_path = std::path::PathBuf::from(&path);

    if !scan_path.exists() {
        return Err(format!("Scan folder does not exist: {}", path));
    }

    let state = app.state::<AppState>();
    if state.scan_watching.load(Ordering::SeqCst) {
        if let Some(tx) = state.scan_watch_stop_tx.lock().take() {
            let _ = tx.send(());
        }
    }
    state.scan_watching.store(true, Ordering::SeqCst);

    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    *state.scan_watch_stop_tx.lock() = Some(stop_tx);

    let app_clone = app.clone();

    std::thread::spawn(move || {
        let list_mc_files = |dir: &std::path::Path| -> std::collections::HashSet<String> {
            std::fs::read_dir(dir)
                .ok()
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if !p.is_file() {
                        return None;
                    }
                    let ext = p
                        .extension()
                        .and_then(|x| x.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if matches!(ext.as_str(), "mcpack" | "mcaddon" | "mctemplate") {
                        Some(p.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                })
                .collect()
        };

        let app_for_events = app_clone.clone();
        let mut watcher: notify::RecommendedWatcher = match Watcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in &event.paths {
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if ext == "mcpack" || ext == "mcaddon" || ext == "mctemplate" {
                            let _ = app_for_events
                                .emit("scan-folder-changed", path.to_string_lossy().to_string());
                            break;
                        }
                    }
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create scan folder watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(&scan_path, RecursiveMode::NonRecursive) {
            eprintln!("Failed to watch scan folder: {}", e);
            return;
        }

        let mut known_files = list_mc_files(&scan_path);
        loop {
            match stop_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    let current_files = list_mc_files(&scan_path);
                    if current_files.iter().any(|f| !known_files.contains(f)) {
                        let _ = app_clone.emit("scan-folder-changed", "polled".to_string());
                    }
                    known_files = current_files;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn stop_watching_scan_folder(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.scan_watching.store(false, Ordering::SeqCst);
    if let Some(tx) = state.scan_watch_stop_tx.lock().take() {
        let _ = tx.send(());
    }
    Ok(())
}

#[tauri::command]
fn list_mc_files_in_dir(directory: String) -> Result<Vec<String>, String> {
    let dir = std::path::Path::new(&directory);
    if !dir.exists() {
        return Err(format!("Directory does not exist: {}", directory));
    }
    let files: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let ext = path.extension()?.to_str()?.to_lowercase();
            if matches!(ext.as_str(), "mcpack" | "mcaddon" | "mctemplate") {
                Some(path.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    Ok(files)
}

#[tauri::command]
fn list_archive_files(
    path: String,
    subfolder: Option<String>,
    nested_mcpack: Option<String>,
) -> Result<Vec<String>, String> {
    use std::io::Read;

    fn entry_name_and_symlink<R: Read + std::io::Seek>(
        archive: &mut ZipArchive<R>,
        index: usize,
    ) -> Result<(String, bool), String> {
        let zip_file = archive.by_index(index).map_err(|e| e.to_string())?;
        let name = zip_file.name().to_string();
        let is_symlink = zip_file
            .unix_mode()
            .map(|m| (m & 0o170000) == 0o120000)
            .unwrap_or(false);
        Ok((name, is_symlink))
    }

    let file = std::fs::File::open(&path).map_err(|e| format!("Failed to open archive: {}", e))?;
    let mut outer_archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    let mut nested_archive = match &nested_mcpack {
        Some(entry_name) => {
            let mut bytes = Vec::new();
            outer_archive
                .by_name(entry_name)
                .map_err(|e| format!("Failed to read nested pack '{}': {}", entry_name, e))?
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Failed to read nested pack '{}': {}", entry_name, e))?;
            Some(
                ZipArchive::new(std::io::Cursor::new(bytes))
                    .map_err(|e| format!("Failed to read nested pack archive: {}", e))?,
            )
        }
        None => None,
    };

    let archive_len = match &nested_archive {
        Some(archive) => archive.len(),
        None => outer_archive.len(),
    };

    let mut files: Vec<String> = Vec::new();

    for i in 0..archive_len {
        let (name, is_symlink) = match &mut nested_archive {
            Some(archive) => entry_name_and_symlink(archive, i)?,
            None => entry_name_and_symlink(&mut outer_archive, i)?,
        };

        if is_symlink {
            continue;
        }

        let relative = if nested_archive.is_some() {
            name
        } else if let Some(ref sf) = subfolder {
            let prefix = format!("{}/", sf);
            if name.starts_with(&prefix) {
                name[prefix.len()..].to_string()
            } else {
                continue;
            }
        } else {
            name
        };

        let relative = relative.trim_start_matches('/').to_string();
        if relative.is_empty() {
            continue;
        }

        if std::path::Path::new(&relative)
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            continue;
        }

        files.push(relative);
    }

    files.sort();
    Ok(files)
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

    app.state::<AppState>()
        .watching
        .store(true, Ordering::SeqCst);

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
                    }
                    .to_string();

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

        emit_log(
            &app,
            "INFO",
            &format!("Watching: {}", premium_cache.display()),
        );

        let _ = stop_rx.recv();
    });

    Ok(())
}

#[tauri::command]
fn stop_watching(app: AppHandle) -> Result<(), String> {
    app.state::<AppState>()
        .watching
        .store(false, Ordering::SeqCst);
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

/// Returns true if a folder name (any casing) contains a mash-up keyword.
fn folder_name_is_mashup(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("mashup") || lower.contains("mash-up") || lower.contains("mash up")
}

/// Strip all trailing parenthesised tags from a folder name to get the base title.
///
/// Real-world examples this handles:
///   "Adventure Time Mash-up (world_template)"  -> "adventure time mash-up"
///   "Dragons Mash-Up (world_template) (TEMPLATE)" -> "dragons mash-up"
///   "Halloween Mash-up (resources) (RESOURCE)"    -> "halloween mash-up"
///   "1,000,000 Horses! (addon) (BP)"              -> "1,000,000 horses!"
///
/// Strategy: repeatedly strip the last " (…)" group until none remain,
/// then lowercase and trim.
fn pack_base_name(name: &str) -> String {
    let mut s = name.trim().to_string();
    loop {
        if let Some(open) = s.rfind(" (") {
            // Make sure there is a closing ')' after the '('
            if s[open..].contains(')') {
                s = s[..open].trim().to_string();
                continue;
            }
        }
        break;
    }
    s.to_lowercase()
}

/// Collect a map of base_name -> count-of-entries for every immediate subdir
/// in the given path option.  Files and non-dirs are skipped.
fn subdir_base_names(path_opt: &Option<String>) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    if let Some(p) = path_opt {
        if let Ok(entries) = std::fs::read_dir(p) {
            for entry in entries.flatten() {
                let ep = entry.path();
                if ep.is_dir() {
                    if let Some(n) = ep.file_name().and_then(|n| n.to_str()) {
                        names.insert(pack_base_name(n));
                    }
                }
            }
        }
    }
    names
}

/// Returns ALL candidate `com.mojang/<subfolder>` paths on this machine —
/// both the `Shared` folder and every GUID user folder — regardless of which
/// one is configured as the primary destination.  This ensures the installed-
/// packs views and mashup correlation see packs in every location (e.g. STAR
/// WARS world template lives in `Shared/world_templates` while the primary WT
/// destination is the GUID folder that has more entries).
fn all_mc_subfolder_paths(subfolder: &str) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(roaming) = dirs::config_dir() {
        let mc_users = roaming.join("Minecraft Bedrock").join("Users");
        if let Ok(entries) = std::fs::read_dir(&mc_users) {
            for entry in entries.flatten() {
                let candidate = entry
                    .path()
                    .join("games")
                    .join("com.mojang")
                    .join(subfolder);
                if candidate.exists() && candidate.is_dir() {
                    paths.push(candidate.to_string_lossy().into_owned());
                }
            }
        }
    }
    paths
}

/// Given the configured paths, build the set of base names that are present in
/// world_templates AND in resource_packs.
/// Scans ALL candidate MC paths (Shared + every GUID folder) so packs spread
/// across locations are correlated correctly.
/// A skin-pack alone sharing a base name with a world template is not a reliable
/// mashup signal (e.g. "BIG ONE BLOCK" has both a WT and a SP but is not a mashup).
/// Requiring WT + RP correctly identifies real mashups (Dragons, Biome Survival, etc.)
/// while excluding world templates that merely happen to share a name with a skin pack.
fn build_correlated_mashup_bases(
    _rp_path: &Option<String>,
    _sp_path: &Option<String>,
    _wt_path: &Option<String>,
) -> std::collections::HashSet<String> {
    let mut rp_bases = std::collections::HashSet::new();
    for p in all_mc_subfolder_paths("resource_packs") {
        for base in subdir_base_names(&Some(p)) {
            rp_bases.insert(base);
        }
    }
    let mut wt_bases = std::collections::HashSet::new();
    for p in all_mc_subfolder_paths("world_templates") {
        for base in subdir_base_names(&Some(p)) {
            wt_bases.insert(base);
        }
    }
    wt_bases
        .into_iter()
        .filter(|b| rp_bases.contains(b))
        .collect()
}

/// The single source of truth for "is this folder a mash-up pack?".
/// Checks keyword first, then cross-folder correlation.
fn is_mashup(folder_name: &str, correlated: &std::collections::HashSet<String>) -> bool {
    folder_name_is_mashup(folder_name) || correlated.contains(&pack_base_name(folder_name))
}

#[tauri::command]
async fn get_installed_packs_stats(_app: AppHandle) -> Result<Vec<PackStats>, String> {
    // Build correlation set scanning ALL candidate MC paths.
    let correlated = build_correlated_mashup_bases(&None, &None, &None);

    // Enumerate ALL candidate locations for each pack type.
    let mut folders: Vec<(&'static str, String)> = Vec::new();
    for p in all_mc_subfolder_paths("behavior_packs") {
        folders.push(("BehaviorPack", p));
    }
    for p in all_mc_subfolder_paths("resource_packs") {
        folders.push(("ResourcePack", p));
    }
    for p in all_mc_subfolder_paths("skin_packs") {
        folders.push(("SkinPack", p));
    }
    for p in all_mc_subfolder_paths("world_templates") {
        folders.push(("WorldTemplate", p));
    }

    let stats: Vec<PackStats> =
        tokio::task::spawn_blocking(move || -> Result<Vec<PackStats>, String> {
            use rayon::prelude::*;

            // Pass 1 (cheap, sequential): enumerate every candidate pack directory and
            // dedupe by canonical path across Shared/GUID locations. This must stay
            // sequential since the HashSet dedupe decision has to happen before sizing.
            let mut seen = std::collections::HashSet::new();
            let mut entries: Vec<(&'static str, bool, std::path::PathBuf)> = Vec::new();

            for (pack_type, path_str) in &folders {
                let path = std::path::Path::new(path_str);
                if !path.exists() {
                    continue;
                }

                let dirs: Vec<std::path::PathBuf> = std::fs::read_dir(path)
                    .ok()
                    .into_iter()
                    .flat_map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()))
                    .collect();

                for dir in dirs {
                    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
                    if !seen.insert(canonical) {
                        continue;
                    }

                    let raw_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let is_mu = *pack_type == "WorldTemplate" && is_mashup(raw_name, &correlated);
                    entries.push((*pack_type, is_mu, dir));
                }
            }

            // Pass 2 (expensive, parallel): walking each pack's directory tree to sum
            // its size is I/O-bound and independent per entry, so fan it out via a
            // bounded background-priority pool instead of sizing every installed pack
            // one at a time (or saturating every core via the global rayon pool).
            let pool = build_background_pool()?;
            let sized: Vec<(&'static str, bool, u64)> = pool.install(|| {
                entries
                    .into_par_iter()
                    .map(|(pack_type, is_mu, dir)| (pack_type, is_mu, calculate_folder_size(&dir)))
                    .collect()
            });

            let mut bp_count = 0usize;
            let mut bp_size = 0u64;
            let mut rp_count = 0usize;
            let mut rp_size = 0u64;
            let mut sp_count = 0usize;
            let mut sp_size = 0u64;
            let mut wt_count = 0usize;
            let mut wt_size = 0u64;
            let mut mu_count = 0usize;
            let mut mu_size = 0u64;

            for (pack_type, is_mu, size) in sized {
                // Only world-template folders are promoted to MashupPack.
                // RP/SP/BP entries that share a name keep their own type for
                // accurate per-category counts.
                if is_mu {
                    mu_count += 1;
                    mu_size += size;
                } else {
                    match pack_type {
                        "BehaviorPack" => {
                            bp_count += 1;
                            bp_size += size;
                        }
                        "ResourcePack" => {
                            rp_count += 1;
                            rp_size += size;
                        }
                        "SkinPack" => {
                            sp_count += 1;
                            sp_size += size;
                        }
                        "WorldTemplate" => {
                            wt_count += 1;
                            wt_size += size;
                        }
                        _ => {}
                    }
                }
            }

            let mut results: Vec<PackStats> = Vec::new();
            if bp_count > 0 {
                results.push(PackStats {
                    pack_type: "BehaviorPack".to_string(),
                    count: bp_count,
                    total_size: bp_size,
                    total_size_formatted: format_bytes(bp_size),
                });
            }
            if rp_count > 0 {
                results.push(PackStats {
                    pack_type: "ResourcePack".to_string(),
                    count: rp_count,
                    total_size: rp_size,
                    total_size_formatted: format_bytes(rp_size),
                });
            }
            if sp_count > 0 {
                results.push(PackStats {
                    pack_type: "SkinPack".to_string(),
                    count: sp_count,
                    total_size: sp_size,
                    total_size_formatted: format_bytes(sp_size),
                });
            }
            if wt_count > 0 {
                results.push(PackStats {
                    pack_type: "WorldTemplate".to_string(),
                    count: wt_count,
                    total_size: wt_size,
                    total_size_formatted: format_bytes(wt_size),
                });
            }
            if mu_count > 0 {
                results.push(PackStats {
                    pack_type: "MashupPack".to_string(),
                    count: mu_count,
                    total_size: mu_size,
                    total_size_formatted: format_bytes(mu_size),
                });
            }
            Ok(results)
        })
        .await
        .map_err(|e| e.to_string())??;

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
                    if is_direct_managed_pack_directory(&entry_path, &app) {
                        move_to_recycle_bin(&entry_path)
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

/// Puts the CURRENT thread into Windows "background processing mode"
/// (`THREAD_MODE_BACKGROUND_BEGIN`), which lowers its scheduling, I/O, and
/// memory priority all at once for as long as the thread lives. This is the
/// API Windows itself recommends for bulk background work (bulk disk I/O +
/// CPU-heavy processing) that must not compete with foreground/real-time
/// work such as audio playback. A plain `SetThreadPriority` with a low
/// priority class only affects CPU scheduling and was not enough on its own
/// to stop audible glitches while this scan's rayon workers decoded/resized
/// pack icons in parallel.
#[cfg(target_os = "windows")]
fn enter_background_processing_mode() {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn SetThreadPriority(h_thread: isize, priority: i32) -> i32;
    }
    const THREAD_MODE_BACKGROUND_BEGIN: i32 = 0x0001_0000;
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}

#[cfg(not(target_os = "windows"))]
fn enter_background_processing_mode() {}

/// Builds a bounded rayon thread pool (one fewer than the number of logical
/// cores, minimum 1) whose worker threads enter Windows background-processing
/// mode before running any task. Use this instead of rayon's bare/global
/// `.par_iter()`/`.into_par_iter()` for ANY bulk CPU/IO-heavy parallel work
/// (archive scanning, icon decode/resize, folder-size walking, etc.) — the
/// global pool uses every core at normal priority, which can starve the
/// system badly enough that this app's `transparent: true` window fails to
/// composite a frame and shows straight through to the desktop (see the
/// `scan_packs`/`get_directory_folders` fixes for the original bug reports).
fn build_background_pool() -> Result<rayon::ThreadPool, String> {
    let worker_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .saturating_sub(1)
        .max(1);

    rayon::ThreadPoolBuilder::new()
        .num_threads(worker_threads)
        .spawn_handler(|thread| {
            let mut builder = std::thread::Builder::new();
            if let Some(name) = thread.name() {
                builder = builder.name(name.to_string());
            }
            if let Some(stack_size) = thread.stack_size() {
                builder = builder.stack_size(stack_size);
            }
            builder.spawn(move || {
                enter_background_processing_mode();
                thread.run();
            })?;
            Ok(())
        })
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_directory_folders(app: AppHandle) -> Result<Vec<PackInfo>, String> {
    // Build correlation set scanning ALL candidate MC paths.
    let correlated = build_correlated_mashup_bases(&None, &None, &None);

    // Enumerate ALL candidate locations for each pack type.
    let pack_subfolders: &[(&str, &str)] = &[
        ("BehaviorPack", "behavior_packs"),
        ("ResourcePack", "resource_packs"),
        ("SkinPack", "skin_packs"),
        ("WorldTemplate", "world_templates"),
    ];

    let mut folder_paths: Vec<(String, String, String)> = Vec::new();
    let mut seen_canonical = std::collections::HashSet::new();

    for (type_str, subfolder) in pack_subfolders {
        for path_str in all_mc_subfolder_paths(subfolder) {
            let path = std::path::Path::new(&path_str);
            if path.exists() && path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            // Deduplicate via canonical path.
                            let canonical = entry_path
                                .canonicalize()
                                .unwrap_or_else(|_| entry_path.clone());
                            if !seen_canonical.insert(canonical) {
                                continue;
                            }

                            let folder_name = entry_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .to_string();
                            folder_paths.push((
                                entry_path.to_string_lossy().to_string(),
                                folder_name,
                                type_str.to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    let total_folders = folder_paths.len();
    let progress_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // A single dedicated ticker is the ONLY thing that emits progress events.
    // Previously every rayon worker thread emitted its own event as soon as it
    // finished an item; since those emits happened concurrently from multiple
    // native threads with no ordering guarantee, the frontend could receive a
    // lower `current` value after a higher one, making the progress bar
    // visibly jump backwards ("reset"). Polling a shared atomic counter from
    // one thread guarantees strictly increasing, in-order updates.
    let done_flag = Arc::new(AtomicBool::new(false));
    let ticker_app = app.clone();
    let ticker_counter = Arc::clone(&progress_counter);
    let ticker_done = Arc::clone(&done_flag);
    let ticker = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            let current = ticker_counter.load(std::sync::atomic::Ordering::SeqCst);
            let _ = ticker_app.emit(
                "packs_scan_progress",
                serde_json::json!({ "current": current, "total": total_folders }),
            );
            if ticker_done.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
        }
    });

    let counter_for_pool = Arc::clone(&progress_counter);
    let all_folders: Result<Vec<PackInfo>, String> = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;

        // Custom spawn handler so every worker thread in this pool enters
        // Windows background-processing mode before doing any work — this is
        // what actually stops the parallel icon decode/resize work from
        // starving the system's real-time audio thread (see
        // `enter_background_processing_mode` doc comment for details).
        let pool = build_background_pool()?;

        let mut final_results: Vec<PackInfo> = pool.install(|| {
            folder_paths
                .into_par_iter()
                .map(|(path, folder_name, pack_type_str)| {
                    let entry_path = std::path::Path::new(&path);
                    let (uuid, display_name, version) = read_pack_metadata_fast(entry_path);
                    let icon = read_pack_icon(entry_path);
                    // Only world template folders can be promoted to MashupPack.
                    // RP/SP/BP entries that share a name with a mashup keep their own type
                    // so the frontend can correctly group and display them as children.
                    let pack_type = if pack_type_str == "WorldTemplate"
                        && is_mashup(&folder_name, &correlated)
                    {
                        PackType::MashupPack
                    } else {
                        parse_pack_type(&pack_type_str)
                    };

                    counter_for_pool.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                    PackInfo {
                        path: path.clone(),
                        name: display_name.unwrap_or_else(|| folder_name.clone()),
                        pack_type,
                        uuid,
                        version,
                        extracted: false,
                        icon_base64: icon,
                        subfolder: None,
                        nested_mcpack: None,
                        folder_size: None,
                        folder_size_formatted: None,
                        needs_attention: None,
                        attention_message: None,
                        is_installed: None,
                        is_update: None,
                        installed_version: None,
                    }
                })
                .collect()
        });

        final_results.sort_by_key(|pack| pack.name.to_lowercase());
        Ok(final_results)
    })
    .await
    .map_err(|e| e.to_string())?;

    done_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = ticker.await;
    let _ = app.emit(
        "packs_scan_progress",
        serde_json::json!({ "current": total_folders, "total": total_folders }),
    );

    all_folders
}

fn read_pack_icon(folder_path: &std::path::Path) -> Option<String> {
    let icon_names = [
        "pack_icon.png",
        "Pack_Icon.png",
        "world_icon.jpeg",
        "world_icon.jpg",
        "icon.png",
    ];
    // 64 MB hard cap — anything larger is almost certainly corrupt/wrong
    const MAX_ICON_SIZE: u64 = 64 * 1024 * 1024;
    const MAX_DIMENSION: u32 = 256;

    for icon_name in &icon_names {
        let icon_path = folder_path.join(icon_name);
        if icon_path.exists() {
            let file_size = icon_path.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
            if file_size > MAX_ICON_SIZE {
                continue;
            }
            if let Ok(icon_data) = std::fs::read(&icon_path) {
                // If the image fits within our dimension limit, encode it directly
                // without a full decode/re-encode cycle (fast path).
                // For oversized files we decode, resize, and re-encode as PNG.
                let is_jpeg = icon_name.ends_with(".jpg") || icon_name.ends_with(".jpeg");

                // Attempt a fast path: decode just the dimensions.
                let needs_resize = if let Ok(reader) =
                    image::ImageReader::new(std::io::Cursor::new(&icon_data)).with_guessed_format()
                {
                    if let Ok((w, h)) = reader.into_dimensions() {
                        w > MAX_DIMENSION || h > MAX_DIMENSION
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !needs_resize {
                    let mime = if is_jpeg { "image/jpeg" } else { "image/png" };
                    let b64 = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &icon_data,
                    );
                    return Some(format!("data:{};base64,{}", mime, b64));
                }

                // Slow path: decode → resize → re-encode as PNG
                if let Ok(img) = image::load_from_memory(&icon_data) {
                    let resized = img.resize(
                        MAX_DIMENSION,
                        MAX_DIMENSION,
                        image::imageops::FilterType::Lanczos3,
                    );
                    let mut buf = Vec::new();
                    if resized
                        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                        .is_ok()
                    {
                        let b64 = base64::Engine::encode(
                            &base64::engine::general_purpose::STANDARD,
                            &buf,
                        );
                        return Some(format!("data:image/png;base64,{}", b64));
                    }
                }
            }
        }
    }

    None
}

/// Bedrock manifests often store `header.name` as a localization key (e.g. `"pack.name"`)
/// whose real display value lives in a `texts/*.lang` file next to the manifest. Resolve it
/// if possible; otherwise the raw manifest value is used unchanged by the caller.
fn resolve_localized_display_name(folder_path: &std::path::Path, raw_name: &str) -> Option<String> {
    let texts_dir = folder_path.join("texts");
    let preferred = texts_dir.join("en_US.lang");
    let candidate_path = if preferred.exists() {
        Some(preferred)
    } else {
        std::fs::read_dir(&texts_dir).ok().and_then(|entries| {
            entries.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("lang"))
            })
        })
    }?;

    let content = std::fs::read_to_string(&candidate_path).ok()?;
    let search_prefix = format!("{}=", raw_name);
    for line in content.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix(&search_prefix) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn read_pack_metadata_fast(
    folder_path: &std::path::Path,
) -> (Option<String>, Option<String>, Option<String>) {
    let manifest_path = folder_path.join("manifest.json");

    if manifest_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let uuid = json
                    .get("header")
                    .and_then(|h| h.get("uuid"))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string());

                let name = json
                    .get("header")
                    .and_then(|h| h.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
                    .map(|raw| resolve_localized_display_name(folder_path, &raw).unwrap_or(raw));

                let version = json
                    .get("header")
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
                        } else {
                            v.as_str().map(str::to_string)
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
        " (addon)",
        "(addon)",
        " (add-on)",
        "(add-on)",
        " (resource)",
        "(resource)",
        " (resources)",
        "(resources)",
        " (behavior)",
        "(behavior)",
        " (behaviour)",
        "(behaviour)",
        " (bp)",
        "(bp)",
        " (rp)",
        "(rp)",
        " (skin)",
        "(skin)",
        " (skins)",
        "(skins)",
        " (template)",
        "(template)",
        " (world_template)",
        "(world_template)",
        " (mashup)",
        "(mashup)",
        " (mash-up)",
        "(mash-up)",
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
        &EXTRACT_VERSION_1, // "V.1.0.1" or "v.1.0.1"
        &EXTRACT_VERSION_2, // "v1.0.1"
        &EXTRACT_VERSION_3, // " 1.8.1 ("
        &EXTRACT_VERSION_4, // " 1.8.1" at end
        &EXTRACT_VERSION_5, // " 1 ("
        &EXTRACT_VERSION_6, // " 1.1 " (version surrounded by spaces)
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
    let name = path.split(['/', '\\']).next_back().unwrap_or(path);

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
        " (ADDON)",
        "(ADDON)",
        " (addon)",
        "(addon)",
        " (RESOURCE)",
        "(RESOURCE)",
        " (resource)",
        "(resource)",
        " (SKIN)",
        "(SKIN)",
        " (skin)",
        "(skin)",
        " (TEMPLATE)",
        "(TEMPLATE)",
        " (template)",
        "(template)",
        " (MASHUP)",
        "(MASHUP)",
        " (mashup)",
        "(mashup)",
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

#[derive(Clone)]
struct InstalledPackInfo {
    uuid: Option<String>,
    name: String,
    pack_type: PackType,
    version: Option<String>,
    path: String,
    folder_name: String,
}

fn get_installed_packs_info(_app: &AppHandle) -> Vec<InstalledPackInfo> {
    // Build correlation set scanning ALL candidate MC paths.
    let correlated = build_correlated_mashup_bases(&None, &None, &None);

    let pack_subfolders: Vec<(&str, Vec<String>)> = [
        ("BehaviorPack", "behavior_packs"),
        ("ResourcePack", "resource_packs"),
        ("SkinPack", "skin_packs"),
        ("WorldTemplate", "world_templates"),
    ]
    .into_iter()
    .map(|(pack_type_str, subfolder)| (pack_type_str, all_mc_subfolder_paths(subfolder)))
    .collect();

    scan_installed_pack_folders(&pack_subfolders, &correlated)
}

/// Same as `get_installed_packs_info`, but scoped to only the single set of
/// destination folders configured in Settings, instead of every Minecraft user
/// profile on the machine. Used by the duplicate finder so that Mojang's own
/// vanilla/system packs (which legitimately exist once per profile) don't get
/// flagged as "duplicates" just because the machine has multiple Bedrock profiles.
fn installed_packs_in_configured_destinations(app: &AppHandle) -> Vec<InstalledPackInfo> {
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    let correlated = build_correlated_mashup_bases(&None, &None, &None);

    let pack_subfolders: Vec<(&str, Vec<String>)> = [
        ("BehaviorPack", settings.behavior_pack_path.clone()),
        ("ResourcePack", settings.resource_pack_path.clone()),
        ("SkinPack", settings.skin_pack_path.clone()),
        ("WorldTemplate", settings.world_template_path.clone()),
    ]
    .into_iter()
    .map(|(pack_type_str, path)| (pack_type_str, path.into_iter().collect::<Vec<_>>()))
    .collect();

    scan_installed_pack_folders(&pack_subfolders, &correlated)
}

fn scan_installed_pack_folders(
    pack_subfolders: &[(&str, Vec<String>)],
    correlated: &std::collections::HashSet<String>,
) -> Vec<InstalledPackInfo> {
    let mut installed_packs: Vec<InstalledPackInfo> = Vec::new();
    let mut seen_canonical = std::collections::HashSet::new();

    for (pack_type_str, dirs) in pack_subfolders {
        for path_str in dirs {
            let path = std::path::Path::new(path_str);
            if path.exists() && path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            let canonical = entry_path
                                .canonicalize()
                                .unwrap_or_else(|_| entry_path.clone());
                            if !seen_canonical.insert(canonical) {
                                continue;
                            }

                            let folder_name = entry_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .to_string();

                            let (uuid, display_name, version) =
                                read_pack_metadata_fast(&entry_path);

                            let pack_type = if *pack_type_str == "WorldTemplate"
                                && is_mashup(&folder_name, correlated)
                            {
                                PackType::MashupPack
                            } else {
                                parse_pack_type(pack_type_str)
                            };

                            installed_packs.push(InstalledPackInfo {
                                uuid,
                                name: display_name.unwrap_or_else(|| folder_name.clone()),
                                pack_type,
                                version,
                                path: entry_path.to_string_lossy().to_string(),
                                folder_name,
                            });
                        }
                    }
                }
            }
        }
    }

    installed_packs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSuggestion {
    pub path: String,
    pub current_name: String,
    pub suggested_name: String,
    pub pack_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackRenameRequest {
    pub path: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameResult {
    pub path: String,
    pub new_path: Option<String>,
    pub error: Option<String>,
}

/// Scans the configured destination folders and suggests a canonical name for any
/// installed pack folder whose name still carries decorations left behind by older
/// Blocksmith versions or manual installs (e.g. stray `(addon)`/`- ppackN` fragments,
/// `§` formatting codes) — used to power the "Clean Up Names" bulk renamer.
#[tauri::command]
fn suggest_pack_renames(app: AppHandle) -> Vec<RenameSuggestion> {
    installed_packs_in_configured_destinations(&app)
        .into_iter()
        .filter_map(|pack| {
            let suggested = modules::pack_detector::suggest_clean_folder_name(
                &pack.folder_name,
                pack.pack_type,
            );
            if suggested == pack.folder_name {
                return None;
            }
            Some(RenameSuggestion {
                path: pack.path,
                current_name: pack.folder_name,
                suggested_name: suggested,
                pack_type: format!("{:?}", pack.pack_type),
            })
        })
        .collect()
}

/// Renames a managed pack directory in place, validating it's a direct pack folder
/// under a configured destination (same check `delete_pack` uses) before touching disk.
/// Treats a same-path, case-only rename (e.g. `(addon)` -> `(ADDON)`) as valid even
/// though Windows' case-insensitive filesystem reports the target path as "existing".
fn rename_installed_pack_impl(
    path: &str,
    new_name: &str,
    app: &AppHandle,
) -> Result<String, String> {
    let folder_path = std::path::Path::new(path);
    if !is_direct_managed_pack_directory(folder_path, app) {
        return Err("Path is not a direct managed pack directory".to_string());
    }

    let sanitized = modules::pack_detector::sanitize_filename_component(new_name);
    if sanitized.is_empty() {
        return Err("New name is empty after sanitization".to_string());
    }

    let canonical_path = folder_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve pack path: {}", e))?;
    let parent = canonical_path
        .parent()
        .ok_or("Pack path has no parent directory")?
        .to_path_buf();
    let new_path = parent.join(&sanitized);

    if new_path == canonical_path {
        return Ok(canonical_path.to_string_lossy().to_string());
    }

    if new_path.exists() {
        let existing_canonical = new_path.canonicalize().unwrap_or_else(|_| new_path.clone());
        if existing_canonical != canonical_path {
            return Err(format!(
                "A pack folder named \"{}\" already exists in that destination",
                sanitized
            ));
        }
    }

    std::fs::rename(&canonical_path, &new_path)
        .map_err(|e| format!("Failed to rename pack folder: {}", e))?;

    Ok(new_path.to_string_lossy().to_string())
}

#[tauri::command]
fn rename_installed_pack(path: String, new_name: String, app: AppHandle) -> Result<String, String> {
    rename_installed_pack_impl(&path, &new_name, &app)
}

#[tauri::command]
fn rename_installed_packs(renames: Vec<PackRenameRequest>, app: AppHandle) -> Vec<RenameResult> {
    renames
        .into_iter()
        .map(
            |req| match rename_installed_pack_impl(&req.path, &req.new_name, &app) {
                Ok(new_path) => RenameResult {
                    path: req.path,
                    new_path: Some(new_path),
                    error: None,
                },
                Err(error) => RenameResult {
                    path: req.path,
                    new_path: None,
                    error: Some(error),
                },
            },
        )
        .collect()
}

#[tauri::command]
async fn get_all_folder_sizes(paths: Vec<String>) -> Result<Vec<(String, u64, String)>, String> {
    let results: Vec<(String, u64, String)> =
        tokio::task::spawn_blocking(move || -> Result<Vec<(String, u64, String)>, String> {
            use rayon::prelude::*;
            let pool = build_background_pool()?;
            Ok(pool.install(|| {
                paths
                    .into_par_iter()
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
            }))
        })
        .await
        .map_err(|e| e.to_string())??;

    Ok(results)
}

#[tauri::command]
fn get_folder_size(path: String) -> Result<(u64, String), String> {
    let folder_path = std::path::Path::new(&path);
    if !folder_path.exists() || !folder_path.is_dir() {
        return Err(format!(
            "Path does not exist or is not a directory: {}",
            path
        ));
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
    ]
    .into_iter()
    .flatten()
    .cloned()
    .collect();

    let Ok(canonical_path) = path.canonicalize() else {
        return false;
    };
    configured.iter().any(|dir| {
        let base = std::path::Path::new(dir);
        let Ok(canonical_base) = base.canonicalize() else {
            return false;
        };
        canonical_path.starts_with(&canonical_base)
    })
}

fn is_direct_managed_pack_directory(path: &std::path::Path, app: &AppHandle) -> bool {
    let Ok(canonical_path) = path.canonicalize() else {
        return false;
    };
    if !canonical_path.is_dir() || !canonical_path.join("manifest.json").is_file() {
        return false;
    }
    let Some(parent) = canonical_path.parent() else {
        return false;
    };
    let state = app.state::<AppState>();
    let settings = state.settings.read();
    let configured: Vec<&String> = [
        settings.behavior_pack_path.as_ref(),
        settings.resource_pack_path.as_ref(),
        settings.skin_pack_path.as_ref(),
        settings.skin_pack_4d_path.as_ref(),
        settings.world_template_path.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    configured.into_iter().any(|root| {
        std::path::Path::new(root)
            .canonicalize()
            .is_ok_and(|canonical_root| parent == canonical_root)
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecycledPackInfo {
    pub recycle_path: String,
    pub original_path: String,
    pub name: String,
    pub deleted_at: u64,
    pub size: u64,
    pub size_formatted: String,
}

fn recycle_bin_root() -> Result<PathBuf, String> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| "Could not determine config directory".to_string())?;
    let root = config_dir.join("blocksmith").join("recycle_bin");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

/// Moves a managed pack directory into the recycle bin instead of permanently deleting it,
/// recording the original location in a sidecar `.meta.json` so it can be restored later.
fn move_to_recycle_bin(path: &std::path::Path) -> Result<(), String> {
    let root = recycle_bin_root()?;
    let folder_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("pack");
    let sanitized = modules::pack_detector::sanitize_filename_component(folder_name);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as u64;
    let dest_name = format!("{}__{}", now, sanitized);
    let dest_path = root.join(&dest_name);

    // `rename` only works within the same volume. The recycle bin lives under the user's
    // config directory, which may be on a different drive than the pack being deleted, so
    // fall back to a recursive copy + remove for cross-volume moves.
    if std::fs::rename(path, &dest_path).is_err() {
        copy_dir_recursive(path, &dest_path)?;
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("Failed to remove original after copy: {}", e))?;
    }

    let meta = serde_json::json!({
        "original_path": path.to_string_lossy(),
        "deleted_at": now,
    });
    let meta_path = root.join(format!("{}.meta.json", dest_name));
    std::fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Validates that `recycle_path` is a direct child of the recycle bin root (preventing
/// path traversal) and returns the canonicalized recycle bin root and entry path.
fn validate_recycle_entry(recycle_path: &str) -> Result<(PathBuf, PathBuf), String> {
    let root = recycle_bin_root()?;
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical_path = std::path::Path::new(recycle_path)
        .canonicalize()
        .map_err(|_| "Recycle bin item not found".to_string())?;
    if canonical_path.parent() != Some(canonical_root.as_path()) {
        return Err("Path is not a recycle bin item".to_string());
    }
    Ok((canonical_root, canonical_path))
}

#[tauri::command]
fn list_recycled_packs() -> Result<Vec<RecycledPackInfo>, String> {
    let root = recycle_bin_root()?;
    let mut results = Vec::new();

    for entry in std::fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let meta_path = root.join(format!("{}.meta.json", name));
        let (original_path, deleted_at) = std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
            .map(|v| {
                (
                    v.get("original_path")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    v.get("deleted_at").and_then(|n| n.as_u64()).unwrap_or(0),
                )
            })
            .unwrap_or_default();

        let display_name = name
            .split_once("__")
            .map(|(_, rest)| rest.to_string())
            .unwrap_or_else(|| name.clone());
        let size = calculate_folder_size(&p);

        results.push(RecycledPackInfo {
            recycle_path: p.to_string_lossy().to_string(),
            original_path,
            name: display_name,
            deleted_at,
            size,
            size_formatted: format_bytes(size),
        });
    }

    results.sort_by_key(|r| std::cmp::Reverse(r.deleted_at));
    Ok(results)
}

#[tauri::command]
fn restore_recycled_pack(recycle_path: String) -> Result<String, String> {
    let (root, canonical_path) = validate_recycle_entry(&recycle_path)?;
    let name = canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid recycle entry")?
        .to_string();
    let meta_path = root.join(format!("{}.meta.json", name));
    let meta_content = std::fs::read_to_string(&meta_path)
        .map_err(|_| "Missing recycle bin metadata".to_string())?;
    let meta: serde_json::Value = serde_json::from_str(&meta_content).map_err(|e| e.to_string())?;
    let original_path_str = meta
        .get("original_path")
        .and_then(|s| s.as_str())
        .ok_or("Missing original path in recycle bin metadata")?;
    let original_path = std::path::Path::new(original_path_str);

    if original_path.exists() {
        return Err("A pack already exists at the original location".to_string());
    }
    let parent = original_path.parent().ok_or("Invalid original path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;

    if std::fs::rename(&canonical_path, original_path).is_err() {
        copy_dir_recursive(&canonical_path, original_path)?;
        std::fs::remove_dir_all(&canonical_path).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_file(&meta_path);

    Ok(original_path.to_string_lossy().to_string())
}

#[tauri::command]
fn permanently_delete_recycled(recycle_path: String) -> Result<(), String> {
    let (root, canonical_path) = validate_recycle_entry(&recycle_path)?;
    let name = canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid recycle entry")?
        .to_string();
    std::fs::remove_dir_all(&canonical_path).map_err(|e| e.to_string())?;
    let meta_path = root.join(format!("{}.meta.json", name));
    let _ = std::fs::remove_file(&meta_path);
    Ok(())
}

#[tauri::command]
fn empty_recycle_bin() -> Result<(), String> {
    let root = recycle_bin_root()?;
    for entry in std::fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let p = entry.path();
        if p.is_dir() {
            let _ = std::fs::remove_dir_all(&p);
        } else {
            let _ = std::fs::remove_file(&p);
        }
    }
    Ok(())
}

#[tauri::command]
fn delete_pack(path: String, app: AppHandle) -> Result<(), String> {
    let folder_path = std::path::Path::new(&path);
    if !is_direct_managed_pack_directory(folder_path, &app) {
        return Err("Path is not a direct managed pack directory".to_string());
    }

    move_to_recycle_bin(folder_path)
}

#[tauri::command]
fn delete_packs(paths: Vec<String>, app: AppHandle) -> Result<Vec<String>, String> {
    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for path in paths {
        let folder_path = std::path::Path::new(&path);
        if !is_direct_managed_pack_directory(folder_path, &app) {
            errors.push(format!("{}: not a direct managed pack directory", path));
            continue;
        }
        match move_to_recycle_bin(folder_path) {
            Ok(()) => deleted.push(path),
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
    let canonical_parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    let canonical_scan = std::path::Path::new(scan_location)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(scan_location));
    let parent_str = canonical_parent.to_string_lossy().to_lowercase();
    let scan_str = canonical_scan.to_string_lossy().to_lowercase();
    if parent_str != scan_str {
        return Err("File is outside the scan folder".to_string());
    }
    std::fs::remove_file(file_path).map_err(|e| format!("Failed to delete file: {}", e))
}

#[tauri::command]
async fn get_all_pack_icons(paths: Vec<String>) -> Result<Vec<(String, Option<String>)>, String> {
    let results: Vec<(String, Option<String>)> =
        tokio::task::spawn_blocking(move || -> Result<Vec<(String, Option<String>)>, String> {
            use rayon::prelude::*;
            let pool = build_background_pool()?;
            Ok(pool.install(|| {
                paths
                    .into_par_iter()
                    .map(|path| {
                        let icon = read_pack_icon(std::path::Path::new(&path));
                        (path, icon)
                    })
                    .collect()
            }))
        })
        .await
        .map_err(|e| e.to_string())??;

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
            let uuid = json
                .get("header")
                .and_then(|h| h.get("uuid"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string());

            let name = json
                .get("header")
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
    log_content.push_str(&format!(
        "Timestamp: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
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
        if bordered {
            "defaultborder"
        } else {
            "defaultnoborder"
        }
    } else {
        if bordered {
            "blackredborder"
        } else {
            "blackrednoborder"
        }
    };

    emit_log(&app, "INFO", &format!("Setting icon: {}", icon_name));

    let bytes = icon_bytes_for(icon_name).ok_or_else(|| format!("Unknown icon: {}", icon_name))?;

    let icon = decode_icon(bytes).ok_or_else(|| format!("Failed to decode icon: {}", icon_name))?;

    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    window.set_icon(icon).map_err(|e| {
        let msg = format!("Failed to set icon: {}", e);
        emit_log(&app, "ERROR", &msg);
        msg
    })?;

    emit_log(
        &app,
        "INFO",
        &format!("Window icon changed to: {}", icon_name),
    );

    Ok(())
}

#[tauri::command]
fn minimize_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;
    window
        .minimize()
        .map_err(|e| format!("Failed to minimize: {}", e))?;
    Ok(())
}

#[tauri::command]
fn maximize_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;
    let is_maximized = window.is_maximized().unwrap_or(false);
    if is_maximized {
        window
            .unmaximize()
            .map_err(|e| format!("Failed to unmaximize: {}", e))?;
    } else {
        window
            .maximize()
            .map_err(|e| format!("Failed to maximize: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn close_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;
    window
        .close()
        .map_err(|e| format!("Failed to close: {}", e))?;
    Ok(())
}

fn calculate_folder_size(path: &std::path::Path) -> u64 {
    const MAX_ENTRIES: usize = 1_000_000;
    let mut size = 0;
    let mut stack = vec![path.to_path_buf()];
    let mut visited = 0usize;

    while let Some(current_path) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current_path) {
            for entry in entries.flatten() {
                if visited >= MAX_ENTRIES {
                    return size;
                }
                visited += 1;
                match std::fs::symlink_metadata(entry.path()) {
                    Ok(metadata) => {
                        if metadata.file_type().is_symlink() {
                            continue;
                        }
                        if metadata.is_dir() {
                            stack.push(entry.path());
                        } else {
                            size = size.saturating_add(metadata.len());
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

fn validate_process_packs(packs: &[PackInfo], scan_location: Option<&str>) -> Result<(), String> {
    if packs.is_empty() {
        return Err("No packs were selected".to_string());
    }
    let scan_location = scan_location.ok_or("No scan location configured")?;
    let scan_directory = std::path::Path::new(scan_location)
        .canonicalize()
        .map_err(|_| "Configured scan location does not exist")?;
    let allowed_extensions = ["mcpack", "mcaddon", "mctemplate"];

    for pack in packs {
        let path = std::path::Path::new(&pack.path);
        let canonical_path = path
            .canonicalize()
            .map_err(|_| format!("Pack file does not exist: {}", pack.path))?;
        let extension = canonical_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .unwrap_or_default();
        if !canonical_path.is_file()
            || !allowed_extensions.contains(&extension.as_str())
            || canonical_path.parent() != Some(scan_directory.as_path())
        {
            return Err(format!(
                "Pack must be a direct archive file in the scan folder: {}",
                pack.path
            ));
        }
    }
    Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceIconResult {
    pub path: String,
    pub icon_base64: String,
}

/// Reads keys.tsv from the Toolcoin installation, matches the provided pack UUIDs
/// to marketplace UUIDs, fetches thumbnail images, and returns base64-encoded icons.
/// Packs whose UUID is not in keys.tsv, or whose fetch fails, are silently skipped.
#[tauri::command]
async fn fetch_marketplace_icons(
    packs: Vec<serde_json::Value>,
) -> Result<Vec<MarketplaceIconResult>, String> {
    const KEYS_TSV: &str = r"C:\Program Files\alphtoolcoin\data\flutter_assets\assets\keys.tsv";
    const MAX_DIMENSION: u32 = 256;
    const MAX_REMOTE_ICON_BYTES: usize = 8 * 1024 * 1024;

    // Parse keys.tsv: ManifestUUID (col 1, 0-indexed) → MarketUUID (col 0)
    let tsv_content =
        std::fs::read_to_string(KEYS_TSV).map_err(|e| format!("Cannot read keys.tsv: {e}"))?;

    let mut manifest_to_market: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for line in tsv_content.lines().skip(1) {
        let cols: Vec<&str> = line.splitn(4, '\t').collect();
        if cols.len() >= 2 {
            let market_uuid = cols[0].trim().to_lowercase();
            let manifest_uuid = cols[1].trim().to_lowercase();
            manifest_to_market.insert(manifest_uuid, market_uuid);
        }
    }

    // Build list of (path, market_uuid) for packs that have a matching UUID
    let mut to_fetch: Vec<(String, String)> = Vec::new();
    for pack in &packs {
        let path = pack
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let uuid = pack
            .get("uuid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        if let Some(market_uuid) = manifest_to_market.get(&uuid) {
            to_fetch.push((path, market_uuid.clone()));
        }
    }

    if to_fetch.is_empty() {
        return Ok(vec![]);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();

    'fetch: for (path, market_uuid) in to_fetch {
        let url = format!(
            "https://ugc.production.minecraftservices.com/v1/publishedServiceContent/image/offer/{}/Thumbnail_512.0.jpg",
            market_uuid
        );

        let Ok(mut resp) = client.get(&url).send().await else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        if resp
            .content_length()
            .is_some_and(|length| length > MAX_REMOTE_ICON_BYTES as u64)
        {
            continue;
        }
        let mut bytes = Vec::new();
        loop {
            let Ok(chunk) = resp.chunk().await else {
                continue 'fetch;
            };
            let Some(chunk) = chunk else { break };
            if bytes.len().saturating_add(chunk.len()) > MAX_REMOTE_ICON_BYTES {
                continue 'fetch;
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            continue;
        }

        // Resize if needed
        let icon_b64 = if let Ok(img) = image::load_from_memory(&bytes) {
            let resized = if img.width() > MAX_DIMENSION || img.height() > MAX_DIMENSION {
                img.resize(
                    MAX_DIMENSION,
                    MAX_DIMENSION,
                    image::imageops::FilterType::Lanczos3,
                )
            } else {
                img
            };
            let mut buf = Vec::new();
            if resized
                .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .is_ok()
            {
                format!(
                    "data:image/png;base64,{}",
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buf)
                )
            } else {
                continue;
            }
        } else {
            // fallback: encode raw JPEG bytes
            format!(
                "data:image/jpeg;base64,{}",
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
            )
        };

        results.push(MarketplaceIconResult {
            path,
            icon_base64: icon_b64,
        });
    }

    Ok(results)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = load_settings_from_file();
    let icon_style = settings
        .taskbar_icon_style
        .clone()
        .unwrap_or_else(|| "blackred".to_string());
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
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            settings: RwLock::new(settings),
            move_history: Arc::new(RwLock::new(Vec::new())),
            watching: AtomicBool::new(false),
            debug_mode: AtomicBool::new(debug_mode),
            watch_stop_tx: parking_lot::Mutex::new(None),
            scan_watching: AtomicBool::new(false),
            scan_watch_stop_tx: parking_lot::Mutex::new(None),
            cancel_requested: AtomicBool::new(false),
        })
        .setup(move |app| {
            let icon_name = if icon_style == "default" {
                if icon_bordered {
                    "defaultborder"
                } else {
                    "defaultnoborder"
                }
            } else {
                if icon_bordered {
                    "blackredborder"
                } else {
                    "blackrednoborder"
                }
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
            open_help_page,
            write_export_file,
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
            delete_packs,
            suggest_pack_renames,
            rename_installed_pack,
            rename_installed_packs,
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
            fetch_marketplace_icons,
            find_duplicate_packs,
            watch_scan_folder,
            stop_watching_scan_folder,
            list_archive_files,
            list_mc_files_in_dir,
            request_cancel,
            resolve_scan_directory,
            list_recycled_packs,
            restore_recycled_pack,
            permanently_delete_recycled,
            empty_recycle_bin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
