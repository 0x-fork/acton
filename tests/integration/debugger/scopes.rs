use crate::support::debugger::debug::DebugBuilder;

#[test]
fn loop_transfers_close_nested_scopes() -> anyhow::Result<()> {
    let session = DebugBuilder::new("loop-scopes")
        .code(
            r"fun main(limit: int): int {
    val outer = limit + 100;
    var total = 0;
    var i = 0;
    while (i < limit) {
        val loopValue = i + 10;
        i += 1;
        val nested = loopValue + 20;
        if (i == 1) {
            val skipped = nested + 1;
            total += skipped;
            continue;
        }
        repeat (2) {
            val innerLoop = nested + 30;
            if (innerLoop > 0) {
                val breaking = innerLoop + 40;
                total += breaking;
                break;
            }
        }
        total += loopValue;
        if (i == 3) {
            break;
        }
        total += outer;
    }
    val afterLoop = total + outer;
    return afterLoop;
}",
        )
        .accept_int(4)
        .build();
    let mut client = session.start();
    let result = client.execute(|executor| executor.step_in_until_terminated(100))?;

    result.assert_trace_snapshot_matches(
        "integration/snapshots/debugger/scopes/loop_transfers.trace.txt",
    );
    Ok(())
}

#[test]
fn inline_returns_preserve_outer_scopes() -> anyhow::Result<()> {
    for value in [0, 1, 2] {
        let session = DebugBuilder::new("inline-scopes")
            .code(
                r"@inline
fun choose(value: int): int {
    val outer = value + 10;
    if (value > 0) {
        val nested = outer + 20;
        if (value == 1) {
            return nested;
        }
        val afterBranch = nested + outer;
        return afterBranch;
    }
    return outer;
}

fun main(value: int): int {
    val caller = value + 100;
    {
        val kept = caller + 200;
        val result = choose(value);
        val afterCall = kept + result;
        return afterCall;
    }
}",
            )
            .accept_int(value)
            .build();
        let mut client = session.start();
        let result = client.execute(|executor| executor.step_in_until_terminated(100))?;

        result.assert_trace_snapshot_matches(&format!(
            "integration/snapshots/debugger/scopes/inline_return_{value}.trace.txt"
        ));
    }
    Ok(())
}
