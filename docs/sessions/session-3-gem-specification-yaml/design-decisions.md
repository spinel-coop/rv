# Session 3: rv-gem-specification-yaml Design Decisions

## Overview
Create a new crate `rv-gem-specification-yaml` that provides YAML serialization/deserialization for `Gem::Specification` objects with full Ruby compatibility.

## Design Decisions

### 1. Event-Based YAML Parsing with saphyr-parser
**Decision**: Use `saphyr-parser` events for YAML parsing instead of serde or high-level saphyr APIs
**Rationale**: Ruby's YAML output uses tags and custom serialization patterns that require strict validation. Event-based parsing provides:
- Precise source location tracking for error reporting
- Strict validation of expected tags and types
- Better error messages with miette integration
- Full control over parsing flow
**Alternatives**:
- serde_yaml: Too rigid for Ruby's tag-based serialization
- saphyr high-level API: Less strict, harder to provide precise error locations
- yaml-rust2: Similar limitations to serde

**Implementation Strategy**: Follow gemspec-rs pattern with event-based parsing

### 2. Test-Driven Development with Ruby Fixtures
**Decision**: Generate test cases by creating actual Ruby `Gem::Specification` objects and dumping their YAML
**Rationale**: Ensures compatibility with real-world Ruby output rather than guessing at the format
**Implementation**: Use insta snapshots for regression testing

### 3. Separate Crate Architecture
**Decision**: Create a dedicated crate for YAML functionality
**Rationale**:
- Keeps YAML dependencies isolated
- Allows optional YAML support in main library
- Follows Rust ecosystem patterns (e.g., serde_json vs serde)

### 4. Strict Parser with miette Error Reporting
**Decision**: Implement strict parsing that fails on unexpected types or tags, with detailed error reporting using miette
**Rationale**:
- Ruby's YAML has a well-defined structure that should be validated strictly
- Better error messages help users identify malformed YAML quickly
- Source span tracking allows pinpointing exact error locations
- Prevents silent data corruption from mismatched types
**Trade-offs**: Less permissive than lenient parsing, but much safer and user-friendly

### 5. Event-Based Parsing Architecture
**Decision**: Use saphyr-parser events to build a streaming parser
**Benefits**:
- Memory efficient for large YAML files
- Precise error location tracking
- Strict validation at each parsing step
- Can provide context-aware error messages
**Implementation**: Parser state machine that validates structure and types progressively

## Ruby YAML Characteristics to Handle
- Object tags: `!ruby/object:Gem::Specification`, `!ruby/object:Gem::Version`, `!ruby/object:Gem::Requirement`, `!ruby/object:Gem::Dependency`
- Array serialization patterns
- Nil value handling
- Symbol vs string key differences
- Strict type validation (strings vs numbers vs booleans)
- Nested tag validation (versions within requirements, etc.)

## 6. Round-Trip Testing Strategy
**Decision**: Implement both parsing and serialization to enable round-trip testing
**Rationale**:
- Ensures our implementation maintains full fidelity with Ruby's YAML format
- Validates that we can read Ruby YAML and produce equivalent Ruby YAML
- Provides confidence in interoperability with Ruby tools
**Implementation**:
- Add YAML serialization alongside deserialization
- Create tests that parse Ruby YAML, serialize it back, and compare results
- Test both semantic equivalence (parsed objects) and format preservation (YAML structure)

**Trade-offs**:
- Additional implementation complexity for serialization
- Benefit: Higher confidence in Ruby compatibility and bidirectional interoperability

## 7. Lazy Event Parsing with Parser Storage
**Decision**: Store the saphyr-parser `Parser` instance instead of eagerly collecting all events into a Vec
**Rationale**:
- **Performance**: Avoids upfront memory allocation for all events
- **Memory efficiency**: Events are processed one at a time instead of stored in memory
- **Span access**: Enables precise error reporting with exact source locations
- **Error handling**: Parser errors can be reported with accurate position information

**Implementation Details**:
- `GemSpecificationParser` stores `Parser<'a, StrInput<'a>>` and `current_event: Option<(Event<'a>, Span)>`
- Added `peek()` method for event lookahead without advancing parser
- Changed `advance()` to return `Result<bool>` instead of cloned events
- Integrated `NamedSource<String>` for better error context instead of separate source + name

**Error Type Improvements**:
- Replaced generic `Structure` errors with specific `ExpectedEvent`, `UnexpectedTag`, `UnexpectedEnd`
- All errors include precise span information using `span.start.index()` for accurate positioning
- Better error messages describing what was expected vs found

**Challenges Encountered**:
- Parser positioning requires careful state management in the main parsing loop
- `expect_*` methods must coordinate with main loop advancement correctly
- Complex nested object parsing (version objects) requires proper positioning after completion

**Status**: Core lazy parsing infrastructure completed. Main parsing loop positioning needs refinement.

**Benefits Achieved**:
- ✅ Lazy event pulling instead of eager collection
- ✅ Span-based error reporting with precise source locations
- ✅ More specific error types for better diagnostics
- ✅ Avoided unnecessary event cloning
- ✅ `NamedSource` integration for better error context


## 8. Winnow-Based Parser Combinator Architecture

**Decision**: Use winnow parser combinator library with simplified architecture based on a single `YamlParser<T, 'a>` trait and type-driven specialization
**Rationale**:
- Winnow provides battle-tested combinator patterns (`many0`, `separated_list0`, `opt`, `alt`, etc.)
- Rich error handling with context built-in
- Zero-cost abstractions designed for performance
- Familiar API that Rust developers already know from nom/winnow ecosystem
- Single trait design is simpler than complex trait hierarchies
- Parser combinator patterns naturally compose like we need
- Type-driven specialization eliminates need for boxed trait objects

### 9.1 Simplified Single-Trait Design

**Core Design**: Single `YamlParser<T, 'a>` trait with type-driven specialization
```rust
trait YamlParser<T, 'a> {
    type Error: std::error::Error + Send + Sync + miette::Diagnostic + 'static;
    
    fn parse(&mut self) -> Result<T, Self::Error>;
    
    // Base functionality for event access
    fn current_event(&self) -> Option<&Event<'a>>;
    fn advance(&mut self) -> Result<bool, Self::Error>;
    fn current_span(&self) -> Option<Span>;
}

// Specialized implementations for different types
impl YamlParser<String, 'a> for MyParser { /* parse scalars */ }
impl YamlParser<Version, 'a> for MyParser { /* parse tagged version objects */ }
impl YamlParser<Vec<String>, 'a> for MyParser { /* parse string sequences */ }
impl YamlParser<IndexMap<String, String>, 'a> for MyParser { /* parse mappings */ }
impl YamlParser<Requirement, 'a> for MyParser { /* parse requirement objects */ }
```

### 9.2 Winnow Stream Integration

**YAML Event Stream**: Implement winnow's `Stream` trait for our event iterator
```rust
use winnow::prelude::*;
use winnow::stream::Stream;

struct YamlEventStream<'a> {
    parser: Parser<'a, StrInput<'a>>,
    current_event: Option<(Event<'a>, Span)>,
    named_source: NamedSource<String>,
}

impl<'a> Stream for YamlEventStream<'a> {
    type Token = Event<'a>;
    type Slice = &'a [Event<'a>];
    type IterOffsets = std::iter::Enumerate<std::slice::Iter<'a, Event<'a>>>;
    type Checkpoint = usize;
    
    fn iter_offsets(&self) -> Self::IterOffsets {
        // Provide iterator over events with offsets
    }
    
    fn eof_offset(&self) -> usize {
        // Return end-of-stream offset
    }
    
    fn next_token(&mut self) -> Option<Self::Token> {
        // Advance to next event and return it
        self.advance().ok().and_then(|_| {
            self.current_event.as_ref().map(|(event, _)| event.clone())
        })
    }
    
    fn checkpoint(&self) -> Self::Checkpoint {
        // Create parsing checkpoint for backtracking
    }
    
    fn reset(&mut self, checkpoint: Self::Checkpoint) {
        // Reset stream to checkpoint (for backtracking)
    }
}
```

### 9.3 Winnow-Based Parser Functions

**Basic Event Parsers**: Low-level parsers for YAML events
```rust
use winnow::combinator::*;
use winnow::token::*;

// Parse specific event types
fn scalar_event<'a>(input: &mut YamlEventStream<'a>) -> PResult<String> {
    any.verify_map(|event| match event {
        Event::Scalar(value, _, _, _) => Some(value.to_string()),
        _ => None,
    }).parse_next(input)
}

fn mapping_start<'a>(input: &mut YamlEventStream<'a>) -> PResult<()> {
    any.verify_map(|event| match event {
        Event::MappingStart(_, _) => Some(()),
        _ => None,
    }).parse_next(input)
}

fn mapping_end<'a>(input: &mut YamlEventStream<'a>) -> PResult<()> {
    any.verify_map(|event| match event {
        Event::MappingEnd => Some(()),
        _ => None,
    }).parse_next(input)
}

fn sequence_start<'a>(input: &mut YamlEventStream<'a>) -> PResult<()> {
    any.verify_map(|event| match event {
        Event::SequenceStart(_, _) => Some(()),
        _ => None,
    }).parse_next(input)
}

fn sequence_end<'a>(input: &mut YamlEventStream<'a>) -> PResult<()> {
    any.verify_map(|event| match event {
        Event::SequenceEnd => Some(()),
        _ => None,
    }).parse_next(input)
}

// Parse tagged objects
fn tagged_mapping_start<'a>(expected_tag: &str) -> impl FnMut(&mut YamlEventStream<'a>) -> PResult<()> {
    move |input| {
        any.verify_map(|event| match event {
            Event::MappingStart(_, Some(tag)) if tag.suffix == expected_tag => Some(()),
            _ => None,
        }).parse_next(input)
    }
}
```

### 9.4 Combinator-Based Object Parsers

**Ruby Object Parsing**: Using winnow combinators for complex structures
```rust
// Parse Gem::Version objects
fn parse_version<'a>(input: &mut YamlEventStream<'a>) -> PResult<Version> {
    preceded(
        tagged_mapping_start("ruby/object:Gem::Version"),
        delimited(
            success(()),  // Already consumed MappingStart
            parse_version_fields,
            mapping_end
        )
    ).parse_next(input)
}

fn parse_version_fields<'a>(input: &mut YamlEventStream<'a>) -> PResult<Version> {
    let mut version_str: Option<String> = None;
    
    repeat(0.., (
        scalar_event,  // Parse key
        alt((
            preceded(literal("version"), scalar_event).map(Some),
            preceded(scalar_event, skip_value).map(|_| None),  // Skip unknown fields
        ))
    )).fold(|| (), |_, (key, value)| {
        if key == "version" {
            if let Some(v) = value {
                version_str = Some(v);
            }
        }
    }).parse_next(input)?;
    
    let version_str = version_str.ok_or_else(|| {
        ErrMode::Cut(ContextError::new().add_context("version", "missing required field"))
    })?;
    
    Version::new(&version_str)
        .map_err(|e| ErrMode::Cut(ContextError::new().add_context("version", e)))
}

// Parse sequences using built-in combinators
fn parse_string_array<'a>(input: &mut YamlEventStream<'a>) -> PResult<Vec<String>> {
    delimited(
        sequence_start,
        many0(scalar_event),
        sequence_end
    ).parse_next(input)
}

// Parse dependencies with nested structure
fn parse_dependencies<'a>(input: &mut YamlEventStream<'a>) -> PResult<Vec<Dependency>> {
    delimited(
        sequence_start,
        many0(parse_dependency),
        sequence_end
    ).parse_next(input)
}

fn parse_dependency<'a>(input: &mut YamlEventStream<'a>) -> PResult<Dependency> {
    preceded(
        tagged_mapping_start("ruby/object:Gem::Dependency"),
        delimited(
            success(()),
            parse_dependency_fields,
            mapping_end
        )
    ).parse_next(input)
}

// Parse requirements with complex nested constraints
fn parse_requirement<'a>(input: &mut YamlEventStream<'a>) -> PResult<Requirement> {
    preceded(
        tagged_mapping_start("ruby/object:Gem::Requirement"),
        delimited(
            success(()),
            preceded(
                (literal("requirements"), sequence_start),
                terminated(
                    many0(parse_constraint_pair),
                    (sequence_end, mapping_end)
                )
            )
        )
    ).map(|constraints| {
        if constraints.is_empty() {
            Requirement::new(vec![">= 0".to_string()])
        } else {
            Requirement::new(constraints)
        }
    }).parse_next(input)
}

fn parse_constraint_pair<'a>(input: &mut YamlEventStream<'a>) -> PResult<String> {
    delimited(
        sequence_start,
        (scalar_event, parse_version),  // [operator, version]
        sequence_end
    ).map(|(op, version)| format!("{} {}", op, version))
    .parse_next(input)
}
```

### 9.5 Document-Level Parser

**Top-Level Integration**: Document parsing with winnow
```rust
fn parse_gem_specification<'a>(input: &mut YamlEventStream<'a>) -> PResult<Specification> {
    preceded(
        stream_start,
        terminated(
            preceded(
                tagged_mapping_start("ruby/object:Gem::Specification"),
                delimited(
                    success(()),
                    parse_specification_fields,
                    mapping_end
                )
            ),
            stream_end
        )
    ).parse_next(input)
}

fn parse_specification_fields<'a>(input: &mut YamlEventStream<'a>) -> PResult<Specification> {
    let mut builder = SpecificationBuilder::new();
    
    repeat(0.., (
        scalar_event,  // Field name
        dispatch! {scalar_event;
            "name" => scalar_event.map(|s| builder.set_name(s)),
            "version" => parse_version.map(|v| builder.set_version(v)),
            "dependencies" => parse_dependencies.map(|d| builder.set_dependencies(d)),
            "authors" => parse_string_array.map(|a| builder.set_authors(a)),
            "requirements" => parse_requirement.map(|r| builder.set_required_ruby_version(r)),
            // ... other fields
            _ => skip_value.map(|_| ()),  // Skip unknown fields
        }
    )).parse_next(input)?;
    
    builder.build()
        .map_err(|e| ErrMode::Cut(ContextError::new().add_context("specification", e)))
}
```

### 9.6 Error Integration with miette

**Rich Error Reporting**: Combining winnow errors with miette diagnostics
```rust
use winnow::error::{ErrMode, ErrorKind, ContextError};

// Convert winnow errors to our miette-based errors
impl From<ErrMode<ContextError>> for DeserializationError {
    fn from(err: ErrMode<ContextError>) -> Self {
        match err {
            ErrMode::Incomplete(_) => DeserializationError::UnexpectedEnd {
                message: "Incomplete input".to_string(),
                src: named_source.clone(),
                bad_bit: SourceSpan::new(0.into(), 1),
            },
            ErrMode::Backtrack(ctx) | ErrMode::Cut(ctx) => {
                DeserializationError::ExpectedEvent {
                    expected: ctx.to_string(),
                    found: "unexpected event".to_string(),
                    src: named_source.clone(),
                    bad_bit: SourceSpan::new(current_span.start.index().into(), 1),
                }
            }
        }
    }
}
```

### 9.7 Benefits of Winnow Integration

- **Proven Patterns**: Battle-tested combinator library with established patterns
- **Rich Error Context**: Built-in error context and recovery mechanisms  
- **Zero Cost**: Compiled to efficient code without runtime overhead
- **Familiar API**: Developers already know nom/winnow patterns
- **Composability**: Easy to build complex parsers from simple combinators
- **Backtracking**: Built-in support for alternative parsing paths
- **Streaming**: Designed for efficient streaming parsing

**Migration Path**: Start with basic event parsers, then build up combinator-based object parsers, maintaining the existing API surface while transitioning to winnow internally.
