#![no_main]

use libfuzzer_sys::fuzz_target;
use rv_gem_specification_yaml::parse;

fuzz_target!(|data: &str| {
    let _parse_res = parse(data);
});
