// Copyright 2022 The Jujutsu Authors
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

use crate::common::TestEnvironment;

#[test]
fn test_undo_root_operation() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    let output = work_dir.run_jj(["undo"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Reverted operation: 8f47435a3990 (2001-02-03 08:05:07) add workspace 'default'
    [EOF]
    ");

    let output = work_dir.run_jj(["undo"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Error: Cannot revert root operation
    [EOF]
    [exit status: 1]
    ");
}

#[test]
fn test_undo_merge_operation() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.run_jj(["new"]).success();
    work_dir.run_jj(["new", "--at-op=@-"]).success();
    let output = work_dir.run_jj(["undo"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Concurrent modification detected, resolving automatically.
    Error: Cannot revert a merge operation
    [EOF]
    [exit status: 1]
    ");
}

#[test]
fn test_undo_jump_old_undo_stack() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    let snapshot = || work_dir.run_jj(["describe", "--no-edit"]);

    // create a few normal operations
    for state in 'A'..='D' {
        work_dir.write_file("state", state.to_string());
        snapshot();
    }
    assert_eq!(work_dir.read_file("state"), "D");

    // undo operations D and C, restoring the state of B
    work_dir.run_jj(["undo"]).success();
    assert_eq!(work_dir.read_file("state"), "C");
    work_dir.run_jj(["undo"]).success();
    assert_eq!(work_dir.read_file("state"), "B");

    // create operations E and F
    work_dir.write_file("state", "E");
    snapshot();
    work_dir.write_file("state", "F");
    snapshot();
    assert_eq!(work_dir.read_file("state"), "F");

    // undo operations F, E and B, restoring the state of A while skipping the
    // undo-stack of C and D in the op log
    work_dir.run_jj(["undo"]).success();
    assert_eq!(work_dir.read_file("state"), "E");
    work_dir.run_jj(["undo"]).success();
    assert_eq!(work_dir.read_file("state"), "B");
    work_dir.run_jj(["undo"]).success();
    assert_eq!(work_dir.read_file("state"), "A");
}

#[test]
fn test_op_revert_is_ignored() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    let snapshot = || work_dir.run_jj(["describe", "--no-edit"]);

    // create a few normal operations
    work_dir.write_file("state", "A");
    snapshot();
    work_dir.write_file("state", "B");
    snapshot();
    assert_eq!(work_dir.read_file("state"), "B");

    // `op revert` works the same way as `undo` initially, but running `undo`
    // afterwards will result in a no-op. `undo` does not recognize operations
    // created by `op revert` as undo-operations on which an undo-stack can
    // be grown.
    work_dir.run_jj(["op", "revert"]).success();
    assert_eq!(work_dir.read_file("state"), "A");
    work_dir.run_jj(["undo"]).success();
    assert_eq!(work_dir.read_file("state"), "B");
}
