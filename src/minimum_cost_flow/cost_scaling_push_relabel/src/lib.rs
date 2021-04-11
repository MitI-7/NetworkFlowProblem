use std::collections::VecDeque;
use push_relabel::LowerBound;
use std::time::Instant;
use std::fmt::Display;
use num_traits::NumCast;
use num::{ToPrimitive, FromPrimitive};
use num_traits::{NumAssign};
use std::{
    fmt,
    iter::{Product, Sum},
    ops::{
        Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div,
        DivAssign, Mul, MulAssign, Not, Rem, RemAssign, Shl, ShlAssign, Shr, ShrAssign, Sub,
        SubAssign,
    },
};
use num::CheckedMul;


pub trait Flow:
'static
// + Send
// + Sync
+ Copy
+ Ord
+ Not<Output = Self>
// + Add<Output = Self>
// + Sub<Output = Self>
// + Mul<Output = Self>
// + Div<Output = Self>
// + Rem<Output = Self>
// + AddAssign
// + SubAssign
// + MulAssign
// + DivAssign
// + RemAssign
// + Sum
+ Product
// + BitOr<Output = Self>
// + BitAnd<Output = Self>
// + BitXor<Output = Self>
// + BitOrAssign
// + BitAndAssign
// + BitXorAssign
+ Shl<Output = Self>
+ Shr<Output = Self>
+ ShlAssign
+ ShrAssign
+ fmt::Display
// + fmt::Debug
// + fmt::Binary
// + fmt::Octal
// + Zero
// + One
+ BoundedBelow
+ BoundedAbove
+ FromPrimitive
+ ToPrimitive
+ NumAssign
+ CheckedMul
{
}

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
pub struct Edge<F: Flow> {
    pub from: usize,
    pub to: usize,
    pub rev: usize,     // 逆辺のindex. graph[to][rev]でアクセスできる
    pub flow: F,
    pub lower: F,
    pub upper: F,
    pub cost: F,
    pub is_rev: bool,   // 逆辺かどうか
}

impl<F: Flow> Edge<F> {
    pub fn new(from: usize, to: usize, rev: usize, flow: F, lower: F, upper: F, cost: F, is_rev: bool) -> Self {
        Edge {
            from, to, rev, flow, lower, upper, cost, is_rev
        }
    }

    pub fn residual_capacity(&self) -> F {
        self.upper - self.flow
    }
}


pub struct CostScalingPushRelabel<F: Flow> {
    num_of_nodes: usize,
    graph: Vec<Vec<Edge<F>>>,
    active_nodes: VecDeque<usize>,
    gamma: F,
    pos: Vec<(usize, usize)>,
    current_edges: Vec<usize>,  // current candidate to test for admissibility
    alpha: F,
    cost_scaling_factor: F,

    // Node
    initial_excess: Vec<F>,
    excess: Vec<F>,
    potentials: Vec<F>,

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
impl<F: Flow + std::ops::Neg<Output = F>> CostScalingPushRelabel<F> {
    pub fn new(num_of_nodes: usize) -> Self {
        let alpha = F::from_i32(5).unwrap();
        // assert!(alpha >= 2);
        CostScalingPushRelabel {
            num_of_nodes: num_of_nodes,
            graph: vec![vec![]; num_of_nodes],
            active_nodes: VecDeque::new(),
            gamma: F::zero(),
            pos: Vec::new(),
            current_edges: vec![0; num_of_nodes],
            alpha: alpha,   // it was usually between 8 and 24
            // cost_scaling_factor: 1 + alpha * num_of_nodes as i64,
            cost_scaling_factor: F::from_i64(3 + num_of_nodes as i64).unwrap(),

            // Node
            initial_excess: vec![F::zero(); num_of_nodes],
            excess: vec![F::zero(); num_of_nodes],
            potentials: vec![F::zero(); num_of_nodes],

            status: Status::NotSolved,
            optimal_cost: None,

            check_feasibility: true,

            num_discharge: 0,
            num_relabel: 0,
            num_test: 0,
        }
    }

    pub fn add_directed_edge(&mut self, from: usize, to: usize, lower: F, upper: F, cost: F) -> usize {
        assert!(lower <= upper);

        let e = self.graph[from].len();
        let re = if from == to {
            e + 1
        } else {
            self.graph[to].len()
        };

        let e1 = Edge::new(from, to, re, F::zero(), lower, upper, cost, false);
        self.graph[from].push(e1);

        let e2 = Edge::new(to, from, e, F::zero(), F::zero(), -lower, -cost, true);
        self.graph[to].push(e2);

        if cost < F::zero() {
            self.gamma = F::max(self.gamma, -cost);
        } else {
            self.gamma = F::max(self.gamma, cost);
        }

        self.pos.push((from, e));
        self.pos.len() - 1
    }

    pub fn get_directed_edge(&self, i: usize) -> Edge<F> {
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

    pub fn add_supply(&mut self, node: usize, supply: F) {
        self.initial_excess[node] += supply;
        self.excess[node] += supply;
    }

    pub fn set_check_feasibility(&mut self, check: bool) {
        self.check_feasibility = check;
    }

    pub fn solve(&mut self) -> Status {
        self.status = Status::NotSolved;

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
            epsilon = F::max(epsilon / self.alpha, F::one());

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
            self.status != Status::Infeasible && epsilon != F::one()
        } {}

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
                if !edge.is_rev {
                    solver.add_edge(edge.from, edge.to, F::to_i64(&edge.lower).unwrap(), F::to_i64(&edge.upper).unwrap());
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
                if !edge.is_rev {
                    let flow = edge.lower;
                    self.push_flow(u, i, flow);
                }
            }
        }
    }

    // make epsilon-optimal flow
    fn refine(&mut self, epsilon: F) {
        // make 0-optimal pseudo flow
        for u in 0..self.num_of_nodes {
            for i in 0..self.graph[u].len() {
                let edge = &self.graph[u][i];
                if edge.is_rev {
                    continue;
                }

                let reduced_cost = self.reduced_cost(&edge);
                if reduced_cost < F::zero() {
                    // 流量を上界にする
                    let flow = edge.residual_capacity();
                    if flow != F::zero() {
                        self.push_flow(u, i, flow);
                    }
                    // assert_eq!(self.graph[u][i].flow, self.graph[u][i].upper);
                } else if reduced_cost > F::zero() {
                    // 流量を下界にする
                    let flow = edge.lower - edge.flow;
                    if flow != F::zero() {
                        self.push_flow(u, i, flow);
                    }
                    // assert_eq!(self.graph[u][i].flow, self.graph[u][i].lower);
                }
            }
        }
        // assert!(self.is_epsilon_optimal(0));

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
            self.discharge(u, epsilon);

            if self.status == Status::Infeasible {
                return;
            }
        }
    }

    fn discharge(&mut self, u: usize, epsilon: F) {
        self.num_discharge += 1;

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
        let from = self.graph[u][i].from;
        let rev = self.graph[u][i].rev;

        self.graph[u][i].flow += flow;
        self.graph[to][rev].flow -= flow;
        self.excess[from] -= flow;
        self.excess[to] += flow;
    }

    fn reduced_cost(&self, edge: &Edge<F>) -> F {
        edge.cost + self.potentials[edge.from] - self.potentials[edge.to]
    }

    fn is_admissible(&self, edge: &Edge<F>, _epsilon: F) -> bool {
        self.reduced_cost(edge) < F::zero()
    }

    fn is_active(&self, u: usize) -> bool {
        self.excess[u] > F::zero()
    }

    // uから隣接ノードにpushする
    fn push(&mut self, u: usize, epsilon: F) {
        assert!(self.is_active(u));

        for i in self.current_edges[u]..self.graph[u].len() {
            self.num_test += 1;
            let edge = &self.graph[u][i];
            if edge.residual_capacity() <= F::zero() {
                continue;
            }

            if self.is_admissible(&edge, epsilon) {
                let to = edge.to;

                if !self.look_ahead(to, epsilon) {
                    if !self.is_admissible(&self.graph[u][i], epsilon) {
                        continue;
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
        self.num_relabel += 1;
        let guaranteed_new_potential = self.potentials[u] - epsilon;

        let mut maxi_potential = F::min_value();
        let mut previous_maxi_potential = F::min_value();
        let mut current_edges_for_u = 0;

        for i in 0..self.graph[u].len() {
            self.num_test += 1;
            if self.graph[u][i].residual_capacity() <= F::zero() {
                continue;
            }
            let to = self.graph[u][i].to;
            let cost = self.graph[u][i].cost;

            // (u->to)のreduced_cost(= cost + potential[u] - potential[to])を0にするpotential
            let new_potential = self.potentials[to] - cost;
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

    fn look_ahead(&mut self, u: usize, epsilon: F) -> bool {
        if self.excess[u] < F::zero() {
            return true;
        }

        // admissibleがあればok
        for i in self.current_edges[u]..self.graph[u].len() {
            self.num_test += 1;
            if self.graph[u][i].residual_capacity() <= F::zero() {
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

    pub fn calculate_potential(&self) -> Vec<F> {
        let mut p = vec![F::zero(); self.num_of_nodes];
        // bellman-ford
        // 最適flowに対して，残余ネットワーク上で最短経路問題を解く
        for _ in 0..self.num_of_nodes {
            let mut update = false;
            for u in 0..self.num_of_nodes {
                for e in &self.graph[u] {
                    if e.residual_capacity() > F::zero() {
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
            for e in &self.graph[u] {
                if !e.is_rev {
                    println!("{} -> {}(lower:{} flow:{} upper:{} cost:{} rest:{})", u, e.to, e.flow, e.flow, e.upper, e.cost, e.residual_capacity());
                }
            }
        }
    }

    fn excess_is_valid(&self) -> bool {
        let mut e = vec![F::zero(); self.num_of_nodes];
        for u in 0..self.num_of_nodes {
            e[u] += self.initial_excess[u];
            for edge in &self.graph[u] {
                if !edge.is_rev {
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
        let mut e = vec![F::zero(); self.num_of_nodes];
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
            if self.initial_excess[u] != e[u] {
                return false;
            }
        }

        true
    }
}
