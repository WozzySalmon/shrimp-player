//! Types that name a track.

/// The id of a track in the library index.
///
/// It is the `id` column of the `tracks` table (SPEC §4.2). Wrapping the
/// number in its own type means a track id can't be mixed up with some
/// other number, such as a position in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub i64);
