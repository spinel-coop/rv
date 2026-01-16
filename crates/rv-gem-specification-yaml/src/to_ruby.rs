use rv_gem_types::Specification;

fn to_ruby(spec: Specification) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_test(name: &str) {
        println!("{}", std::env::current_dir().unwrap().display().to_string());
        let input_path = format!("tests/yaml-to-ruby/{name}.yaml");
        let output_path = format!("tests/yaml-to-ruby/{name}.gemspec");
        let input = fs_err::read_to_string(input_path).unwrap();
        let expected = fs_err::read_to_string(output_path).unwrap();
        let actual = to_ruby(crate::parse(&input).unwrap());
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_abbrev() {
        run_test("abbrev");
    }
}
