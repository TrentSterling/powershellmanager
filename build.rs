fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/tront-icon.ico");
        res.set("ProductName", "PowerShell Manager");
        res.set("FileDescription", "Terminal window grid arranger");
        res.set("CompanyName", "Trent Sterling");
        if let Err(e) = res.compile() {
            eprintln!("winresource: {}", e);
        }
    }
}
