fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");

        let version = env!("CARGO_PKG_VERSION");
        res.set("CompanyName", "CS Inc.");
        res.set("ProductName", "CSClient");
        res.set("FileDescription", "CSClient");
        res.set("LegalCopyright", "© 2026 CS Inc.");
        res.set("OriginalFilename", "csclient_app.exe");
        res.set("InternalName", "csclient_app");
        res.set("FileVersion", version);
        res.set("ProductVersion", version);

        res.compile()
            .expect("failed to embed Windows icon resource");
    }
}
