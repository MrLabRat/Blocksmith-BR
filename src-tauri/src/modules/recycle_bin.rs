use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecycledPackInfo {
    pub recycle_path: String,
    pub original_path: String,
    pub name: String,
    pub deleted_at: u64,
    pub size: u64,
    pub size_formatted: String,
}

pub fn recycle_bin_root() -> Result<PathBuf, String> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| "Could not determine config directory".to_string())?;
    let root = config_dir.join("blocksmith").join("recycle_bin");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_entry = entry.path();
        let dst_entry = dst.join(entry.file_name());

        if src_entry.is_dir() {
            copy_dir_recursive(&src_entry, &dst_entry)?;
        } else {
            fs::copy(&src_entry, &dst_entry).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn move_to_recycle_bin(path: &Path) -> Result<(), String> {
    let root = recycle_bin_root()?;
    let folder_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("pack");
    let sanitized = super::pack_detector::sanitize_filename_component(folder_name);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as u64;
    let dest_name = format!("{}__{}", now, sanitized);
    let dest_path = root.join(&dest_name);

    if fs::rename(path, &dest_path).is_err() {
        copy_dir_recursive(path, &dest_path)?;
        fs::remove_dir_all(path)
            .map_err(|e| format!("Failed to remove original after copy: {}", e))?;
    }

    let meta = serde_json::json!({
        "original_path": path.to_string_lossy(),
        "deleted_at": now,
    });
    let meta_path = root.join(format!("{}.meta.json", dest_name));
    fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn validate_recycle_entry(recycle_path: &str) -> Result<(PathBuf, PathBuf), String> {
    let root = recycle_bin_root()?;
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical_path = Path::new(recycle_path)
        .canonicalize()
        .map_err(|_| "Recycle bin item not found".to_string())?;
    if canonical_path.parent() != Some(canonical_root.as_path()) {
        return Err("Path is not a recycle bin item".to_string());
    }
    Ok((canonical_root, canonical_path))
}
