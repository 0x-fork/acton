use expect_test::expect_file;
use rustc_hash::FxHashSet;
use std::fmt::Write;
use std::path::PathBuf;
use tolk_dataflow::{
    ControlFlowGraph, DataflowAnalysis, Direction, NodeId, build_cfg_for_top_level_with_source,
    solve,
};
use tolk_resolver::resolve_index::{FileResolveIndex, LocalDefId};
use tolk_resolver::{FileDb, ProjectIndex, resolve};

// Cases adapted from TON's tolk-tester/tests/break-continue-tests.tolk.
#[test]
fn repeat_count_and_nested_loop_targets() {
    check(
        r"fun main(count: int, flag: int) {
            repeat (count += 1) {
                while (flag > 0) {
                    match (flag) {
                        1 => { flag -= 1; continue; }
                        else => break,
                    }
                }
                if (flag == 0) { continue; }
                break;
            }
            debug.print(count);
        }",
        expect_file!["snapshots/repeat_count_and_nested_loop_targets.txt"],
    );
}

#[test]
fn continue_reaches_do_condition_and_break_skips_it() {
    check(
        r"fun main(flag: int) {
            var x: int;
            do {
                if (flag == 1) { x = 4; continue; }
                if (flag == 2) { x = 7; break; }
                x = 9;
            } while (x < 0);
            return x;
        }",
        expect_file!["snapshots/continue_reaches_do_condition_and_break_skips_it.txt"],
    );
}

#[test]
fn only_reachable_breaks_leave_infinite_loop() {
    check(
        r"fun main(flag: int) {
            var x: int;
            while (true) {
                if (flag == 1) { x = 4; break; }
                if (flag == 2) { x = 7; break; }
                throw 100;
                x = flag;
                break;
            }
            return x;
        }",
        expect_file!["snapshots/only_reachable_breaks_leave_infinite_loop.txt"],
    );
}

#[test]
fn match_expression_break_skips_assignment() {
    check(
        r"fun main(flag: int, value: int) {
            var result = 10;
            while (true) {
                result = match (flag) {
                    0 => { break; value / 2 }
                    else => value
                };
                break;
            }
            return result;
        }",
        expect_file!["snapshots/match_expression_break_skips_assignment.txt"],
    );
}

#[test]
fn infinite_loop_keeps_live_reads_and_excludes_dead_suffix() {
    check(
        r"fun main(flag: int, value: int) {
            while (true) {
                debug.print(value);
                if (false) { break; }
                continue;
                value = flag;
            }
            return flag;
        }",
        expect_file!["snapshots/infinite_loop_keeps_live_reads_and_excludes_dead_suffix.txt"],
    );
}

fn check(source: &str, expected: expect_test::ExpectFile) {
    let directory = tempfile::tempdir().expect("create temporary project");
    let path = directory.path().join("main.tolk");
    std::fs::write(&path, source).expect("write source");
    let file_db = FileDb::new(PathBuf::from("/__dummy_stdlib__"), None);
    let mut project = ProjectIndex::builder(&file_db, path.clone())
        .build()
        .expect("index project");
    resolve(&file_db, &mut project);
    let canonical = file_db
        .canonicalize(&path)
        .expect("canonicalize source path");
    let file_id = project.get_file_by_path(&canonical).expect("indexed file");
    let file = file_db.get_by_id(file_id).expect("source file");
    assert!(!file.source().has_errors());
    let declaration = file
        .source()
        .top_levels()
        .next()
        .expect("function declaration");
    let index = project.get_resolved_uses(file_id).expect("resolved locals");
    let cfg = build_cfg_for_top_level_with_source(&declaration, index, Some(source))
        .expect("function CFG");
    let liveness = solve(&cfg, &LiveVariables);
    assert!(liveness.converged);

    let mut snapshot = String::new();
    for node in cfg.nodes() {
        let text = node.span.map_or("", |span| {
            source[span.start()..span.end()]
                .lines()
                .next()
                .unwrap_or("")
                .trim()
        });
        writeln!(
            snapshot,
            "{} {:?} {} {:?} R:[{}] W:[{}] live:[{}]",
            node.id.index(),
            node.kind,
            if liveness.is_reachable(node.id) {
                "reachable"
            } else {
                "dead"
            },
            text,
            names(&node.reads, index),
            names(&node.writes, index),
            names(liveness.in_at(node.id), index),
        )
        .expect("write snapshot");
        for edge in cfg.successors(node.id) {
            writeln!(snapshot, "  {:?} -> {}", edge.kind, edge.to.index()).expect("write snapshot");
        }
    }
    expected.assert_eq(&snapshot);
}

fn names(locals: &FxHashSet<LocalDefId>, index: &FileResolveIndex) -> String {
    let mut names = locals
        .iter()
        .map(|id| {
            index
                .locals
                .iter()
                .find(|local| local.id == *id)
                .expect("local in resolver index")
                .name
                .as_ref()
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.join(", ")
}

struct LiveVariables;

impl DataflowAnalysis for LiveVariables {
    type State = FxHashSet<LocalDefId>;

    fn direction(&self) -> Direction {
        Direction::Backward
    }

    fn bottom(&self, _cfg: &ControlFlowGraph) -> Self::State {
        FxHashSet::default()
    }

    fn boundary(&self, _cfg: &ControlFlowGraph) -> Self::State {
        FxHashSet::default()
    }

    fn merge(&self, into: &mut Self::State, other: &Self::State) -> bool {
        let previous_len = into.len();
        into.extend(other);
        into.len() != previous_len
    }

    fn transfer(&self, cfg: &ControlFlowGraph, node: NodeId, state: &Self::State) -> Self::State {
        let node = cfg.node(node);
        state
            .difference(&node.writes)
            .chain(&node.reads)
            .copied()
            .collect()
    }
}
