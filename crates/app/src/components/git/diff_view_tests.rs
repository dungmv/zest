use super::*;

#[test]
fn test_parse_diff_filters_new_file_mode_and_initial_hunk() {
    let raw_diff = r#"diff --git a/collections/speedtest/new/new-request.yaml b/collections/speedtest/new/new-request.yaml
new file mode 100644
index 0000000..c221a93
--- /dev/null
+++ b/collections/speedtest/new/new-request.yaml
@@ -0,0 +1,3 @@
+name: New Request
+method: GET
+url: https://example.com"#;

    let rows = parse_side_by_side_diff(raw_diff);

    // Should NOT have new file mode as code row
    for r in &rows {
        if let SideBySideRow::Diff { left, right } = r {
            if let Some(l) = left {
                assert!(!l.content.contains("new file mode"));
                assert!(!l.content.contains("@@"));
            }
            if let Some(r) = right {
                assert!(!r.content.contains("new file mode"));
                assert!(!r.content.contains("@@"));
            }
        }
    }

    // Must have 3 added lines
    assert_eq!(rows.len(), 3);
    if let SideBySideRow::Diff {
        left: None,
        right: Some(r),
    } = &rows[0]
    {
        assert_eq!(r.content, "name: New Request");
        assert_eq!(r.kind, DiffLineKind::Added);
        assert_eq!(r.line_no, Some(1));
    } else {
        panic!("Expected added diff row at index 0");
    }
}

#[test]
fn test_parse_diff_suppresses_hunk_headers_and_numbers_accurately() {
    let raw_diff = r#"diff --git a/file.txt b/file.txt
index 1111111..2222222 100644
--- a/file.txt
+++ b/file.txt
@@ -10,3 +10,3 @@
 normal line
-old line
+new line"#;

    let rows = parse_side_by_side_diff(raw_diff);
    // Hunk header @@ -10,3 is suppressed: only 2 code rows (1 normal line + 1 modified row)
    assert_eq!(rows.len(), 2);
    if let SideBySideRow::Diff {
        left: Some(l),
        right: Some(r),
    } = &rows[0]
    {
        assert_eq!(l.content, "normal line");
        assert_eq!(l.line_no, Some(10));
        assert_eq!(r.line_no, Some(10));
    } else {
        panic!("Expected normal diff row at index 0");
    }
}

#[test]
fn test_parse_multi_file_diff_inserts_file_headers() {
    let raw_diff = r#"diff --git a/file1.txt b/file1.txt
index 1111111..2222222 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1,1 +1,1 @@
-a
+b
diff --git a/file2.txt b/file2.txt
index 3333333..4444444 100644
--- a/file2.txt
+++ b/file2.txt
@@ -1,1 +1,1 @@
-c
+d"#;

    let rows = parse_side_by_side_diff(raw_diff);
    let file_headers: Vec<_> = rows
        .iter()
        .filter_map(|r| match r {
            SideBySideRow::FileHeader(f) => Some(f.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(file_headers, vec!["file1.txt", "file2.txt"]);
}

#[test]
fn test_split_diff_into_files() {
    let diff = r#"diff --git a/file1.txt b/file1.txt
index 1111111..2222222 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1,1 +1,1 @@
-a
+b
diff --git a/dir/file2.rs b/dir/file2.rs
index 3333333..4444444 100644
--- a/dir/file2.rs
+++ b/dir/file2.rs
@@ -1,1 +1,1 @@
-c
+d"#;

    let files = split_diff_into_files(diff);
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].0, "file1.txt");
    assert!(files[0].1.contains("-a"));
    assert_eq!(files[1].0, "dir/file2.rs");
    assert!(files[1].1.contains("+d"));
}
