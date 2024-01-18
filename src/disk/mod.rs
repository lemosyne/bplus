pub mod error;
mod get;
mod insert;
mod iter;
mod node;
mod persist;
mod remove;

use self::{
    error::Error,
    node::{Link, Node, NodeRef},
};
use serde::Deserialize;
use std::{
    borrow::Borrow,
    fmt::Debug,
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

    fn pretty_print_recursive(&self, node: &Node<K, V>, depth: usize) -> Result<(), Error>
    where
        for<'de> K: Deserialize<'de> + Debug,
        for<'de> V: Deserialize<'de> + Debug,
    {
        print!("{}", "    ".repeat(depth));

        match node {
            Node::Internal(node) => {
                println!("{:?}", node.keys);
                for child in &node.children {
                    unsafe {
                        self.pretty_print_recursive(
                            (*child.as_ptr()).access(&self.path)?,
                            depth + 1,
                        )?;
                    }
                }
            }
            Node::Leaf(node) => {
                println!("{:?}", node.keys);
            }
        }

        Ok(())
    }

    pub fn pretty_print(&self) -> Result<(), Error>
    where
        for<'de> K: Deserialize<'de> + Debug,
        for<'de> V: Deserialize<'de> + Debug,
    {
        unsafe {
            if let Some(root) = self.root {
                self.pretty_print_recursive((*root.as_ptr()).access(&self.path)?, 0)?;
            }
            Ok(())
        }
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
        let mut tree = BPTree::new("/tmp/bptree");

        for i in [25, 4, 1, 16, 9, 20, 13, 15, 10, 11, 12] {
            println!("Insert {i}:");
            tree.insert(i, i.to_string())?;
            tree.pretty_print()?;
        }

        for n in tree.iter().filter_map(Result::ok) {
            println!("{n:?}");
        }

        for n in [13, 15, 1] {
            println!("Delete {n}:");
            tree.remove_entry(&n)?;
            tree.pretty_print()?;
        }

        for n in tree.iter().filter_map(Result::ok) {
            println!("{n:?}");
        }

        for n in [25, 4, 16, 9, 20, 10, 11, 12] {
            println!("Delete {n}:");
            tree.remove_entry(&n)?;
            tree.pretty_print()?;
        }

        Ok(())
    }
}
