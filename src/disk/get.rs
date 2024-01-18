use serde::Deserialize;

use super::{
    error::Error,
    node::{Link, Node},
    BPTree,
};
use std::{
    borrow::Borrow,
    fmt::{self, Debug},
    ops::{Deref, DerefMut},
};

impl<K, V> BPTree<K, V> {
    pub fn get_key_value<Q>(&self, key: &Q) -> Result<Option<(&K, &V)>, Error>
    where
        for<'de> K: Deserialize<'de> + Borrow<Q>,
        for<'de> V: Deserialize<'de>,
        Q: Ord,
    {
        if self.root.is_none() {
            return Ok(None);
        }

        unsafe {
            let mut cursor = self.root.unwrap();

            while let Node::Internal(node) = (*cursor.as_ptr()).access(&self.path)? {
                let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = (*cursor.as_ptr()).access(&self.path)? {
                Ok(node
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .map(|index| (&node.keys[index], &node.values[index]))
                    .ok())
            } else {
                Ok(None)
            }
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Result<Option<&V>, Error>
    where
        for<'de> K: Deserialize<'de> + Borrow<Q>,
        for<'de> V: Deserialize<'de>,
        Q: Ord,
    {
        Ok(self.get_key_value(key)?.map(|(_, value)| value))
    }

    pub fn get_key_value_mut<Q>(
        &mut self,
        key: &Q,
    ) -> Result<Option<(&K, ValueMutationGuard<K, V>)>, Error>
    where
        for<'de> K: Deserialize<'de> + Borrow<Q>,
        for<'de> V: Deserialize<'de>,
        Q: Ord,
    {
        if self.root.is_none() {
            return Ok(None);
        }

        unsafe {
            let mut cursor = self.root.unwrap();

            while let Node::Internal(node) = (*cursor.as_ptr()).access(&self.path)? {
                let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = (*cursor.as_ptr()).access_mut(&self.path)? {
                Ok(node
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .map(|index| {
                        (
                            &node.keys[index],
                            ValueMutationGuard {
                                value: &mut node.values[index],
                                cursor: Some(cursor),
                                tree: self,
                            },
                        )
                    })
                    .ok())
            } else {
                Ok(None)
            }
        }
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Result<Option<ValueMutationGuard<K, V>>, Error>
    where
        for<'de> K: Deserialize<'de> + Borrow<Q>,
        for<'de> V: Deserialize<'de>,
        Q: Ord,
    {
        Ok(self.get_key_value_mut(key)?.map(|(_, value)| value))
    }
}

pub struct ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    value: &'a mut V,
    cursor: Option<Link<K, V>>,
    tree: &'a mut BPTree<K, V>,
}

impl<'a, K, V> Deref for ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, K, V> DerefMut for ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<'a, K, V> Drop for ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    fn drop(&mut self) {
        unsafe {
            while let Some(cursor) = self.cursor {
                match (*cursor.as_ptr()).access_mut(&self.tree.path).unwrap() {
                    Node::Internal(node) => {
                        node.dirty = true;
                        self.cursor = node.parent;
                    }
                    Node::Leaf(node) => {
                        node.dirty = true;
                        self.cursor = node.parent;
                    }
                }
            }
        }
    }
}

impl<'a, K, V> Debug for ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de> + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.value)
    }
}
