use super::{error::Error, node::Node, BPTree};
use path_macro::path;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, fs, path::PathBuf};

impl<K, V> BPTree<K, V> {
    fn root_metadata_path(&self) -> PathBuf {
        path![self.path / "root"]
    }

    fn order_metadata_path(&self) -> PathBuf {
        path![self.path / "order"]
    }

    fn len_metadata_path(&self) -> PathBuf {
        path![self.path / "len"]
    }

    fn persist_metadata(&mut self) -> Result<(), Error> {
        fs::create_dir_all(&self.path)?;

        if self.root_is_dirty {
            fs::write(
                self.root_metadata_path(),
                bincode::serialize(&self.root).map_err(|_| Error::Serde)?,
            )?;
            self.root_is_dirty = false;
        }

        if self.order_is_dirty {
            fs::write(
                self.order_metadata_path(),
                bincode::serialize(&self.order).map_err(|_| Error::Serde)?,
            )?;
            self.order_is_dirty = false;
        }

        if self.len_is_dirty {
            fs::write(
                self.len_metadata_path(),
                bincode::serialize(&self.len).map_err(|_| Error::Serde)?,
            )?;
            self.len_is_dirty = false;
        }

        Ok(())
    }

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

        self.persist_metadata()?;

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

        self.persist_metadata()?;

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
