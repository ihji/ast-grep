use std::cmp::{max, min};
use std::ops::{Add, Mul, Sub};

/// Represents an inclusive interval [low, high].
/// An empty interval is represented by low > high.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Interval {
  pub low: i64,
  pub high: i64,
}

impl Interval {
  /// Creates a new interval. If `low > high`, it represents an empty interval.
  pub fn new(low: i64, high: i64) -> Self {
    Self { low, high }
  }

  /// Creates an interval representing a single constant value.
  pub fn constant(val: i64) -> Self {
    Self {
      low: val,
      high: val,
    }
  }

  /// Creates an interval representing values less than `val`.
  pub fn create_lt(val: i64) -> Self {
    Self::new(i64::MIN, val.saturating_sub(1))
  }

  /// Creates an interval representing values less than or equal to `val`.
  pub fn create_lte(val: i64) -> Self {
    Self::new(i64::MIN, val)
  }

  /// Creates an interval representing values greater than `val`.
  pub fn create_gt(val: i64) -> Self {
    Self::new(val.saturating_add(1), i64::MAX)
  }

  /// Creates an interval representing values greater than or equal to `val`.
  pub fn create_gte(val: i64) -> Self {
    Self::new(val, i64::MAX)
  }

  /// Represents the set of all possible integers.
  pub fn top() -> Self {
    Self {
      low: i64::MIN,
      high: i64::MAX,
    }
  }

  /// Represents an empty set of integers, indicating a conflict.
  pub fn bottom() -> Self {
    Self { low: 1, high: 0 }
  }

  /// Returns true if the interval is empty.
  pub fn is_empty(&self) -> bool {
    self.low > self.high
  }

  /// Computes the intersection of two intervals.
  /// If the intervals do not overlap, the result is an empty interval.
  pub fn intersection(&self, other: &Self) -> Self {
    if self.is_empty() || other.is_empty() {
      return Self::bottom();
    }
    let low = max(self.low, other.low);
    let high = min(self.high, other.high);
    Self::new(low, high)
  }
}

impl Add for Interval {
  type Output = Self;

  fn add(self, rhs: Self) -> Self::Output {
    if self.is_empty() || rhs.is_empty() {
      return Self::bottom();
    }
    let low = self.low.saturating_add(rhs.low);
    let high = self.high.saturating_add(rhs.high);
    Self::new(low, high)
  }
}

impl Sub for Interval {
  type Output = Self;

  fn sub(self, rhs: Self) -> Self::Output {
    if self.is_empty() || rhs.is_empty() {
      return Self::bottom();
    }
    // [a, b] - [c, d] = [a, b] + [-d, -c] = [a - d, b - c]
    let low = self.low.saturating_sub(rhs.high);
    let high = self.high.saturating_sub(rhs.low);
    Self::new(low, high)
  }
}

impl Mul for Interval {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    if self.is_empty() || rhs.is_empty() {
      return Self::bottom();
    }
    // The new bounds are the min and max of the four products of the endpoints.
    let p1 = self.low.saturating_mul(rhs.low);
    let p2 = self.low.saturating_mul(rhs.high);
    let p3 = self.high.saturating_mul(rhs.low);
    let p4 = self.high.saturating_mul(rhs.high);

    let low = min(min(p1, p2), min(p3, p4));
    let high = max(max(p1, p2), max(p3, p4));
    Self::new(low, high)
  }
}
