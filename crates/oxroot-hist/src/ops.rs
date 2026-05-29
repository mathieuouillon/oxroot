//! Histogram arithmetic: scale, add (merge), multiply, divide, integral.
//!
//! These follow ROOT's `TH1::Scale`/`Add`/`Multiply`/`Divide` semantics,
//! including per-bin error (`Sumw2`) propagation. `add` with `c = 1` is the
//! bin-by-bin merge used to combine outputs across parallel jobs (`hadd`).

use crate::{TH1, TH2, TH3};

/// Effective per-bin error² for `other`: its `fSumw2[i]` if tracked, else the
/// content (for an unweighted histogram, `Σw² == Σw == content`).
fn err2(sumw2: &[f64], contents: &[f64], i: usize) -> f64 {
    sumw2.get(i).copied().unwrap_or_else(|| contents[i].abs())
}

impl TH1 {
    /// Multiply all bin contents (and errors) by `c`. The mean is preserved.
    pub fn scale(&mut self, c: f64) {
        for v in &mut self.contents {
            *v *= c;
        }
        for v in &mut self.sumw2 {
            *v *= c * c;
        }
        self.tsumw *= c;
        self.tsumw2 *= c * c;
        self.tsumwx *= c;
        self.tsumwx2 *= c;
    }

    /// Add `c * other` into this histogram (a bin-by-bin merge when `c == 1`).
    /// Returns `false` and makes no change if the binnings differ. Errors are
    /// tracked if either side tracks them (or `c != 1`).
    pub fn add(&mut self, other: &TH1, c: f64) -> bool {
        if self.contents.len() != other.contents.len() {
            return false;
        }
        if self.sumw2.is_empty() && (!other.sumw2.is_empty() || c != 1.0) {
            self.sumw2 = self.contents.iter().map(|v| v.abs()).collect();
        }
        for i in 0..self.contents.len() {
            self.contents[i] += c * other.contents[i];
        }
        for i in 0..self.sumw2.len() {
            self.sumw2[i] += c * c * err2(&other.sumw2, &other.contents, i);
        }
        self.entries += c * other.entries;
        self.tsumw += c * other.tsumw;
        self.tsumw2 += c * c * other.tsumw2;
        self.tsumwx += c * other.tsumwx;
        self.tsumwx2 += c * other.tsumwx2;
        true
    }

    /// Sum of the in-range bin contents (excludes under/overflow).
    pub fn integral(&self) -> f64 {
        let n = self.contents.len();
        if n < 2 {
            return 0.0;
        }
        self.contents[1..n - 1].iter().sum()
    }

    /// Multiply bin-by-bin by `other`, propagating errors as ROOT does
    /// (`e² = e1²·c2² + e2²·c1²`). Returns `false` if the binnings differ.
    pub fn multiply(&mut self, other: &TH1) -> bool {
        if self.contents.len() != other.contents.len() {
            return false;
        }
        if self.sumw2.is_empty() {
            self.sumw2 = self.contents.iter().map(|v| v.abs()).collect();
        }
        for i in 0..self.contents.len() {
            let (c1, c2) = (self.contents[i], other.contents[i]);
            let (e1, e2) = (self.sumw2[i], err2(&other.sumw2, &other.contents, i));
            self.sumw2[i] = e1 * c2 * c2 + e2 * c1 * c1;
            self.contents[i] = c1 * c2;
        }
        true
    }

    /// Divide bin-by-bin by `other` (0 where the denominator is 0), propagating
    /// errors as ROOT's default `e² = (e1²·c2² + e2²·c1²) / c2⁴`. Returns
    /// `false` if the binnings differ.
    pub fn divide(&mut self, other: &TH1) -> bool {
        if self.contents.len() != other.contents.len() {
            return false;
        }
        if self.sumw2.is_empty() {
            self.sumw2 = self.contents.iter().map(|v| v.abs()).collect();
        }
        for i in 0..self.contents.len() {
            let (c1, c2) = (self.contents[i], other.contents[i]);
            if c2 == 0.0 {
                self.contents[i] = 0.0;
                self.sumw2[i] = 0.0;
                continue;
            }
            let (e1, e2) = (self.sumw2[i], err2(&other.sumw2, &other.contents, i));
            let c2sq = c2 * c2;
            self.sumw2[i] = (e1 * c2sq + e2 * c1 * c1) / (c2sq * c2sq);
            self.contents[i] = c1 / c2;
        }
        true
    }
}

impl TH2 {
    /// Multiply all bin contents (and errors) by `c`.
    pub fn scale(&mut self, c: f64) {
        for v in &mut self.contents {
            *v *= c;
        }
        for v in &mut self.sumw2 {
            *v *= c * c;
        }
        self.tsumw *= c;
        self.tsumw2 *= c * c;
        self.tsumwx *= c;
        self.tsumwx2 *= c;
        self.tsumwy *= c;
        self.tsumwy2 *= c;
        self.tsumwxy *= c;
    }

    /// Add `c * other` into this histogram (merge when `c == 1`). `false` if the
    /// binnings differ.
    pub fn add(&mut self, other: &TH2, c: f64) -> bool {
        if self.contents.len() != other.contents.len() {
            return false;
        }
        if self.sumw2.is_empty() && (!other.sumw2.is_empty() || c != 1.0) {
            self.sumw2 = self.contents.iter().map(|v| v.abs()).collect();
        }
        for i in 0..self.contents.len() {
            self.contents[i] += c * other.contents[i];
        }
        for i in 0..self.sumw2.len() {
            self.sumw2[i] += c * c * err2(&other.sumw2, &other.contents, i);
        }
        self.entries += c * other.entries;
        self.tsumw += c * other.tsumw;
        self.tsumw2 += c * c * other.tsumw2;
        self.tsumwx += c * other.tsumwx;
        self.tsumwx2 += c * other.tsumwx2;
        self.tsumwy += c * other.tsumwy;
        self.tsumwy2 += c * other.tsumwy2;
        self.tsumwxy += c * other.tsumwxy;
        true
    }

    /// Sum of the in-range cell contents (excludes flow on both axes).
    pub fn integral(&self) -> f64 {
        let (nx, ny) = (self.nx(), self.ny());
        let stride = nx + 2;
        (1..=nx)
            .flat_map(|ix| (1..=ny).map(move |iy| (ix, iy)))
            .map(|(ix, iy)| self.contents[ix + stride * iy])
            .sum()
    }
}

impl TH3 {
    /// Multiply all bin contents (and errors) by `c`.
    pub fn scale(&mut self, c: f64) {
        for v in &mut self.contents {
            *v *= c;
        }
        for v in &mut self.sumw2 {
            *v *= c * c;
        }
        self.tsumw *= c;
        self.tsumw2 *= c * c;
        self.tsumwx *= c;
        self.tsumwx2 *= c;
        self.tsumwy *= c;
        self.tsumwy2 *= c;
        self.tsumwxy *= c;
        self.tsumwz *= c;
        self.tsumwz2 *= c;
        self.tsumwxz *= c;
        self.tsumwyz *= c;
    }

    /// Add `c * other` into this histogram (merge when `c == 1`). `false` if the
    /// binnings differ.
    pub fn add(&mut self, other: &TH3, c: f64) -> bool {
        if self.contents.len() != other.contents.len() {
            return false;
        }
        if self.sumw2.is_empty() && (!other.sumw2.is_empty() || c != 1.0) {
            self.sumw2 = self.contents.iter().map(|v| v.abs()).collect();
        }
        for i in 0..self.contents.len() {
            self.contents[i] += c * other.contents[i];
        }
        for i in 0..self.sumw2.len() {
            self.sumw2[i] += c * c * err2(&other.sumw2, &other.contents, i);
        }
        self.entries += c * other.entries;
        self.tsumw += c * other.tsumw;
        self.tsumw2 += c * c * other.tsumw2;
        self.tsumwx += c * other.tsumwx;
        self.tsumwx2 += c * other.tsumwx2;
        self.tsumwy += c * other.tsumwy;
        self.tsumwy2 += c * other.tsumwy2;
        self.tsumwxy += c * other.tsumwxy;
        self.tsumwz += c * other.tsumwz;
        self.tsumwz2 += c * other.tsumwz2;
        self.tsumwxz += c * other.tsumwxz;
        self.tsumwyz += c * other.tsumwyz;
        true
    }

    /// Sum of the in-range cell contents (excludes flow on all axes).
    pub fn integral(&self) -> f64 {
        let (nx, ny, nz) = (self.nx(), self.ny(), self.nz());
        let (sx, sy) = (nx + 2, ny + 2);
        (1..=nx)
            .flat_map(|ix| (1..=ny).flat_map(move |iy| (1..=nz).map(move |iz| (ix, iy, iz))))
            .map(|(ix, iy, iz)| self.contents[ix + sx * (iy + sy * iz)])
            .sum()
    }
}
