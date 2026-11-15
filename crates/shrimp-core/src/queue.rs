//! The play queue (SPEC §4.4). **This one is Mr.Bigg's to write.**
//!
//! # How to work on it
//!
//! Every method below that says `todo!()` is yours. The task is "make the
//! tests pass". The tests are in `queue/tests.rs`, in three groups, and this
//! is a good order to do them in:
//!
//! 1. `basics`: start a queue, move through it, add, remove, move and clear.
//! 2. `repeat`: what happens at the ends of the queue.
//! 3. `shuffle`: a second order that can be turned on and off.
//!
//! Run one group at a time:
//!
//! ```text
//! cargo test -p shrimp-core queue::tests::basics
//! ```
//!
//! The private fields of `Queue` are a suggestion. You can change them, as
//! long as the public methods keep working the same way. If you do, update
//! `Queue::with_seed` to match.
//!
//! # Words used in this file
//!
//! - **Play order:** the order the tracks will play in. It is what the
//!   "up next" list shows. Every *position* that a method takes or returns is
//!   a position in the play order, starting at 0.
//! - **Original order:** the order before shuffling. It is the order of the
//!   album (or artist, or song list) the queue was started from, plus any
//!   tracks added later. With shuffle off, the two orders are the same.
//! - **Current track:** the track that is playing, or would play if you
//!   pressed play. An empty queue has no current track.

// TODO(Mr.Bigg): delete this line when the queue is done. It stops the
// compiler warning about the unused fields and arguments of the `todo!()`
// methods while you work.
#![allow(unused_variables, dead_code)]

use std::time::Duration;

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::TrackId;

/// What happens at the ends of the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    /// Stop after the last track.
    #[default]
    Off,
    /// Go back to the first track after the last one.
    All,
    /// Play the current track again when it finishes by itself.
    ///
    /// This changes only `track_finished`. For `skip_next` and
    /// `skip_previous`, it acts like `All`, so the skip buttons still work.
    One,
}

/// What the player should do after `Queue::skip_previous`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Previous {
    /// Seek the current track back to 0:00. The current track did not change.
    RestartCurrent,
    /// The queue moved back to this track, which is now the current track.
    GoTo(TrackId),
}

/// How long a track has to play before "previous" restarts it instead of
/// going back. SPEC §4.4 says "more than 3 s".
pub const RESTART_THRESHOLD: Duration = Duration::from_secs(3);

/// One place in the queue.
///
/// The same track can be in the queue twice, so a `TrackId` alone can't say
/// *which* copy you mean. Each entry gets its own `key`, which is never
/// reused, so the same entry can be found in both orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    key: u64,
    track: TrackId,
}

/// The play queue: an ordered list of tracks, a current track, shuffle and
/// repeat. It is pure logic, with no I/O and no audio.
pub struct Queue {
    /// The queue in play order.
    play_order: Vec<Entry>,
    /// The same entries in their original order. With shuffle off, this is
    /// the same as `play_order`.
    original_order: Vec<Entry>,
    /// The position of the current track in `play_order`. `None` when the
    /// queue is empty.
    current: Option<usize>,
    /// The key for the next new `Entry`. Add 1 after each use.
    next_key: u64,
    shuffle: bool,
    repeat: Repeat,
    /// Random numbers for shuffling. Use it like this:
    /// `some_vec.shuffle(&mut self.rng)`, after `use rand::seq::SliceRandom;`.
    rng: StdRng,
}

impl Queue {
    /// An empty queue, with shuffle off and repeat off.
    ///
    /// Shuffling uses a different random order each time the app runs.
    pub fn new() -> Self {
        Self::with_rng(rand::make_rng())
    }

    /// Like `new`, but the shuffle order comes from `seed`. The same seed
    /// always gives the same shuffle order, which is what the tests need.
    pub fn with_seed(seed: u64) -> Self {
        Self::with_rng(StdRng::seed_from_u64(seed))
    }

    fn with_rng(rng: StdRng) -> Self {
        Queue {
            play_order: Vec::new(),
            original_order: Vec::new(),
            current: None,
            next_key: 0,
            shuffle: false,
            repeat: Repeat::Off,
            rng,
        }
    }

    // ----- Looking at the queue -------------------------------------------

    /// How many tracks are in the queue.
    pub fn len(&self) -> usize {
        todo!()
    }

    /// `true` when the queue has no tracks.
    pub fn is_empty(&self) -> bool {
        todo!()
    }

    /// The tracks in play order.
    pub fn tracks(&self) -> Vec<TrackId> {
        todo!()
    }

    /// The current track, or `None` when the queue is empty.
    pub fn current(&self) -> Option<TrackId> {
        todo!()
    }

    /// The position of the current track in the play order, or `None` when
    /// the queue is empty.
    pub fn current_index(&self) -> Option<usize> {
        todo!()
    }

    // ----- Changing what is in the queue ----------------------------------

    /// Replace the whole queue with `context` and make `context[start]` the
    /// current track.
    ///
    /// This is "play this track now": tap a song in an album, and the queue
    /// becomes that album, starting at that song. If shuffle is on, the
    /// start track goes first and the rest are shuffled after it. An empty
    /// `context` empties the queue.
    ///
    /// `start` must be a valid position in a non-empty `context`, the same
    /// as for `context[start]`.
    pub fn play_now(&mut self, context: Vec<TrackId>, start: usize) {
        todo!()
    }

    /// Put `track` right after the current track, so it plays next.
    ///
    /// With shuffle on, it also goes right after the current track in the
    /// original order. If the queue is empty, `track` becomes the only track
    /// and the current track.
    pub fn add_next(&mut self, track: TrackId) {
        todo!()
    }

    /// Put `track` at the end of the queue (in both orders). The current
    /// track does not change, unless the queue was empty: then `track`
    /// becomes the current track.
    pub fn add_to_end(&mut self, track: TrackId) {
        todo!()
    }

    /// Take out the track at `position` and return it.
    ///
    /// With shuffle on, the same entry also leaves the original order.
    ///
    /// If it was the current track, the track after it becomes current. If
    /// there is no track after it, the track before it becomes current. If
    /// the queue is now empty, there is no current track. Otherwise the
    /// current track stays the same (but its position can change).
    ///
    /// Returns `None`, and changes nothing, if `position` is past the end.
    pub fn remove(&mut self, position: usize) -> Option<TrackId> {
        todo!()
    }

    /// Move the track at `from` so that it ends up at position `to`. The
    /// other tracks shift to make room, the same as `Vec::remove(from)`
    /// followed by `Vec::insert(to, ...)`.
    ///
    /// The current track stays the same, even if it is the one that moved.
    /// With shuffle on, only the play order changes: turning shuffle off
    /// goes back to the original order as if nothing moved.
    ///
    /// Returns `false`, and changes nothing, if `from` or `to` is past the
    /// end. Otherwise returns `true`.
    pub fn move_track(&mut self, from: usize, to: usize) -> bool {
        todo!()
    }

    /// Empty the queue. Shuffle and repeat keep their settings.
    pub fn clear(&mut self) {
        todo!()
    }

    // ----- Moving through the queue ---------------------------------------

    /// The "next" button: move to the next track and return it.
    ///
    /// At the last track:
    /// - with `Repeat::Off`, return `None` and stay on the last track (the
    ///   player stops);
    /// - with `Repeat::All` or `Repeat::One`, go back to the first track.
    ///
    /// Returns `None` for an empty queue.
    pub fn skip_next(&mut self) -> Option<TrackId> {
        todo!()
    }

    /// The current track ended by itself. Move on and return the track to
    /// play now.
    ///
    /// This is the same as `skip_next`, except with `Repeat::One`: then the
    /// current track stays current, and it is returned so it plays again.
    pub fn track_finished(&mut self) -> Option<TrackId> {
        todo!()
    }

    /// The "previous" button. `played` is how far into the current track
    /// the player is.
    ///
    /// - If `played` is more than `RESTART_THRESHOLD`, restart the current
    ///   track.
    /// - Otherwise go back one track.
    /// - At the first track, go back to the last track with `Repeat::All` or
    ///   `Repeat::One`. With `Repeat::Off`, restart the first track.
    ///
    /// Returns `None` for an empty queue.
    pub fn skip_previous(&mut self, played: Duration) -> Option<Previous> {
        todo!()
    }

    // ----- Shuffle and repeat ---------------------------------------------

    /// `true` when shuffle is on.
    pub fn shuffle(&self) -> bool {
        todo!()
    }

    /// Turn shuffle on or off. The current track stays current either way.
    ///
    /// - **On:** the current track moves to the front of the play order,
    ///   and all the other tracks are shuffled after it.
    /// - **Off:** the play order goes back to the original order.
    ///
    /// Asking for the setting it already has changes nothing. In particular,
    /// turning shuffle on when it is already on does not shuffle again.
    pub fn set_shuffle(&mut self, on: bool) {
        todo!()
    }

    /// The repeat setting.
    pub fn repeat(&self) -> Repeat {
        todo!()
    }

    /// Change the repeat setting. Nothing else changes.
    pub fn set_repeat(&mut self, repeat: Repeat) {
        todo!()
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
