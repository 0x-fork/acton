use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::localnet::{
    latest_masterchain_seqno, parse_address_balance, pretty_json_for_snapshot, response_payload,
    summarize_admin_response,
};
use crate::support::project::ProjectBuilder;
use serde_json::{Value, json};
use std::fs;

#[test]
fn localnet_state_dump_and_load_replace_live_state_and_clear_checkpoints() {
    let project = ProjectBuilder::new("localnet-state-transfer").build();
    let node = project
        .localnet()
        .args(["--no-mining", "--mine-empty-blocks"])
        .start();

    let first_mine = node.post_json("/acton_mine", &json!({}));
    let dumped_seqno = response_payload(&first_mine)["last_block_seqno"]
        .as_u64()
        .expect("mine response must expose last_block_seqno") as u32;
    let state_path = project.path().join("state.json");
    let state_path_arg = state_path.display().to_string();
    let port = node.port().to_string();
    let dump_output = project
        .acton()
        .args(["localnet", "state", "dump"])
        .arg(&state_path_arg)
        .arg("--port")
        .arg(&port)
        .run()
        .success();
    let state_json = fs::read(&state_path).expect("state command must write the JSON file");
    let state_document: Value =
        serde_json::from_slice(&state_json).expect("dumped state must be valid JSON");
    let checkpoint = node.post_json(
        "/acton_createCheckpoint",
        &json!({ "name": "cleared-on-load" }),
    );

    let target = "0:5555555555555555555555555555555555555555555555555555555555555555";
    let funded = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": target,
            "amount": 1_000_000_000u128,
        }),
    );
    let second_mine = node.post_json("/acton_mine", &json!({}));
    let mutated_seqno = response_payload(&second_mine)["last_block_seqno"]
        .as_u64()
        .expect("mine response must expose last_block_seqno") as u32;
    let target_after_mutation =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));

    let load_output = project
        .acton()
        .args(["localnet", "state", "load"])
        .arg(&state_path_arg)
        .arg("--port")
        .arg(&port)
        .run()
        .success();
    let loaded_seqno = latest_masterchain_seqno(&node);
    let target_after_load =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let checkpoints_after_load = node.get_json("/acton_listCheckpoints");

    let snapshot = json!({
        "dump": {
            "output": dump_output.get_stdout().trim(),
            "seqno": dumped_seqno,
            "state_head_seqno": state_document["globals"]["head_seqno"].as_u64(),
        },
        "mutate": {
            "checkpoint": summarize_admin_response(&checkpoint),
            "fund_ok": funded["ok"].as_bool(),
            "seqno": mutated_seqno,
            "balance": parse_address_balance(&target_after_mutation).to_string(),
        },
        "load": {
            "output": load_output.get_stdout().trim(),
            "seqno": loaded_seqno,
            "balance": parse_address_balance(&target_after_load).to_string(),
            "checkpoints": summarize_admin_response(&checkpoints_after_load),
        },
    });

    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_state_transfer.summary.json"),
    );

    node.stop();
}
