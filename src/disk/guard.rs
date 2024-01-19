use super::node::{Link, Node};
use serde::Deserialize;
use std::{
    fmt::{self, Debug},
    ops::{Deref, DerefMut},
    path::PathBuf,
};

pub struct ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    pub(crate) value: &'a mut V,
    pub(crate) cursor: Link<K, V>,
    pub(crate) path: &'a PathBuf,
}

impl<'a, K, V> Deref for ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, K, V> DerefMut for ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<'a, K, V> Drop for ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de>,
{
    fn drop(&mut self) {
        unsafe {
            match (*self.cursor.as_ptr()).access_mut(&self.path).unwrap() {
                Node::Internal(node) => node.is_dirty = true,
                Node::Leaf(node) => node.is_dirty = true,
            }
        }
    }
}

impl<'a, K, V> Debug for ValueMutationGuard<'a, K, V>
where
    for<'de> K: Deserialize<'de>,
    for<'de> V: Deserialize<'de> + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.value)
    }
}
