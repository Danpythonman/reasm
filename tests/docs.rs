//! Keeps docs/CONFIGURATION.md and the code in sync (CFG-6).

use reasm::Config;

fn default_file_block() -> String {
    let doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/CONFIGURATION.md"
    ))
    .expect("read CONFIGURATION.md");
    let section = doc
        .split("## Full default file")
        .nth(1)
        .expect("'Full default file' section");
    let start = section.find("```toml\n").expect("toml block") + "```toml\n".len();
    let len = section[start..].find("```").expect("end of toml block");
    section[start..start + len].to_string()
}

#[test]
fn documented_default_file_matches_print_config_exactly() {
    let documented = default_file_block();
    let printed = Config::default().to_toml();
    assert_eq!(
        documented, printed,
        "docs/CONFIGURATION.md 'Full default file' differs from `reasm --print-config default`"
    );
}

#[test]
fn every_key_has_a_reference_row() {
    let doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/CONFIGURATION.md"
    ))
    .unwrap();
    let defaults = toml::Table::try_from(Config::default()).unwrap();
    for (key, value) in &defaults {
        match value {
            toml::Value::Table(t) => {
                assert!(
                    doc.contains(&format!("## `[{key}]`")),
                    "no section for [{key}]"
                );
                for sub in t.keys() {
                    assert!(
                        doc.contains(&format!("| `{sub}` |")),
                        "no row for {key}.{sub}"
                    );
                }
            }
            _ => assert!(doc.contains(&format!("| `{key}` |")), "no row for {key}"),
        }
    }
}
