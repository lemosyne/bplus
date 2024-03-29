pub mod error;
mod get;
mod guard;
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
    fmt::{self, Debug},
    path::{Path, PathBuf},
};

const DEFAULT_ORDER: usize = 3;

pub struct BPTree<K, V> {
    path: PathBuf,
    root: Option<Link<K, V>>,
    root_is_dirty: bool,
    order: usize,
    order_is_dirty: bool,
    len: usize,
    len_is_dirty: bool,
}

impl<K, V> BPTree<K, V> {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::with_order(path, DEFAULT_ORDER)
    }

    pub fn with_order(path: impl AsRef<Path>, order: usize) -> Self {
        Self {
            path: path.as_ref().into(),
            root: None,
            root_is_dirty: true,
            order,
            order_is_dirty: true,
            len: 0,
            len_is_dirty: true,
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
                println!(
                    "{:?} {}",
                    node.keys,
                    if node.is_dirty { "[dirty]" } else { "[clean]" }
                );

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
                print!("[");
                for (i, (key, value)) in node.keys.iter().zip(node.values.iter()).enumerate() {
                    print!("{key:?}: {value:?}");
                    if i + 1 != node.keys.len() {
                        print!(", ");
                    }
                }
                println!("] {}", if node.is_dirty { "[dirty]" } else { "[clean]" });
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

impl<K, V> fmt::Debug for BPTree<K, V>
where
    for<'de> K: Deserialize<'de> + Debug,
    for<'de> V: Deserialize<'de> + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (i, (key, value)) in self.iter().filter_map(Result::ok).enumerate() {
            write!(f, "{key:?}: {value:?}")?;
            if i + 1 != self.len {
                write!(f, ", ")?;
            }
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn it_works() -> Result<(), Error> {
        let mut tree = BPTree::new("/tmp/bptree");

        for n in [25, 4, 1, 16, 9, 20, 13, 15, 10, 11, 12] {
            println!("Insert {n}:");
            tree.insert(n, n)?;
            tree.pretty_print()?;
            assert_eq!(tree.get(&n)?, Some(&n));
        }

        for n in tree.iter().filter_map(Result::ok) {
            println!("{n:?}");
        }
        println!("{:?}", tree);

        for n in [13, 15, 1] {
            println!("Delete {n}:");
            assert_eq!(tree.remove_entry(&n)?, Some((n, n)));
            tree.pretty_print()?;
            assert_eq!(tree.get(&n)?, None);
        }
        println!("{:?}", tree);

        for n in tree.iter().filter_map(Result::ok) {
            println!("{n:?}");
        }

        tree.persist()?;
        tree.pretty_print()?;

        let x = tree.get_mut(&4)?;
        *x.unwrap() += 1;
        tree.pretty_print()?;

        tree.persist()?;
        tree.pretty_print()?;

        for n in [25, 4, 16, 9, 20, 10, 11, 12] {
            println!("Delete {n}:");
            tree.remove_entry(&n)?;
            tree.pretty_print()?;
        }

        tree.persist()?;
        tree.pretty_print()?;

        let _ = fs::remove_dir_all("/tmp/bptree");

        Ok(())
    }

    #[test]
    fn reload() -> Result<(), Error> {
        let _ = fs::remove_dir_all("/tmp/bptree-reload");

        let mut tree: BPTree<usize, usize> = BPTree::new("/tmp/bptree-reload");

        for n in 0..10 {
            tree.insert(n, n)?;
        }

        tree.persist()?;

        let tree: BPTree<usize, usize> = BPTree::load("/tmp/bptree-reload")?;

        for n in 0..10 {
            assert_eq!(tree.get(&n)?, Some(&n));
        }

        println!("{tree:?}");

        Ok(())
    }
}
