fn main() {
    // Try to find IPOPT and its dependencies using pkg-config, which is the
    // standard, platform-agnostic way.
    if pkg_config::Config::new().probe("ipopt").is_ok() {
        // If pkg-config succeeds, it will automatically emit the correct
        // linker flags (`-L` and `-l`), and we don't need to do anything else.
    } else {
        // If pkg-config fails (e.g., in a minimal Docker container), we
        // fall back to manually specifying the linker flags.
        println!("cargo:warning=pkg-config failed to find ipopt. Using manual fallback linking.");

        if cfg!(target_os = "macos") {
            // Fallback for macOS with Homebrew.
            println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
            println!("cargo:rustc-link-lib=ipopt");

        } else if cfg!(target_os = "linux") {
            // Fallback for Linux, especially for the `manylinux` container where
            // yum installs libraries to /usr/lib64.
            println!("cargo:rustc-link-search=native=/usr/lib64");
            println!("cargo:rustc-link-lib=ipopt");
            // IPOPT's dependencies also need to be specified manually here.
            println!("cargo:rustc-link-lib=lapack");
            println!("cargo:rustc-link-lib=blas");

        } else {
            // For other OSes, just link by name and hope it's in a default path.
            println!("cargo:rustc-link-lib=ipopt");
        }
    }

    // IPOPT is a C++ library, so it always needs to be linked against the
    // C++ standard library.
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
    }
}