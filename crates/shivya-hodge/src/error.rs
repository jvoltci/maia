/// Recoverable math failures surface through this enum instead of `panic!`.
///
/// The substrate's runtime layers (orchestrator, ensemble, daemon, reconciler)
/// treat every variant as a soft signal — either by retrying with regularised
/// inputs or by gracefully degrading to a no-op so a degenerate telemetry
/// sample never kills the daemon process.
///
/// Lives in `shivya-hodge` (Layer 0) because both the discrete-exterior-calculus
/// operators and the higher-layer matrix inverter need a shared error vocabulary;
/// `shivya-flux` re-exports it so existing call sites keep working.
#[derive(Debug, Clone)]
pub enum SubstrateError {
    /// Covariance / precision matrix was singular at the listed size.
    /// `det` is the determinant at the point of failure.
    SingularMatrix { size: usize, det: f64 },
    /// Even after a ridge of `ridge` the matrix remained ill-conditioned;
    /// the math path fell back to the identity (= maximum-entropy prior).
    StabilizationFailed { size: usize, ridge: f64 },
    /// Combining vectors/matrices with mismatched extents.
    DimensionMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for SubstrateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubstrateError::SingularMatrix { size, det } => {
                write!(f, "singular {0}x{0} matrix (det={1:.3e})", size, det)
            }
            SubstrateError::StabilizationFailed { size, ridge } => {
                write!(f, "ridge {0:.1e} insufficient to stabilise {1}x{1} matrix", ridge, size)
            }
            SubstrateError::DimensionMismatch { expected, actual } => {
                write!(f, "dimension mismatch: expected {}, got {}", expected, actual)
            }
        }
    }
}

impl std::error::Error for SubstrateError {}
