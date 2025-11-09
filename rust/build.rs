fn main() {
    // Use pkg-config to find IPOPT. This is the platform-agnostic way.
    // It will automatically emit the correct `cargo:rustc-link-search` and
    // `cargo:rustc-link-lib` flags. This works as long as the -dev
    // package (e.g., coinor-libipopt-dev on Debian/Ubuntu) is installed.
    if pkg_config::Config::new().probe("ipopt").is_err() {
        // Fallback for systems where pkg-config might not be perfectly set up,
        // like older macOS with Homebrew.
        if cfg!(target_os = "macos") {
            println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
        }
        // If pkg-config fails, we still need to specify the library name manually.
        println!("cargo:rustc-link-lib=ipopt");
    }

    // IPOPT is a C++ library, so it depends on the C++ standard library.
    // This part remains platform-specific.
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
    } else {
        // On Linux, it's typically libstdc++.
        println!("cargo:rustc-link-lib=stdc++");
    }
}