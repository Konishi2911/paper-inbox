use std::future::Future;

use crate::model::{MetadataCandidate, PaperIdentifier};

pub trait MetadataResolver: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    fn resolve(
        &self,
        identifier: &PaperIdentifier,
    ) -> impl Future<Output = Result<Vec<MetadataCandidate>, Self::Error>> + Send;
}
