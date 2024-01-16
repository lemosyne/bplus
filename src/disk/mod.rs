mod error;
mod node;

use uuid::Uuid;

use self::{
    error::Error,
    node::{Internal, Leaf, Link, Node, NodeRef},
};
use std::{
    borrow::Borrow,
    mem,
    path::{Path, PathBuf},
    ptr::NonNull,
};

const DEFAULT_ORDER: usize = 3;

pub struct BPTree<K, V> {
    path: PathBuf,
    root: Option<Link<K, V>>,
    order: usize,
    len: usize,
}

impl<K, V> BPTree<K, V> {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::with_order(path, DEFAULT_ORDER)
    }

    pub fn with_order(path: impl AsRef<Path>, order: usize) -> Self {
        Self {
            path: path.as_ref().into(),
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

    pub fn get_key_value<Q>(&self, key: &Q) -> Result<Option<(&K, &V)>, Error>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe {
            if self.root.is_none() {
                return Ok(None);
            }

            let mut cursor = self.root.unwrap();

            while let Node::Internal(node) = &(*cursor.as_ptr()).access(&self.path)? {
                let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = (*cursor.as_ptr()).access(&self.path)? {
                Ok(node
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .map(|index| (&node.keys[index], &node.values[index]))
                    .ok())
            } else {
                Ok(None)
            }
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Result<Option<&V>, Error>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        Ok(self.get_key_value(key)?.map(|(_, value)| value))
    }

    pub fn get_mut<Q>(&self, key: &Q) -> Result<Option<&mut V>, Error>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe {
            if self.root.is_none() {
                return Ok(None);
            }

            let mut cursor = self.root.unwrap();

            while let Node::Internal(node) = &(*cursor.as_ptr()).access(&self.path)? {
                let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = (*cursor.as_ptr()).access(&self.path)? {
                Ok(node
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .map(|index| &mut node.values[index])
                    .ok())
            } else {
                Ok(None)
            }
        }
    }

    pub fn contains_key<Q>(&self, key: &Q) -> Result<bool, Error>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        Ok(self.get(key)?.is_some())
    }

    pub fn insert(&mut self, key: K, mut value: V) -> Result<Option<V>, Error>
    where
        K: Ord + Clone,
    {
        unsafe {
            if self.root.is_none() {
                let new_root = NonNull::new_unchecked(Box::into_raw(Box::new(NodeRef::Loaded(
                    Node::Leaf(Leaf {
                        uuid: Uuid::new_v4(),
                        keys: vec![key],
                        values: vec![value],
                        parent: None,
                        next_leaf: None,
                    }),
                ))));

                self.root = Some(new_root);
                self.len += 1;
                return Ok(None);
            }

            let mut cursor = self.root.unwrap();

            // Descend the tree to the leaf node that the key should go in.
            while let Node::Internal(node) = (*cursor.as_ptr()).access(&self.path)? {
                let index = match node.keys.binary_search(&key) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = (*cursor.as_ptr()).access(&self.path)? {
                // Check if we already have a copy of this key and just need to
                // swap in the updated value.
                match node.keys.binary_search(&key) {
                    Ok(index) => {
                        // The key exists.
                        mem::swap(&mut node.values[index], &mut value);
                        return Ok(Some(value));
                    }
                    Err(index) => {
                        // The key doesn't exist, so insert it.
                        node.keys.insert(index, key);
                        node.values.insert(index, value);
                        self.len += 1;

                        // We're done if the node isn't overfull.
                        if !node.is_overfull(self.order) {
                            return Ok(None);
                        }

                        // The leaf node is overfull, so we split it in two.
                        let split_index = node.keys.len() / 2;
                        let sibling_keys = node.keys.drain(split_index..).collect::<Vec<_>>();
                        let sibling_values = node.values.drain(split_index..).collect::<Vec<_>>();
                        let split_key = sibling_keys[0].clone();

                        // Make the sibling now so we can link to it.
                        let sibling = NonNull::new_unchecked(Box::into_raw(Box::new(
                            NodeRef::Loaded(Node::Leaf(Leaf {
                                uuid: Uuid::new_v4(),
                                keys: sibling_keys,
                                values: sibling_values,
                                parent: node.parent,
                                next_leaf: node.next_leaf,
                            })),
                        )));

                        // Connect to the sibling.
                        node.next_leaf = Some(sibling);

                        if Some(cursor) == self.root {
                            // We need a new root since we split it.
                            let new_root = NonNull::new_unchecked(Box::into_raw(Box::new(
                                NodeRef::Loaded(Node::Internal(Internal {
                                    uuid: Uuid::new_v4(),
                                    keys: vec![split_key],
                                    children: vec![cursor, sibling],
                                    parent: None,
                                })),
                            )));

                            // Connect the cursor to the new root.
                            if let Node::Leaf(node) = (*cursor.as_ptr()).access(&self.path)? {
                                node.parent = Some(new_root);
                            }

                            // Connect the sibling to the new root.
                            if let Node::Leaf(sibling) = (*cursor.as_ptr()).access(&self.path)? {
                                sibling.parent = Some(new_root);
                            }

                            // Use the new root.
                            self.root = Some(new_root);
                        } else {
                            // Insert to the parent.
                            self.insert_internal(split_key, node.parent.unwrap(), sibling)?;
                        }
                    }
                }
            }

            Ok(None)
        }
    }

    pub fn insert_internal(
        &mut self,
        key: K,
        cursor: Link<K, V>,
        child: Link<K, V>,
    ) -> Result<(), Error>
    where
        K: Ord + Clone,
    {
        unsafe {
            if let Node::Internal(node) = (*cursor.as_ptr()).access(&self.path)? {
                // Find where the key should go.
                let index = match node.keys.binary_search(&key) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };

                // Insert the key and child.
                node.keys.insert(index, key);
                node.children.insert(index + 1, child);

                // We're done if the node isn't overfull.
                if !node.is_overfull(self.order) {
                    return Ok(());
                }

                // Split the overfull node in two.
                let split_index = node.keys.len() / 2;
                let sibling_keys = node.keys.drain(split_index + 1..).collect::<Vec<_>>();
                let sibling_children = node.children.drain(split_index + 1..).collect::<Vec<_>>();
                let split_key = node.keys.pop().unwrap();

                // Make the sibling now so we can link to it.
                let sibling = NonNull::new_unchecked(Box::into_raw(Box::new(NodeRef::Loaded(
                    Node::Internal(Internal {
                        uuid: Uuid::new_v4(),
                        keys: sibling_keys,
                        children: sibling_children,
                        parent: node.parent,
                    }),
                ))));

                // Fix up the parent for the sibling children.
                if let Node::Internal(sibling_node) = (*sibling.as_ptr()).access(&self.path)? {
                    for child in sibling_node.children.iter_mut() {
                        match (*child.as_ptr()).access(&self.path)? {
                            Node::Internal(child) => {
                                child.parent = Some(sibling);
                            }
                            Node::Leaf(child) => {
                                child.parent = Some(sibling);
                            }
                        }
                    }
                }

                if Some(cursor) == self.root {
                    // The root split, so create a new root.
                    let new_root = NonNull::new_unchecked(Box::into_raw(Box::new(
                        NodeRef::Loaded(Node::Internal(Internal {
                            uuid: Uuid::new_v4(),
                            keys: vec![split_key],
                            children: vec![cursor, sibling],
                            parent: None,
                        })),
                    )));

                    if let Node::Internal(sibling) = (*sibling.as_ptr()).access(&self.path)? {
                        sibling.parent = Some(new_root);
                    }

                    node.parent = Some(new_root);
                    self.root = Some(new_root);
                } else {
                    // Recursively insert the split key into the parent.
                    self.insert_internal(split_key, node.parent.unwrap(), sibling)?;
                }
            }

            Ok(())
        }
    }
}
