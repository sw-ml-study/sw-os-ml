The latency escape hatch, `docs/architecture.md` s.6: parameter-major
scheduling costs latency variance, because a session waits for its
layer's turn. A latency-contracted session buys its way out of the batch
at the price of a private read. Add the contract field, the escape, and
the measurement: what a session's wait is under parameter-major, what
the escape costs in extra reads, and where the break-even sits.
