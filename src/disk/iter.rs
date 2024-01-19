use super::{
    error::Error,
    guard::ValueMutationGuard,
    node::{Link, Node},
    BPTree,
};
use serde::Deserialize;
use std::path::PathBuf;

impl<K, V> BPTree<K, V> {
    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            cursor: self.root,
            index: 0,
            len: self.len,
            errored: false,
            at_leaves: false,
            path: &self.path,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            cursor: self.root,
            index: 0,
            len: self.len,
            errored: false,
            at_leaves: false,
            path: &self.path,
        }
    }

    pub fn keys(&self) -> Keys<K, V> {
        Keys(self.iter())
    }

    pub fn values(&self) -> Values<K, V> {
        Values(self.iter())
    }

    pub fn values_mut(&mut self) -> ValuesMut<K, V> {
        ValuesMut(self.iter_mut())
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
    pub(crate) path: &'a PathBuf,
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    for<'de> K: Deserialize<'de> + 'a,
    for<'de> V: Deserialize<'de> + 'a,
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
                    match (*cursor.as_ptr()).access(&self.path) {
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
            match (*cursor.as_ptr()).access(&self.path) {
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

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, K, V> ExactSizeIterator for Iter<'a, K, V>
where
    for<'de> K: Deserialize<'de> + 'a,
    for<'de> V: Deserialize<'de> + 'a,
{
    fn len(&self) -> usize {
        self.len
    }
}

pub struct IterMut<'a, K, V> {
    pub(crate) cursor: Option<Link<K, V>>,
    pub(crate) index: usize,
    pub(crate) len: usize,
    pub(crate) errored: bool,
    pub(crate) at_leaves: bool,
    pub(crate) path: &'a PathBuf,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V>
where
    for<'de> K: Deserialize<'de> + 'a,
    for<'de> V: Deserialize<'de> + 'a,
{
    type Item = Result<(&'a K, ValueMutationGuard<'a, K, V>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 || self.errored {
            return None;
        }

        let mut cursor = self.cursor?;

        if !self.at_leaves {
            loop {
                unsafe {
                    match (*cursor.as_ptr()).access(&self.path) {
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
            match (*cursor.as_ptr()).access_mut(&self.path) {
                Ok(node) => match node {
                    Node::Internal(_) => None,
                    Node::Leaf(node) => {
                        let result = (
                            &node.keys[self.index],
                            ValueMutationGuard {
                                value: &mut node.values[self.index],
                                cursor,
                                path: &self.path,
                            },
                        );

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

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, K, V> ExactSizeIterator for IterMut<'a, K, V>
where
    for<'de> K: Deserialize<'de> + 'a,
    for<'de> V: Deserialize<'de> + 'a,
{
    fn len(&self) -> usize {
        self.len
    }
}
pub struct Keys<'a, K, V>(pub(crate) Iter<'a, K, V>);

impl<'a, K, V> Iterator for Keys<'a, K, V>
where
    for<'de> K: Deserialize<'de> + 'a,
    for<'de> V: Deserialize<'de> + 'a,
{
    type Item = Result<&'a K, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|res| res.and_then(|(key, _)| Ok(key)))
    }
}

pub struct Values<'a, K, V>(pub(crate) Iter<'a, K, V>);

impl<'a, K, V> Iterator for Values<'a, K, V>
where
    for<'de> K: Deserialize<'de> + 'a,
    for<'de> V: Deserialize<'de> + 'a,
{
    type Item = Result<&'a V, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|res| res.and_then(|(_, value)| Ok(value)))
    }
}

pub struct ValuesMut<'a, K, V>(pub(crate) IterMut<'a, K, V>);

impl<'a, K, V> Iterator for ValuesMut<'a, K, V>
where
    for<'de> K: Deserialize<'de> + 'a,
    for<'de> V: Deserialize<'de> + 'a,
{
    type Item = Result<ValueMutationGuard<'a, K, V>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|res| res.and_then(|(_, value)| Ok(value)))
    }
}
