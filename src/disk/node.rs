use super::error::Error;
use std::{path::Path, ptr::NonNull};
use uuid::Uuid;

pub(crate) type Link<K, V> = NonNull<NodeRef<K, V>>;

pub(crate) enum NodeRef<K, V> {
    Loaded(Node<K, V>),
    Unloaded(Uuid),
}

impl<K, V> NodeRef<K, V> {
    pub(crate) fn access(&mut self, _path: &Path) -> Result<&mut Node<K, V>, Error> {
        match self {
            Self::Loaded(node) => Ok(node),
            Self::Unloaded(_) => todo!(),
        }
    }
}

pub(crate) enum Node<K, V> {
    Internal(Internal<K, V>),
    Leaf(Leaf<K, V>),
}

pub(crate) struct Internal<K, V> {
    pub(crate) uuid: Uuid,
    pub(crate) keys: Vec<K>,
    pub(crate) children: Vec<Link<K, V>>,
    pub(crate) parent: Option<Link<K, V>>,
}

impl<K, V> Internal<K, V> {
    pub fn is_underfull(&self, order: usize) -> bool {
        self.keys.len() < order / 2
    }

    pub fn is_overfull(&self, order: usize) -> bool {
        self.keys.len() > order
    }

    pub fn has_extra_keys(&self, order: usize) -> bool {
        self.keys.len() > order / 2
    }
}

pub(crate) struct Leaf<K, V> {
    pub(crate) uuid: Uuid,
    pub(crate) keys: Vec<K>,
    pub(crate) values: Vec<V>,
    pub(crate) parent: Option<Link<K, V>>,
    pub(crate) next_leaf: Option<Link<K, V>>,
}

impl<K, V> Leaf<K, V> {
    pub fn is_underfull(&self, order: usize) -> bool {
        self.keys.len() < order.div_ceil(2)
    }

    pub fn is_overfull(&self, order: usize) -> bool {
        self.keys.len() > order
    }

    pub fn has_extra_keys(&self, order: usize) -> bool {
        self.keys.len() > order.div_ceil(2)
    }
}
