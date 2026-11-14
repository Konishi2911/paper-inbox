#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeResult<T> {
    Unchanged(T),
    TakeLocal(T),
    TakeRemote(T),
    Converged(T),

    Conflict { base: T, local: T, remote: T },
}

pub fn three_way_merge<T>(base: &T, local: &T, remote: &T) -> MergeResult<T>
where
    T: Clone + PartialEq,
{
    if local == base && remote == base {
        MergeResult::Unchanged(base.clone())
    } else if local == remote {
        MergeResult::Converged(local.clone())
    } else if remote == base {
        MergeResult::TakeLocal(local.clone())
    } else if local == base {
        MergeResult::TakeRemote(remote.clone())
    } else {
        MergeResult::Conflict {
            base: base.clone(),
            local: local.clone(),
            remote: remote.clone(),
        }
    }
}
