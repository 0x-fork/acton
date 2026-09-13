use super::*;
use expect_test::expect;
use serde_json::json;
use std::path::Path;
use ton_executor::DEFAULT_CONFIG;
use ton_executor::message::RunTransactionResultSuccess;
use tycho_types::cell::Lazy;
use tycho_types::models::{
    Account, CurrencyCollection, IntMsgInfo, MsgInfo, OptionalAccount, OwnedMessage, SpecialFlags,
    StateInit, StdAddr,
};

const NOW: u32 = 1_780_000_000;
const SEED: [u8; 32] = [0x42; 32];

// Distinct counters make incorrect tick/tock dispatch and omitted predecessors
// observable in storage, independently of the retracer's reported status.
const CONTRACT: &str = r"
struct Storage {
    ticks: uint32
    tocks: uint32
    messages: uint32
}

fun onRunTickTock(isTock: bool) {
    var storage = Storage.fromCell(contract.getData());
    if (isTock) {
        storage.tocks += 1;
    } else {
        storage.ticks += 1;
    }
    contract.setData(storage.toCell());
}

fun onInternalMessage(_: InMessage) {
    var storage = Storage.fromCell(contract.getData());
    storage.messages += 1;
    contract.setData(storage.toCell());
}
";

fn fixture() -> anyhow::Result<(StdAddr, ShardAccount)> {
    let path = Path::new("/retrace-tests/tick-tock.tolk");
    let compiler = tolk_compiler::Compiler::new(2).with_source_overrides([(path, CONTRACT)]);
    let tolk_compiler::CompilerResult::Success(compiled) = compiler.compile(path, false) else {
        anyhow::bail!("Cannot compile tick-tock test contract");
    };
    let address = StdAddr::new(-1, HashBytes([0x11; 32]));
    let account = Account {
        address: address.clone().into(),
        storage_stat: Default::default(),
        last_trans_lt: 0,
        balance: CurrencyCollection::new(10_000_000_000),
        state: AccountState::Active(StateInit {
            // Preserve both flags during replay, even when running just tick or tock.
            special: Some(SpecialFlags {
                tick: true,
                tock: true,
            }),
            code: Some(Boc::decode_base64(compiled.code_boc64)?),
            data: Some(to_cell(&(0u32, 0u32, 0u32))),
            ..Default::default()
        }),
    };

    Ok((
        address,
        ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(account)))?,
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        },
    ))
}

// Produce reference transactions directly with the executor, without using
// retrace's transaction-kind dispatch or state reconstruction.
fn reference_transaction(
    address: &StdAddr,
    account: &ShardAccount,
    kind: Option<TickTock>,
    lt: u64,
) -> anyhow::Result<(Transaction, ShardAccount)> {
    let message = if kind.is_some() {
        String::new()
    } else {
        Boc::encode_base64(to_cell(&OwnedMessage {
            info: MsgInfo::Int(IntMsgInfo {
                src: StdAddr::new(-1, HashBytes([0x22; 32])).into(),
                dst: address.clone().into(),
                value: CurrencyCollection::new(1_000_000_000),
                ..Default::default()
            }),
            init: None,
            body: Cell::empty_cell().into(),
            layout: None,
        }))
    };
    let executor = Executor::new(
        ExecutorVerbosity::FullLocationStackVerbose,
        Some(DEFAULT_CONFIG),
    )?;
    let (result, _) = executor.run_transaction(
        &message,
        &RunTransactionArgs {
            shard_account: Boc::encode_base64(to_cell(account)),
            now: NOW,
            lt,
            random_seed: Some(SEED),
            is_tick_tock: kind.map(|_| true),
            is_tock: kind.map(|kind| kind == TickTock::Tock),
            ..Default::default()
        },
    )?;
    let result = success(result)?;

    Ok((
        Boc::decode_base64(result.transaction.as_ref())?.parse()?,
        Boc::decode_base64(result.shard_account.as_ref())?.parse()?,
    ))
}

fn success(result: EmulationResult) -> anyhow::Result<RunTransactionResultSuccess> {
    match result {
        EmulationResult::Success(result) => Ok(result),
        EmulationResult::Error(error) => anyhow::bail!("Emulation failed: {}", error.error),
    }
}

fn counters(account: &ShardAccount) -> anyhow::Result<(u32, u32, u32)> {
    let account = account.load_account()?.context("Missing account")?;
    let AccountState::Active(state) = account.state else {
        anyhow::bail!("Account is not active");
    };
    Ok(state.data.context("Missing storage")?.parse()?)
}

#[test]
fn replay_tick_and_tock_targets() -> anyhow::Result<()> {
    let (address, account) = fixture()?;
    let balance = account.load_account()?.unwrap().balance.tokens;
    let mut results = Vec::new();

    for kind in [TickTock::Tick, TickTock::Tock] {
        let (original, expected_account) =
            reference_transaction(&address, &account, Some(kind), 1_000_000)?;
        let (replayed, _) = emulate(&original, DEFAULT_CONFIG, &account, None, SEED)?;
        let replayed = success(replayed)?;
        let (sender, contract, amount, money, transaction, compute) =
            compute_final_data(&replayed, balance, &address)?;
        let replayed_account: ShardAccount =
            Boc::decode_base64(replayed.shard_account.as_ref())?.parse()?;
        let (actions, _) = find_final_actions(&replayed);

        results.push(json!({
            "kind": format!("{kind:?}"),
            "sender": sender.map(|value| value.to_string()),
            "contract": contract.to_string(),
            "amount": amount.map(u128::from),
            "opcode": tx_opcode(&transaction),
            "in_message": transaction.in_msg.is_some(),
            "compute": compute,
            "sent_total": money.sent_total,
            "actions": actions.len(),
            "balance_after_matches": money.balance_after == u128::from(expected_account.load_account()?.unwrap().balance.tokens) as u64,
            "state_hash_matches": transaction.state_update.load()?.new == original.state_update.load()?.new,
            "account_matches": replayed_account == expected_account,
            "storage": counters(&replayed_account)?,
        }));
    }

    expect![[r#"
        [
          {
            "account_matches": true,
            "actions": 0,
            "amount": null,
            "balance_after_matches": true,
            "compute": {
              "exitCode": 0,
              "gasFees": 13200000,
              "gasUsed": 1320,
              "success": true,
              "type": "success",
              "vmSteps": 25
            },
            "contract": "-1:1111111111111111111111111111111111111111111111111111111111111111",
            "in_message": false,
            "kind": "Tick",
            "opcode": null,
            "sender": null,
            "sent_total": 0,
            "state_hash_matches": true,
            "storage": [
              1,
              0,
              0
            ]
          },
          {
            "account_matches": true,
            "actions": 0,
            "amount": null,
            "balance_after_matches": true,
            "compute": {
              "exitCode": 0,
              "gasFees": 12840000,
              "gasUsed": 1284,
              "success": true,
              "type": "success",
              "vmSteps": 23
            },
            "contract": "-1:1111111111111111111111111111111111111111111111111111111111111111",
            "in_message": false,
            "kind": "Tock",
            "opcode": null,
            "sender": null,
            "sent_total": 0,
            "state_hash_matches": true,
            "storage": [
              0,
              1,
              0
            ]
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&results)?);
    Ok(())
}

#[test]
fn replay_tick_tock_predecessors_before_ordinary_target() -> anyhow::Result<()> {
    let (address, account) = fixture()?;
    let balance = account.load_account()?.unwrap().balance.tokens;
    let (tick, after_tick) =
        reference_transaction(&address, &account, Some(TickTock::Tick), 1_000_000)?;
    let (tock, after_tock) =
        reference_transaction(&address, &after_tick, Some(TickTock::Tock), 2_000_000)?;
    let (target, expected_account) = reference_transaction(&address, &after_tock, None, 3_000_000)?;

    let (balance_before, before_target) = emulate_previous_transactions(
        &vec![tick, tock],
        &account,
        &balance,
        None,
        DEFAULT_CONFIG,
        SEED,
    )?;
    let (replayed, _) = emulate(&target, DEFAULT_CONFIG, &before_target, None, SEED)?;
    let replayed = success(replayed)?;
    let (sender, contract, amount, _, transaction, compute) =
        compute_final_data(&replayed, balance_before, &address)?;
    let replayed_account: ShardAccount =
        Boc::decode_base64(replayed.shard_account.as_ref())?.parse()?;

    expect![[r#"
        {
          "account_matches": true,
          "amount": 1000000000,
          "compute": {
            "exitCode": 0,
            "gasFees": 12770000,
            "gasUsed": 1277,
            "success": true,
            "type": "success",
            "vmSteps": 21
          },
          "contract": "-1:1111111111111111111111111111111111111111111111111111111111111111",
          "predecessors_match": true,
          "sender": "-1:2222222222222222222222222222222222222222222222222222222222222222",
          "state_hash_matches": true,
          "storage_after": [
            1,
            1,
            1
          ],
          "storage_before": [
            1,
            1,
            0
          ]
        }"#]].assert_eq(&serde_json::to_string_pretty(&json!({
        "predecessors_match": before_target == after_tock,
        "storage_before": counters(&before_target)?,
        "storage_after": counters(&replayed_account)?,
        "state_hash_matches": transaction.state_update.load()?.new == target.state_update.load()?.new,
        "account_matches": replayed_account == expected_account,
        "sender": sender.map(|value| value.to_string()),
        "contract": contract.to_string(),
        "amount": amount.map(u128::from),
        "compute": compute,
    }))?);
    Ok(())
}

#[test]
fn ordinary_transaction_still_requires_incoming_message() -> anyhow::Result<()> {
    let (address, account) = fixture()?;
    let (mut transaction, _) = reference_transaction(&address, &account, None, 1_000_000)?;
    transaction.in_msg = None;

    let error = emulate(&transaction, DEFAULT_CONFIG, &account, None, SEED).unwrap_err();
    expect![["No in_message was found in transaction"]].assert_eq(&error.to_string());
    Ok(())
}
