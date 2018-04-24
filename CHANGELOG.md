# Tābin Plugins Change Log

# 0.3.1

## CLI Changes

* `check-disk` now behaves more like `df` and doesn't error out for lack of
  permissions. ([#15](https://github.com/quodlibetor/tabin-plugins/pull/15))

# 0.3.0

## CLI Changes

* Better error messages and fewer unrecoverable errors throughout
* `check-graphite`
  * Add a `--window-start` parameter that allows you to start the scan some
    time in the past. Mostly useful for verifying that your newly-written
    assertion would have caught a past problem.
  * Null points no longer count toward assertions, so saying `all points are
    ..` is much more likely to trigger in the face of data that is slightly
    laggy.
* `check-procs`
  * Add the ability to filter processes to a specific state (e.g. `zombie`).
  * Add the ability to kill matching processes or their parents

## Library Changes

* More explicit enums/newtypes, instead of bare ints
* Better error handling, better result types.
