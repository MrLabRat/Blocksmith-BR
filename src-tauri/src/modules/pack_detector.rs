use super::pack_type::{PackInfo, PackType};
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::ZipArchive;

pub fn scan_single_pack(file_path: &Path) -> Vec<PackInfo> {
    let file = match fs::File::open(file_path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return vec![],
    };

    let filename = file_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let is_mashup = is_mashup_name(&filename);
    let cleaned_name = clean_pack_name(&filename);

    // Check for skins.json anywhere in the archive (not just root)
    let mut has_skins_json = archive.by_name("skins.json").is_ok();
    let mut skins_json_subfolder: Option<String> = None;

    if !has_skins_json {
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                let name = file.name();
                if name.ends_with("skins.json") {
                    has_skins_json = true;
                    if let Some(idx) = name.rfind('/') {
                        skins_json_subfolder = Some(name[..idx].to_string());
                    }
                    break;
                }
            }
        }
    }

    if has_skins_json {
        let is_4d = check_4d_in_archive(&mut archive);
        let pack_type = if is_4d {
            PackType::SkinPack4D
        } else {
            PackType::SkinPack
        };

        let (needs_attention, attention_message) = if is_4d {
            check_4d_special_files(&mut archive)
        } else {
            (false, None)
        };

        let icon = extract_icon_from_archive(&mut archive, "");

        return vec![PackInfo {
            path: file_path.to_string_lossy().to_string(),
            name: cleaned_name,
            pack_type,
            uuid: None,
            version: None,
            extracted: false,
            icon_base64: icon,
            subfolder: skins_json_subfolder,
            folder_size: None,
            folder_size_formatted: None,
            needs_attention: Some(needs_attention),
            attention_message,
            is_installed: None,
            is_update: None,
            installed_version: None,
        }];
    }

    let subfolders = detect_subfolders(&mut archive);

    if !subfolders.is_empty() {
        return process_multi_pack_archive(file_path, &mut archive, &subfolders);
    }

    let (pack_type, uuid, version) = get_pack_info_from_archive(&mut archive);
    let icon = extract_icon_from_archive(&mut archive, "");

    // Override to MashupPack if name indicates mashup and it's a world template
    let final_type = if is_mashup && pack_type == PackType::WorldTemplate {
        PackType::MashupPack
    } else {
        pack_type
    };

    vec![PackInfo {
        path: file_path.to_string_lossy().to_string(),
        name: cleaned_name,
        pack_type: final_type,
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
    }]
}

fn is_mashup_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("mashup") || lower.contains("mash-up") || lower.contains("mash up")
}

fn check_4d_special_files(archive: &mut ZipArchive<fs::File>) -> (bool, Option<String>) {
    let mut has_readme = false;
    let mut has_multiple_geometry_folders = false;
    let mut geometry_folders = std::collections::HashSet::new();

    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name().to_lowercase();

            // Check for readme/instruction files
            if name.contains("readme") || name.contains("instructions") || name.contains("install")
            {
                if name.ends_with(".txt") || name.ends_with(".md") {
                    has_readme = true;
                }
            }

            // Check for multiple geometry folders
            if name.contains("geometry") && name.contains('/') {
                if let Some(folder) = name.split('/').next() {
                    geometry_folders.insert(folder.to_string());
                }
            }
        }
    }

    if geometry_folders.len() > 1 {
        has_multiple_geometry_folders = true;
    }

    if has_readme || has_multiple_geometry_folders {
        let mut messages = Vec::new();
        if has_readme {
            messages.push("Contains instructions/readme");
        }
        if has_multiple_geometry_folders {
            messages.push("Multiple geometry folders detected");
        }
        messages.push("May require manual setup");
        messages.push("SkinMaster may not work with this pack");

        (true, Some(messages.join(". ") + "."))
    } else {
        (false, None)
    }
}

fn detect_subfolders(archive: &mut ZipArchive<fs::File>) -> Vec<String> {
    let mut manifest_folders = std::collections::HashSet::new();
    let mut is_world_template = false;
    let mut has_root_manifest = false;

    // First pass: find all manifest.json folders and collect info
    let mut manifest_contents: Vec<(String, String)> = Vec::new(); // (path, content)

    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            let name = file.name().to_string();

            if name.ends_with("manifest.json") {
                // Get the folder containing manifest.json
                if let Some(idx) = name.rfind('/') {
                    let folder = &name[..idx];
                    if !folder.is_empty() {
                        manifest_folders.insert(folder.to_string());
                    }
                } else {
                    // Root manifest.json (no folder)
                    has_root_manifest = true;
                }

                // Read content for later analysis
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    manifest_contents.push((name.clone(), content));
                }
            }
        }
    }

    // Analyze manifest contents to detect world template
    for (path, content) in &manifest_contents {
        // Check if this is a root manifest
        let is_root = !path.contains('/');
        if is_root {
            if let Ok(json) = serde_json::from_str::<Value>(content) {
                if let Some(modules) = json.get("modules").and_then(|m| m.as_array()) {
                    for module in modules {
                        if let Some(type_str) = module.get("type").and_then(|t| t.as_str()) {
                            if type_str == "world_template" {
                                is_world_template = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Determine the actual pack subfolders
    let mut subfolders: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for folder in &manifest_folders {
        let parts: Vec<&str> = folder.split('/').collect();

        if parts.len() == 1 {
            // Direct pack folder at root level: "ppack0" or "ppack1"
            if !seen.contains(folder) {
                seen.insert(folder.clone());
                subfolders.push(folder.clone());
            }
        } else if parts.len() == 2 {
            // Nested under container: "behavior_packs/ppack0" or "resource_packs/ppack1"
            let container = parts[0].to_lowercase();

            // If this is a world template, skip behavior_packs and resource_packs inside it
            // They are internal to the template and not standalone packs
            if is_world_template
                && (container == "behavior_packs"
                    || container == "behaviour_packs"
                    || container == "resource_packs")
            {
                continue;
            }

            if container == "behavior_packs"
                || container == "behaviour_packs"
                || container == "resource_packs"
                || container == "skin_packs"
            {
                if !seen.contains(folder) {
                    seen.insert(folder.clone());
                    subfolders.push(folder.clone());
                }
            } else {
                // Unknown container, use the folder as-is
                if !seen.contains(folder) {
                    seen.insert(folder.clone());
                    subfolders.push(folder.clone());
                }
            }
        } else if parts.len() >= 3 {
            // Deep nesting
            let container = parts[0].to_lowercase();

            // If this is a world template, skip internal behavior_packs and resource_packs
            if is_world_template
                && (container == "behavior_packs"
                    || container == "behaviour_packs"
                    || container == "resource_packs")
            {
                continue;
            }

            let nested_path = format!("{}/{}", parts[0], parts[1]);
            if !seen.contains(&nested_path) {
                seen.insert(nested_path.clone());
                subfolders.push(nested_path);
            }
        }
    }

    // If this is a world template with root manifest and no subfolders,
    // or we filtered out all subfolders, return empty to process as single pack
    if is_world_template && has_root_manifest {
        // World templates should be processed as a single unit
        return vec![];
    }

    // Sort: behavior packs first (ppack0, behavior_packs/*), then resource packs (ppack1, resource_packs/*)
    subfolders.sort_by(|a, b| {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();

        fn is_behavior_pack(s: &str) -> bool {
            s.contains("behavior")
                || s.contains("behaviour")
                || s.contains("ppack0")
                || s.contains("/bp0")
                || s.contains("/bp1")
                || s.ends_with("pack0")
                || (s.contains("ppack") && s.contains("0"))
        }

        let a_is_bp = is_behavior_pack(&a_lower);
        let b_is_bp = is_behavior_pack(&b_lower);

        if a_is_bp && !b_is_bp {
            std::cmp::Ordering::Less
        } else if !a_is_bp && b_is_bp {
            std::cmp::Ordering::Greater
        } else {
            a_lower.cmp(&b_lower)
        }
    });

    subfolders
}

fn process_multi_pack_archive(
    file_path: &Path,
    archive: &mut ZipArchive<fs::File>,
    subfolders: &[String],
) -> Vec<PackInfo> {
    let mut packs = Vec::new();
    let base_filename = file_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let cleaned_name = clean_pack_name(&base_filename);
    let is_mashup = is_mashup_name(&base_filename);

    for subfolder in subfolders.iter() {
        let (mut pack_type, uuid, version) = get_pack_info_from_subfolder(archive, subfolder);
        let icon = extract_icon_from_archive(archive, subfolder);

        // Override to MashupPack if filename indicates mash-up
        if is_mashup {
            pack_type = PackType::MashupPack;
        }

        packs.push(PackInfo {
            path: file_path.to_string_lossy().to_string(),
            name: cleaned_name.clone(),
            pack_type,
            uuid,
            version,
            extracted: false,
            icon_base64: icon,
            subfolder: Some(subfolder.clone()),
            folder_size: None,
            folder_size_formatted: None,
            needs_attention: None,
            attention_message: None,
            is_installed: None,
            is_update: None,
            installed_version: None,
        });
    }

    if packs.is_empty() {
        let (pack_type, uuid, version) = get_pack_info_from_archive(archive);
        let icon = extract_icon_from_archive(archive, "");

        packs.push(PackInfo {
            path: file_path.to_string_lossy().to_string(),
            name: cleaned_name,
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
        });
    }

    packs
}

fn clean_pack_name(name: &str) -> String {
    let mut cleaned = name.to_string();

    let suffixes = [
        " (addon)",
        "(addon)",
        " (ADDON)",
        "(ADDON)",
        " (Addon)",
        "(Addon)",
        " (behavior)",
        "(behavior)",
        " (BEHAVIOR)",
        "(BEHAVIOR)",
        " (resource)",
        "(resource)",
        " (RESOURCE)",
        "(RESOURCE)",
        " (resources)",
        "(resources)",
        " (RESOURCES)",
        "(RESOURCES)",
        " (bp)",
        "(bp)",
        " (BP)",
        "(BP)",
        " (rp)",
        "(rp)",
        " (RP)",
        "(RP)",
        " (world_template)",
        "(world_template)",
        " (WORLD_TEMPLATE)",
        "(WORLD_TEMPLATE)",
        " (template)",
        "(template)",
        " (TEMPLATE)",
        "(TEMPLATE)",
        " (skin_pack)",
        "(skin_pack)",
        " (SKIN_PACK)",
        "(SKIN_PACK)",
        " (skin)",
        "(skin)",
        " (SKIN)",
        "(SKIN)",
    ];

    for suffix in &suffixes {
        let cleaned_lower = cleaned.to_lowercase();
        let suffix_lower = suffix.to_lowercase();
        if cleaned_lower.ends_with(&suffix_lower) {
            cleaned = cleaned[..cleaned.len() - suffix.len()].to_string();
            break;
        }
    }

    cleaned.trim().to_string()
}

fn get_pack_info_from_subfolder(
    archive: &mut ZipArchive<fs::File>,
    subfolder: &str,
) -> (PackType, Option<String>, Option<String>) {
    let manifest_path = format!("{}/manifest.json", subfolder);

    if let Ok(mut file) = archive.by_name(&manifest_path) {
        let mut content = String::new();
        if file.read_to_string(&mut content).is_ok() {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                let pack_type = determine_pack_type(&json);
                let uuid = extract_uuid(&json);
                let version = extract_version(&json);

                if pack_type == PackType::Unknown {
                    let subfolder_lower = subfolder.to_lowercase();
                    let fallback_type = if subfolder_lower.contains("behavior")
                        || subfolder_lower.contains("behaviour")
                        || subfolder_lower == "ppack0"
                        || subfolder_lower.ends_with("/ppack0")
                    {
                        PackType::BehaviorPack
                    } else if subfolder_lower.contains("resource")
                        || subfolder_lower == "ppack1"
                        || subfolder_lower.ends_with("/ppack1")
                    {
                        PackType::ResourcePack
                    } else {
                        pack_type
                    };
                    return (fallback_type, uuid, version);
                }

                return (pack_type, uuid, version);
            }
        }
    }

    let subfolder_lower = subfolder.to_lowercase();

    let pack_type = if subfolder_lower.contains("behavior")
        || subfolder_lower.contains("behaviour")
        || subfolder_lower == "ppack0"
        || subfolder_lower.ends_with("/ppack0")
    {
        PackType::BehaviorPack
    } else if subfolder_lower.contains("resource")
        || subfolder_lower == "ppack1"
        || subfolder_lower.ends_with("/ppack1")
    {
        PackType::ResourcePack
    } else {
        PackType::Unknown
    };

    (pack_type, None, None)
}

fn get_pack_info_from_archive(
    archive: &mut ZipArchive<fs::File>,
) -> (PackType, Option<String>, Option<String>) {
    if let Ok(mut file) = archive.by_name("manifest.json") {
        let mut content = String::new();
        if file.read_to_string(&mut content).is_ok() {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                let pack_type = determine_pack_type(&json);
                let uuid = extract_uuid(&json);
                let version = extract_version(&json);
                return (pack_type, uuid, version);
            }
        }
    }

    (PackType::Unknown, None, None)
}

fn extract_icon_from_archive(
    archive: &mut ZipArchive<fs::File>,
    subfolder: &str,
) -> Option<String> {
    let icon_names = if subfolder.is_empty() {
        vec![
            "pack_icon.png".to_string(),
            "Pack_Icon.png".to_string(),
            "world_icon.jpeg".to_string(),
            "world_icon.jpg".to_string(),
        ]
    } else {
        vec![
            format!("{}/pack_icon.png", subfolder),
            format!("{}/Pack_Icon.png", subfolder),
            format!("{}/world_icon.jpeg", subfolder),
            format!("{}/world_icon.jpg", subfolder),
        ]
    };

    for icon_name in &icon_names {
        if let Ok(mut file) = archive.by_name(icon_name) {
            let mut buffer = Vec::new();
            if file.read_to_end(&mut buffer).is_ok() {
                return Some(general_purpose::STANDARD.encode(&buffer));
            }
        }
    }

    if subfolder.is_empty() {
        let mut found_index: Option<usize> = None;
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                let name = file.name().to_lowercase();
                if (name.ends_with("pack_icon.png")
                    || name.ends_with("world_icon.jpeg")
                    || name.ends_with("world_icon.jpg"))
                    && !name.contains('/')
                {
                    found_index = Some(i);
                    break;
                }
            }
        }

        if let Some(idx) = found_index {
            if let Ok(mut f) = archive.by_index(idx) {
                let mut buffer = Vec::new();
                if f.read_to_end(&mut buffer).is_ok() {
                    return Some(general_purpose::STANDARD.encode(&buffer));
                }
            }
        }
    }

    None
}

fn check_4d_in_archive(archive: &mut ZipArchive<fs::File>) -> bool {
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name().to_lowercase();
            if name.contains("geometry") && name.ends_with(".json") {
                return true;
            }
        }
    }
    false
}

fn extract_uuid(json: &Value) -> Option<String> {
    json.get("header")
        .and_then(|h| h.get("uuid"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_string())
}

fn extract_version(json: &Value) -> Option<String> {
    json.get("header")
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
        })
}

fn determine_pack_type(json: &Value) -> PackType {
    // Check modules array
    if let Some(modules) = json.get("modules").and_then(|m| m.as_array()) {
        for module in modules {
            if let Some(type_str) = module.get("type").and_then(|t| t.as_str()) {
                match type_str {
                    "data" => return PackType::BehaviorPack,
                    "resources" => return PackType::ResourcePack,
                    "world_template" => return PackType::WorldTemplate,
                    "skin_pack" => return PackType::SkinPack,
                    "script" => return PackType::BehaviorPack,
                    _ => {}
                }
            }
        }
    }

    // Fallback: check header capabilities
    if let Some(header) = json.get("header") {
        if let Some(capabilities) = header.get("capabilities").and_then(|c| c.as_array()) {
            for cap in capabilities {
                if let Some(cap_str) = cap.as_str() {
                    match cap_str {
                        "scriptEngineVersion" => return PackType::BehaviorPack,
                        _ => {}
                    }
                }
            }
        }

        // Check for behavior pack indicators in header
        if let Some(name) = header.get("name").and_then(|n| n.as_str()) {
            let name_lower = name.to_lowercase();
            if name_lower.contains("behavior")
                || name_lower.contains("behaviour")
                || name_lower.contains("addon")
            {
                return PackType::BehaviorPack;
            }
        }
    }

    PackType::Unknown
}

pub fn extract_pack_to_destination(
    file_path: &Path,
    destination_dir: &Path,
    pack_type: PackType,
    subfolder: Option<&str>,
    output_name_override: Option<&str>,
) -> Result<String, String> {
    let filename = file_path
        .file_stem()
        .ok_or("Invalid filename")?
        .to_string_lossy()
        .to_string();

    let type_suffix = match pack_type {
        PackType::BehaviorPack => " (ADDON)",
        PackType::ResourcePack => " (RESOURCE)",
        PackType::SkinPack => " (SKIN)",
        PackType::SkinPack4D => "",
        PackType::WorldTemplate => " (TEMPLATE)",
        PackType::MashupPack => " (MASHUP)",
        PackType::Unknown => "",
    };

    let output_name = if let Some(name) = output_name_override {
        name.to_string()
    } else {
        format!("{}{}", filename, type_suffix)
    };

    let output_path = destination_dir.join(&output_name);

    if output_path.exists() {
        fs::remove_dir_all(&output_path)
            .map_err(|e| format!("Failed to remove existing directory: {}", e))?;
    }

    fs::create_dir_all(&output_path).map_err(|e| format!("Failed to create directory: {}", e))?;

    let file = fs::File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("Failed to read archive: {}", e))?;

    let file_count = archive.len();
    let mut dirs_to_create: Vec<std::path::PathBuf> = Vec::new();
    let mut files_to_extract: Vec<(usize, std::path::PathBuf)> = Vec::new();

    for i in 0..file_count {
        let zip_file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let name = zip_file.name();

        if zip_file
            .unix_mode()
            .map(|m| (m & 0o170000) == 0o120000)
            .unwrap_or(false)
        {
            continue;
        }

        let relative_path = if let Some(sf) = subfolder {
            if name.starts_with(&format!("{}/", sf)) {
                name.strip_prefix(&format!("{}/", sf)).unwrap_or(name)
            } else if name.starts_with(sf) {
                name.strip_prefix(sf)
                    .unwrap_or(name)
                    .trim_start_matches('/')
            } else {
                continue;
            }
        } else {
            name
        };

        let relative_path = relative_path.trim_start_matches('/');

        if relative_path.is_empty() {
            continue;
        }

        if std::path::Path::new(relative_path)
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(format!(
                "Security: Attempted path traversal in zip file: {}",
                relative_path
            ));
        }

        let outpath = output_path.join(relative_path);

        if name.ends_with('/') {
            dirs_to_create.push(outpath);
        } else {
            if let Some(p) = outpath.parent() {
                let p_buf = p.to_path_buf();
                if !dirs_to_create.contains(&p_buf) && !p.exists() {
                    dirs_to_create.push(p_buf);
                }
            }
            files_to_extract.push((i, outpath));
        }
    }

    drop(archive);

    let file = fs::File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("Failed to read archive: {}", e))?;

    for dir in dirs_to_create {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    const BUFFER_SIZE: usize = 256 * 1024;
    let mut buffer = vec![0u8; BUFFER_SIZE];

    for (i, outpath) in files_to_extract {
        let mut zip_file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read entry: {}", e))?;
        let mut outfile =
            fs::File::create(&outpath).map_err(|e| format!("Failed to create file: {}", e))?;
        let mut writer = std::io::BufWriter::with_capacity(BUFFER_SIZE, &mut outfile);

        loop {
            let bytes_read = zip_file
                .read(&mut buffer)
                .map_err(|e| format!("Failed to read: {}", e))?;
            if bytes_read == 0 {
                break;
            }
            writer
                .write_all(&buffer[..bytes_read])
                .map_err(|e| format!("Failed to write: {}", e))?;
        }
    }

    Ok(output_path.to_string_lossy().to_string())
}
