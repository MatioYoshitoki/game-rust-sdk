//! Historical regression data supplied outside the SDK checkout.
macro_rules! fixture {
    ($relative:literal) => {{
        static DATA: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        DATA.get_or_init(|| {
            let path = std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join($relative);
            std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("load regression input {}: {error}", path.display()))
        })
        .as_str()
    }};
}
pub(crate) use fixture;
