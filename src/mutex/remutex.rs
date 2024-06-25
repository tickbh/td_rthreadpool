// Copyright 2015 The Rust Project Developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use std::marker;
use std::ops::{Deref, DerefMut};
use std::cell::UnsafeCell;

use super::sys;
use super::poison::{self, TryLockError, TryLockResult, LockResult};

/// A re-entrant mutual exclusion
///
/// This mutex will block *other* threads waiting for the lock to become available. The thread
/// which has already locked the mutex can lock it multiple times without blocking, preventing a
/// common source of deadlocks.
pub struct ReentrantMutex<T: ?Sized> {
    inner: Box<sys::ReentrantMutex>,
    poison: poison::Flag,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send + ?Sized> Send for ReentrantMutex<T> {}
unsafe impl<T: Send + ?Sized> Sync for ReentrantMutex<T> {}

#[must_use]
pub struct ReentrantMutexGuard<'a, T: ?Sized + 'a> {
    __lock: &'a ReentrantMutex<T>,
    __poison: poison::Guard,
    __marker: marker::PhantomData<*mut ()>, // !Send
}

impl<T> ReentrantMutex<T> {
    /// Creates a new reentrant mutex in an unlocked state.
    pub fn new(t: T) -> ReentrantMutex<T> {
        unsafe {
            let mut mutex = ReentrantMutex {
                inner: Box::new(sys::ReentrantMutex::uninitialized()),
                poison: poison::Flag::new(),
                data: UnsafeCell::new(t),
            };
            mutex.inner.init();
            mutex
        }
    }

    /// Acquires a mutex, blocking the current thread until it is able to do so.
    ///
    /// This function will block the caller until it is available to acquire the mutex.
    /// Upon returning, the thread is the only thread with the mutex held. When the thread
    /// calling this method already holds the lock, the call shall succeed without
    /// blocking.
    ///
    /// # Failure
    ///
    /// If another user of this mutex panicked while holding the mutex, then
    /// this call will return failure if the mutex would otherwise be
    /// acquired.
    pub fn lock(&self) -> LockResult<ReentrantMutexGuard<T>> {
        unsafe { self.inner.lock() }
        ReentrantMutexGuard::new(&self)
    }

    /// Attempts to acquire this lock.
    ///
    /// If the lock could not be acquired at this time, then `Err` is returned.
    /// Otherwise, an RAII guard is returned.
    ///
    /// This function does not block.
    ///
    /// # Failure
    ///
    /// If another user of this mutex panicked while holding the mutex, then
    /// this call will return failure if the mutex would otherwise be
    /// acquired.
    pub fn try_lock(&self) -> TryLockResult<ReentrantMutexGuard<T>> {
        if unsafe { self.inner.try_lock() } {
            Ok(ReentrantMutexGuard::new(&self)?)
        } else {
            Err(TryLockError::WouldBlock)
        }
    }
}

impl<T: ?Sized> Drop for ReentrantMutex<T> {
    fn drop(&mut self) {
        unsafe { self.inner.destroy() }
    }
}

impl<T: fmt::Debug + 'static> fmt::Debug for ReentrantMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => write!(f, "ReentrantMutex {{ data: {:?} }}", &*guard),
            Err(TryLockError::Poisoned(err)) => {
                write!(f,
                       "ReentrantMutex {{ data: Poisoned({:?}) }}",
                       &**err.get_ref())
            }
            Err(TryLockError::WouldBlock) => write!(f, "ReentrantMutex {{ <locked> }}"),
        }
    }
}

impl<'mutex, T> ReentrantMutexGuard<'mutex, T> {
    fn new(lock: &'mutex ReentrantMutex<T>) -> LockResult<ReentrantMutexGuard<'mutex, T>> {
        poison::map_result(lock.poison.borrow(), |guard| {
            ReentrantMutexGuard {
                __lock: lock,
                __poison: guard,
                __marker: marker::PhantomData,
            }
        })
    }
}

impl<'mutex, T> Deref for ReentrantMutexGuard<'mutex, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.__lock.data.get() }
    }
}

impl<'mutex, T> DerefMut for ReentrantMutexGuard<'mutex, T> {
    // type Target = T;
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.__lock.data.get() }
    }
}

// impl<'mutex, T: ?Sized> DerefMut for MutexGuard<'mutex, T> {
//     fn deref_mut(&mut self) -> &mut T {
//         unsafe { &mut *self.__data.get() }
//     }
// }

impl<'a, T: ?Sized> Drop for ReentrantMutexGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.__lock.poison.done(&self.__poison);
            self.__lock.inner.unlock();
        }
    }
}


#[cfg(test)]
mod test {
    use super::ReentrantMutex;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn smoke() {
        let m = ReentrantMutex::new(());
        {
            let a = m.lock().unwrap();
            {
                let b = m.lock().unwrap();
                {
                    let c = m.lock().unwrap();
                    assert_eq!(*c, ());
                }
                assert_eq!(*b, ());
            }
            assert_eq!(*a, ());
        }
    }

    #[test]
    fn is_mutex() {


        let m = Arc::new(ReentrantMutex::new(0));
        let lock = m.lock().unwrap();
        {
            let mc = m.clone();
            let handle = thread::spawn(move || {
                let lock = mc.lock().unwrap();
                assert_eq!(*lock, 4950);
            });
            for i in 0..100 {
                let mut lock = m.lock().unwrap();
                *lock += i;
            }
            drop(lock);
            drop(handle);
        }
    }

    #[test]
    #[allow(unused_must_use)]
    fn trylock_works() {
        let m = Arc::new(ReentrantMutex::new(()));
        let _lock1 = m.try_lock().unwrap();
        let _lock2 = m.try_lock().unwrap();
        {
            let m = m.clone();
            thread::spawn(move || {
                let lock = m.try_lock();
                assert!(lock.is_err());
            })
                .join();
        }
        let _lock3 = m.try_lock().unwrap();
    }

}
