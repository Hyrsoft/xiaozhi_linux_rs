fn main() {
    println!("cargo:rerun-if-env-changed=SPEEXDSP_STATIC");

    let statik = std::env::var_os("SPEEXDSP_STATIC").is_some();
    pkg_config::Config::new()
        .statik(statik)
        .probe("speexdsp")
        .expect("SpeexDSP must be provided by the host or target SDK");
}
