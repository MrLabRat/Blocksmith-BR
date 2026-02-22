fn main() {
    // Copy custom icon files to output directory
    let icons = [
        "blackredborder.png",
        "blackrednoborder.png",
        "defaultborder.png",
        "defaultnoborder.png",
    ];

    let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();

    // Try to copy to debug/release icons folder
    for icon in &icons {
        let src = std::path::Path::new(&manifest_dir).join("icons").join(icon);

        // Try multiple possible output locations
        let dest_locations = vec![std::path::Path::new(&out_dir).join("icons").join(icon)];

        for dest in dest_locations {
            if src.exists() {
                let _ = std::fs::create_dir_all(dest.parent().unwrap());
                let _ = std::fs::copy(&src, &dest);
            }
        }
    }

    tauri_build::build()
}
