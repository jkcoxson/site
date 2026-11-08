// Jackson Coxson
//
// Private pages live in src/private/, a separate (private) git repo that is
// gitignored here. When it's checked out we compile it in; when it isn't
// (a fresh public clone) lib.rs falls back to an empty stub.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(private_pages)");
    println!("cargo::rerun-if-changed=src");
    if std::path::Path::new("src/private/mod.rs").exists() {
        println!("cargo::rustc-cfg=private_pages");
    }
}
