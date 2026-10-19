//! Wall-clock fuzzing of the parser and simplifier on deep random inputs.
//!
//! Needs `std` for timing, so it lives in an integration test rather than the
//! `no_std` library. Run with `cargo test --all-features`.

use std::time::{Duration, Instant};
use tpt_symbolic::parse::parse;
use tpt_symbolic::simplify::simplify;
use tpt_symbolic::testkit::{random_expr, Rng};

/// Generous per-case budget so the test is stable on slow CI machines.
const BUDGET: Duration = Duration::from_secs(5);

#[test]
fn simplify_is_idempotent_and_within_budget_on_deep_inputs() {
    let mut rng = Rng::new(99);
    for i in 0..2000 {
        let e = random_expr(&mut rng, 6);
        let start = Instant::now();
        let s = simplify(&e);
        let elapsed = start.elapsed();
        assert!(elapsed < BUDGET, "case {i}: simplify took {elapsed:?} on {e}");
        assert_eq!(simplify(&s), s, "case {i}: not idempotent for {e}");
    }
}

#[test]
fn parser_round_trips_deep_inputs() {
    let mut rng = Rng::new(4242);
    for i in 0..2000 {
        let e = random_expr(&mut rng, 6);
        let text = e.to_string();
        let start = Instant::now();
        let back = parse(&text).unwrap_or_else(|err| panic!("case {i}: {text:?}: {err}"));
        assert!(start.elapsed() < BUDGET, "case {i}: parse took too long on {text:?}");
        assert_eq!(back, e, "case {i}: round trip failed for {text:?}");
    }
}
