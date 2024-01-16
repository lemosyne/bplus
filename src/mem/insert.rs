use super::{
    node::{Internal, Leaf, Link, Node},
    BPTreeMap,
};
use std::{mem, ptr::NonNull};

impl<K, V> BPTreeMap<K, V> {
    pub fn insert(&mut self, key: K, mut value: V) -> Option<V>
    where
        K: Ord + Clone,
    {
        unsafe {
            if self.root.is_none() {
                let new_root = NonNull::new_unchecked(Box::into_raw(Box::new(Node::Leaf(Leaf {
                    keys: vec![key],
                    values: vec![value],
                    parent: None,
                    next_leaf: None,
                }))));

                self.root = Some(new_root);
                self.len += 1;
                return None;
            }

            let mut cursor = self.root?;

            // Descend the tree to the leaf node that the key should go in.
            while let Node::Internal(node) = &(*cursor.as_ptr()) {
                let index = match node.keys.binary_search(&key) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = &mut (*cursor.as_ptr()) {
                // Check if we already have a copy of this key and just need
                // to swap in the updated value.
                match node.keys.binary_search(&key) {
                    Ok(index) => {
                        // The key exists.
                        mem::swap(&mut node.values[index], &mut value);
                        return Some(value);
                    }
                    Err(index) => {
                        // The key doesn't exist, so insert it.
                        node.keys.insert(index, key);
                        node.values.insert(index, value);
                        self.len += 1;

                        // We're done if the node isn't overfull.
                        if !node.is_overfull(self.order) {
                            return None;
                        }

                        // The leaf node is overfull, so we split it in two.
                        let split_index = node.keys.len() / 2;
                        let sibling_keys = node.keys.drain(split_index..).collect::<Vec<_>>();
                        let sibling_values = node.values.drain(split_index..).collect::<Vec<_>>();
                        let split_key = sibling_keys[0].clone();

                        // Make the sibling now so we can link to it.
                        let sibling =
                            NonNull::new_unchecked(Box::into_raw(Box::new(Node::Leaf(Leaf {
                                keys: sibling_keys,
                                values: sibling_values,
                                parent: node.parent,
                                next_leaf: node.next_leaf,
                            }))));

                        // Connect to the sibling.
                        node.next_leaf = Some(sibling);

                        if Some(cursor) == self.root {
                            // We need a new root since we split it.
                            let new_root = NonNull::new_unchecked(Box::into_raw(Box::new(
                                Node::Internal(Internal {
                                    keys: vec![split_key],
                                    children: vec![cursor, sibling],
                                    parent: None,
                                }),
                            )));

                            // Connect the cursor to the new root.
                            if let Node::Leaf(node) = &mut (*cursor.as_ptr()) {
                                node.parent = Some(new_root);
                            }

                            // Connect the sibling to the new root.
                            if let Node::Leaf(sibling) = &mut (*sibling.as_ptr()) {
                                sibling.parent = Some(new_root);
                            }

                            // Use the new root.
                            self.root = Some(new_root);
                        } else {
                            // Insert to the parent.
                            self.insert_internal(split_key, node.parent.unwrap(), sibling)
                        }
                    }
                }
            }
        }

        None
    }

    // This is called when `insert()` results in a split node, or if
    // `insert_internal()` results in a split node.
    fn insert_internal(&mut self, key: K, cursor: Link<K, V>, child: Link<K, V>)
    where
        K: Ord + Clone,
    {
        unsafe {
            if let Node::Internal(node) = &mut (*cursor.as_ptr()) {
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
                    return;
                }

                // Split the overfull node in two.
                let split_index = node.keys.len() / 2;
                let sibling_keys = node.keys.drain(split_index + 1..).collect::<Vec<_>>();
                let sibling_children = node.children.drain(split_index + 1..).collect::<Vec<_>>();
                let split_key = node.keys.pop().unwrap();

                // Make the sibling now so we can link to it.
                let sibling =
                    NonNull::new_unchecked(Box::into_raw(Box::new(Node::Internal(Internal {
                        keys: sibling_keys,
                        children: sibling_children,
                        parent: node.parent,
                    }))));

                // Fix up the parent for the sibling children.
                if let Node::Internal(sibling_node) = &mut (*sibling.as_ptr()) {
                    for child in sibling_node.children.iter_mut() {
                        match &mut (*child.as_ptr()) {
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
                    let new_root =
                        NonNull::new_unchecked(Box::into_raw(Box::new(Node::Internal(Internal {
                            keys: vec![split_key],
                            children: vec![cursor, sibling],
                            parent: None,
                        }))));

                    if let Node::Internal(sibling) = &mut (*sibling.as_ptr()) {
                        sibling.parent = Some(new_root);
                    }

                    node.parent = Some(new_root);
                    self.root = Some(new_root);
                } else {
                    // Recursively insert the split key into the parent.
                    self.insert_internal(split_key, node.parent.unwrap(), sibling);
                }
            }
        }
    }
}
