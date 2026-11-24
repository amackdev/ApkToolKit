//! Patch smali at method granularity.

use crate::error::Result;

/// Find a method definition line in smali text.
///
/// Returns the line index of the `.method` directive for the given method name,
/// or `None` if not found.
pub fn find_method_line(smali: &str, method_name: &str) -> Option<usize> {
    smali.lines().position(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(".method") && trimmed.contains(method_name)
    })
}

/// Find the closing `.end method` line for a method starting at `start_line`.
///
/// Returns the line index of `.end method`, or `None` if not found.
pub fn find_method_end(smali: &str, start_line: usize) -> Option<usize> {
    let lines: Vec<&str> = smali.lines().collect();
    let mut depth: i32 = 0;

    for (i, line) in lines.iter().enumerate().skip(start_line) {
        let trimmed = line.trim();
        if trimmed.starts_with(".method") {
            depth += 1;
        } else if trimmed == ".end method" {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }

    None
}

/// Replace a method's body in smali text.
///
/// Takes the original smali text, the method name, and the new smali body
/// (everything between `.method ...` and `.end method`, exclusive).
/// Returns the modified smali text.
pub fn replace_method_body(
    smali: &str,
    method_name: &str,
    new_body: &str,
) -> Result<String> {
    let start = find_method_line(smali, method_name)
        .ok_or_else(|| crate::error::Error::MethodNotFound(method_name.to_string()))?;
    let end = find_method_end(smali, start)
        .ok_or_else(|| crate::error::Error::Other(format!("No .end method for {method_name}")))?;

    let lines: Vec<&str> = smali.lines().collect();

    // Get the .method declaration line
    let method_decl = lines[start];

    // Build the replacement: declaration + new body + .end method
    let mut result = String::new();

    // Lines before the method
    for line in &lines[..start] {
        result.push_str(line);
        result.push('\n');
    }

    // New method
    result.push_str(method_decl);
    result.push('\n');

    for body_line in new_body.lines() {
        result.push_str(body_line);
        result.push('\n');
    }

    result.push_str(".end method");
    result.push('\n');

    // Lines after .end method
    for line in &lines[end + 1..] {
        result.push_str(line);
        result.push('\n');
    }

    Ok(result)
}

/// Check if smali text contains a given method.
pub fn has_method(smali: &str, method_name: &str) -> bool {
    find_method_line(smali, method_name).is_some()
}
