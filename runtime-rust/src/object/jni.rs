#[no_mangle]
pub extern "C" fn Java_riko_Object_close(env: ::jni::JNIEnv, this: ::jni::objects::JObject) {
    super::POOL.drop(get_handle(env, this));
}

#[no_mangle]
pub extern "C" fn Java_riko_Object_alive(
    env: ::jni::JNIEnv,
    this: ::jni::objects::JObject,
) -> bool {
    super::POOL.alive(get_handle(env, this))
}

fn get_handle(env: ::jni::JNIEnv, this: ::jni::objects::JObject) -> super::Handle {
    env.get_field(this, "handle", "I")
        .expect("Failed to get field `handle`")
        .i()
        .expect("`handle` is not an int")
}
