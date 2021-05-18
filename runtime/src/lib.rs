//! Runtime for wrapper code generated by Riko - Core component.

#![feature(once_cell)]

use serde::de::DeserializeOwned;
use serde::Serialize;

pub mod future;
pub mod object;
pub mod returned;

/// Data marshaled between the FFI boundary.
///
/// Data will be encoded as BSON bytes and sent across FFI.
pub trait Marshal: Serialize + DeserializeOwned {}

/// All data types that supports Serde automatically implement this trait.
impl<T: Serialize + DeserializeOwned> Marshal for T {}

/// Opaque handle pointing to some artifact in Rust.
pub type Handle = i32;
