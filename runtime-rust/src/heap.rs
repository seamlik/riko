//! Handling heap-allocated objects.

use crate::returned::Returned;
use rand::Rng;
use std::collections::HashMap;
use std::error::Error;
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
pub trait Pool<T> {
    /// Runs an action on a [Heaped].
    fn peek<R>(&self, handle: Handle, action: impl FnOnce(&mut T) -> R) -> R;

    /// Drops the object pointed by the [Handle].
    fn drop(&self, handle: Handle);

    /// Stores an object.
    fn store(&self, obj: T) -> Handle;
}

/// Simple implementation of [Pool].
pub type SimplePool<T> = RwLock<HashMap<Handle, Arc<Mutex<T>>>>;

impl<T> Pool<T> for SimplePool<T> {
    fn peek<R>(&self, handle: Handle, action: impl FnOnce(&mut T) -> R) -> R {
        let pool_guard = self.read().expect("Failed to read-lock the pool");
        let obj_arc = pool_guard[&handle].clone();
        std::mem::drop(pool_guard);

        let mut obj = obj_arc.lock().expect("Failed to lock the object");
        action(&mut *obj)
    }

    fn drop(&self, handle: Handle) {
        self.write()
            .expect("Failed to write-lock the pool!")
            .remove(&handle);
    }

    fn store(&self, obj: T) -> Handle {
        let mut pool_guard = self.write().expect("Failed to write-lock the pool!");
        let mut rng = rand::thread_rng();
        let handle = loop {
            let candidate = rng.gen();
            if !pool_guard.contains_key(&candidate) {
                break candidate;
            }
        };
        pool_guard.insert(handle, Arc::new(Mutex::new(obj)));
        handle
    }
}
