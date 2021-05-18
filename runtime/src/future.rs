//! Handles async functions

use crate::returned::Returned;
use crate::Handle;
use crate::Marshal;
use futures_executor::ThreadPool;
use futures_util::future::RemoteHandle;
use futures_util::task::SpawnExt;
use futures_util::FutureExt;
use std::collections::HashMap;
use std::future::Future;
use std::lazy::SyncLazy;
use std::sync::Mutex;

/// Pool of [Future]s being run by Riko's own executor.
pub struct Pool {
    pool: HashMap<Handle, RemoteHandle<()>>,
    executor: ThreadPool,
    counter: Handle,
}

impl Pool {
    /// Spawns a future.
    pub fn spawn<F, N, R, T>(&mut self, task: F, notifier: N) -> Handle
    where
        F: Future<Output = R> + Send + 'static,
        N: FnOnce(Handle, Returned<T>) + Send + 'static,
        R: Into<Returned<T>>,
        T: Marshal,
    {
        let handle = self.new_handle();
        let task = async move {
            let result = task.await.into();
            if let Some(token) = POOL.lock().unwrap().pool.remove(&handle) {
                notifier(handle, result);
                token.forget()
            }
        };
        let (task, token) = task.remote_handle();
        let old_handle = self.pool.insert(handle, token);
        assert!(old_handle.is_none(), "Same handle used more than once");
        self.executor.spawn(task).expect("Failed to spawn a job");
        handle
    }

    /// Cancels a [Future] run by this [Pool].
    pub fn cancel(&mut self, handle: Handle) {
        self.pool.remove(&handle);
    }

    /// Creates a [Handle] by incrementing the internal counter.
    ///
    /// New [Handle]s are always monotonically increasing, which avoid collision.
    fn new_handle(&mut self) -> Handle {
        let previous = self.counter;
        self.counter += 1;
        previous
    }
}

impl Default for Pool {
    fn default() -> Self {
        let executor = ThreadPool::builder()
            .name_prefix("riko-")
            .create()
            .expect("Failed to create executor for Riko language bindings");
        Self {
            executor,
            pool: Default::default(),
            counter: Default::default(),
        }
    }
}

/// The singleton instance of [Pool].
pub static POOL: SyncLazy<Mutex<Pool>> = SyncLazy::new(Default::default);
