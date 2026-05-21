# Citations

The Shivya substrate borrows established mathematical machinery from four independent traditions. This file records the source material so claims in the README and `docs/` can be checked against the original literature.

We do not claim novelty over any of the results below; we claim to compose them into a runtime.

---

## Layer 0 — Hodge Mesh: Discrete Exterior Calculus

The simplicial state complex (`crates/shivya-hodge`) and the Hodge curl-projector used by `reconcile_state_delta` rest on:

- **Hirani, A. N.** *Discrete Exterior Calculus.* Ph.D. dissertation, California Institute of Technology, 2003. — Defines the discrete boundary operators ($d_0$, $d_1$), the discrete Hodge star, and the orthogonal decomposition of $k$-cochains into exact, coexact, and harmonic components on simplicial complexes. The crate's `complex.rs::d0` / `complex.rs::d1` and `reconciler.rs::reconcile_state_delta` are direct implementations of §4 of this work.
- **Desbrun, M., Hirani, A. N., Leok, M., & Marsden, J. E.** *Discrete Exterior Calculus.* arXiv:math/0508341, 2005. — Self-contained reference for the conjugate-gradient solution of $L_2 \beta = b_2$ where $L_2 = d_1 d_1^{\top}$ is the coexact Laplacian.
- **Bossavit, A.** *Computational Electromagnetism: Variational Formulations, Complementarity, Edge Elements.* Academic Press, 1998. — Practical reference for discrete edge-flow projection; the curl-removal step in `reconciler.rs` is the discrete analogue of the magnetic-vector-potential gauge fix described in chapters 4–5.

## Layer 1 — Gibbs Flux: Variational Free Energy & Active Inference

The `GibbsFluxAgent` and `DynamicGibbsAgent` types (`crates/shivya-flux`, `crates/shivya-morphic`) implement a standard variational Active-Inference update:

- **Friston, K.** *The free-energy principle: a unified brain theory?* Nature Reviews Neuroscience, 11(2):127–138, 2010. — The original statement of the Free Energy Principle that the agents minimise. The KL-divergence + likelihood form used in `model.rs::compute_free_energy` follows §3 of the paper.
- **Friston, K., FitzGerald, T., Rigoli, F., Schwartenbeck, P., & Pezzulo, G.** *Active inference: a process theory.* Neural Computation, 29(1):1–49, 2017. — Pragmatic + epistemic policy decomposition used in `model.rs::evaluate_policies`.
- **Da Costa, L., Parr, T., Sajid, N., Veselic, S., Neacsu, V., & Friston, K.** *Active inference on discrete state-spaces: A synthesis.* Journal of Mathematical Psychology, 99:102447, 2020. — Discrete-state derivation that maps closely to the bounded-dimensional Gaussian beliefs Shivya uses.

## Layer 3 — Onsager Ensemble: Reciprocal Relations & Game Theory

The collective `OnsagerCollectiveEnsemble` and the `harsanyi.rs` coalition solver in `crates/shivya-onsager` build on:

- **Onsager, L.** *Reciprocal relations in irreversible processes. I.* Physical Review, 37(4):405–426, 1931. — Establishes the symmetric phenomenological coupling matrix $L_{ij} = L_{ji}$ relating thermodynamic forces to fluxes. The `field.rs` regulator enforces this antisymmetry directly.
- **Onsager, L.** *Reciprocal relations in irreversible processes. II.* Physical Review, 38(12):2265–2279, 1931. — Companion paper extending the result; together they form the basis of linear non-equilibrium thermodynamics.
- **Harsanyi, J. C.** *A bargaining model for the cooperative n-person game.* In *Contributions to the Theory of Games, IV*, Princeton University Press, 1959. — Defines the Harsanyi dividend $d(S) = v(S) - \sum_{T \subsetneq S} d(T)$ that `harsanyi.rs::compute_dividends` evaluates via the standard $(\text{mask}-1) \mathrel{\&} \text{mask}$ subset enumeration. This is the classical cooperative-game-theory recursion, *not* a novel construction.
- **Shapley, L. S.** *A value for n-person games.* In *Contributions to the Theory of Games, II*, Princeton University Press, 1953. — Background on the Shapley value, of which Harsanyi dividends are the Möbius transform.

## Layer 4 — Turing Morphogenesis: Reaction-Diffusion on Graphs

The `MorphogenSystem` and the mitosis/apoptosis engines (`crates/shivya-turing`) implement:

- **Turing, A. M.** *The chemical basis of morphogenesis.* Philosophical Transactions of the Royal Society B, 237(641):37–72, 1952. — Original derivation of activator-inhibitor instabilities that produce stable spatial patterns. The Gierer-Meinhardt kinetics in `morphogen.rs` are the canonical instance.
- **Gierer, A., & Meinhardt, H.** *A theory of biological pattern formation.* Kybernetik, 12(1):30–39, 1972. — Specific reaction system $\partial_t u = d_u \nabla^2 u + a - b\,u + u^2/v$, $\partial_t v = d_v \nabla^2 v + c\,u^2 - d\,v$ implemented in `morphogen.rs::reaction_kinetics`.
- **Nakao, H., & Mikhailov, A. S.** *Turing patterns in network-organized activator-inhibitor systems.* Nature Physics, 6(7):544–550, 2010. — Extension of Turing's continuous PDE to discrete graphs via the graph Laplacian, which is the form `morphogen.rs::step_rk4` actually integrates.
- **Hairer, E., Nørsett, S. P., & Wanner, G.** *Solving Ordinary Differential Equations I: Nonstiff Problems.* 2nd ed., Springer, 1993. — Reference for the classical RK4 scheme used in `step_rk4`, and for the CFL stability bound $\Delta t < 1 / (2 \cdot d_{\max} \cdot \deg_{\max})$ implemented in `morphogen.rs::adaptive_timestep`.

## P2P Transport — Kademlia

The `crates/shivya-p2p` transport implements:

- **Maymounkov, P., & Mazières, D.** *Kademlia: A peer-to-peer information system based on the XOR metric.* In *International Workshop on Peer-to-Peer Systems (IPTPS)*, pp. 53-65, Springer, 2002. — Source for the XOR distance metric, the K-bucket structure with $K = 4$ used in `routing.rs`, the LRU eviction guard in `transport.rs`, and the `PING` / `STORE` / `FIND_NODE` / `FIND_VALUE` RPC set.

## Foundational Computational Models

The Layer-2 register VM (`crates/shivya-morphic/src/vm/eval.rs`) is grounded in:

- **Turing, A. M.** *On computable numbers, with an application to the Entscheidungsproblem.* Proceedings of the London Mathematical Society, s2-42(1):230–265, 1936. — Establishes the abstract machine model. The morphic VM is not a Turing machine but is computationally bounded by an explicit instruction-cycle budget for sandboxing, a discipline informed by this lineage.
- **von Neumann, J.** *Theory of Self-Reproducing Automata.* University of Illinois Press, 1966. — Conceptual grounding for the `MorphicHotSwapper` self-rewriting behavior, in which the cell state (the AST) is itself part of the addressable program.

---

## What this project does *not* claim to be backed by

To be honest about scope:

- We do not claim a peer-reviewed proof that the substrate provides linearizable or sequential consistency. The post-partition convergence demonstrated in [`tests/jepsen_partitions.rs`](tests/jepsen_partitions.rs) shows curl-removal on a fixed bowtie topology, not a general-graph theorem.
- We do not claim Byzantine fault tolerance. The current Kademlia layer has no defense against eclipse attacks, peer spoofing, or malicious `FOUND_VALUE` poisoning.
- We do not claim performance parity with Raft, etcd, or any production consensus system. No comparative benchmarks have been run.

The mathematics this project draws on is well-established; the *engineering composition* into a runtime is the part that remains experimental.
