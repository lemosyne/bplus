use super::{error::Error, node::Node, BPTree};
use std::borrow::Borrow;

impl<K, V> BPTree<K, V> {
    pub fn get_key_value<Q>(&self, key: &Q) -> Result<Option<(&K, &V)>, Error>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        if self.root.is_none() {
            return Ok(None);
        }

        let mut cursor = self.root.unwrap();

        while let Node::Internal(node) = cursor.access(&self.path)? {
            let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                Ok(index) => index + 1,
                Err(index) => index,
            };
            cursor = node.children[index];
        }

        if let Node::Leaf(node) = cursor.access(&self.path)? {
            Ok(node
                .keys
                .binary_search_by(|probe| probe.borrow().cmp(key))
                .map(|index| (&node.keys[index], &node.values[index]))
                .ok())
        } else {
            Ok(None)
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Result<Option<&V>, Error>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        Ok(self.get_key_value(key)?.map(|(_, value)| value))
    }

    pub fn get_mut<Q>(&self, key: &Q) -> Result<Option<&mut V>, Error>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        if self.root.is_none() {
            return Ok(None);
        }

        let mut cursor = self.root.unwrap();

        while let Node::Internal(node) = cursor.access(&self.path)? {
            let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                Ok(index) => index + 1,
                Err(index) => index,
            };
            cursor = node.children[index];
        }

        if let Node::Leaf(node) = cursor.access_mut(&self.path)? {
            Ok(node
                .keys
                .binary_search_by(|probe| probe.borrow().cmp(key))
                .map(|index| &mut node.values[index])
                .ok())
        } else {
            Ok(None)
        }
    }
}
