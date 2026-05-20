use shivya::hodge::complex::SimplicialStateComplex;
use shivya::hodge::reconciler::reconcile_state_delta;
use shivya::morphic::{DynamicGibbsAgent, Expr, MorphicHotSwapper, compile};
use shivya::onsager::OnsagerCollectiveEnsemble;
use shivya::turing::{MorphogenSystem, MitosisEngine, ApoptosisEngine};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct NodeStatus {
    pub id: usize,
    pub active: bool,
    pub free_energy: f64,
    pub belief_dim: usize,
    pub beliefs: Vec<f64>,
    pub morphic_equation: String,
    pub instruction_count: usize,
    pub morphogen_u: f64,
    pub morphogen_v: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SystemStatus {
    pub collective_free_energy: f64,
    pub curl_deviation: f64,
    pub active_nodes_count: usize,
    pub active_pool: Vec<usize>,
    pub nodes: Vec<NodeStatus>,
}

pub struct NativeOrchestrator {
    pub max_nodes: usize,
    pub complex: SimplicialStateComplex,
    pub ensemble: OnsagerCollectiveEnsemble,
    pub swappers: Vec<MorphicHotSwapper>,
    pub turing: MorphogenSystem,
    pub mitosis: MitosisEngine,
    pub apoptosis: ApoptosisEngine,
    pub step_count: usize,
    pub last_status: SystemStatus,
}

impl NativeOrchestrator {
    pub fn new(max_nodes: usize) -> Self {
        let mut complex = SimplicialStateComplex::new();
        // Setup initial simplicial mesh
        complex.add_vertex("Node0", 1.0);
        complex.add_vertex("Node1", 1.2);
        complex.add_vertex("Node2", 0.9);
        complex.add_edge("Node0", "Node1", 0.5);
        complex.add_edge("Node1", "Node2", 0.6);
        complex.add_edge("Node0", "Node2", 0.8);

        let adjacent_nodes = vec![
            vec![1, 2],
            vec![0, 2],
            vec![0, 1],
            vec![], // Slot 3 dormant
            vec![], // Slot 4 dormant
            vec![], // Slot 5 dormant
            vec![], // Slot 6 dormant
            vec![], // Slot 7 dormant
            vec![], // Slot 8 dormant
            vec![], // Slot 9 dormant
        ];

        let create_agent = |mu_prior_val: f64| {
            DynamicGibbsAgent::new(
                2, 1, 2,
                vec![mu_prior_val, 0.0],
                vec![vec![10.0, 0.0], vec![0.0, 10.0]],
                vec![vec![1.5, 0.2], vec![0.2, 1.2]],
                vec![vec![0.1, 0.0], vec![0.0, 0.1]],
                vec![vec![0.0], vec![0.0]],
                vec![vec![0.0], vec![0.0]],
                vec![0.0, 0.0],
                vec![vec![1.0, 0.0], vec![0.0, 1.0]],
                5.0,
            )
        };

        let mut agents = Vec::new();
        for i in 0..max_nodes {
            agents.push(create_agent(0.1 + (i as f64) * 0.05));
        }

        let base_coupling = 0.5;
        let ensemble = OnsagerCollectiveEnsemble::new(agents, adjacent_nodes, base_coupling);

        let create_swapper = || {
            MorphicHotSwapper::new(Expr::Mul(
                Box::new(Expr::Const(1.0)),
                Box::new(Expr::Var(0)),
            ))
        };
        let mut swappers = Vec::new();
        for _ in 0..max_nodes {
            swappers.push(create_swapper());
        }

        let mut turing = MorphogenSystem::new(max_nodes, 0.01, 0.1);
        turing.activate_node(0, 0.5, 1.0);
        turing.activate_node(1, 0.2, 1.0);
        turing.activate_node(2, 0.3, 1.0);
        turing.set_edge(0, 1, 1.0);
        turing.set_edge(1, 2, 1.0);
        turing.set_edge(0, 2, 1.0);

        let mitosis = MitosisEngine::new(2.0, 0.01);
        let apoptosis = ApoptosisEngine::new(0.05);

        let last_status = SystemStatus {
            collective_free_energy: 0.0,
            curl_deviation: 0.0,
            active_nodes_count: 3,
            active_pool: vec![0, 1, 2],
            nodes: Vec::new(),
        };

        Self {
            max_nodes,
            complex,
            ensemble,
            swappers,
            turing,
            mitosis,
            apoptosis,
            step_count: 0,
            last_status,
        }
    }

    pub fn step(&mut self, cpu_load: f64, net_rate: f64) {
        self.step_count += 1;

        // 1. Gather active status
        let mut active_indices = Vec::new();
        for i in 0..self.max_nodes {
            if self.turing.active[i] {
                active_indices.push(i);
            }
        }

        // Scale coupling coefficients dynamically based on network bit-rate
        let net_rate_scaled = (net_rate / 1_000_000.0).min(5.0); // max 5.0 scale
        let base_coupling = 0.5;
        for i in 0..self.max_nodes {
            for j in 0..self.max_nodes {
                if i != j {
                    self.ensemble.regulator.l_matrix[i][j] = base_coupling * (1.0 + net_rate_scaled);
                }
            }
        }

        // 2. Prepare CPU observations for active agents
        let obs_val = cpu_load / 100.0;
        let mut obs = vec![vec![0.0, 0.0]; self.max_nodes];
        for &i in &active_indices {
            obs[i] = vec![obs_val, obs_val * 0.9];
        }

        // Step Onsager Collective Ensemble for active nodes
        // Since step() iterates over all agents, we only pass non-zero values to active ones
        let collective_f = self.ensemble.step(&obs, 0.1, 10, 1e-4, 0.1);

        // 3. Morphic Hot-swapping VM updates
        for &i in &active_indices {
            let dataset = vec![
                (vec![obs[i][0]], obs[i][0] * 1.5),
                (vec![obs[i][1]], obs[i][1] * 1.5),
            ];
            let seed = (self.step_count + i * 99) as u32;
            self.swappers[i].run_metamorphic_step(&dataset, seed);
        }

        // 4. Reconcile topological states (Layer 0)
        let mut delta_s = vec![0.0; self.complex.edges.len()];
        // Fill delta_s with tiny perturbation from CPU stress
        for i in 0..delta_s.len() {
            delta_s[i] = obs_val * 0.1;
        }
        let reconciled = reconcile_state_delta(&self.complex, &delta_s);
        let curl_deviation: f64 = reconciled.iter().zip(delta_s.iter())
            .map(|(&r, &d)| (r - d).powi(2))
            .sum::<f64>().sqrt();

        // 5. Gierer-Meinhardt reaction diffusion step (Layer 4)
        // Runge-Kutta 4th order system updates with CFL stability guard
        self.turing.step_rk4(0.05);

        // Extract beliefs and adjacency lists for mitosis/apoptosis
        let mut beliefs: Vec<Vec<f64>> = self.ensemble.agents.iter().map(|a| a.mu_q.clone()).collect();
        let mut adjacent_nodes = self.ensemble.adjacent_nodes.clone();

        // 6. Mitosis Engine split evaluation
        if let Some((parent, child)) = self.mitosis.evaluate_and_split(&mut self.turing, &mut beliefs, &mut adjacent_nodes) {
            // Apply new belief dimension to child agent
            self.ensemble.agents[child].mu_q = beliefs[child].clone();
            self.ensemble.agents[child].i_dim = beliefs[child].len();
            // Sync adjacency list
            self.ensemble.adjacent_nodes = adjacent_nodes.clone();
            // Update Hodge Mesh topology
            let child_label = format!("Node{}", child);
            self.complex.add_vertex(&child_label, 1.0);
            let parent_label = format!("Node{}", parent);
            self.complex.add_edge(&parent_label, &child_label, 1.0);
        }

        // 7. Apoptosis Engine pruning evaluation
        let mut free_energies = vec![0.0; self.max_nodes];
        for i in 0..self.max_nodes {
            free_energies[i] = self.ensemble.agents[i].f_history.last().cloned().unwrap_or(0.0);
        }
        if let Some(_pruned_node) = self.apoptosis.evaluate_and_prune(&mut self.turing, &mut beliefs, &mut adjacent_nodes, &free_energies, 5.0) {
            // Sync changes back
            self.ensemble.adjacent_nodes = adjacent_nodes;
            // Sever edge in Hodge Mesh complex by setting weight to zero or state subtraction
            // Note: Since SimplicialStateComplex is fixed-size during execution loop run, we keep it as is.
        }

        // 8. Capture updated status state
        let mut nodes_status = Vec::new();
        for i in 0..self.max_nodes {
            let active = self.turing.active[i];
            let agent = &self.ensemble.agents[i];
            let (insts, _) = compile(&self.swappers[i].current_expr);
            nodes_status.push(NodeStatus {
                id: i,
                active,
                free_energy: agent.f_history.last().cloned().unwrap_or(0.0),
                belief_dim: agent.i_dim,
                beliefs: agent.mu_q.clone(),
                morphic_equation: format!("{:?}", self.swappers[i].current_expr),
                instruction_count: insts.len(),
                morphogen_u: self.turing.u[i],
                morphogen_v: self.turing.v[i],
            });
        }

        self.last_status = SystemStatus {
            collective_free_energy: collective_f,
            curl_deviation,
            active_nodes_count: active_indices.len(),
            active_pool: active_indices,
            nodes: nodes_status,
        };
    }

    pub fn get_status_json(&self) -> String {
        serde_json::to_string_pretty(&self.last_status).unwrap_or_default()
    }
}
