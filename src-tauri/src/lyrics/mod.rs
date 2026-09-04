use serde::{Deserialize, Serialize};

pub mod amll_ttml;
pub(crate) mod conversion;
pub mod credentials;
pub(crate) mod encoding;
pub mod kugou;
pub mod kuwo;
pub mod lrclib;
pub mod migu;
pub mod musixmatch;
pub mod netease;
pub mod provider;
pub mod qqmusic;

include!("types.rs");
include!("parser.rs");
include!("runtime.rs");

#[cfg(test)]
include!("tests.rs");
