use std::io::{Read, Write};

use crate::model::ContentHash;

pub trait BlobStore: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    fn put(&self, reader: &mut dyn Read) -> Result<ContentHash, Self::Error>;

    fn read(&self, hash: &ContentHash, writer: &mut dyn Write) -> Result<(), Self::Error>;

    fn contains(&self, hash: &ContentHash) -> Result<bool, Self::Error>;
}
