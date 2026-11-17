//! Guessing a dev server's port from its log output (T4.9). Only a hint: M5 reads
//! the real port table. Matches addresses such as `localhost:3000`,
//! `http://127.0.0.1:5173/` and `[::1]:8080`, and phrases like "listening on port 4000".

const HOSTS: [&str; 5] = ["localhost:", "127.0.0.1:", "0.0.0.0:", "[::1]:", "[::]:"];

pub fn find_port(line: &str) -> Option<u16> {
    let lower = line.to_ascii_lowercase();
    for host in HOSTS {
        let mut rest = lower.as_str();
        while let Some(i) = rest.find(host) {
            rest = &rest[i + host.len()..];
            if let Some(port) = leading_port(rest) {
                return Some(port);
            }
        }
    }
    // "on port 4000", "port: 4000" -- "port" must be a whole word, not "report".
    for (i, _) in lower.match_indices("port") {
        let starts_word = lower[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        let after = lower[i + 4..].trim_start_matches([' ', ':', '=']);
        if starts_word && let Some(port) = leading_port(after) {
            return Some(port);
        }
    }
    None
}

fn leading_port(s: &str) -> Option<u16> {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 2 || digits.len() > 5 {
        return None;
    }
    digits.parse::<u16>().ok().filter(|p| *p > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_dev_server_banners() {
        assert_eq!(
            find_port("   - Local:        http://localhost:3000"),
            Some(3000)
        );
        assert_eq!(
            find_port("  ➜  Local:   http://127.0.0.1:5173/"),
            Some(5173)
        );
        assert_eq!(find_port("Listening on http://[::1]:8080"), Some(8080));
        assert_eq!(find_port("server listening on 0.0.0.0:4000"), Some(4000));
        assert_eq!(find_port("Server started on port 4321"), Some(4321));
        assert_eq!(find_port("PORT=9000"), Some(9000));
    }

    #[test]
    fn non_ports_are_ignored() {
        assert_eq!(find_port("compiled 3 modules"), None);
        assert_eq!(find_port("localhost:abc"), None);
        assert_eq!(find_port("report 2024 ready"), None);
        assert_eq!(find_port("localhost:99999"), None);
    }

    #[test]
    fn keeps_looking_after_a_bad_match() {
        assert_eq!(find_port("localhost:x then localhost:3001"), Some(3001));
    }
}
