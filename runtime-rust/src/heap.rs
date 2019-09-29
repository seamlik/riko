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

/// Thread-safe and type-safe collection of [Heaped](crate::heap::Heaped)s.
pub type Pool<T> = RwLock<HashMap<Handle, Arc<Mutex<T>>>>;

/// Opaque handle pointing to a [Heaped](crate::heap::Heaped).
pub type Handle = i32;

/// Applies a closure on a [Heaped](crate::heap::Heaped).
pub fn peek<T, R>(pool: &Pool<T>, handle: Handle, action: impl FnOnce(&mut T) -> R) -> R {
    let pool_guard = pool.read().expect("Failed to read-lock the pool!");
    let raw = pool_guard.get(&handle).expect("Invalid handle!").clone();
    std::mem::drop(pool_guard);

    let mut obj = raw.lock().expect("Failed to lock the object!");
    action(&mut *obj)
}

/// Drops the object pointed by the [Handle].
pub fn drop<T>(pool: &Pool<T>, handle: Handle) {
    pool.write()
        .expect("Failed to write-lock the pool!")
        .remove(&handle);
}

/// Stores an object in its pool.
pub fn store<T>(pool: &Pool<T>, obj: T) -> Handle {
    let mut pool_guard = pool.write().expect("Failed to write-lock the pool!");
    let mut rng = rand::thread_rng();
    let handle = loop {
        let candidate = rng.gen();
        if !pool_guard.contains_key(&candidate) {
            break candidate;
        }
    };
    pool_guard.insert(handle.clone(), Arc::new(Mutex::new(obj)));
    handle
}
