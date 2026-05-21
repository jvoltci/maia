use crate::error::SubstrateError;

#[derive(Clone, Debug)]
pub struct SparseMatrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<Vec<(usize, f64)>>, // row index -> list of (col_index, value)
}

impl SparseMatrix {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![Vec::new(); rows],
        }
    }

    /// Insert (or overwrite) the entry at `(r, c)`.
    ///
    /// Out-of-bounds indices used to abort the process; they now surface as
    /// `SubstrateError::DimensionMismatch` so the math path can never panic.
    /// Internal callers in this crate construct `(r, c)` from topologically
    /// valid sources, so the error variant is in practice unreachable; we
    /// keep it as the structural guarantee, not as a hot-path branch.
    pub fn insert(&mut self, r: usize, c: usize, val: f64) -> Result<(), SubstrateError> {
        if r >= self.rows {
            return Err(SubstrateError::DimensionMismatch { expected: self.rows, actual: r });
        }
        if c >= self.cols {
            return Err(SubstrateError::DimensionMismatch { expected: self.cols, actual: c });
        }
        if let Some(entry) = self.data[r].iter_mut().find(|(col, _)| *col == c) {
            entry.1 = val;
        } else {
            self.data[r].push((c, val));
        }
        Ok(())
    }

    pub fn get(&self, r: usize, c: usize) -> f64 {
        if r >= self.rows || c >= self.cols {
            return 0.0;
        }
        self.data[r]
            .iter()
            .find(|(col, _)| *col == c)
            .map(|(_, val)| *val)
            .unwrap_or(0.0)
    }

    pub fn mul_vec(&self, x: &[f64]) -> Result<Vec<f64>, SubstrateError> {
        if x.len() != self.cols {
            return Err(SubstrateError::DimensionMismatch { expected: self.cols, actual: x.len() });
        }
        let mut y = vec![0.0; self.rows];
        for r in 0..self.rows {
            let mut sum = 0.0;
            for &(c, val) in &self.data[r] {
                sum += val * x[c];
            }
            y[r] = sum;
        }
        Ok(y)
    }

    pub fn transpose(&self) -> Result<Self, SubstrateError> {
        let mut t = Self::new(self.cols, self.rows);
        for r in 0..self.rows {
            for &(c, val) in &self.data[r] {
                t.insert(c, r, val)?;
            }
        }
        Ok(t)
    }

    // Multiply two sparse matrices: self * other
    pub fn mul_mat(&self, other: &Self) -> Result<Self, SubstrateError> {
        if self.cols != other.rows {
            return Err(SubstrateError::DimensionMismatch { expected: self.cols, actual: other.rows });
        }
        let mut result = Self::new(self.rows, other.cols);
        for r in 0..self.rows {
            for &(c_self, val_self) in &self.data[r] {
                for &(c_other, val_other) in &other.data[c_self] {
                    let current = result.get(r, c_other);
                    result.insert(r, c_other, current + val_self * val_other)?;
                }
            }
        }
        Ok(result)
    }
}
