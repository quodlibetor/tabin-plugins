Tābin Plugins
=============

[![Build Status](https://travis-ci.org/quodlibetor/tabin-plugins.svg)](https://travis-ci.org/quodlibetor/tabin-plugins)

This crate contains some utilities for building nagios-compatible check
scripts, some utilities for reading and dealing with system information on
Linux, and several implementations of check scripts.

[Library docs](http://quodlibetor.github.io/tabin-plugins/doc/tabin_plugins/index.html)

See `src/bin` for the scripts that exist. You can see all their `--help`
messages in
[the script docs](http://quodlibetor.github.io/tabin-plugins/doc/tabin_plugins/scripts/index.html).

The utilities are pretty stable, but the linux system information should
probably be moved into something like
[procinfo-rs](https://github.com/danburkert/procinfo-rs), although the
implementation here appears to be more complete and type safe than any of the
other options out there.
