use std::ops::Not;

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
        specification_version,
        files,
        executables,
        extensions,
        dependencies,
        post_install_message: _,
        requirements: _,
        test_files: _,
        extra_rdoc_files,
        rdoc_options,
        cert_chain,
        signing_key: _,
        autorequire: _,
        installed_by_version: _,
    } = spec;

    use std::fmt::Write;
    let mut ruby_src = format!(
        "# -*- encoding: utf-8 -*-
# stub: {name} {version} ruby lib\n"
    );
    if extensions.is_empty().not() {
        ruby_src.push_str("# stub: ");
        // Yes, this is actually the joining character, null byte.
        let exts = extensions.join("\0");
        ruby_src.push_str(&exts);
        ruby_src.push('\n');
    }

    // Done with stubs, so
    ruby_src.push('\n');

    ruby_src.push_str("Gem::Specification.new do |s|\n");
    writeln!(ruby_src, "  s.name = \"{}\".freeze", name).unwrap();
    writeln!(ruby_src, "  s.version = \"{}\".freeze", version).unwrap();
    ruby_src.push('\n');
    writeln!(ruby_src, "  s.required_rubygems_version = Gem::Requirement.new(\"{}\".freeze) if s.respond_to? :required_rubygems_version=", required_rubygems_version).unwrap();
    if !metadata.is_empty() {
        write!(ruby_src, "  s.metadata = {{ ").unwrap();
        let mut md_items = Vec::with_capacity(metadata.len());
        for (k, v) in &metadata {
            md_items.push(format!("\"{}\" => \"{}\"", k, v));
        }
        ruby_src.push_str(&md_items.join(", "));
        writeln!(ruby_src, " }} if s.respond_to? :metadata=").unwrap();
    }
    writeln!(
        ruby_src,
        "  s.require_paths = [{}]",
        ruby_list(&require_paths)
    )
    .unwrap();
    writeln!(ruby_src, "  s.authors = [{}]", ruby_list_opt(&authors)).unwrap();
    if cert_chain.is_empty().not() {
        writeln!(ruby_src, "  s.cert_chain = [{}]", ruby_list(&cert_chain)).unwrap();
    }
    if bindir != "bin" {
        writeln!(ruby_src, "  s.bindir = \"{}\".freeze", bindir).unwrap();
    }
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
        writeln!(
            ruby_src,
            "  s.description = \"{}\".freeze",
            ruby_scalar(&description)
        )
        .unwrap();
    }
    if email.is_empty().not() {
        writeln!(ruby_src, "  s.email = [{}]", ruby_list_opt_nil(&email)).unwrap();
    }
    if executables.is_empty().not() {
        writeln!(ruby_src, "  s.executables = [{}]", ruby_list(&executables)).unwrap();
    }
    if !extensions.is_empty() {
        writeln!(ruby_src, "  s.extensions = [{}]", ruby_list(&extensions)).unwrap();
    }
    if !extra_rdoc_files.is_empty() {
        writeln!(
            ruby_src,
            "  s.extra_rdoc_files = [{}]",
            ruby_list(&extra_rdoc_files)
        )
        .unwrap();
    }
    if !files.is_empty() {
        writeln!(ruby_src, "  s.files = [{}]", ruby_list(&files)).unwrap();
    }
    if let Some(homepage) = homepage {
        writeln!(ruby_src, "  s.homepage = \"{}\".freeze", homepage).unwrap();
    }
    writeln!(ruby_src, "  s.licenses = [{}]", ruby_list(&licenses)).unwrap();
    if rdoc_options.is_empty().not() {
        writeln!(
            ruby_src,
            "  s.rdoc_options = [{}]",
            ruby_list(&rdoc_options)
        )
        .unwrap();
    }
    if required_ruby_version != Default::default() {
        writeln!(
            ruby_src,
            "  s.required_ruby_version = Gem::Requirement.new(\"{}\".freeze)",
            required_ruby_version
        )
        .unwrap();
    }
    writeln!(
        ruby_src,
        "  s.rubygems_version = \"{}\".freeze",
        rubygems_version
    )
    .unwrap();
    writeln!(
        ruby_src,
        "  s.summary = \"{}\".freeze",
        ruby_scalar(&summary)
    )
    .unwrap();

    // Wrap it up.
    ruby_src.push_str("\n  s.installed_by_version = \"4.0.3\".freeze\n");

    if dependencies.is_empty().not() {
        writeln!(
            ruby_src,
            "\n  s.specification_version = {}\n",
            specification_version,
        )
        .unwrap();

        for dep in dependencies {
            let dep_type = if dep.is_runtime() {
                "runtime"
            } else {
                "development"
            };
            let dep_name = dep.name;
            let dep_req: Vec<_> = dep
                .requirement
                .constraints
                .into_iter()
                .map(|constraint| format!("\"{}\".freeze", constraint))
                .collect();
            let dep_req = dep_req.join(", ");
            writeln!(
                ruby_src,
                "  s.add_{}_dependency(%q<{}>.freeze, [{}])",
                dep_type, dep_name, dep_req,
            )
            .unwrap();
        }
    }
    ruby_src.push_str("end\n");
    ruby_src
}

fn ruby_scalar<T: std::fmt::Display>(input: &T) -> String {
    // Escape strings so they can be put into a Ruby string literal.
    let mut s = String::new();
    for ch in input.to_string().chars() {
        // Escape double-quotes
        if ch == '"' {
            s.push('\\');
            s.push(ch);
        // Escape newlines
        } else if ch == '\n' {
            s.push('\\');
            s.push('n');
        // Escape backslashes
        } else if ch == '\\' {
            s.push('\\');
            s.push(ch);
        // No escape needed, just do the character like normal.
        } else {
            s.push(ch);
        }
    }
    s
}

fn ruby_list(v: &[String]) -> String {
    v.iter()
        .map(|p| format!("\"{}\".freeze", ruby_scalar(p)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn ruby_list_opt(v: &[Option<String>]) -> String {
    v.iter()
        .flatten()
        .map(|p| format!("\"{}\".freeze", ruby_scalar(p)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn ruby_list_opt_nil(v: &[Option<String>]) -> String {
    v.iter()
        .map(|p| {
            if let Some(p) = p {
                format!("\"{}\".freeze", ruby_scalar(p))
            } else {
                "nil".to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {

    use super::*;

    fn run_test(name: &str) {
        let input_path = format!("tests/yaml-to-ruby/{}.yaml", name);
        let output_path = format!("tests/yaml-to-ruby/{}.gemspec", name);
        let input = fs_err::read_to_string(input_path).unwrap();
        let expected = fs_err::read_to_string(output_path).unwrap();
        let actual = to_ruby(crate::parse(&input).unwrap());
        pretty_assertions::assert_eq!(expected, actual);
    }

    #[test]
    fn test_abbrev() {
        run_test("abbrev");
    }

    #[test]
    fn test_base64() {
        run_test("base64");
    }

    #[test]
    fn test_benchmark() {
        run_test("benchmark");
    }

    #[test]
    fn test_bigdecimal() {
        run_test("bigdecimal");
    }

    #[test]
    fn test_bootsnap() {
        run_test("bootsnap");
    }

    #[test]
    fn test_bundler() {
        run_test("bundler");
    }

    #[test]
    fn test_csv() {
        run_test("csv");
    }

    #[test]
    fn test_date() {
        run_test("date");
    }

    #[test]
    fn test_debug() {
        run_test("debug");
    }

    #[test]
    fn test_delegate() {
        run_test("delegate");
    }

    #[test]
    fn test_did_you_mean() {
        run_test("did_you_mean");
    }

    #[test]
    fn test_digest() {
        run_test("digest");
    }

    #[test]
    fn test_drb() {
        run_test("drb");
    }

    #[test]
    fn test_english() {
        run_test("english");
    }

    #[test]
    fn test_erb() {
        run_test("erb");
    }

    #[test]
    fn test_error_highlight() {
        run_test("error_highlight");
    }

    #[test]
    fn test_etc() {
        run_test("etc");
    }

    #[test]
    fn test_fcntl() {
        run_test("fcntl");
    }

    #[test]
    fn test_fiddle() {
        run_test("fiddle");
    }

    #[test]
    fn test_fileutils() {
        run_test("fileutils");
    }

    #[test]
    fn test_find() {
        run_test("find");
    }

    #[test]
    fn test_forwardable() {
        run_test("forwardable");
    }

    #[test]
    fn test_getoptlong() {
        run_test("getoptlong");
    }

    #[test]
    fn test_console() {
        run_test("io-console");
    }

    #[test]
    fn test_nonblock() {
        run_test("io-nonblock");
    }

    #[test]
    fn test_wait() {
        run_test("io-wait");
    }

    #[test]
    fn test_ipaddr() {
        run_test("ipaddr");
    }

    #[test]
    fn test_irb() {
        run_test("irb");
    }

    #[test]
    fn test_json() {
        run_test("json");
    }

    #[test]
    fn test_logger() {
        run_test("logger");
    }

    #[test]
    fn test_matrix() {
        run_test("matrix");
    }

    #[test]
    fn test_minitest() {
        run_test("minitest");
    }

    #[test]
    fn test_msgpack() {
        run_test("msgpack");
    }

    #[test]
    fn test_mutex_m() {
        run_test("mutex_m");
    }

    #[test]
    fn test_ftp() {
        run_test("net-ftp");
    }

    #[test]
    fn test_http() {
        run_test("net-http");
    }

    #[test]
    fn test_imap() {
        run_test("net-imap");
    }

    #[test]
    fn test_pop() {
        run_test("net-pop");
    }

    #[test]
    fn test_protocol() {
        run_test("net-protocol");
    }

    #[test]
    fn test_smtp() {
        run_test("net-smtp");
    }

    #[test]
    fn test_nkf() {
        run_test("nkf");
    }

    #[test]
    fn test_observer() {
        run_test("observer");
    }

    #[test]
    fn test_open_uri() {
        run_test("open-uri");
    }

    #[test]
    fn test_open3() {
        run_test("open3");
    }

    #[test]
    fn test_openssl() {
        run_test("openssl");
    }

    #[test]
    fn test_optparse() {
        run_test("optparse");
    }

    #[test]
    fn test_ostruct() {
        run_test("ostruct");
    }

    #[test]
    fn test_power_assert() {
        run_test("power_assert");
    }

    #[test]
    fn test_pp() {
        run_test("pp");
    }

    #[test]
    fn test_prettyprint() {
        run_test("prettyprint");
    }

    #[test]
    fn test_prime() {
        run_test("prime");
    }

    #[test]
    fn test_prism() {
        run_test("prism");
    }

    #[test]
    fn test_pstore() {
        run_test("pstore");
    }

    #[test]
    fn test_psych() {
        run_test("psych");
    }

    #[test]
    fn test_racc() {
        run_test("racc");
    }

    #[test]
    fn test_rake() {
        run_test("rake");
    }

    #[test]
    fn test_rbs() {
        run_test("rbs");
    }

    #[test]
    fn test_rdoc() {
        run_test("rdoc");
    }

    #[test]
    fn test_readline() {
        run_test("readline");
    }

    #[test]
    fn test_reline() {
        run_test("reline");
    }

    #[test]
    fn test_repl_type_completor() {
        run_test("repl_type_completor");
    }

    #[test]
    fn test_replace() {
        run_test("resolv-replace");
    }

    #[test]
    fn test_resolv() {
        run_test("resolv");
    }

    #[test]
    fn test_rexml() {
        run_test("rexml");
    }

    #[test]
    fn test_rinda() {
        run_test("rinda");
    }

    #[test]
    fn test_rss() {
        run_test("rss");
    }

    #[test]
    fn test_ruby2_keywords() {
        run_test("ruby2_keywords");
    }

    #[test]
    fn test_securerandom() {
        run_test("securerandom");
    }

    #[test]
    fn test_shellwords() {
        run_test("shellwords");
    }

    #[test]
    fn test_singleton() {
        run_test("singleton");
    }

    #[test]
    fn test_stringio() {
        run_test("stringio");
    }

    #[test]
    fn test_strscan() {
        run_test("strscan");
    }

    #[test]
    fn test_syntax_suggest() {
        run_test("syntax_suggest");
    }

    #[test]
    fn test_syslog() {
        run_test("syslog");
    }

    #[test]
    fn test_tempfile() {
        run_test("tempfile");
    }

    #[test]
    fn test_unit() {
        run_test("test-unit");
    }

    #[test]
    fn test_time() {
        run_test("time");
    }

    #[test]
    fn test_timeout() {
        run_test("timeout");
    }

    #[test]
    fn test_tmpdir() {
        run_test("tmpdir");
    }

    #[test]
    fn test_tsort() {
        run_test("tsort");
    }

    #[test]
    fn test_typeprof() {
        run_test("typeprof");
    }

    #[test]
    fn test_un() {
        run_test("un");
    }

    #[test]
    fn test_uri() {
        run_test("uri");
    }

    #[test]
    fn test_weakref() {
        run_test("weakref");
    }

    #[test]
    fn test_yaml() {
        run_test("yaml");
    }

    #[test]
    fn test_zlib() {
        run_test("zlib");
    }
}
