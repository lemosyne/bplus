use std::marker::PhantomData;

use super::{
    node::{Link, Node},
    BPTreeMap,
};

impl<K, V> BPTreeMap<K, V> {
    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            cursor: self.root,
            index: 0,
            len: self.len,
            at_leaves: false,
            _lifetime: PhantomData,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            cursor: self.root,
            index: 0,
            len: self.len,
            at_leaves: false,
            _lifetime: PhantomData,
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
    pub(crate) at_leaves: bool,
    pub(crate) _lifetime: PhantomData<(&'a K, &'a V)>,
}

impl<'a, K, V> IntoIterator for &'a BPTreeMap<K, V> {
    type IntoIter = Iter<'a, K, V>;
    type Item = (&'a K, &'a V);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: 'a,
    V: 'a,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }

        let mut cursor = self.cursor?;

        if !self.at_leaves {
            unsafe {
                while let Node::Internal(node) = &(*cursor.as_ptr()) {
                    cursor = node.children[0];
                }

                self.cursor = Some(cursor);
                self.at_leaves = true;
            }
        }

        unsafe {
            if let Node::Leaf(node) = &(*cursor.as_ptr()) {
                let result = (&node.keys[self.index], &node.values[self.index]);

                self.len -= 1;
                self.index += 1;

                if self.index >= node.keys.len() {
                    self.index = 0;
                    self.cursor = node.next_leaf;
                }

                Some(result)
            } else {
                None
            }
        }
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
    pub(crate) at_leaves: bool,
    pub(crate) _lifetime: PhantomData<(&'a K, &'a mut V)>,
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
        if self.len == 0 {
            return None;
        }

        let mut cursor = self.cursor?;

        if !self.at_leaves {
            unsafe {
                while let Node::Internal(node) = &(*cursor.as_ptr()) {
                    cursor = node.children[0];
                }

                self.cursor = Some(cursor);
                self.at_leaves = true;
            }
        }

        unsafe {
            if let Node::Leaf(node) = &mut (*cursor.as_ptr()) {
                let result = (&node.keys[self.index], &mut node.values[self.index]);

                self.len -= 1;
                self.index += 1;

                if self.index >= node.keys.len() {
                    self.index = 0;
                    self.cursor = node.next_leaf;
                }

                Some(result)
            } else {
                None
            }
        }
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
