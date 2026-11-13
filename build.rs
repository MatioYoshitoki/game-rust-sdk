fn main() {
    println!("cargo::rustc-check-cfg=cfg(match_sdk_test_data)");
    println!("cargo::rerun-if-env-changed=MATCH_SDK_TEST_DATA_DIR");
    if let Some(path) = std::env::var_os("MATCH_SDK_TEST_DATA_DIR") {
        let path = std::fs::canonicalize(path).expect("resolve MATCH_SDK_TEST_DATA_DIR");
        assert!(
            path.is_dir(),
            "MATCH_SDK_TEST_DATA_DIR must name a directory"
        );
        println!(
            "cargo::rustc-env=MATCH_SDK_TEST_DATA_DIR={}",
            path.display()
        );
        println!("cargo::rustc-cfg=match_sdk_test_data");
    }
}
