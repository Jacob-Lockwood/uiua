use std::{
    borrow::Cow,
    fmt,
    hash::Hash,
    ops::{Deref, DerefMut, RangeBounds},
};

use serde::*;
use tinyvec::{ArrayVec, TinyVec};

/// Uiua's array shape type
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Shape {
    dims: TinyVec<[usize; 3]>,
}

impl Shape {
    /// A shape with no dimensions
    pub const SCALAR: Self = Shape {
        dims: TinyVec::Inline(ArrayVec::from_array_empty([0; 3])),
    };
    /// An empty list shape
    pub const EMPTY_LIST: Self = Shape {
        dims: TinyVec::Inline(match ArrayVec::try_from_array_len([0, 0, 0], 1) {
            Ok(v) => v,
            Err(_) => unreachable!(),
        }),
    };
    /// Create a new scalar shape with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Shape {
            dims: TinyVec::with_capacity(capacity),
        }
    }
    /// Remove dimensions in the given range
    pub fn drain(&mut self, range: impl RangeBounds<usize>) {
        self.dims.drain(range);
    }
    /// Add a trailing dimension
    pub fn push(&mut self, dim: usize) {
        self.dims.push(dim);
    }
    /// Remove the last dimension
    pub fn pop(&mut self) -> Option<usize> {
        self.dims.pop()
    }
    /// Insert a dimension at the given index
    pub fn insert(&mut self, index: usize, dim: usize) {
        self.dims.insert(index, dim);
    }
    /// Get a mutable reference to the first dimension, setting it if empty
    pub fn row_count_mut(&mut self) -> &mut usize {
        if self.is_empty() {
            self.push(1);
        }
        &mut self.dims[0]
    }
    /// Remove the dimension at the given index
    pub fn remove(&mut self, index: usize) -> usize {
        self.dims.remove(index)
    }
    /// Get the row count
    #[inline(always)]
    pub fn row_count(&self) -> usize {
        self.dims.first().copied().unwrap_or(1)
    }
    /// Get the row length
    pub fn row_len(&self) -> usize {
        self.dims.iter().skip(1).product()
    }
    /// Get the row shape
    pub fn row(&self) -> Shape {
        let mut shape = self.clone();
        shape.make_row();
        shape
    }
    /// Get the row shape slice
    pub fn row_slice(&self) -> &[usize] {
        &self.dims[self.len().min(1)..]
    }
    /// Get the number of elements
    pub fn elements(&self) -> usize {
        self.iter().product()
    }
    /// Make the shape its row shape
    pub fn make_row(&mut self) {
        if !self.is_empty() {
            self.dims.remove(0);
        }
    }
    /// Make the shape 1-dimensional
    pub fn deshape(&mut self) {
        if self.len() != 1 {
            *self = self.elements().into();
        }
    }
    /// Add a 1-length dimension to the front of the array's shape
    pub fn fix(&mut self) {
        self.fix_depth(0);
    }
    pub(crate) fn fix_depth(&mut self, depth: usize) -> usize {
        let depth = depth.min(self.len());
        self.insert(depth, 1);
        depth
    }
    /// Remove a 1-length dimension from the front of the array's shape
    pub fn unfix(&mut self) -> Result<(), Cow<'static, str>> {
        match self.unfix_inner() {
            Some(1) => Ok(()),
            Some(d) => Err(Cow::Owned(format!("Cannot unfix array with length {d}"))),
            None if self.contains(&0) => Err("Cannot unfix empty array".into()),
            None if self.is_empty() => Err("Cannot unfix scalar".into()),
            None => Err(Cow::Owned(format!(
                "Cannot unfix array with shape {self:?}"
            ))),
        }
    }
    /// Collapse the top two dimensions of the array's shape
    pub fn undo_fix(&mut self) {
        self.unfix_inner();
    }
    /// Unfix the shape
    ///
    /// Returns the first dimension
    fn unfix_inner(&mut self) -> Option<usize> {
        match &mut **self {
            [1, ..] => Some(self.remove(0)),
            [a, b, ..] => {
                let new_first_dim = *a * *b;
                *b = new_first_dim;
                Some(self.remove(0))
            }
            _ => None,
        }
    }
    /// Extend the shape with the given dimensions
    pub fn extend_from_slice(&mut self, dims: &[usize]) {
        self.dims.extend_from_slice(dims);
    }
    /// Split the shape at the given index
    pub fn split_off(&mut self, at: usize) -> Self {
        Shape {
            dims: self.dims.split_off(at),
        }
    }
    /// Get a reference to the dimensions
    pub fn dims(&self) -> &[usize] {
        &self.dims
    }
    /// Get a mutable reference to the dimensions
    pub fn dims_mut(&mut self) -> &mut [usize] {
        &mut self.dims
    }
    /// Truncate the shape
    #[track_caller]
    pub fn truncate(&mut self, len: usize) {
        self.dims.truncate(len);
    }
    pub(crate) fn flat_to_dims(&self, flat: usize, index: &mut Vec<usize>) {
        index.clear();
        let mut flat = flat;
        for &dim in self.dims.iter().rev() {
            index.push(flat % dim);
            flat /= dim;
        }
        index.reverse();
    }
    pub(crate) fn dims_to_flat(&self, index: &[usize]) -> Option<usize> {
        let mut flat = 0;
        for (&dim, &i) in self.dims.iter().zip(index) {
            if i >= dim {
                return None;
            }
            flat = flat * dim + i;
        }
        Some(flat)
    }
    pub(crate) fn i_dims_to_flat(&self, index: &[isize]) -> Option<usize> {
        let mut flat = 0;
        for (&dim, &i) in self.dims.iter().zip(index) {
            if i < 0 || i >= dim as isize {
                return None;
            }
            flat = flat * dim + i as usize;
        }
        Some(flat)
    }
}

impl fmt::Debug for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, dim) in self.dims.iter().enumerate() {
            if i > 0 {
                write!(f, " × ")?;
            }
            write!(f, "{}", dim)?;
        }
        write!(f, "]")
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<usize> for Shape {
    fn from(dim: usize) -> Self {
        Self::from([dim])
    }
}

impl From<&[usize]> for Shape {
    fn from(dims: &[usize]) -> Self {
        Self {
            dims: dims.iter().copied().collect(),
        }
    }
}

impl From<Vec<usize>> for Shape {
    fn from(dims: Vec<usize>) -> Self {
        Self {
            dims: TinyVec::Heap(dims),
        }
    }
}

impl<const N: usize> From<[usize; N]> for Shape {
    fn from(dims: [usize; N]) -> Self {
        dims.as_slice().into()
    }
}

impl Deref for Shape {
    type Target = [usize];
    fn deref(&self) -> &Self::Target {
        &self.dims
    }
}

impl DerefMut for Shape {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dims
    }
}

impl IntoIterator for Shape {
    type Item = usize;
    type IntoIter = <TinyVec<[usize; 3]> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.dims.into_iter()
    }
}

impl<'a> IntoIterator for &'a Shape {
    type Item = &'a usize;
    type IntoIter = <&'a [usize] as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.dims.iter()
    }
}

impl FromIterator<usize> for Shape {
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Self {
        Self {
            dims: iter.into_iter().collect(),
        }
    }
}

impl Extend<usize> for Shape {
    fn extend<I: IntoIterator<Item = usize>>(&mut self, iter: I) {
        self.dims.extend(iter);
    }
}

impl PartialEq<usize> for Shape {
    fn eq(&self, other: &usize) -> bool {
        self == [*other]
    }
}

impl PartialEq<usize> for &Shape {
    fn eq(&self, other: &usize) -> bool {
        *self == [*other]
    }
}

impl<const N: usize> PartialEq<[usize; N]> for Shape {
    fn eq(&self, other: &[usize; N]) -> bool {
        self == other.as_slice()
    }
}

impl<const N: usize> PartialEq<[usize; N]> for &Shape {
    fn eq(&self, other: &[usize; N]) -> bool {
        *self == other.as_slice()
    }
}

impl PartialEq<[usize]> for Shape {
    fn eq(&self, other: &[usize]) -> bool {
        self.dims == other
    }
}

impl PartialEq<[usize]> for &Shape {
    fn eq(&self, other: &[usize]) -> bool {
        *self == other
    }
}

impl PartialEq<&[usize]> for Shape {
    fn eq(&self, other: &&[usize]) -> bool {
        self.dims == *other
    }
}

impl PartialEq<Shape> for &[usize] {
    fn eq(&self, other: &Shape) -> bool {
        other == self
    }
}

impl PartialEq<Shape> for [usize] {
    fn eq(&self, other: &Shape) -> bool {
        other == self
    }
}
