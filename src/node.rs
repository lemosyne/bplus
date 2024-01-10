use std::{
    fmt::{self, Debug},
    ptr::NonNull,
};

pub(crate) type Link<K, V> = NonNull<Node<K, V>>;
pub(crate) enum Node<K, V> {
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
