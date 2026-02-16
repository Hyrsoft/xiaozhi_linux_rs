fn main() {
    // Link libspeexdsp via pkg-config
    pkg_config::Config::new()
        .probe("speexdsp")
        .expect("Failed to find speexdsp. Please install libspeexdsp-dev.");
}
