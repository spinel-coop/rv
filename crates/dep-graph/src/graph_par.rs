use crate::{
    error::Error,
    graph::{remove_node_id, DepGraph, DependencyMap},
};
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
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, RwLock,
};
use std::thread;
use std::time::Duration;

/// Default timeout in milliseconds
const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1000);

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
    // Reference to the number of items being currently processed
    counter: Arc<AtomicUsize>,
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
    pub fn new(inner: I, counter: Arc<AtomicUsize>, item_done_tx: Sender<I>) -> Self {
        (*counter).fetch_add(1, Ordering::SeqCst);
        Self {
            inner,
            counter,
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
        (*self.counter).fetch_sub(1, Ordering::SeqCst);
        // Ignore send errors - the receiver may have been dropped.
        // Drop implementations should not panic per Rust best practices,
        // as panicking during unwinding from another panic will abort the program.
        // See: https://doc.rust-lang.org/std/ops/trait.Drop.html
        // See: https://github.com/nmoutschen/dep-graph/issues/3
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
    timeout: Arc<RwLock<Duration>>,
    counter: Arc<AtomicUsize>,
    item_ready_rx: Receiver<I>,
    item_done_tx: Sender<I>,
    /// Total number of nodes in the graph (used for rayon's len() hint)
    total_nodes: usize,
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
        let timeout = Arc::new(RwLock::new(DEFAULT_TIMEOUT));
        let counter = Arc::new(AtomicUsize::new(0));

        // Capture total node count before moving deps to the dispatcher thread.
        // This is used by IndexedParallelIterator::len() to give rayon an accurate
        // hint about parallelism, preventing it from spawning more workers than items.
        let total_nodes = deps.read().unwrap().len();

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

        // Clone Arcs for dispatcher thread
        let loop_timeout = timeout.clone();

        // Start dispatcher thread
        thread::spawn(move || {
            loop {
                crossbeam_channel::select! {
                    // Grab a processed node ID
                    recv(item_done_rx) -> id => {
                        let id = id.unwrap();
                        in_flight -= 1;

                        // Remove the node from all reverse dependencies
                        let next_nodes = remove_node_id::<I>(id, &deps, &rdeps)?;

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
                    default(*loop_timeout.read().unwrap()) => {
                        let deps = deps.read().unwrap();
                        if deps.is_empty() {
                            break;
                        // There are still items in flight (dispatched but not completed).
                        // This properly handles the race window between recv() and Wrapper::new().
                        // See: https://github.com/nmoutschen/dep-graph/issues/3
                        } else if in_flight > 0 {
                            continue;
                        } else {
                            return Err(Error::ResolveGraphError("circular dependency detected"));
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
            timeout,
            counter,
            item_ready_rx,
            item_done_tx,
            total_nodes,
        }
    }

    pub fn with_timeout(self, timeout: Duration) -> Self {
        *self.timeout.write().unwrap() = timeout;
        self
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
        // Return the minimum of node count and CPU count.
        //
        // - If len() > total_nodes: rayon spawns excess workers that block
        //   forever on recv(), causing deadlocks (seen on ARM with 8+ cores).
        // - If len() > num_cpus: rayon's work distribution becomes inefficient,
        //   causing severe slowdowns (seen on ubuntu-latest with 2 cores).
        //
        // Using min() handles both cases: it prevents deadlocks while keeping
        // rayon's work-stealing scheduler efficient.
        std::cmp::min(self.total_nodes, num_cpus::get())
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
            counter: self.counter,
            item_ready_rx: self.item_ready_rx,
            item_done_tx: self.item_done_tx,
        })
    }
}

struct DepGraphProducer<I>
where
    I: Clone + fmt::Debug + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    counter: Arc<AtomicUsize>,
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
            Ok(item) => Some(Wrapper::new(
                item,
                self.counter.clone(),
                self.item_done_tx.clone(),
            )),
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
            counter: self.counter,
            item_ready_rx: self.item_ready_rx,
            item_done_tx: self.item_done_tx,
        }
    }

    fn split_at(self, _: usize) -> (Self, Self) {
        (
            Self {
                counter: self.counter.clone(),
                item_ready_rx: self.item_ready_rx.clone(),
                item_done_tx: self.item_done_tx.clone(),
            },
            Self {
                counter: self.counter.clone(),
                item_ready_rx: self.item_ready_rx.clone(),
                item_done_tx: self.item_done_tx,
            },
        )
    }
}
