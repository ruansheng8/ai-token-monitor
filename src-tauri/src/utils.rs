use std::path::Path;

/// Decode Claude session storage path to actual project path
///
/// Claude stores sessions in ~/.claude/projects/ with the project path encoded:
/// - `/Users/jack/.claude/projects/-Users-jack-my-project` → `/Users/jack/my-project`
/// - `C:\Users\jack\.claude\projects\-d-my-project` → `D:\my-project`
pub fn decode_project_path(session_storage_path: &str) -> String {
    // 1. Try reading originalPath from sessions-index.json (most reliable)
    let index_path = Path::new(session_storage_path).join("sessions-index.json");
    if let Ok(content) = std::fs::read_to_string(&index_path) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(original) = parsed.get("originalPath").and_then(|v| v.as_str()) {
                if !original.is_empty() && Path::new(original).is_absolute() {
                    return original.to_string();
                }
            }
        }
    }

    // 2. Fallback: decode from encoded directory name
    const MARKER: &str = ".claude/projects/";
    let encoded = if let Some(marker_pos) = session_storage_path.find(MARKER) {
        &session_storage_path[marker_pos + MARKER.len()..]
    } else {
        Path::new(session_storage_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(session_storage_path)
    };

    if let Some(stripped) = encoded.strip_prefix('-') {
        // Try filesystem-based decoding (recursive)
        if let Some(path) = decode_with_filesystem_check(stripped) {
            return path;
        }

        // Fallback: use heuristic decoding
        let parts: Vec<&str> = encoded.splitn(4, '-').collect();
        if parts.len() >= 4 {
            // On Windows, if parts[1] is a drive letter (1 char), construct drive format
            if parts[1].len() == 1 && parts[1].chars().next().unwrap().is_ascii_alphabetic() {
                return format!("{}:/{}/{}", parts[1], parts[2], parts[3]);
            }
            return format!("/{}/{}/{}", parts[1], parts[2], parts[3]);
        } else if parts.len() == 3 {
            if parts[1].len() == 1 && parts[1].chars().next().unwrap().is_ascii_alphabetic() {
                return format!("{}:/{}", parts[1], parts[2]);
            }
            return format!("/{}/{}", parts[1], parts[2]);
        } else if parts.len() == 2 {
            if parts[1].len() == 1 && parts[1].chars().next().unwrap().is_ascii_alphabetic() {
                return format!("{}:/", parts[1]);
            }
            return format!("/{}", parts[1]);
        }
    }

    session_storage_path.to_string()
}

/// Decode path by checking filesystem existence at each possible split point
fn decode_with_filesystem_check(encoded: &str) -> Option<String> {
    decode_recursive(encoded, "")
}

fn decode_recursive(encoded: &str, base_path: &str) -> Option<String> {
    decode_recursive_inner(encoded, base_path, 0)
}

fn decode_recursive_inner(encoded: &str, base_path: &str, depth: usize) -> Option<String> {
    if depth > 20 {
        return None;
    }
    if encoded.is_empty() {
        if !base_path.is_empty() && Path::new(base_path).exists() {
            return Some(base_path.to_string());
        }
        return None;
    }

    let hyphen_positions: Vec<usize> = encoded
        .char_indices()
        .filter(|(_, c)| *c == '-')
        .map(|(i, _)| i)
        .collect();

    // Try each hyphen as a potential path separator
    for &pos in &hyphen_positions {
        let segment = &encoded[..pos];
        if segment.is_empty() {
            continue;
        }

        let candidate = if base_path.is_empty() {
            // On Windows, handle drive letters (e.g. segment "d" -> "d:/")
            if cfg!(windows) && segment.len() == 1 && segment.chars().next().unwrap().is_ascii_alphabetic() {
                format!("{}:/", segment)
            } else {
                format!("/{segment}")
            }
        } else {
            // Standard path join with normal slash representation
            let mut base = base_path.to_string();
            if !base.ends_with('/') && !base.ends_with('\\') {
                base.push('/');
            }
            base.push_str(segment);
            base
        };

        // Use symlink_metadata to avoid following symlinks
        let is_real_dir = std::fs::symlink_metadata(&candidate)
            .map(|m| m.file_type().is_dir())
            .unwrap_or(false);

        if is_real_dir {
            let remaining = &encoded[pos + 1..];
            if remaining.is_empty() {
                return Some(candidate);
            }

            // First try: remaining as a single leaf (no more splitting needed)
            let mut full_path = candidate.clone();
            if !full_path.ends_with('/') && !full_path.ends_with('\\') {
                full_path.push('/');
            }
            full_path.push_str(remaining);

            let full_path_is_real = std::fs::symlink_metadata(&full_path)
                .map(|m| !m.file_type().is_symlink())
                .unwrap_or(false);
            if full_path_is_real {
                return Some(full_path);
            }

            // Recurse: remaining may itself contain hyphens that are path separators
            if let result @ Some(_) = decode_recursive_inner(remaining, &candidate, depth + 1) {
                return result;
            }
        }
    }

    // No hyphen worked as separator — treat entire encoded as a single segment
    if !base_path.is_empty() {
        let mut full_path = base_path.to_string();
        if !full_path.ends_with('/') && !full_path.ends_with('\\') {
            full_path.push('/');
        }
        full_path.push_str(encoded);

        if Path::new(&full_path).exists() {
            return Some(full_path);
        }
    }

    None
}

/// Extract project name from the raw encoded name or path
pub fn extract_project_name(raw_project_name: &str) -> String {
    // If it starts with '-', it's a Claude encoded project folder
    if raw_project_name.starts_with('-') {
        let decoded = decode_project_path(raw_project_name);
        if let Some(leaf) = Path::new(&decoded).file_name() {
            return leaf.to_string_lossy().to_string();
        }
        
        // Fallback: original heuristic
        let parts: Vec<&str> = raw_project_name.splitn(4, '-').collect();
        if parts.len() == 4 {
            return parts[3].to_string();
        }
    }
    
    // If it's a full path, extract the file name
    let path = Path::new(raw_project_name);
    if let Some(leaf) = path.file_name() {
        return leaf.to_string_lossy().to_string();
    }

    raw_project_name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_project_path_fallback() {
        // Without drive letter (Unix style fallback)
        assert_eq!(
            decode_project_path("-Users-jack-my-project"),
            "/Users/jack/my-project"
        );

        // With drive letter (Windows style fallback)
        assert_eq!(
            decode_project_path("-d-VibeCoding-ai-token-monitor"),
            "d:/VibeCoding/ai-token-monitor"
        );
    }

    #[test]
    fn test_extract_project_name() {
        // Encoded style
        assert_eq!(extract_project_name("-Users-jack-my-project"), "my-project");
        assert_eq!(extract_project_name("-d-VibeCoding-ai-token-monitor"), "ai-token-monitor");

        // Normal path style
        assert_eq!(extract_project_name("/Users/jack/projects/my-project"), "my-project");
        assert_eq!(extract_project_name("C:\\Users\\jack\\my-project"), "my-project");

        // Plain string style
        assert_eq!(extract_project_name("simple-project"), "simple-project");
    }
}
