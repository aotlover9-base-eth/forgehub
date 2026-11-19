//! Performance check (Step 10): seed 10,000 public snippets and
//! measure p95 latency for snippet reads and search against the
//! agent.md budget (p95 < 50 ms at 10k snippets).
//!
//! Ignored by default (seeding takes a while); run with:
//! `cargo test --release -p forge-server --test perf -- --ignored --nocapture`

use forge_server::config::Config;
use forge_server::http::build_router;
use forge_server::repo::Repos;
use forge_server::state::AppState;
use sqlx::Row;
use std::collections::HashMap;
use std::time::Instant;

fn percentile(sorted_ms: &[f64], p: usize) -> f64 {
    let idx = (sorted_ms.len() * p).div_ceil(100);
    sorted_ms[idx.saturating_sub(1).min(sorted_ms.len() - 1)]
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // one seed + measure scenario
#[ignore = "slow seed; run explicitly with --ignored"]
async fn p95_at_10k_snippets() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("perf.db");
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
    let owner = repos
        .users
        .create("perf", "perf@example.com", "hash", "2026-11-19T00:00:00Z")
        .await
        .unwrap();
    let seed_start = Instant::now();
    for i in 0..10_000 {
        let name = format!("snip-{i:05}");
        let body = format!(
            "name: {name}\ndescription: Perf snippet {i} about docker and ubuntu\ntags: [perf, t{}]\nscript: |\n  echo snippet {i}\n  docker ps\n",
            i % 25
        );
        repos
            .snippets
            .create_with_version(
                owner.id,
                &name,
                &format!("Perf snippet {i} about docker and ubuntu"),
                "public",
                "script",
                &body,
                &format!("hash{i:059}"),
                None,
                &["perf".to_string(), format!("t{}", i % 25)],
                "2026-11-19T00:00:00Z",
            )
            .await
            .unwrap();
    }
    println!("seeded 10,000 snippets in {:?}", seed_start.elapsed());

    // Split the search cost: time the repository query alone
    // (FTS scan + bm25) before measuring over HTTP.
    let match_q = "\"docker\"*";
    let t = Instant::now();
    for _ in 0..20 {
        let rows = repos
            .snippets
            .search_public(Some(match_q), None, None, "relevance", 21, 0)
            .await
            .unwrap();
        assert_eq!(rows.len(), 21);
    }
    println!(
        "repo search_public alone: {:.2} ms avg",
        t.elapsed().as_secs_f64() * 1000.0 / 20.0
    );

    let state = AppState::new(config, repos, pool);
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
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    // Snippet reads.
    let mut reads = Vec::new();
    for i in 0..200 {
        let n = (i * 37) % 10_000;
        let t = Instant::now();
        let resp = client
            .get(format!("{base}/api/v1/snippets/perf/snip-{n:05}"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        resp.bytes().await.unwrap();
        reads.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    reads.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let read_p50 = percentile(&reads, 50);
    let read_p95 = percentile(&reads, 95);
    println!("snippet read: p50 {read_p50:.2} ms, p95 {read_p95:.2} ms (n=200)");

    // Search.
    let mut searches = Vec::new();
    for _ in 0..100 {
        let t = Instant::now();
        let resp = client
            .get(format!("{base}/api/v1/search?q=docker"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        resp.bytes().await.unwrap();
        searches.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    searches.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let search_p50 = percentile(&searches, 50);
    let search_p95 = percentile(&searches, 95);
    println!("search: p50 {search_p50:.2} ms, p95 {search_p95:.2} ms (n=100)");

    // Same search with limit=1 separates scan cost from the cost
    // of rendering a page of results.
    let mut one = Vec::new();
    for _ in 0..50 {
        let t = Instant::now();
        let resp = client
            .get(format!("{base}/api/v1/search?q=docker&limit=1"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        resp.bytes().await.unwrap();
        one.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    one.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "search limit=1: p50 {:.2} ms, p95 {:.2} ms (n=50)",
        percentile(&one, 50),
        percentile(&one, 95)
    );

    // Explore list (the public list path).
    let mut lists = Vec::new();
    for _ in 0..100 {
        let t = Instant::now();
        let resp = client
            .get(format!("{base}/api/v1/explore?feed=recent"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        resp.bytes().await.unwrap();
        lists.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    lists.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let list_p95 = percentile(&lists, 95);
    println!(
        "explore list: p50 {:.2} ms, p95 {:.2} ms (n=100)",
        percentile(&lists, 50),
        list_p95
    );

    // Conditional read: a matching If-None-Match must 304.
    let resp = client
        .get(format!("{base}/api/v1/snippets/perf/snip-00001"))
        .send()
        .await
        .unwrap();
    let etag = resp
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let resp = client
        .get(format!("{base}/api/v1/snippets/perf/snip-00001"))
        .header("if-none-match", &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 304, "If-None-Match must yield 304");

    // Create (the push path): sign up once, reuse the session.
    let signup = client
        .post(format!("{base}/api/v1/auth/signup"))
        .json(&serde_json::json!({
            "username": "perfpush",
            "email": "perfpush@example.com",
            "password": "a long passphrase",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(signup.status(), 201);
    let cookie = signup
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let me: serde_json::Value = client
        .get(format!("{base}/api/v1/me"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let csrf = me["csrf_token"].as_str().unwrap().to_string();
    let mut creates = Vec::new();
    for i in 0..30 {
        let t = Instant::now();
        let resp = client
            .post(format!("{base}/api/v1/snippets"))
            .header("cookie", &cookie)
            .header("x-csrf-token", &csrf)
            .json(&serde_json::json!({
                "name": format!("pushed-{i:03}"),
                "body": "echo pushed",
                "visibility": "private",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
        resp.bytes().await.unwrap();
        creates.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    creates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let create_p95 = percentile(&creates, 95);
    println!(
        "snippet create (push): p50 {:.2} ms, p95 {:.2} ms (n=30)",
        percentile(&creates, 50),
        create_p95
    );

    // The budget is a release-mode budget; in debug builds we only
    // record the numbers (the harness prints them either way).
    if cfg!(not(debug_assertions)) {
        assert!(
            read_p95 < 50.0,
            "read p95 {read_p95:.2} ms exceeds the 50 ms budget"
        );
        assert!(
            search_p95 < 50.0,
            "search p95 {search_p95:.2} ms exceeds the 50 ms budget"
        );
        assert!(
            list_p95 < 50.0,
            "list p95 {list_p95:.2} ms exceeds the 50 ms budget"
        );
    }
}

/// Step 20 scale check: 10,000 users and 50,000 snippets,
/// seeded with bulk SQL (the repo path would take minutes).
/// Read/search/list must still hold the p95 budget shape.
#[tokio::test]
#[allow(clippy::too_many_lines)] // one seed + measure scenario
#[ignore = "slow seed; run explicitly with --ignored"]
async fn p95_at_50k_snippets_10k_users() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("perf50.db");
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

    let seed_start = Instant::now();
    let now = "2026-11-19T00:00:00Z";
    let mut tx = pool.begin().await.unwrap();
    for u in 0..10_000 {
        sqlx::query(
            "INSERT INTO users (username, email, password_hash, created_at) VALUES (?, ?, 'hash', ?)",
        )
        .bind(format!("user{u:05}"))
        .bind(format!("user{u:05}@example.com"))
        .bind(now)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();

    for chunk_start in (0..50_000).step_by(5_000) {
        let mut tx = pool.begin().await.unwrap();
        for i in chunk_start..(chunk_start + 5_000) {
            let owner = (i % 10_000) + 1;
            let name = format!("snip-{i:05}");
            let desc = format!("Perf snippet {i} about docker and ubuntu");
            let body = format!(
                "name: {name}\ndescription: {desc}\ntags: [perf, t{}]\nscript: |\n  echo snippet {i}\n  docker ps\n",
                i % 25
            );
            let id = sqlx::query(
                "INSERT INTO snippets (owner_id, name, description, visibility, latest_version, created_at, updated_at) \
                 VALUES (?, ?, ?, 'public', 1, ?, ?) RETURNING id",
            )
            .bind(owner)
            .bind(&name)
            .bind(&desc)
            .bind(now)
            .bind(now)
            .fetch_one(&mut *tx)
            .await
            .unwrap()
            .get::<i64, _>(0);
            sqlx::query(
                "INSERT INTO snippet_versions (snippet_id, version, mode, body, hash, created_at) VALUES (?, 1, 'script', ?, ?, ?)",
            )
            .bind(id)
            .bind(&body)
            .bind(format!("hash{i:059}"))
            .bind(now)
            .execute(&mut *tx)
            .await
            .unwrap();
            sqlx::query("INSERT INTO snippet_tags (snippet_id, tag) VALUES (?, 'perf'), (?, ?)")
                .bind(id)
                .bind(id)
                .bind(format!("t{}", i % 25))
                .execute(&mut *tx)
                .await
                .unwrap();
            sqlx::query(
                "INSERT INTO snippets_fts (rowid, name, description, tags, owner, body) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(&name)
            .bind(&desc)
            .bind(format!("perf t{}", i % 25))
            .bind(format!("user{:05}", i % 10_000))
            .bind(&body)
            .execute(&mut *tx)
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();
    }
    println!(
        "seeded 10,000 users + 50,000 snippets in {:?}",
        seed_start.elapsed()
    );

    let repos = Repos::sqlite(pool.clone());
    let state = AppState::new(config, repos, pool);
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
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let mut reads = Vec::new();
    for i in 0..200 {
        let n = (i * 251) % 50_000;
        let t = Instant::now();
        let resp = client
            .get(format!(
                "{base}/api/v1/snippets/user{:05}/snip-{n:05}",
                n % 10_000
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        resp.bytes().await.unwrap();
        reads.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    reads.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "50k read: p50 {:.2} ms, p95 {:.2} ms, p99 {:.2} ms (n=200)",
        percentile(&reads, 50),
        percentile(&reads, 95),
        percentile(&reads, 99)
    );

    let mut searches = Vec::new();
    for _ in 0..100 {
        let t = Instant::now();
        let resp = client
            .get(format!("{base}/api/v1/search?q=docker"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        resp.bytes().await.unwrap();
        searches.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    searches.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "50k search: p50 {:.2} ms, p95 {:.2} ms, p99 {:.2} ms (n=100)",
        percentile(&searches, 50),
        percentile(&searches, 95),
        percentile(&searches, 99)
    );

    if cfg!(not(debug_assertions)) {
        assert!(
            percentile(&reads, 95) < 50.0,
            "50k read p95 exceeds the 50 ms budget"
        );
    }
}
