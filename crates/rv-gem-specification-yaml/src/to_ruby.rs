use rv_gem_types::Specification;

/// Converts a Gemspec to Ruby source.
pub fn to_ruby(spec: Specification) -> String {
    let Specification {
        name,
        version,
        summary,
        require_paths,
        rubygems_version,
        date,
        authors,
        email,
        homepage,
        description,
        licenses,
        metadata,
        bindir,
        required_ruby_version,
        required_rubygems_version,
        platform: _,
        specification_version: _,
        files: _,
        executables: _,
        extensions: _,
        dependencies: _,
        post_install_message: _,
        requirements: _,
        test_files: _,
        extra_rdoc_files: _,
        rdoc_options: _,
        cert_chain: _,
        signing_key: _,
        autorequire: _,
        installed_by_version: _,
    } = spec;

    use std::fmt::Write;
    let start = format!(
        "# -*- encoding: utf-8 -*-
# stub: {name} {version} ruby lib

Gem::Specification.new do |s|\n"
    );
    let mut ruby_src = start.to_owned();
    writeln!(ruby_src, "  s.name = \"{}\".freeze", name).unwrap();
    writeln!(ruby_src, "  s.version = \"{}\".freeze", version).unwrap();
    ruby_src.push('\n');
    writeln!(ruby_src, "  s.required_rubygems_version = Gem::Requirement.new(\"{}\".freeze) if s.respond_to? :required_rubygems_version=", required_rubygems_version).unwrap();
    if !metadata.is_empty() {
        write!(ruby_src, "  s.metadata = {{ ").unwrap();
        let mut md_items = Vec::with_capacity(metadata.len());
        for (k, v) in &metadata {
            md_items.push(format!("\"{k}\" => \"{v}\""));
        }
        ruby_src.push_str(&md_items.join(", "));
        writeln!(ruby_src, " }} if s.respond_to? :metadata=").unwrap();
    }
    writeln!(
        ruby_src,
        "  s.require_paths = [{}]",
        ruby_list(require_paths)
    )
    .unwrap();
    writeln!(ruby_src, "  s.authors = [{}]", ruby_list_opt(authors)).unwrap();
    writeln!(ruby_src, "  s.bindir = \"{}\".freeze", bindir).unwrap();
    writeln!(
        ruby_src,
        "  s.date = \"{}\"",
        if let Some(date) = date.strip_suffix(" 00:00:00.000000000 Z") {
            date
        } else {
            &date
        }
    )
    .unwrap();
    if let Some(description) = description {
        writeln!(ruby_src, "  s.description = \"{}\".freeze", description).unwrap();
    }
    writeln!(ruby_src, "  s.email = [{}]", ruby_list_opt(email)).unwrap();
    if let Some(homepage) = homepage {
        writeln!(ruby_src, "  s.homepage = \"{}\".freeze", homepage).unwrap();
    }
    writeln!(ruby_src, "  s.licenses = [{}]", ruby_list(licenses)).unwrap();
    writeln!(
        ruby_src,
        "  s.required_ruby_version = Gem::Requirement.new(\"{}\".freeze)",
        required_ruby_version
    )
    .unwrap();
    writeln!(
        ruby_src,
        "  s.rubygems_version = \"{}\".freeze",
        rubygems_version
    )
    .unwrap();
    writeln!(ruby_src, "  s.summary = \"{}\".freeze", summary).unwrap();

    // Wrap it up.
    let end = "\n  s.installed_by_version = \"4.0.3\".freeze
end\n";
    ruby_src.push_str(end);
    ruby_src
}

fn ruby_list<T: std::fmt::Display>(v: Vec<T>) -> String {
    v.into_iter()
        .map(|p| format!("\"{p}\".freeze"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn ruby_list_opt<T: std::fmt::Display>(v: Vec<Option<T>>) -> String {
    v.into_iter()
        .flatten()
        .map(|p| format!("\"{p}\".freeze"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_test(name: &str) {
        let input_path = format!("tests/yaml-to-ruby/{name}.yaml");
        let output_path = format!("tests/yaml-to-ruby/{name}.gemspec");
        let input = fs_err::read_to_string(input_path).unwrap();
        let expected = fs_err::read_to_string(output_path).unwrap();
        let actual = to_ruby(crate::parse(&input).unwrap());
        pretty_assertions::assert_eq!(expected, actual);
    }

    #[test]
    fn test_all() {
        run_test("abbrev");
        run_test("base64");
        run_test("benchmark");
    }
}
