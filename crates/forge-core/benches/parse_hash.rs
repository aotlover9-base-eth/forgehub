//! Parse + hash benchmark for a ~64 KiB script-mode snippet
//! (roadmap Step 3: record the numbers in the step report).
//!
//! Run with: `cargo bench -p forge-core --bench parse_hash`
//! A plain timing harness (std only) so the workspace stays dependency-
//! light; it prints mean/p95 per iteration over 200 iterations.

use std::fmt::Write as _;
use std::time::Instant;

fn main() {
    // Build a script body just under the 64 KiB cap.
    let mut body = String::from("name: bench\nmode: script\nscript: |\n");
    let mut i = 0;
    while body.len() < 60_000 {
        // Writing to a String is infallible.
        let _ = writeln!(
            body,
            "  echo step {i} && some-command --with-flags value{i}"
        );
        i += 1;
    }
    println!("input: {} bytes", body.len());

    // Warm-up.
    for _ in 0..10 {
        let s = forge_core::Snippet::from_yaml(&body).expect("bench input parses");
        std::hint::black_box(forge_core::hash::content_hash(&s));
    }

    let iterations: u32 = 200;
    let mut times = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let start = Instant::now();
        let s = forge_core::Snippet::from_yaml(&body).expect("bench input parses");
        let h = forge_core::hash::content_hash(&s);
        times.push(start.elapsed());
        std::hint::black_box(h);
    }
    times.sort();
    let total: std::time::Duration = times.iter().sum();
    let mean = total / iterations;
    let p95_index = usize::try_from(iterations * 95 / 100 - 1).unwrap_or(0);
    let p95 = times[p95_index];
    println!(
        "parse+hash of ~64 KiB script: mean {mean:?}, p95 {p95:?} over {iterations} iterations"
    );
}
