use std::time::Duration;

use acton_abi_catalog::find_abis_by_opcode;
use anyhow::{Context, Result};
use clap::Args;
use reqwest::blocking::Client;
use serde::Deserialize;

const DEFAULT_URL: &str = "https://api.actonscan.com/api/v1/stats/opcodes";
const HTTP_TIMEOUT_SECS: u64 = 20;
const MAX_OPCODE_LIMIT: u64 = 1_000;

#[derive(Args)]
pub(crate) struct ActonscanOpcodesArgs {
    #[arg(long, default_value = DEFAULT_URL)]
    url: String,
    #[arg(long, default_value_t = 2)]
    min_messages: u64,
}

#[derive(Deserialize)]
struct OpcodeSnapshot {
    first_masterchain_seqno: Option<u32>,
    latest_masterchain_seqno: Option<u32>,
    total_messages: u64,
    messages_with_opcode: u64,
    matching_opcodes: u64,
    opcodes: Vec<OpcodeCount>,
}

#[derive(Deserialize)]
struct OpcodeCount {
    opcode: u32,
    messages: u64,
    example_transaction_hashes: Vec<String>,
}

pub(crate) fn run(args: ActonscanOpcodesArgs) -> Result<()> {
    let snapshot = fetch_snapshot(&args)?;
    let first_seqno = snapshot
        .first_masterchain_seqno
        .map_or_else(|| "-".to_owned(), |seqno| seqno.to_string());
    let latest_seqno = snapshot
        .latest_masterchain_seqno
        .map_or_else(|| "-".to_owned(), |seqno| seqno.to_string());

    println!("masterchain: {first_seqno}..{latest_seqno}");
    println!("total messages: {}", snapshot.total_messages);
    println!("messages with opcode: {}", snapshot.messages_with_opcode);
    println!();
    println!("{:<10} {:>12}  catalog contracts", "opcode", "messages");

    let mut known_opcodes = 0_u64;
    let mut known_messages = 0_u64;

    for entry in snapshot.opcodes.iter().filter(|entry| entry.opcode != 0) {
        let mut contracts = find_abis_by_opcode(entry.opcode)
            .into_iter()
            .map(|abi| abi.contract_name.clone())
            .collect::<Vec<_>>();
        contracts.sort_unstable();
        contracts.dedup();
        let unknown = contracts.is_empty();

        let catalog_match = match contracts.split_first() {
            None => "unknown".to_owned(),
            Some((first, rest)) => {
                known_opcodes = known_opcodes.saturating_add(1);
                known_messages = known_messages.saturating_add(entry.messages);
                if rest.is_empty() {
                    first.clone()
                } else {
                    format!("{first} (+{} more)", rest.len())
                }
            }
        };

        println!(
            "0x{:08x} {:>12}  {catalog_match}",
            entry.opcode, entry.messages
        );
        if unknown {
            for hash in &entry.example_transaction_hashes {
                println!("{:>24}  transaction {hash}", "");
            }
        }
    }

    let returned_opcodes = u64::try_from(snapshot.opcodes.len()).unwrap_or(u64::MAX);
    let shown_opcodes = snapshot
        .opcodes
        .iter()
        .filter(|entry| entry.opcode != 0)
        .count()
        .try_into()
        .unwrap_or(u64::MAX);
    let shown_messages = snapshot
        .opcodes
        .iter()
        .filter(|entry| entry.opcode != 0)
        .map(|entry| entry.messages)
        .sum::<u64>();

    println!();
    println!(
        "shown opcodes: {shown_opcodes} ({known_opcodes} known, {} unknown)",
        shown_opcodes.saturating_sub(known_opcodes)
    );
    println!(
        "shown messages: {shown_messages} ({known_messages} known, {} unknown)",
        shown_messages.saturating_sub(known_messages)
    );

    if returned_opcodes < snapshot.matching_opcodes {
        eprintln!(
            "warning: the API returned {returned_opcodes} of {} matching opcodes",
            snapshot.matching_opcodes
        );
    }

    Ok(())
}

fn fetch_snapshot(args: &ActonscanOpcodesArgs) -> Result<OpcodeSnapshot> {
    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .context("failed to create HTTP client")?;

    client
        .get(&args.url)
        .query(&[
            ("limit", MAX_OPCODE_LIMIT),
            ("min_messages", args.min_messages.max(1)),
        ])
        .send()
        .with_context(|| {
            format!(
                "failed to request Actonscan opcode statistics: {}",
                args.url
            )
        })?
        .error_for_status()
        .context("Actonscan opcode statistics request failed")?
        .json()
        .context("failed to parse Actonscan opcode statistics")
}
