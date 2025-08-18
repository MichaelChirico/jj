// Copyright 2025 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use itertools::Itertools as _;
use jj_lib::object_id::ObjectId as _;
use jj_lib::operation::Operation;

use crate::cli_util::CommandHelper;
use crate::command_error::CommandError;
use crate::command_error::user_error;
use crate::commands::operation::DEFAULT_REVERT_WHAT;
use crate::commands::operation::RevertWhatToRestore;
use crate::commands::operation::revert::OperationRevertArgs;
use crate::commands::operation::revert::cmd_op_revert_with_tx_description;
use crate::commands::undo::UNDO_OP_DESC_PREFIX;
use crate::ui::Ui;

/// Redo the most recently undone operation
///
/// This is the natural counterpart of `jj undo`.
#[derive(clap::Args, Clone, Debug)]
pub struct RedoArgs {
    /// What portions of the local state to restore (can be repeated)
    ///
    /// This option is EXPERIMENTAL.
    #[arg(long, value_enum, default_values_t = DEFAULT_REVERT_WHAT)]
    what: Vec<RevertWhatToRestore>,
}

const REDO_OP_DESC_PREFIX: &str = "redo operation ";

fn tx_description(op: &Operation) -> String {
    format!("{REDO_OP_DESC_PREFIX}{}", op.id().hex())
}

pub fn cmd_redo(ui: &mut Ui, command: &CommandHelper, args: &RedoArgs) -> Result<(), CommandError> {
    let workspace_command = command.workspace_helper(ui)?;

    let mut op_to_redo = workspace_command.resolve_single_op("@")?;

    // Growing the "redo-stack" works very similar to the
    // [undo-stack](./undo.rs). `jj redo` and `jj undo` track their stacks
    // separately, but redo follows the pointers of undo-operations to determine
    // what operation to redo next.
    //
    // - If the operation to redo is a regular one (neither an undo- or
    //   redo-operation): Fail, because there is nothing to redo.
    // - If the operation to redo is an undo-operation, redo it (by reverting that
    //   undo-operation).
    // - If the operation to redo is a redo-operation itself, try to redo the parent
    //   of the operation that was already redone.
    // - Repeat the process of following redo-operations to the operations they
    //   redid until the first non-redo-operation is found.
    //
    // This described behavior leads to "jumping over" old redo-stacks if the
    // current one grows into it. Consider the following op-log example, where
    // redo-stacks are shown on the left and undo-stacks on the right:
    //
    // +--- "redo C" * H
    // |             |
    // | +- "redo F" * G
    // | |           |
    // | +------->   * F "undo E" -+
    // |             |             |
    // | +- "redo D" * E   <-------+
    // | |           |
    // | +------->   * D "undo A" ---+
    // |             |               |
    // +--------->   * C "undo B" -+ |
    //               |             | |
    //               * B   <-------+ |
    //               |               |
    //               * A   <---------+
    //
    // The interesting operation is the last one (H). It's a redo-operation and
    // it went like this:
    // - Attempt to redo G.
    // - G is a redo-operation of F, so attempt to redo its parent E next.
    // - E is also a redo-operation. Attempt to redo the parent of D, which is C.
    // - C is an undo-operation. Redo it.
    //
    while let Some(id_of_redone_op) = op_to_redo
        .metadata()
        .description
        .strip_prefix(REDO_OP_DESC_PREFIX)
    {
        let redone_op = workspace_command.resolve_single_op(id_of_redone_op)?;
        op_to_redo = match redone_op.parents().at_most_one().ok().flatten() {
            Some(parent_of_undone_op) => parent_of_undone_op?,
            None => return Err(user_error("Nothing to redo.")),
        };
    }

    if !op_to_redo
        .metadata()
        .description
        .starts_with(UNDO_OP_DESC_PREFIX)
    {
        // cannot redo a non-undo-operation
        return Err(user_error("Nothing to redo."));
    }

    let args = OperationRevertArgs {
        operation: op_to_redo.id().to_string(),
        what: args.what.clone(),
    };
    cmd_op_revert_with_tx_description(ui, command, &args, tx_description)?;

    Ok(())
}
