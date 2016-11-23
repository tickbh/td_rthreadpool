extern crate td_rthreadpool;
use td_rthreadpool::ThreadPool;
use std::thread::{self, sleep};
use std::sync;
use std::time::Duration;

const TEST_TASKS: usize = 4;
#[test]
fn join_all() {
    let pool = ThreadPool::new(4);

    let (tx_, rx) = sync::mpsc::channel();

    let tx = tx_.clone();
    pool.execute(move || {
        sleep(Duration::from_millis(1000));
        tx.send(2).unwrap();
    });

    let tx = tx_.clone();
    pool.execute(move || {
        tx.send(1).unwrap();
    });

    pool.join_all();

    let tx = tx_.clone();
    pool.execute(move || {
        tx.send(3).unwrap();
    });


    assert_eq!(rx.iter().take(3).collect::<Vec<_>>(), vec![1, 2, 3]);
}

#[test]
fn join_all_with_thread_panic() {
    let mut pool = ThreadPool::new(TEST_TASKS);
    for _ in 0..4 {
        pool.execute(move || {
            panic!();
        });
    }

    sleep(Duration::from_millis(1000));

    let active_count = pool.active_count();
    assert_eq!(active_count, TEST_TASKS);
    let initialized_count = pool.max_count();
    assert_eq!(initialized_count, TEST_TASKS);
}

#[test]
fn test_set_threads_decreasing() {
    let new_thread_amount = 2;
    let mut pool = ThreadPool::new(TEST_TASKS);
    for _ in 0..TEST_TASKS {
        pool.execute(move || {
            1 + 1;
        });
    }
    pool.set_threads(new_thread_amount);
    for _ in 0..new_thread_amount {
        pool.execute(move || {
            loop {
                sleep(Duration::from_millis(10000));
            }
        });
    }
    sleep(Duration::from_millis(1000));
    assert_eq!(pool.active_count(), new_thread_amount);
}

#[test]
fn test_active_count() {
    let pool = ThreadPool::new(TEST_TASKS);
    for _ in 0..TEST_TASKS {
        pool.execute(move|| {
            loop {
                sleep(Duration::from_millis(10000));
            }
        });
    }
    sleep(Duration::from_millis(1000));
    let active_count = pool.active_count();
    assert_eq!(active_count, TEST_TASKS);
    let initialized_count = pool.max_count();
    assert_eq!(initialized_count, TEST_TASKS);
}
