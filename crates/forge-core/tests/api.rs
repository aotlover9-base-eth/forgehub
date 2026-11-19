//! Integration tests over the public API, including the Forgenesis
//! fixtures and fuzz-style garbage inputs (the parser must never panic).

use forge_core::{import_forgenesis, Snippet};

#[test]
fn fixture_forgenesis_full_imports() {
    let text = include_str!("fixtures/forgenesis_full.yaml");
    let out = import_forgenesis(text).unwrap();
    assert_eq!(out.snippet.name, "deploy-api");
    assert_eq!(out.snippet.resolved_steps().len(), 3);
    assert_eq!(out.snippet.always_steps().len(), 1);
    assert_eq!(out.notes.len(), 0, "{:?}", out.notes);
    assert_eq!(
        out.snippet.referenced_variables(),
        vec!["BRANCH", "RESTART"]
    );
}

#[test]
fn fixture_forgenesis_legacy_drops_extras() {
    let text = include_str!("fixtures/forgenesis_legacy.yaml");
    let out = import_forgenesis(text).unwrap();
    assert_eq!(out.snippet.name, "legacysetup");
    assert_eq!(out.snippet.command_count(), 2);
    // packs, sync, plugin dropped + name lowercased = 4 notes.
    assert_eq!(out.notes.len(), 4, "{:?}", out.notes);
}

#[test]
fn public_api_end_to_end() {
    let s =
        Snippet::from_yaml("name: hello\ntags: [Demo]\nsteps:\n  - echo hello from {{DIRNAME}}\n")
            .unwrap();
    assert_eq!(s.tags, vec!["demo"]);
    let values = forge_core::vars::resolve(&s, &std::collections::BTreeMap::new()).unwrap();
    let rendered = forge_core::vars::substitute("echo hello from {{DIRNAME}}", &values).unwrap();
    assert!(rendered.starts_with("echo hello "));
    assert_eq!(forge_core::hash::content_hash(&s).len(), 64);
}

#[test]
fn garbage_never_panics() {
    let inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0u8; 1024],
        b"\xff\xfe\xfd".to_vec(),
        b"name: x\nsteps: [".to_vec(),
        "name: x\nsteps:\n - {\n".as_bytes().to_vec(),
        "😀".repeat(500).into_bytes(),
        b"a".repeat(70_000),
        b"name: x\nscript: |\n  ".to_vec(),
        b"\x00\x01\x02".to_vec(),
        b"name: x\nsteps:\n  - &a x\n  - *a\n".to_vec(),
    ];
    for input in inputs {
        // Returning Err is fine; panicking is not.
        let _ = Snippet::from_bytes(&input);
        let text = String::from_utf8_lossy(&input).into_owned();
        let _ = import_forgenesis(&text);
        let _ = forge_core::scan::scan(&text);
        let _ = forge_core::lint::lint(&text);
    }
}

#[test]
fn deeply_repeated_input_is_bounded() {
    // Alias-bomb-shaped input: anchors are rejected before expansion.
    let bomb = "name: x\nsteps: &a [echo]\ndescription: *a\n";
    assert!(Snippet::from_yaml(bomb).is_err());
}
