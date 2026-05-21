pub mod complex;
pub mod error;
pub mod operators;
pub mod reconciler;
pub mod solver;

pub use complex::SimplicialStateComplex;
pub use error::SubstrateError;
pub use operators::SparseMatrix;
pub use reconciler::{reconcile_state_delta, try_reconcile_state_delta};
pub use solver::conjugate_gradient;
