fn main() {
    // DJ Sugar: the release this copy was taken from, said outright. Nothing is
    // read from git or the clock, so the same source always builds the same.
    println!("cargo:rustc-env=VERGEN_GIT_SHA=v0.8.0");
    println!("cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=2025-11-10");
    println!("cargo:rustc-env=VERGEN_BUILD_DATE=2025-11-10");
    println!("cargo:rustc-env=LIBRESPOT_BUILD_ID=djsugar0");
}
