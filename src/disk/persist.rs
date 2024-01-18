use super::{error::Error, node::Node, BPTree};
use path_macro::path;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Borrow,
    fs,
    path::{Path, PathBuf},
};

impl<K, V> BPTree<K, V> {
    fn root_metadata_path(path: &Path) -> PathBuf {
        path![path / "root"]
    }

    fn order_metadata_path(path: &Path) -> PathBuf {
        path![path / "order"]
    }

    fn len_metadata_path(path: &Path) -> PathBuf {
        path![path / "len"]
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let root = bincode::deserialize(
            &fs::read(Self::root_metadata_path(path.as_ref())).map_err(|_| Error::BadBPTree)?,
        )
        .map_err(|_| Error::Serde)?;

        let order = bincode::deserialize(
            &fs::read(Self::order_metadata_path(path.as_ref())).map_err(|_| Error::BadBPTree)?,
        )
        .map_err(|_| Error::Serde)?;

        let len = bincode::deserialize(
            &fs::read(Self::len_metadata_path(path.as_ref())).map_err(|_| Error::BadBPTree)?,
        )
        .map_err(|_| Error::Serde)?;

        Ok(BPTree {
            path: path.as_ref().into(),
            root,
            root_is_dirty: false,
            order,
            order_is_dirty: false,
            len,
            len_is_dirty: false,
        })
    }

    fn persist_metadata(&mut self) -> Result<(), Error> {
        fs::create_dir_all(&self.path)?;

        if self.root_is_dirty {
            fs::write(
                Self::root_metadata_path(&self.path),
                bincode::serialize(&self.root).map_err(|_| Error::Serde)?,
            )?;
            self.root_is_dirty = false;
        }

        if self.order_is_dirty {
            fs::write(
                Self::order_metadata_path(&self.path),
                bincode::serialize(&self.order).map_err(|_| Error::Serde)?,
            )?;
            self.order_is_dirty = false;
        }

        if self.len_is_dirty {
            fs::write(
                Self::len_metadata_path(&self.path),
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

        let is_dirty = match node {
            Node::Internal(node) => node.is_dirty,
            Node::Leaf(node) => node.is_dirty,
        };

        if is_dirty {
            node.persist(&self.path)?;
        }

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

            let is_dirty = match node {
                Node::Internal(node) => {
                    let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                        Ok(index) => index + 1,
                        Err(index) => index,
                    };
                    cursor = node.children[index];
                    node.is_dirty
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
                    node.is_dirty
                }
            };

            if is_dirty {
                node.persist(&self.path)?;
            }
        }

        Ok(())
    }
}
