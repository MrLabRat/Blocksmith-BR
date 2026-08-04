use super::pack_type::{PackInfo, PackType};
use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::fs;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use zip::ZipArchive;

// Legitimate high-resolution texture packs can easily contain 10,000-30,000+ individual
// files (one PNG + one texture_set.json per texture variant). The real protection against
// zip-bomb/decompression-DoS style archives is the uncompressed size limits below, not the
// raw entry count, so this ceiling is kept generous to avoid silently dropping real packs.
const MAX_ARCHIVE_ENTRIES: usize = 100_000;
const MAX_ARCHIVE_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_ICON_BYTES: usize = 8 * 1024 * 1024;

fn validated_relative_path(value: &str, field: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    let mut components = path.components();
    let is_single_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if value.is_empty() || !is_single_normal_component {
        return Err(format!(
            "Invalid {}: must be a relative path without separators or traversal",
            field
        ));
    }
    Ok(path.to_path_buf())
}

/// Pack manifests can contain almost any Unicode text in their display name/title,
/// including characters that are illegal in a Windows file/directory name (`< > : " / \ | ? *`
/// and other control characters), or that make the name end in a trailing dot/space, or that
/// collide with a reserved DOS device name (CON, PRN, NUL, COM1, ...). Any of these causes
/// `fs::create_dir`/`fs::rename` to fail with os error 123 ("The filename, directory name, or
/// volume label syntax is incorrect"). This sanitizes a proposed name into something that is
/// always safe to use as a single Windows path component, while keeping it as close to the
/// original title as possible.
pub fn sanitize_filename_component(name: &str) -> String {
    const RESERVED_NAMES: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    let mut sanitized: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();

    // Windows forbids directory/file names that end in a dot or space.
    while sanitized.ends_with(['.', ' ']) {
        sanitized.pop();
    }

    let trimmed = sanitized.trim();
    sanitized = if trimmed.is_empty() {
        "Unnamed Pack".to_string()
    } else {
        trimmed.to_string()
    };

    let base = sanitized.split('.').next().unwrap_or(&sanitized);
    if RESERVED_NAMES.contains(&base.to_ascii_uppercase().as_str()) {
        sanitized.push('_');
    }

    sanitized
}

fn validated_archive_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!("Security: unsafe archive entry path: {}", value));
    }
    Ok(path.to_path_buf())
}

/// Returns a human-readable reason if the archive violates entry-count or size limits,
/// or `None` if it is within limits.
fn archive_limit_violation(archive: &mut ZipArchive<fs::File>) -> Option<String> {
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Some(format!(
            "archive contains {} entries, exceeding the {} entry limit",
            archive.len(),
            MAX_ARCHIVE_ENTRIES
        ));
    }

    let mut total_size = 0u64;
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index(index) else {
            return Some("failed to read an archive entry while checking limits".to_string());
        };
        if entry.size() > MAX_ARCHIVE_ENTRY_BYTES {
            return Some(format!(
                "entry '{}' exceeds the {} byte size limit",
                entry.name(),
                MAX_ARCHIVE_ENTRY_BYTES
            ));
        }
        let Some(next_total) = total_size.checked_add(entry.size()) else {
            return Some("archive size overflowed while checking limits".to_string());
        };
        total_size = next_total;
        if total_size > MAX_ARCHIVE_UNCOMPRESSED_BYTES {
            return Some(format!(
                "archive expands to more than the {} byte limit",
                MAX_ARCHIVE_UNCOMPRESSED_BYTES
            ));
        }
    }

    None
}

/// Diagnostic helper for callers: when a scan yields zero packs for a file, this explains
/// whether it was rejected for exceeding the archive limits (as opposed to simply not
/// containing a recognizable pack).
pub fn archive_rejection_reason(file_path: &Path) -> Option<String> {
    let file = fs::File::open(file_path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    archive_limit_violation(&mut archive)
}

pub fn scan_single_pack(file_path: &Path) -> Vec<PackInfo> {
    let file = match fs::File::open(file_path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return vec![],
    };
    if archive_limit_violation(&mut archive).is_some() {
        return vec![];
    }

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
            nested_mcpack: None,
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

    // Some .mcaddon files bundle their BP/RP as nested .mcpack zip entries instead of
    // exploded folders (e.g. "Feather FPS Boost Mod [BP].mcpack" sitting at the archive
    // root) — no manifest.json is directly visible until we open each nested archive.
    let nested_mcpacks = detect_nested_mcpack_entries(&mut archive);
    if !nested_mcpacks.is_empty() {
        return process_nested_mcpack_archive(file_path, &mut archive, &nested_mcpacks);
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
        nested_mcpack: None,
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
            if (name.contains("readme")
                || name.contains("instructions")
                || name.contains("install"))
                && (name.ends_with(".txt") || name.ends_with(".md"))
            {
                has_readme = true;
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
                || s.contains("bpack0")
                || s.contains("/bp0")
                || s.contains("/bp1")
                || s.ends_with("pack0")
                || s.ends_with(" bp")
                || s.ends_with("[bp]")
                || s.ends_with("(bp)")
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
        let (mut pack_type, uuid, version, manifest_name) =
            get_pack_info_from_subfolder(archive, subfolder);
        let icon = extract_icon_from_archive(archive, subfolder);

        // Override to MashupPack if filename indicates mash-up
        if is_mashup {
            pack_type = PackType::MashupPack;
        }

        let pack_name = manifest_name
            .as_deref()
            .map(clean_pack_name)
            .unwrap_or_else(|| cleaned_name.clone());

        packs.push(PackInfo {
            path: file_path.to_string_lossy().to_string(),
            name: pack_name,
            pack_type,
            uuid,
            version,
            extracted: false,
            icon_base64: icon,
            subfolder: Some(subfolder.clone()),
            nested_mcpack: None,
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
            nested_mcpack: None,
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

/// Finds top-level archive entries that are themselves `.mcpack` zip files (rather than
/// exploded folders containing a `manifest.json`). Some `.mcaddon` creators just zip up
/// their existing `.mcpack` files without extracting them first.
fn detect_nested_mcpack_entries<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Vec<String> {
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let Ok(file) = archive.by_index(i) else {
            continue;
        };
        let name = file.name();
        if name.to_lowercase().ends_with(".mcpack") && !name.contains('/') {
            entries.push(name.to_string());
        }
    }
    entries
}

/// `(pack_type, uuid, version, name, icon_base64)` extracted from a nested `.mcpack` entry.
type NestedPackInfo = (
    PackType,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Reads a nested `.mcpack` entry's raw bytes into memory and inspects it as its own
/// self-contained zip archive to determine pack type, uuid, version, name, and icon.
fn read_nested_mcpack_info<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    entry_name: &str,
) -> Option<NestedPackInfo> {
    let mut bytes = Vec::new();
    archive
        .by_name(entry_name)
        .ok()?
        .read_to_end(&mut bytes)
        .ok()?;
    let mut nested = ZipArchive::new(Cursor::new(bytes)).ok()?;

    let mut manifest_content = String::new();
    nested
        .by_name("manifest.json")
        .ok()?
        .read_to_string(&mut manifest_content)
        .ok()?;
    let json: Value = serde_json::from_str(&manifest_content).ok()?;

    let mut pack_type = determine_pack_type(&json);
    let uuid = extract_uuid(&json);
    let version = extract_version(&json);
    let raw_name = json
        .get("header")
        .and_then(|h| h.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let name =
        raw_name.map(|raw| resolve_localized_pack_name(&mut nested, "", &raw).unwrap_or(raw));

    if pack_type == PackType::Unknown {
        let entry_lower = entry_name.to_lowercase();
        if entry_lower.contains("behavior")
            || entry_lower.contains("behaviour")
            || entry_lower.ends_with("[bp].mcpack")
            || entry_lower.ends_with("(bp).mcpack")
            || entry_lower.ends_with(" bp.mcpack")
        {
            pack_type = PackType::BehaviorPack;
        } else if entry_lower.contains("resource")
            || entry_lower.ends_with("[rp].mcpack")
            || entry_lower.ends_with("(rp).mcpack")
            || entry_lower.ends_with(" rp.mcpack")
        {
            pack_type = PackType::ResourcePack;
        }
    }

    let icon = extract_icon_from_archive(&mut nested, "");

    Some((pack_type, uuid, version, name, icon))
}

fn process_nested_mcpack_archive<R: Read + Seek>(
    file_path: &Path,
    archive: &mut ZipArchive<R>,
    entries: &[String],
) -> Vec<PackInfo> {
    let mut packs = Vec::new();
    let base_filename = file_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let cleaned_name = clean_pack_name(&base_filename);
    let is_mashup = is_mashup_name(&base_filename);

    for entry_name in entries {
        let Some((mut pack_type, uuid, version, manifest_name, icon)) =
            read_nested_mcpack_info(archive, entry_name)
        else {
            continue;
        };

        if is_mashup {
            pack_type = PackType::MashupPack;
        }

        let entry_stem = Path::new(entry_name)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or(entry_name);
        let pack_name = manifest_name
            .as_deref()
            .map(clean_pack_name)
            .unwrap_or_else(|| clean_pack_name(entry_stem));

        packs.push(PackInfo {
            path: file_path.to_string_lossy().to_string(),
            name: pack_name,
            pack_type,
            uuid,
            version,
            extracted: false,
            icon_base64: icon,
            subfolder: None,
            nested_mcpack: Some(entry_name.clone()),
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
            nested_mcpack: None,
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

/// Strips Bedrock's `\u00A7`-prefixed color/formatting codes (e.g. `\u00A7b`, `\u00A77`),
/// which manifest authors sometimes bake directly into `header.name`.
fn strip_minecraft_formatting_codes(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c == '\u{00A7}' {
            chars.next();
        } else {
            result.push(c);
        }
    }
    result
}

fn clean_pack_name(name: &str) -> String {
    let mut cleaned = strip_minecraft_formatting_codes(name).trim().to_string();

    let suffixes = [
        " (addon)",
        "(addon)",
        " [addon]",
        "[addon]",
        " (behavior)",
        "(behavior)",
        " [behavior]",
        "[behavior]",
        " (resource)",
        "(resource)",
        " [resource]",
        "[resource]",
        " (resources)",
        "(resources)",
        " [resources]",
        "[resources]",
        " (bp)",
        "(bp)",
        " [bp]",
        "[bp]",
        " (rp)",
        "(rp)",
        " [rp]",
        "[rp]",
        " (world_template)",
        "(world_template)",
        " [world_template]",
        "[world_template]",
        " (template)",
        "(template)",
        " [template]",
        "[template]",
        " (skin_pack)",
        "(skin_pack)",
        " [skin_pack]",
        "[skin_pack]",
        " (skin)",
        "(skin)",
        " [skin]",
        "[skin]",
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

/// Bedrock manifests often store `header.name` as a localization key (e.g. `"pack.name"`)
/// whose real display value lives in a `texts/*.lang` file alongside the manifest. Resolve
/// it if possible; otherwise return the raw manifest value unchanged.
fn resolve_localized_pack_name<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    subfolder: &str,
    raw_name: &str,
) -> Option<String> {
    let prefix = if subfolder.is_empty() {
        String::new()
    } else {
        format!("{}/", subfolder)
    };
    let prefix_lower = prefix.to_lowercase();

    let mut preferred: Option<usize> = None;
    let mut fallback: Option<usize> = None;

    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        let name_lower = entry.name().to_lowercase();
        let Some(relative) = name_lower.strip_prefix(&prefix_lower) else {
            continue;
        };
        if relative == "texts/en_us.lang" {
            preferred = Some(i);
            break;
        }
        if relative.starts_with("texts/") && relative.ends_with(".lang") && fallback.is_none() {
            fallback = Some(i);
        }
    }

    let index = preferred.or(fallback)?;
    let mut file = archive.by_index(index).ok()?;
    let mut content = String::new();
    file.read_to_string(&mut content).ok()?;
    drop(file);

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

fn get_pack_info_from_subfolder(
    archive: &mut ZipArchive<fs::File>,
    subfolder: &str,
) -> (PackType, Option<String>, Option<String>, Option<String>) {
    let manifest_path = format!("{}/manifest.json", subfolder);

    let manifest_content: Option<String> =
        archive.by_name(&manifest_path).ok().and_then(|mut file| {
            let mut content = String::new();
            file.read_to_string(&mut content).ok()?;
            Some(content)
        });

    if let Some(content) = manifest_content {
        {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                let pack_type = determine_pack_type(&json);
                let uuid = extract_uuid(&json);
                let version = extract_version(&json);
                let raw_name = json
                    .get("header")
                    .and_then(|h| h.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                let name = raw_name.map(|raw| {
                    resolve_localized_pack_name(archive, subfolder, &raw).unwrap_or(raw)
                });

                if pack_type == PackType::Unknown {
                    let subfolder_lower = subfolder.to_lowercase();
                    let fallback_type = if subfolder_lower.contains("behavior")
                        || subfolder_lower.contains("behaviour")
                        || subfolder_lower == "ppack0"
                        || subfolder_lower == "bpack0"
                        || subfolder_lower.ends_with("/ppack0")
                        || subfolder_lower.ends_with("/bpack0")
                        || subfolder_lower.ends_with(" bp")
                        || subfolder_lower.ends_with("[bp]")
                        || subfolder_lower.ends_with("(bp)")
                    {
                        PackType::BehaviorPack
                    } else if subfolder_lower.contains("resource")
                        || subfolder_lower == "ppack1"
                        || subfolder_lower == "bpack1"
                        || subfolder_lower.ends_with("/ppack1")
                        || subfolder_lower.ends_with("/bpack1")
                        || subfolder_lower.ends_with(" rp")
                        || subfolder_lower.ends_with("[rp]")
                        || subfolder_lower.ends_with("(rp)")
                    {
                        PackType::ResourcePack
                    } else {
                        pack_type
                    };
                    return (fallback_type, uuid, version, name);
                }

                return (pack_type, uuid, version, name);
            }
        }
    }

    let subfolder_lower = subfolder.to_lowercase();

    let pack_type = if subfolder_lower.contains("behavior")
        || subfolder_lower.contains("behaviour")
        || subfolder_lower == "ppack0"
        || subfolder_lower == "bpack0"
        || subfolder_lower.ends_with("/ppack0")
        || subfolder_lower.ends_with("/bpack0")
        || subfolder_lower.ends_with(" bp")
        || subfolder_lower.ends_with("[bp]")
        || subfolder_lower.ends_with("(bp)")
    {
        PackType::BehaviorPack
    } else if subfolder_lower.contains("resource")
        || subfolder_lower == "ppack1"
        || subfolder_lower == "bpack1"
        || subfolder_lower.ends_with("/ppack1")
        || subfolder_lower.ends_with("/bpack1")
        || subfolder_lower.ends_with(" rp")
        || subfolder_lower.ends_with("[rp]")
        || subfolder_lower.ends_with("(rp)")
    {
        PackType::ResourcePack
    } else {
        PackType::Unknown
    };

    (pack_type, None, None, None)
}

fn get_pack_info_from_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
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

fn extract_icon_from_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
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
            if file.size() > MAX_ARCHIVE_ICON_BYTES as u64 {
                continue;
            }
            let mut buffer = Vec::new();
            if file.read_to_end(&mut buffer).is_ok() {
                let mime = if icon_name.ends_with(".jpg") || icon_name.ends_with(".jpeg") {
                    "image/jpeg"
                } else {
                    "image/png"
                };
                return Some(format!(
                    "data:{};base64,{}",
                    mime,
                    general_purpose::STANDARD.encode(&buffer)
                ));
            }
        }
    }

    if subfolder.is_empty() {
        let mut found_index: Option<usize> = None;
        let mut found_is_jpeg = false;
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                let name = file.name().to_lowercase();
                if (name.ends_with("pack_icon.png")
                    || name.ends_with("world_icon.jpeg")
                    || name.ends_with("world_icon.jpg"))
                    && !name.contains('/')
                {
                    found_is_jpeg = name.ends_with(".jpeg") || name.ends_with(".jpg");
                    found_index = Some(i);
                    break;
                }
            }
        }

        if let Some(idx) = found_index {
            if let Ok(mut f) = archive.by_index(idx) {
                if f.size() > MAX_ARCHIVE_ICON_BYTES as u64 {
                    return None;
                }
                let mut buffer = Vec::new();
                if f.read_to_end(&mut buffer).is_ok() {
                    let mime = if found_is_jpeg {
                        "image/jpeg"
                    } else {
                        "image/png"
                    };
                    return Some(format!(
                        "data:{};base64,{}",
                        mime,
                        general_purpose::STANDARD.encode(&buffer)
                    ));
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
            } else {
                v.as_str().map(str::to_string)
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
                if cap.as_str() == Some("scriptEngineVersion") {
                    return PackType::BehaviorPack;
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

/// Marker trait so `extract_pack_to_destination` can read either the outer file directly,
/// or an in-memory nested `.mcpack` entry's bytes, through a single generic code path.
trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

/// The canonical trailing suffix appended to an output folder name for a given pack
/// type. Shared by `extract_pack_to_destination` (new installs) and
/// `suggest_clean_folder_name` (renaming legacy/manually-installed folders) so both
/// paths always produce identical, consistent naming.
pub(crate) fn canonical_type_suffix(pack_type: PackType) -> &'static str {
    match pack_type {
        PackType::BehaviorPack => " (ADDON)",
        PackType::ResourcePack => " (RESOURCE)",
        PackType::SkinPack => " (SKIN)",
        PackType::SkinPack4D => "",
        PackType::WorldTemplate => " (TEMPLATE)",
        PackType::MashupPack => " (MASHUP)",
        PackType::Unknown => "",
    }
}

static PPACK_TRAILING_SUFFIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\s*-\s*ppack\d+\s*$").unwrap());
static TAG_TRAILING_SUFFIX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\s*[\(\[](add-?on|resources?|behaviou?r|bp|rp|skin_pack|skins?|world_template|template|mash-?up)[\)\]]\s*$")
        .unwrap()
});

/// Repeatedly strips every recognized trailing decoration (parenthesized/bracketed
/// pack-type tags and `"- ppackN"` fragments, plus Minecraft `§` formatting codes) from
/// a folder name, then re-appends the single canonical suffix for `pack_type`. Used to
/// clean up folder names left behind by older Blocksmith versions or manual installs,
/// e.g. `"Dragons! Biomes (addon) - ppack1 (RESOURCE)"` -> `"Dragons! Biomes (RESOURCE)"`.
/// Returns the name unchanged if nothing recognizable was found to strip, so plain,
/// undecorated folder names are never force-tagged.
pub(crate) fn suggest_clean_folder_name(name: &str, pack_type: PackType) -> String {
    let trimmed_original = name.trim().to_string();
    let mut base = strip_minecraft_formatting_codes(&trimmed_original)
        .trim()
        .to_string();
    let mut changed = base != trimmed_original;

    loop {
        let before = base.clone();
        base = PPACK_TRAILING_SUFFIX.replace(&base, "").trim().to_string();
        base = TAG_TRAILING_SUFFIX.replace(&base, "").trim().to_string();
        if base == before {
            break;
        }
        changed = true;
    }

    if !changed || base.is_empty() {
        return trimmed_original;
    }

    format!("{}{}", base, canonical_type_suffix(pack_type))
}

/// Opens the byte source to extract from: the file itself normally, or — when `nested_mcpack`
/// names a `.mcpack` entry inside a `.mcaddon` — that entry's bytes read fully into memory
/// and exposed as its own seekable source.
fn open_extract_source(
    file_path: &Path,
    nested_mcpack: Option<&str>,
) -> Result<Box<dyn ReadSeek>, String> {
    let file = fs::File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let Some(entry_name) = nested_mcpack else {
        return Ok(Box::new(std::io::BufReader::new(file)));
    };

    let mut outer = ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("Failed to read archive: {}", e))?;
    let mut bytes = Vec::new();
    outer
        .by_name(entry_name)
        .map_err(|e| format!("Failed to read nested pack '{}': {}", entry_name, e))?
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read nested pack '{}': {}", entry_name, e))?;
    Ok(Box::new(Cursor::new(bytes)))
}

pub fn extract_pack_to_destination(
    file_path: &Path,
    destination_dir: &Path,
    pack_type: PackType,
    subfolder: Option<&str>,
    nested_mcpack: Option<&str>,
    output_name_override: Option<&str>,
) -> Result<String, String> {
    let type_suffix = canonical_type_suffix(pack_type);

    let output_name = if let Some(name) = output_name_override {
        name.to_string()
    } else if let Some(entry_name) = nested_mcpack {
        let filename = Path::new(entry_name)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");
        format!("{}{}", filename, type_suffix)
    } else {
        let filename = file_path
            .file_stem()
            .ok_or("Invalid filename")?
            .to_str()
            .ok_or("Filename is not valid UTF-8")?;
        format!("{}{}", filename, type_suffix)
    };
    let output_name = sanitize_filename_component(&output_name);

    let output_name = validated_relative_path(&output_name, "output folder name")?;
    let output_path = destination_dir.join(&output_name);
    let staging_path = destination_dir.join(format!(
        ".{}.staging-{}",
        output_name.display(),
        uuid::Uuid::new_v4()
    ));

    let mut archive = ZipArchive::new(open_extract_source(file_path, nested_mcpack)?)
        .map_err(|e| format!("Failed to read archive: {}", e))?;

    let file_count = archive.len();
    if file_count > MAX_ARCHIVE_ENTRIES {
        return Err(format!(
            "Archive has too many entries: {} (maximum {})",
            file_count, MAX_ARCHIVE_ENTRIES
        ));
    }
    let total_uncompressed_size = (0..file_count).try_fold(0u64, |total, index| {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        if entry.size() > MAX_ARCHIVE_ENTRY_BYTES {
            return Err(format!("Archive entry is too large: {}", entry.name()));
        }
        total
            .checked_add(entry.size())
            .ok_or_else(|| "Archive size overflow".to_string())
    })?;
    if total_uncompressed_size > MAX_ARCHIVE_UNCOMPRESSED_BYTES {
        return Err(format!(
            "Archive expands to {} bytes, exceeding the {} byte limit",
            total_uncompressed_size, MAX_ARCHIVE_UNCOMPRESSED_BYTES
        ));
    }

    fs::create_dir_all(destination_dir)
        .map_err(|e| format!("Failed to create destination directory: {}", e))?;
    fs::create_dir(&staging_path)
        .map_err(|e| format!("Failed to create staging directory: {}", e))?;

    let extraction_result = (|| -> Result<(), String> {
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

            let outpath = staging_path.join(validated_archive_path(relative_path)?);

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

        let mut archive = ZipArchive::new(open_extract_source(file_path, nested_mcpack)?)
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

        Ok(())
    })();

    if let Err(error) = extraction_result {
        let _ = fs::remove_dir_all(&staging_path);
        return Err(error);
    }

    let backup_path = destination_dir.join(format!(
        ".{}.backup-{}",
        output_name.display(),
        uuid::Uuid::new_v4()
    ));
    let had_existing_destination = output_path.exists();
    if had_existing_destination {
        fs::rename(&output_path, &backup_path)
            .map_err(|e| format!("Failed to prepare existing pack for replacement: {}", e))?;
    }

    if let Err(error) = fs::rename(&staging_path, &output_path) {
        if had_existing_destination {
            let _ = fs::rename(&backup_path, &output_path);
        }
        let _ = fs::remove_dir_all(&staging_path);
        return Err(format!("Failed to finalize extracted pack: {}", error));
    }

    if had_existing_destination {
        fs::remove_dir_all(&backup_path)
            .map_err(|e| format!("Failed to remove replaced pack backup: {}", e))?;
    }

    Ok(output_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        clean_pack_name, detect_nested_mcpack_entries, process_nested_mcpack_archive,
        sanitize_filename_component, suggest_clean_folder_name, validated_archive_path,
        validated_relative_path, PackType,
    };
    use std::io::{Cursor, Write};
    use std::path::Path;
    use zip::write::SimpleFileOptions;
    use zip::{ZipArchive, ZipWriter};

    fn build_test_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut writer = ZipWriter::new(cursor);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for (name, data) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buf
    }

    #[test]
    fn allows_nested_normal_archive_paths() {
        assert!(validated_archive_path("textures/items/stone.png").is_ok());
    }

    #[test]
    fn rejects_archive_traversal_and_absolute_paths() {
        for value in [
            "../outside.txt",
            "/outside.txt",
            "C:\\outside.txt",
            "\\\\server\\share\\outside.txt",
        ] {
            assert!(
                validated_archive_path(value).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_unsafe_output_folder_names() {
        for value in [
            "../outside",
            "nested/name",
            "C:\\outside",
            "\\\\server\\share",
        ] {
            assert!(
                validated_relative_path(value, "output folder name").is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn sanitize_replaces_windows_forbidden_characters() {
        assert_eq!(sanitize_filename_component("a:b"), "a_b");
        assert_eq!(sanitize_filename_component("a\"b"), "a_b");
        assert_eq!(sanitize_filename_component("a<b>c"), "a_b_c");
        assert_eq!(sanitize_filename_component("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_filename_component("a|b"), "a_b");
        assert_eq!(sanitize_filename_component("a?b"), "a_b");
        assert_eq!(sanitize_filename_component("a*b"), "a_b");
    }

    #[test]
    fn sanitize_strips_trailing_dots_and_spaces() {
        assert_eq!(sanitize_filename_component("Pack Name..  "), "Pack Name");
        assert_eq!(sanitize_filename_component("Pack Name"), "Pack Name");
    }

    #[test]
    fn sanitize_handles_reserved_device_names() {
        assert_eq!(sanitize_filename_component("CON"), "CON_");
        assert_eq!(sanitize_filename_component("con"), "con_");
        assert_eq!(sanitize_filename_component("Contains"), "Contains");
    }

    #[test]
    fn sanitize_replaces_control_characters_and_handles_empty_result() {
        assert_eq!(sanitize_filename_component("a\tb\nc"), "a_b_c");
        assert_eq!(sanitize_filename_component("..."), "Unnamed Pack");
        assert_eq!(sanitize_filename_component("   "), "Unnamed Pack");
    }

    #[test]
    fn clean_pack_name_strips_bracket_suffixes_and_formatting_codes() {
        assert_eq!(
            clean_pack_name("\u{00A7}bFeather FPS Boost V9 \u{00A7}7[BP]"),
            "Feather FPS Boost V9"
        );
        assert_eq!(
            clean_pack_name("\u{00A7}bFeather FPS Boost V9 \u{00A7}7[RP]"),
            "Feather FPS Boost V9"
        );
        assert_eq!(clean_pack_name("My Pack [Addon]"), "My Pack");
        assert_eq!(clean_pack_name("My Pack (BP)"), "My Pack");
    }

    fn sample_manifest(name: &str) -> Vec<u8> {
        format!(
            r#"{{"header":{{"name":"{name}","uuid":"11111111-1111-1111-1111-111111111111","version":[1,0,0]}},"modules":[{{"type":"data"}}]}}"#
        )
        .into_bytes()
    }

    #[test]
    fn detects_nested_mcpack_entries_at_archive_root() {
        let bp_manifest = sample_manifest("Feather FPS Boost");
        let nested_bp = build_test_zip(&[("manifest.json", &bp_manifest)]);
        let rp_manifest = {
            let mut m = serde_json::from_slice::<serde_json::Value>(&sample_manifest(
                "Feather FPS Boost RP",
            ))
            .unwrap();
            m["modules"][0]["type"] = serde_json::json!("resources");
            serde_json::to_vec(&m).unwrap()
        };
        let nested_rp = build_test_zip(&[("manifest.json", &rp_manifest)]);

        let outer = build_test_zip(&[
            ("Feather FPS Boost Mod [BP].mcpack", &nested_bp),
            ("Feather FPS Boost Mod [RP].mcpack", &nested_rp),
            ("some_other_file.txt", b"not a pack"),
        ]);

        let mut archive = ZipArchive::new(Cursor::new(outer)).unwrap();
        let mut entries = detect_nested_mcpack_entries(&mut archive);
        entries.sort();

        assert_eq!(
            entries,
            vec![
                "Feather FPS Boost Mod [BP].mcpack".to_string(),
                "Feather FPS Boost Mod [RP].mcpack".to_string(),
            ]
        );
    }

    #[test]
    fn processes_nested_mcpack_archive_into_pack_infos() {
        let bp_manifest = sample_manifest("Feather FPS Boost");
        let nested_bp = build_test_zip(&[("manifest.json", &bp_manifest)]);
        let outer = build_test_zip(&[("Feather FPS Boost Mod [BP].mcpack", &nested_bp)]);

        let mut archive = ZipArchive::new(Cursor::new(outer)).unwrap();
        let entries = detect_nested_mcpack_entries(&mut archive);
        assert_eq!(entries.len(), 1);

        let packs = process_nested_mcpack_archive(
            Path::new("Feather FPS Boost Mod.mcaddon"),
            &mut archive,
            &entries,
        );

        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].pack_type, PackType::BehaviorPack);
        assert_eq!(
            packs[0].nested_mcpack.as_deref(),
            Some("Feather FPS Boost Mod [BP].mcpack")
        );
        assert_eq!(packs[0].name, "Feather FPS Boost");
    }

    #[test]
    fn suggest_clean_folder_name_strips_stacked_legacy_suffixes() {
        assert_eq!(
            suggest_clean_folder_name(
                "Dragons! Biomes (addon) - ppack1 (RESOURCE)",
                PackType::ResourcePack
            ),
            "Dragons! Biomes (RESOURCE)"
        );
        assert_eq!(
            suggest_clean_folder_name(
                "Dungeons and Bosses Add-on (addon) (RP)",
                PackType::ResourcePack
            ),
            "Dungeons and Bosses Add-on (RESOURCE)"
        );
    }

    #[test]
    fn suggest_clean_folder_name_leaves_plain_names_untouched() {
        assert_eq!(
            suggest_clean_folder_name("My Cool Pack", PackType::BehaviorPack),
            "My Cool Pack"
        );
        assert_eq!(
            suggest_clean_folder_name("Already Clean (ADDON)", PackType::BehaviorPack),
            "Already Clean (ADDON)"
        );
    }
}
