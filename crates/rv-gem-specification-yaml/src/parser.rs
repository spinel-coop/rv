use std::collections::HashMap;

use indexmap::IndexMap;
use miette::{Result, SourceSpan};
use saphyr::Scalar;
use saphyr_parser::{Event, Parser as SaphyrParser, Span, StrInput};
use winnow::combinator::*;
use winnow::error::{ContextError, ErrMode, ParserError, StrContext, StrContextValue};
use winnow::token::*;
use winnow::{ModalResult, Parser};

use rv_gem_types::{Dependency, DependencyType, Platform, Requirement, Specification, Version};

pub use error::DeserializationError;

mod error;
type AnchorMap = HashMap<usize, String>;

// Helper function to parse YAML into events
fn parse_yaml_events<'a>(source: &'a str) -> Result<Vec<(Event<'a>, Span)>> {
    let input = StrInput::new(source);
    let parser = SaphyrParser::new(input);

    // Collect all events upfront
    let events: Result<Vec<_>, _> = parser.collect();
    events.map_err(|e| {
        // Extract the exact location from the ScanError
        let marker = e.marker();
        let start_offset = marker.index();
        let length = 1; // For YAML scan errors, typically point to a single character

        DeserializationError::Parse {
            source: e,
            bad_bit: SourceSpan::new(start_offset.into(), length),
        }
        .into()
    })
}

// Basic event parsers
fn scalar_event<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<Scalar<'a>, ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::Scalar(value, style, _, tag) => {
            Scalar::parse_from_cow_and_metadata(value, style, tag.as_ref())
        }
        _ => None,
    })
    .parse_next(input)
}

fn string<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<String, ContextError> {
    scalar_event
        .verify_map(|s| s.as_str().map(|s| s.to_string()))
        .parse_next(input)
}

// Parse optional scalar - returns None for nil/null values, Some for actual values
fn optional_string<'a>(
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Option<String>, ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::Scalar(value, style, _, tag) => {
            Scalar::parse_from_cow_and_metadata(value, style, tag.as_ref()).map(|s| match s {
                Scalar::String(s) => Some(s.to_string()),
                _ => None,
            })
        }
        _ => None,
    })
    .parse_next(input)
}

fn mapping_start<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::MappingStart(_, _) => Some(()),
        _ => None,
    })
    .parse_next(input)
}

fn mapping_end<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::MappingEnd => Some(()),
        _ => None,
    })
    .parse_next(input)
}

fn sequence_start<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<usize, ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::SequenceStart(anchor_id, _) => Some(anchor_id),
        _ => None,
    })
    .parse_next(input)
}

fn sequence_end<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::SequenceEnd => Some(()),
        _ => None,
    })
    .parse_next(input)
}

fn stream_start<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::StreamStart => Some(()),
        _ => None,
    })
    .parse_next(input)
}

fn stream_end<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::StreamEnd => Some(()),
        _ => None,
    })
    .parse_next(input)
}

fn document_start<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::DocumentStart(_) => Some(()),
        _ => None,
    })
    .parse_next(input)
}

fn document_end<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    any.verify_map(|(event, _span)| match event {
        Event::DocumentEnd => Some(()),
        _ => None,
    })
    .parse_next(input)
}

fn tagged_mapping_start<'a>(
    expected_tag: &'static str,
) -> impl winnow::Parser<&'a [(Event<'a>, Span)], usize, ContextError> + 'a {
    move |input: &mut &'a [(Event<'a>, Span)]| {
        any.verify_map(|(event, _span)| match event {
            Event::MappingStart(anchor_id, Some(tag))
                if tag.handle == "!" && tag.suffix == expected_tag =>
            {
                Some(anchor_id)
            }
            _ => None,
        })
        .context(StrContext::Expected(expected_tag.into()))
        .parse_next(input)
    }
}

fn parse_version<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<Version, ContextError> {
    delimited(
        tagged_mapping_start("ruby/object:Gem::Version"),
        parse_version_fields,
        mapping_end,
    )
    .context(StrContext::Label("Gem::Version object"))
    .parse_next(input)
}

fn parse_version_fields<'a>(
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Version, ContextError> {
    let mut version_str: Option<String> = None;

    // Parse all fields in the version object until we hit mapping_end
    loop {
        // Check if we're at the end of the mapping
        match peek(any::<_, ContextError>).parse_next(input) {
            Ok((Event::MappingEnd, _)) => break,
            Ok(_) => {
                // Continue parsing key-value pairs
            }
            Err(_) => break, // End of stream
        }

        // Parse key-value pair
        let key = string
            .context(StrContext::Expected(StrContextValue::Description(
                "version field key",
            )))
            .parse_next(input)?;
        match key.as_str() {
            "version" => {
                version_str = Some(
                    string
                        .context(StrContext::Expected(StrContextValue::Description(
                            "version string value",
                        )))
                        .parse_next(input)?,
                );
            }
            "prerelease" => {
                // Skip prerelease field - we don't need it for the version string
                skip_value.parse_next(input)?;
            }
            _ => {
                // Skip unknown fields
                skip_value.parse_next(input)?;
            }
        }
    }

    // TODO: surface missing fields as errors that span the entire mapping
    let version_str = version_str.ok_or_else(|| ErrMode::Cut(ContextError::from_input(input)))?;
    Version::new(&version_str).map_err(|_e| ErrMode::Cut(ContextError::from_input(input)))
}

// Skip any YAML value - handles scalars, sequences, mappings, and nil values
fn skip_value<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    alt((
        scalar_event.map(|_| ()),
        skip_alias,
        skip_sequence,
        skip_mapping,
    ))
    .parse_next(input)
}

fn skip_sequence<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    delimited(
        sequence_start,
        repeat::<_, _, Vec<_>, _, _>(0.., skip_value),
        sequence_end,
    )
    .map(|_| ())
    .parse_next(input)
}

fn skip_mapping<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    delimited(
        mapping_start,
        repeat::<_, _, Vec<_>, _, _>(0.., (skip_value, skip_value)), // key-value pairs
        mapping_end,
    )
    .map(|_| ())
    .parse_next(input)
}

fn skip_alias<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    any.verify_map(|(e, _)| match e {
        Event::Alias(_) => Some(()),
        _ => None,
    })
    .parse_next(input)
}

// String array parsing for regular string arrays
fn parse_string_array<'a>(
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Vec<String>, ContextError> {
    alt((
        delimited(sequence_start, repeat(0.., string), sequence_end),
        null.map(|_| vec![]),
        string.map(|s| vec![s]),
    ))
    .context(StrContext::Label("string array"))
    .parse_next(input)
}

// Optional string array parsing - handles arrays with null/empty values
fn parse_optional_string_array<'a>(
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Vec<Option<String>>, ContextError> {
    alt((
        delimited(
            sequence_start,
            repeat(0.., parse_optional_string_entry),
            sequence_end,
        ),
        null.map(|_| vec![]),
        string.map(|s| vec![Some(s)]),
    ))
    .context(StrContext::Label("optional string array"))
    .parse_next(input)
}

// Parse a string entry that might be null/empty
fn parse_optional_string_entry<'a>(
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Option<String>, ContextError> {
    alt((string.map(Some), null.map(|_| None))).parse_next(input)
}

fn null<'a>(input: &mut &'a [(Event<'a>, Span)]) -> ModalResult<(), ContextError> {
    scalar_event
        .verify_map(|s| s.is_null().then_some(()))
        .parse_next(input)
}

// Parse metadata as a BTreeMap of string key-value pairs
fn parse_metadata_as_map<'a>(
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<IndexMap<String, String>, ContextError> {
    delimited(
        mapping_start,
        repeat::<_, _, Vec<_>, _, _>(0.., (string, string)), // key-value pairs of strings
        mapping_end,
    )
    .map(|pairs: Vec<(String, String)>| pairs.into_iter().collect())
    .parse_next(input)
}

fn parse_requirement<'a>(
    anchors: &mut AnchorMap,
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Requirement, ContextError> {
    alt([
        tagged_mapping_start("ruby/object:Gem::Requirement"),
        tagged_mapping_start("ruby/object:Gem::Version::Requirement"),
    ])
    .parse_next(input)?;
    let fields = parse_requirement_fields(anchors, input)?;
    mapping_end.parse_next(input)?;
    Ok(fields)
}

fn parse_requirement_fields<'a>(
    anchors: &mut AnchorMap,
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Requirement, ContextError> {
    let mut constraints: Option<Vec<String>> = None;

    // Parse all fields in the requirement object until we hit mapping_end
    loop {
        // Check if we're at the end of the mapping
        match peek(any::<_, ContextError>).parse_next(input) {
            Ok((Event::MappingEnd, _)) => break,
            Ok(_) => {
                // Continue parsing key-value pairs
            }
            Err(_) => break, // End of stream
        }

        // Parse key-value pair
        let key = string
            .context(StrContext::Expected(StrContextValue::Description(
                "requirement field key",
            )))
            .parse_next(input)?;
        match key.as_str() {
            "requirements" => {
                constraints = Some(parse_constraint_array(anchors, input)?);
            }
            "none" => {
                // Skip the 'none' field - it's legacy metadata
                skip_value.parse_next(input)?;
            }
            _ => {
                // Skip unknown fields
                skip_value.parse_next(input)?;
            }
        }
    }

    let constraints = constraints.ok_or_else(|| ErrMode::Cut(ContextError::new()))?;
    Requirement::new(constraints).map_err(|_e| ErrMode::Cut(ContextError::new()))
}

fn parse_constraint_array<'a>(
    anchors: &mut AnchorMap,
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Vec<String>, ContextError> {
    sequence_start.parse_next(input)?;
    let mut constraints = Vec::new();
    loop {
        match peek(any::<_, ContextError>).parse_next(input) {
            Ok((Event::SequenceEnd, _)) => break,
            Ok(_) => {
                let context = StrContext::Label("constraint array");
                constraints.push(parse_constraint_pair(input, anchors, context)?);
            }
            Err(_) => break,
        }
    }
    sequence_end.parse_next(input)?;
    Ok(constraints)
}

fn parse_constraint_pair<'a>(
    input: &mut &'a [(Event<'a>, Span)],
    anchors: &mut AnchorMap,
    context: StrContext,
) -> ModalResult<String, ContextError> {
    // Check what kind of event we have
    match peek(any::<_, ContextError>)
        .context(context)
        .parse_next(input)
    {
        Ok((Event::Alias(anchor_id), _)) => {
            // Consume the alias event
            let _ = any::<_, ContextError>.parse_next(input)?;
            match anchors.get(&anchor_id) {
                Some(source) => Ok(source.to_string()),
                _ => Err(ErrMode::Backtrack(ContextError::new())),
            }
        }
        Ok((Event::SequenceStart(_, _), _)) => {
            // Parse a sequence like [">=", "2.0"]
            let anchor_id = sequence_start.parse_next(input)?;
            let constraint = (string, parse_version)
                .map(|(op, version)| format!("{op} {version}"))
                .parse_next(input)?;
            anchors.insert(anchor_id, constraint.to_string());
            sequence_end.parse_next(input)?;
            Ok(constraint)
        }
        _ => Err(ErrMode::Backtrack(ContextError::new())),
    }
}

fn parse_dependency<'a>(
    anchors: &mut AnchorMap,
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Dependency, ContextError> {
    tagged_mapping_start("ruby/object:Gem::Dependency").parse_next(input)?;
    let fields = parse_dependency_fields(anchors, input);
    mapping_end.parse_next(input)?;
    fields
}

fn parse_dependency_fields<'a>(
    anchors: &mut AnchorMap,
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Dependency, ContextError> {
    let mut name: Option<String> = None;
    let mut requirement: Option<Requirement> = None;
    let mut dep_type = DependencyType::Runtime; // default

    // Parse all fields in the dependency until we would hit mapping_end
    loop {
        // Check if we can peek at the next event to see if it's mapping_end
        match peek(any::<_, ContextError>).parse_next(input) {
            Ok((Event::MappingEnd, _)) => break,
            Ok(_) => {
                // Continue parsing key-value pairs
            }
            Err(_) => break, // End of stream
        }

        // Parse key-value pair
        let key = string.parse_next(input)?;
        match key.as_str() {
            "name" => {
                name = Some(string.parse_next(input)?);
            }
            "requirement" => {
                requirement = Some(parse_requirement(anchors, input)?);
            }
            // Handle older gem specification field names
            "version_requirements" => {
                requirement = match requirement {
                    Some(r) => {
                        skip_value.parse_next(input)?;
                        Some(r)
                    }
                    None => Some(parse_requirement(anchors, input)?),
                };
            }
            "type" => {
                let type_str = string.parse_next(input)?;
                dep_type = match type_str.as_str() {
                    ":development" => DependencyType::Development,
                    ":runtime" => DependencyType::Runtime,
                    _ => DependencyType::Runtime, // default fallback
                };
            }
            "prerelease" => {
                skip_value.parse_next(input)?;
            }
            _ => {
                // Skip unknown fields
                skip_value.parse_next(input)?;
            }
        }
    }

    let name = name.ok_or_else(|| ErrMode::Cut(ParserError::assert(input, "name missing")))?;
    let requirement = requirement
        .ok_or_else(|| ErrMode::Cut(ParserError::assert(input, "requirement missing")))?;

    Ok(Dependency {
        name,
        requirement,
        dep_type,
    })
}

fn parse_dependencies<'a>(
    anchors: &mut AnchorMap,
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Vec<Dependency>, ContextError> {
    let _ = sequence_start.parse_next(input)?;
    let mut deps = Vec::new();
    while let Ok(dep) = parse_dependency(anchors, input) {
        deps.push(dep);
    }
    sequence_end.parse_next(input)?;
    Ok(deps)
}

fn parse_gem_specification_winnow<'a>(
    input: &mut &'a [(Event<'a>, Span)],
) -> ModalResult<Specification, ContextError> {
    let anchors: &mut AnchorMap = &mut Default::default();

    // Skip stream/document start events
    let _ = opt(stream_start).parse_next(input)?;
    let _ = opt(document_start).parse_next(input)?;

    // Parse the main specification mapping
    tagged_mapping_start("ruby/object:Gem::Specification")
        .context(StrContext::Expected(StrContextValue::Description(
            "Gem::Specification root object",
        )))
        .parse_next(input)?;

    // Parse all fields in a more flexible way
    let mut name: Option<String> = None;
    let mut version: Option<Version> = None;
    let mut authors: Vec<Option<String>> = Vec::new();
    let mut email: Vec<Option<String>> = Vec::new();
    let mut homepage: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut description: Option<String> = None;
    let mut licenses: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut executables: Vec<String> = Vec::new();
    let mut extensions: Vec<String> = Vec::new();
    let mut dependencies: Vec<Dependency> = Vec::new();
    let mut metadata: IndexMap<String, String> = Default::default();
    let mut platform: Option<String> = None;
    let mut bindir: Option<String> = None;
    let mut post_install_message: Option<String> = None;
    let mut requirements: Vec<String> = Vec::new();
    let mut required_ruby_version: Option<Requirement> = None;
    let mut required_rubygems_version: Option<Requirement> = None;
    let mut test_files: Vec<String> = Vec::new();
    let mut extra_rdoc_files: Vec<String> = Vec::new();
    let mut rdoc_options: Vec<String> = Vec::new();
    let mut cert_chain: Vec<String> = Vec::new();
    let mut signing_key: Option<String> = None;
    let mut autorequire: Option<String> = None;
    let mut require_paths: Option<Vec<String>> = None;
    let mut rubygems_version: Option<String> = None;
    let mut specification_version: Option<i32> = None;
    let mut date: Option<String> = None;

    // Parse all fields until we hit mapping end or document/stream end
    loop {
        // Check if we're at the end of the mapping or document
        match peek(any::<_, ContextError>).parse_next(input) {
            Ok((Event::MappingEnd, _)) => break,
            Ok(_) => {
                // Continue parsing key-value pairs
            }
            Err(_) => break, // End of stream
        }

        // Parse key-value pairs
        let key = string
            .context(StrContext::Expected(StrContextValue::Description(
                "field name",
            )))
            .parse_next(input)?;
        match key.as_str() {
            "name" => {
                name = Some(
                    string
                        .context(StrContext::Expected(StrContextValue::Description(
                            "gem name string",
                        )))
                        .parse_next(input)?,
                );
            }
            "version" => {
                version = Some(
                    parse_version
                        .context(StrContext::Expected(StrContextValue::Description(
                            "Gem::Version object",
                        )))
                        .parse_next(input)?,
                );
            }
            "authors" => {
                authors = parse_optional_string_array.parse_next(input)?;
            }
            "dependencies" => {
                dependencies = parse_dependencies(anchors, input)?;
            }
            "cert_chain" => {
                cert_chain = parse_string_array.parse_next(input)?;
            }
            "executables" => {
                executables = parse_string_array.parse_next(input)?;
            }
            "extensions" => {
                extensions = parse_string_array.parse_next(input)?;
            }
            "extra_rdoc_files" => {
                extra_rdoc_files = parse_string_array.parse_next(input)?;
            }
            "files" => {
                files = parse_string_array.parse_next(input)?;
            }
            "licenses" => {
                licenses = parse_string_array.parse_next(input)?;
            }
            "rdoc_options" => {
                rdoc_options = parse_string_array.parse_next(input)?;
            }
            "require_paths" => {
                require_paths = Some(parse_string_array.parse_next(input)?);
            }
            "requirements" => {
                requirements = parse_string_array.parse_next(input)?;
            }
            "test_files" => {
                test_files = parse_string_array.parse_next(input)?;
            }
            "required_ruby_version" => {
                required_ruby_version = Some(parse_requirement(anchors, input)?);
            }
            "required_rubygems_version" => {
                required_rubygems_version = Some(parse_requirement(anchors, input)?);
            }
            "metadata" => {
                metadata = parse_metadata_as_map.parse_next(input)?;
            }
            "email" => {
                email = parse_optional_string_array.parse_next(input)?;
            }
            "homepage" => {
                homepage = optional_string.parse_next(input)?;
            }
            "summary" => {
                summary = optional_string.parse_next(input)?;
            }
            "description" => {
                description = optional_string.parse_next(input)?;
            }
            "platform" => {
                platform = optional_string.parse_next(input)?;
            }
            "bindir" => {
                bindir = optional_string.parse_next(input)?;
            }
            "post_install_message" => {
                post_install_message = optional_string.parse_next(input)?;
            }
            "signing_key" => {
                signing_key = optional_string.parse_next(input)?;
            }
            "autorequire" => {
                autorequire = optional_string.parse_next(input)?;
            }
            "rubygems_version" => {
                rubygems_version = optional_string.parse_next(input)?;
            }
            "date" => {
                date = optional_string.parse_next(input)?;
            }
            "specification_version" => {
                let version: i32 = scalar_event
                    .verify_map(|s| s.as_integer())
                    .verify_map(|s| s.try_into().ok())
                    .parse_next(input)?;
                specification_version = Some(version);
            }
            _ => {
                // Skip all other fields for now
                skip_value.parse_next(input)?;
            }
        }
    }

    // Consume the mapping end if present
    let _ = opt(mapping_end).parse_next(input)?;

    // Skip document/stream end events
    let _ = opt(document_end).parse_next(input)?;
    let _ = opt(stream_end).parse_next(input)?;

    // Create the specification with required fields
    let name = name.ok_or_else(|| ErrMode::Cut(ContextError::new()))?;
    let version = version.ok_or_else(|| ErrMode::Cut(ContextError::new()))?;

    let mut spec =
        Specification::new(name, version).map_err(|_e| ErrMode::Cut(ContextError::new()))?;

    // Set all the parsed fields
    spec.authors = authors;
    spec.email = email;
    spec.homepage = homepage;
    spec.description = description;
    spec.licenses = licenses;
    spec.files = files;
    spec.executables = executables;
    spec.extensions = extensions;
    spec.dependencies = dependencies;
    spec.metadata = metadata;
    spec.cert_chain = cert_chain;
    spec.signing_key = signing_key;
    spec.autorequire = autorequire;
    spec.requirements = requirements;
    spec.test_files = test_files;
    spec.extra_rdoc_files = extra_rdoc_files;
    spec.rdoc_options = rdoc_options;
    spec.post_install_message = post_install_message;

    // Set optional fields with defaults if not provided
    if let Some(paths) = require_paths {
        spec.require_paths = paths;
    }
    if let Some(summary_val) = summary {
        spec.summary = summary_val;
    }
    if let Some(bindir_val) = bindir {
        spec.bindir = bindir_val;
    }
    if let Some(platform_str) = platform {
        // Parse platform string into Platform enum
        spec.platform = platform_str.parse().unwrap_or(Platform::Ruby);
    }
    if let Some(required_ruby) = required_ruby_version {
        spec.required_ruby_version = required_ruby;
    }
    if let Some(required_rubygems) = required_rubygems_version {
        spec.required_rubygems_version = required_rubygems;
    }
    if let Some(rubygems_ver) = rubygems_version {
        spec.rubygems_version = rubygems_ver;
    }
    if let Some(spec_ver) = specification_version {
        spec.specification_version = spec_ver;
    }
    if let Some(date_val) = date {
        spec.date = date_val;
    }

    Ok(spec)
}

fn parse_winnow(yaml_str: &str) -> Result<Specification> {
    let events = parse_yaml_events(yaml_str)?;
    let mut input = events.as_slice();

    match parse_gem_specification_winnow(&mut input) {
        Ok(spec) => Ok(spec),
        Err(err) => {
            // Convert winnow errors to our DeserializationError with better context
            let (expected, found, span_start, span_length) =
                get_error_details(&events, input, &err);

            let error = match err {
                ErrMode::Incomplete(_) => DeserializationError::UnexpectedEnd {
                    message: "Incomplete YAML input".to_string(),
                    bad_bit: SourceSpan::new(span_start.into(), span_length),
                },
                ErrMode::Backtrack(_) | ErrMode::Cut(_) => DeserializationError::ExpectedEvent {
                    expected,
                    found,
                    bad_bit: SourceSpan::new(span_start.into(), span_length),
                },
            };
            Err(error.into())
        }
    }
}

fn get_error_details(
    events: &[(Event, Span)],
    remaining_input: &[(Event, Span)],
    err: &ErrMode<ContextError>,
) -> (String, String, usize, usize) {
    let expected = match err {
        ErrMode::Incomplete(_) => "more YAML content".to_string(),
        ErrMode::Cut(context_err) | ErrMode::Backtrack(context_err) => {
            // Extract more meaningful context from winnow's ContextError
            if let Some(context) = context_err.context().next() {
                match context {
                    winnow::error::StrContext::Label(label) => label.to_string(),
                    winnow::error::StrContext::Expected(expected) => format!("{expected}"),
                    _ => "valid YAML structure".to_string(),
                }
            } else {
                "valid YAML structure".to_string()
            }
        }
    };

    let (found, span_start, span_length) = if let Some((event, span)) = remaining_input.first() {
        let start_idx = span.start.index();
        let end_idx = span.end.index();
        let length = if end_idx > start_idx {
            end_idx - start_idx
        } else {
            1
        };
        let found_description = match event {
            Event::StreamStart => "stream start".to_string(),
            Event::StreamEnd => "stream end".to_string(),
            Event::DocumentStart(_) => "document start".to_string(),
            Event::DocumentEnd => "document end".to_string(),
            Event::MappingStart(anchor_id, Some(tag)) => {
                if *anchor_id > 0 {
                    format!(
                        "mapping start with tag '{}' and anchor '{}'",
                        tag.suffix, anchor_id
                    )
                } else {
                    format!("mapping start with tag '{}'", tag.suffix)
                }
            }
            Event::MappingStart(_, None) => "mapping start".to_string(),
            Event::MappingEnd => "mapping end".to_string(),
            Event::SequenceStart(anchor_id, Some(tag)) => {
                if *anchor_id > 0 {
                    format!(
                        "sequence start with tag '{}' and anchor '{}'",
                        tag.suffix, anchor_id
                    )
                } else {
                    format!("sequence start with tag '{}'", tag.suffix)
                }
            }
            Event::SequenceStart(_, None) => "sequence start".to_string(),
            Event::SequenceEnd => "sequence end".to_string(),
            Event::Scalar(value, _, _, _) => format!("scalar value '{value}'"),
            Event::Alias(value) => format!("alias id '{value}'"),
            Event::Nothing => "nothing".to_string(),
        };
        (found_description, start_idx, length)
    } else {
        ("end of stream".to_string(), events.len(), 1)
    };

    (expected, found, span_start, span_length)
}

pub fn parse(yaml_str: &str) -> Result<Specification> {
    // If input string has a line containing only "'", it's (hopefully) one way to detect a wrongly
    // indented multiline quoted scalar. Correct the indentation so that gemspecs with this issue
    // still parse fine
    let amended_yaml_str = yaml_str.replacen("\n'\n", "\n  '\n", 1);

    parse_winnow(&amended_yaml_str).map_err(|e| e.with_source_code(yaml_str.to_string()))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_simple_yaml_parsing() {
        let yaml = r#"--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
"#;

        let spec = parse(yaml).expect("Failed to parse simple YAML");
        assert_eq!(spec.name, "test-gem");
        assert_eq!(spec.version.to_string(), "1.0.0");
    }

    #[test]
    fn test_yaml_with_authors() {
        let yaml = r#"--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
authors:
- Test Author
"#;

        let spec = parse(yaml).expect("Failed to parse YAML with authors");
        assert_eq!(spec.name, "test-gem");
        assert_eq!(spec.version.to_string(), "1.0.0");
        assert_eq!(spec.authors, vec![Some("Test Author".to_string())]);
    }

    #[test]
    fn test_invalid_yaml_scan_error() {
        // Test that malformed YAML produces a Parse error with proper span information
        let yaml = "invalid yaml: [unclosed";
        let parse_result = parse(yaml);
        let err = parse_result.unwrap_err();

        // Check that it's a YAML parsing error - the details are in the diagnostic output
        let error_msg = format!("{err}");
        assert!(error_msg.contains("YAML parsing error"));

        // The detailed error with location is available via the Debug format or diagnostic chain
        let debug_msg = format!("{err:?}");
        assert!(debug_msg.contains("flow sequence") || debug_msg.contains("expected"));
    }
}
