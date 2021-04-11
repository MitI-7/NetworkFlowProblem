// verification-helper: PROBLEM https://judge.yosupo.jp/problem/min_cost_b_flow
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

#[allow(non_snake_case)]
fn main() {
    let (n, m) = (read(), read());

    let mut solver: CostScalingPushRelabel<i64> = CostScalingPushRelabel::new(n);

    for u in 0..n {
        let b = read();
        solver.add_supply(u, b);
    }

    let mut edges = Vec::new();
    for _i in 0..m {
        let (s, t, l, u, c) = (read(), read(), read(), read(), read());
        let edge_id = solver.add_directed_edge(s, t, l, u, c);
        edges.push(edge_id);
    }

    let status = solver.solve();
    match status {
        Status::Optimal => {
            println!("{}", solver.optimal_cost().unwrap_or(0));
            let p = solver.calculate_potential();
            for u in 0..n {
                println!("{}", p[u]);
            }
            for edge_id in &edges {
                println!("{}", solver.get_directed_edge(*edge_id).flow);
            }
        }
        _ => {
            println!("infeasible");
        }
    }
}