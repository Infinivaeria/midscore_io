fn main() {
    println!("cargo:rerun-if-env-changed=RUBY");
    // println!("cargo:rustc-link-lib=dylib=x64-ucrt-ruby340");
    // println!("cargo:rustc-link-search=C:/Ruby34-x64/lib");
}
