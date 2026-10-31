# Range requests need a positive bar count

`HistoricalClient` previously encoded every range request with `bar_count=0`,
even when the caller supplied a positive `num_bars`. Its argument-generation
tests explicitly required zero. Those tests verified the implementation's
assumption, rather than the live server's current behavior.

On October 31, 2026, anonymous requests to `DataServer::Data` rejected zero
with `critical_error`, `invalid parameters`, and the offending `create_series`
arguments. Requests with the same symbol, interval, and time range succeeded
when only the bar count changed to 512. This was reproduced for NASDAQ:AAPL,
NASDAQ:NDX, and TVC:GOLD.

## Reproduce independently of the fix

From the repository root:

```sh
cargo run -p tradingview-rs --no-default-features --features rustls-tls \
  --example reproduce_range_bar_count --locked
```

Optionally pass symbols after `--`, for example `-- NASDAQ:AAPL`.
The example constructs raw protocol arguments itself, bypassing
`build_create_series_args`. It therefore works both before and after the fix.
It fixes one three-year time range for all comparisons and changes only the
bar-count parameter within each pair, apart from unique session identifiers.
It does not send `request_more_data`.

The process fails on connection errors, timeouts, unrelated protocol errors,
zero-count success, or an insufficient positive-count result. Those outcomes
must not be presented as evidence that zero was rejected.

Observed results for `from=1698862582`, `to=1793470582`, interval `1D`:

| Symbol | Count 0 | Count 512 |
| --- | --- | --- |
| NASDAQ:AAPL | `critical_error: invalid parameters` | `series_completed`, 752 bars |
| NASDAQ:NDX | `critical_error: invalid parameters` | `series_completed`, 752 bars |
| TVC:GOLD | `critical_error: invalid parameters` | `series_completed`, 751 bars |

Representative raw error (the session identifier varies):

```text
CriticalError ["cs_1IvU7ASgtFon","invalid parameters","method: create_series. args: \"[sds_1, s1, symbol_1, 1D, 0, r,1698862582:1793470582]\""]
```

The positive result exceeds 512 bars, showing that this parameter does not
cap the returned data for these range requests. This is an observation for
the tested endpoint, anonymous access, symbols, and daily interval, not a
guarantee of unlimited history or identical behavior for every server.

## Fix

In range mode, preserve a requested positive `num_bars`, default to 512 when
omitted, and normalize an explicit zero to 1. Keep the seven-argument range
shape and the existing six-argument count mode unchanged. Update the old
zero-expecting unit tests and add coverage for explicit zero.

```sh
cargo test -p tradingview-rs --no-default-features --features rustls-tls \
  --lib historical::client::tests --locked
```

The 12 historical-client unit tests passed with the fix. The live experiment
is deliberately an opt-in example and adds no network dependency to unit tests.
