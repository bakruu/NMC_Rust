fn main() {
    println!("cargo:rustc-link-search=native=C:\\WpdPack\\Lib\\x64");
    println!("cargo:rustc-link-lib=static=wpcap");
    println!("cargo:rustc-link-lib=static=packet");
} 