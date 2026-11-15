//! Tests for the queue. Make these pass; don't change them. If a test looks
//! wrong to you, say so, and we'll talk about it.

use std::time::Duration;

use super::{Previous, Queue, Repeat};
use crate::TrackId;

/// Track ids 1, 2, ..., n.
fn tracks(n: i64) -> Vec<TrackId> {
    (1..=n).map(TrackId).collect()
}

/// The queue's tracks as plain numbers, so a failing test prints `[1, 2, 3]`
/// instead of `[TrackId(1), TrackId(2), TrackId(3)]`.
fn ids(queue: &Queue) -> Vec<i64> {
    queue.tracks().iter().map(|t| t.0).collect()
}

/// A queue playing tracks 1 to `n`, with track `start + 1` current.
fn playing(n: i64, start: usize) -> Queue {
    let mut q = Queue::with_seed(7);
    q.play_now(tracks(n), start);
    q
}

fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(s)
}

mod basics {
    use super::*;

    #[test]
    fn a_new_queue_is_empty() {
        let q = Queue::with_seed(1);
        assert_eq!(q.len(), 0);
        assert!(q.is_empty());
        assert_eq!(ids(&q), Vec::<i64>::new());
        assert_eq!(q.current(), None);
        assert_eq!(q.current_index(), None);
        assert!(!q.shuffle());
        assert_eq!(q.repeat(), Repeat::Off);
    }

    #[test]
    fn play_now_starts_at_the_chosen_track() {
        let q = playing(5, 2);
        assert_eq!(ids(&q), vec![1, 2, 3, 4, 5]);
        assert_eq!(q.len(), 5);
        assert!(!q.is_empty());
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(2));
    }

    #[test]
    fn play_now_replaces_the_old_queue() {
        let mut q = playing(5, 4);
        q.play_now(vec![TrackId(10), TrackId(11)], 0);
        assert_eq!(ids(&q), vec![10, 11]);
        assert_eq!(q.current(), Some(TrackId(10)));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn play_now_with_nothing_empties_the_queue() {
        let mut q = playing(5, 1);
        q.play_now(Vec::new(), 0);
        assert!(q.is_empty());
        assert_eq!(q.current(), None);
    }

    #[test]
    fn next_moves_forward() {
        let mut q = playing(3, 0);
        assert_eq!(q.skip_next(), Some(TrackId(2)));
        assert_eq!(q.current(), Some(TrackId(2)));
        assert_eq!(q.skip_next(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(2));
    }

    #[test]
    fn next_at_the_end_stops_and_stays_on_the_last_track() {
        let mut q = playing(3, 2);
        assert_eq!(q.skip_next(), None);
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(2));
    }

    #[test]
    fn next_on_an_empty_queue_gives_nothing() {
        let mut q = Queue::with_seed(1);
        assert_eq!(q.skip_next(), None);
        assert_eq!(q.current(), None);
    }

    #[test]
    fn a_finished_track_moves_to_the_next_one() {
        let mut q = playing(3, 0);
        assert_eq!(q.track_finished(), Some(TrackId(2)));
        assert_eq!(q.current(), Some(TrackId(2)));
    }

    #[test]
    fn a_finished_last_track_stops() {
        let mut q = playing(3, 2);
        assert_eq!(q.track_finished(), None);
        assert_eq!(q.current(), Some(TrackId(3)));
    }

    #[test]
    fn previous_early_in_a_track_goes_back() {
        let mut q = playing(3, 2);
        assert_eq!(q.skip_previous(secs(1.0)), Some(Previous::GoTo(TrackId(2))));
        assert_eq!(q.current(), Some(TrackId(2)));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn previous_at_exactly_three_seconds_still_goes_back() {
        let mut q = playing(3, 2);
        assert_eq!(q.skip_previous(secs(3.0)), Some(Previous::GoTo(TrackId(2))));
    }

    #[test]
    fn previous_after_three_seconds_restarts_the_track() {
        let mut q = playing(3, 2);
        assert_eq!(q.skip_previous(secs(3.1)), Some(Previous::RestartCurrent));
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(2));
    }

    #[test]
    fn previous_on_the_first_track_restarts_it() {
        let mut q = playing(3, 0);
        assert_eq!(q.skip_previous(secs(0.5)), Some(Previous::RestartCurrent));
        assert_eq!(q.current(), Some(TrackId(1)));
    }

    #[test]
    fn previous_on_an_empty_queue_gives_nothing() {
        let mut q = Queue::with_seed(1);
        assert_eq!(q.skip_previous(secs(0.0)), None);
    }

    #[test]
    fn add_next_goes_right_after_the_current_track() {
        let mut q = playing(4, 1);
        q.add_next(TrackId(9));
        assert_eq!(ids(&q), vec![1, 2, 9, 3, 4]);
        assert_eq!(q.current(), Some(TrackId(2)));
        assert_eq!(q.skip_next(), Some(TrackId(9)));
    }

    #[test]
    fn add_next_twice_plays_the_newest_first() {
        let mut q = playing(2, 0);
        q.add_next(TrackId(8));
        q.add_next(TrackId(9));
        assert_eq!(ids(&q), vec![1, 9, 8, 2]);
    }

    #[test]
    fn add_next_to_an_empty_queue_makes_it_current() {
        let mut q = Queue::with_seed(1);
        q.add_next(TrackId(9));
        assert_eq!(ids(&q), vec![9]);
        assert_eq!(q.current(), Some(TrackId(9)));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn add_to_end_goes_last_and_keeps_the_current_track() {
        let mut q = playing(3, 1);
        q.add_to_end(TrackId(9));
        assert_eq!(ids(&q), vec![1, 2, 3, 9]);
        assert_eq!(q.current(), Some(TrackId(2)));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn add_to_end_of_an_empty_queue_makes_it_current() {
        let mut q = Queue::with_seed(1);
        q.add_to_end(TrackId(9));
        q.add_to_end(TrackId(10));
        assert_eq!(ids(&q), vec![9, 10]);
        assert_eq!(q.current(), Some(TrackId(9)));
    }

    #[test]
    fn the_same_track_can_be_queued_twice() {
        let mut q = playing(3, 0);
        q.add_to_end(TrackId(1));
        assert_eq!(ids(&q), vec![1, 2, 3, 1]);
        assert_eq!(q.remove(3), Some(TrackId(1)));
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn remove_before_the_current_track_keeps_it_current() {
        let mut q = playing(4, 2);
        assert_eq!(q.remove(0), Some(TrackId(1)));
        assert_eq!(ids(&q), vec![2, 3, 4]);
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn remove_after_the_current_track_keeps_it_current() {
        let mut q = playing(4, 1);
        assert_eq!(q.remove(3), Some(TrackId(4)));
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current(), Some(TrackId(2)));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn removing_the_current_track_moves_to_the_next_one() {
        let mut q = playing(4, 1);
        assert_eq!(q.remove(1), Some(TrackId(2)));
        assert_eq!(ids(&q), vec![1, 3, 4]);
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn removing_the_current_last_track_moves_to_the_one_before() {
        let mut q = playing(4, 3);
        assert_eq!(q.remove(3), Some(TrackId(4)));
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(2));
    }

    #[test]
    fn removing_the_only_track_empties_the_queue() {
        let mut q = playing(1, 0);
        assert_eq!(q.remove(0), Some(TrackId(1)));
        assert!(q.is_empty());
        assert_eq!(q.current(), None);
        assert_eq!(q.current_index(), None);
    }

    #[test]
    fn remove_past_the_end_changes_nothing() {
        let mut q = playing(3, 1);
        assert_eq!(q.remove(3), None);
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn move_a_track_later() {
        let mut q = playing(5, 4);
        assert!(q.move_track(0, 2));
        assert_eq!(ids(&q), vec![2, 3, 1, 4, 5]);
        assert_eq!(q.current(), Some(TrackId(5)));
    }

    #[test]
    fn move_a_track_earlier() {
        let mut q = playing(5, 0);
        assert!(q.move_track(4, 1));
        assert_eq!(ids(&q), vec![1, 5, 2, 3, 4]);
        assert_eq!(q.current(), Some(TrackId(1)));
    }

    #[test]
    fn moving_the_current_track_keeps_it_current() {
        let mut q = playing(5, 1);
        assert!(q.move_track(1, 3));
        assert_eq!(ids(&q), vec![1, 3, 4, 2, 5]);
        assert_eq!(q.current(), Some(TrackId(2)));
        assert_eq!(q.current_index(), Some(3));
    }

    #[test]
    fn moving_a_track_from_before_to_after_the_current_one() {
        let mut q = playing(5, 2);
        assert!(q.move_track(0, 4));
        assert_eq!(ids(&q), vec![2, 3, 4, 5, 1]);
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn moving_a_track_from_after_to_before_the_current_one() {
        let mut q = playing(5, 2);
        assert!(q.move_track(4, 0));
        assert_eq!(ids(&q), vec![5, 1, 2, 3, 4]);
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.current_index(), Some(3));
    }

    #[test]
    fn moving_a_track_to_where_it_is_changes_nothing() {
        let mut q = playing(3, 1);
        assert!(q.move_track(2, 2));
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn move_past_the_end_changes_nothing() {
        let mut q = playing(3, 1);
        assert!(!q.move_track(3, 0));
        assert!(!q.move_track(0, 3));
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn clear_empties_the_queue_but_keeps_the_settings() {
        let mut q = playing(3, 1);
        q.set_repeat(Repeat::All);
        q.set_shuffle(true);
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.current(), None);
        assert_eq!(q.repeat(), Repeat::All);
        assert!(q.shuffle());
    }
}

mod repeat {
    use super::*;

    #[test]
    fn set_repeat_changes_only_the_setting() {
        let mut q = playing(3, 1);
        q.set_repeat(Repeat::One);
        assert_eq!(q.repeat(), Repeat::One);
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn repeat_all_goes_from_the_last_track_to_the_first() {
        let mut q = playing(3, 2);
        q.set_repeat(Repeat::All);
        assert_eq!(q.skip_next(), Some(TrackId(1)));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn repeat_all_goes_round_when_the_last_track_finishes() {
        let mut q = playing(3, 2);
        q.set_repeat(Repeat::All);
        assert_eq!(q.track_finished(), Some(TrackId(1)));
    }

    #[test]
    fn repeat_all_previous_on_the_first_track_goes_to_the_last() {
        let mut q = playing(3, 0);
        q.set_repeat(Repeat::All);
        assert_eq!(q.skip_previous(secs(1.0)), Some(Previous::GoTo(TrackId(3))));
        assert_eq!(q.current_index(), Some(2));
    }

    #[test]
    fn repeat_all_still_restarts_after_three_seconds() {
        let mut q = playing(3, 0);
        q.set_repeat(Repeat::All);
        assert_eq!(q.skip_previous(secs(10.0)), Some(Previous::RestartCurrent));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn repeat_all_with_one_track_plays_it_again() {
        let mut q = playing(1, 0);
        q.set_repeat(Repeat::All);
        assert_eq!(q.skip_next(), Some(TrackId(1)));
        assert_eq!(q.track_finished(), Some(TrackId(1)));
    }

    #[test]
    fn repeat_one_plays_a_finished_track_again() {
        let mut q = playing(3, 1);
        q.set_repeat(Repeat::One);
        assert_eq!(q.track_finished(), Some(TrackId(2)));
        assert_eq!(q.track_finished(), Some(TrackId(2)));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn repeat_one_still_lets_next_skip() {
        let mut q = playing(3, 1);
        q.set_repeat(Repeat::One);
        assert_eq!(q.skip_next(), Some(TrackId(3)));
        assert_eq!(q.skip_next(), Some(TrackId(1)));
    }

    #[test]
    fn repeat_one_previous_on_the_first_track_goes_to_the_last() {
        let mut q = playing(3, 0);
        q.set_repeat(Repeat::One);
        assert_eq!(q.skip_previous(secs(0.0)), Some(Previous::GoTo(TrackId(3))));
    }

    #[test]
    fn repeat_one_on_an_empty_queue_gives_nothing() {
        let mut q = Queue::with_seed(1);
        q.set_repeat(Repeat::One);
        assert_eq!(q.track_finished(), None);
        assert_eq!(q.skip_next(), None);
    }
}

mod shuffle {
    use super::*;

    /// The ids, sorted, so we can check a shuffle has the same tracks.
    fn sorted(mut v: Vec<i64>) -> Vec<i64> {
        v.sort();
        v
    }

    #[test]
    fn shuffle_on_puts_the_current_track_first() {
        let mut q = playing(20, 5);
        q.set_shuffle(true);
        assert!(q.shuffle());
        assert_eq!(q.current(), Some(TrackId(6)));
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.tracks()[0], TrackId(6));
    }

    #[test]
    fn shuffle_on_keeps_the_same_tracks_in_a_new_order() {
        let mut q = playing(20, 5);
        q.set_shuffle(true);
        let shuffled = ids(&q);
        assert_ne!(shuffled, (1..=20).collect::<Vec<_>>(), "nothing moved");
        assert_eq!(sorted(shuffled), (1..=20).collect::<Vec<_>>());
    }

    #[test]
    fn the_same_seed_gives_the_same_shuffle() {
        let mut a = Queue::with_seed(42);
        let mut b = Queue::with_seed(42);
        a.play_now(tracks(20), 0);
        b.play_now(tracks(20), 0);
        a.set_shuffle(true);
        b.set_shuffle(true);
        assert_eq!(ids(&a), ids(&b));
    }

    #[test]
    fn different_seeds_give_different_shuffles() {
        let mut a = Queue::with_seed(1);
        let mut b = Queue::with_seed(2);
        a.play_now(tracks(20), 0);
        b.play_now(tracks(20), 0);
        a.set_shuffle(true);
        b.set_shuffle(true);
        assert_ne!(ids(&a), ids(&b));
    }

    #[test]
    fn shuffle_off_goes_back_to_the_original_order() {
        let mut q = playing(20, 5);
        q.set_shuffle(true);
        q.skip_next();
        q.skip_next();
        let now_playing = q.current().unwrap();
        q.set_shuffle(false);
        assert!(!q.shuffle());
        assert_eq!(ids(&q), (1..=20).collect::<Vec<_>>());
        assert_eq!(q.current(), Some(now_playing));
        assert_eq!(q.current_index(), Some(now_playing.0 as usize - 1));
    }

    #[test]
    fn shuffle_on_twice_does_not_shuffle_again() {
        let mut q = playing(20, 0);
        q.set_shuffle(true);
        q.skip_next();
        let before = ids(&q);
        q.set_shuffle(true);
        assert_eq!(ids(&q), before);
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn shuffle_off_when_it_is_off_changes_nothing() {
        let mut q = playing(5, 3);
        q.set_shuffle(false);
        assert_eq!(ids(&q), vec![1, 2, 3, 4, 5]);
        assert_eq!(q.current_index(), Some(3));
    }

    #[test]
    fn shuffle_on_an_empty_queue_is_fine() {
        let mut q = Queue::with_seed(1);
        q.set_shuffle(true);
        assert!(q.is_empty());
        q.set_shuffle(false);
        assert!(q.is_empty());
    }

    #[test]
    fn play_now_while_shuffled_starts_with_the_chosen_track() {
        let mut q = Queue::with_seed(3);
        q.set_shuffle(true);
        q.play_now(tracks(20), 9);
        assert_eq!(q.current(), Some(TrackId(10)));
        assert_eq!(q.current_index(), Some(0));
        assert_ne!(ids(&q), (1..=20).collect::<Vec<_>>(), "nothing moved");
        assert_eq!(sorted(ids(&q)), (1..=20).collect::<Vec<_>>());

        q.set_shuffle(false);
        assert_eq!(ids(&q), (1..=20).collect::<Vec<_>>());
        assert_eq!(q.current_index(), Some(9));
    }

    #[test]
    fn add_next_while_shuffled_stays_next_after_shuffle_off() {
        let mut q = playing(10, 4);
        q.set_shuffle(true);
        q.add_next(TrackId(99));
        assert_eq!(q.tracks()[1], TrackId(99));
        assert_eq!(q.skip_next(), Some(TrackId(99)));

        q.set_shuffle(false);
        assert_eq!(ids(&q), vec![1, 2, 3, 4, 5, 99, 6, 7, 8, 9, 10]);
        assert_eq!(q.current(), Some(TrackId(99)));
    }

    #[test]
    fn add_to_end_while_shuffled_is_last_in_both_orders() {
        let mut q = playing(10, 0);
        q.set_shuffle(true);
        q.add_to_end(TrackId(99));
        assert_eq!(q.tracks().last(), Some(&TrackId(99)));

        q.set_shuffle(false);
        assert_eq!(ids(&q), vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 99]);
    }

    #[test]
    fn remove_while_shuffled_is_gone_after_shuffle_off() {
        let mut q = playing(10, 0);
        q.set_shuffle(true);
        let pos = q.tracks().iter().position(|&t| t == TrackId(7)).unwrap();
        assert_eq!(q.remove(pos), Some(TrackId(7)));

        q.set_shuffle(false);
        assert_eq!(ids(&q), vec![1, 2, 3, 4, 5, 6, 8, 9, 10]);
    }

    #[test]
    fn removing_one_copy_of_a_twice_queued_track_keeps_the_other() {
        let mut q = Queue::with_seed(5);
        q.play_now(vec![TrackId(1), TrackId(2), TrackId(1), TrackId(3)], 1);
        q.set_shuffle(true);
        let pos = q.tracks().iter().position(|&t| t == TrackId(1)).unwrap();
        q.remove(pos);

        q.set_shuffle(false);
        let left = ids(&q);
        assert_eq!(left.iter().filter(|&&t| t == 1).count(), 1);
        assert_eq!(sorted(left), vec![1, 2, 3]);
        assert_eq!(q.current(), Some(TrackId(2)));
    }

    #[test]
    fn move_while_shuffled_leaves_the_original_order_alone() {
        let mut q = playing(10, 0);
        q.set_shuffle(true);
        let before = ids(&q);
        assert!(q.move_track(9, 1));
        let mut expected = before.clone();
        let moved = expected.remove(9);
        expected.insert(1, moved);
        assert_eq!(ids(&q), expected);

        q.set_shuffle(false);
        assert_eq!(ids(&q), (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn repeat_all_while_shuffled_keeps_the_same_order() {
        let mut q = playing(5, 0);
        q.set_shuffle(true);
        q.set_repeat(Repeat::All);
        let order = ids(&q);
        for _ in 0..5 {
            q.skip_next();
        }
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(ids(&q), order);
    }
}
