fn main() {
    // This build script tells cargo how to link to the IPOPT library.
    // It assumes IPOPT is installed via Homebrew on Apple Silicon. A more
    // robust script for cross-platform support would check multiple paths
    // or use environment variables.
    println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
    println!("cargo:rustc-link-lib=ipopt");

    // IPOPT is a C++ library, so it depends on the C++ standard library.
    // On macOS, this is typically handled by linking against libc++.
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
    } else {
        // On Linux, it's typically libstdc++.
        println!("cargo:rustc-link-lib=stdc++");
    }
}