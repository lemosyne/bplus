pub mod error;
mod get;
mod insert;
mod node;
mod persist;
mod remove;

use self::{
    error::Error,
    node::{Link, Node},
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
        K: Borrow<Q>,
        Q: Ord,
    {
        Ok(self.get(key)?.is_some())
    }
}

impl<K, V> Drop for BPTree<K, V> {
    fn drop(&mut self) {
        fn recursive_drop<K, V>(node: Link<K, V>) {
            unsafe {
                match node {
                    Link::Loaded(node) => {
                        let boxed_node = Box::from_raw(node.as_ptr());
                        if let Node::Internal(node) = *boxed_node {
                            for child in node.children {
                                recursive_drop(child);
                            }
                        }
                    }
                    Link::Unloaded(_) => {}
                }
            }
        }

        if let Some(root) = &self.root {
            recursive_drop(root.clone());
        }
    }
}
