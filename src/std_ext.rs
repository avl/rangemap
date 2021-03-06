use std::ops::Range;

pub trait RangeExt<T> {
    fn overlaps(&self, other: &Self) -> bool;
    fn touches(&self, other: &Self) -> bool;
    // TODO: Remove once https://github.com/rust-lang/rust/issues/32311
    // is stabilized.
    fn contains_item(&self, item: &T) -> bool;
}

impl<T> RangeExt<T> for Range<T>
where
    T: Ord,
{
    fn overlaps(&self, other: &Self) -> bool {
        use std::cmp::{max, min};
        // Strictly less than, because ends are excluded.
        max(&self.start, &other.start) < min(&self.end, &other.end)
    }

    fn touches(&self, other: &Self) -> bool {
        use std::cmp::{max, min};
        // Less-than-or-equal-to because if one end is excluded, the other is included.
        // I.e. the two could be joined into a single range, because they're overlapping
        // or immediately adjacent.
        max(&self.start, &other.start) <= min(&self.end, &other.end)
    }

    // TODO: Remove once https://github.com/rust-lang/rust/issues/32311
    // is stabilized.
    fn contains_item(&self, item: &T) -> bool {
        *item >= self.start && *item < self.end
    }
}
