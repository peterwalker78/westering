//! What the README promises about the Flatpak, checked against its manifest.

#[test]
fn the_flatpak_has_no_network_permission() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../flatpak/io.github.peterwalker78.Westering.yml"
    );
    let manifest = std::fs::read_to_string(path).expect("the manifest is there");
    assert!(manifest.contains("finish-args:"));
    assert!(!manifest.contains("--share=network"));
}
