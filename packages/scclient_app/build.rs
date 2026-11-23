fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");

        let version = env!("CARGO_PKG_VERSION");
        res.set("CompanyName", "SC Inc.");
        res.set("ProductName", "SCClient");
        res.set("FileDescription", "SCClient");
        res.set("LegalCopyright", "© 2026 SC Inc.");
        res.set("OriginalFilename", "scclient_app.exe");
        res.set("InternalName", "scclient_app");
        res.set("FileVersion", version);
        res.set("ProductVersion", version);

        res.compile()
            .expect("failed to embed Windows icon resource");
    }
}
