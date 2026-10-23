//! Reading the compiled scripts: which variables a trigger stores, reads and compares.

use hashbrown::HashSet;
use omsi_script::{BlockId, Op, Program, VarId};

/// Every op of `block` and the macros it calls, once each.
pub(super) fn each_op(p: &Program, block: BlockId, f: &mut dyn FnMut(&[Op], usize)) {
    let mut seen = HashSet::new();
    let mut todo = vec![block];
    while let Some(b) = todo.pop() {
        if !seen.insert(b) {
            continue;
        }
        let Some(blk) = p.blocks.get(b as usize) else {
            continue;
        };
        for (i, op) in blk.ops.iter().enumerate() {
            if let Op::Macro(m) = op {
                todo.push(*m);
            }
            f(&blk.ops, i);
        }
    }
}

/// The value a `Store` at `i` writes when it is a literal or constfile number.
pub(super) fn stored_constant(ops: &[Op], i: usize) -> Option<f32> {
    match ops[..i].iter().rev().find(|o| !matches!(o, Op::Store(_))) {
        Some(Op::Push(c)) | Some(Op::Const(c)) => Some(*c),
        _ => None,
    }
}

pub(super) fn stores(p: &Program, block: BlockId) -> (HashSet<VarId>, HashSet<u32>) {
    let (mut f, mut s) = (HashSet::new(), HashSet::new());
    each_op(p, block, &mut |ops, i| match &ops[i] {
        Op::Store(v) => {
            f.insert(*v);
        }
        Op::StoreStr(v) => {
            s.insert(*v);
        }
        _ => {}
    });
    (f, s)
}

pub(super) fn loads(p: &Program, block: BlockId) -> (HashSet<VarId>, HashSet<u32>) {
    let (mut f, mut s) = (HashSet::new(), HashSet::new());
    each_op(p, block, &mut |ops, i| match &ops[i] {
        Op::Load(v) => {
            f.insert(*v);
        }
        Op::LoadStr(v) => {
            s.insert(*v);
        }
        _ => {}
    });
    (f, s)
}

/// Constants `block` (with its macros) stores into `var`.
pub(super) fn constants_stored(p: &Program, block: BlockId, var: VarId) -> Vec<f32> {
    let mut out = Vec::new();
    each_op(p, block, &mut |ops, i| {
        if ops[i] == Op::Store(var) {
            if let Some(c) = stored_constant(ops, i) {
                out.push(c);
            }
        }
    });
    out
}

/// Values the block itself (not its macros) compares `var` with (`(L.L.var) c =`).
pub(super) fn gate_values(p: &Program, block: BlockId, var: VarId) -> Vec<f32> {
    let Some(b) = p.blocks.get(block as usize) else {
        return Vec::new();
    };
    b.ops
        .windows(3)
        .filter_map(|w| {
            if let (Op::Load(v), Op::Push(c), Op::Eq) = (&w[0], &w[1], &w[2]) {
                (*v == var).then_some(*c)
            } else {
                None
            }
        })
        .collect()
}

/// The (variables, string variables) a key stores or reads.
pub(super) type Sets = (HashSet<VarId>, HashSet<u32>);

/// A key that does not drive: its name says it cancels or clears.
pub(super) fn refused(n: &str) -> bool {
    [
        "cancel", "loesch", "lösch", "clear", "storno", "korr", "back", "rueck", "zurueck",
    ]
    .iter()
    .any(|w| n.contains(w))
}

/// Where a name sits in a list of words (the end of the list when none matches).
pub(super) fn rank(n: &str, words: &[&str]) -> usize {
    words
        .iter()
        .position(|w| n.contains(w))
        .unwrap_or(words.len())
}
