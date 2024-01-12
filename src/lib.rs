mod iter;
mod node;

use iter::{Iter, IterMut, Keys, Values, ValuesMut};
use node::{Internal, Leaf, Link, Node};
use std::{
    borrow::Borrow,
    fmt::{self, Debug},
    marker::PhantomData,
    mem,
    ptr::NonNull,
};

const DEFAULT_ORDER: usize = 3;

pub struct BPTreeMap<K, V> {
    root: Option<Link<K, V>>,
    order: usize,
    len: usize,
}

impl<K, V> BPTreeMap<K, V> {
    pub fn new() -> Self {
        Self::with_order(DEFAULT_ORDER)
    }

    pub fn with_order(order: usize) -> Self {
        Self {
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

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe {
            let mut cursor = self.root?;

            while let Node::Internal(node) = &(*cursor.as_ptr()) {
                let index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[index];
            }

            if let Node::Leaf(node) = &(*cursor.as_ptr()) {
                node.keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .map(|index| &node.values[index])
                    .ok()
            } else {
                None
            }
        }
    }

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
                    prev_leaf: None,
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

                        if node.is_overfull(self.order) {
                            // The leaf node is full, so we split it in two.
                            let split_index = node.keys.len() / 2;
                            let sibling_keys = node.keys.drain(split_index..).collect::<Vec<_>>();
                            let sibling_values =
                                node.values.drain(split_index..).collect::<Vec<_>>();
                            let split_key = sibling_keys[0].clone();

                            // Make the sibling now so we can link to it.
                            let sibling =
                                NonNull::new_unchecked(Box::into_raw(Box::new(Node::Leaf(Leaf {
                                    keys: sibling_keys,
                                    values: sibling_values,
                                    parent: node.parent,
                                    next_leaf: node.next_leaf,
                                    prev_leaf: Some(cursor),
                                }))));

                            // Fix sibling links.
                            if let Some(next_leaf) = node.next_leaf {
                                if let Node::Leaf(next_leaf) = &mut (*next_leaf.as_ptr()) {
                                    next_leaf.prev_leaf = Some(sibling);
                                }
                            }
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

                        self.len += 1;
                        return None;
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

    pub fn remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        unsafe {
            let mut cursor = self.root?;
            let mut cursor_index = 0;

            while let Node::Internal(node) = &(*cursor.as_ptr()) {
                cursor_index = match node.keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                cursor = node.children[cursor_index];
            }

            if let Node::Leaf(node) = &mut (*cursor.as_ptr()) {
                let index = node
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .ok()?;

                let key = node.keys.remove(index);
                let value = node.values.remove(index);
                self.len -= 1;

                if !node.is_underfull(self.order) || Some(cursor) == self.root {
                    return Some((key, value));
                }

                // We might have an underfull non-root leaf node.
                if let Node::Internal(parent) = &mut (*node.parent.unwrap().as_ptr()) {
                    // Check if the left sibling has any extra keys.
                    if cursor_index > 0 {
                        if let Node::Leaf(left_sibling) =
                            &mut (*parent.children[cursor_index - 1].as_ptr())
                        {
                            if left_sibling.has_extra_keys(self.order) {
                                // We want the max key/value pair from the left
                                // sibling.
                                let max_key = left_sibling.keys.pop().unwrap();
                                let max_value = left_sibling.values.pop().unwrap();

                                // The max key/value pair from the left sibling
                                // is smaller than any key/value in the cursor
                                // node.
                                node.keys.insert(0, max_key);
                                node.values.insert(0, max_value);

                                // Update parent key.
                                parent.keys[cursor_index - 1] = node.keys[0].clone();

                                return Some((key, value));
                            }
                        }
                    }

                    // Check if the right sibling has any extra keys.
                    if cursor_index + 1 < parent.children.len() {
                        if let Node::Leaf(right_sibling) =
                            &mut (*parent.children[cursor_index + 1].as_ptr())
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

                                return Some((key, value));
                            }
                        }
                    }

                    // Check if we can merge into the left sibling.
                    if cursor_index > 0 {
                        if let Node::Leaf(left_sibling) =
                            &mut (*parent.children[cursor_index - 1].as_ptr())
                        {
                            // Take/merge in the keys and values.
                            left_sibling.keys.append(&mut node.keys);
                            left_sibling.values.append(&mut node.values);

                            // Relink the left sibling.
                            left_sibling.next_leaf = node.next_leaf.and_then(|node| {
                                if let Node::Leaf(next_leaf) = &mut (*node.as_ptr()) {
                                    next_leaf.prev_leaf = Some(parent.children[cursor_index - 1]);
                                    Some(node)
                                } else {
                                    None
                                }
                            });

                            // Remove the split key.
                            self.remove_entry_internal(
                                parent.keys[cursor_index - 1].borrow(),
                                node.parent.unwrap(),
                                cursor,
                            );

                            return Some((key, value));
                        }
                    }

                    // Check if we can merge the right sibling.
                    if cursor_index + 1 < parent.children.len() {
                        if let Node::Leaf(right_sibling) =
                            &mut (*parent.children[cursor_index + 1].as_ptr())
                        {
                            // Take/merge in the keys and values.
                            node.keys.append(&mut right_sibling.keys);
                            node.values.append(&mut right_sibling.values);

                            // Relink the right sibling.
                            node.next_leaf = right_sibling.next_leaf.and_then(|node| {
                                if let Node::Leaf(next_leaf) = &mut (*node.as_ptr()) {
                                    next_leaf.prev_leaf = Some(cursor);
                                    Some(node)
                                } else {
                                    None
                                }
                            });

                            // Remove the split key from the parent.
                            // The clone is to satisfy miri's stacked borrow check.
                            self.remove_entry_internal(
                                parent.keys[cursor_index].clone().borrow(),
                                node.parent.unwrap(),
                                parent.children[cursor_index + 1],
                            );

                            return Some((key, value));
                        }
                    }
                }
            }

            None
        }
    }

    fn remove_entry_internal<Q>(&mut self, key: &Q, cursor: Link<K, V>, child: Link<K, V>)
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        unsafe {
            if Some(cursor) == self.root {
                if let Node::Internal(node) = &mut (*cursor.as_ptr()) {
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

                        return;
                    }
                }
            }

            if let Node::Internal(node) = &mut (*cursor.as_ptr()) {
                let index = node
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .unwrap();
                node.keys.remove(index);

                let child_index = node.children.binary_search(&child).unwrap();
                let _ = Box::from_raw(node.children.remove(child_index).as_ptr());

                if !node.is_underfull(self.order) || Some(cursor) == self.root {
                    return;
                }

                if let Node::Internal(parent) = &mut (*node.parent.unwrap().as_ptr()) {
                    let cursor_index = parent.children.binary_search(&cursor).unwrap();

                    // Check if there's a left sibling with extra keys.
                    if cursor_index > 0 {
                        if let Node::Internal(left_sibling) =
                            &mut (*parent.children[cursor_index - 1].as_ptr())
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
                                match &mut (*node.children[0].as_ptr()) {
                                    Node::Internal(max_child) => {
                                        max_child.parent = Some(parent.children[cursor_index - 1]);
                                    }
                                    Node::Leaf(max_child) => {
                                        max_child.parent = Some(parent.children[cursor_index - 1]);
                                    }
                                }

                                return;
                            }
                        }
                    }

                    // Check if there's a right sibling with extra keys.
                    if cursor_index + 1 < parent.children.len() {
                        if let Node::Internal(right_sibling) =
                            &mut (*parent.children[cursor_index + 1].as_ptr())
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
                                match &mut (*node.children[node.children.len() - 1].as_ptr()) {
                                    Node::Internal(min_child) => {
                                        min_child.parent = Some(parent.children[cursor_index + 1]);
                                    }
                                    Node::Leaf(min_child) => {
                                        min_child.parent = Some(parent.children[cursor_index + 1]);
                                    }
                                }

                                return;
                            }
                        }
                    }

                    // Check if there's a left sibling to merge with.
                    if cursor_index > 0 {
                        if let Node::Internal(left_sibling) =
                            &mut (*parent.children[cursor_index - 1].as_ptr())
                        {
                            // Left sibling keys, split key, then cursor keys.
                            left_sibling
                                .keys
                                .push(parent.keys[cursor_index - 1].clone());
                            left_sibling.keys.append(&mut node.keys);

                            // Update the parent for the to-be-merged children.
                            for child in node.children.iter_mut() {
                                match &mut (*child.as_ptr()) {
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
                            // The clone is to satisfy miri's stacked borrow check.
                            self.remove_entry_internal(
                                parent.keys[cursor_index - 1].clone().borrow(),
                                node.parent.unwrap(),
                                cursor,
                            );

                            return;
                        }
                    }

                    // Check if there's a right sibling to merge with.
                    if cursor_index + 1 < parent.children.len() {
                        if let Node::Internal(right_sibling) =
                            &mut (*parent.children[cursor_index + 1].as_ptr())
                        {
                            // Cursor keys, split key, then right sibling keys.
                            node.keys.push(parent.keys[cursor_index].clone());
                            node.keys.append(&mut right_sibling.keys);

                            // Update the parent for the to-be-merged children.
                            for child in right_sibling.children.iter_mut() {
                                match &mut (*child.as_ptr()) {
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
                            // The clone is to satisfy miri's stacked borrow check.
                            self.remove_entry_internal(
                                parent.keys[cursor_index].clone().borrow(),
                                node.parent.unwrap(),
                                parent.children[cursor_index + 1],
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        self.remove_entry(key).map(|(_, value)| value)
    }

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

impl<K, V> Drop for BPTreeMap<K, V> {
    fn drop(&mut self) {
        fn recursive_drop<K, V>(node: Link<K, V>) {
            unsafe {
                let boxed_node = Box::from_raw(node.as_ptr());
                match *boxed_node {
                    Node::Internal(node) => {
                        for child in node.children {
                            recursive_drop(child)
                        }
                    }
                    Node::Leaf(_) => {}
                }
            }
        }

        if let Some(root) = self.root {
            recursive_drop(root);
        }
    }
}

impl<K, V> Default for BPTreeMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Debug for BPTreeMap<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(root) = self.root {
            unsafe { write!(f, "{:?}", &(*root.as_ptr())) }
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut tree = BPTreeMap::new();

        for n in [25, 4, 1, 16, 9, 20, 13, 15, 10, 11, 12] {
            println!("Insert {n}:");
            tree.insert(n, ());
            println!("{:?}", tree);
        }

        for n in [13, 15, 1] {
            println!("Delete {n}:");
            tree.remove_entry(&n);
            println!("{:?}", tree);
        }

        for n in tree.iter() {
            println!("{n:?}");
        }
    }
}
