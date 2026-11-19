//! Property tests: hashing determinism and semantic invariance
//! (spec §9), parser robustness on arbitrary text.

use forge_core::hash::content_hash;
use forge_core::Snippet;
use proptest::prelude::*;

fn arb_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,10}".prop_filter("reserved words are not valid names", |n| {
        !forge_core::names::is_reserved(n)
    })
}

proptest! {
    #[test]
    fn hash_is_deterministic(name in arb_name(), cmd in "[a-z][a-z ]{0,29}") {
        let yaml = format!("name: {name}\nsteps: ['{cmd}']\n");
        let a = Snippet::from_yaml(&yaml).unwrap();
        let b = Snippet::from_yaml(&yaml).unwrap();
        prop_assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn key_order_never_changes_hash(
        name in arb_name(),
        desc in "[a-z][a-z ]{0,19}",
        cmd in "[a-z][a-z ]{0,29}",
    ) {
        let a = Snippet::from_yaml(&format!(
            "name: {name}\ndescription: {desc}\nsteps: ['{cmd}']\n"
        ))
        .unwrap();
        let b = Snippet::from_yaml(&format!(
            "steps: ['{cmd}']\ndescription: {desc}\nname: {name}\n"
        ))
        .unwrap();
        prop_assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn changing_the_command_changes_the_hash(
        name in arb_name(),
        cmd_a in "[a-z]{2,20}",
        cmd_b in "[a-z]{2,20}",
    ) {
        prop_assume!(cmd_a != cmd_b);
        let a = Snippet::from_yaml(&format!("name: {name}\nsteps: ['{cmd_a}']\n")).unwrap();
        let b = Snippet::from_yaml(&format!("name: {name}\nsteps: ['{cmd_b}']\n")).unwrap();
        prop_assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn parser_never_panics_on_arbitrary_text(text in "\\PC{0,200}") {
        let _ = Snippet::from_yaml(&text);
        let _ = forge_core::scan::scan(&text);
        let _ = forge_core::lint::lint(&text);
    }

    #[test]
    fn tag_folding_is_idempotent(tag in "[A-Za-z0-9-]{1,15}") {
        let once = forge_core::names::normalize_tag(&tag);
        if let Ok(t) = once {
            let twice = forge_core::names::normalize_tag(&t).unwrap();
            prop_assert_eq!(t, twice);
        }
    }
}
