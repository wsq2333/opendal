// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

// Unix filesystems preserve trailing spaces; Windows filename rules differ.
#![cfg(unix)]

use opendal_core::{ErrorKind, Operator};
use opendal_service_fs::Fs;

#[tokio::test]
async fn listed_whitespace_names_remain_readable_after_reopening() {
    let root = tempfile::tempdir().unwrap();
    let names = ["file", "file ", " leading", " ", "tab\t", "unicode\u{a0}"];
    // Seed through the filesystem so matching bugs in write and read cannot cancel out.
    for name in names {
        std::fs::write(root.path().join(name), name.as_bytes()).unwrap();
    }
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    let entries = op.list("/").await.unwrap();
    let mut listed: Vec<_> = entries
        .iter()
        .filter(|entry| entry.metadata().is_file())
        .map(|entry| entry.path())
        .collect();
    listed.sort_unstable();
    let mut expected = names.to_vec();
    expected.sort_unstable();
    assert_eq!(listed, expected);
    drop(op);

    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    for name in names {
        assert_eq!(
            op.stat(name).await.unwrap().content_length(),
            name.len() as u64
        );
        assert_eq!(op.read(name).await.unwrap().to_vec(), name.as_bytes());
    }
}

#[tokio::test]
async fn read_whitespace_name_does_not_return_plain_file_contents() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), b"plain").unwrap();
    std::fs::write(root.path().join("file "), b"spaced").unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    assert_eq!(op.read("file ").await.unwrap().to_vec(), b"spaced");
    assert_eq!(op.read("file").await.unwrap().to_vec(), b"plain");
}

#[tokio::test]
async fn missing_whitespace_name_does_not_alias_plain_file() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), b"plain").unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    assert_eq!(
        op.stat("file ").await.unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert_eq!(
        op.read("file ").await.unwrap_err().kind(),
        ErrorKind::NotFound
    );
}

#[tokio::test]
async fn write_rename_and_delete_preserve_plain_siblings() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), b"plain source").unwrap();
    std::fs::write(root.path().join("renamed"), b"plain target").unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    op.write("file ", "spaced").await.unwrap();
    assert_eq!(
        std::fs::read(root.path().join("file")).unwrap(),
        b"plain source"
    );
    assert_eq!(std::fs::read(root.path().join("file ")).unwrap(), b"spaced");
    op.rename("file ", "renamed ").await.unwrap();
    assert!(!root.path().join("file ").exists());
    assert_eq!(
        std::fs::read(root.path().join("renamed ")).unwrap(),
        b"spaced"
    );
    assert_eq!(
        std::fs::read(root.path().join("renamed")).unwrap(),
        b"plain target"
    );
    op.delete("renamed ").await.unwrap();
    assert!(!root.path().join("renamed ").exists());
    assert_eq!(
        std::fs::read(root.path().join("file")).unwrap(),
        b"plain source"
    );
    assert_eq!(
        std::fs::read(root.path().join("renamed")).unwrap(),
        b"plain target"
    );
}

#[tokio::test]
async fn nested_whitespace_paths_preserve_file_identity() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(" leading/dir ")).unwrap();
    std::fs::write(root.path().join(" leading/dir /file"), b"plain").unwrap();
    std::fs::write(root.path().join(" leading/dir /file "), b"spaced").unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    assert_eq!(
        op.read(" leading/dir /file ").await.unwrap().to_vec(),
        b"spaced"
    );
    assert_eq!(
        op.read(" leading/dir /file").await.unwrap().to_vec(),
        b"plain"
    );
    let mut entries: Vec<_> = op
        .list(" leading/dir /")
        .await
        .unwrap()
        .into_iter()
        .filter(|entry| entry.metadata().is_file())
        .map(|entry| entry.path().to_owned())
        .collect();
    entries.sort_unstable();
    assert_eq!(entries, [" leading/dir /file", " leading/dir /file "]);
}
