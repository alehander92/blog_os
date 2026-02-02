### toy

based on Blog OS by Philipp Oppermann: full credit to him and all other blog os maintainers/contributors!

Also credit to overall rust community and also to the osdev commmunity and for some friends/colleagues for help!

plan

* simple shell and "programs" tasks; for now based on the blog os's cooperative async/await support or just as functions
  eventually preemption/more advanced processes/userland? maybe in the future
* db-like/file abstraction: first in memory; eventually for support for hard disk as well
* simple chat program: some kind of remote support: in VM(by mapped memory or socket?) or real: TCP/IP (custom manual or library like [smoltcp](https://github.com/smoltcp-rs/smoltcp) used in [the Moros OS](https://moros.cc/))
* eventually: others

### shell:

```
user> pwd
os>
-------
@first

user> write groceries.text bread;fish
os> ok: file written
user> ls
os> ls
-------
| groceries | Text | 10 |
```

original README from blog-os: README of the branch after the 12th post:

# Blog OS (Async/Await)

[![Build Status](https://github.com/phil-opp/blog_os/workflows/Code/badge.svg?branch=post-12)](https://github.com/phil-opp/blog_os/actions?query=workflow%3A%22Code%22+branch%3Apost-12)

This repository contains the source code for the [Async/Await][post] post of the [Writing an OS in Rust](https://os.phil-opp.com) series.

[post]: https://os.phil-opp.com/async-await/

**Check out the [master branch](https://github.com/phil-opp/blog_os) for more information.**

## Building

This project requires a nightly version of Rust because it uses some unstable features. At least nightly _2020-07-15_ is required for building. You might need to run `rustup update nightly --force` to update to the latest nightly even if some components such as `rustfmt` are missing it.

You can build the project by running:

```
cargo build
```

To create a bootable disk image from the compiled kernel, you need to install the [`bootimage`] tool:

[`bootimage`]: https://github.com/rust-osdev/bootimage

```
cargo install bootimage
```

After installing, you can create the bootable disk image by running:

```
cargo bootimage
```

This creates a bootable disk image in the `target/x86_64-blog_os/debug` directory.

Please file an issue if you have any problems.

## Running

You can run the disk image in [QEMU] through:

[QEMU]: https://www.qemu.org/

```
cargo run
```

[QEMU] and the [`bootimage`] tool need to be installed for this.

You can also write the image to an USB stick for booting it on a real machine. On Linux, the command for this is:

```
dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

Where `sdX` is the device name of your USB stick. **Be careful** to choose the correct device name, because everything on that device is overwritten.

## Testing

To run the unit and integration tests, execute `cargo xtest`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Note that this only applies to this git branch, other branches might be licensed differently.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
