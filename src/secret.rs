#[derive(Debug, Clone)]
pub struct RedactionResult {
    pub content: String,
    pub redactions: usize,
}

pub fn redact_secrets(input: &str) -> RedactionResult {
    let mut output = String::with_capacity(input.len());
    let mut redactions = 0;
    let mut in_private_key = false;
    for line in input.lines() {
        if !in_private_key && line.contains("-----BEGIN ") && line.contains("PRIVATE KEY-----") {
            output.push_str("[REDACTED PRIVATE KEY BLOCK]");
            output.push('\n');
            redactions += 1;
            in_private_key = true;
            continue;
        }
        if in_private_key {
            if line.contains("-----END ") && line.contains("PRIVATE KEY-----") {
                in_private_key = false;
            }
            continue;
        }
        let (redacted, count) = redact_line(line);
        output.push_str(&redacted);
        output.push('\n');
        redactions += count;
    }
    if !input.ends_with('\n') && output.ends_with('\n') {
        output.pop();
    }
    RedactionResult {
        content: output,
        redactions,
    }
}

fn redact_line(line: &str) -> (String, usize) {

    let mut output = line.to_string();
    let mut count = 0;
    for prefix in ["sk-", "sk-proj-", "ghp_", "github_pat_", "AKIA", "ASIA"] {
        loop {
            let Some(start) = find_token_prefix(&output, prefix) else {
                break;
            };
            let end = token_end(&output, start);
            if end.saturating_sub(start) < prefix.len() + 8 {
                break;
            }
            output.replace_range(start..end, "[REDACTED_SECRET]");
            count += 1;
        }
    }

    let lower = output.to_ascii_lowercase();
    for marker in [
        "api_key",
        "apikey",
        "secret_key",
        "client_secret",
        "access_token",
        "auth_token",
        "password",
        "passwd",
    ] {
        if let Some(position) = lower.find(marker) {
            if let Some(separator_offset) = output[position..].find(|character| character == '=' || character == ':') {
                let value_start = position + separator_offset + 1;
                let tail = output[value_start..].trim();
                if tail.len() >= 8 && !tail.starts_with("[REDACTED") {
                    output.replace_range(value_start.., " [REDACTED_SECRET]");
                    count += 1;
                    break;
                }
            }
        }
    }
    (output, count)
}

fn find_token_prefix(value: &str, prefix: &str) -> Option<usize> {
    value.find(prefix)
}

fn token_end(value: &str, start: usize) -> usize {
    let bytes = value.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            index += 1;
        } else {
            break;
        }
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_tokens() {
        let result = redact_secrets("OPENAI_API_KEY=sk-proj-1234567890abcdefgh\nlet x = 1;");
        assert_eq!(result.redactions, 1);
        assert!(!result.content.contains("1234567890"));
    }

    #[test]
    fn redacts_complete_private_key_blocks() {
        let result = redact_secrets(
            "before\n-----BEGIN PRIVATE KEY-----\nvery-secret-material\n-----END PRIVATE KEY-----\nafter",
        );
        assert_eq!(result.redactions, 1);
        assert!(!result.content.contains("very-secret-material"));
        assert!(result.content.contains("before"));
        assert!(result.content.contains("after"));
    }
}
