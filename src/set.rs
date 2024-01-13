use std::borrow::Borrow;

use crate::{iter::Keys, BPTreeMap};

pub struct BPTreeSet<K>(BPTreeMap<K, ()>);

impl<K> BPTreeSet<K> {
    pub fn new() -> Self {
        Self(BPTreeMap::new())
    }

    pub fn with_order(order: usize) -> Self {
        Self(BPTreeMap::with_order(order))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&K>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.0.get_key_value(key).map(|(key, _)| key)
    }

    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.0.contains_key(key)
    }

    pub fn insert(&mut self, key: K) -> bool
    where
        K: Ord + Clone,
    {
        self.0.insert(key, ()).is_none()
    }

    pub fn remove<Q>(&mut self, key: &Q) -> bool
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        self.0.remove(key).is_some()
    }

    pub fn iter(&self) -> Iter<K> {
        Iter(self.0.keys())
    }
}

pub struct Iter<'a, K>(Keys<'a, K, ()>);

impl<'a, K> IntoIterator for &'a BPTreeSet<K> {
    type IntoIter = Iter<'a, K>;
    type Item = &'a K;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K> Iterator for Iter<'a, K> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
