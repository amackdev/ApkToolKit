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

/// Get the body lines of a method (everything between `.method ...` and `.end method`).
fn method_body_range(smali: &str, method_name: &str) -> Result<(usize, usize)> {
    let start = find_method_line(smali, method_name)
        .ok_or_else(|| crate::error::Error::MethodNotFound(method_name.to_string()))?;
    let end = find_method_end(smali, start)
        .ok_or_else(|| crate::error::Error::Other(format!("No .end method for {method_name}")))?;
    Ok((start + 1, end)) // exclusive of .method, exclusive of .end method
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
    let (body_start, body_end) = method_body_range(smali, method_name)?;
    let lines: Vec<&str> = smali.lines().collect();

    // Get the .method declaration line
    let method_decl = lines[body_start - 1];

    // Build the replacement: declaration + new body + .end method
    let mut result = String::new();

    // Lines before the method
    for line in &lines[..body_start - 1] {
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
    for line in &lines[body_end + 1..] {
        result.push_str(line);
        result.push('\n');
    }

    Ok(result)
}

/// Check if smali text contains a given method.
pub fn has_method(smali: &str, method_name: &str) -> bool {
    find_method_line(smali, method_name).is_some()
}

/// Replace `old` with `new` inside a method's body only.
///
/// Like `sed -i 's/old/new/g'` but scoped to one method so it cannot
/// corrupt unrelated code elsewhere in the class.
pub fn replace_text_in_method(
    smali: &str,
    method_name: &str,
    old: &str,
    new: &str,
) -> Result<String> {
    let (body_start, body_end) = method_body_range(smali, method_name)?;
    let lines: Vec<&str> = smali.lines().collect();

    let mut result = String::new();

    // Lines before the method body
    for line in &lines[..body_start] {
        result.push_str(line);
        result.push('\n');
    }

    // Method body with replacements
    for line in &lines[body_start..body_end] {
        result.push_str(&line.replace(old, new));
        result.push('\n');
    }

    // .end method and everything after
    for line in &lines[body_end..] {
        result.push_str(line);
        result.push('\n');
    }

    Ok(result)
}

/// Insert `text` after the first line in a method whose trimmed content
/// contains `anchor`. The inserted text is placed on its own line(s).
pub fn insert_after_line(
    smali: &str,
    method_name: &str,
    anchor: &str,
    text: &str,
) -> Result<String> {
    let (body_start, body_end) = method_body_range(smali, method_name)?;
    let lines: Vec<&str> = smali.lines().collect();

    let mut insert_idx: Option<usize> = None;
    for (i, line) in lines[body_start..body_end].iter().enumerate() {
        if line.contains(anchor) {
            insert_idx = Some(body_start + i);
            break;
        }
    }
    let insert_idx = insert_idx.ok_or_else(|| {
        crate::error::Error::Other(format!("Anchor '{anchor}' not found in method {method_name}"))
    })?;

    let mut result = String::new();

    // Lines up to and including the anchor line
    for line in &lines[..=insert_idx] {
        result.push_str(line);
        result.push('\n');
    }

    // Inserted text
    for text_line in text.lines() {
        result.push_str(text_line);
        result.push('\n');
    }

    // Remaining lines
    for line in &lines[insert_idx + 1..] {
        result.push_str(line);
        result.push('\n');
    }

    Ok(result)
}

/// Insert `text` before the first line in a method whose trimmed content
/// contains `anchor`.
pub fn insert_before_line(
    smali: &str,
    method_name: &str,
    anchor: &str,
    text: &str,
) -> Result<String> {
    let (body_start, body_end) = method_body_range(smali, method_name)?;
    let lines: Vec<&str> = smali.lines().collect();

    let mut insert_idx: Option<usize> = None;
    for (i, line) in lines[body_start..body_end].iter().enumerate() {
        if line.contains(anchor) {
            insert_idx = Some(body_start + i);
            break;
        }
    }
    let insert_idx = insert_idx.ok_or_else(|| {
        crate::error::Error::Other(format!("Anchor '{anchor}' not found in method {method_name}"))
    })?;

    let mut result = String::new();

    // Lines before the anchor
    for line in &lines[..insert_idx] {
        result.push_str(line);
        result.push('\n');
    }

    // Inserted text
    for text_line in text.lines() {
        result.push_str(text_line);
        result.push('\n');
    }

    // Anchor line and everything after
    for line in &lines[insert_idx..] {
        result.push_str(line);
        result.push('\n');
    }

    Ok(result)
}

/// Remove all lines in a method whose trimmed content contains `pattern`.
pub fn remove_lines_matching(
    smali: &str,
    method_name: &str,
    pattern: &str,
) -> Result<String> {
    let (body_start, body_end) = method_body_range(smali, method_name)?;
    let lines: Vec<&str> = smali.lines().collect();

    let mut result = String::new();

    // Lines before the method body
    for line in &lines[..body_start] {
        result.push_str(line);
        result.push('\n');
    }

    // Method body, skipping matching lines
    for line in &lines[body_start..body_end] {
        if !line.contains(pattern) {
            result.push_str(line);
            result.push('\n');
        }
    }

    // .end method and everything after
    for line in &lines[body_end..] {
        result.push_str(line);
        result.push('\n');
    }

    Ok(result)
}

/// Append a new method to the end of a class smali (just before the last
/// `.end method` of the class, or at end of file if none).
///
/// `method_decl` is the full `.method ...` declaration line.
/// `body` is the method body (without `.method`/`.end method`).
pub fn append_method(smali: &str, method_decl: &str, body: &str) -> Result<String> {
    let lines: Vec<&str> = smali.lines().collect();

    // Find the last `.end method` in the file
    let mut last_end: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == ".end method" {
            last_end = Some(i);
        }
    }

    let mut result = String::new();

    match last_end {
        Some(idx) => {
            // Everything up to and including the last .end method
            for line in &lines[..=idx] {
                result.push_str(line);
                result.push('\n');
            }
        }
        None => {
            // No .end method found, append at end of file
            for line in &lines {
                result.push_str(line);
                result.push('\n');
            }
        }
    }

    // New method
    result.push_str(method_decl);
    result.push('\n');
    for body_line in body.lines() {
        result.push_str(body_line);
        result.push('\n');
    }
    result.push_str(".end method");
    result.push('\n');

    Ok(result)
}
