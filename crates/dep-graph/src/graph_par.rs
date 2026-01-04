use crate::graph::{remove_node_id, DepGraph, DependencyMap};
use crossbeam_channel::{Receiver, Sender};

use rayon::iter::{
    plumbing::{bridge, Consumer, Producer, ProducerCallback, UnindexedConsumer},
    IndexedParallelIterator, IntoParallelIterator, ParallelIterator,
};
use std::cmp;

use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::{DoubleEndedIterator, ExactSizeIterator};

use std::ops;
use std::thread;
use std::time::Duration;

/// Default timeout in milliseconds
const DEFAULT_TIMEOUT: Duration = Duration::from_millis(100);

/// Add into_par_iter() to DepGraph
impl<I> IntoParallelIterator for DepGraph<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    type Item = Wrapper<I>;
    type Iter = DepGraphParIter<I>;

    fn into_par_iter(self) -> Self::Iter {
        DepGraphParIter::new(self.ready_nodes, self.deps, self.rdeps)
    }
}

/// Wrapper for an item
///
/// This is used to pass items through parallel iterators. When the wrapper is
/// dropped, we decrement the processing `counter` and notify the dispatcher
/// thread through the `item_done_tx` channel.
#[derive(Clone)]
pub struct Wrapper<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    // Wrapped item
    inner: I,
    // Channel to notify that the item is done processing (upon drop)
    item_done_tx: Sender<I>,
}

impl<I> Wrapper<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    /// Create a new Wrapper item
    ///
    /// This needs a reference to the processing counter to keep count of the
    /// number of items currently processed (used to check for circular
    /// dependencies) and the item done channel to notify the dispatcher
    /// thread.
    ///
    /// Upon creating of a `Wrapper`, we also increment the processing counter.
    pub fn new(inner: I, item_done_tx: Sender<I>) -> Self {
        Self {
            inner,
            item_done_tx,
        }
    }
}

/// Drop implementation to decrement the processing counter and notify the
/// dispatcher thread.
impl<I> Drop for Wrapper<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    /// Triggered when the wrapper is dropped.
    ///
    /// This will decrement the processing counter and notify the dispatcher thread.
    fn drop(&mut self) {
        let _ = self.item_done_tx.send(self.inner.clone());
    }
}

/// Dereference implementation to access the inner item
///
/// This allow accessing the item using `(*wrapper)`.
impl<I> ops::Deref for Wrapper<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    type Target = I;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Dereference implementation to access the inner item
///
/// This allow accessing the item using `(*wrapper)`.
impl<I> ops::DerefMut for Wrapper<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<I> Eq for Wrapper<I> where I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static
{}

impl<I> Hash for Wrapper<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<I> cmp::PartialEq for Wrapper<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

/// Parallel iterator for DepGraph
pub struct DepGraphParIter<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    item_ready_rx: Receiver<I>,
    item_done_tx: Sender<I>,
}

impl<I> DepGraphParIter<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    /// Create a new parallel iterator
    ///
    /// This will create a thread and crossbeam channels to listen/send
    /// available and processed nodes.
    pub fn new(ready_nodes: Vec<I>, deps: DependencyMap<I>, rdeps: DependencyMap<I>) -> Self {
        // Create communication channel for processed nodes
        let (item_ready_tx, item_ready_rx) = crossbeam_channel::unbounded::<I>();
        let (item_done_tx, item_done_rx) = crossbeam_channel::unbounded::<I>();

        // Track items in flight: dispatched but not yet completed.
        // This is more reliable than checking counter + pending_items because
        // there's a race window between recv() and Wrapper::new() where neither
        // counter nor pending_items accounts for the item.
        let mut in_flight: usize = 0;

        // Inject ready nodes
        for node in &ready_nodes {
            item_ready_tx.send(node.clone()).unwrap();
            in_flight += 1;
        }

        // Start dispatcher thread
        thread::spawn(move || {
            loop {
                crossbeam_channel::select! {
                    // Grab a processed node ID
                    recv(item_done_rx) -> id => {
                        let id = id.unwrap();
                        in_flight -= 1;

                        // Remove the node from all reverse dependencies
                        let next_nodes = remove_node_id::<I>(id, &deps, &rdeps).unwrap();

                        // Send the next available nodes to the channel.
                        for node_id in &next_nodes {
                            item_ready_tx.send(node_id.clone()).unwrap();
                            in_flight += 1;
                        }

                        // If there are no more nodes, leave the loop
                        if deps.read().unwrap().is_empty() {
                            break;
                        }
                    },
                    // Timeout
                    default(DEFAULT_TIMEOUT) => {
                        let deps = deps.read().unwrap();
                        if deps.is_empty() {
                            break;
                        // There are still items in flight (dispatched but not completed).
                        // This properly handles the race window between recv() and Wrapper::new().
                        // See: https://github.com/nmoutschen/dep-graph/issues/3
                        } else if in_flight > 0 {
                            continue;
                        } else {
                            return Err(crate::error::Error::ResolveGraphError("circular dependency detected"));
                        }
                    },
                };
            }

            // Drop channel
            // This will close threads listening to it
            drop(item_ready_tx);
            Ok(())
        });

        DepGraphParIter {
            item_ready_rx,
            item_done_tx,
        }
    }
}

impl<I> ParallelIterator for DepGraphParIter<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    type Item = Wrapper<I>;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }
}

impl<I> IndexedParallelIterator for DepGraphParIter<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    fn len(&self) -> usize {
        num_cpus::get()
    }

    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        callback.callback(DepGraphProducer {
            item_ready_rx: self.item_ready_rx,
            item_done_tx: self.item_done_tx,
        })
    }
}

struct DepGraphProducer<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    item_ready_rx: Receiver<I>,
    item_done_tx: Sender<I>,
}

impl<I> Iterator for DepGraphProducer<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    type Item = Wrapper<I>;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO: Check until there is an item available
        match self.item_ready_rx.recv() {
            Ok(item) => Some(Wrapper::new(item, self.item_done_tx.clone())),
            Err(_) => None,
        }
    }
}

impl<I> DoubleEndedIterator for DepGraphProducer<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.next()
    }
}

impl<I> ExactSizeIterator for DepGraphProducer<I> where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static
{
}

impl<I> Producer for DepGraphProducer<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    type Item = Wrapper<I>;
    type IntoIter = Self;

    fn into_iter(self) -> Self::IntoIter {
        Self {
            item_ready_rx: self.item_ready_rx,
            item_done_tx: self.item_done_tx,
        }
    }

    fn split_at(self, _: usize) -> (Self, Self) {
        (
            Self {
                item_ready_rx: self.item_ready_rx.clone(),
                item_done_tx: self.item_done_tx.clone(),
            },
            Self {
                item_ready_rx: self.item_ready_rx.clone(),
                item_done_tx: self.item_done_tx,
            },
        )
    }
}
