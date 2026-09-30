use expect_test::expect_file;
use tolk_compiler::{Compiler, CompilerResult};

#[test]
fn getter_prototypes_preserve_abi_without_generating_methods() {
    // Getter signatures from Tolk 1.5's abi-tests.tolk.
    let source = r"
struct CalcSwapCostReply {
    from: address
    to: address
    cost: coins
}

get fun calc_swap_cost(from: address): CalcSwapCostReply

/// @param precision number of decimal places
get fun pool_reserves(precision: uint8 = 9): (coins, coins);
";
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("pool.types.tolk");
    let compiler = Compiler::new()
        .with_allow_no_entrypoint(true)
        .with_allow_empty_get_fun(true)
        .with_source_overrides([(path.clone(), source)]);

    let errors = compiler.check(&path).expect("check ABI interface");
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");

    let CompilerResult::Success(result) = compiler.compile(&path, false) else {
        panic!("getter prototypes must compile");
    };
    let abi =
        serde_json::to_string_pretty(&result.abi.expect("interface ABI")).expect("serialize ABI");
    expect_file!["snapshots/getter_prototypes.json"].assert_eq(&abi);

    let CompilerResult::Success(without_getters) = compiler
        .with_source_overrides([(path.clone(), "")])
        .compile(&path, false)
    else {
        panic!("empty interface must compile");
    };
    assert_eq!(result.code_hash_hex, without_getters.code_hash_hex);
}
