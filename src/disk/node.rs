use super::error::Error;
use path_macro::path;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fs,
    ops::{Deref, DerefMut},
    path::Path,
    ptr::NonNull,
};
use uuid::Uuid;

pub struct Link<K, V>(NonNull<NodeRef<K, V>>);

impl<K, V> Link<K, V> {
    pub fn new(node: Node<K, V>) -> Self {
        unsafe {
            Self(NonNull::new_unchecked(Box::into_raw(Box::new(
                NodeRef::Loaded(node),
            ))))
        }
    }

    pub fn free(self) {
        unsafe {
            let _ = Box::from_raw(self.as_ptr());
        }
    }

    pub fn reclaim(self, path: &Path) -> Result<(), Error> {
        unsafe {
            (*self.as_ptr()).reclaim(path)?;
            self.free();
            Ok(())
        }
    }
}

impl<K, V> Clone for Link<K, V> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<K, V> Copy for Link<K, V> {}

impl<K, V> Deref for Link<K, V> {
    type Target = NonNull<NodeRef<K, V>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V> DerefMut for Link<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K, V> PartialEq for Link<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<K, V> Serialize for Link<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        unsafe { (*self.0.as_ptr()).serialize(serializer) }
    }
}

impl<'de, K, V> Deserialize<'de> for Link<K, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        unsafe {
            Ok(Link(NonNull::new_unchecked(Box::into_raw(Box::new(
                NodeRef::deserialize(deserializer)?,
            )))))
        }
    }
}

pub enum NodeRef<K, V> {
    Loaded(Node<K, V>),
    Unloaded(Uuid),
}

impl<K, V> NodeRef<K, V> {
    pub unsafe fn access(&mut self, path: &Path) -> Result<&Node<K, V>, Error>
    where
        for<'de> K: Deserialize<'de>,
        for<'de> V: Deserialize<'de>,
    {
        match self {
            Self::Loaded(node) => Ok(node),
            Self::Unloaded(uuid) => {
                let path = path![path / uuid.to_string()];
                let data = fs::read(&path)?;
                let node = bincode::deserialize(&data).map_err(|_| Error::Serde)?;
                *self = Self::Loaded(node);
                self.access(&path)
            }
        }
    }

    pub unsafe fn access_mut(&mut self, path: &Path) -> Result<&mut Node<K, V>, Error>
    where
        for<'de> K: Deserialize<'de>,
        for<'de> V: Deserialize<'de>,
    {
        match self {
            Self::Loaded(node) => Ok(node),
            Self::Unloaded(uuid) => {
                let path = path![path / uuid.to_string()];
                let data = fs::read(&path)?;
                let node = bincode::deserialize(&data).map_err(|_| Error::Serde)?;
                *self = Self::Loaded(node);
                self.access_mut(&path)
            }
        }
    }

    pub fn reclaim(&self, path: &Path) -> Result<(), Error> {
        match self {
            Self::Loaded(node) => match node {
                Node::Internal(node) => {
                    let _ = fs::remove_file(path![path / node.uuid.to_string()]);
                }
                Node::Leaf(node) => {
                    let _ = fs::remove_file(path![path / node.uuid.to_string()]);
                }
            },
            Self::Unloaded(uuid) => {
                let _ = fs::remove_file(path![path / uuid.to_string()]);
            }
        }
        Ok(())
    }
}

impl<K, V> PartialEq for NodeRef<K, V> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NodeRef::Loaded(this), NodeRef::Loaded(other)) => match (this, other) {
                (Node::Internal(this), Node::Internal(other)) => this.uuid == other.uuid,
                (Node::Internal(this), Node::Leaf(other)) => this.uuid == other.uuid,
                (Node::Leaf(this), Node::Internal(other)) => this.uuid == other.uuid,
                (Node::Leaf(this), Node::Leaf(other)) => this.uuid == other.uuid,
            },
            (NodeRef::Loaded(this), NodeRef::Unloaded(other_uuid)) => match this {
                Node::Internal(this) => this.uuid == *other_uuid,
                Node::Leaf(this) => this.uuid == *other_uuid,
            },
            (NodeRef::Unloaded(this_uuid), NodeRef::Loaded(other)) => match other {
                Node::Internal(other) => *this_uuid == other.uuid,
                Node::Leaf(other) => *this_uuid == other.uuid,
            },
            (NodeRef::Unloaded(this_uuid), NodeRef::Unloaded(other_uuid)) => {
                this_uuid == other_uuid
            }
        }
    }
}

impl<K, V> Serialize for NodeRef<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            NodeRef::Loaded(node) => match node {
                Node::Internal(node) => node.uuid.serialize(serializer),
                Node::Leaf(node) => node.uuid.serialize(serializer),
            },
            NodeRef::Unloaded(uuid) => uuid.serialize(serializer),
        }
    }
}

impl<'de, K, V> Deserialize<'de> for NodeRef<K, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(NodeRef::Unloaded(Uuid::deserialize(deserializer)?))
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) enum Node<K, V> {
    Internal(Internal<K, V>),
    Leaf(Leaf<K, V>),
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
}
