use std::{fmt, io::Read, ops::Deref, path::Path, str::FromStr};

use anyhow::{Result, anyhow};
use zeroize::Zeroizing;

pub(crate) const SECRET_INPUT_MAX_BYTES: usize = 16 * 1024;

/// A passphrase that is redacted when formatted and zeroized when dropped.
#[derive(Clone)]
pub(crate) struct Secret(Zeroizing<String>);

impl Secret {
    fn unchecked(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    fn validated(self, label: &str) -> Result<Self> {
        validate_secret(&self.0, label)?;
        Ok(self)
    }

    fn from_bytes(mut bytes: Zeroizing<Vec<u8>>, label: &str) -> Result<Self> {
        remove_one_trailing_line_ending_bytes(&mut bytes);
        if bytes.len() > SECRET_INPUT_MAX_BYTES {
            return Err(anyhow!(
                "{label} is too large (maximum {SECRET_INPUT_MAX_BYTES} bytes)"
            ));
        }
        let value = std::str::from_utf8(&bytes)
            .map_err(|_| anyhow!("{label} must be valid UTF-8"))?
            .to_owned();
        validate_secret(&value, label)?;
        Ok(Self::unchecked(value))
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl Deref for Secret {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.expose()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret([REDACTED])")
    }
}

// Parsing is intentionally infallible. Validation happens after argument
// parsing so validation errors never interpolate the supplied value.
impl FromStr for Secret {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self::unchecked(value.to_owned()))
    }
}

pub(crate) trait SecretPrompter {
    fn prompt(&mut self, prompt: &str) -> Result<String>;
}

pub(crate) struct TerminalPrompter;

impl SecretPrompter for TerminalPrompter {
    fn prompt(&mut self, prompt: &str) -> Result<String> {
        rpassword::prompt_password(prompt)
            .map_err(|_| anyhow!("could not read a passphrase from the terminal"))
    }
}

pub(crate) fn resolve_identity_passphrase<R: Read, P: SecretPrompter>(
    direct: Option<Secret>,
    prompt: bool,
    stdin: bool,
    file: Option<&Path>,
    reader: &mut R,
    prompter: &mut P,
) -> Result<Option<Secret>> {
    let selected = usize::from(direct.is_some())
        + usize::from(prompt)
        + usize::from(stdin)
        + usize::from(file.is_some());
    if selected > 1 {
        return Err(anyhow!(
            "choose exactly one identity passphrase source: prompt, standard input, or file"
        ));
    }

    if let Some(secret) = direct {
        validate_secret_length(secret.expose(), "identity passphrase")?;
        if secret.expose().trim().is_empty() {
            return Ok(None);
        }
        return secret.validated("identity passphrase").map(Some);
    }
    if prompt {
        return prompt_once(prompter, "Identity passphrase: ", "identity passphrase").map(Some);
    }
    if stdin {
        return read_secret(reader, "identity passphrase").map(Some);
    }
    if let Some(path) = file {
        return read_secret_file(path, "identity passphrase").map(Some);
    }
    Ok(None)
}

pub(crate) fn resolve_recovery_passphrase<R: Read, P: SecretPrompter>(
    direct: Option<Secret>,
    stdin: bool,
    file: Option<&Path>,
    confirm_prompt: bool,
    reader: &mut R,
    prompter: &mut P,
) -> Result<Secret> {
    let selected = usize::from(direct.is_some()) + usize::from(stdin) + usize::from(file.is_some());
    if selected > 1 {
        return Err(anyhow!(
            "choose exactly one recovery passphrase source: standard input or file"
        ));
    }

    if let Some(secret) = direct {
        return secret.validated("recovery passphrase");
    }
    if stdin {
        return read_secret(reader, "recovery passphrase");
    }
    if let Some(path) = file {
        return read_secret_file(path, "recovery passphrase");
    }

    let secret = prompt_once(
        prompter,
        "Recovery bundle passphrase: ",
        "recovery passphrase",
    )?;
    if confirm_prompt {
        let confirmation = prompt_once(
            prompter,
            "Confirm recovery bundle passphrase: ",
            "recovery passphrase confirmation",
        )?;
        if secret.expose() != confirmation.expose() {
            return Err(anyhow!("recovery passphrase confirmation did not match"));
        }
    }
    Ok(secret)
}

fn prompt_once<P: SecretPrompter>(prompter: &mut P, prompt: &str, label: &str) -> Result<Secret> {
    Secret::unchecked(prompter.prompt(prompt)?).validated(label)
}

fn read_secret<R: Read>(reader: &mut R, label: &str) -> Result<Secret> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(256));
    reader
        .take((SECRET_INPUT_MAX_BYTES + 3) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow!("could not read {label} from standard input"))?;
    if bytes.len() > SECRET_INPUT_MAX_BYTES + 2 {
        return Err(anyhow!(
            "{label} is too large (maximum {SECRET_INPUT_MAX_BYTES} bytes)"
        ));
    }
    Secret::from_bytes(bytes, label)
}

#[cfg(unix)]
fn read_secret_file(path: &Path, label: &str) -> Result<Secret> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| anyhow!("could not securely open {label} file"))?;
    let metadata = file
        .metadata()
        .map_err(|_| anyhow!("could not inspect {label} file"))?;
    if !metadata.is_file() {
        return Err(anyhow!("{label} file must be a regular file"));
    }
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err(anyhow!("{label} file must be owned by the current user"));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(anyhow!(
            "{label} file permissions must not grant access to group or other users"
        ));
    }

    read_secret(&mut file.take((SECRET_INPUT_MAX_BYTES + 3) as u64), label)
        .map_err(|error| anyhow!("could not read a valid {label} from its file: {error}"))
}

#[cfg(not(unix))]
fn read_secret_file(_path: &Path, label: &str) -> Result<Secret> {
    Err(anyhow!(
        "{label} file input is unavailable because secure file validation is unsupported on this platform"
    ))
}

fn validate_secret(value: &str, label: &str) -> Result<()> {
    validate_secret_length(value, label)?;
    if value.trim().is_empty() {
        return Err(anyhow!("{label} cannot be blank"));
    }
    Ok(())
}

fn validate_secret_length(value: &str, label: &str) -> Result<()> {
    if value.len() > SECRET_INPUT_MAX_BYTES {
        return Err(anyhow!(
            "{label} is too large (maximum {SECRET_INPUT_MAX_BYTES} bytes)"
        ));
    }
    Ok(())
}

fn remove_one_trailing_line_ending_bytes(value: &mut Vec<u8>) {
    if value.ends_with(b"\r\n") {
        value.truncate(value.len() - 2);
    } else if value.ends_with(b"\n") {
        value.truncate(value.len() - 1);
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, io::Cursor};

    use super::*;

    struct FakePrompter {
        answers: VecDeque<String>,
        prompts: Vec<String>,
    }

    impl FakePrompter {
        fn new(answers: impl IntoIterator<Item = &'static str>) -> Self {
            Self {
                answers: answers.into_iter().map(str::to_owned).collect(),
                prompts: Vec::new(),
            }
        }
    }

    impl SecretPrompter for FakePrompter {
        fn prompt(&mut self, prompt: &str) -> Result<String> {
            self.prompts.push(prompt.to_owned());
            self.answers
                .pop_front()
                .ok_or_else(|| anyhow!("test prompt had no answer"))
        }
    }

    #[test]
    fn debug_output_is_always_redacted() {
        let secret = Secret::unchecked("do-not-print-this".to_owned());

        let debug = format!("{secret:?}");

        assert_eq!(debug, "Secret([REDACTED])");
        assert!(!debug.contains("do-not-print-this"));
    }

    #[test]
    fn stdin_preserves_whitespace_and_removes_exactly_one_line_ending() {
        for (input, expected) in [
            ("  secret  \n", "  secret  "),
            ("\tsecret\t\r\n", "\tsecret\t"),
            ("secret\n\n", "secret\n"),
            ("secret\r", "secret\r"),
        ] {
            let mut reader = Cursor::new(input.as_bytes());
            let secret = read_secret(&mut reader, "test passphrase").unwrap();
            assert_eq!(secret.expose(), expected);
        }
    }

    #[test]
    fn direct_and_prompt_sources_preserve_trailing_line_endings() {
        let direct = Secret::unchecked("direct secret\n".to_owned())
            .validated("test passphrase")
            .unwrap();
        assert_eq!(direct.expose(), "direct secret\n");

        let mut prompt = FakePrompter::new(["prompt secret\r\n"]);
        let prompted =
            resolve_identity_passphrase(None, true, false, None, &mut Cursor::new([]), &mut prompt)
                .unwrap()
                .unwrap();
        assert_eq!(prompted.expose(), "prompt secret\r\n");
    }

    #[test]
    fn legacy_blank_identity_argument_remains_equivalent_to_no_passphrase() {
        for direct in ["", " \t\r\n "] {
            let resolved = resolve_identity_passphrase(
                Some(Secret::unchecked(direct.to_owned())),
                false,
                false,
                None,
                &mut Cursor::new([]),
                &mut FakePrompter::new([]),
            )
            .unwrap();
            assert!(resolved.is_none());
        }

        let oversized_blank = " ".repeat(SECRET_INPUT_MAX_BYTES + 1);
        let error = resolve_identity_passphrase(
            Some(Secret::unchecked(oversized_blank.clone())),
            false,
            false,
            None,
            &mut Cursor::new([]),
            &mut FakePrompter::new([]),
        )
        .unwrap_err();
        assert!(error.to_string().contains("too large"));
        assert!(!error.to_string().contains(&oversized_blank));
    }

    #[test]
    fn stdin_rejects_blank_invalid_utf8_and_oversized_values_without_echoing_them() {
        let blank = read_secret(&mut Cursor::new(b" \t\r\n"), "test passphrase").unwrap_err();
        assert!(blank.to_string().contains("cannot be blank"));

        let invalid = read_secret(&mut Cursor::new([0xff, b'\n']), "test passphrase").unwrap_err();
        assert!(invalid.to_string().contains("valid UTF-8"));

        let oversized_value = "s".repeat(SECRET_INPUT_MAX_BYTES + 1);
        let oversized = read_secret(
            &mut Cursor::new(oversized_value.as_bytes()),
            "test passphrase",
        )
        .unwrap_err();
        assert!(oversized.to_string().contains("too large"));
        assert!(!oversized.to_string().contains(&oversized_value));
    }

    #[test]
    fn export_prompt_requires_matching_confirmation() {
        let mut prompt = FakePrompter::new(["correct horse", "correct horse"]);
        let secret =
            resolve_recovery_passphrase(None, false, None, true, &mut Cursor::new([]), &mut prompt)
                .unwrap();
        assert_eq!(secret.expose(), "correct horse");
        assert_eq!(prompt.prompts.len(), 2);

        let mut mismatch = FakePrompter::new(["first secret", "second secret"]);
        let error = resolve_recovery_passphrase(
            None,
            false,
            None,
            true,
            &mut Cursor::new([]),
            &mut mismatch,
        )
        .unwrap_err();
        assert!(error.to_string().contains("did not match"));
        assert!(!error.to_string().contains("first secret"));
        assert!(!error.to_string().contains("second secret"));
    }

    #[test]
    fn import_and_identity_prompts_read_once() {
        let mut recovery_prompt = FakePrompter::new(["recovery secret"]);
        resolve_recovery_passphrase(
            None,
            false,
            None,
            false,
            &mut Cursor::new([]),
            &mut recovery_prompt,
        )
        .unwrap();
        assert_eq!(recovery_prompt.prompts.len(), 1);

        let mut identity_prompt = FakePrompter::new(["identity secret"]);
        resolve_identity_passphrase(
            None,
            true,
            false,
            None,
            &mut Cursor::new([]),
            &mut identity_prompt,
        )
        .unwrap();
        assert_eq!(identity_prompt.prompts.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn file_input_requires_owner_only_regular_non_symlink_file() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let directory = tempfile::tempdir().unwrap();
        let secret_path = directory.path().join("secret");
        std::fs::write(&secret_path, b"  file secret  \r\n").unwrap();
        std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let secret = read_secret_file(&secret_path, "test passphrase").unwrap();
        assert_eq!(secret.expose(), "  file secret  ");

        std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o400)).unwrap();
        let read_only_secret = read_secret_file(&secret_path, "test passphrase").unwrap();
        assert_eq!(read_only_secret.expose(), "  file secret  ");

        std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o640)).unwrap();
        let permissions_error = read_secret_file(&secret_path, "test passphrase").unwrap_err();
        assert!(permissions_error.to_string().contains("permissions"));

        std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let symlink_path = directory.path().join("secret-link");
        symlink(&secret_path, &symlink_path).unwrap();
        let symlink_error = read_secret_file(&symlink_path, "test passphrase").unwrap_err();
        assert!(symlink_error.to_string().contains("securely open"));

        let directory_error = read_secret_file(directory.path(), "test passphrase").unwrap_err();
        assert!(directory_error.to_string().contains("regular file"));

        let oversized_path = directory.path().join("oversized-secret");
        std::fs::write(&oversized_path, vec![b's'; SECRET_INPUT_MAX_BYTES + 3]).unwrap();
        std::fs::set_permissions(&oversized_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let oversized_error = read_secret_file(&oversized_path, "test passphrase").unwrap_err();
        assert!(oversized_error.to_string().contains("too large"));
    }
}
