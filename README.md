# Experiment ownership of manages things

Doing something but **NOT** multiple threads yet.

```
wink@3900x 23-02-14T21:14:35.267Z:~/prgs/rust/myrepos/exper_ownership_of_managed_things (main)
$ cargo test -- --nocapture
    Finished test [unoptimized + debuginfo] target(s) in 0.00s
     Running unittests src/lib.rs (target/debug/deps/exper_ownership_of_managed_things-fe14290c80eef164)

running 1 test
main: thing1=Thing { name: "t1", channel: ThingChannel { tx: Sender { .. }, rx: Receiver { .. } }, counter: 0 }
main: new manager=Manager { things: [] }
main: t1_handle=0 manager=Manager { things: [TheThing(Thing { name: "t1", channel: ThingChannel { tx: Sender { .. }, rx: Receiver { .. } }, counter: 0 })] }
Thing::increment: counter=1
main: t1=Thing { name: "t1", channel: ThingChannel { tx: Sender { .. }, rx: Receiver { .. } }, counter: 1 }
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests exper_ownership_of_managed_things

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
