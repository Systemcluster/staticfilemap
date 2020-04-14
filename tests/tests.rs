#[test]
fn simple() {
    use staticfilemap::*;

    #[derive(StaticFileMap)]
    #[names = "readme;license"]
    #[files = "README.md;LICENSE"]
    struct StaticMap;

    let content = StaticMap::get("license").unwrap();
    let string = String::from_utf8_lossy(&content[..10]);
    assert!(&string == "Copyright ");
}

#[test]
fn get_match() {
    use staticfilemap::*;

    #[derive(StaticFileMap)]
    #[names = "readme;license"]
    #[files = "README.md;LICENSE"]
    struct StaticMap;

    let content = StaticMap::get_match("lic").unwrap();
    let string = String::from_utf8_lossy(&content[..10]);
    assert!(&string == "Copyright ");
}

#[test]
fn compression() {
    use minilz4::Decoder;
    use staticfilemap::*;
    use std::io::Read;

    #[derive(StaticFileMap)]
    #[names = "readme;license"]
    #[files = "README.md;LICENSE"]
    #[compression = 1]
    struct StaticMap;

    let compressed = StaticMap::get("license").unwrap();

    let mut content = Vec::new();
    let mut decoder = Decoder::new(compressed).unwrap();
    decoder.read_to_end(&mut content).unwrap();

    let string = String::from_utf8_lossy(&content[..10]);
    assert!(&string == "Copyright ");
}

#[test]
fn parse_env() {
    use staticfilemap::*;

    #[derive(StaticFileMap)]
    #[parse = "env"]
    #[names = "CARGO_PKG_NAME"]
    #[files = "CARGO"]
    struct StaticMap;

    let _ = StaticMap::get("staticfilemap").unwrap();
}

#[test]
fn keys() {
    use staticfilemap::*;

    #[derive(StaticFileMap)]
    #[names = "readme;license"]
    #[files = "README.md;LICENSE"]
    struct StaticMap;

    let keys = StaticMap::keys();
    assert_eq!(keys, &["readme", "license"])
}

#[test]
fn types() {
    use staticfilemap::*;

    #[derive(StaticFileMap)]
    #[names = "readme;license"]
    #[files = "README.md;LICENSE"]
    struct StaticMap;

    let _: &[u8] = StaticMap::get("readme").unwrap();
    let _: &[&str] = StaticMap::keys();
}
