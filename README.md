Tābin
=====

[![Build Status](https://travis-ci.org/quodlibetor/tabin.svg)](https://travis-ci.org/quodlibetor/tabin)

Tabin (Japanese for turbine) is a project to play with Rust and write some
helpers for Nagios-style check scripts, and some scripts.

It's called Tabin because I'm mining the the
[sensu-plugins](https://github.com/sensu-plugins) repo for ideas for scripts to
build, because even though Ruby is great for writing things quickly when you
have a check-cpu script that spends a third of a second of CPU time to check
the cpu usage over the course of a second is silly.

I would like for all the features that are built to be generally useful for any
monitoring system, but for now I'm focused on actually implementing
Nagios-style scripts.
