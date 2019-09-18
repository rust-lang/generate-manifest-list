# Generates manifest lists from static.rust-lang.org

`cargo run` will query AWS credentials from the environment using `rusoto`'s default provider.

The `manifests.txt` will be put into the working directory which will have a list of
static.rust-lang.org "links" to all known manifests, sorted by date.

#### License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
