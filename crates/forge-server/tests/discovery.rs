//! Discovery tests (Step 10): search safety and relevance, stars,
//! forks, profiles, explore/tags, pull counting, reports, and the
//! admin operations (hide/ban) exercised through `admin::run`.

use forge_server::config::Config;
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use std::collections::HashMap;

struct TestServer {
    base: String,
    client: reqwest::Client,
    repos: Repos,
    _dir: tempfile::TempDir,
}

async fn start() -> TestServer {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("discovery.db");
    let map: HashMap<String, String> = [
        (
            "FORGEHUB_DATABASE_URL",
            format!("sqlite://{}", db.display()),
        ),
        (
            "FORGEHUB_WEB_DIR",
            dir.path().join("no-web").display().to_string(),
        ),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.clone()))
    .collect();
    let config = Config::from_vars(&|k| map.get(k).cloned()).unwrap();
    let pool = forge_server::db::connect(&config.database_url)
        .await
        .unwrap();
    forge_server::db::migrate(&pool).await.unwrap();
    let repos = Repos::sqlite(pool.clone());
    let state = AppState::new(config, Repos::sqlite(pool.clone()), pool);
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    TestServer {
        base: format!("http://{addr}"),
        client: reqwest::Client::new(),
        repos,
        _dir: dir,
    }
}

const PASSWORD: &str = "a-perfectly-fine-passphrase";

async fn signup(s: &TestServer, username: &str) -> (String, String) {
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/signup", s.base))
        .json(&serde_json::json!({
            "username": username,
            "email": format!("{username}@example.com"),
            "password": PASSWORD,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "signup {username}");
    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let id = cookie
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("forgehub_session=")
        .to_string();
    let me: serde_json::Value = resp.json().await.unwrap();
    (
        format!("forgehub_session={id}"),
        me["csrf_token"].as_str().unwrap().to_string(),
    )
}

async fn create(
    s: &TestServer,
    cookie: &str,
    csrf: &str,
    payload: serde_json::Value,
) -> (u16, serde_json::Value) {
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets", s.base))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .json(&payload)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(serde_json::Value::Null))
}

async fn get(s: &TestServer, path: &str, cookie: Option<&str>) -> (u16, serde_json::Value) {
    let mut req = s.client.get(format!("{}{path}", s.base));
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(serde_json::Value::Null))
}

async fn seed(s: &TestServer) -> (String, String, String, String) {
    let (anna, ac) = signup(s, "anna").await;
    let (ben, bc) = signup(s, "ben").await;
    for (name, vis, body, tags) in [
        (
            "docker-cleanup",
            "public",
            "docker system prune -af\ndocker volume prune\n",
            vec!["docker", "ops"],
        ),
        (
            "docker-diary",
            "private",
            "docker logs my-diary\n",
            vec!["docker"],
        ),
        ("docker-unlisted", "unlisted", "docker ps\n", vec!["docker"]),
        (
            "apt-update",
            "public",
            "sudo apt update && sudo apt upgrade\n",
            vec!["ubuntu"],
        ),
    ] {
        let (status, v) = create(
            s,
            &anna,
            &ac,
            serde_json::json!({ "name": name, "visibility": vis, "body": body, "tags": tags }),
        )
        .await;
        assert_eq!(status, 201, "seed {name}: {v}");
    }
    (anna, ac, ben, bc)
}

#[tokio::test]
async fn search_finds_public_only_with_highlights() {
    let s = start().await;
    seed(&s).await;
    let (status, v) = get(&s, "/api/v1/search?q=docker", None).await;
    assert_eq!(status, 200);
    let results = v["results"].as_array().unwrap();
    assert_eq!(results.len(), 1, "only the public docker snippet: {v}");
    assert_eq!(results[0]["snippet"]["name"], "docker-cleanup");
    let highlights = results[0]["highlights"].as_array().unwrap();
    let name_hl = highlights
        .iter()
        .find(|h| h["field"] == "name")
        .expect("name highlight");
    let start = usize::try_from(name_hl["start"].as_u64().unwrap()).unwrap();
    let end = usize::try_from(name_hl["end"].as_u64().unwrap()).unwrap();
    assert_eq!(&"docker-cleanup"[start..end], "docker");
    // Tag and owner filters; prefix matching.
    let (_, v) = get(&s, "/api/v1/search?tag=ubuntu", None).await;
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    let (_, v) = get(&s, "/api/v1/search?q=dock&owner=anna", None).await;
    assert_eq!(v["results"].as_array().unwrap().len(), 1, "prefix match");
    let (_, v) = get(&s, "/api/v1/search?q=docker&owner=nobody", None).await;
    assert_eq!(
        v["results"].as_array().unwrap().len(),
        0,
        "unknown owner -> empty page"
    );
    let (_, v) = get(&s, "/api/v1/search?q=prune", None).await;
    assert_eq!(
        v["results"].as_array().unwrap().len(),
        1,
        "body terms are indexed"
    );
}

#[tokio::test]
async fn search_fts_injection_is_inert() {
    let s = start().await;
    seed(&s).await;
    for q in [
        "docker OR apt",
        "name:docker",
        "\"docker",
        "NEAR(docker apt)",
        "*",
        "docker) (",
        "AND OR NOT",
        "docker\"* OR visibility:private",
        "'; DROP TABLE snippets; --",
    ] {
        let encoded = urlencoding(q);
        let (status, v) = get(&s, &format!("/api/v1/search?q={encoded}"), None).await;
        assert_eq!(status, 200, "query {q:?} must not error: {v}");
        for r in v["results"].as_array().unwrap() {
            assert_eq!(
                r["snippet"]["visibility"], "public",
                "query {q:?} leaked a non-public snippet"
            );
        }
    }
    // The table still exists and search still works afterwards.
    let (_, v) = get(&s, "/api/v1/search?q=docker", None).await;
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
}

fn urlencoding(raw: &str) -> String {
    raw.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[tokio::test]
async fn stars_flow() {
    let s = start().await;
    let (anna, ac, ben, bc) = seed(&s).await;
    let star = |method_put: bool, cookie: Option<&str>, csrf: Option<&str>, path: &str| {
        let (s, path) = (&s, path.to_string());
        let (cookie, csrf) = (cookie.map(str::to_string), csrf.map(str::to_string));
        async move {
            let mut req = if method_put {
                s.client.put(format!("{}{path}", s.base))
            } else {
                s.client.delete(format!("{}{path}", s.base))
            };
            if let Some(c) = &cookie {
                req = req.header("cookie", c);
            }
            if let Some(c) = &csrf {
                req = req.header("x-csrf-token", c);
            }
            req.send().await.unwrap().status().as_u16()
        }
    };
    let path = "/api/v1/snippets/anna/docker-cleanup/star";
    assert_eq!(star(true, None, None, path).await, 401, "anonymous star");
    assert_eq!(star(true, Some(&ben), Some(&bc), path).await, 204);
    assert_eq!(
        star(true, Some(&ben), Some(&bc), path).await,
        204,
        "idempotent"
    );
    let (_, v) = get(&s, "/api/v1/snippets/anna/docker-cleanup", None).await;
    assert_eq!(v["star_count"], 1);
    assert_eq!(v["starred_by_me"], false, "anonymous viewer");
    let (_, v) = get(&s, "/api/v1/me/stars", Some(&ben)).await;
    assert_eq!(v["snippets"].as_array().unwrap().len(), 1);
    assert_eq!(v["snippets"][0]["starred_by_me"], true);
    assert_eq!(
        star(false, Some(&ben), Some(&bc), path).await,
        204,
        "unstar"
    );
    let (_, v) = get(&s, "/api/v1/snippets/anna/docker-cleanup", None).await;
    assert_eq!(v["star_count"], 0);
    // Private snippets cannot be starred - not even by their owner.
    assert_eq!(
        star(
            true,
            Some(&anna),
            Some(&ac),
            "/api/v1/snippets/anna/docker-diary/star"
        )
        .await,
        404
    );
    // Unlisted can be starred by anyone who can see it.
    assert_eq!(
        star(
            true,
            Some(&ben),
            Some(&bc),
            "/api/v1/snippets/anna/docker-unlisted/star"
        )
        .await,
        204
    );
}

#[tokio::test]
async fn fork_flow() {
    let s = start().await;
    let (_anna, _ac, ben, bc) = seed(&s).await;
    let resp = s
        .client
        .post(format!(
            "{}/api/v1/snippets/anna/docker-cleanup/fork",
            s.base
        ))
        .header("cookie", &ben)
        .header("x-csrf-token", &bc)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["name"], "docker-cleanup");
    assert_eq!(v["visibility"], "private", "forks start private");
    assert_eq!(v["owner"], "ben");
    assert_eq!(v["forked_from"]["owner"], "anna");
    assert_eq!(v["forked_from"]["name"], "docker-cleanup");
    assert_eq!(v["forked_from"]["version"], 1);
    assert_eq!(
        v["forked_from"]["hash"], v["hash"],
        "fork copies the exact version"
    );
    let (_, src) = get(&s, "/api/v1/snippets/anna/docker-cleanup", None).await;
    assert_eq!(src["fork_count"], 1);
    // Second fork with the same name -> 409; custom name works.
    let resp = s
        .client
        .post(format!(
            "{}/api/v1/snippets/anna/docker-cleanup/fork",
            s.base
        ))
        .header("cookie", &ben)
        .header("x-csrf-token", &bc)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let resp = s
        .client
        .post(format!(
            "{}/api/v1/snippets/anna/docker-cleanup/fork",
            s.base
        ))
        .header("cookie", &ben)
        .header("x-csrf-token", &bc)
        .json(&serde_json::json!({ "name": "my-cleanup" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    // Forking someone else's private snippet -> 404.
    let resp = s
        .client
        .post(format!("{}/api/v1/snippets/anna/docker-diary/fork", s.base))
        .header("cookie", &ben)
        .header("x-csrf-token", &bc)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn profiles_never_leak_private_data() {
    let s = start().await;
    seed(&s).await;
    let resp = s
        .client
        .get(format!("{}/api/v1/users/anna", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let text = resp.text().await.unwrap();
    assert!(
        !text.contains("example.com"),
        "email must never appear: {text}"
    );
    assert!(!text.contains("email"), "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["username"], "anna");
    assert_eq!(
        v["public_snippet_count"], 2,
        "public only (not private/unlisted)"
    );
    let (_, v) = get(&s, "/api/v1/users/anna/snippets", None).await;
    let names: Vec<&str> = v["snippets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"docker-cleanup") && names.contains(&"apt-update"));
    let (status, _) = get(&s, "/api/v1/users/ghost", None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn explore_and_tags() {
    let s = start().await;
    let (_a, _ac, ben, bc) = seed(&s).await;
    let (_, v) = get(&s, "/api/v1/tags", None).await;
    let tags = v["tags"].as_array().unwrap();
    let docker = tags
        .iter()
        .find(|t| t["tag"] == "docker")
        .expect("docker tag");
    assert_eq!(docker["count"], 1, "only the public snippet counts");
    let (_, v) = get(&s, "/api/v1/explore?feed=recent", None).await;
    assert_eq!(v["snippets"].as_array().unwrap().len(), 2, "public only");
    // Star one, popular feed puts it first.
    let resp = s
        .client
        .put(format!("{}/api/v1/snippets/anna/apt-update/star", s.base))
        .header("cookie", &ben)
        .header("x-csrf-token", &bc)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    let (_, v) = get(&s, "/api/v1/explore?feed=popular", None).await;
    assert_eq!(v["snippets"][0]["name"], "apt-update");
}

#[tokio::test]
async fn pull_counter_dedupes_per_ip() {
    let s = start().await;
    let (anna, _ac, _b, _bc) = seed(&s).await;
    let path = "/api/v1/snippets/anna/docker-cleanup";
    let (_, v) = get(&s, &format!("{path}?intent=run"), None).await;
    assert_eq!(v["pull_count"], 1);
    let (_, v) = get(&s, &format!("{path}?intent=run"), None).await;
    assert_eq!(
        v["pull_count"], 1,
        "same IP within the hour does not recount"
    );
    let (_, v) = get(&s, path, None).await;
    assert_eq!(v["pull_count"], 1, "no intent, no count");
    let (_, v) = get(&s, &format!("{path}?intent=run"), Some(&anna)).await;
    assert_eq!(v["pull_count"], 1, "the owner never counts");
    let (_, v) = get(&s, "/api/v1/snippets/anna/apt-update?intent=run", None).await;
    assert_eq!(v["pull_count"], 1, "dedup is per snippet");
}

#[tokio::test]
async fn report_flow_dedupes() {
    let s = start().await;
    let (_a, _ac, ben, bc) = seed(&s).await;
    let report = |cookie: Option<&str>, csrf: Option<&str>, reason: &str| {
        let (s, reason) = (&s, reason.to_string());
        let (cookie, csrf) = (cookie.map(str::to_string), csrf.map(str::to_string));
        async move {
            let mut req = s
                .client
                .post(format!(
                    "{}/api/v1/snippets/anna/docker-cleanup/report",
                    s.base
                ))
                .json(&serde_json::json!({ "reason": reason }));
            if let Some(c) = &cookie {
                req = req.header("cookie", c);
            }
            if let Some(c) = &csrf {
                req = req.header("x-csrf-token", c);
            }
            req.send().await.unwrap().status().as_u16()
        }
    };
    assert_eq!(report(None, None, "spam").await, 401);
    assert_eq!(report(Some(&ben), Some(&bc), "").await, 400);
    assert_eq!(report(Some(&ben), Some(&bc), "looks malicious").await, 204);
    assert_eq!(
        report(Some(&ben), Some(&bc), "looks malicious").await,
        204,
        "duplicate is a silent success"
    );
    let open = s.repos.reports.count_by_status("open").await.unwrap();
    assert_eq!(open, 1, "but only one report is stored");
}

#[tokio::test]
async fn admin_hide_and_ban() {
    let s = start().await;
    let (anna, _ac, _b, _bc) = seed(&s).await;
    // Hide: public surfaces lose it; the owner keeps access.
    forge_server::admin::run(
        &s.repos,
        &["hide-snippet".into(), "anna/docker-cleanup".into()],
    )
    .await
    .unwrap();
    let (status, _) = get(&s, "/api/v1/snippets/anna/docker-cleanup", None).await;
    assert_eq!(status, 404, "hidden 404s for the public");
    let (status, _) = get(&s, "/api/v1/snippets/anna/docker-cleanup", Some(&anna)).await;
    assert_eq!(status, 200, "owner still sees their hidden snippet");
    let (_, v) = get(&s, "/api/v1/search?q=docker", None).await;
    assert_eq!(
        v["results"].as_array().unwrap().len(),
        0,
        "hidden leaves search"
    );
    forge_server::admin::run(
        &s.repos,
        &["unhide-snippet".into(), "anna/docker-cleanup".into()],
    )
    .await
    .unwrap();
    let (status, _) = get(&s, "/api/v1/snippets/anna/docker-cleanup", None).await;
    assert_eq!(status, 200);
    // Ban: sessions die, profile 404s; unban restores login.
    forge_server::admin::run(&s.repos, &["ban-user".into(), "anna".into()])
        .await
        .unwrap();
    let (status, _) = get(&s, "/api/v1/me", Some(&anna)).await;
    assert_eq!(status, 401, "banned user's session is revoked");
    let (status, _) = get(&s, "/api/v1/users/anna", None).await;
    assert_eq!(status, 404);
    forge_server::admin::run(&s.repos, &["unban-user".into(), "anna".into()])
        .await
        .unwrap();
    let resp = s
        .client
        .post(format!("{}/api/v1/auth/login", s.base))
        .json(&serde_json::json!({ "identifier": "anna", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "unbanned user can log in again");
}
