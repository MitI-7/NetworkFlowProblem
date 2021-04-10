use cost_scaling_push_relabel::{CostScalingPushRelabel, Status};
use std::fs::File;
use std::time::Instant;
use std::io::{BufRead, BufReader};


fn main() {
    // let input_file = "/mnt/c/Users/sakuya/src/Algorithm/verification/network_flow/minimum_cost_flow/cost_scaling_push_relabel/src/bin/netgen_lo_8_16a.min";
    // let input_file = "/mnt/c/Users/sakuya/src/Algorithm/verification/network_flow/minimum_cost_flow/cost_scaling_push_relabel/src/bin/netgen_sr_16a.min";
    let input_file = "C:/Users/sakuya/src/Algorithm/verification/network_flow/minimum_cost_flow/cost_scaling_push_relabel/src/bin/hand.in";
    // let input_file = "./netgen_sr_16a.min";

    let mut num_of_nodes = 0;
    let mut num_of_edges = 0;

    let mut solver = CostScalingPushRelabel::new(0);
    for result in BufReader::new(File::open(input_file).unwrap()).lines() {
        let line = result.unwrap();
        let v: Vec<&str> = line.split_whitespace().collect();
        if v[0] == "p" {
            num_of_nodes = v[2].parse().unwrap();
            num_of_edges = v[3].parse().unwrap();
            solver = CostScalingPushRelabel::new(num_of_nodes);
        }
        if v[0] == "n" {
            let u: usize = v[1].parse().unwrap();
            let s: i64 = v[2].parse().unwrap();
            solver.add_supply(u - 1, s);
        }
        if v[0] == "a" {
            let f: usize = v[1].parse().unwrap();
            let t: usize = v[2].parse().unwrap();
            let l: i64 = v[3].parse().unwrap();
            let u: i64 = v[4].parse().unwrap();
            let c: i64 = v[5].parse().unwrap();

            solver.add_directed_edge(f - 1, t - 1, l, u, c);
        }
    }
    println!("#nodes:{} #edges:{}", num_of_nodes, num_of_edges);

    let start = Instant::now();

    // solver.set_check_feasibility(false);
    let status = solver.solve();
    match status {
        Status::Optimal => {
            println!("cost:{}", solver.optimal_cost().unwrap_or(0));
        }
        _ => {
            println!("infeasible");
        }
    }

    let end = start.elapsed();
    println!("{}.{:03}秒経過しました。", end.as_secs(), end.subsec_nanos() / 1_000_000);

}