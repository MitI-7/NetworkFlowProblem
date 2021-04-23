use cost_scaling_push_relabel::{CostScalingPushRelabel, Status};
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();

    let input_file = &args[1];

    let mut num_of_nodes: usize = 0;
    let mut num_of_edges: usize = 0;
    for result in BufReader::new(File::open(input_file).unwrap()).lines() {
        let line = result.unwrap();
        let v: Vec<&str> = line.split_whitespace().collect();
        if v[0] == "p" {
            num_of_nodes = v[2].parse().unwrap();
            num_of_edges = v[3].parse().unwrap();
        }
    }

    let mut solver: CostScalingPushRelabel<i64> = CostScalingPushRelabel::new(num_of_nodes);
    solver.set_check_feasibility(false);

    // let mut vi = vec![vec![(-1, -100, -1); num_of_nodes]; num_of_nodes];

    for result in BufReader::new(File::open(input_file).unwrap()).lines() {
        let line = result.unwrap();
        let v: Vec<&str> = line.split_whitespace().collect();

        if v[0] == "n" {
            let u: usize = v[1].parse().unwrap();
            let s = v[2].parse().unwrap();
            solver.add_supply(u - 1, s);
        }

        if v[0] == "a" {
            let f: usize = v[1].parse().unwrap();
            let t: usize = v[2].parse().unwrap();
            let l = v[3].parse().unwrap();
            let u = v[4].parse().unwrap();
            let c = v[5].parse().unwrap();
            solver.add_directed_edge(f - 1, t - 1, l, u, c);
            //         vi[f - 1][t - 1] = (l, u, c);
        }
    }

    // for result in BufReader::new(File::open("C:/Users/sakuya/src/or-tools/out2.txt").unwrap()).lines() {
    //     let line = result.unwrap();
    //     let v: Vec<&str> = line.split_terminator(",").collect();
    //     let f: usize = v[0].parse().unwrap();
    //     let t: usize = v[1].parse().unwrap();
    //     let cap = v[2].parse().unwrap();
    //     let cos = v[3].parse().unwrap();
    //     solver.add_directed_edge(f, t, 0, cap, cos);
    // }

    // solver.add_supply(8192, 148449);
    // solver.add_supply(8193, -148449);
    //
    eprintln!("#nodes:{} #edges:{}", num_of_nodes, num_of_edges);

    let start = Instant::now();
    let status = solver.solve();
    let end = start.elapsed();
    println!("{}.{:03}", end.as_secs(), end.subsec_nanos() / 1_000_000);

    match status {
        Status::Optimal => {
            println!("{}", solver.optimal_cost().unwrap_or(0));
        }
        Status::BadCostRange => {
            println!("BadCostRange");
        }
        _ => {
            println!("infeasible");
        }
    }
}
