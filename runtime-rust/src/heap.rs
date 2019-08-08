//! Operations for handling heap-allocated objects.

use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

/// Thread-safe and type-safe collection of [Heap](crate::Heap)s.
pub type Pool<T> = RwLock<HashMap<Handle, Arc<Mutex<T>>>>;

/// Opaque handle pointing to a [Heap](crate::Heap).
pub type Handle = i32;

/// Applies a closure on a [Heap](crate::Heap).
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
