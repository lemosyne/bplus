use std::borrow::Borrow;

use super::{
    error::Error,
    node::{Link, Node},
    BPTree,
};

impl<K, V> BPTree<K, V> {
    pub fn persist(&self) -> Result<(), Error> {
        unimplemented!()
    }

    pub fn persist_key<Q>(&self, key: &Q) -> Result<(), Error>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let walk = Walk {
            key,
            cursor: self.root,
            tree: self,
            errored: false,
        };

        for node in walk {
            node?.persist(&self.path)?;
        }

        Ok(())
    }
}

struct Walk<'a, Q, K, V> {
    key: &'a Q,
    cursor: Option<Link<K, V>>,
    tree: &'a BPTree<K, V>,
    errored: bool,
}

impl<'a, Q, K, V> Iterator for Walk<'a, Q, K, V>
where
    K: Borrow<Q>,
    Q: Ord,
{
    type Item = Result<&'a Node<K, V>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.errored {
            return None;
        }

        let cursor = self.cursor?;
        let node = match cursor.access(&self.tree.path) {
            Ok(node) => node,
            Err(err) => {
                self.errored = true;
                return Some(Err(err));
            }
        };

        match node {
            Node::Internal(internal) => {
                let index = match internal
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(self.key))
                {
                    Ok(index) => index + 1,
                    Err(index) => index,
                };
                self.cursor = Some(internal.children[index]);
                Some(Ok(node))
            }
            Node::Leaf(leaf) => {
                self.cursor = None;
                if leaf
                    .keys
                    .binary_search_by(|probe| probe.borrow().cmp(self.key))
                    .is_ok()
                {
                    Some(Ok(node))
                } else {
                    Some(Err(Error::UnknownKey))
                }
            }
        }
    }
}