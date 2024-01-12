use std::{
    fmt::{self, Debug},
    ptr::NonNull,
};

pub(crate) type Link<K, V> = NonNull<Node<K, V>>;
pub(crate) enum Node<K, V> {
    Internal(Internal<K, V>),
    Leaf(Leaf<K, V>),
}

pub(crate) struct Internal<K, V> {
    pub(crate) keys: Vec<K>,
    pub(crate) children: Vec<Link<K, V>>,
    pub(crate) parent: Option<Link<K, V>>,
}

impl<K, V> Internal<K, V> {
    pub fn is_underfull(&self, order: usize) -> bool {
        self.keys.len() < order / 2
    }

    pub fn is_overfull(&self, order: usize) -> bool {
        self.keys.len() > order
    }

    pub fn has_extra_keys(&self, order: usize) -> bool {
        self.keys.len() > order / 2
    }
}

pub(crate) struct Leaf<K, V> {
    pub(crate) keys: Vec<K>,
    pub(crate) values: Vec<V>,
    pub(crate) parent: Option<Link<K, V>>,
    pub(crate) next_leaf: Option<Link<K, V>>,
}

impl<K, V> Leaf<K, V> {
    pub fn is_underfull(&self, order: usize) -> bool {
        self.keys.len() < order.div_ceil(2)
    }

    pub fn is_overfull(&self, order: usize) -> bool {
        self.keys.len() > order
    }

    pub fn has_extra_keys(&self, order: usize) -> bool {
        self.keys.len() > order.div_ceil(2)
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
                Node::Internal(node) => {
                    writeln!(f, "{:?}", node.keys)?;

                    unsafe {
                        for (i, child) in node.children.iter().enumerate() {
                            recursive_fmt(
                                &(*child.as_ptr()),
                                f,
                                depth + 1,
                                i + 1 == node.children.len() && last,
                            )?;
                        }
                    }

                    Ok(())
                }
                Node::Leaf(node) => {
                    if last {
                        write!(f, "{:?}", node.keys)
                    } else {
                        writeln!(f, "{:?}", node.keys)
                    }
                }
            }
        }

        recursive_fmt(self, f, 0, true)
    }
}
