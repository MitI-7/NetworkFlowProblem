// verification-helper: PROBLEM https://judge.yosupo.jp/problem/assignment
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
    let n: usize = read();
    let mut A = vec![vec![0; n]; n];

    for i in 0..n {
        for j in 0..n {
            let a =read();
            A[i][j] = a;
        }
    }

    let mut solver: CostScalingPushRelabel<i64> = CostScalingPushRelabel::new(2 * n);

    let mut edges = Vec::new();
    for i in 0..n {
        for j in 0..n {
            let edge_id = solver.add_directed_edge(i, n + j, 0, 1, A[i][j]);
            edges.push(edge_id);
        }
    }

    for u in 0..n {
        solver.add_supply(u, 1);
        solver.add_supply(n + u, -1);
    }

    let status = solver.solve();
    assert!(status == Status::Optimal);

    let mut p = vec![0; n];
    for edge_id in &edges {
        if solver.get_directed_edge(*edge_id).flow == 1 {
            p[solver.get_directed_edge(*edge_id).from] = solver.get_directed_edge(*edge_id).to - n;
        }
    }
    println!("{}", solver.optimal_cost().unwrap_or(0));
    for i in 0..n {
        print!("{} ", p[i])
    }
    println!();
}