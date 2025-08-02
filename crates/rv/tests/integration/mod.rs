pub mod common;

use common::RvTest;
use insta::assert_snapshot;

// #[test]
// fn test_ruby_list_text_output_empty() {
//     let test = RvTest::new();
//     let output = test.ruby_list(&[]);

//     assert!(output.success(), "rv ruby list should succeed");
//     assert_snapshot!(output.normalized_stdout());
// }

// #[test]
// fn test_ruby_list_json_output_empty() {
//     let test = RvTest::new();
//     let output = test.ruby_list(&["--format", "json"]);

//     assert!(
//         output.success(),
//         "rv ruby list --format json should succeed"
//     );
//     assert_snapshot!(output.normalized_stdout());
// }

// #[test]
// fn test_ruby_list_text_output_with_rubies() {
//     let test = RvTest::new();

//     // Create some mock Ruby installations
//     test.create_ruby_dir("ruby-3.1.4");
//     test.create_ruby_dir("ruby-3.2.0");

//     let output = test.ruby_list(&[]);

//     assert!(output.success(), "rv ruby list should succeed");
//     assert_snapshot!(output.normalized_stdout());
// }

// #[test]
// fn test_ruby_list_json_output_with_rubies() {
//     let test = RvTest::new();

//     // Create some mock Ruby installations
//     test.create_ruby_dir("ruby-3.1.4");
//     test.create_ruby_dir("ruby-3.2.0");

//     let output = test.ruby_list(&["--format", "json"]);

//     assert!(
//         output.success(),
//         "rv ruby list --format json should succeed"
//     );

//     // Verify it's valid JSON
//     let stdout = output.stdout();
//     let _: serde_json::Value = serde_json::from_str(&stdout).expect("Output should be valid JSON");

//     assert_snapshot!(output.normalized_stdout());
// }
