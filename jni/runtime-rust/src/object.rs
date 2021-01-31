//! JNI bridge functions for heap-allocated objects.

use jni::objects::JObject;
use jni::JNIEnv;
use riko_runtime::object::POOL;
use riko_runtime::Handle;

#[no_mangle]
pub extern "C" fn Java_riko_Object_close(env: JNIEnv, this: JObject) {
    POOL.drop(get_handle(env, this));
}

#[no_mangle]
pub extern "C" fn Java_riko_Object_alive(env: JNIEnv, this: JObject) -> bool {
    POOL.alive(get_handle(env, this))
}

fn get_handle(env: JNIEnv, this: JObject) -> Handle {
    env.get_field(this, "handle", "I")
        .expect("Failed to get field `handle`")
        .i()
        .expect("`handle` is not an int")
}
