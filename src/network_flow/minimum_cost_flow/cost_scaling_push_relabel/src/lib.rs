use std::collections::VecDeque;
use push_relabel::LowerBound;
use std::time::Instant;

#[derive(PartialEq)]
pub enum Status {
    NotSolved,
    Optimal,
    Feasible,
    Infeasible,
    Unbalanced,
    BadResult,
    BadCostRange,
}

#[derive(Clone)]
#[derive(Debug)]
struct Node {
    b: i64,
    excess: i64,
    potential: i64,
}

impl Node {
    pub fn new() -> Self {
        Node { b: 0, excess: 0, potential: 0 }
    }

    pub fn is_active(&self) -> bool {
        self.excess > 0
    }
}

#[derive(Clone)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    rev: usize,     // 逆辺のindex. graph[to][rev]でアクセスできる
    pub flow: i64,
    lower: i64,
    upper: i64,
    cost: i64,
    is_rev: bool,   // 逆辺かどうか
}

impl Edge {
    pub fn new(from: usize, to: usize, rev: usize, flow: i64, lower: i64, upper: i64, cost: i64, is_rev: bool) -> Self {
        Edge {
            from, to, rev, flow, lower, upper, cost, is_rev
        }
    }

    pub fn residual_capacity(&self) -> i64 {
        self.upper - self.flow
    }
}


pub struct CostScalingPushRelabel {
    num_of_nodes: usize,
    nodes: Vec<Node>,
    graph: Vec<Vec<Edge>>,
    active_nodes: VecDeque<usize>,
    gamma: i64,
    pos: Vec<(usize, usize)>,
    current_edges: Vec<usize>,  // current candidate to test for admissibility
    alpha: i64,
    cost_scaling_factor: i64,

    // status
    status: Status,
    optimal_cost: Option<i128>,

    // settings
    check_feasibility: bool,

    // debug
    num_discharge: i64,
    num_relabel: i64,
    num_test: i64,
}

#[allow(dead_code)]
impl CostScalingPushRelabel {
    pub fn new(num_of_nodes: usize) -> Self {
        let alpha = 5;
        assert!(alpha >= 2);
        CostScalingPushRelabel {
            num_of_nodes: num_of_nodes,
            nodes: vec![Node::new(); num_of_nodes],
            graph: vec![vec![]; num_of_nodes],
            active_nodes: VecDeque::new(),
            gamma: 0,
            pos: Vec::new(),
            current_edges: vec![0; num_of_nodes],
            alpha: alpha,   // it was usually between 8 and 24
            // cost_scaling_factor: 1 + alpha * num_of_nodes as i64,
            cost_scaling_factor: 3 + num_of_nodes as i64,

            status: Status::NotSolved,
            optimal_cost: None,

            check_feasibility: true,

            num_discharge: 0,
            num_relabel: 0,
            num_test: 0,
        }
    }

    pub fn add_directed_edge(&mut self, from: usize, to: usize, lower: i64, upper: i64, cost: i64) -> usize {
        assert!(lower <= upper);

        let e = self.graph[from].len();
        let re = if from == to {
            e + 1
        } else {
            self.graph[to].len()
        };

        let e1 = Edge::new(from, to, re, 0, lower, upper, cost, false);
        self.graph[from].push(e1);

        let e2 = Edge::new(to, from, e, 0, 0, -lower, -cost, true);
        self.graph[to].push(e2);

        self.gamma = i64::max(self.gamma, cost.abs());

        self.pos.push((from, e));
        self.pos.len() - 1
    }

    pub fn get_directed_edge(&self, i: usize) -> Edge {
        let (a, b) = self.pos[i];
        let e = &self.graph[a][b];
        Edge {
            from: e.from,
            to: e.to,
            rev: e.rev,
            flow: e.flow,
            lower: e.lower,
            upper: e.upper,
            cost: e.cost,
            is_rev: e.is_rev,
        }
    }

    pub fn add_supply(&mut self, node: usize, supply: i64) {
        self.nodes[node].b += supply;
        self.nodes[node].excess += supply;
    }

    pub fn set_check_feasibility(&mut self, check: bool) {
        self.check_feasibility = check;
    }

    pub fn solve(&mut self) -> Status {
        if self.is_unbalanced() {
            return Status::Unbalanced;
        }

        if self.check_feasibility && self.is_infeasible() {
            return Status::Infeasible;
        }

        let mut epsilon = i64::max(1, self.gamma * self.cost_scaling_factor);

        self.scale_cost();

        self.initialize();

        while {
            // do
            eprintln!("epsilon: {}", epsilon);
            epsilon = i64::max(epsilon / self.alpha, 1);

            let start = Instant::now();
            self.refine(epsilon);
            let end = start.elapsed();
            eprintln!("#time:{}.{:03}", end.as_secs(), end.subsec_nanos() / 1_000_000);
            // assert!(self.excess_is_valid());
            // assert!(self.is_feasible_flow());
            // assert!(self.is_epsilon_optimal(0, true));

            // eprintln!("#relabel:{}", self.num_relabel);
            // eprintln!("#discharge:{}", self.num_discharge);
            // eprintln!("#edge_test_count:{}", self.num_test);
            eprintln!();
            self.num_relabel = 0;
            self.num_discharge = 0;
            self.num_test = 0;


            // while
            self.status != Status::Infeasible && epsilon != 1
        } {}

        self.unscale_cost();

        if self.status == Status::Infeasible {
            return Status::Infeasible;
        }

        let mut cost = 0;
        for u in 0..self.num_of_nodes {
            for edge in self.graph[u].iter() {
                cost += edge.flow as i128 * edge.cost as i128;
            }
        }
        self.optimal_cost = Some(cost / 2);

        self.status = Status::Optimal;
        Status::Optimal
    }

    // TODO
    pub fn solve_max_flow_with_min_cost() {}

    pub fn optimal_cost(&mut self) -> Option<i128> {
        self.optimal_cost
    }

    fn scale_cost(&mut self) {
        for u in 0..self.num_of_nodes {
            for edge in self.graph[u].iter_mut() {
                edge.cost *= self.cost_scaling_factor;
            }
        }
    }

    fn unscale_cost(&mut self) {
        for u in 0..self.num_of_nodes {
            for edge in self.graph[u].iter_mut() {
                edge.cost /= self.cost_scaling_factor;
            }
        }
    }

    fn is_unbalanced(&self) -> bool {
        let mut total = 0;
        for u in 0..self.num_of_nodes {
            total += self.nodes[u].b;
        }
        total != 0
    }

    fn is_infeasible(&self) -> bool {
        let mut solver = LowerBound::new(self.num_of_nodes);

        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                if !edge.is_rev {
                    solver.add_edge(edge.from, edge.to, edge.lower, edge.upper);
                }
            }
        }

        for u in 0..self.num_of_nodes {
            solver.add_supply(u, self.nodes[u].b);
        }
        let status = solver.solve();
        !status
    }

    fn initialize(&mut self) {
        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                if !edge.is_rev {
                    let flow = edge.lower;
                    self.push_flow(u, i, flow);
                }
            }
        }
    }

    // make epsilon-optimal flow
    fn refine(&mut self, epsilon: i64) {
        // make 0-optimal pseudo flow
        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                if edge.is_rev {
                    continue;
                }

                let reduced_cost = self.reduced_cost(&edge);
                if reduced_cost < 0 {
                    // 流量を上界にする
                    let flow = edge.residual_capacity();
                    if flow != 0 {
                        self.push_flow(u, i, flow);
                    }
                    assert_eq!(self.graph[u][i].flow, self.graph[u][i].upper);
                } else if reduced_cost > 0 {
                    // 流量を下界にする
                    let flow = edge.lower - edge.flow;
                    if flow != 0 {
                        self.push_flow(u, i, flow);
                    }
                    assert_eq!(self.graph[u][i].flow, self.graph[u][i].lower);
                }
            }
        }
        // assert!(self.is_epsilon_optimal(0));

        for u in 0..self.num_of_nodes {
            self.current_edges[u] = 0;
        }

        assert_eq!(self.active_nodes.len(), 0);
        for u in 0..self.nodes.len() {
            if self.nodes[u].is_active() {
                self.active_nodes.push_back(u);
            }
        }

        // 0-optimal pseudo flow -> epsilon-optimal feasible flow
        while let Some(u) = self.active_nodes.pop_back() {
            self.discharge(u, epsilon);

            if self.status == Status::Infeasible {
                return;
            }
        }
    }

    fn discharge(&mut self, u: usize, epsilon: i64) {
        self.num_discharge += 1;

        while self.status != Status::Infeasible && self.nodes[u].is_active() {
            self.push(u, epsilon);
            if self.nodes[u].is_active() {
                assert_eq!(self.current_edges[u], self.graph[u].len());
                self.relabel(u, epsilon);
            }
        }
    }

    fn push_flow(&mut self, u: usize, i: usize, flow: i64) {
        if flow == 0 {
            return;
        }

        let to = self.graph[u][i].to;
        let from = self.graph[u][i].from;
        let rev = self.graph[u][i].rev;

        self.graph[u][i].flow += flow;
        self.graph[to][rev].flow -= flow;
        self.nodes[from].excess -= flow;
        self.nodes[to].excess += flow;
    }

    fn reduced_cost(&self, edge: &Edge) -> i64 {
        edge.cost + self.nodes[edge.from].potential - self.nodes[edge.to].potential
    }

    fn is_admissible(&self, edge: &Edge, _epsilon: i64) -> bool {
        self.reduced_cost(edge) < 0
    }

    // uから隣接ノードにpushする
    fn push(&mut self, u: usize, epsilon: i64) {
        assert!(self.nodes[u].is_active());

        for i in self.current_edges[u]..self.graph[u].len() {
            self.num_test += 1;
            let edge = &self.graph[u][i];
            if edge.residual_capacity() <= 0 {
                continue;
            }

            if self.is_admissible(&edge, epsilon) {
                let to = edge.to;

                if !self.look_ahead(to, epsilon) {
                    if !self.is_admissible(&self.graph[u][i], epsilon) {
                        continue;
                    }
                }

                let flow = i64::min(self.graph[u][i].residual_capacity(), self.nodes[u].excess);
                self.push_flow(u, i, flow);

                // toが新たにactiveになった
                if self.nodes[to].excess > 0 && self.nodes[to].excess <= flow {
                    self.active_nodes.push_back(to);
                }

                if !self.nodes[u].is_active() {
                    self.current_edges[u] = i;
                    return;
                }
            }
        }

        // node has no admissible edge
        self.current_edges[u] = self.graph[u].len();
    }

    // uのpotentialを修正してadmissible edgeをふやす
    fn relabel(&mut self, u: usize, epsilon: i64) {
        self.num_relabel += 1;
        let guaranteed_new_potential = self.nodes[u].potential - epsilon;

        let mut maxi_potential = i64::MIN;
        let mut previous_maxi_potential = i64::MIN;
        let mut current_edges_for_u = 0;

        for i in 0..self.graph[u].len() {
            self.num_test += 1;
            if self.graph[u][i].residual_capacity() <= 0 {
                continue;
            }
            let to = self.graph[u][i].to;
            let cost = self.graph[u][i].cost;

            // (u->to)のreduced_cost(= cost + potential[u] - potential[to])を0にするpotential
            let new_potential = self.nodes[to].potential - cost;
            if new_potential > maxi_potential {

                // epsilon引いただけでadmissible edgeができる
                if new_potential > guaranteed_new_potential {
                    self.nodes[u].potential = guaranteed_new_potential;
                    self.current_edges[u] = i;
                    return;
                }

                previous_maxi_potential = maxi_potential;
                maxi_potential = new_potential;
                current_edges_for_u = i;
            }
        }

        // ポテンシャルをさげてもadmissible edgeをつくることができない
        if maxi_potential == i64::MIN {
            if self.nodes[u].excess != 0 {
                self.status = Status::Infeasible;
                return;
            } else {
                // すきなだけpotentialをさげることができるが，とりあえずguaranteed_new_potentialをいれておく
                self.nodes[u].potential = guaranteed_new_potential;
                self.current_edges[u] = 0;
            }
            return;
        }

        // epsilonさげただけじゃだめだけどもっとさげればadmissible edgeを作れる
        let new_potential = maxi_potential - epsilon;
        self.nodes[u].potential = new_potential;

        if previous_maxi_potential <= new_potential {
            // previous_maxi_potentialをつくったedgeからみればいい
            self.current_edges[u] = current_edges_for_u;
        } else {
            self.current_edges[u] = 0;
        }
    }

    fn look_ahead(&mut self, u: usize, epsilon: i64) -> bool {
        if self.nodes[u].excess < 0 {
            return true;
        }

        // admissibleがあればok
        for i in self.current_edges[u]..self.graph[u].len() {
            self.num_test += 1;
            if self.graph[u][i].residual_capacity() <= 0 {
                continue;
            }

            if self.is_admissible(&self.graph[u][i], epsilon) {
                self.current_edges[u] = i;
                return true;
            }
        }

        self.relabel(u, epsilon);
        false
    }

    pub fn calculate_potential(&self) -> Vec<i64> {
        let mut p = vec![0; self.num_of_nodes];
        // bellman-ford
        // 最適flowに対して，残余ネットワーク上で最短経路問題を解く
        for _ in 0..self.num_of_nodes {
            let mut update = false;
            for u in 0..self.num_of_nodes {
                for e in &self.graph[u] {
                    if e.residual_capacity() > 0 {
                        let new_pot = p[u] + e.cost;
                        if new_pot < p[e.to] {
                            p[e.to] = new_pot;
                            update = true;
                        }
                    }
                }
            }
            if !update {
                break;
            }
        }
        p
    }

    // debug
    fn print_excess(&self) {
        print!("excess: ");
        for u in 0..self.num_of_nodes {
            print!("{} ", self.nodes[u].excess);
        }
        println!();
    }

    fn print_potential(&self) {
        print!("potential: ");
        for u in 0..self.num_of_nodes {
            print!("{} ", self.nodes[u].potential);
        }
        println!();
    }

    pub fn show(&self) {
        for u in 0..self.num_of_nodes {
            for e in &self.graph[u] {
                if !e.is_rev {
                    println!("{} -> {}(lower:{} flow:{} upper:{} cost:{} rest:{})", u, e.to, e.flow, e.flow, e.upper, e.cost, e.residual_capacity());
                }
            }
        }
    }

    fn excess_is_valid(&self) -> bool {
        let mut excess = vec![0; self.num_of_nodes];
        for u in 0..self.num_of_nodes {
            excess[u] += self.nodes[u].b;
            for e in &self.graph[u] {
                if !e.is_rev {
                    excess[u] -= e.flow;
                    excess[e.to] += e.flow;
                }
            }
        }

        for u in 0..self.num_of_nodes {
            if self.nodes[u].excess != excess[u] {
                return false;
            }
        }

        true
    }

    fn is_epsilon_optimal(&self, epsilon: i64) -> bool {
        // assert!(epsilon > 0);

        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                if edge.is_rev {
                    continue;
                }

                let reduced_cost = self.reduced_cost(edge);
                if reduced_cost > epsilon {
                    if edge.flow != edge.lower {
                        return false;
                    }
                }
                if -epsilon <= reduced_cost && reduced_cost <= epsilon {
                    if !(edge.lower <= edge.flow && edge.flow <= edge.upper) {
                        return false;
                    }
                }
                if reduced_cost < epsilon {
                    if edge.flow != edge.upper {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn is_feasible_flow(&self) -> bool {
        let mut e = vec![0; self.num_of_nodes];
        for u in 0..self.num_of_nodes {
            for edge in &self.graph[u] {
                if !edge.is_rev {
                    // check capacity constraint
                    if edge.flow < edge.lower || edge.flow > edge.upper {
                        return false;
                    }
                    e[edge.from] += edge.flow;
                    e[edge.to] -= edge.flow;
                }
            }
        }

        // check flow conservation constraint
        for u in 0..self.num_of_nodes {
            if self.nodes[u].b != e[u] {
                return false;
            }
        }

        true
    }
}
