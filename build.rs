fn main() {
    // Re-run if the icon changes
    println!("cargo:rerun-if-changed=assets/icon.png");

    #[cfg(target_os = "windows")]
    generate_windows_resources();
}

#[cfg(target_os = "windows")]
fn generate_windows_resources() {
    use std::io::BufWriter;
    use std::path::PathBuf;

    // Decode the PNG source icon
    let img = image::open("assets/icon.png")
        .expect("failed to open assets/icon.png")
        .into_rgba8();

    // Write an ICO file containing multiple sizes into OUT_DIR
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico_path = out_dir.join("icon.ico");

    let ico_file = std::fs::File::create(&ico_path).expect("failed to create icon.ico");
    let writer = BufWriter::new(ico_file);

    // Build an ICO with common Windows icon sizes
    let sizes: &[u32] = &[256, 128, 64, 48, 32, 16];
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for &size in sizes {
        let resized =
            image::imageops::resize(&img, size, size, image::imageops::FilterType::Lanczos3);
        let ico_image = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        icon_dir.add_entry(ico::IconDirEntry::encode(&ico_image).expect("failed to encode icon"));
    }

    icon_dir.write(writer).expect("failed to write icon.ico");

    // Embed the generated ICO into the exe via a Windows resource
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().unwrap());
    res.compile().expect("failed to compile Windows resources");
}
