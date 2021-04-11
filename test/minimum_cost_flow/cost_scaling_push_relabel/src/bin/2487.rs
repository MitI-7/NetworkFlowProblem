// verification-helper: PROBLEM https://judge.u-aizu.ac.jp/onlinejudge/description.jsp?id=2487
use std::str::FromStr;
use std::io::*;
use std::collections::HashSet;
use std::collections::HashMap;


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
    let NAB: usize = read();
    let NBA: usize = read();

    let mut ta =  HashSet::new();
    let mut tb =  HashSet::new();
    let mut C1 = vec![0; NAB];
    let mut D1 = vec![0; NAB];
    let mut E1 = vec![0; NAB];
    for i in 0..NAB {
        let (c1, d1, e1) = (read(), read(), read());
        C1[i] = c1;
        D1[i] = d1;
        E1[i] = e1;
        ta.insert(d1);
        tb.insert(e1);
    }

    let mut C2 = vec![0; NBA];
    let mut D2 = vec![0; NBA];
    let mut E2 = vec![0; NBA];
    for i in 0..NBA {
        let (c2, d2, e2) = (read(), read(), read());
        C2[i] = c2;
        D2[i] = d2;
        E2[i] = e2;
        tb.insert(d2);
        ta.insert(e2);
    }

    let mut time_a: Vec<usize> = ta.into_iter().collect();
    time_a.sort();
    let mut time_b: Vec<usize> = tb.into_iter().collect();
    time_b.sort();

    let base = time_a.len();

    let mut time_index_a: HashMap<usize, usize> = HashMap::new();
    let mut time_index_b: HashMap<usize, usize> = HashMap::new();

    for i in 0..time_a.len() {
        time_index_a.insert(time_a[i], i);
    }
    for i in 0..time_b.len() {
        time_index_b.insert(time_b[i], base + i);
    }

    let mut solver = CostScalingPushRelabel::new(time_a.len() + time_b.len());

    // A
    for i in 0..time_a.len() - 1 {
        let from = time_index_a[&time_a[i]];
        let to = time_index_a[&time_a[i + 1]];
        solver.add_directed_edge(from, to, 0, 500, 0);
    }
    // B
    for i in 0..time_b.len() - 1 {
        let from = time_index_b[&time_b[i]];
        let to = time_index_b[&time_b[i + 1]];
        solver.add_directed_edge(from, to, 0, 500, 0);
    }

    // A -> B
    for i in 0..NAB {
        let from = time_index_a[&D1[i]];
        let to = time_index_b[&E1[i]];
        solver.add_directed_edge(from, to, 0, C1[i], -1);
    }
    // B -> A
    for i in 0..NBA {
        let from = time_index_b[&D2[i]];
        let to = time_index_a[&E2[i]];
        solver.add_directed_edge(from, to, 0, C2[i], 0);
    }

    let source = time_index_a[&time_a[0]];
    solver.add_supply(source, 1);
    let sink = time_index_b[&time_b[time_b.len() - 1]];
    solver.add_supply(sink, -1);

    match solver.solve() {
        Status::Optimal => println!("{}", -solver.optimal_cost().unwrap_or(0)),
        _ => println!("0"),
    }
}