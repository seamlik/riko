//! Bridge functions for handling async functions.

use riko_runtime::future::POOL;
use riko_runtime::returned::Returned;
use riko_runtime::Handle;
use riko_runtime::Marshal;
use std::future::Future;

fn notify_completed<T: Marshal>(handle: Handle, result: Returned<T>) {
    let jvm_nullable = crate::java_vm();
    let jvm = jvm_nullable
        .as_ref()
        .expect("Riko runtime is not initialized");
    let env = jvm
        .attach_current_thread()
        .expect("Failed to attach current thread to JVM");
    let data = crate::marshal(&result, &env);

    env.call_static_method(
        env.find_class("riko/Future")
            .expect("Class `riko.Future` not found"),
        "complete",
        "(I[B)V",
        &[handle.into(), data.into()],
    )
    .expect("Failed to notify the completion of a future");
}

/// Spawns a [Future] and returns a [Handle] to it.
pub fn spawn<F, R, T>(task: F) -> Handle
where
    F: Future<Output = R> + Send + 'static,
    R: Into<Returned<T>>,
    T: Marshal + 'static,
{
    POOL.lock().unwrap().spawn(task, notify_completed)
}

#[no_mangle]
pub extern "C" fn Java_riko_Future_cancel(
    _: ::jni::JNIEnv,
    _: ::jni::objects::JClass,
    handle: Handle,
) {
    POOL.lock().unwrap().cancel(handle)
}
