//! Handling heap-allocated objects.

use crate::returned::Returned;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

/// Object allocated on the heap.
///
/// These objects are allocated and freed on the Rust side while only expose a reference to the
/// target side. Target code must integrate the manual memory management into its own mechanism as
/// those memory management strategy (usually garbage collection) is not aware of any native code.
pub trait Heaped: Sized {
    fn into_handle(self) -> Returned<Handle>;
}

impl<T: Heaped, E: Error> Heaped for Result<T, E> {
    fn into_handle(self) -> Returned<Handle> {
        match self {
            Ok(obj) => obj.into_handle(),
            Err(err) => Returned {
                error: Some(err.into()),
                value: None,
            },
        }
    }
}

/// Opaque handle pointing to a [Heaped].
pub type Handle = i32;

/// Thread-safe and type-safe collection of [Heaped]s.
pub struct Pool<T> {
    pool: RwLock<HashMap<Handle, Arc<Mutex<T>>>>,
    counter: AtomicI32,
}

impl<T> Pool<T> {
    /// Runs an action on a [Heaped].
    pub fn peek<R>(&self, handle: Handle, action: impl FnOnce(&mut T) -> R) -> R {
        let pool_guard = self.pool.read().expect("Failed to read-lock the pool");
        let obj_arc = pool_guard[&handle].clone();
        drop(pool_guard);

        let mut obj = obj_arc.lock().expect("Failed to lock the object");
        action(&mut *obj)
    }

    /// Drops the object pointed by the [Handle].
    pub fn drop(&self, handle: Handle) {
        self.pool
            .write()
            .expect("Failed to write-lock the pool!")
            .remove(&handle);
    }

    /// Stores an object.
    pub fn store(&self, obj: T) -> Handle {
        let mut pool_guard = self.pool.write().expect("Failed to write-lock the pool");
        let handle = self.counter.fetch_add(1, Ordering::Relaxed);
        pool_guard.insert(handle, Arc::new(Mutex::new(obj)));
        handle
    }

    pub const fn new() -> Lazy<Self> {
        Lazy::new(Default::default)
    }
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self {
            pool: HashMap::with_capacity(0).into(),
            counter: 0.into(),
        }
    }
}

pub type LazyPool<T> = Lazy<Pool<T>>;
