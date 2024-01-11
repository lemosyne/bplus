mod iter;
mod node;

use iter::{Iter, IterMut, Keys, Values, ValuesMut};
use node::{Link, Node};
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
        self.root.and_then(|root| {
            let mut cursor = root;
            unsafe {
                loop {
                    match &(*cursor.as_ptr()) {
                        Node::Internal {
                            keys,
                            children,
                            parent: _,
                        } => {
                            let index = match keys.binary_search_by(|probe| probe.borrow().cmp(key))
                            {
                                Ok(index) => index + 1,
                                Err(index) => index,
                            };
                            cursor = children[index];
                        }
                        Node::Leaf {
                            keys,
                            values,
                            parent: _,
                            next_leaf: _,
                            prev_leaf: _,
                        } => {
                            return keys
                                .binary_search_by(|probe| probe.borrow().cmp(key))
                                .map(|index| &values[index])
                                .ok()
                        }
                    }
                }
            }
        })
    }

    pub fn insert(&mut self, key: K, mut value: V) -> Option<V>
    where
        K: Ord + Clone,
    {
        if let Some(root) = self.root {
            let mut cursor = root;
            unsafe {
                // Descend the tree to the leaf node that the key should go in.
                loop {
                    match &mut (*cursor.as_ptr()) {
                        Node::Internal {
                            keys,
                            children,
                            parent: _,
                        } => {
                            let index = match keys.binary_search(&key) {
                                Ok(index) => index + 1,
                                Err(index) => index,
                            };
                            cursor = children[index];
                        }
                        Node::Leaf {
                            keys,
                            values,
                            parent,
                            next_leaf,
                            prev_leaf: _,
                        } => {
                            // Check if we already have a copy of this key and just need
                            // to swap in the updated value.
                            match keys.binary_search(&key) {
                                Ok(index) => {
                                    // The key exists.
                                    mem::swap(&mut values[index], &mut value);
                                    return Some(value);
                                }
                                Err(index) => {
                                    // The key doesn't exist, so insert it.
                                    keys.insert(index, key);
                                    values.insert(index, value);

                                    if keys.len() > self.order {
                                        // The leaf node is full, so we split it in two.
                                        let split_index = keys.len() / 2;
                                        let sibling_keys =
                                            keys.drain(split_index..).collect::<Vec<_>>();
                                        let sibling_values =
                                            values.drain(split_index..).collect::<Vec<_>>();
                                        let split_key = sibling_keys[0].clone();

                                        // Make the sibling now so we can link to it.
                                        let sibling = NonNull::new_unchecked(Box::into_raw(
                                            Box::new(Node::<K, V>::Leaf {
                                                keys: sibling_keys,
                                                values: sibling_values,
                                                parent: *parent,
                                                next_leaf: *next_leaf,
                                                prev_leaf: Some(cursor),
                                            }),
                                        ));

                                        // Fix sibling links.
                                        if let Some(next_leaf) = next_leaf {
                                            if let Node::Leaf {
                                                keys: _,
                                                values: _,
                                                parent: _,
                                                next_leaf: _,
                                                prev_leaf,
                                            } = &mut (*next_leaf.as_ptr())
                                            {
                                                *prev_leaf = Some(sibling);
                                            }
                                        }
                                        *next_leaf = Some(sibling);

                                        if Some(cursor) == self.root {
                                            // We need a new root since we split it.
                                            let new_root = NonNull::new_unchecked(Box::into_raw(
                                                Box::new(Node::<K, V>::Internal {
                                                    keys: vec![split_key],
                                                    children: vec![cursor, sibling],
                                                    parent: None,
                                                }),
                                            ));

                                            // Connect the cursor to the new root.
                                            if let Node::Leaf {
                                                keys: _,
                                                values: _,
                                                parent,
                                                next_leaf: _,
                                                prev_leaf: _,
                                            } = &mut (*cursor.as_ptr())
                                            {
                                                *parent = Some(new_root);
                                            }

                                            // Connect the sibling to the new root.
                                            if let Node::Leaf {
                                                keys: _,
                                                values: _,
                                                parent,
                                                next_leaf: _,
                                                prev_leaf: _,
                                            } = &mut (*sibling.as_ptr())
                                            {
                                                *parent = Some(new_root);
                                            }

                                            // Use the new root.
                                            self.root = Some(new_root);
                                        } else {
                                            // Insert to the parent.
                                            self.insert_internal(
                                                split_key,
                                                parent.unwrap(),
                                                sibling,
                                            )
                                        }
                                    }

                                    self.len += 1;
                                    return None;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // We don't have a root yet, so create a new one that contains the
            // key-value pair.
            unsafe {
                let new_root =
                    NonNull::new_unchecked(Box::into_raw(Box::new(Node::<K, V>::Leaf {
                        keys: vec![key],
                        values: vec![value],
                        parent: None,
                        next_leaf: None,
                        prev_leaf: None,
                    })));

                self.root = Some(new_root);
                self.len += 1;
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
            if let Node::Internal {
                keys,
                children,
                parent,
            } = &mut (*cursor.as_ptr())
            {
                // Find where the key should go.
                let index = match keys.binary_search(&key) {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };

                // Insert the key and child.
                keys.insert(index, key);
                children.insert(index + 1, child);

                if keys.len() > self.order {
                    // The node is overfull now, so we need to split it into two.
                    let split_index = keys.len() / 2;
                    let sibling_keys = keys.drain(split_index + 1..).collect::<Vec<_>>();
                    let sibling_children = children.drain(split_index + 1..).collect::<Vec<_>>();
                    let split_key = keys.pop().unwrap();

                    // Make the sibling now so we can link to it.
                    let sibling =
                        NonNull::new_unchecked(Box::into_raw(Box::new(Node::<K, V>::Internal {
                            keys: sibling_keys,
                            children: sibling_children,
                            parent: *parent,
                        })));

                    // Fix up the parent for the sibling children.
                    if let Node::Internal {
                        keys: _,
                        children: sibling_children,
                        parent: _,
                    } = &mut (*sibling.as_ptr())
                    {
                        for child in sibling_children {
                            match &mut (*child.as_ptr()) {
                                Node::Internal {
                                    keys: _,
                                    children: _,
                                    parent: sibling_child_parent,
                                } => {
                                    *sibling_child_parent = Some(sibling);
                                }
                                Node::Leaf {
                                    keys: _,
                                    values: _,
                                    parent: sibling_child_parent,
                                    next_leaf: _,
                                    prev_leaf: _,
                                } => {
                                    *sibling_child_parent = Some(sibling);
                                }
                            }
                        }
                    }

                    if Some(cursor) == self.root {
                        // The root split, so create a new root.
                        let new_root = NonNull::new_unchecked(Box::into_raw(Box::new(Node::<
                            K,
                            V,
                        >::Internal {
                            keys: vec![split_key],
                            children: vec![cursor, sibling],
                            parent: None,
                        })));

                        if let Node::Internal {
                            keys: _,
                            children: _,
                            parent,
                        } = &mut (*sibling.as_ptr())
                        {
                            *parent = Some(new_root);
                        }

                        *parent = Some(new_root);
                        self.root = Some(new_root);
                    } else {
                        // Recursively insert the split key into the parent.
                        self.insert_internal(split_key, parent.unwrap(), sibling);
                    }
                }
            }
        }
    }

    pub fn remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        self.root.and_then(|root| {
            let mut cursor = root;
            let mut cursor_index = 0;
            unsafe {
                loop {
                    match &mut (*cursor.as_ptr()) {
                        Node::Internal {
                            keys,
                            children,
                            parent: _,
                        } => {
                            cursor_index =
                                match keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                                    Ok(index) => index + 1,
                                    Err(index) => index,
                                };
                            cursor = children[cursor_index];
                        }
                        Node::Leaf {
                            keys,
                            values,
                            parent,
                            next_leaf,
                            prev_leaf: _,
                        } => {
                            let index = keys
                                .binary_search_by(|probe| probe.borrow().cmp(key))
                                .ok()?;

                            let key = keys.remove(index);
                            let value = values.remove(index);
                            self.len -= 1;

                            // We might have an underfull non-root leaf node.
                            if keys.len() < self.order.div_ceil(2) && Some(cursor) != self.root {
                                if let Node::Internal {
                                    keys: parent_keys,
                                    children: parent_children,
                                    parent: _,
                                } = &mut (*parent.unwrap().as_ptr())
                                {
                                    // Check if the left sibling has any extra keys.
                                    if cursor_index > 0 {
                                        if let Node::Leaf {
                                            keys: left_sibling_keys,
                                            values: left_sibling_values,
                                            parent: _,
                                            next_leaf: _,
                                            prev_leaf: _,
                                        } = &mut (*parent_children[cursor_index - 1].as_ptr())
                                        {
                                            if left_sibling_keys.len() > self.order.div_ceil(2) {
                                                // We want the max key/value
                                                // pair from the left sibling.
                                                let max_key = left_sibling_keys.pop().unwrap();
                                                let max_value = left_sibling_values.pop().unwrap();

                                                // The max key/value pair from
                                                // the left sibling is smaller
                                                // than any key/value in the
                                                // cursor node.
                                                keys.insert(0, max_key);
                                                values.insert(0, max_value);

                                                // Update parent key.
                                                if let Node::Internal {
                                                    keys: parent_keys,
                                                    children: _,
                                                    parent: _,
                                                } = &mut (*parent.unwrap().as_ptr())
                                                {
                                                    parent_keys[cursor_index - 1] = keys[0].clone();
                                                }

                                                break Some((key, value));
                                            }
                                        }
                                    }

                                    // Check if the right sibling has any extra keys.
                                    if cursor_index + 1 < parent_children.len() {
                                        if let Node::Leaf {
                                            keys: right_sibling_keys,
                                            values: right_sibling_values,
                                            parent: _,
                                            next_leaf: _,
                                            prev_leaf: _,
                                        } = &mut (*parent_children[cursor_index + 1].as_ptr())
                                        {
                                            if right_sibling_keys.len() > self.order.div_ceil(2) {
                                                // We want the min key/value
                                                // pair from the right sibling.
                                                let min_key = right_sibling_keys.remove(0);
                                                let min_value = right_sibling_values.remove(0);

                                                // The min key/value pair from
                                                // the left sibling is larger
                                                // than any key/value in the
                                                // cursor node.
                                                keys.push(min_key);
                                                values.push(min_value);

                                                // Update parent key.
                                                if let Node::Internal {
                                                    keys: parent_keys,
                                                    children: _,
                                                    parent: _,
                                                } = &mut (*parent.unwrap().as_ptr())
                                                {
                                                    parent_keys[cursor_index] =
                                                        right_sibling_keys[0].clone();
                                                }

                                                break Some((key, value));
                                            }
                                        }
                                    }

                                    // Check if we can merge into the left sibling.
                                    if cursor_index > 0 {
                                        if let Node::Leaf {
                                            keys: left_sibling_keys,
                                            values: left_sibling_values,
                                            parent: _,
                                            next_leaf: left_sibling_next_leaf,
                                            prev_leaf: _,
                                        } = &mut (*parent_children[cursor_index - 1].as_ptr())
                                        {
                                            // Take/merge in the keys and values.
                                            left_sibling_keys.append(keys);
                                            left_sibling_values.append(values);

                                            // Relink the left sibling.
                                            *left_sibling_next_leaf = next_leaf
                                                .map(|node| {
                                                    if let Node::Leaf {
                                                        keys: _,
                                                        values: _,
                                                        parent: _,
                                                        next_leaf: _,
                                                        prev_leaf,
                                                    } = &mut (*node.as_ptr())
                                                    {
                                                        *prev_leaf =
                                                            Some(parent_children[cursor_index - 1]);
                                                        Some(node)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .flatten();

                                            // Remove the split key.
                                            if let Node::Internal {
                                                keys: parent_keys,
                                                children: _,
                                                parent: _,
                                            } = &(*parent.unwrap().as_ptr())
                                            {
                                                self.remove_entry_internal(
                                                    parent_keys[cursor_index - 1].borrow(),
                                                    parent.unwrap(),
                                                    cursor,
                                                )
                                            }
                                        }
                                    }

                                    // Check if we can merge the right sibling.
                                    if cursor_index + 1 < parent_children.len() {
                                        if let Node::Leaf {
                                            keys: right_sibling_keys,
                                            values: right_sibling_values,
                                            parent: _,
                                            next_leaf: right_sibling_next_leaf,
                                            prev_leaf: _,
                                        } = &mut (*parent_children[cursor_index + 1].as_ptr())
                                        {
                                            // Take/merge in the keys and values.
                                            keys.append(right_sibling_keys);
                                            values.append(right_sibling_values);

                                            // Relink the right sibling.
                                            *next_leaf = right_sibling_next_leaf
                                                .map(|node| {
                                                    if let Node::Leaf {
                                                        keys: _,
                                                        values: _,
                                                        parent: _,
                                                        next_leaf: _,
                                                        prev_leaf,
                                                    } = &mut (*node.as_ptr())
                                                    {
                                                        *prev_leaf = Some(cursor);
                                                        Some(node)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .flatten();

                                            // Remove the split key from the parent.
                                            // The clone is to satisfy miri's stacked borrow check.
                                            self.remove_entry_internal(
                                                parent_keys[cursor_index].clone().borrow(),
                                                parent.unwrap(),
                                                parent_children[cursor_index + 1],
                                            );
                                        }
                                    }
                                }
                            }

                            break Some((key, value));
                        }
                    }
                }
            }
        })
    }

    fn remove_entry_internal<Q>(&mut self, key: &Q, cursor: Link<K, V>, child: Link<K, V>)
    where
        K: Borrow<Q> + Clone,
        Q: Ord,
    {
        unsafe {
            if Some(cursor) == self.root {
                if let Node::Internal {
                    keys,
                    children,
                    parent: _,
                } = &mut (*cursor.as_ptr())
                {
                    // Check if we're deleting the final key from the root.
                    if keys.len() == 1 {
                        // Decide which child is the new root.
                        if children[1] == child {
                            self.root = Some(children[0]);
                        } else {
                            self.root = Some(children[1]);
                        }

                        // Re-`Box` the root and child to drop them.
                        let _ = Box::from_raw(cursor.as_ptr());
                        let _ = Box::from_raw(child.as_ptr());

                        return;
                    }
                }
            }

            if let Node::Internal {
                keys,
                children,
                parent,
            } = &mut (*cursor.as_ptr())
            {
                let index = keys
                    .binary_search_by(|probe| probe.borrow().cmp(key))
                    .unwrap();
                keys.remove(index);

                let child_index = children.binary_search(&child).unwrap();
                let _ = Box::from_raw(children.remove(child_index).as_ptr());

                if keys.len() < self.order / 2 && Some(cursor) != self.root {
                    if let Node::Internal {
                        keys: parent_keys,
                        children: parent_children,
                        parent: _,
                    } = &mut (*parent.unwrap().as_ptr())
                    {
                        let cursor_index = parent_children.binary_search(&cursor).unwrap();

                        // Check if there's a left sibling with extra keys.
                        if cursor_index > 0 {
                            if let Node::Internal {
                                keys: left_sibling_keys,
                                children: left_sibling_children,
                                parent: _,
                            } = &mut (*parent_children[cursor_index - 1].as_ptr())
                            {
                                // Does the left sibling have extra keys?
                                if left_sibling_keys.len() > self.order / 2 {
                                    // Take the max key and clone it to the parent.
                                    let mut max_key = left_sibling_keys.pop().unwrap();
                                    mem::swap(&mut parent_keys[cursor_index - 1], &mut max_key);
                                    keys.insert(0, max_key);

                                    // Take the max child.
                                    let max_child = left_sibling_children.pop().unwrap();
                                    children.insert(0, max_child);

                                    // Fix max child's parent.
                                    match &mut (*children[0].as_ptr()) {
                                        Node::Internal {
                                            keys: _,
                                            children: _,
                                            parent: max_child_parent,
                                        } => {
                                            *max_child_parent =
                                                Some(parent_children[cursor_index - 1]);
                                        }
                                        Node::Leaf {
                                            keys: _,
                                            values: _,
                                            parent: max_child_parent,
                                            next_leaf: _,
                                            prev_leaf: _,
                                        } => {
                                            *max_child_parent =
                                                Some(parent_children[cursor_index - 1]);
                                        }
                                    }

                                    return;
                                }
                            }
                        }

                        // Check if there's a right sibling with extra keys.
                        if cursor_index + 1 < parent_children.len() {
                            if let Node::Internal {
                                keys: right_sibling_keys,
                                children: right_sibling_children,
                                parent: _,
                            } = &mut (*parent_children[cursor_index + 1].as_ptr())
                            {
                                if right_sibling_keys.len() > self.order / 2 {
                                    // Take the min key and clone it to the parent.
                                    let mut min_key = right_sibling_keys.remove(0);
                                    mem::swap(&mut parent_keys[cursor_index], &mut min_key);
                                    keys.push(min_key);

                                    // Take the min child.
                                    let min_child = right_sibling_children.remove(0);
                                    children.push(min_child);

                                    // Fix min child's parent.
                                    match &mut (*children[children.len() - 1].as_ptr()) {
                                        Node::Internal {
                                            keys: _,
                                            children: _,
                                            parent: min_child_parent,
                                        } => {
                                            *min_child_parent =
                                                Some(parent_children[cursor_index + 1]);
                                        }
                                        Node::Leaf {
                                            keys: _,
                                            values: _,
                                            parent: min_child_parent,
                                            next_leaf: _,
                                            prev_leaf: _,
                                        } => {
                                            *min_child_parent =
                                                Some(parent_children[cursor_index + 1]);
                                        }
                                    }

                                    return;
                                }
                            }
                        }

                        // Check if there's a left sibling to merge with.
                        if cursor_index > 0 {
                            if let Node::Internal {
                                keys: left_sibling_keys,
                                children: left_sibling_children,
                                parent: _,
                            } = &mut (*parent_children[cursor_index - 1].as_ptr())
                            {
                                // Left sibling keys, split key, then cursor keys.
                                left_sibling_keys.push(parent_keys[cursor_index - 1].clone());
                                left_sibling_keys.append(keys);

                                // Update the parent for the to-be-merged children.
                                for child in children.iter_mut() {
                                    match &mut (*child.as_ptr()) {
                                        Node::Internal {
                                            keys: _,
                                            children: _,
                                            parent: child_parent,
                                        } => {
                                            *child_parent = Some(parent_children[cursor_index - 1]);
                                        }
                                        Node::Leaf {
                                            keys: _,
                                            values: _,
                                            parent: child_parent,
                                            next_leaf: _,
                                            prev_leaf: _,
                                        } => {
                                            *child_parent = Some(parent_children[cursor_index - 1]);
                                        }
                                    }
                                }

                                // Merge the children into the left sibling.
                                left_sibling_children.append(children);

                                // Remove the split key from the parent.
                                // The clone is to satisfy miri's stacked borrow check.
                                self.remove_entry_internal(
                                    parent_keys[cursor_index - 1].clone().borrow(),
                                    parent.unwrap(),
                                    cursor,
                                );

                                return;
                            }
                        }

                        // Check if there's a right sibling to merge with.
                        if cursor_index + 1 < parent_children.len() {
                            if let Node::Internal {
                                keys: right_sibling_keys,
                                children: right_sibling_children,
                                parent: _,
                            } = &mut (*parent_children[cursor_index + 1].as_ptr())
                            {
                                // Cursor keys, split key, then right sibling keys.
                                keys.push(parent_keys[cursor_index].clone());
                                keys.append(right_sibling_keys);

                                // Update the parent for the to-be-merged children.
                                for child in right_sibling_children.iter_mut() {
                                    match &mut (*child.as_ptr()) {
                                        Node::Internal {
                                            keys: _,
                                            children: _,
                                            parent: child_parent,
                                        } => {
                                            *child_parent = Some(cursor);
                                        }
                                        Node::Leaf {
                                            keys: _,
                                            values: _,
                                            parent: child_parent,
                                            next_leaf: _,
                                            prev_leaf: _,
                                        } => {
                                            *child_parent = Some(cursor);
                                        }
                                    }
                                }

                                // Merge in the right sibling's children.
                                children.append(right_sibling_children);

                                // Remove the split key from the parent.
                                // The clone is to satisfy miri's stacked borrow check.
                                self.remove_entry_internal(
                                    parent_keys[cursor_index].clone().borrow(),
                                    parent.unwrap(),
                                    parent_children[cursor_index + 1],
                                );
                            }
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
            let mut cursor = root;
            unsafe {
                loop {
                    match &(*cursor.as_ptr()) {
                        Node::Internal {
                            keys: _,
                            children,
                            parent: _,
                        } => cursor = children[0],
                        Node::Leaf {
                            keys: _,
                            values: _,
                            parent: _,
                            next_leaf: _,
                            prev_leaf: _,
                        } => {
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
            let mut cursor = root;
            unsafe {
                loop {
                    match &(*cursor.as_ptr()) {
                        Node::Internal {
                            keys: _,
                            children,
                            parent: _,
                        } => cursor = children[0],
                        Node::Leaf {
                            keys: _,
                            values: _,
                            parent: _,
                            next_leaf: _,
                            prev_leaf: _,
                        } => {
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
                    Node::Internal {
                        keys: _,
                        ref children,
                        parent: _,
                    } => {
                        for child in children {
                            recursive_drop(*child)
                        }
                    }
                    Node::Leaf {
                        keys: _,
                        values: _,
                        parent: _,
                        next_leaf: _,
                        prev_leaf: _,
                    } => {}
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
