//! Dense matrices with symbolic entries.
//!
//! Numeric matrices (every entry an exact constant) use exact Gaussian elimination, so
//! their determinants and inverses are exact rationals. A matrix with symbolic entries
//! uses cofactor expansion, which is correct for any entries but grows factorially, so
//! it is limited to [`MAX_SYMBOLIC_SIZE`].
//!
//! Symbolic inverses divide by the determinant. That is valid only where the
//! determinant is non-zero, which this module does not check for symbolic
//! determinants. The caller must make that assumption.

use crate::error::Error;
use crate::expr::Expr;
use crate::mathr::Number;
use crate::simplify::simplify;
use alloc::vec;
use alloc::vec::Vec;

/// Largest dimension for symbolic determinants and inverses.
pub const MAX_SYMBOLIC_SIZE: usize = 7;

/// A dense `rows x cols` matrix, stored row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<Expr>,
}

impl Matrix {
    /// Builds a matrix from row-major `data`. Fails when the length is not `rows * cols`.
    pub fn new(rows: usize, cols: usize, data: Vec<Expr>) -> Result<Matrix, Error> {
        if rows.checked_mul(cols) != Some(data.len()) {
            return Err(Error::Unsupported(
                "matrix data does not match its dimensions",
            ));
        }
        Ok(Matrix { rows, cols, data })
    }

    /// Builds a matrix from a list of rows. Fails when the rows have different lengths.
    pub fn from_rows(rows: Vec<Vec<Expr>>) -> Result<Matrix, Error> {
        let cols = rows.first().map_or(0, Vec::len);
        if rows.iter().any(|r| r.len() != cols) {
            return Err(Error::Unsupported("ragged matrix rows"));
        }
        let n = rows.len();
        Matrix::new(n, cols, rows.into_iter().flatten().collect())
    }

    /// The `n x n` identity matrix.
    pub fn identity(n: usize) -> Matrix {
        let mut data = vec![Expr::zero(); n * n];
        for i in 0..n {
            data[i * n + i] = Expr::one();
        }
        Matrix {
            rows: n,
            cols: n,
            data,
        }
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// The entry at row `r`, column `c`, counting from zero.
    pub fn get(&self, r: usize, c: usize) -> Option<&Expr> {
        if r < self.rows && c < self.cols {
            self.data.get(r * self.cols + c)
        } else {
            None
        }
    }

    /// The transpose.
    pub fn transpose(&self) -> Matrix {
        let mut data = Vec::with_capacity(self.data.len());
        for c in 0..self.cols {
            for r in 0..self.rows {
                data.push(self.data[r * self.cols + c].clone());
            }
        }
        Matrix {
            rows: self.cols,
            cols: self.rows,
            data,
        }
    }

    /// Every entry simplified.
    pub fn simplified(&self) -> Matrix {
        Matrix {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(simplify).collect(),
        }
    }

    /// Entrywise sum. Both matrices must have the same shape.
    pub fn add(&self, other: &Matrix) -> Result<Matrix, Error> {
        self.zip_with(other, Expr::sum2)
    }

    /// Entrywise difference. Both matrices must have the same shape.
    pub fn sub(&self, other: &Matrix) -> Result<Matrix, Error> {
        self.zip_with(other, Expr::difference)
    }

    /// Every entry multiplied by `k`.
    pub fn scale(&self, k: &Expr) -> Matrix {
        Matrix {
            rows: self.rows,
            cols: self.cols,
            data: self
                .data
                .iter()
                .map(|e| Expr::product(vec![k.clone(), e.clone()]))
                .collect(),
        }
    }

    /// Matrix product. The column count of `self` must equal the row count of `other`.
    pub fn mul(&self, other: &Matrix) -> Result<Matrix, Error> {
        if self.cols != other.rows {
            return Err(Error::Unsupported("matrix product dimensions do not match"));
        }
        let mut data = Vec::with_capacity(self.rows * other.cols);
        for r in 0..self.rows {
            for c in 0..other.cols {
                let terms = (0..self.cols)
                    .map(|k| {
                        Expr::product(vec![
                            self.data[r * self.cols + k].clone(),
                            other.data[k * other.cols + c].clone(),
                        ])
                    })
                    .collect();
                data.push(Expr::sum(terms));
            }
        }
        Ok(Matrix {
            rows: self.rows,
            cols: other.cols,
            data,
        })
    }

    /// The determinant.
    ///
    /// Exact for numeric matrices at any size. Symbolic matrices are limited to
    /// [`MAX_SYMBOLIC_SIZE`].
    /// # Example
    /// ```
    /// use tpt_symbolic::matrix::Matrix;
    /// use tpt_symbolic::Expr;
    /// let m = Matrix::from_rows(vec![
    ///     vec![Expr::int(1), Expr::int(2)],
    ///     vec![Expr::int(3), Expr::int(4)],
    /// ]).unwrap();
    /// assert_eq!(m.det().unwrap(), Expr::int(-2));
    /// ```
    pub fn det(&self) -> Result<Expr, Error> {
        self.require_square()?;
        if let Some(rows) = self.numeric_rows() {
            return Ok(match det_numeric(rows) {
                Some(d) => Expr::Const(d),
                None => Expr::zero(),
            });
        }
        if self.rows > MAX_SYMBOLIC_SIZE {
            return Err(Error::Unsupported(
                "symbolic determinant is limited to small matrices",
            ));
        }
        Ok(simplify(&det_cofactor(&self.data, self.rows)))
    }

    /// The inverse.
    ///
    /// For numeric matrices this is exact and fails with [`Error::DivisionByZero`] when
    /// the matrix is singular. For symbolic matrices, see the module note on the
    /// determinant assumption.
    /// # Example
    /// ```
    /// use tpt_symbolic::matrix::Matrix;
    /// use tpt_symbolic::Expr;
    /// let m = Matrix::from_rows(vec![
    ///     vec![Expr::int(1), Expr::int(2)],
    ///     vec![Expr::int(3), Expr::int(4)],
    /// ]).unwrap();
    /// let inv = m.inverse().unwrap();
    /// assert_eq!(inv.get(0, 0), Some(&Expr::int(-2)));
    /// ```
    pub fn inverse(&self) -> Result<Matrix, Error> {
        self.require_square()?;
        let n = self.rows;
        if let Some(rows) = self.numeric_rows() {
            let inv = inverse_numeric(rows).ok_or(Error::DivisionByZero)?;
            let data = inv.into_iter().flatten().map(Expr::Const).collect();
            return Ok(Matrix {
                rows: n,
                cols: n,
                data,
            });
        }
        if n > MAX_SYMBOLIC_SIZE {
            return Err(Error::Unsupported(
                "symbolic inverse is limited to small matrices",
            ));
        }
        let d = self.det()?;
        if d.as_number().is_some_and(|x| x.is_zero()) {
            return Err(Error::DivisionByZero);
        }
        // Adjugate: entry (i, j) is the cofactor of (j, i) over the determinant.
        let mut data = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                let minor = minor_of(&self.data, n, j, i);
                let sign = if (i + j) % 2 == 0 { 1 } else { -1 };
                let cof = Expr::product(vec![Expr::int(sign), det_cofactor(&minor, n - 1)]);
                data.push(simplify(&Expr::quotient(cof, d.clone())));
            }
        }
        Ok(Matrix {
            rows: n,
            cols: n,
            data,
        })
    }

    /// Solves `self * x = b` for `x`, where `b` is a column vector of the same height.
    pub fn solve(&self, b: &[Expr]) -> Result<Vec<Expr>, Error> {
        if b.len() != self.rows {
            return Err(Error::Unsupported(
                "right-hand side does not match the matrix",
            ));
        }
        let column = Matrix::new(b.len(), 1, b.to_vec())?;
        let x = self.inverse()?.mul(&column)?;
        Ok(x.data)
    }

    fn require_square(&self) -> Result<(), Error> {
        if self.rows == self.cols {
            Ok(())
        } else {
            Err(Error::Unsupported("this operation needs a square matrix"))
        }
    }

    fn zip_with(&self, other: &Matrix, op: impl Fn(Expr, Expr) -> Expr) -> Result<Matrix, Error> {
        if self.rows != other.rows || self.cols != other.cols {
            return Err(Error::Unsupported("matrix shapes do not match"));
        }
        let data = self
            .data
            .iter()
            .zip(&other.data)
            .map(|(a, b)| op(a.clone(), b.clone()))
            .collect();
        Ok(Matrix {
            rows: self.rows,
            cols: self.cols,
            data,
        })
    }

    /// Every entry as an exact number, when all of them are constants.
    fn numeric_rows(&self) -> Option<Vec<Vec<Number>>> {
        let flat: Option<Vec<Number>> = self.data.iter().map(Expr::as_number).collect();
        let flat = flat?;
        Some(flat.chunks(self.cols.max(1)).map(|r| r.to_vec()).collect())
    }
}

impl Expr {
    /// Binary sum, used by [`Matrix::add`].
    fn sum2(a: Expr, b: Expr) -> Expr {
        Expr::sum(vec![a, b])
    }
}

/// Exact determinant by Gaussian elimination. `None` never occurs for square input, but
/// the arithmetic is checked anyway.
fn det_numeric(mut m: Vec<Vec<Number>>) -> Option<Number> {
    let n = m.len();
    let mut det = Number::one();
    for col in 0..n {
        let Some(pivot) = (col..n).find(|&r| !m[r][col].is_zero()) else {
            return Some(Number::zero());
        };
        if pivot != col {
            m.swap(pivot, col);
            det = det.checked_neg()?;
        }
        let p = m[col][col].clone();
        det = det.checked_mul(&p)?;
        let pivot_row = m[col].clone();
        for row in m.iter_mut().skip(col + 1) {
            let factor = row[col].checked_div(&p)?;
            for (cell, pivot) in row.iter_mut().zip(&pivot_row).skip(col) {
                let delta = factor.checked_mul(pivot)?;
                *cell = cell.checked_sub(&delta)?;
            }
        }
    }
    Some(det)
}

/// Exact inverse by Gauss-Jordan elimination. `None` when the matrix is singular.
fn inverse_numeric(mut m: Vec<Vec<Number>>) -> Option<Vec<Vec<Number>>> {
    let n = m.len();
    let mut inv: Vec<Vec<Number>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    if i == j {
                        Number::one()
                    } else {
                        Number::zero()
                    }
                })
                .collect()
        })
        .collect();
    for col in 0..n {
        let pivot = (col..n).find(|&r| !m[r][col].is_zero())?;
        m.swap(pivot, col);
        inv.swap(pivot, col);
        let p = m[col][col].clone();
        for c in 0..n {
            m[col][c] = m[col][c].checked_div(&p)?;
            inv[col][c] = inv[col][c].checked_div(&p)?;
        }
        for r in 0..n {
            if r == col || m[r][col].is_zero() {
                continue;
            }
            let factor = m[r][col].clone();
            for c in 0..n {
                let dm = factor.checked_mul(&m[col][c])?;
                m[r][c] = m[r][c].checked_sub(&dm)?;
                let di = factor.checked_mul(&inv[col][c])?;
                inv[r][c] = inv[r][c].checked_sub(&di)?;
            }
        }
    }
    Some(inv)
}

/// The determinant of the `n x n` row-major matrix `m`, by expansion along the first row.
fn det_cofactor(m: &[Expr], n: usize) -> Expr {
    match n {
        0 => Expr::one(),
        1 => m[0].clone(),
        _ => {
            let mut terms = Vec::with_capacity(n);
            for j in 0..n {
                let minor = minor_of(m, n, 0, j);
                let sign = if j % 2 == 0 { 1 } else { -1 };
                terms.push(Expr::product(vec![
                    Expr::int(sign),
                    m[j].clone(),
                    det_cofactor(&minor, n - 1),
                ]));
            }
            Expr::sum(terms)
        }
    }
}

/// The `(n-1) x (n-1)` matrix left after deleting row `skip_row` and column `skip_col`.
fn minor_of(m: &[Expr], n: usize, skip_row: usize, skip_col: usize) -> Vec<Expr> {
    let mut out = Vec::with_capacity((n - 1) * (n - 1));
    for r in 0..n {
        if r == skip_row {
            continue;
        }
        for c in 0..n {
            if c != skip_col {
                out.push(m[r * n + c].clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    fn m2(a: &str, b: &str, c: &str, d: &str) -> Matrix {
        Matrix::from_rows(vec![vec![p(a), p(b)], vec![p(c), p(d)]]).unwrap()
    }

    #[test]
    fn numeric_determinant_and_inverse_are_exact() {
        let m = m2("1", "2", "3", "4");
        assert_eq!(m.det().unwrap(), Expr::int(-2));
        let inv = m.inverse().unwrap();
        // [[-2, 1], [3/2, -1/2]]
        assert_eq!(inv.get(0, 0), Some(&Expr::int(-2)));
        assert_eq!(inv.get(0, 1), Some(&Expr::int(1)));
        assert_eq!(inv.get(1, 0), Some(&Expr::rational(3, 2).unwrap()));
        assert_eq!(inv.get(1, 1), Some(&Expr::rational(-1, 2).unwrap()));
        assert_eq!(m.mul(&inv).unwrap(), Matrix::identity(2));
    }

    #[test]
    fn singular_numeric_matrix_has_no_inverse() {
        let m = m2("1", "2", "2", "4");
        assert_eq!(m.det().unwrap(), Expr::zero());
        assert_eq!(m.inverse(), Err(Error::DivisionByZero));
    }

    #[test]
    fn symbolic_determinant_and_inverse() {
        let m = m2("a", "b", "c", "d");
        assert_eq!(m.det().unwrap(), simplify(&p("a*d - b*c")));
        let inv = m.inverse().unwrap();
        // The simplifier does not prove the identity symbolically (it would need to factor
        // ad - bc out of the sum), so check it at a point where ad - bc != 0.
        let at_point = |e: &Expr| {
            ["a", "b", "c", "d"]
                .iter()
                .zip([2, 1, 1, 1])
                .fold(e.clone(), |acc, (v, k)| {
                    crate::eval::subs(&acc, v, &Expr::int(k))
                })
        };
        let product = m.mul(&inv).unwrap();
        let product = product.simplified();
        assert_eq!(simplify(&at_point(product.get(0, 0).unwrap())), Expr::one());
        assert_eq!(
            simplify(&at_point(product.get(0, 1).unwrap())),
            Expr::zero()
        );
        assert_eq!(simplify(&at_point(product.get(1, 1).unwrap())), Expr::one());
    }

    #[test]
    fn solve_a_linear_system() {
        // 2x + y = 5, x + 3y = 10 gives x = 1, y = 3.
        let a = m2("2", "1", "1", "3");
        let x = a.solve(&[Expr::int(5), Expr::int(10)]).unwrap();
        assert_eq!(x, vec![Expr::int(1), Expr::int(3)]);
    }

    #[test]
    fn shape_errors_are_reported() {
        assert!(Matrix::new(2, 2, vec![Expr::zero()]).is_err());
        assert!(Matrix::from_rows(vec![vec![Expr::zero()], vec![]]).is_err());
        let a = Matrix::new(2, 3, vec![Expr::zero(); 6]).unwrap();
        assert!(a.det().is_err());
        assert!(a.mul(&a).is_err());
        assert_eq!(a.transpose().rows(), 3);
    }

    #[test]
    fn three_by_three_numeric_determinant() {
        let m = Matrix::from_rows(vec![
            vec![Expr::int(2), Expr::int(0), Expr::int(1)],
            vec![Expr::int(3), Expr::int(0), Expr::int(0)],
            vec![Expr::int(5), Expr::int(1), Expr::int(1)],
        ])
        .unwrap();
        // Expanding along the middle column leaves only the 1 at row 3: 1 * (2*0 - 1*3)... sign + gives 3.
        assert_eq!(m.det().unwrap(), Expr::int(3));
    }
}
