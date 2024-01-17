use super::error::Error;
use serde::{Deserialize, Serialize};
use std::{path::Path, ptr::NonNull};
use uuid::Uuid;

pub(crate) enum Link<K, V> {
    Loaded(NonNull<Node<K, V>>),
    Unloaded(Uuid),
}

impl<K, V> Link<K, V> {
    pub fn access(self, _path: &Path) -> Result<&Node<K, V>, Error> {
        match self {
            Self::Loaded(node) => unsafe { Ok(&(*node.as_ptr())) },
            Self::Unloaded(_) => todo!(),
        }
    }

    pub fn access_mut(self, _path: &Path) -> Result<&mut Node<K, V>, Error> {
        match self {
            Self::Loaded(node) => unsafe { Ok(&mut (*node.as_ptr())) },
            Self::Unloaded(_) => todo!(),
        }
    }

    pub fn reclaim(self, _path: &Path) -> Result<(), Error> {
        match self {
            Self::Loaded(node) => {
                let _ = unsafe { Box::from_raw(node.as_ptr()) };
                Ok(())
            }
            Self::Unloaded(_) => todo!(),
        }
    }
}

impl<K, V> Copy for Link<K, V> {}

impl<K, V> Clone for Link<K, V> {
    fn clone(&self) -> Self {
        match self {
            Self::Loaded(node) => Self::Loaded(node.clone()),
            Self::Unloaded(uuid) => Self::Unloaded(uuid.clone()),
        }
    }
}

impl<K, V> PartialEq for Link<K, V> {
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            match (self, other) {
                (Link::Loaded(this), Link::Loaded(other)) => {
                    match (&(*this.as_ptr()), &(*other.as_ptr())) {
                        (Node::Internal(this), Node::Internal(other)) => this.uuid == other.uuid,
                        (Node::Internal(this), Node::Leaf(other)) => this.uuid == other.uuid,
                        (Node::Leaf(this), Node::Internal(other)) => this.uuid == other.uuid,
                        (Node::Leaf(this), Node::Leaf(other)) => this.uuid == other.uuid,
                    }
                }
                (Link::Loaded(this), Link::Unloaded(other_uuid)) => match &(*this.as_ptr()) {
                    Node::Internal(this) => this.uuid == *other_uuid,
                    Node::Leaf(this) => this.uuid == *other_uuid,
                },
                (Link::Unloaded(this_uuid), Link::Loaded(other)) => match &(*other.as_ptr()) {
                    Node::Internal(other) => *this_uuid == other.uuid,
                    Node::Leaf(other) => *this_uuid == other.uuid,
                },
                (Link::Unloaded(this_uuid), Link::Unloaded(other_uuid)) => this_uuid == other_uuid,
            }
        }
    }
}

impl<K, V> Serialize for Link<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        unsafe {
            match self {
                Link::Loaded(node) => match &(*node.as_ptr()) {
                    Node::Internal(node) => node.uuid.serialize(serializer),
                    Node::Leaf(node) => node.uuid.serialize(serializer),
                },
                Link::Unloaded(uuid) => uuid.serialize(serializer),
            }
        }
    }
}

impl<'de, K, V> Deserialize<'de> for Link<K, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Link::Unloaded(Uuid::deserialize(deserializer)?))
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) enum Node<K, V> {
    Internal(Internal<K, V>),
    Leaf(Leaf<K, V>),
}

impl<K, V> Node<K, V> {
    pub fn persist(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        match self {
            Node::Internal(node) => node.persist(path),
            Node::Leaf(node) => node.persist(path),
        }
    }
}

#[derive(Deserialize, Serialize)]
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

    pub fn persist(&self, _path: impl AsRef<Path>) -> Result<(), Error> {
        todo!()
    }
}

#[derive(Deserialize, Serialize)]
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

    pub fn persist(&self, _path: impl AsRef<Path>) -> Result<(), Error> {
        todo!()
    }
}
