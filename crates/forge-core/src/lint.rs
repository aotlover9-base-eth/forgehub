//! Danger lints (warnings only, never blocking - ADR 0007 separates
//! these from secret findings). Each warning names the line and says
//! plainly what is dangerous.

use regex::Regex;
use std::sync::OnceLock;

/// The kind of danger a lint reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintKind {
    /// `rm -rf` aimed at `/`, `~`, `$HOME` or a root glob.
    RmRfRoot,
    /// `dd` writing to a block device.
    DdToBlockDevice,
    /// `mkfs` - formatting a filesystem.
    Mkfs,
    /// The classic fork bomb.
    ForkBomb,
    /// `chmod -R 777 /`-style world-writable root.
    Chmod777Root,
    /// Downloading a script and piping it to a shell.
    PipeToShell,
    /// Redirecting output onto a raw block device.
    WriteToBlockDevice,
    /// Any use of `sudo`.
    Sudo,
    /// `eval` of downloaded content.
    EvalDownload,
}

impl LintKind {
    /// Stable machine name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            LintKind::RmRfRoot => "rm_rf_root",
            LintKind::DdToBlockDevice => "dd_to_block_device",
            LintKind::Mkfs => "mkfs",
            LintKind::ForkBomb => "fork_bomb",
            LintKind::Chmod777Root => "chmod_777_root",
            LintKind::PipeToShell => "pipe_to_shell",
            LintKind::WriteToBlockDevice => "write_to_block_device",
            LintKind::Sudo => "sudo",
            LintKind::EvalDownload => "eval_download",
        }
    }

    /// Plain-language explanation shown to the user.
    #[must_use]
    pub fn message(&self) -> &'static str {
        match self {
            LintKind::RmRfRoot => {
                "recursive forced delete aimed at the filesystem root or home directory"
            }
            LintKind::DdToBlockDevice => "dd writing directly to a block device",
            LintKind::Mkfs => "mkfs formats a filesystem and destroys its data",
            LintKind::ForkBomb => "fork bomb: spawns processes until the machine is unusable",
            LintKind::Chmod777Root => "making the filesystem root world-writable",
            LintKind::PipeToShell => {
                "downloads a script and runs it without inspection (curl | sh)"
            }
            LintKind::WriteToBlockDevice => "writing raw output onto a block device",
            LintKind::Sudo => "runs with sudo (administrator privileges)",
            LintKind::EvalDownload => "eval of downloaded content",
        }
    }
}

/// One warning: kind + 1-based line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LintWarning {
    /// What is dangerous.
    pub kind: LintKind,
    /// 1-based line number.
    pub line: usize,
}

struct Patterns {
    /// Candidate `rm <flags> <root-ish target>`; the caller additionally
    /// checks the flags contain both `r` and `f`.
    rm_rf: Regex,
    dd: Regex,
    mkfs: Regex,
    chmod: Regex,
    pipe_shell: Regex,
    dev_write: Regex,
    sudo: Regex,
    eval_download: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        rm_rf: Regex::new(r"\brm((?:\s+-\S+)+)\s+(/|/\*|~/?\*?|\$HOME|\$\{HOME\}|~/)(\s|;|&|$)")
            .expect("static regex"),
        dd: Regex::new(r"\bdd\b[^|;&]*\bof=/dev/(sd|hd|nvme|mmcblk|disk)").expect("static regex"),
        mkfs: Regex::new(r"\bmkfs(\.[a-z0-9]+)?\b").expect("static regex"),
        chmod: Regex::new(r"\bchmod\s+(?:-\S+\s+)*777\s+/(\s|;|&|$)").expect("static regex"),
        pipe_shell: Regex::new(r"\b(curl|wget)\b[^|]*\|\s*(sudo\s+)?(ba|z|fi)?sh\b")
            .expect("static regex"),
        dev_write: Regex::new(r">>?\s*/dev/(sd|hd|nvme|mmcblk|disk)").expect("static regex"),
        sudo: Regex::new(r"(?:^|[;|&]\s*|\s)sudo\s").expect("static regex"),
        eval_download: Regex::new(r"\beval\b[^;]*\$\(\s*(curl|wget)\b").expect("static regex"),
    })
}

/// Lint text (one command or a whole script body). Warnings are sorted
/// by line; a line can carry several warnings.
#[must_use]
pub fn lint(text: &str) -> Vec<LintWarning> {
    let p = patterns();
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let mut push = |kind: LintKind| {
            out.push(LintWarning {
                kind,
                line: line_no,
            });
        };
        if let Some(cap) = p.rm_rf.captures(line) {
            let flags = &cap[1];
            if flags.contains('r') && flags.contains('f') {
                push(LintKind::RmRfRoot);
            }
        }
        if p.dd.is_match(line) {
            push(LintKind::DdToBlockDevice);
        }
        if p.mkfs.is_match(line) {
            push(LintKind::Mkfs);
        }
        if line.contains(":(){") && line.contains(":|:&") {
            push(LintKind::ForkBomb);
        }
        if p.chmod.is_match(line) {
            push(LintKind::Chmod777Root);
        }
        if p.pipe_shell.is_match(line) {
            push(LintKind::PipeToShell);
        }
        if p.dev_write.is_match(line) {
            push(LintKind::WriteToBlockDevice);
        }
        if p.sudo.is_match(line) || line.trim_start().starts_with("sudo ") {
            push(LintKind::Sudo);
        }
        if p.eval_download.is_match(line) {
            push(LintKind::EvalDownload);
        }
    }
    out
}

/// Lint every command/script line of a snippet (line = command index).
#[must_use]
pub fn lint_snippet(snippet: &crate::model::Snippet) -> Vec<LintWarning> {
    let mut out = Vec::new();
    for (i, text) in snippet.all_command_texts().iter().enumerate() {
        for mut w in lint(text) {
            w.line = i + 1;
            out.push(w);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<LintKind> {
        lint(text).iter().map(|w| w.kind).collect()
    }

    #[test]
    fn rm_rf_root_flagged() {
        assert_eq!(kinds("rm -rf /"), vec![LintKind::RmRfRoot]);
        assert_eq!(kinds("rm -rf ~"), vec![LintKind::RmRfRoot]);
        assert_eq!(kinds("rm -rf $HOME"), vec![LintKind::RmRfRoot]);
        assert_eq!(kinds("rm -rf /*"), vec![LintKind::RmRfRoot]);
    }

    #[test]
    fn rm_rf_subdirectory_is_clean() {
        assert_eq!(lint("rm -rf build/ node_modules dist").len(), 0);
        assert_eq!(lint("rm -rf ~/projects/old-app").len(), 0);
        assert_eq!(lint("rm -f file.txt").len(), 0);
    }

    #[test]
    fn dd_to_disk_flagged() {
        assert_eq!(
            kinds("dd if=img.iso of=/dev/sda bs=4M"),
            vec![LintKind::DdToBlockDevice]
        );
        assert_eq!(
            kinds("dd if=/dev/zero of=/dev/nvme0n1"),
            vec![LintKind::DdToBlockDevice]
        );
    }

    #[test]
    fn dd_to_file_is_clean() {
        assert_eq!(lint("dd if=/dev/zero of=out.bin bs=1M count=10").len(), 0);
    }

    #[test]
    fn mkfs_flagged() {
        assert_eq!(kinds("mkfs.ext4 /dev/sdb1"), vec![LintKind::Mkfs]);
        assert_eq!(
            kinds("sudo mkfs /dev/sdb"),
            vec![LintKind::Mkfs, LintKind::Sudo]
        );
    }

    #[test]
    fn fork_bomb_flagged() {
        assert_eq!(kinds(":(){ :|:& };:"), vec![LintKind::ForkBomb]);
    }

    #[test]
    fn chmod_777_root_flagged() {
        assert_eq!(kinds("chmod -R 777 /"), vec![LintKind::Chmod777Root]);
        assert_eq!(kinds("chmod 777 /"), vec![LintKind::Chmod777Root]);
    }

    #[test]
    fn chmod_777_subdir_is_clean() {
        assert_eq!(lint("chmod 777 ./shared").len(), 0);
        assert_eq!(lint("chmod 755 /usr/local/bin/tool").len(), 0);
    }

    #[test]
    fn curl_pipe_sh_flagged() {
        assert_eq!(
            kinds("curl -fsSL https://example.com/install.sh | sh"),
            vec![LintKind::PipeToShell]
        );
        assert_eq!(
            kinds("wget -qO- https://x | sudo bash"),
            vec![LintKind::PipeToShell, LintKind::Sudo]
        );
    }

    #[test]
    fn download_without_pipe_is_clean() {
        assert_eq!(
            lint("curl -fsSL https://example.com/install.sh -o install.sh").len(),
            0
        );
        assert_eq!(lint("curl https://api.example.com | jq .").len(), 0);
    }

    #[test]
    fn write_to_block_device_flagged() {
        assert_eq!(
            kinds("cat img > /dev/sdb"),
            vec![LintKind::WriteToBlockDevice]
        );
    }

    #[test]
    fn dev_null_and_tty_are_clean() {
        assert_eq!(lint("cmd 2>/dev/null").len(), 0);
        assert_eq!(lint("echo hi > /dev/tty").len(), 0);
    }

    #[test]
    fn sudo_flagged_as_warning() {
        assert_eq!(kinds("sudo apt update"), vec![LintKind::Sudo]);
        assert_eq!(
            kinds("apt update && sudo apt upgrade"),
            vec![LintKind::Sudo]
        );
    }

    #[test]
    fn sudo_inside_word_is_clean() {
        assert_eq!(lint("echo pseudocode is fun").len(), 0);
    }

    #[test]
    fn eval_download_flagged() {
        assert_eq!(
            kinds("eval \"$(curl -s https://x)\""),
            vec![LintKind::EvalDownload]
        );
    }

    #[test]
    fn line_numbers_exact() {
        let found = lint("echo ok\nrm -rf /\necho done");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
    }

    #[test]
    fn lint_snippet_uses_command_index() {
        let s = crate::model::Snippet::from_yaml(
            "name: x\nsteps:\n  - echo ok\n  - sudo systemctl restart app\n",
        )
        .unwrap();
        let found = lint_snippet(&s);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
        assert_eq!(found[0].kind, LintKind::Sudo);
    }

    #[test]
    fn every_kind_has_name_and_message() {
        for kind in [
            LintKind::RmRfRoot,
            LintKind::DdToBlockDevice,
            LintKind::Mkfs,
            LintKind::ForkBomb,
            LintKind::Chmod777Root,
            LintKind::PipeToShell,
            LintKind::WriteToBlockDevice,
            LintKind::Sudo,
            LintKind::EvalDownload,
        ] {
            assert_ne!(kind.as_str().len(), 0);
            assert_ne!(kind.message().len(), 0);
        }
    }
}
