# Experiment incrementally adding actors to an executor

In my model for a Rust implemenation of the actor model
actors need to communicate with each other. For that I
will use channels and each connection will be independent.
Further more, for efficiency, mulitple actors can share
a thread. To accomplish these goals I need a communication
mechanism where a single thread can "wait" on multiple
channels. This cannot be accomplished with the Rust
`std::sync::mpsc::channel`. Instead, I've chosen to use
`crossbeam_channel`s.

In particular I'm using
[`Select`](https://docs.rs/crossbeam/latest/crossbeam/channel/struct.Select.html)
which supports selecting a ready channel from a set of channels.

In all of the [`examples`](https://docs.rs/crossbeam/latest/crossbeam/channel/struct.Select.html)
I found the set of operations, `Receivers<_>` in this case
is created first and then added enmass to `Select` using the
`recv` method.

But I want to be able to add actors incrementally and potentially
move actors between threads. So initially I tried a simple solution,
create a `Vec<Receivers<BoxMsgAny>>` ne solution I found was to pre-create
all of the channels but that feels to limiting.
So it can accomplish what I need. But I have one other constraint,
I want to be able to add actors to `Select` incrementally.

But I ran into a problem,  in a crossbeam_channel Select uses
the index into a vector of references to receivers as
the "operation" handle. And when the Selector finds an
receiver that is "ready" it returns its "operation" (i.e. the index).
This by itself is fine, but the problem is you need an array of
channels and you give a reference to the `Selector` into that array.
That means the array of channels must be immutable.

This is a problem as I want to be able to incrementally add channels
which means the array of channels must be mutable. But then you run
into a compiler problem because there are multiple references into
a mutable array and it won't compile.

The solution I found is to use `UnsafeCell` in `struct VecBdlcs`.
I don't like using unsafe but it works for now.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
