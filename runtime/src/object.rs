//! Handling heap-allocated objects.

use crate::returned::Returned;
use crate::Handle;
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::lazy::SyncLazy;
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
pub trait Object: Send + Sync + Any {}

/// Shelves an [Object] into [POOL].
pub trait Shelve: Any + Send + Sync {
    /// Shelves it.
    fn shelve(self) -> Returned<Handle>;
}

impl<T: Object> Shelve for T {
    fn shelve(self) -> Returned<Handle> {
        POOL.store(self).into()
    }
}

impl<T: Shelve> Shelve for Arc<T> {
    fn shelve(self) -> Returned<Handle> {
        POOL.store(self).into()
    }
}

impl<T: Shelve> Shelve for Option<T> {
    fn shelve(self) -> Returned<Handle> {
        match self {
            Some(obj) => obj.shelve(),
            None => Default::default(),
        }
    }
}

impl<T, E> Shelve for Result<T, E>
where
    T: Shelve,
    E: Error + Send + Sync + 'static,
{
    fn shelve(self) -> Returned<Handle> {
        match self {
            Ok(obj) => obj.shelve(),
            Err(err) => Returned {
                error: Some(err.into()),
                value: None,
            },
        }
    }
}

/// The global [Pool] that every [Object] is stored.
pub static POOL: SyncLazy<Pool> = SyncLazy::new(Default::default);

/// Thread-safe collection of [Object]s.
pub struct Pool {
    pool: RwLock<HashMap<Handle, Arc<Mutex<dyn Any + Send>>>>,
    counter: AtomicI32,
}

impl Pool {
    /// Runs an action on a [Object].
    pub fn peek<T: Any, R>(&self, handle: Handle, action: impl FnOnce(&mut T) -> R) -> R {
        let pool_guard = self.pool.read().expect("Failed to read-lock the pool");
        let obj_arc = pool_guard[&handle].clone();
        drop(pool_guard);

        let mut obj = obj_arc.lock().expect("Failed to lock the object");
        if let Some(obj_concrete) = obj.downcast_mut::<T>() {
            action(obj_concrete)
        } else {
            panic!("Incorrect data type pointed by this handle")
        }
    }

    /// Drops the object pointed by the [Handle].
    pub fn drop(&self, handle: Handle) {
        self.pool
            .write()
            .expect("Failed to write-lock the pool!")
            .remove(&handle);
    }

    /// Stores an object.
    pub fn store<T: Any + Send>(&self, obj: T) -> Handle {
        let mut pool_guard = self.pool.write().expect("Failed to write-lock the pool");
        let handle = self.counter.fetch_add(1, Ordering::Relaxed);
        pool_guard.insert(handle, Arc::new(Mutex::new(obj)));
        handle
    }

    /// Checks if the object pointed by `handle` is alive.
    pub fn alive(&self, handle: Handle) -> bool {
        self.pool
            .read()
            .expect("Failed to write-lock the pool")
            .contains_key(&handle)
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self {
            pool: HashMap::with_capacity(0).into(),
            counter: 0.into(),
        }
    }
}
