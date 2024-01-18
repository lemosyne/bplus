mod get;
mod insert;
mod iter;
mod node;
mod remove;

use self::node::{Link, Node};
use std::{borrow::Borrow, fmt::Debug};

const DEFAULT_ORDER: usize = 3;

pub struct BPTreeMap<K, V> {
    root: Option<Link<K, V>>,
    order: usize,
    len: usize,
}

impl<K, V> BPTreeMap<K, V> {
    pub fn new() -> Self {
        Self::with_order(DEFAULT_ORDER)
    }

    pub fn with_order(order: usize) -> Self {
        Self {
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

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.get(key).is_some()
    }

    fn pretty_print_recursive(&self, node: &Node<K, V>, depth: usize)
    where
        K: Debug,
        V: Debug,
    {
        print!("{}", "    ".repeat(depth));

        match node {
            Node::Internal(node) => {
                println!("{:?}", node.keys);
                for child in &node.children {
                    unsafe {
                        self.pretty_print_recursive(&(*child.as_ptr()), depth + 1);
                    }
                }
            }
            Node::Leaf(node) => {
                println!("{:?}", node.keys);
            }
        }
    }

    pub fn pretty_print(&self)
    where
        K: Debug,
        V: Debug,
    {
        unsafe {
            if let Some(root) = self.root {
                self.pretty_print_recursive(&(*root.as_ptr()), 0)
            }
        }
    }
}

impl<K, V> Drop for BPTreeMap<K, V> {
    fn drop(&mut self) {
        fn recursive_drop<K, V>(node: Link<K, V>) {
            unsafe {
                let boxed_node = Box::from_raw(node.as_ptr());
                if let Node::Internal(node) = *boxed_node {
                    for child in node.children {
                        recursive_drop(child);
                    }
                }
            }
        }

        if let Some(root) = self.root {
            recursive_drop(root);
        }
    }
}

impl<K, V> Default for BPTreeMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut tree = BPTreeMap::new();

        for n in [25, 4, 1, 16, 9, 20, 13, 15, 10, 11, 12] {
            println!("Insert {n}:");
            tree.insert(n, ());
            tree.pretty_print();
        }

        for n in tree.iter() {
            println!("{n:?}");
        }

        for n in [13, 15, 1] {
            println!("Delete {n}:");
            tree.remove_entry(&n);
            tree.pretty_print();
        }

        for n in tree.iter() {
            println!("{n:?}");
        }

        for n in [25, 4, 16, 9, 20, 10, 11, 12] {
            println!("Delete {n}:");
            tree.remove_entry(&n);
            tree.pretty_print();
        }
    }
}
