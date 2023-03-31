#[allow(clippy::wildcard_imports /* this module exists to reduce file size, not to introduce a new scope */)]
use super::*;

/// A 64-bit float that implements `Eq`, `Ord` and `Hash`.
///
/// Panics if it's `NaN` during any of those operations.
#[derive(Default, Copy, Clone)]
pub struct Scalar(pub f64);

impl Numeric for Scalar {
    #[inline]
    fn zero() -> Self {
        Self(0.0)
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.0.is_finite()
    }
}

impl From<f64> for Scalar {
    #[inline]
    fn from(float: f64) -> Self {
        Self(float)
    }
}

impl From<Scalar> for f64 {
    #[inline]
    fn from(scalar: Scalar) -> Self {
        scalar.0
    }
}

impl Debug for Scalar {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Ord for Scalar {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).expect("float is NaN")
    }
}

impl PartialOrd for Scalar {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }

    #[inline]
    fn lt(&self, other: &Self) -> bool {
        self.0 < other.0
    }

    #[inline]
    fn le(&self, other: &Self) -> bool {
        self.0 <= other.0
    }

    #[inline]
    fn gt(&self, other: &Self) -> bool {
        self.0 > other.0
    }

    #[inline]
    fn ge(&self, other: &Self) -> bool {
        self.0 >= other.0
    }
}

impl Eq for Scalar {}

impl PartialEq for Scalar {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        assert!(!self.0.is_nan() && !other.0.is_nan(), "float is NaN");
        self.0 == other.0
    }
}

impl PartialEq<f64> for Scalar {
    #[inline]
    fn eq(&self, other: &f64) -> bool {
        self == &Self(*other)
    }
}

impl Hash for Scalar {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        debug_assert!(!self.0.is_nan(), "float is NaN");
        self.0.to_bits().hash(state);
    }
}

impl Neg for Scalar {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl<T: Into<Self>> Add<T> for Scalar {
    type Output = Self;

    #[inline]
    fn add(self, rhs: T) -> Self::Output {
        Self(self.0 + rhs.into().0)
    }
}

impl<T: Into<Self>> AddAssign<T> for Scalar {
    #[inline]
    fn add_assign(&mut self, rhs: T) {
        self.0 += rhs.into().0;
    }
}

impl<T: Into<Self>> Sub<T> for Scalar {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: T) -> Self::Output {
        Self(self.0 - rhs.into().0)
    }
}

impl<T: Into<Self>> SubAssign<T> for Scalar {
    #[inline]
    fn sub_assign(&mut self, rhs: T) {
        self.0 -= rhs.into().0;
    }
}

impl<T: Into<Self>> Mul<T> for Scalar {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: T) -> Self::Output {
        Self(self.0 * rhs.into().0)
    }
}

impl<T: Into<Self>> MulAssign<T> for Scalar {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        self.0 *= rhs.into().0;
    }
}

impl<T: Into<Self>> Div<T> for Scalar {
    type Output = Self;

    #[inline]
    fn div(self, rhs: T) -> Self::Output {
        Self(self.0 / rhs.into().0)
    }
}

impl<T: Into<Self>> DivAssign<T> for Scalar {
    #[inline]
    fn div_assign(&mut self, rhs: T) {
        self.0 /= rhs.into().0;
    }
}

impl<T: Into<Self>> Rem<T> for Scalar {
    type Output = Self;

    #[inline]
    fn rem(self, rhs: T) -> Self::Output {
        Self(self.0 % rhs.into().0)
    }
}

impl<T: Into<Self>> RemAssign<T> for Scalar {
    #[inline]
    fn rem_assign(&mut self, rhs: T) {
        self.0 %= rhs.into().0;
    }
}

impl Sum for Scalar {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self(iter.map(|s| s.0).sum())
    }
}

impl<'a> Sum<&'a Self> for Scalar {
    #[inline]
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        Self(iter.map(|s| s.0).sum())
    }
}
