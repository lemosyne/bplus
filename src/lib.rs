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
        self.root
            .map(|root| {
                let mut cursor = root;
                unsafe {
                    loop {
                        match &(*cursor.as_ptr()) {
                            Node::Internal {
                                keys,
                                children,
                                parent: _,
                            } => {
                                let index =
                                    match keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                                        Ok(index) => index,
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
            .flatten()
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
                                Ok(index) => index,
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
                    Ok(index) => index,
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
        K: Borrow<Q>,
        Q: Ord,
    {
        self.root
            .map(|root| {
                let mut cursor = root;
                unsafe {
                    loop {
                        match &mut (*cursor.as_ptr()) {
                            Node::Internal {
                                keys,
                                children,
                                parent: _,
                            } => {
                                let index =
                                    match keys.binary_search_by(|probe| probe.borrow().cmp(key)) {
                                        Ok(index) => index,
                                        Err(index) => index,
                                    };
                                cursor = children[index];
                            }
                            Node::Leaf {
                                keys,
                                values,
                                parent,
                                next_leaf,
                                prev_leaf,
                            } => {
                                let index = keys
                                    .binary_search_by(|probe| probe.borrow().cmp(key))
                                    .ok()?;

                                let key = keys.remove(index);
                                let value = values.remove(index);
                                self.len -= 1;

                                // We might have an underfull non-root leaf node.
                                if keys.len() < self.order / 2 && Some(cursor) != self.root {
                                    // Check if the left sibling has any extra keys.
                                    if let Some(left_sibling) = prev_leaf {
                                        if let Node::Leaf {
                                            keys: left_sibling_keys,
                                            values: left_sibling_values,
                                            parent: _,
                                            next_leaf: _,
                                            prev_leaf: _,
                                        } = &mut (*left_sibling.as_ptr())
                                        {
                                            if left_sibling_keys.len() > self.order / 2 {
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
                                                todo!();

                                                break Some((key, value));
                                            }
                                        }
                                    }

                                    // Check if the right sibling has any extra keys.
                                    if let Some(right_sibling) = next_leaf {
                                        if let Node::Leaf {
                                            keys: right_sibling_keys,
                                            values: right_sibling_values,
                                            parent: _,
                                            next_leaf: _,
                                            prev_leaf: _,
                                        } = &mut (*right_sibling.as_ptr())
                                        {
                                            if right_sibling_keys.len() > self.order / 2 {
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
                                                todo!();

                                                break Some((key, value));
                                            }
                                        }
                                    }

                                    // Check if we can merge with the left
                                    // sibling.
                                    if let Some(left_sibling) = prev_leaf {
                                        if let Node::Leaf {
                                            keys: left_sibling_keys,
                                            values: left_sibling_values,
                                            parent: _,
                                            next_leaf: left_sibling_next_leaf,
                                            prev_leaf: _,
                                        } = &mut (*left_sibling.as_ptr())
                                        {
                                            // Take/merge in the keys and values.
                                            left_sibling_keys.extend(keys.drain(..));
                                            left_sibling_values.extend(values.drain(..));

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
                                                        *prev_leaf = Some(*left_sibling);
                                                        Some(node)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .flatten();

                                            // Remove the split key.
                                            todo!();

                                            // Re-`Box` the cursor node to drop it.
                                            let _ = Box::from_raw(cursor.as_ptr());
                                        }
                                    }

                                    // Check if we can merge with the right // sibling.
                                    if let Some(right_sibling) = next_leaf {
                                        if let Node::Leaf {
                                            keys: right_sibling_keys,
                                            values: right_sibling_values,
                                            parent: _,
                                            next_leaf: _,
                                            prev_leaf: right_sibling_prev_leaf,
                                        } = &mut (*right_sibling.as_ptr())
                                        {
                                            // Take/merge in the keys and // values.
                                            right_sibling_keys.splice(..0, keys.drain(..));
                                            right_sibling_values.splice(..0, values.drain(..));

                                            // Relink the right sibling.
                                            *right_sibling_prev_leaf = prev_leaf
                                                .map(|node| {
                                                    if let Node::Leaf {
                                                        keys: _,
                                                        values: _,
                                                        parent: _,
                                                        next_leaf,
                                                        prev_leaf: _,
                                                    } = &mut (*node.as_ptr())
                                                    {
                                                        *next_leaf = Some(*right_sibling);
                                                        Some(node)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .flatten();

                                            // Remove the split key.
                                            todo!();

                                            // Re-`Box` the cursor node to drop it.
                                            let _ = Box::from_raw(cursor.as_ptr());
                                        }
                                    }
                                }

                                break Some((key, value));
                            }
                        }
                    }
                }
            })
            .flatten()
    }

    fn remove_entry_internal<Q>(&mut self, key: &Q, cursor: Link<K, V>, child: Link<K, V>)
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unimplemented!()
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
        if let Some(root) = self.root {
            unsafe {
                let _ = Box::from_raw(root.as_ptr());
            }
        }
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
            tree.insert(n, ());
            println!("Insert {n}:");
            println!("{:?}", tree);
        }

        for n in tree.iter() {
            println!("{n:?}");
        }
    }
}
