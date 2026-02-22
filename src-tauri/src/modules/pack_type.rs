use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackType {
    BehaviorPack,
    ResourcePack,
    SkinPack,
    SkinPack4D,
    WorldTemplate,
    MashupPack,
    Unknown,
}

impl std::fmt::Display for PackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackType::BehaviorPack => write!(f, "Behavior Pack (Addon)"),
            PackType::ResourcePack => write!(f, "Resource Pack"),
            PackType::SkinPack => write!(f, "Skin Pack"),
            PackType::SkinPack4D => write!(f, "Skin Pack (4D Geometry)"),
            PackType::WorldTemplate => write!(f, "World Template"),
            PackType::MashupPack => write!(f, "Mash-Up Pack"),
            PackType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackInfo {
    pub path: String,
    pub name: String,
    pub pack_type: PackType,
    pub uuid: Option<String>,
    pub version: Option<String>,
    pub extracted: bool,
    pub icon_base64: Option<String>,
    pub subfolder: Option<String>,
    pub folder_size: Option<u64>,
    pub folder_size_formatted: Option<String>,
    pub needs_attention: Option<bool>,
    pub attention_message: Option<String>,
    pub is_installed: Option<bool>,
    pub is_update: Option<bool>,
    pub installed_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub behavior_pack_path: Option<String>,
    pub resource_pack_path: Option<String>,
    pub skin_pack_path: Option<String>,
    pub skin_pack_4d_path: Option<String>,
    pub world_template_path: Option<String>,
    pub scan_location: Option<String>,
    pub dry_run: bool,
    pub delete_source: bool,
    pub disable_animations: Option<bool>,
    pub animation_speed_ms: Option<u32>,
    pub ui_scale: Option<u32>,
    pub taskbar_icon_style: Option<String>,
    pub taskbar_icon_border: Option<bool>,
    pub app_icon_style: Option<String>,
    pub app_icon_border: Option<bool>,
    pub debug_mode: Option<bool>,
    pub disable_tip_notifications: Option<bool>,
    pub theme: Option<String>,
    pub background_style: Option<String>,
    pub background_smoke: Option<u32>,
    pub background_blobs: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            behavior_pack_path: None,
            resource_pack_path: None,
            skin_pack_path: None,
            skin_pack_4d_path: None,
            world_template_path: None,
            scan_location: None,
            dry_run: false,
            delete_source: false,
            disable_animations: Some(false),
            animation_speed_ms: Some(300),
            ui_scale: Some(100),
            taskbar_icon_style: Some("default".to_string()),
            taskbar_icon_border: Some(false),
            app_icon_style: Some("default".to_string()),
            app_icon_border: Some(false),
            debug_mode: Some(false),
            disable_tip_notifications: Some(false),
            theme: Some("darkred".to_string()),
            background_style: Some("embers".to_string()),
            background_smoke: Some(5),
            background_blobs: Some(5),
        }
    }
}
