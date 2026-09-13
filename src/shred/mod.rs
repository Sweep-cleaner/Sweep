//! Secure deletion.

pub mod overwrite;

pub use overwrite::{
    overwrite_contents, random_name, rename_random, shred_file, truncate_contents, Pattern,
    ShredSpec,
};
