use num::{CheckedMul, FromPrimitive, ToPrimitive};
use num_traits::NumAssign;
use push_relabel::LowerBound;
use std::collections::VecDeque;
use std::fmt::{Debug, Display};
use std::time::Instant;

pub trait Flow: 'static + Copy + Ord + Display + Debug + BoundedBelow + BoundedAbove + FromPrimitive + ToPrimitive + NumAssign + CheckedMul {}

pub trait Zero {
    fn zero() -> Self;
}

pub trait One {
    fn one() -> Self;
}

pub trait BoundedBelow {
    fn min_value() -> Self;
}

pub trait BoundedAbove {
    fn max_value() -> Self;
}

macro_rules! impl_integral {
    ($($ty:ty),*) => {
        $(
            impl Zero for $ty {
                #[inline]
                fn zero() -> Self {
                    0
                }
            }

            impl One for $ty {
                #[inline]
                fn one() -> Self {
                    1
                }
            }

            impl BoundedBelow for $ty {
                #[inline]
                fn min_value() -> Self {
                    Self::min_value()
                }
            }

            impl BoundedAbove for $ty {
                #[inline]
                fn max_value() -> Self {
                    Self::max_value()
                }
            }

            impl Flow for $ty {}
        )*
    };
}

impl_integral!(i8, i16, i32, i64, i128);

#[derive(PartialEq, Debug)]
pub enum Status {
    NotSolved,
    Optimal,
    Feasible,
    Infeasible,
    Unbalanced,
    BadResult,
    BadCostRange,
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct EdgeId(usize, usize);

pub struct Edge<F: Flow> {
    pub from: usize,
    pub to: usize,
    pub flow: F,
    pub lower: F,
    pub upper: F,
    pub cost: F,
}

impl<F: Flow> Edge<F> {
    pub fn new(from: usize, to: usize, flow: F, lower: F, upper: F, cost: F) -> Self {
        Edge { from, to, flow, lower, upper, cost }
    }
}

#[derive(Clone)]
struct InternalEdge<F: Flow> {
    to: usize,
    rev: usize, // 逆辺のindex. graph[to][rev]でアクセスできる
    flow: F,
    lower: F,
    upper: F,
    cost: F,
}

impl<F: Flow> InternalEdge<F> {
    pub fn new(to: usize, rev: usize, flow: F, lower: F, upper: F, cost: F) -> Self {
        InternalEdge { to, rev, flow, lower, upper, cost }
    }

    pub fn residual_capacity(&self) -> F {
        self.upper - self.flow
    }
}

pub struct CostScalingPushRelabel<F: Flow> {
    num_of_nodes: usize,
    graph: Vec<Vec<InternalEdge<F>>>,
    active_nodes: VecDeque<usize>,
    gamma: F,                  // maximum absolute value of any edge cost
    current_edges: Vec<usize>, // current candidate to test for admissibility

    // Node
    initial_excess: Vec<F>,
    excess: Vec<F>,
    potentials: Vec<F>,

    // Edge
    is_rev: Vec<Vec<bool>>, // TODO: remove

    // status
    status: Status,
    optimal_cost: Option<i128>,
    num_relabel: u64,

    // settings
    alpha: F,
    cost_scaling_factor: F,
    check_feasibility: bool,
    use_look_ahead_heuristic: bool,
    use_price_update_heuristic: bool,
    use_price_refinement_heuristic: bool,
}

#[allow(dead_code)]
impl<F: Flow + std::ops::Neg<Output = F>> CostScalingPushRelabel<F> {
    pub fn new(num_of_nodes: usize) -> Self {
        CostScalingPushRelabel {
            num_of_nodes: num_of_nodes,
            graph: vec![vec![]; num_of_nodes],
            active_nodes: VecDeque::new(),
            gamma: F::zero(),
            current_edges: vec![0; num_of_nodes],

            // Node
            initial_excess: vec![F::zero(); num_of_nodes],
            excess: vec![F::zero(); num_of_nodes],
            potentials: vec![F::zero(); num_of_nodes],

            // Edge
            is_rev: vec![vec![]; num_of_nodes],

            status: Status::NotSolved,
            optimal_cost: None,
            num_relabel: 0,

            alpha: F::from_usize(5).unwrap(),
            cost_scaling_factor: F::zero(),
            check_feasibility: true,
            use_look_ahead_heuristic: true,
            use_price_update_heuristic: false,
            use_price_refinement_heuristic: false,
        }
    }

    pub fn add_directed_edge(&mut self, from: usize, to: usize, lower: F, upper: F, cost: F) -> EdgeId {
        assert!(lower <= upper);
        assert!(from < self.num_of_nodes);
        assert!(to < self.num_of_nodes);

        let e = self.graph[from].len();
        let re = if from == to { e + 1 } else { self.graph[to].len() };

        let e1 = InternalEdge::new(to, re, F::zero(), lower, upper, cost);
        self.graph[from].push(e1);
        self.is_rev[from].push(false);

        let e2 = InternalEdge::new(from, e, F::zero(), F::zero(), -lower, -cost);
        self.graph[to].push(e2);
        self.is_rev[to].push(true);

        if cost < F::zero() {
            self.gamma = F::max(self.gamma, -cost);
        } else {
            self.gamma = F::max(self.gamma, cost);
        }

        EdgeId(from, e)
    }

    pub fn get_directed_edge(&self, edge_id: EdgeId) -> Edge<F> {
        let e = &self.graph[edge_id.0][edge_id.1];
        Edge { from: edge_id.0, to: e.to, flow: e.flow, lower: e.lower, upper: e.upper, cost: e.cost }
    }

    pub fn get_potential(&self) -> Vec<F> {
        self.potentials.clone()
    }

    pub fn add_supply(&mut self, node: usize, supply: F) {
        self.initial_excess[node] += supply;
        self.excess[node] += supply;
    }

    pub fn set_alpha(&mut self, alpha: F) {
        assert!(alpha >= F::from_i32(2).unwrap());
        self.alpha = alpha;
    }

    pub fn set_check_feasibility(&mut self, check: bool) {
        self.check_feasibility = check;
    }

    pub fn use_look_ahead_heuristic(&mut self, b: bool) {
        self.use_look_ahead_heuristic = b;
    }

    pub fn solve(&mut self) -> Status {
        self.status = Status::NotSolved;

        self.cost_scaling_factor = self.alpha * F::from_usize(self.num_of_nodes).unwrap();

        if self.num_of_nodes == 0 {
            self.status = Status::Optimal;
            return Status::Optimal;
        }

        if self.is_unbalanced() {
            return Status::Unbalanced;
        }

        if self.check_feasibility && self.is_infeasible() {
            return Status::Infeasible;
        }

        let mut epsilon;
        match self.gamma.checked_mul(&self.cost_scaling_factor) {
            Some(p) => epsilon = F::max(F::one(), p),
            None => {
                self.status = Status::BadCostRange;
                return Status::BadCostRange;
            }
        }

        self.scale_cost();
        if self.status == Status::BadCostRange {
            self.status = Status::BadCostRange;
            return Status::BadCostRange;
        }

        self.initialize();

        let mut num_loop = 0;
        loop {
            let start = Instant::now();

            num_loop += 1;
            epsilon = F::max(epsilon / self.alpha, F::one());
            eprintln!("epsilon: {}", epsilon);

            if self.use_price_refinement_heuristic && num_loop > 1 && epsilon != F::one() {
                if self.price_refinement(epsilon) {
                    continue;
                }
            }

            self.refine(epsilon);
            let end = start.elapsed();
            eprintln!("#time:{}.{:03}", end.as_secs(), end.subsec_nanos() / 1_000_000);
            // assert!(self.excess_is_valid());
            // assert!(self.is_feasible_flow());
            // assert!(self.is_epsilon_optimal(0, true));

            if self.status == Status::Infeasible || epsilon == F::one() {
                break;
            }
        }

        self.unscale_cost();

        if self.status == Status::Infeasible {
            return Status::Infeasible;
        }

        let mut cost = 0;
        for u in 0..self.num_of_nodes {
            for edge in self.graph[u].iter() {
                cost += F::to_i128(&edge.flow).unwrap() * F::to_i128(&edge.cost).unwrap();
            }
        }
        self.optimal_cost = Some(cost / 2);

        self.status = Status::Optimal;
        // self.update_potential();

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
                match edge.cost.checked_mul(&self.cost_scaling_factor) {
                    Some(p) => {
                        edge.cost = p;
                    }
                    None => {
                        self.status = Status::BadCostRange;
                        return;
                    }
                }
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
        let mut total = F::zero();
        for u in 0..self.num_of_nodes {
            total += self.initial_excess[u];
        }
        total != F::zero()
    }

    fn is_infeasible(&self) -> bool {
        let mut solver = LowerBound::new(self.num_of_nodes);

        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                if !self.is_rev[u][i] {
                    solver.add_edge(u, edge.to, F::to_i64(&edge.lower).unwrap(), F::to_i64(&edge.upper).unwrap());
                }
            }
        }

        for u in 0..self.num_of_nodes {
            solver.add_supply(u, F::to_i64(&self.initial_excess[u]).unwrap());
        }
        let status = solver.solve();
        !status
    }

    fn initialize(&mut self) {
        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                let flow = edge.lower;
                self.push_flow(u, i, flow);
            }
        }
    }

    // make epsilon-optimal flow
    fn refine(&mut self, epsilon: F) {
        // make 0-optimal pseudo flow
        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];

                let reduced_cost = self.reduced_cost(u, &edge);
                if reduced_cost < F::zero() {
                    // 流量を上界にする
                    let flow = edge.residual_capacity();
                    if flow != F::zero() {
                        self.push_flow(u, i, flow);
                    }
                    assert_eq!(self.graph[u][i].flow, self.graph[u][i].upper);
                }
            }
        }
        // assert!(self.is_epsilon_optimal(F::zero()));

        for u in 0..self.num_of_nodes {
            self.current_edges[u] = 0;
        }

        assert_eq!(self.active_nodes.len(), 0);
        for u in 0..self.num_of_nodes {
            if self.is_active(u) {
                self.active_nodes.push_back(u);
            }
        }

        // 0-optimal pseudo flow -> epsilon-optimal feasible flow
        while let Some(u) = self.active_nodes.pop_back() {
            if self.use_price_update_heuristic {
                if self.num_relabel > self.num_of_nodes as u64 {
                    self.price_update(epsilon);
                    self.num_relabel = 0;
                    eprintln!("do price update");
                }
            }

            self.discharge(u, epsilon);

            if self.status == Status::Infeasible {
                return;
            }
        }
    }

    fn discharge(&mut self, u: usize, epsilon: F) {
        while self.status != Status::Infeasible && self.is_active(u) {
            self.push(u, epsilon);
            if self.is_active(u) {
                assert_eq!(self.current_edges[u], self.graph[u].len());
                self.relabel(u, epsilon);
            }
        }
    }

    fn push_flow(&mut self, u: usize, i: usize, flow: F) {
        if flow == F::zero() {
            return;
        }

        let to = self.graph[u][i].to;
        let from = u;
        let rev = self.graph[u][i].rev;

        self.graph[u][i].flow += flow;
        self.graph[to][rev].flow -= flow;
        self.excess[from] -= flow;
        self.excess[to] += flow;
    }

    fn reduced_cost(&self, u: usize, edge: &InternalEdge<F>) -> F {
        edge.cost + self.potentials[u] - self.potentials[edge.to]
    }

    fn is_admissible(&self, u: usize, edge: &InternalEdge<F>, _epsilon: F) -> bool {
        self.reduced_cost(u, edge) < F::zero()
    }

    fn is_active(&self, u: usize) -> bool {
        self.excess[u] > F::zero()
    }

    // uから隣接ノードにpushする
    fn push(&mut self, u: usize, epsilon: F) {
        assert!(self.is_active(u));

        for i in self.current_edges[u]..self.graph[u].len() {
            let edge = &self.graph[u][i];
            if edge.residual_capacity() <= F::zero() {
                continue;
            }

            if self.is_admissible(u, &edge, epsilon) {
                let to = edge.to;

                if self.use_look_ahead_heuristic {
                    if !self.look_ahead(to, epsilon) {
                        // toがrelabelしているので，edgeがadmissibleかチェックする
                        if !self.is_admissible(u, &self.graph[u][i], epsilon) {
                            continue;
                        }
                    }
                }

                let flow = F::min(self.graph[u][i].residual_capacity(), self.excess[u]);
                self.push_flow(u, i, flow);

                // toが新たにactiveになった
                if self.is_active(to) && self.excess[to] <= flow {
                    self.active_nodes.push_back(to);
                }

                if !self.is_active(u) {
                    self.current_edges[u] = i;
                    return;
                }
            }
        }

        // node has no admissible edge
        self.current_edges[u] = self.graph[u].len();
    }

    // uのpotentialを修正してadmissible edgeをふやす
    fn relabel(&mut self, u: usize, epsilon: F) {
        let guaranteed_new_potential = self.potentials[u] - epsilon;

        let mut maxi_potential = F::min_value();
        let mut previous_maxi_potential = F::min_value();
        let mut current_edges_for_u = 0;

        for (i, edge) in self.graph[u].iter().enumerate() {
            if edge.residual_capacity() <= F::zero() {
                continue;
            }

            // (u->to)のreduced_cost(= cost + potential[u] - potential[to])を0にするpotential
            let new_potential = self.potentials[edge.to] - edge.cost;
            if new_potential > maxi_potential {
                // epsilon引いただけでadmissible edgeができる
                if new_potential > guaranteed_new_potential {
                    self.potentials[u] = guaranteed_new_potential;
                    self.current_edges[u] = i;
                    return;
                }

                previous_maxi_potential = maxi_potential;
                maxi_potential = new_potential;
                current_edges_for_u = i;
            }
        }

        // ポテンシャルをさげてもadmissible edgeをつくることができない
        if maxi_potential == F::min_value() {
            if self.excess[u] != F::zero() {
                self.status = Status::Infeasible;
                return;
            } else {
                // すきなだけpotentialをさげることができるが，とりあえずguaranteed_new_potentialをいれておく
                self.potentials[u] = guaranteed_new_potential;
                self.current_edges[u] = 0;
            }
            return;
        }

        // epsilonさげただけじゃだめだけどもっとさげればadmissible edgeを作れる
        let new_potential = maxi_potential - epsilon;
        self.potentials[u] = new_potential;

        if previous_maxi_potential <= new_potential {
            // previous_maxi_potentialをつくったedgeからみればいい
            self.current_edges[u] = current_edges_for_u;
        } else {
            self.current_edges[u] = 0;
        }
    }

    // check whether u has an outgoing admissible arc or whether excess[u] < 0
    fn look_ahead(&mut self, u: usize, epsilon: F) -> bool {
        if self.excess[u] < F::zero() {
            return true;
        }

        // admissibleがあればok
        for i in self.current_edges[u]..self.graph[u].len() {
            let edge = &self.graph[u][i];
            if edge.residual_capacity() <= F::zero() {
                continue;
            }

            if self.is_admissible(u, &edge, epsilon) {
                self.current_edges[u] = i;
                return true;
            }
        }

        self.relabel(u, epsilon);
        false
    }

    // check whether now flow is epsilon-optimal or not
    // o(nm)
    fn price_refinement(&mut self, epsilon: F) -> bool {
        let mut p = vec![F::zero(); self.num_of_nodes];

        // bellman-ford
        let mut update = false;
        for _ in 0..self.num_of_nodes {
            for u in 0..self.num_of_nodes {
                for edge in self.graph[u].iter() {
                    if edge.residual_capacity() > F::zero() {
                        let new_pot = p[u] + edge.cost + epsilon;
                        if new_pot < p[edge.to] {
                            p[edge.to] = new_pot;
                            update = true;
                        }
                    }
                }
            }
            if !update {
                break;
            }
        }

        // have negative cycle
        if update {
            return false;
        }

        // update potential
        for u in 0..self.num_of_nodes {
            self.potentials[u] = p[u];
        }
        true
    }

    fn price_update_naive(&mut self, epsilon: F) {
        let mut s = Vec::new();
        let mut in_s = vec![false; self.num_of_nodes];
        let mut total_s = F::zero();
        for u in 0..self.num_of_nodes {
            if self.excess[u] < F::zero() {
                s.push(u);
                in_s[u] = true;
                total_s += self.excess[u];
            }
        }

        while total_s < F::zero() {
            let mut new_s = Vec::new();
            for v in &s {
                for edge in &self.graph[*v] {
                    let u = edge.to;
                    if in_s[u] {
                        continue;
                    }
                    let rev_edge = &self.graph[u][edge.rev]; // u -> v
                    if self.is_admissible(u, rev_edge, epsilon) {
                        new_s.push(u);
                        in_s[u] = true;
                        total_s += self.excess[u];
                    }
                }
            }

            if total_s < F::zero() {
                break;
            }

            for u in 0..self.num_of_nodes {
                if !in_s[u] {
                    self.potentials[u] -= epsilon;
                }
            }

            for u in new_s {
                s.push(u);
            }
        }
    }

    fn price_update(&mut self, epsilon: F) {
        let inf = self.num_of_nodes;

        // deficit nodesからadmissible edgesを逆にたどって到達できるnodesを求める
        // そのとき，何stepで到達できるかをメモしておく
        let mut buckets = vec![Vec::new(); self.num_of_nodes + 10];
        let mut belonging_bucket = vec![inf as i64; self.num_of_nodes + 10];
        let mut total_s = F::zero();

        for u in 0..self.num_of_nodes {
            if self.excess[u] < F::zero() {
                buckets[0].push(u);
                belonging_bucket[u] = 0;
                total_s += self.excess[u];
            }
        }

        let mut in_s = vec![false; self.num_of_nodes + 10];
        let mut labels = vec![inf; self.num_of_nodes + 10];
        let mut last = 0;
        let mut i = 0_i64;
        while total_s < F::zero() {
            if i >= buckets.len() as i64 {
                break;
            }
            while let Some(v) = buckets[i as usize].pop() {
                // v is deleted
                if belonging_bucket[v] != i {
                    continue;
                }

                for edge in &self.graph[v] {
                    let u = edge.to;
                    if in_s[u] {
                        continue;
                    }

                    let rev_edge = &self.graph[u][edge.rev]; // u -> v
                    let x = (self.reduced_cost(u, rev_edge) / epsilon + F::one());
                    // eprintln!("x:{}", x);
                    let mut new_distance = F::to_i64(&x).unwrap();
                    new_distance = i64::min(i64::max(new_distance, 1), inf as i64);
                    if new_distance < belonging_bucket[u] {
                        belonging_bucket[u] = new_distance;
                        buckets[new_distance as usize].push(u);
                    }
                }

                in_s[v] = true;
                labels[v] = i as usize;
                total_s += self.excess[v];

                last = i;
            }
            i += 1;
        }

        // update potentials
        for u in 0..self.num_of_nodes {
            if labels[u] < inf {
                self.current_edges[u] = 0;
                self.potentials[u] -= epsilon * F::from_usize(labels[u]).unwrap();
            } else {
                self.current_edges[u] = 0;
                self.potentials[u] -= epsilon * F::from_i64(last + 1).unwrap();
            }
        }
    }

    pub fn update_potential(&mut self) {
        assert_eq!(self.status, Status::Optimal);
        use std::collections::BinaryHeap;

        self.potentials = vec![F::zero(); self.num_of_nodes];
        let mut heap = BinaryHeap::new();

        for u in 0..self.num_of_nodes {
            heap.push((F::zero(), u));
        }

        // dijkstra
        // optimal flow does not have negative cycle in residual network
        while let Some((cost, u)) = heap.pop() {
            if cost > self.potentials[u] {
                continue;
            }

            for edge in &self.graph[u] {
                if edge.residual_capacity() > F::zero() {
                    let new_cost = cost + edge.cost;
                    let v = edge.to;

                    if new_cost < self.potentials[v] {
                        heap.push((new_cost, v));
                        self.potentials[v] = new_cost;
                    }
                }
            }
        }
    }

    pub fn increase_capacity_unit(&mut self, edge_id: EdgeId) {
        assert_eq!(self.status, Status::Optimal);
        let (u, i) = (edge_id.0, edge_id.1);

        self.graph[u][i].upper += F::one();
        if self.graph[u][i].flow < self.graph[u][i].upper - F::one() {
            return;
        }

        self.update_potential();

        // it satisfies the reduced cost optimality conditions
        if self.reduced_cost(u, &self.graph[u][i]) >= F::zero() {
            return;
        }

        // 流量を上界にする
        self.push_flow(u, i, F::one());
        assert_eq!(self.graph[u][i].flow, self.graph[u][i].upper);

        // find shortest path from v to u
        let v = self.graph[u][i].to;
    }

    pub fn decrease_capacity(&mut self, edge_id: EdgeId) {
        assert_eq!(self.status, Status::Optimal);
        assert!(self.graph[edge_id.0][edge_id.1].upper >= F::one());

        self.graph[edge_id.0][edge_id.1].upper -= F::one();

        let edge = &self.graph[edge_id.0][edge_id.1];

        if edge.flow <= edge.upper {
            return;
        }
    }

    // debug
    fn print_excess(&self) {
        print!("excess: ");
        for u in 0..self.num_of_nodes {
            print!("{} ", self.excess[u]);
        }
        println!();
    }

    fn print_potential(&self) {
        print!("potential: ");
        for u in 0..self.num_of_nodes {
            print!("{} ", self.potentials[u]);
        }
        println!();
    }

    pub fn show(&self) {
        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let e = &self.graph[u][i];
                if !self.is_rev[u][i] {
                    println!("{} -> {}(lower:{} flow:{} upper:{} cost:{} rest:{})", u, e.to, e.flow, e.flow, e.upper, e.cost, e.residual_capacity());
                }
            }
        }
    }

    fn excess_is_valid(&self) -> bool {
        let mut e = vec![F::zero(); self.num_of_nodes];
        for u in 0..self.num_of_nodes {
            e[u] += self.initial_excess[u];
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                if !self.is_rev[u][i] {
                    e[u] -= edge.flow;
                    e[edge.to] += edge.flow;
                }
            }
        }

        for u in 0..self.num_of_nodes {
            if self.excess[u] != e[u] {
                return false;
            }
        }

        true
    }

    fn is_epsilon_optimal(&self, epsilon: F) -> bool {
        // assert!(epsilon > 0);

        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                if self.is_rev[u][i] {
                    continue;
                }

                let reduced_cost = self.reduced_cost(u, edge);
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
        let mut e = vec![F::zero(); self.num_of_nodes];
        for u in 0..self.num_of_nodes {
            for (i, edge) in self.graph[u].iter().enumerate() {
                if !self.is_rev[u][i] {
                    // check capacity constraint
                    if edge.flow < edge.lower || edge.flow > edge.upper {
                        return false;
                    }
                    e[u] += edge.flow;
                    e[edge.to] -= edge.flow;
                }
            }
        }

        // check flow conservation constraint
        for u in 0..self.num_of_nodes {
            if self.initial_excess[u] != e[u] {
                return false;
            }
        }

        true
    }

    fn is_feasible_potential(&self) -> bool {
        for u in 0..self.num_of_nodes {
            for (i, edge) in self.graph[u].iter().enumerate() {
                if !self.is_rev[u][i] {
                    let v = edge.to;
                    if self.potentials[u] + edge.cost < self.potentials[v] {
                        return false;
                    }
                }
            }
        }

        true
    }
}
