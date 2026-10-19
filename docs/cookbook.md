# Cookbook

Short recipes for common tasks. Each block is a doctest, so it is checked on
every `cargo test --doc` run.

## Build an expression in Rust

Parsing and the operators build the same canonical tree:

```rust
use tpt_symbolic::parse::parse;
use tpt_symbolic::Expr;

let x = Expr::var("x");
let built = x.clone() * x.clone() + Expr::from(1i64);
assert_eq!(built, parse("x*x + 1").unwrap());
```

## Check whether two expressions are equal

`simplify` does not expand powers of sums, so expand both sides first:

```rust
use tpt_symbolic::parse::parse;
use tpt_symbolic::simplify::{expand, simplify};

let lhs = parse("(x + 1)^2 - 2*x").unwrap();
let rhs = parse("x^2 + 1").unwrap();
assert_eq!(simplify(&expand(&lhs)), simplify(&expand(&rhs)));
```

## Differentiate and check with a substitution

```rust
use tpt_symbolic::calculus::diff;
use tpt_symbolic::eval::subs;
use tpt_symbolic::parse::parse;
use tpt_symbolic::Expr;

let f = parse("x^3").unwrap();
let df = diff(&f, "x").unwrap();
assert_eq!(df, parse("3*x^2").unwrap());

// Compare at a point, exactly.
let at_two = subs(&df, "x", &Expr::from(2i64));
assert_eq!(at_two, parse("12").unwrap());
```


## Integrate and keep only verified answers

`integrate` checks each antiderivative by differentiating it. An unsupported
form comes back as `Unevaluated`, never as a wrong answer:

```rust
use tpt_symbolic::calculus::{integrate, Integral};
use tpt_symbolic::parse::parse;

match integrate(&parse("2*x + 1").unwrap(), "x") {
    Integral::Exact(f) => assert_eq!(f, parse("x^2 + x").unwrap()),
    Integral::Unevaluated(_) => panic!("supported form"),
    _ => panic!("unexpected variant"),
}
```

## Solve an equation and handle every outcome

```rust
use tpt_symbolic::parse::parse;
use tpt_symbolic::solve::{solve, SolveResult};

let eq = parse("x^2 - 5*x + 6 = 0").unwrap();
match solve(&eq, "x").unwrap() {
    SolveResult::Exact(roots) => assert_eq!(roots.len(), 2),
    SolveResult::Numeric(root) => println!("hand off to numeric root-finding: {}", root.expr),
    SolveResult::NoSolution => println!("no real solution"),
    _ => {}
}
```

## Get exact radical roots

```rust
use tpt_symbolic::parse::parse;
use tpt_symbolic::solve::{solve, SolveResult};

// x^3 = 2 has one real root, 2^(1/3). It is kept exact, not rounded.
let root = solve(&parse("x^3 - 2 = 0").unwrap(), "x").unwrap();
assert!(matches!(root, SolveResult::Exact(ref r) if r.len() == 1));
```

## Solve a linear system exactly

```rust
use tpt_symbolic::mathr::Number;
use tpt_symbolic::solve::{solve_linear_system, SolveResult};

let q = Number::from_int;
// 2x + 3y = 8 and x - y = -1.
let a = vec![vec![q(2), q(3)], vec![q(1), q(-1)]];
let b = vec![q(8), q(-1)];
assert!(matches!(solve_linear_system(&a, &b).unwrap(), SolveResult::Exact(_)));
```

## Print a decimal to a fixed number of places

```rust
use tpt_symbolic::eval::to_decimal;
use tpt_symbolic::Number;

// Truncated, not rounded.
assert_eq!(to_decimal(&Number::new(2, 3).unwrap(), 4), "0.6666");
```

## Print LaTeX

```rust
use tpt_symbolic::latex::to_latex;
use tpt_symbolic::parse::parse;

assert_eq!(to_latex(&parse("x/2").unwrap()), "\\frac{x}{2}");
```

## Report a parse error with its position

```rust
use tpt_symbolic::error::{line_col, Error};
use tpt_symbolic::parse::parse;

// A misspelt function name gets a suggestion and the position of the name.
match parse("sni(x)") {
    Err(Error::UnknownFunction { pos, name, suggestion }) => {
        assert_eq!(name, "sni");
        assert_eq!(suggestion, Some("sin"));
        assert_eq!(line_col("sni(x)", pos), (1, 1));
    }
    other => panic!("expected an unknown-function error, got {other:?}"),
}

// Any other syntax error is `Error::Parse`, with a byte offset.
match parse("2 * )") {
    Err(Error::Parse { pos, .. }) => assert_eq!(line_col("2 * )", pos), (1, 5)),
    other => panic!("expected a parse error, got {other:?}"),
}
```

## Check a value numerically without floats

Exact rationals, so there is no rounding error to reason about:

```rust
use tpt_symbolic::eval::eval;
use tpt_symbolic::parse::parse;
use tpt_symbolic::Number;

let f = parse("x^2 - x/3").unwrap();
let v = eval(&f, &[("x", Number::new(2, 3).unwrap())]).unwrap();
assert_eq!(v, Number::new(2, 9).unwrap());
```
