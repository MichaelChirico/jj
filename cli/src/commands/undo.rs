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

use clap_complete::ArgValueCandidates;
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
use crate::complete;
use crate::ui::Ui;

/// Undo the last operation
///
/// This undoes the last operation by applying its inverse as a new operation.
#[derive(clap::Args, Clone, Debug)]
pub struct UndoArgs {
    /// (deprecated, use `jj op revert <operation>`)
    ///
    /// The operation to undo
    ///
    /// Use `jj op log` to find an operation to undo.
    // TODO: Delete in jj 0.39+
    #[arg(default_value = "@", add = ArgValueCandidates::new(complete::operations))]
    operation: String,

    /// What portions of the local state to restore (can be repeated)
    ///
    /// This option is EXPERIMENTAL.
    #[arg(long, value_enum, default_values_t = DEFAULT_REVERT_WHAT)]
    what: Vec<RevertWhatToRestore>,
}

const UNDO_OP_DESC_PREFIX: &str = "undo operation ";

fn tx_description(op: &Operation) -> String {
    format!("{UNDO_OP_DESC_PREFIX}{}", op.id().hex())
}

pub fn cmd_undo(ui: &mut Ui, command: &CommandHelper, args: &UndoArgs) -> Result<(), CommandError> {
    if args.operation != "@" {
        writeln!(
            ui.warning_default(),
            "`jj undo <operation>` is deprecated; use `jj op revert <operation>` instead"
        )?;
        let args = OperationRevertArgs {
            operation: args.operation.clone(),
            what: args.what.clone(),
        };
        return cmd_op_revert_with_tx_description(ui, command, &args, tx_description);
    }

    let workspace_command = command.workspace_helper(ui)?;

    let mut op_to_undo = workspace_command.resolve_single_op(&args.operation)?;

    // Growing the "undo-stack" works like this:
    // - If the operation to undo is a regular one (not an undo-operation), simply
    //   undo it.
    // - If the operation to undo is an undo-operation itself, try to undo the
    //   parent of the operation that was already undone.
    // - Repeat the process of following undo-operations to the operations they
    //   undid until the first undoable operation is found - then undo it.
    //
    // This described behavior leads to "jumping over" old undo-stacks if the
    // current one grows into it. For example, Consider the this op-log example:
    //
    // * F "undo A" ---+
    // |               |
    // * E "undo D" -+ |
    // |             | |
    // * D   <-------+ |
    // |               |
    // * C "undo B" -+ |
    // |             | |
    // * B   <-------+ |
    // |               |
    // * A   <---------+
    //
    // It was produced by the following sequence of events:
    // - do normal operation A
    // - do normal operation B
    // - undo B
    // - do normal operation D
    // - undo D
    // - undo A
    //
    // Notice that running `undo` after having undone D leads to A being undone
    // (as opposed to C). The undo-stack spanning B and C was "jumped over".
    //
    while let Some(id_of_undone_op) = op_to_undo
        .metadata()
        .description
        .strip_prefix(UNDO_OP_DESC_PREFIX)
    {
        let undone_op = workspace_command.resolve_single_op(id_of_undone_op)?;
        op_to_undo = match undone_op.parents().at_most_one() {
            Ok(Some(parent_of_undone_op)) => parent_of_undone_op?,
            Ok(None) => return Err(user_error("Cannot undo root operation")),
            Err(_) => return Err(user_error("Cannot undo a merge operation")),
        };
    }

    let args = OperationRevertArgs {
        operation: op_to_undo.id().to_string(),
        what: args.what.clone(),
    };
    cmd_op_revert_with_tx_description(ui, command, &args, tx_description)?;

    Ok(())
}
