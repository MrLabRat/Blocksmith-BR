use std::path::PathBuf;
use std::fs;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use super::pack_type::{PackInfo, PackType, Settings};
use super::pack_detector::extract_pack_to_destination;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOperation {
    pub source: String,
    pub destination: String,
    pub pack_name: String,
    pub pack_type: PackType,
    pub success: bool,
    pub error: Option<String>,
    pub is_template_update: Option<bool>,
    pub skin_pack_4d_path: Option<String>,
    pub deleted_old_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub type LogSender = mpsc::UnboundedSender<LogEntry>;
pub type MoveHistory = Arc<RwLock<Vec<MoveOperation>>>;

fn strip_pack_suffix(name: &str) -> String {
    let suffixes = [" (ADDON)", "(ADDON)", " (RESOURCE)", "(RESOURCE)", " (SKIN)", "(SKIN)", " (TEMPLATE)", "(TEMPLATE)", " (MASHUP)", "(MASHUP)"];
    let mut result = name.to_string();
    for suffix in &suffixes {
        if result.ends_with(suffix) {
            result = result[..result.len()-suffix.len()].to_string();
            break;
        }
    }
    result.trim().to_string()
}

fn find_old_pack_path(dest_base: &PathBuf, pack_name: &str, pack_type: PackType) -> Option<PathBuf> {
    if !dest_base.exists() {
        return None;
    }
    
    let base_name = strip_pack_suffix(pack_name).to_lowercase();
    
    if let Ok(entries) = fs::read_dir(dest_base) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                if let Some(folder_name) = entry_path.file_name().and_then(|n| n.to_str()) {
                    let folder_base = strip_pack_suffix(folder_name).to_lowercase();
                    
                    if folder_base == base_name {
                        let type_suffix = match pack_type {
                            PackType::BehaviorPack => " (ADDON)",
                            PackType::ResourcePack => " (RESOURCE)",
                            PackType::SkinPack => " (SKIN)",
                            PackType::SkinPack4D => "",
                            PackType::WorldTemplate => " (TEMPLATE)",
                            PackType::MashupPack => " (MASHUP)",
                            PackType::Unknown => "",
                        };
                        
                        let expected_name = format!("{}{}", strip_pack_suffix(pack_name), type_suffix);
                        if folder_name != expected_name {
                            return Some(entry_path);
                        }
                    }
                }
            }
        }
    }
    
    None
}

pub struct FileMover {
    settings: Settings,
    log_tx: Option<LogSender>,
    history: MoveHistory,
}

impl FileMover {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            log_tx: None,
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub fn set_log_sender(&mut self, tx: LogSender) {
        self.log_tx = Some(tx);
    }
    
    fn log(&self, level: &str, message: &str) {
        if let Some(tx) = &self.log_tx {
            let _ = tx.send(LogEntry {
                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                level: level.to_string(),
                message: message.to_string(),
            });
        }
    }
    
    pub fn get_destination_path(&self, pack_type: PackType, scan_dir: Option<&PathBuf>) -> Option<PathBuf> {
        match pack_type {
            PackType::SkinPack4D => {
                if let Some(scan) = scan_dir {
                    return Some(scan.join("4D Skin Packs"));
                }
                return None;
            }
            _ => {
                let path_str = match pack_type {
                    PackType::BehaviorPack => &self.settings.behavior_pack_path,
                    PackType::ResourcePack => &self.settings.resource_pack_path,
                    PackType::SkinPack => &self.settings.skin_pack_path,
                    PackType::WorldTemplate | PackType::MashupPack => &self.settings.world_template_path,
                    PackType::Unknown => return None,
                    PackType::SkinPack4D => unreachable!("SkinPack4D handled above"),
                };
                
                path_str.as_ref().map(|p| PathBuf::from(p))
            }
        }
    }
    
    pub async fn process_pack(&self, pack: &PackInfo, scan_dir: Option<&PathBuf>) -> MoveOperation {
        let source = PathBuf::from(&pack.path);
        
        let (dest_base, is_4d_skin_pack) = if pack.pack_type == PackType::SkinPack4D {
            let parent_dir = source.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            let four_d_dir = parent_dir.join("4D Skin Packs");
            (four_d_dir, true)
        } else {
            match self.get_destination_path(pack.pack_type, scan_dir) {
                Some(p) => (p, false),
                None => {
                    self.log("ERROR", &format!("No destination path configured for {}", pack.pack_type));
                    return MoveOperation {
                        source: pack.path.clone(),
                        destination: String::new(),
                        pack_name: pack.name.clone(),
                        pack_type: pack.pack_type,
                        success: false,
                        error: Some("No destination path configured".to_string()),
                        is_template_update: None,
                        skin_pack_4d_path: None,
                        deleted_old_path: None,
                    };
                }
            }
        };
        
        let type_suffix = match pack.pack_type {
            PackType::BehaviorPack => " (ADDON)",
            PackType::ResourcePack => " (RESOURCE)",
            PackType::SkinPack => " (SKIN)",
            PackType::SkinPack4D => "",
            PackType::WorldTemplate => " (TEMPLATE)",
            PackType::MashupPack => " (MASHUP)",
            PackType::Unknown => "",
        };
        
        let output_name = format!("{}{}", pack.name, type_suffix);
        let destination = dest_base.join(&output_name);
        
        let is_template_update = (pack.pack_type == PackType::WorldTemplate || pack.pack_type == PackType::MashupPack) 
            && destination.exists();
        
        let old_pack_path = if !is_4d_skin_pack && pack.is_update.unwrap_or(false) {
            find_old_pack_path(&dest_base, &pack.name, pack.pack_type)
        } else {
            None
        };
        
        if self.settings.dry_run {
            self.log("INFO", &format!("[DRY RUN] Would extract '{}' to '{}'", pack.name, destination.display()));
            if let Some(ref old_path) = old_pack_path {
                self.log("INFO", &format!("[DRY RUN] Would delete old version at '{}'", old_path.display()));
            }
            return MoveOperation {
                source: pack.path.clone(),
                destination: destination.to_string_lossy().to_string(),
                pack_name: output_name,
                pack_type: pack.pack_type,
                success: true,
                error: None,
                is_template_update: if is_template_update { Some(true) } else { None },
                skin_pack_4d_path: if is_4d_skin_pack { Some(destination.to_string_lossy().to_string()) } else { None },
                deleted_old_path: old_pack_path.map(|p| p.to_string_lossy().to_string()),
            };
        }
        
        if pack.pack_type == PackType::SkinPack4D {
            self.log("INFO", "4D Skin Pack will be extracted for use with SkinMaster");
        }
        
        if let Some(ref old_path) = old_pack_path {
            self.log("INFO", &format!("Deleting old version at '{}'", old_path.display()));
            if let Err(e) = fs::remove_dir_all(old_path) {
                self.log("WARN", &format!("Failed to delete old version: {}", e));
            }
        }
        
        self.log("INFO", &format!("Extracting '{}' to '{}'", pack.name, destination.display()));
        
        let source_clone = source.clone();
        let dest_base_clone = dest_base.clone();
        let pack_type_for_extract = pack.pack_type;
        let subfolder = pack.subfolder.clone();
        let output_name_for_extract = output_name.clone();
        let old_pack_path_clone = old_pack_path.clone();
        
        let result = tokio::task::spawn_blocking(move || {
            extract_pack_to_destination(
                &source_clone, 
                &dest_base_clone, 
                pack_type_for_extract, 
                subfolder.as_deref(),
                Some(&output_name_for_extract),
            )
        }).await;
        
        let result = match result {
            Ok(r) => r,
            Err(e) => Err(e.to_string()),
        };
        
        match result {
            Ok(dest_path) => {
                self.log("SUCCESS", &format!("Successfully extracted '{}' to '{}'", pack.name, dest_path));
                if is_template_update {
                    self.log("WARN", "World template updated - existing worlds may need manual update");
                }
                if is_4d_skin_pack {
                    self.log("INFO", &format!("4D Skin Pack extracted. Use this path with SkinMaster: {}", dest_path));
                }
                let op = MoveOperation {
                    source: pack.path.clone(),
                    destination: dest_path.clone(),
                    pack_name: output_name,
                    pack_type: pack.pack_type,
                    success: true,
                    error: None,
                    is_template_update: if is_template_update { Some(true) } else { None },
                    skin_pack_4d_path: if is_4d_skin_pack { Some(dest_path) } else { None },
                    deleted_old_path: old_pack_path_clone.map(|p| p.to_string_lossy().to_string()),
                };
                self.history.write().push(op.clone());
                op
            }
            Err(e) => {
                self.log("ERROR", &format!("Failed to extract '{}': {}", pack.name, e));
                MoveOperation {
                    source: pack.path.clone(),
                    destination: destination.to_string_lossy().to_string(),
                    pack_name: output_name,
                    pack_type: pack.pack_type,
                    success: false,
                    error: Some(e),
                    is_template_update: None,
                    skin_pack_4d_path: None,
                    deleted_old_path: None,
                }
            }
        }
    }
    
    pub async fn rollback_last(&self) -> Option<MoveOperation> {
        let op = {
            let mut history = self.history.write();
            history.pop()
        }?;
        
        if self.settings.dry_run {
            self.log("INFO", &format!("[DRY RUN] Would rollback '{}'", op.pack_name));
            return Some(op);
        }
        
        self.log("INFO", &format!("Rolling back '{}'", op.pack_name));
        
        let source = PathBuf::from(&op.destination);
        
        let result = tokio::task::spawn_blocking(move || {
            if !source.exists() {
                return Err("Extracted folder no longer exists".to_string());
            }
            
            fs::remove_dir_all(&source).map_err(|e| e.to_string())?;
            
            Ok::<(), String>(())
        }).await;
        
        match result {
            Ok(Ok(())) => {
                self.log("SUCCESS", &format!("Successfully rolled back '{}'", op.pack_name));
                Some(op)
            }
            Ok(Err(e)) => {
                self.log("ERROR", &format!("Failed to rollback '{}': {}", op.pack_name, e));
                None
            }
            Err(e) => {
                self.log("ERROR", &format!("Failed to rollback '{}': {}", op.pack_name, e));
                None
            }
        }
    }
}
