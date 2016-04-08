#td_rthreadpool
==========

A thread pool for running a number of jobs on a fixed set of worker threads. And has the reenter mutex lock.

[![Build Status](https://travis-ci.org/tickbh/td_rthreadpool.svg?branch=master)](https://travis-ci.org/tickbh/td_rthreadpool)

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
td_rthreadpool = "0.1.0"
```

and this to your crate root:

```rust
extern crate td_rthreadpool;
```


## License

Licensed under either of

 * Apache License, Version 2.0
 * MIT license

at your option.
