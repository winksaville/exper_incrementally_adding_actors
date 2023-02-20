# Experiment ownership of manages things

This "runs" but hangs and you have to Ctrl-C out because
we have not successfully added a receiver for the new actor
which is added to the ActorExecutor in test_conn_mgr.
```
wink@3900x 23-02-20T23:16:06.125Z:~/prgs/rust/myrepos/exper_ownership_of_managed_things (access_ae_from_ae_actor)
$ cargo test -- --nocapture
   Compiling exper_ownership_of_managed_things v0.1.0 (/home/wink/prgs/rust/myrepos/exper_ownership_of_managed_things)
    Finished test [unoptimized + debuginfo] target(s) in 0.25s
     Running unittests src/lib.rs (target/debug/deps/exper_ownership_of_managed_things-fe14290c80eef164)

running 1 test

test_conn_mgr:+
test_conn_mgr: executor1_tx=BiDirLocalChannel { self_tx: Sender { .. }, tx: Sender { .. }, rx: Receiver { .. } }
test_conn_mgr: thing1=Thing { name: "t1", counter: 0 }
recv_bdlc:+
AE:executor1:+
AE:executor1: TOL
ActorExecutor.prossess_msg_any: msg=MsgReqAeAddActor { actor: Thing { name: "t1", counter: 0 }, rsp_tx: Sender { .. } }
ActorExecutor.prossess_msg_any: added new receiver for t1
AE:executor1: TOL
recv_bdlc: msg_any=Any { .. }
recv_bdlc: msg=MsgRspAeAddActor { bdlc: BiDirLocalChannel { self_tx: Sender { .. }, tx: Sender { .. }, rx: Receiver { .. } } }
recv_bdlc:-
test_conn_mgr: thing_bdlc=BiDirLocalChannel { self_tx: Sender { .. }, tx: Sender { .. }, rx: Receiver { .. } }
test_conn_mgr: send MsgInc
test_conn_mgr: sent MsgInc
test_conn_mgr: send MsgReqCounter
test_conn_mgr: sent MsgReqCounter
test_conn_mgr: recv msg_any of MsgRspCounter
^C
```

If I uncomment line 195 in src/main.rs:
```
   192	                            // This is the key to making ActorExecutor work, we need to add a
   193	                            // new receiver for this Actor, but this causes two compile
   194	                            // error[E0502]'s above, i.e. immutable and mutable borrows :(
   195	                            selector.recv(x.our_channel.get_recv());
```
 I get two compile errors:
```
wink@3900x 23-02-20T23:16:03.581Z:~/prgs/rust/myrepos/exper_ownership_of_managed_things (access_ae_from_ae_actor)
$ cargo test -- --nocapture
   Compiling exper_ownership_of_managed_things v0.1.0 (/home/wink/prgs/rust/myrepos/exper_ownership_of_managed_things)
error[E0502]: cannot borrow `ae` as mutable because it is also borrowed as immutable
   --> src/lib.rs:167:64
    |
159 |                 let oper = selector.select();
    |                            ----------------- immutable borrow later used here
...
167 |                     if let Ok(msg_any) = oper.recv(rx).map_err(|why| {
    |                                                                ^^^^^ mutable borrow occurs here
...
170 |                         println!("AE:{}: error on recv: {why} `done = true`", ae.name());
    |                                                                               -- second borrow occurs due to use of `ae` in closure
171 |                         ae.done = true;
    |                         ------- capture is mutable because of use here
...
190 |                             let x = ae.bi_dir_channels_vec.get(actor_idx).unwrap();
    |                                     ------------------------------------- immutable borrow occurs here

error[E0502]: cannot borrow `ae.bi_dir_channels_vec` as mutable because it is also borrowed as immutable
   --> src/lib.rs:188:29
    |
188 | ...                   ae.bi_dir_channels_vec.push(bdlcs);
    |                       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ mutable borrow occurs here
189 | ...
190 | ...                   let x = ae.bi_dir_channels_vec.get(actor_idx).unwrap();
    |                               ------------------------------------- immutable borrow occurs here
...
195 | ...                   selector.recv(x.our_channel.get_recv());
    |                       --------------------------------------- immutable borrow later used here

For more information about this error, try `rustc --explain E0502`.
error: could not compile `exper_ownership_of_managed_things` due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `exper_ownership_of_managed_things` due to 2 previous errors
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
