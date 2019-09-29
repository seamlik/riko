//! Runtime for wrapper code generated by Riko.
//!
//! End user need only to care about the types and functions at the root level. Anything in the
//! other modules are only used by generated code.

use ::jni::sys::jbyteArray;
use ::jni::JNIEnv;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub mod heap;
pub mod returned;

/// Object marshaled between the FFI boundry.
///
/// The marshaling strategy is to utilize [Serde](https://serde.rs). The object will be serialized
/// into [CBOR](https://cbor.io) byte array and sent between the FFI boundary.
pub trait Marshaled: Serialize + DeserializeOwned {
    fn to_jni(&self, env: &JNIEnv) -> jbyteArray {
        let output = serde_cbor::to_vec(self).expect("Failed to marshal the object.");
        env.byte_array_from_slice(&output)
            .expect("Failed to send the marshaled data to JNI.")
    }

    fn from_jni(env: &JNIEnv, src: jbyteArray) -> Self {
        let input = env
            .convert_byte_array(src)
            .expect("Failed to receive a byte array from JNI.");
        serde_cbor::from_slice(&input).expect("Type mismatch for this CBOR data.")
    }
}

/// All data types that supports Serde altomatically implements this trait.
impl<'de, T: Serialize + DeserializeOwned> Marshaled for T {}
