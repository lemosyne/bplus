use super::{
    node::{Link, Node},
    BPTreeMap,
};
use std::{borrow::Borrow, mem};

impl<K, V> BPTreeMap<K, V> {
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

                // Check if the node is now underfull or if its the root. The
                // root is exceptional in that it is allowed to be underfull.
                if !node.is_underfull(self.order) || Some(cursor) == self.root {
                    // Clean out the root if we've emptied it.
                    if Some(cursor) == self.root && node.keys.is_empty() {
                        let _ = Box::from_raw(cursor.as_ptr());
                        self.root = None;
                    }
                    return Some((key, value));
                }

                // We have an underfull non-root leaf node.
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

                                // The min key/value pair from the right sibling
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
                            left_sibling.next_leaf = node.next_leaf;

                            // Remove the split key.
                            self.remove_entry_internal(
                                parent.keys[cursor_index - 1].clone().borrow(),
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
                            node.next_leaf = right_sibling.next_leaf;

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

                let child_index = node
                    .children
                    .iter()
                    .position(|probe| *probe == child)
                    .unwrap();
                let _ = Box::from_raw(node.children.remove(child_index).as_ptr());

                if !node.is_underfull(self.order) || Some(cursor) == self.root {
                    return;
                }

                if let Node::Internal(parent) = &mut (*node.parent.unwrap().as_ptr()) {
                    let cursor_index = parent
                        .children
                        .iter()
                        .position(|probe| *probe == cursor)
                        .unwrap();

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
                                        max_child.parent = Some(cursor);
                                    }
                                    Node::Leaf(max_child) => {
                                        max_child.parent = Some(cursor);
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
                                        min_child.parent = Some(cursor);
                                    }
                                    Node::Leaf(min_child) => {
                                        min_child.parent = Some(cursor);
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
}
