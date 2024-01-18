use super::{
    error::Error,
    node::{Internal, Leaf, Link, Node},
    BPTree,
};
use serde::Deserialize;
use std::mem;
use uuid::Uuid;

impl<K, V> BPTree<K, V> {
    pub fn insert(&mut self, key: K, mut value: V) -> Result<Option<V>, Error>
    where
        for<'de> K: Deserialize<'de> + Ord + Clone,
        for<'de> V: Deserialize<'de>,
    {
        unsafe {
            if self.root.is_none() {
                let new_root = Link::new(Node::Leaf(Leaf {
                    uuid: Uuid::new_v4(),
                    keys: vec![key],
                    values: vec![value],
                    parent: None,
                    next_leaf: None,
                    dirty: true,
                }));

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

            if let Node::Leaf(node) = (*cursor.as_ptr()).access_mut(&self.path)? {
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
                        let sibling = Link::new(Node::Leaf(Leaf {
                            uuid: Uuid::new_v4(),
                            keys: sibling_keys,
                            values: sibling_values,
                            parent: node.parent,
                            next_leaf: node.next_leaf,
                            dirty: true,
                        }));

                        // Connect to the sibling.
                        node.next_leaf = Some(sibling);

                        if Some(cursor) == self.root {
                            // We need a new root since we split it.
                            let new_root = Link::new(Node::Internal(Internal {
                                uuid: Uuid::new_v4(),
                                keys: vec![split_key],
                                children: vec![cursor, sibling],
                                parent: None,
                                dirty: true,
                            }));

                            // Connect the cursor to the new root.
                            if let Node::Leaf(node) = (*cursor.as_ptr()).access_mut(&self.path)? {
                                node.parent = Some(new_root);
                            }

                            // Connect the sibling to the new root.
                            if let Node::Leaf(sibling_node) =
                                (*sibling.as_ptr()).access_mut(&self.path)?
                            {
                                sibling_node.parent = Some(new_root);
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

    fn insert_internal(
        &mut self,
        key: K,
        cursor: Link<K, V>,
        child: Link<K, V>,
    ) -> Result<(), Error>
    where
        for<'de> K: Deserialize<'de> + Ord + Clone,
        for<'de> V: Deserialize<'de>,
    {
        unsafe {
            if let Node::Internal(node) = (*cursor.as_ptr()).access_mut(&self.path)? {
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
                let sibling = Link::new(Node::Internal(Internal {
                    uuid: Uuid::new_v4(),
                    keys: sibling_keys,
                    children: sibling_children,
                    parent: node.parent,
                    dirty: true,
                }));

                // Fix up the parent for the sibling children.
                if let Node::Internal(sibling_node) = (*sibling.as_ptr()).access_mut(&self.path)? {
                    for child in sibling_node.children.iter_mut() {
                        match (*child.as_ptr()).access_mut(&self.path)? {
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
                    let new_root = Link::new(Node::Internal(Internal {
                        uuid: Uuid::new_v4(),
                        keys: vec![split_key],
                        children: vec![cursor, sibling],
                        parent: None,
                        dirty: true,
                    }));

                    if let Node::Internal(sibling) = (*sibling.as_ptr()).access_mut(&self.path)? {
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
