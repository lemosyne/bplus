use super::{node::Node, BPTreeMap};
use std::borrow::Borrow;

impl<K, V> BPTreeMap<K, V> {
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe {
            let mut cursor = self.root?;

            while let Node::Internal(node) = &(*cursor.as_ptr()) {
                let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = &(*cursor.as_ptr()) {
                node.keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .map(|index| (&node.keys[index], &node.values[index]))
                    .ok()
            } else {
                None
            }
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.get_key_value(key).map(|(_, value)| value)
    }

    pub fn get_key_value_mut<Q>(&mut self, key: &Q) -> Option<(&K, &mut V)>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe {
            let mut cursor = self.root?;

            while let Node::Internal(node) = &(*cursor.as_ptr()) {
                let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = &mut (*cursor.as_ptr()) {
                node.keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .map(|index| (&node.keys[index], &mut node.values[index]))
                    .ok()
            } else {
                None
            }
        }
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.get_key_value_mut(key).map(|(_, value)| value)
    }
}
