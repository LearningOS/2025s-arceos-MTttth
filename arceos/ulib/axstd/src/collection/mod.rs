#[cfg(feature = "alloc")]
pub use self::hashmap::HashMap;

#[cfg(feature = "alloc")]
pub mod hashmap;
