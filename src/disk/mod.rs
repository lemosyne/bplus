pub mod error;
mod get;
mod insert;
mod iter;
mod node;
mod persist;
mod remove;

use serde::Deserialize;

use self::{
    error::Error,
    node::{Link, Node, NodeRef},
};
use std::{
    borrow::Borrow,
    path::{Path, PathBuf},
};

const DEFAULT_ORDER: usize = 3;

pub struct BPTree<K, V> {
    path: PathBuf,
    root: Option<Link<K, V>>,
    order: usize,
    len: usize,
}

impl<K, V> BPTree<K, V> {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::with_order(path, DEFAULT_ORDER)
    }

    pub fn with_order(path: impl AsRef<Path>, order: usize) -> Self {
        Self {
            path: path.as_ref().into(),
            root: None,
            order,
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn contains_key<Q>(&self, key: &Q) -> Result<bool, Error>
    where
        for<'de> K: Deserialize<'de> + Borrow<Q>,
        for<'de> V: Deserialize<'de>,
        Q: Ord,
    {
        Ok(self.get(key)?.is_some())
    }
}

impl<K, V> Drop for BPTree<K, V> {
    fn drop(&mut self) {
        fn recursive_drop<K, V>(node: Link<K, V>) {
            unsafe {
                match &(*node.as_ptr()) {
                    NodeRef::Loaded(node) => {
                        if let Node::Internal(node) = node {
                            for child in &node.children {
                                recursive_drop(*child);
                            }
                        }
                    }
                    NodeRef::Unloaded(_) => {}
                }

                node.free();
            }
        }

        if let Some(root) = self.root {
            recursive_drop(root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() -> Result<(), Error> {
        // let mut tree =
        todo!()
    }
}
