//! Secret-scanner corpus: positive cases (every kind must be caught)
//! and negative cases (placeholders, hashes, ordinary commands must
//! stay clean). Lives under tests/ so scripts/scan-staged.sh can
//! exclude this corpus - its fake secrets are fake, but a scanner
//! cannot know that.

use forge_core::scan::{redact, scan, scan_snippet, SecretKind};

// Most fake tokens are built by concatenation/repetition; the few
// literal ones are why this corpus lives under tests/ (excluded from
// scan-staged, like any scanner allowlist).
fn kinds(text: &str) -> Vec<SecretKind> {
    scan(text).iter().map(|f| f.kind).collect()
}

#[test]
fn detects_private_key_block() {
    let text = format!("-----BEGIN {} PRIVATE KEY-----", "RSA");
    assert_eq!(kinds(&text), vec![SecretKind::PrivateKey]);
}

#[test]
fn detects_aws_access_key() {
    let text = format!("export AWS_ACCESS_KEY_ID=AKIA{}", "A".repeat(16));
    assert!(kinds(&text).contains(&SecretKind::AwsAccessKey));
}

#[test]
fn detects_github_classic_token() {
    let text = format!("token ghp_{} end", "a".repeat(36));
    assert_eq!(kinds(&text), vec![SecretKind::GitHubToken]);
}

#[test]
fn detects_github_fine_grained_token() {
    let text = format!("github_pat_{}", "b".repeat(30));
    assert_eq!(kinds(&text), vec![SecretKind::GitHubToken]);
}

#[test]
fn detects_gitlab_token() {
    let text = format!("glpat-{}", "c".repeat(20));
    assert_eq!(kinds(&text), vec![SecretKind::GitLabToken]);
}

#[test]
fn detects_slack_token() {
    let text = format!("xoxb-{}-abcdefghij", "1".repeat(10));
    assert_eq!(kinds(&text), vec![SecretKind::SlackToken]);
}

#[test]
fn detects_stripe_live_secret() {
    let text = format!("sk_live_{}", "d".repeat(24));
    assert_eq!(kinds(&text), vec![SecretKind::StripeSecretKey]);
}

#[test]
fn detects_bearer_token() {
    let text = format!("Authorization: Bearer {}", "aB3".repeat(10));
    assert_eq!(kinds(&text), vec![SecretKind::BearerToken]);
}

#[test]
fn detects_password_flag_literal() {
    assert_eq!(
        kinds("mysql -u root --password=hunter2pw"),
        vec![SecretKind::PasswordLiteral]
    );
}

#[test]
fn detects_password_assignment() {
    assert_eq!(
        kinds("password = sup3rsecret"),
        vec![SecretKind::PasswordLiteral]
    );
}

#[test]
fn detects_secret_named_assignment() {
    assert!(kinds("API_TOKEN=Zx9Qw8Er5Ty2").contains(&SecretKind::SecretAssignment));
}

#[test]
fn detects_high_entropy_assignment() {
    // Pseudo-random mixed-case token, not secret-named.
    let text = "SESSION_DATA=aB3dF6hJ9kL2mN5pQ8rS".to_string();
    assert!(kinds(&text).contains(&SecretKind::HighEntropyAssignment));
}

#[test]
fn line_numbers_are_one_based_and_exact() {
    let text = format!("echo ok\necho fine\nexport TOKEN={}\n", "y".repeat(12));
    let found = scan(&text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
}

#[test]
fn findings_never_contain_the_value() {
    // Finding has only kind+line by construction; assert the Debug
    // output cannot leak the secret either.
    let secret = format!("glpat-{}", "z".repeat(20));
    let found = scan(&secret);
    assert!(!format!("{found:?}").contains(&"z".repeat(20)));
}

// ---- negative corpus: none of these may produce findings ----

#[test]
fn placeholder_password_is_clean() {
    assert_eq!(
        scan("mysql --password={{DB_PASSWORD}}"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("mysql --password $DB_PASSWORD"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("password: changeme"),
        [] as [forge_core::scan::Finding; 0]
    );
}

#[test]
fn placeholder_assignments_are_clean() {
    assert_eq!(
        scan("TOKEN={{TOKEN}}"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("API_KEY=$API_KEY"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("export TOKEN=<your-token-here>"),
        [] as [forge_core::scan::Finding; 0]
    );
}

#[test]
fn ordinary_commands_are_clean() {
    assert_eq!(
        scan("sudo apt update && sudo apt install -y git curl"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("docker compose up -d && docker compose logs -f"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("git commit -m 'a perfectly ordinary message'"),
        [] as [forge_core::scan::Finding; 0]
    );
}

#[test]
fn hashes_and_ids_are_clean() {
    // sha256 checksum assignment: pure hex is never entropy-flagged.
    assert_eq!(
        scan("CHECKSUM=9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"),
        [] as [forge_core::scan::Finding; 0]
    );
    // A bare git SHA in text is not an assignment and not a secret.
    assert_eq!(
        scan("git checkout 9f86d081884c7d659a2feaa0c55ad015"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("docker pull ubuntu@sha256:9f86d081884c7d659a2feaa0c55ad015"),
        [] as [forge_core::scan::Finding; 0]
    );
}

#[test]
fn bearer_word_without_token_is_clean() {
    assert_eq!(
        scan("echo 'Authorization: Bearer token goes here'"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("# the API expects a Bearer header"),
        [] as [forge_core::scan::Finding; 0]
    );
}

#[test]
fn publishable_stripe_key_is_clean() {
    let text = format!("pk_live_{}", "e".repeat(24));
    assert_eq!(scan(&text), [] as [forge_core::scan::Finding; 0]);
}

#[test]
fn test_mode_stripe_key_is_clean() {
    let text = format!("sk_test_{}", "f".repeat(24));
    assert_eq!(scan(&text), [] as [forge_core::scan::Finding; 0]);
}

#[test]
fn short_values_are_clean() {
    assert_eq!(scan("TOKEN=abc"), [] as [forge_core::scan::Finding; 0]);
    assert_eq!(
        scan("MODE=production DEBUG=false PORT=8080"),
        [] as [forge_core::scan::Finding; 0]
    );
}

#[test]
fn code_shaped_assignments_are_clean() {
    // Expressions assigned to secret-named variables are code, not
    // literal secrets (found by dogfooding on the server sources).
    assert_eq!(
        scan("let secret_key = get(\"FORGEHUB_SECRET_KEY\");"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("bucket.tokens = (bucket.tokens + elapsed * rate).min(cap);"),
        [] as [forge_core::scan::Finding; 0]
    );
    assert_eq!(
        scan("let ip_key = key_for(&headers, addr);"),
        [] as [forge_core::scan::Finding; 0]
    );
}

#[test]
fn token_substring_names_are_clean() {
    // `tokenize` merely contains the substring "token".
    assert_eq!(
        scan("content = '', tokenize = 'unicode61 remove_diacritics 2'"),
        [] as [forge_core::scan::Finding; 0]
    );
}

#[test]
fn segmented_secret_names_still_detected() {
    assert!(kinds("GITHUB_TOKEN=Zx9Qw8Er5Ty2").contains(&SecretKind::SecretAssignment));
    assert!(kinds("AUTH_TOKEN=Zx9Qw8Er5Ty2").contains(&SecretKind::SecretAssignment));
}

#[test]
fn redact_removes_token_but_keeps_line() {
    let token = "aB3".repeat(10);
    let dirty = format!("curl -H 'Authorization: Bearer {token}' https://x");
    let clean = redact(&dirty);
    assert!(!clean.contains(&token));
    assert!(clean.contains("https://x"));
}

#[test]
fn redact_removes_aws_key() {
    let key = format!("AKIA{}", "B".repeat(16));
    let clean = redact(&format!("aws configure set id {key}"));
    assert!(!clean.contains(&key));
}

#[test]
fn scan_snippet_uses_command_index() {
    let s = forge_core::Snippet::from_yaml(&format!(
        "name: x\nsteps:\n  - echo ok\n  - export TOKEN={}\n",
        "q".repeat(12)
    ))
    .unwrap();
    let found = scan_snippet(&s);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
}
