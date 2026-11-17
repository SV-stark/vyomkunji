fn main() {
    #[cfg(target_os = "windows")]
    {
        let ico = std::path::Path::new("../assets/app.ico");
        if ico.exists() {
            println!("cargo:rerun-if-changed=../assets/app.ico");
            let mut res = winresource::WindowsResource::new();
            res.set_icon(ico.to_str().expect("valid UTF-8 icon path"));
            if let Err(e) = res.compile() {
                println!("cargo:warning=winresource failed: {e}");
            }
        }
    }
}
