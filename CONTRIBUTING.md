# Contributing to Zirc

Thank you for your interest in contributing to the Zirc programming language! This document provides guidelines and information for contributors.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Development Setup](#development-setup)
3. [Project Structure](#project-structure)
4. [Contributing Guidelines](#contributing-guidelines)
5. [Testing](#testing)
6. [Code Style](#code-style)
7. [Submitting Changes](#submitting-changes)
8. [Areas for Contribution](#areas-for-contribution)

## Getting Started

### Ways to Contribute

- **Bug Reports**: Found a bug? Report it!
- **Feature Requests**: Have an idea for a new feature?
- **Code Contributions**: Fix bugs or implement features
- **Documentation**: Improve docs, tutorials, or examples
- **Testing**: Write tests or test edge cases
- **Community**: Help others in discussions

### Before You Start

1. Check existing [issues](https://github.com/zirc-industries/zirc/issues) and [pull requests](https://github.com/zirc-industries/zirc/pulls)
2. For major changes, open an issue first to discuss the approach
3. Make sure you understand the project's goals and scope

## Development Setup

### Prerequisites

- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **Git**: For version control
- **Text Editor**: VS Code, Vim, or your preferred editor

### Clone and Build

```bash
# Clone the repository
git clone https://github.com/zirc-industries/zirc.git
cd zirc-language

# Build the project
cargo build

# Run tests
cargo test --workspace

# Try the REPL
cargo run -p zirc-cli --bin zirc-repl

# Run an example
cargo run -p zirc-cli --bin zirc examples/basic/hello.zirc
```

### Development Commands

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test --package zirc-lexer

# Run with the VM backend (canonical default)
cargo run -p zirc-cli --bin zirc -- --backend vm program.zirc

# Run with the reference interpreter backend
cargo run -p zirc-cli --bin zirc -- --backend interp program.zirc

# Format code
cargo fmt

# Check code with Clippy
cargo clippy

# Run examples
./scripts/run-examples.sh  # or .bat on Windows
```

## Project Structure

```
zirc-language/
├── crates/
│   ├── zirc-syntax/     # AST definitions and tokens
│   ├── zirc-lexer/      # Tokenizer/lexer
│   ├── zirc-parser/     # Parser (tokens → AST)
│   ├── zirc-interpreter/# Tree-walking interpreter
│   ├── zirc-bytecode/   # Bytecode definitions
│   ├── zirc-compiler/   # AST → bytecode compiler
│   ├── zirc-vm/         # Virtual machine
│   ├── zirc-cli/        # Command-line interface (binaries: zirc, zirc-repl)
│   ├── zirc-fmt/        # Code formatter
│   ├── zirc-lint/       # Static analysis linter
│   ├── zirc-lsp/        # Language server
│   └── zirc-bench/      # Benchmarking tools
├── examples/            # Example Zirc programs
├── stdlib/              # Standard library modules (.zirc)
├── scripts/             # Build and utility scripts
├── docs/                # Documentation
│   └── LANGUAGE_SPEC.md # Language specification
├── README.md           # Project overview
└── CONTRIBUTING.md     # This file
```

### Architecture Overview

1. **Lexer** (`zirc-lexer`): Converts source code into tokens
2. **Parser** (`zirc-parser`): Converts tokens into an Abstract Syntax Tree (AST)
3. **Interpreter** (`zirc-interpreter`): Directly executes the AST
4. **Compiler** (`zirc-compiler`): Converts AST to bytecode
5. **VM** (`zirc-vm`): Executes bytecode efficiently
6. **CLI** (`zirc-cli`): User interface that ties everything together

## Contributing Guidelines

### Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help others learn and grow
- Follow the project's technical standards

### Issue Guidelines

When reporting bugs:
- Use a clear, descriptive title
- Provide steps to reproduce
- Include code examples when relevant
- Specify your environment (OS, Rust version)
- Include error messages and stack traces

For feature requests:
- Explain the use case and motivation
- Consider how it fits with existing features
- Provide examples of how it would be used
- Think about potential implementation challenges

### Pull Request Guidelines

1. **Fork and Branch**: Create a feature branch from `main`
2. **Small Changes**: Keep PRs focused and reasonably sized
3. **Tests**: Add tests for new functionality
4. **Documentation**: Update docs when needed
5. **Commit Messages**: Use clear, descriptive commit messages
6. **CI**: Ensure all tests and checks pass

## Testing

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test --package zirc-lexer

# Specific test
cargo test test_string_operations

# With output
cargo test -- --nocapture
```

### Types of Tests

1. **Unit Tests**: Test individual functions/modules
2. **Integration Tests**: Test component interactions
3. **Example Tests**: Ensure example programs work
4. **Property Tests**: Test with generated inputs (future)

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_name() {
        // Arrange
        let input = "test input";
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected_output);
    }
}
```

### Test Organization

- Unit tests: In the same file as the code being tested
- Integration tests: In `tests/` directory or `lib.rs` test modules
- Example tests: In `crates/zirc-cli/tests/examples.rs`

## Code Style

### Rust Style Guidelines

Follow standard Rust conventions:

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy
```

### Naming Conventions

- **Functions**: `snake_case`
- **Variables**: `snake_case`
- **Types**: `PascalCase`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`

### Documentation

```rust
/// Brief description of the function.
///
/// More detailed explanation if needed.
/// 
/// # Arguments
///
/// * `param1` - Description of parameter
/// * `param2` - Description of parameter
///
/// # Returns
///
/// Description of return value
///
/// # Examples
///
/// ```
/// let result = function_name(arg1, arg2);
/// assert_eq!(result, expected);
/// ```
pub fn function_name(param1: Type1, param2: Type2) -> ReturnType {
    // Implementation
}
```

### Error Handling

- Use `Result<T, Error>` for fallible operations
- Provide helpful error messages
- Include source location information when possible

## Submitting Changes

### Commit Messages

Use the conventional commit format:

```
type(scope): brief description

Longer description if needed.

Fixes #issue_number
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Examples:
- `feat(lexer): add string escape sequence support`
- `fix(parser): handle empty function parameter lists`
- `docs: update installation instructions`

### Pull Request Process

1. **Create a Branch**
   ```bash
   git checkout -b feature/my-feature
   ```

2. **Make Changes**
   - Write code
   - Add tests
   - Update documentation

3. **Test Locally**
   ```bash
   cargo test --workspace
   cargo fmt
   cargo clippy
   ```

4. **Commit Changes**
   ```bash
   git add .
   git commit -m "feat: add new feature"
   ```

5. **Push and Create PR**
   ```bash
   git push origin feature/my-feature
   ```
   Then create a pull request on GitHub.

6. **Address Review Feedback**
   - Make requested changes
   - Push updates to the same branch

## Areas for Contribution

### Beginner-Friendly

- **Documentation**: Fix typos, add examples, improve clarity
- **Examples**: Create more example programs
- **Tests**: Add test cases for edge conditions
- **Error Messages**: Improve error message quality

### Intermediate

- **Built-in Functions**: Add new standard library functions
- **Language Features**: Implement missing operators or constructs
- **CLI Improvements**: Better REPL features, command-line options
- **Performance**: Profile and optimize hot paths

### Advanced

- **Language Design**: Propose and implement new language features
- **VM Optimization**: Improve bytecode execution performance
- **Type System**: Enhance type checking and inference
- **Tooling**: Language server, debugger, profiler

### Current Priorities

Check the [GitHub issues](https://github.com/zirc-industries/zirc/issues) for current priorities and good first issues.

## Getting Help

- **Discord/Chat**: Join our development chat (link in README)
- **GitHub Issues**: Ask questions in issue comments
- **Documentation**: Check existing docs and specs
- **Code**: Read the codebase to understand patterns

## Recognition

Contributors will be recognized in:
- The project's README
- Release notes for significant contributions
- The project's contributor list

Thank you for contributing to Zirc! 🎉
