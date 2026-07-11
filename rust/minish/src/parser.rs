//! # Command Line Parser
//!
//! Tokenizes a shell command line into structured `Command` and `Pipeline` types.
//!
//! ## Supported syntax
//!
//! ```text
//! cmd arg1 arg2              — simple command
//! cmd1 | cmd2 arg            — pipeline
//! cmd > file                 — stdout redirect (truncate)
//! cmd >> file                — stdout redirect (append)
//! cmd < file                 — stdin redirect
//! cmd 2> file                — stderr redirect
//! cmd &                      — background
//! cmd1 && cmd2               — conditional AND
//! cmd1 || cmd2               — conditional OR
//! "quoted string"            — quoted argument
//! 'single quoted'            — single-quoted argument
//! ```

/// A single command with arguments and redirections.
#[derive(Debug, Clone)]
pub struct Command {
    /// Command arguments (argv[0] = command name).
    pub args: Vec<String>,
    /// Optional stdin redirect file.
    pub stdin_file: Option<String>,
    /// Optional stdout redirect file (truncate).
    pub stdout_file: Option<String>,
    /// Optional stdout redirect file (append).
    pub stdout_append: Option<String>,
    /// Optional stderr redirect file.
    pub stderr_file: Option<String>,
    /// Run in background.
    pub background: bool,
}

/// A pipeline of commands connected by `|`.
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// Commands in the pipeline (left-to-right).
    pub commands: Vec<Command>,
    /// Conditional operator: None, And (&&), Or (||)
    pub conditional: Option<Conditional>,
}

/// Conditional operator between pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conditional {
    And,
    Or,
}

impl Command {
    /// Create a new empty command.
    fn new() -> Self {
        Command {
            args: Vec::new(),
            stdin_file: None,
            stdout_file: None,
            stdout_append: None,
            stderr_file: None,
            background: false,
        }
    }
}

/// Parse a command line string into a Pipeline.
pub fn parse_line(line: &str) -> Result<Pipeline, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(Pipeline {
            commands: Vec::new(),
            conditional: None,
        });
    }

    // Tokenize the line
    let tokens = tokenize(trimmed)?;

    // Check for conditional operators at the top level
    // For now, just return a simple pipeline
    build_pipeline(&tokens)
}

/// Tokenize a command line into a list of tokens, respecting quotes.
fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if escape {
            current.push(c);
            escape = false;
            i += 1;
            continue;
        }

        if c == '\\' {
            escape = true;
            i += 1;
            continue;
        }

        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                current.push(c);
            }
            i += 1;
            continue;
        }

        if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' && i + 1 < chars.len() {
                // In double quotes, only some escapes are recognized
                let next = chars[i + 1];
                if next == '"' || next == '\\' || next == '$' || next == '`' {
                    current.push(next);
                    i += 2;
                    continue;
                }
                current.push(c);
            } else {
                current.push(c);
            }
            i += 1;
            continue;
        }

        // Check for combined operators like 2> (stderr redirect)
        if c == '2' && i + 1 < chars.len() && chars[i + 1] == '>' {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            tokens.push("2>".to_string());
            i += 2;
            continue;
        }

        match c {
            '\'' => {
                in_single = true;
                i += 1;
            }
            '"' => {
                in_double = true;
                i += 1;
            }
            ' ' | '\t' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                i += 1;
            }
            '|' | '>' | '<' | '&' | ';' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                // Check for multi-character operators
                if (c == '>' && i + 1 < chars.len() && chars[i + 1] == '>')
                    || (c == '&' && i + 1 < chars.len() && chars[i + 1] == '&')
                    || (c == '|' && i + 1 < chars.len() && chars[i + 1] == '|')
                {
                    let op: String = chars[i..=i + 1].iter().collect();
                    tokens.push(op);
                    i += 2;
                } else {
                    tokens.push(c.to_string());
                    i += 1;
                }
            }
            _ => {
                current.push(c);
                i += 1;
            }
        }
    }

    if in_single || in_double {
        return Err("unterminated quote".to_string());
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Build a Pipeline from a list of tokens.
fn build_pipeline(tokens: &[String]) -> Result<Pipeline, String> {
    if tokens.is_empty() {
        return Ok(Pipeline {
            commands: Vec::new(),
            conditional: None,
        });
    }

    let mut pipeline = Pipeline {
        commands: Vec::new(),
        conditional: None,
    };

    let mut cmd = Command::new();
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];

        match token.as_str() {
            "|" => {
                if cmd.args.is_empty() {
                    return Err("empty command before pipe".to_string());
                }
                pipeline.commands.push(cmd);
                cmd = Command::new();
                i += 1;
                // Trailing pipe (no command after it)
                if i >= tokens.len() {
                    return Err("trailing pipe".to_string());
                }
            }
            ">" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("missing filename after >".to_string());
                }
                cmd.stdout_file = Some(tokens[i].clone());
                i += 1;
            }
            ">>" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("missing filename after >>".to_string());
                }
                cmd.stdout_append = Some(tokens[i].clone());
                i += 1;
            }
            "<" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("missing filename after <".to_string());
                }
                cmd.stdin_file = Some(tokens[i].clone());
                i += 1;
            }
            "2>" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("missing filename after 2>".to_string());
                }
                cmd.stderr_file = Some(tokens[i].clone());
                i += 1;
            }
            "&" => {
                cmd.background = true;
                i += 1;
                // Push this command and stop (nothing after & in simple case)
                if !cmd.args.is_empty() {
                    pipeline.commands.push(cmd);
                    cmd = Command::new();
                }
            }
            "&&" => {
                if cmd.args.is_empty() {
                    return Err("empty command before &&".to_string());
                }
                pipeline.commands.push(cmd);
                cmd = Command::new();
                pipeline.conditional = Some(Conditional::And);
                i += 1;
            }
            "||" => {
                if cmd.args.is_empty() {
                    return Err("empty command before ||".to_string());
                }
                pipeline.commands.push(cmd);
                cmd = Command::new();
                pipeline.conditional = Some(Conditional::Or);
                i += 1;
            }
            ";" => {
                // Semicolons are treated like end of input for now
                // (simple shell — sequential execution without pipes)
                if !cmd.args.is_empty() {
                    pipeline.commands.push(cmd);
                    cmd = Command::new();
                }
                i += 1;
            }
            _ => {
                cmd.args.push(token.clone());
                i += 1;
            }
        }
    }

    // Push the last command
    if !cmd.args.is_empty() {
        pipeline.commands.push(cmd);
    }

    Ok(pipeline)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let p = parse_line("ls -la /tmp").unwrap();
        assert_eq!(p.commands.len(), 1);
        assert_eq!(p.commands[0].args, vec!["ls", "-la", "/tmp"]);
    }

    #[test]
    fn test_empty_line() {
        let p = parse_line("").unwrap();
        assert!(p.commands.is_empty());
    }

    #[test]
    fn test_whitespace_line() {
        let p = parse_line("   ").unwrap();
        assert!(p.commands.is_empty());
    }

    #[test]
    fn test_pipeline() {
        let p = parse_line("ls -la | grep foo").unwrap();
        assert_eq!(p.commands.len(), 2);
        assert_eq!(p.commands[0].args, vec!["ls", "-la"]);
        assert_eq!(p.commands[1].args, vec!["grep", "foo"]);
    }

    #[test]
    fn test_stdout_redirect() {
        let p = parse_line("echo hello > out.txt").unwrap();
        assert_eq!(p.commands.len(), 1);
        assert_eq!(p.commands[0].args, vec!["echo", "hello"]);
        assert_eq!(p.commands[0].stdout_file, Some("out.txt".to_string()));
    }

    #[test]
    fn test_stdout_append() {
        let p = parse_line("echo hello >> out.txt").unwrap();
        assert_eq!(p.commands[0].stdout_append, Some("out.txt".to_string()));
    }

    #[test]
    fn test_stdin_redirect() {
        let p = parse_line("cat < input.txt").unwrap();
        assert_eq!(p.commands[0].stdin_file, Some("input.txt".to_string()));
    }

    #[test]
    fn test_background() {
        let p = parse_line("sleep 10 &").unwrap();
        assert!(p.commands[0].background);
    }

    #[test]
    fn test_quoted_string() {
        let p = parse_line("echo \"hello world\" 'single quoted'").unwrap();
        assert_eq!(p.commands[0].args, vec!["echo", "hello world", "single quoted"]);
    }

    #[test]
    fn test_missing_redirect_file() {
        let result = parse_line("cat >");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_pipe() {
        let result = parse_line("ls |");
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_redirects() {
        let p = parse_line("cmd < in.txt > out.txt 2> err.txt").unwrap();
        assert_eq!(p.commands[0].stdin_file, Some("in.txt".to_string()));
        assert_eq!(p.commands[0].stdout_file, Some("out.txt".to_string()));
        assert_eq!(p.commands[0].stderr_file, Some("err.txt".to_string()));
    }

    #[test]
    fn test_conditional_and() {
        let p = parse_line("make && make install").unwrap();
        assert_eq!(p.commands.len(), 2);
        assert_eq!(p.conditional, Some(Conditional::And));
    }

    #[test]
    fn test_conditional_or() {
        let p = parse_line("false || echo ok").unwrap();
        assert_eq!(p.commands.len(), 2);
        assert_eq!(p.conditional, Some(Conditional::Or));
    }

    #[test]
    fn test_pipeline_with_redirect() {
        let p = parse_line("cat file.txt | grep error > errors.log").unwrap();
        assert_eq!(p.commands.len(), 2);
        assert_eq!(p.commands[1].stdout_file, Some("errors.log".to_string()));
    }

    #[test]
    fn test_unterminated_single_quote() {
        let result = parse_line("echo 'hello");
        assert!(result.is_err());
    }

    #[test]
    fn test_unterminated_double_quote() {
        let result = parse_line("echo \"hello");
        assert!(result.is_err());
    }

    #[test]
    fn test_escape_character() {
        let p = parse_line("echo hello\\ world").unwrap();
        assert_eq!(p.commands[0].args, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_multiple_commands_semicolon() {
        let p = parse_line("echo a; echo b").unwrap();
        assert_eq!(p.commands.len(), 2);
        assert_eq!(p.commands[0].args, vec!["echo", "a"]);
        assert_eq!(p.commands[1].args, vec!["echo", "b"]);
    }
}
