use serde::Deserialize;

use super::{
    error::Error,
    node::{Link, Node},
    BPTree,
};

impl<K, V> BPTree<K, V> {
    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            cursor: self.root,
            index: 0,
            len: self.len,
            errored: false,
            at_leaves: false,
            tree: self,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            cursor: self.root,
            index: 0,
            len: self.len,
            errored: false,
            at_leaves: false,
            tree: self,
        }
    }
}

impl<'a, K, V> IntoIterator for &'a BPTree<K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    type IntoIter = Iter<'a, K, V>;
    type Item = Result<(&'a K, &'a V), Error>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct Iter<'a, K, V> {
    pub(crate) cursor: Option<Link<K, V>>,
    pub(crate) index: usize,
    pub(crate) len: usize,
    pub(crate) errored: bool,
    pub(crate) at_leaves: bool,
    pub(crate) tree: &'a BPTree<K, V>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    type Item = Result<(&'a K, &'a V), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 || self.errored {
            return None;
        }

        let mut cursor = self.cursor?;

        if !self.at_leaves {
            loop {
                unsafe {
                    match (*cursor.as_ptr()).access(&self.tree.path) {
                        Ok(node) => match node {
                            Node::Internal(node) => {
                                cursor = node.children[0];
                            }
                            Node::Leaf(_) => {
                                self.cursor = Some(cursor);
                                self.at_leaves = true;
                                break;
                            }
                        },
                        Err(err) => {
                            self.errored = true;
                            return Some(Err(err));
                        }
                    }
                }
            }
        }

        unsafe {
            match (*cursor.as_ptr()).access(&self.tree.path) {
                Ok(node) => match node {
                    Node::Internal(_) => None,
                    Node::Leaf(node) => {
                        let result = (&node.keys[self.index], &node.values[self.index]);

                        self.len -= 1;
                        self.index += 1;

                        if self.index >= node.keys.len() {
                            self.index = 0;
                            self.cursor = node.next_leaf;
                        }

                        Some(Ok(result))
                    }
                },
                Err(err) => {
                    self.errored = true;
                    Some(Err(err))
                }
            }
        }
    }
}

pub struct IterMut<'a, K, V> {
    pub(crate) cursor: Option<Link<K, V>>,
    pub(crate) index: usize,
    pub(crate) len: usize,
    pub(crate) errored: bool,
    pub(crate) at_leaves: bool,
    pub(crate) tree: &'a mut BPTree<K, V>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    type Item = Result<(&'a K, &'a mut V), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 || self.errored {
            return None;
        }

        let mut cursor = self.cursor?;

        if !self.at_leaves {
            loop {
                unsafe {
                    match (*cursor.as_ptr()).access(&self.tree.path) {
                        Ok(node) => match node {
                            Node::Internal(node) => {
                                cursor = node.children[0];
                            }
                            Node::Leaf(_) => {
                                self.cursor = Some(cursor);
                                self.at_leaves = true;
                                break;
                            }
                        },
                        Err(err) => {
                            self.errored = true;
                            return Some(Err(err));
                        }
                    }
                }
            }
        }

        unsafe {
            match (*cursor.as_ptr()).access_mut(&self.tree.path) {
                Ok(node) => match node {
                    Node::Internal(_) => None,
                    Node::Leaf(node) => {
                        let result = (&node.keys[self.index], &mut node.values[self.index]);

                        self.len -= 1;
                        self.index += 1;

                        if self.index >= node.keys.len() {
                            self.index = 0;
                            self.cursor = node.next_leaf;
                        }

                        Some(Ok(result))
                    }
                },
                Err(err) => {
                    self.errored = true;
                    Some(Err(err))
                }
            }
        }
    }
}
