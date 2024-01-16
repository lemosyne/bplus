use super::{
    node::{Link, Node},
    BPTreeMap,
};
use std::marker::PhantomData;

impl<K, V> BPTreeMap<K, V> {
    pub fn iter(&self) -> Iter<K, V> {
        if let Some(root) = self.root {
            unsafe {
                let mut cursor = root;
                loop {
                    match &(*cursor.as_ptr()) {
                        Node::Internal(node) => cursor = node.children[0],
                        Node::Leaf(_) => {
                            break Iter {
                                cursor: Some(cursor),
                                index: 0,
                                len: self.len,
                                _pd: PhantomData,
                            }
                        }
                    }
                }
            }
        } else {
            Iter {
                cursor: None,
                index: 0,
                len: 0,
                _pd: PhantomData,
            }
        }
    }

    pub fn iter_mut(&self) -> IterMut<K, V> {
        if let Some(root) = self.root {
            unsafe {
                let mut cursor = root;
                loop {
                    match &(*cursor.as_ptr()) {
                        Node::Internal(node) => cursor = node.children[0],
                        Node::Leaf(_) => {
                            break IterMut {
                                cursor: Some(cursor),
                                index: 0,
                                len: self.len,
                                _pd: PhantomData,
                            }
                        }
                    }
                }
            }
        } else {
            IterMut {
                cursor: None,
                index: 0,
                len: 0,
                _pd: PhantomData,
            }
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

pub struct Iter<'a, K, V> {
    pub(crate) cursor: Option<Link<K, V>>,
    pub(crate) index: usize,
    pub(crate) len: usize,
    pub(crate) _pd: PhantomData<&'a (K, V)>,
}

impl<'a, K, V> IntoIterator for &'a BPTreeMap<K, V> {
    type IntoIter = Iter<'a, K, V>;
    type Item = (&'a K, &'a V);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        (self.len > 0)
            .then(|| {
                self.cursor.and_then(|node| unsafe {
                    if let Node::Leaf(node) = &(*node.as_ptr()) {
                        let result = Some((&node.keys[self.index], &node.values[self.index]));

                        // Advance in the index in the node, moving to
                        // the next leaf if we've hit the end.
                        self.index += 1;
                        if self.index >= node.keys.len() {
                            self.index = 0;
                            self.cursor = node.next_leaf;
                        }

                        self.len -= 1;
                        result
                    } else {
                        None
                    }
                })
            })
            .flatten()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, K, V> ExactSizeIterator for Iter<'a, K, V> {
    fn len(&self) -> usize {
        self.len
    }
}

pub struct IterMut<'a, K, V> {
    pub(crate) cursor: Option<Link<K, V>>,
    pub(crate) index: usize,
    pub(crate) len: usize,
    pub(crate) _pd: PhantomData<&'a (K, V)>,
}

impl<'a, K, V> IntoIterator for &'a mut BPTreeMap<K, V> {
    type IntoIter = IterMut<'a, K, V>;
    type Item = (&'a K, &'a mut V);

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        (self.len > 0)
            .then(|| {
                self.cursor.and_then(|node| unsafe {
                    if let Node::Leaf(node) = &mut (*node.as_ptr()) {
                        let result = Some((&node.keys[self.index], &mut node.values[self.index]));

                        // Advance in the index in the node, moving to
                        // the next leaf if we've hit the end.
                        self.index += 1;
                        if self.index >= node.keys.len() {
                            self.index = 0;
                            self.cursor = node.next_leaf;
                        }

                        self.len -= 1;
                        result
                    } else {
                        None
                    }
                })
            })
            .flatten()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, K, V> ExactSizeIterator for IterMut<'a, K, V> {
    fn len(&self) -> usize {
        self.len
    }
}

pub struct Keys<'a, K, V>(pub(crate) Iter<'a, K, V>);

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }
}

pub struct Values<'a, K, V>(pub(crate) Iter<'a, K, V>);

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }
}

pub struct ValuesMut<'a, K, V>(pub(crate) IterMut<'a, K, V>);

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }
}
