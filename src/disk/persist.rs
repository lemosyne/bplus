use super::{error::Error, node::Node, BPTree};
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, fs};

impl<K, V> BPTree<K, V> {
    unsafe fn persist_recursive(&mut self, node: &mut Node<K, V>) -> Result<(), Error>
    where
        for<'de> K: Deserialize<'de> + Serialize,
        for<'de> V: Deserialize<'de> + Serialize,
    {
        if let Node::Internal(node) = node {
            for child in &node.children {
                self.persist_recursive((*child.as_ptr()).access_mut(&self.path)?)?;
            }
        }

        node.persist(&self.path)?;

        Ok(())
    }

    pub fn persist(&mut self) -> Result<(), Error>
    where
        for<'de> K: Deserialize<'de> + Serialize,
        for<'de> V: Deserialize<'de> + Serialize,
    {
        let root = match self.root {
            Some(root) => root,
            None => return Ok(()),
        };

        fs::create_dir_all(&self.path)?;

        unsafe { self.persist_recursive((*root.as_ptr()).access_mut(&self.path)?) }
    }

    pub fn persist_key<Q>(&mut self, key: &Q) -> Result<(), Error>
    where
        for<'de> K: Deserialize<'de> + Serialize + Borrow<Q>,
        for<'de> V: Deserialize<'de> + Serialize,
        Q: Ord,
    {
        let mut key_persisted = false;
        let mut cursor = self.root.ok_or(Error::UnknownKey)?;

        fs::create_dir_all(&self.path)?;

        while !key_persisted {
            let node = unsafe { (*cursor.as_ptr()).access_mut(&self.path)? };

            match node {
                Node::Internal(node) => {
                    let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                        Ok(index) => index + 1,
                        Err(index) => index,
                    };
                    cursor = node.children[index];
                }
                Node::Leaf(node) => {
                    if node
                        .keys
                        .binary_search_by(|probe| probe.borrow().cmp(key))
                        .is_err()
                    {
                        return Err(Error::UnknownKey);
                    }
                    key_persisted = true;
                }
            }

            node.persist(&self.path)?;
        }

        Ok(())
    }
}
