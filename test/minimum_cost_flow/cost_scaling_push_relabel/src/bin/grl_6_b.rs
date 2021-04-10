// verification-helper: PROBLEM https://judge.u-aizu.ac.jp/onlinejudge/description.jsp?id=GRL_6_B
use std::str::FromStr;
use std::io::*;

use cost_scaling_push_relabel::{CostScalingPushRelabel, Status};

fn read<T: FromStr>() -> T {
    let stdin = stdin();
    let stdin = stdin.lock();
    let token: String = stdin
        .bytes()
        .map(|c| c.expect("failed to read char") as char)
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| !c.is_whitespace())
        .collect();
    token.parse().ok().expect("failed to parse token")
}

fn main() {
    let (v, e, f) = (read(), read(), read());

    let mut solver = CostScalingPushRelabel::new(v);
    for _edge in 0..e {
        let (u, v, c, d) = (read(), read(), read(), read());
        solver.add_directed_edge(u, v, 0, c, d);
    }
    solver.add_supply(0, f);
    solver.add_supply(v - 1, -f);
    match solver.solve() {
        Status::Optimal => println!("{}", solver.optimal_cost().unwrap_or(0)),
        _ => println!("-1"),
    }
}