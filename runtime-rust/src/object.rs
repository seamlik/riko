//! Handling heap-allocated objects.

use crate::returned::Returned;
use once_cell::sync::Lazy;
use std::any::Any;
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
pub trait Object: Any + Sized + Send {
    /// Shelves an object into [POOL].
    fn shelve_self(self) -> Returned<Handle> {
        POOL.store(self).into()
    }

    /// Shelves an object in a [Result].
    fn shelve_result<E: Error>(src: Result<Self, E>) -> Returned<Handle> {
        match src {
            Ok(obj) => obj.shelve_self(),
            Err(err) => Returned {
                error: Some(err.into()),
                value: None,
            },
        }
    }

    /// Shelves an optional object.
    fn shelve_option(src: Option<Self>) -> Returned<Handle> {
        match src {
            Some(obj) => obj.shelve_self(),
            None => Default::default(),
        }
    }

    /// Shelves an optional object in a [Result].
    fn shelve_result_option<E: Error>(src: Result<Option<Self>, E>) -> Returned<Handle> {
        match src {
            Ok(Some(obj)) => obj.shelve_self(),
            Ok(None) => Default::default(),
            Err(err) => Returned {
                error: Some(err.into()),
                value: None,
            },
        }
    }
}

impl<T: Any + Sized + Send> Object for T {}

/// Opaque handle pointing to a [Object].
pub type Handle = i32;

/// The global [Pool] that every [Object] is stored.
pub static POOL: Lazy<Pool> = Lazy::new(Default::default);

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

    pub const fn new() -> Lazy<Self> {
        Lazy::new(Default::default)
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

#[cfg(feature = "riko_jni")]
#[no_mangle]
pub extern "C" fn Java_riko_Object_drop(
    _: ::jni::JNIEnv,
    _: ::jni::objects::JClass,
    handle: Handle,
) {
    POOL.drop(handle);
}

#[cfg(feature = "riko_jni")]
#[no_mangle]
pub extern "C" fn Java_riko_Object_aliveNative(
    _: ::jni::JNIEnv,
    _: ::jni::objects::JClass,
    handle: Handle,
) -> bool {
    POOL.alive(handle)
}
