//! Support for marshaling iterators.

use crate::heap::Handle;
use crate::heap::Heaped;
use crate::heap::Pool;
use crate::heap::SimplePool;
use crate::returned::Returned;
use jni::objects::JClass;
use jni::sys::jbyteArray;
use jni::JNIEnv;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::error::Error;

/// [Iterator] to be used by the target code.
///
/// This [Iterator] is allocated on the heap and serializes the emitted items of the original
/// [Iterator].
pub struct ReturningIterator(Box<dyn Iterator<Item = Vec<u8>> + Send>);

impl ReturningIterator {
    pub fn new<T, R, I>(src: I) -> Self
    where
        T: Serialize,
        R: Into<Returned<T>>,
        I: Iterator<Item = R> + Send + 'static,
    {
        let inner = src.map(|item| {
            let returned: Returned<T> = item.into();
            serde_cbor::to_vec(&returned)
                .expect("Failed to serialize the data returned by an iterator.")
        });
        Self(Box::new(inner))
    }
}

impl Iterator for ReturningIterator {
    type Item = Vec<u8>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl Heaped for ReturningIterator {
    fn into_handle(self) -> Returned<Handle> {
        Returned {
            error: None,
            value: Some(POOL.store(self)),
        }
    }
}

static POOL: Lazy<SimplePool<ReturningIterator>> = Lazy::new(Default::default);

#[no_mangle]
pub extern "C" fn Java_riko_ReturningIterator__1_1next(
    env: JNIEnv,
    _: JClass,
    handle: Handle,
) -> jbyteArray {
    let action = |iterator: &mut ReturningIterator| match iterator.next() {
        Some(data) => env
            .byte_array_from_slice(&data)
            .expect("Failed to send the data from an iterator to JNI."),
        None => env
            .new_byte_array(0)
            .expect("Failed to allocate an empty byte array from JNI."),
    };
    POOL.peek(handle, action)
}

#[no_mangle]
pub extern "C" fn Java_riko_ReturningIterator__1_1drop(
    _env: JNIEnv,
    _class: JClass,
    handle: Handle,
) {
    POOL.drop(handle);
}

/// Marshals a raw [Iterator] into a [Handle] of a [ReturningIterator].
///
/// Creating this trait instead of using [From] is because the type parameter `T` is considered
/// unconstrained and must appear in the trait definition. See its implementations for how it is
/// used.
pub trait IntoReturned<T: Serialize> {
    /// Performs the marshaling.
    fn into(self) -> Returned<Handle>;
}

/// Implements on a trait object instead of a concrete [Iterator].
///
/// This is because Rust forbids implementing on both [Iterator] and [Result] considering that the
/// upstream crate might one day implement [Iterator] on [Result].
impl<T, R> IntoReturned<T> for Box<dyn Iterator<Item = R> + Send + 'static>
where
    T: Serialize,
    R: Into<Returned<T>> + 'static,
{
    fn into(self) -> Returned<Handle> {
        ReturningIterator::new(self).into_handle()
    }
}

/// Implements on `Result<I, E>` but not `Result<Option<I>, E>`.
///
/// This is because Rust forbids implementing on both [Iterator] and [Option] considering that the
/// upstream crate might one day implement [Iterator] on [Option].
///
/// To express the semantics of `Option<Iterator>`, an empty [Iterator] can be used instead.
impl<T, R, I, E> IntoReturned<T> for Result<I, E>
where
    E: Error,
    I: Iterator<Item = R> + Send + 'static,
    R: Into<Returned<T>> + 'static,
    T: Serialize,
{
    fn into(self) -> Returned<Handle> {
        self.map(|iter| POOL.store(ReturningIterator::new(iter)))
            .into()
    }
}
