These BoC fixtures were captured from `tvm-ffi` at Acton commit `917dfc0cf`,
before moving its stack models and codec to `rston::models::vm`.

They cover every existing stack value variant, a captured continuation stack,
both int257 limits, and a snake string split across two cells. Tests compare
the encoded bytes to preserve the existing format during the move.
