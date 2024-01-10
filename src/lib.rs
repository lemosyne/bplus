use std::{
    borrow::Borrow,
    fmt::{self, Debug},
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
        K: Borrow<Q> + Ord,
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

    pub fn remove<Q>(&mut self, _key: Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unimplemented!()
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

enum Node<K, V> {
    Internal {
        keys: Vec<K>,
        children: Vec<Link<K, V>>,
        parent: Option<Link<K, V>>,
    },
    Leaf {
        keys: Vec<K>,
        values: Vec<V>,
        parent: Option<Link<K, V>>,
        next_leaf: Option<Link<K, V>>,
        prev_leaf: Option<Link<K, V>>,
    },
}

impl<K, V> Drop for Node<K, V> {
    fn drop(&mut self) {
        match self {
            Node::Internal {
                keys: _,
                children,
                parent: _,
            } => {
                for child in children {
                    unsafe {
                        let _ = Box::from_raw(child.as_ptr());
                    }
                }
            }
            _ => {}
        }
    }
}

impl<K, V> Debug for Node<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn recursive_fmt<K: Debug, V>(
            node: &Node<K, V>,
            f: &mut fmt::Formatter<'_>,
            depth: usize,
            last: bool,
        ) -> fmt::Result {
            write!(f, "{}", "    ".repeat(depth))?;

            match node {
                Node::Internal {
                    keys,
                    children,
                    parent: _,
                } => {
                    writeln!(f, "{:?}", keys)?;

                    unsafe {
                        for (i, child) in children.iter().enumerate() {
                            recursive_fmt(
                                &(*child.as_ptr()),
                                f,
                                depth + 1,
                                i + 1 == children.len() && last,
                            )?;
                        }
                    }

                    Ok(())
                }
                Node::Leaf {
                    keys,
                    values: _,
                    parent: _,
                    next_leaf: _,
                    prev_leaf: _,
                } => {
                    if last {
                        write!(f, "{:?}", keys)
                    } else {
                        writeln!(f, "{:?}", keys)
                    }
                }
            }
        }

        recursive_fmt(self, f, 0, true)
    }
}

type Link<K, V> = NonNull<Node<K, V>>;

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
    }
}
