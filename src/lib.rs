// Relies on 'range_contains' feature, which is very soon
// to be stabilised.
//
// https://github.com/rust-lang/rust/issues/32311
//
// Until then, we're tracking nightly.
#![feature(range_contains)]

use std::collections::BTreeMap;
use std::ops::Range;

pub struct RangeMap<K, V> {
    // Inner B-Tree map. Stores pairs of ranges and their
    // associated keys, indexed by the range starts.
    //
    // REVISIT: Experiment with using two separate `BTreeMap`s
    // for the start and end of ranges; it might make inserts/removals
    // more efficient and the logic simpler.
    btm: BTreeMap<K, (Range<K>, V)>,
}

impl<K, V> RangeMap<K, V>
where
    K: Ord + Clone,
    V: Eq + Clone,
{
    pub fn new() -> RangeMap<K, V> {
        RangeMap {
            btm: BTreeMap::new(),
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        use std::ops::Bound;

        // The only stored range that could contain the given key is the
        // last stored range whose start is less than or equal to this key.
        self.btm
            .range((Bound::Unbounded, Bound::Included(key)))
            .next_back()
            .filter(|(_start, (stored_range, _value))| {
                // Does the only candidate range contain
                // the requested key?
                stored_range.contains(key)
            })
            .map(|(_start, (_stored_range, value))| value)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Range<K>, V)> {
        self.btm.values()
    }

    pub fn insert(&mut self, range: Range<K>, value: V) {
        use std::ops::Bound;

        // We don't want to have to think about empty ranges.
        if range.start == range.end {
            return;
        }

        // We want to be able to expand the range's start and end
        // to "swallow up" any overlapping or immediately-adjacent
        // stored ranges of the same value.
        //
        // Could've just put this on the parameter above, but it looks
        // a little weird, and it's nice to be able to clarify the
        // names a bit so we don't get different ranges confused. :)
        let mut new_range = range;
        let new_value = value;

        // Is there a stored range either overlapping the start of
        // the range to insert or immediately preceding it?
        //
        // If there is any such stored range, it will be the last
        // whose start is less than or equal to the start of the range to insert.
        if let Some((stored_range, stored_value)) = self
            .btm
            .range((Bound::Unbounded, Bound::Included(&new_range.start)))
            .next_back()
            .filter(|(_start, (stored_range, _stored_value))| {
                // Does the only candidate range either overlap
                // or immediately precede the range to insert?
                // (Remember that it might actually cover the _whole_
                // range to insert and then some.)
                stored_range.touches(&new_range)
            })
            .map(|(_start, (stored_range, stored_value))| (stored_range, stored_value))
        {
            self.adjust_touching_ranges_for_insert(
                stored_range.clone(),
                (*stored_value).clone(),
                &mut new_range,
                &new_value,
            );
        }

        // Are there any stored ranges whose heads overlap or immediately
        // follow the range to insert?
        //
        // If there are any such stored ranges (that weren't already caught above),
        // their starts will fall somewhere after the start of the range to insert,
        // and on or before its end.
        //
        // This time around, if the latter holds, it also implies
        // the former so we don't need to check here if they touch.
        while let Some((stored_range, stored_value)) = self
            .btm
            .range((
                Bound::Excluded(&new_range.start),
                Bound::Included(&new_range.end),
            ))
            .next()
            .map(|(_start, (stored_range, stored_value))| (stored_range, stored_value))
        {
            // One extra exception: if we have different values,
            // and the stored range starts at the end of the range to insert,
            // then we don't want to keep looping forever trying to find more!
            if stored_range.start == new_range.end && *stored_value != new_value {
                break;
            }

            self.adjust_touching_ranges_for_insert(
                stored_range.clone(),
                (*stored_value).clone(),
                &mut new_range,
                &new_value,
            );
        }

        // Insert the (possibly expanded) new range, and we're done!
        self.btm
            .insert(new_range.start.clone(), (new_range, new_value));
    }

    fn adjust_touching_ranges_for_insert(
        &mut self,
        stored_range: Range<K>,
        stored_value: V,
        new_range: &mut Range<K>,
        new_value: &V,
    ) {
        use std::cmp::{max, min};

        if stored_value == *new_value {
            // The ranges have the same value, so we can "adopt"
            // the stored range.
            //
            // This means that no matter how big or where the stored range is,
            // we will expand the new range's bounds to subsume it,
            // and then delete the stored range.
            new_range.start = min(&new_range.start, &stored_range.start).clone();
            new_range.end = max(&new_range.end, &stored_range.end).clone();
            self.btm.remove(&stored_range.start);
        } else {
            // The ranges have different values.
            if new_range.overlaps(&stored_range) {
                // The ranges overlap. This is a little bit more complicated.
                // Delete the stored range, and then add back between
                // 0 and 2 subranges at the ends of the range to insert.
                self.btm.remove(&stored_range.start);
                if stored_range.start < new_range.start {
                    // Insert the piece left of the range to insert.
                    self.btm.insert(
                        stored_range.start.clone(),
                        (
                            stored_range.start..new_range.start.clone(),
                            stored_value.clone(),
                        ),
                    );
                }
                if stored_range.end > new_range.end {
                    // Insert the piece right of the range to insert.
                    self.btm.insert(
                        new_range.end.clone(),
                        (new_range.end.clone()..stored_range.end, stored_value),
                    );
                }
            } else {
                // No-op; they're not overlapping,
                // so we can just keep both ranges as they are.
            }
        }
    }
}

trait RangeExt<T> {
    fn overlaps(&self, other: &Self) -> bool;
    fn touches(&self, other: &Self) -> bool;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    trait RangeMapExt<K, V> {
        fn to_vec(&self) -> Vec<(Range<K>, V)>;
    }

    impl<K, V> RangeMapExt<K, V> for RangeMap<K, V>
    where
        K: Ord + Clone,
        V: Eq + Clone,
    {
        fn to_vec(&self) -> Vec<(Range<K>, V)> {
            self.iter().cloned().collect()
        }
    }

    #[test]
    fn empty_map_is_empty() {
        let range_map: RangeMap<u32, bool> = RangeMap::new();
        assert_eq!(range_map.to_vec(), vec![]);
    }

    #[test]
    fn insert_into_empty_map() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        range_map.insert(0..50, false);
        assert_eq!(range_map.to_vec(), vec![(0..50, false)]);
    }

    #[test]
    fn new_same_value_immediately_following_stored() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        range_map.insert(1..3, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◌ ◌ ●---◌ ◌ ◌ ◌ ◌
        range_map.insert(3..5, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●-------◌ ◌ ◌ ◌ ◌
        assert_eq!(range_map.to_vec(), vec![(1..5, false)]);
    }

    #[test]
    fn new_different_value_immediately_following_stored() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        range_map.insert(1..3, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◌ ◌ ◆---◇ ◌ ◌ ◌ ◌
        range_map.insert(3..5, true);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        // ◌ ◌ ◌ ◆---◇ ◌ ◌ ◌ ◌
        assert_eq!(range_map.to_vec(), vec![(1..3, false), (3..5, true)]);
    }

    #[test]
    fn new_same_value_overlapping_end_of_stored() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●-----◌ ◌ ◌ ◌ ◌ ◌
        range_map.insert(1..4, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◌ ◌ ●---◌ ◌ ◌ ◌ ◌
        range_map.insert(3..5, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●-------◌ ◌ ◌ ◌ ◌
        assert_eq!(range_map.to_vec(), vec![(1..5, false)]);
    }

    #[test]
    fn new_different_value_overlapping_end_of_stored() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●-----◌ ◌ ◌ ◌ ◌ ◌
        range_map.insert(1..4, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◌ ◌ ◆---◇ ◌ ◌ ◌ ◌
        range_map.insert(3..5, true);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        // ◌ ◌ ◌ ◆---◇ ◌ ◌ ◌ ◌
        assert_eq!(range_map.to_vec(), vec![(1..3, false), (3..5, true)]);
    }

    #[test]
    fn new_same_value_immediately_preceding_stored() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◌ ◌ ●---◌ ◌ ◌ ◌ ◌
        range_map.insert(3..5, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        range_map.insert(1..3, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●-------◌ ◌ ◌ ◌ ◌
        assert_eq!(range_map.to_vec(), vec![(1..5, false)]);
    }

    #[test]
    fn new_different_value_immediately_preceding_stored() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◌ ◌ ◆---◇ ◌ ◌ ◌ ◌
        range_map.insert(3..5, true);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        range_map.insert(1..3, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        // ◌ ◌ ◌ ◆---◇ ◌ ◌ ◌ ◌
        assert_eq!(range_map.to_vec(), vec![(1..3, false), (3..5, true)]);
    }

    #[test]
    fn new_same_value_wholly_inside_stored() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●-------◌ ◌ ◌ ◌ ◌
        range_map.insert(1..5, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        range_map.insert(2..4, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●-------◌ ◌ ◌ ◌ ◌
        assert_eq!(range_map.to_vec(), vec![(1..5, false)]);
    }

    #[test]
    fn new_different_value_wholly_inside_stored() {
        let mut range_map: RangeMap<u32, bool> = RangeMap::new();
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◆-------◇ ◌ ◌ ◌ ◌
        range_map.insert(1..5, true);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ◌ ●---◌ ◌ ◌ ◌ ◌ ◌ ◌
        range_map.insert(2..4, false);
        // 0 1 2 3 4 5 6 7 8 9
        // ◌ ●-◌ ◌ ◌ ◌ ◌ ◌ ◌ ◌
        // ◌ ◌ ◆---◇ ◌ ◌ ◌ ◌ ◌
        // ◌ ◌ ◌ ◌ ●-◌ ◌ ◌ ◌ ◌
        assert_eq!(
            range_map.to_vec(),
            vec![(1..2, true), (2..4, false), (4..5, true)]
        );
    }

    // TODO: Build StupidRangeMap that is just a `BTreeMap`
    // of individual values, which inserts ranges as individual values.
    //
    // Use this to exhaustively test every step of every permutation
    // of a bunch of overlapping and touching ranges.
}