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

    fn insert_internal(
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

    pub fn remove_entry<Q>(&mut self, key: &Q) -> Result<Option<(K, V)>, Error>
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        unsafe {
            if self.root.is_none() {
                return Ok(None);
            }

            let mut cursor = self.root.unwrap();
            let mut cursor_index = 0;

            while let Node::Internal(node) = (*cursor.as_ptr()).access(&self.path)? {
                cursor_index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[cursor_index];
            }

            if let Node::Leaf(node) = (*cursor.as_ptr()).access(&self.path)? {
                let index = node.keys.binary_search_by(|probe| probe.borrow().cmp(key));
                if index.is_err() {
                    return Ok(None);
                }

                let index = index.unwrap();
                let key = node.keys.remove(index);
                let value = node.values.remove(index);
                self.len -= 1;

                // Check if the node is now underfull or if its the root. The
                // root is exceptional in that it is allowed to be underfull.
                if !node.is_underfull(self.order) || Some(cursor) == self.root {
                    // Clean ou thte root if we've emptied it.
                    if Some(cursor) == self.root && node.keys.is_empty() {
                        let _ = Box::from_raw(cursor.as_ptr());
                        self.root = None;
                    }
                    return Ok(Some((key, value)));
                }

                // We have an underfull non-root leaf node.
                if let Node::Internal(parent) =
                    (*node.parent.unwrap().as_ptr()).access(&self.path)?
                {
                    // Check if the left sibling has any extra keys.
                    if cursor_index > 0 {
                        if let Node::Leaf(left_sibling) =
                            (*parent.children[cursor_index - 1].as_ptr()).access(&self.path)?
                        {
                            if left_sibling.has_extra_keys(self.order) {
                                // We want the max key/value pair from the left
                                // sibling.
                                let max_key = left_sibling.keys.pop().unwrap();
                                let max_value = left_sibling.values.pop().unwrap();

                                // The max key/value pair from the left sibling
                                // is smaller thany any key/value in the cursor
                                // node.
                                node.keys.insert(0, max_key);
                                node.values.insert(0, max_value);

                                // Update parent key.
                                parent.keys[cursor_index - 1] = node.keys[0].clone();

                                return Ok(Some((key, value)));
                            }
                        }
                    }

                    // Check if the right sibling has any extra keys.
                    if cursor_index + 1 < parent.children.len() {
                        if let Node::Leaf(right_sibling) =
                            (*parent.children[cursor_index + 1].as_ptr()).access(&self.path)?
                        {
                            if right_sibling.has_extra_keys(self.order) {
                                // We want the min key/value pair from the right
                                // sibling.
                                let min_key = right_sibling.keys.remove(0);
                                let min_value = right_sibling.values.remove(0);

                                // The min key/value pair from the left sibling
                                // is larger than any key/value in the cursor
                                // node.
                                node.keys.push(min_key);
                                node.values.push(min_value);

                                // Update parent key.
                                parent.keys[cursor_index] = right_sibling.keys[0].clone();

                                return Ok(Some((key, value)));
                            }
                        }
                    }

                    // Check if we can merge into the left sibling.
                    if cursor_index > 0 {
                        if let Node::Leaf(left_sibling) =
                            (*parent.children[cursor_index - 1].as_ptr()).access(&self.path)?
                        {
                            // Take/marge in the keys and values.
                            left_sibling.keys.append(&mut node.keys);
                            left_sibling.values.append(&mut node.values);

                            // Relink the left sibling.
                            left_sibling.next_leaf = node.next_leaf;

                            // Remove the split key.
                            self.remove_entry_internal(
                                parent.keys[cursor_index - 1].clone().borrow(),
                                node.parent.unwrap(),
                                cursor,
                            )?;

                            return Ok(Some((key, value)));
                        }
                    }

                    // Check if we can merge the right sibling.
                    if cursor_index + 1 < parent.children.len() {
                        if let Node::Leaf(right_sibling) =
                            (*parent.children[cursor_index + 1].as_ptr()).access(&self.path)?
                        {
                            // Take/merge in the keys and values.
                            node.keys.append(&mut right_sibling.keys);
                            node.values.append(&mut right_sibling.values);

                            // Relink the right sibling.
                            node.next_leaf = right_sibling.next_leaf;

                            // Remove the split key from the parent.
                            // The clone is to satisfy miri's stacked borrow
                            // check.
                            self.remove_entry_internal(
                                parent.keys[cursor_index + 1].clone().borrow(),
                                node.parent.unwrap(),
                                parent.children[cursor_index + 1],
                            )?;

                            return Ok(Some((key, value)));
                        }
                    }
                }
            }

            Ok(None)
        }
    }

    fn remove_entry_internal<Q>(
        &mut self,
        key: &Q,
        cursor: Link<K, V>,
        child: Link<K, V>,
    ) -> Result<(), Error>
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        unsafe {
            if Some(cursor) == self.root {
                if let Node::Internal(node) = (*cursor.as_ptr()).access(&self.path)? {
                    // Check if we're deleting the final key from the root.
                    if node.keys.len() == 1 {
                        // Decide which child is the new root.
                        self.root = if node.children[1] == child {
                            Some(node.children[0])
                        } else {
                            Some(node.children[1])
                        };

                        // Re-`Box` the root and child to drop them.
                        let _ = Box::from_raw(cursor.as_ptr());
                        let _ = Box::from_raw(child.as_ptr());

                        return Ok(());
                    }
                }
            }

            if let Node::Internal(node) = (*cursor.as_ptr()).access(&self.path)? {
                let index = node
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .unwrap();
                node.keys.remove(index);

                let child_index = node
                    .children
                    .iter()
                    .position(|probe| *probe == child)
                    .unwrap();
                let _ = Box::from_raw(node.children.remove(child_index).as_ptr());

                if !node.is_underfull(self.order) || Some(cursor) == self.root {
                    return Ok(());
                }

                if let Node::Internal(parent) =
                    (*node.parent.unwrap().as_ptr()).access(&self.path)?
                {
                    let cursor_index = parent
                        .children
                        .iter()
                        .position(|probe| *probe == cursor)
                        .unwrap();

                    // Check if there's a left sibling with extra keys.
                    if cursor_index > 0 {
                        if let Node::Internal(left_sibling) =
                            (*parent.children[cursor_index - 1].as_ptr()).access(&self.path)?
                        {
                            // Does the left sibling have extra keys?
                            if left_sibling.has_extra_keys(self.order) {
                                // Take the max key and clone it to the parent.
                                let mut max_key = left_sibling.keys.pop().unwrap();
                                mem::swap(&mut parent.keys[cursor_index - 1], &mut max_key);
                                node.keys.insert(0, max_key);

                                // Take the max child.
                                let max_child = left_sibling.children.pop().unwrap();
                                node.children.insert(0, max_child);

                                // Fix max child's parent.
                                match (*node.children[0].as_ptr()).access(&self.path)? {
                                    Node::Internal(max_child) => {
                                        max_child.parent = Some(cursor);
                                    }
                                    Node::Leaf(max_child) => {
                                        max_child.parent = Some(cursor);
                                    }
                                }

                                return Ok(());
                            }
                        }
                    }

                    // Check if there's a right sibling with extra keys.
                    if cursor_index + 1 < parent.children.len() {
                        if let Node::Internal(right_sibling) =
                            (*parent.children[cursor_index + 1].as_ptr()).access(&self.path)?
                        {
                            if right_sibling.has_extra_keys(self.order) {
                                // Take the min key and clone it to the parent.
                                let mut min_key = right_sibling.keys.remove(0);
                                mem::swap(&mut parent.keys[cursor_index], &mut min_key);
                                node.keys.push(min_key);

                                // Take the min child.
                                let min_child = right_sibling.children.remove(0);
                                node.children.push(min_child);

                                // Fix min child's parent.
                                match (*node.children[node.children.len() - 1].as_ptr())
                                    .access(&self.path)?
                                {
                                    Node::Internal(min_child) => {
                                        min_child.parent = Some(cursor);
                                    }
                                    Node::Leaf(min_child) => {
                                        min_child.parent = Some(cursor);
                                    }
                                }

                                return Ok(());
                            }
                        }
                    }

                    // Check if there's a left sibling to merge with.
                    if cursor_index > 0 {
                        if let Node::Internal(left_sibling) =
                            (*parent.children[cursor_index - 1].as_ptr()).access(&self.path)?
                        {
                            // Left sibling keys, split key, then cursor keys.
                            left_sibling
                                .keys
                                .push(parent.keys[cursor_index - 1].clone());
                            left_sibling.keys.append(&mut node.keys);

                            // Update the parent for the to-be-merged children.
                            for child in node.children.iter_mut() {
                                match (*child.as_ptr()).access(&self.path)? {
                                    Node::Internal(child) => {
                                        child.parent = Some(parent.children[cursor_index - 1]);
                                    }
                                    Node::Leaf(child) => {
                                        child.parent = Some(parent.children[cursor_index - 1]);
                                    }
                                }
                            }

                            // Merge the children into the left sibling.
                            left_sibling.children.append(&mut node.children);

                            // Remove the split key from the parent.
                            // The clone is to satisfy miri's stacked borrow
                            // check.
                            self.remove_entry_internal(
                                parent.keys[cursor_index - 1].clone().borrow(),
                                node.parent.unwrap(),
                                cursor,
                            )?;

                            return Ok(());
                        }
                    }

                    // Check if there's a right sibling to merge with.
                    if cursor_index + 1 < parent.children.len() {
                        if let Node::Internal(right_sibling) =
                            (*parent.children[cursor_index + 1].as_ptr()).access(&self.path)?
                        {
                            // Cursor keys, split key, then right sibling keys.
                            node.keys.push(parent.keys[cursor_index].clone());
                            node.keys.append(&mut right_sibling.keys);

                            // Update the parent for the to-be-merged children.
                            for child in right_sibling.children.iter_mut() {
                                match (*child.as_ptr()).access(&self.path)? {
                                    Node::Internal(child) => {
                                        child.parent = Some(cursor);
                                    }
                                    Node::Leaf(child) => {
                                        child.parent = Some(cursor);
                                    }
                                }
                            }

                            // Merge in the right sibling's children.
                            node.children.append(&mut right_sibling.children);

                            // Remove the split key from the parent.
                            // The clone is to satisfy miri's stacked borrow
                            // check.
                            self.remove_entry_internal(
                                parent.keys[cursor_index].clone().borrow(),
                                node.parent.unwrap(),
                                parent.children[cursor_index + 1],
                            )?;
                        }
                    }
                }
            }

            Ok(())
        }
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Result<Option<V>, Error>
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        Ok(self.remove_entry(key)?.map(|(_, value)| value))
    }
}

impl<K, V> Drop for BPTree<K, V> {
    fn drop(&mut self) {
        fn recursive_drop<K, V>(node: Link<K, V>) {
            unsafe {
                let boxed_node = Box::from_raw(node.as_ptr());
                if let NodeRef::Loaded(Node::Internal(node)) = *boxed_node {
                    for child in node.children {
                        recursive_drop(child);
                    }
                }
            }
        }

        if let Some(root) = self.root {
            recursive_drop(root);
        }
    }
}
