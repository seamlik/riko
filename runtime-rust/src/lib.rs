//! Runtime for wrapper code generated by Riko.
//!
//! # Features
//!
//! This crate has features providing support for language targets:
//!
//! * `riko_jni`

#![feature(const_fn)]

use bson::Bson;
use bson::Document;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub mod object;
pub mod returned;

const ROOT_KEY_OF_ARGUMENT_DOCUMENT: &str = "value";

/// Data marshaled between the FFI boundary.
///
/// This trait defines the functions to marshal data. They are defined as trait methods to gain
/// static type dispatch.
pub trait Marshal: Serialize + DeserializeOwned {
    #[cfg(feature = "riko_jni")]
    fn to_jni(&self, env: &::jni::JNIEnv) -> ::jni::sys::jbyteArray {
        let mut buffer = Vec::<u8>::new();
        bson::to_bson(self)
            .expect("Failed to encode the object as BSON")
            .as_document()
            .expect("The BSON is supposed to be a document")
            .to_writer(&mut buffer)
            .expect("Failed to write to the buffer");
        env.byte_array_from_slice(&buffer)
            .expect("Failed to send the marshaled data to JNI")
    }

    #[cfg(feature = "riko_jni")]
    fn from_jni(env: &::jni::JNIEnv, src: ::jni::sys::jbyteArray) -> Self {
        let input = env
            .convert_byte_array(src)
            .expect("Failed to receive a byte array from JNI");

        let document = if input.is_empty() {
            Default::default()
        } else {
            Document::from_reader(&mut input.as_slice()).expect("Failed to parse the data as BSON")
        };
        let entry = document
            .into_iter()
            .find(|(key, _)| key == ROOT_KEY_OF_ARGUMENT_DOCUMENT);
        if let Some((_, value)) = entry {
            bson::from_bson(value)
        } else {
            bson::from_bson(Bson::Null)
        }
        .expect("Type mismatch")
    }
}

/// All data types that supports Serde automatically implement this trait.
impl<T: Serialize + DeserializeOwned> Marshal for T {}
